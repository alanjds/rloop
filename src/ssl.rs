#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};

use anyhow::Result;
use log::debug;
use mio::{
    Interest,
    net::TcpStream,
};
use pyo3::{IntoPyObjectExt, buffer::PyBuffer, prelude::*, types::PyBytes};
use rustls::{ClientConfig, ServerConfig, ClientConnection, ServerConnection, Stream};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    sync::{Arc, atomic},
};

use crate::{
    event_loop::{EventLoop, EventLoopRunState},
    handles::{BoxedHandle, CBHandle, Handle},
    log::LogExc,
    py::{asyncio_proto_buf, copy_context},
    sock::SocketWrapper,
    utils::syscall,
};

pub(crate) struct SSLTransportState {
    stream: TcpStream,
    tls_connection: TlsConnection,
    write_buf: VecDeque<Box<[u8]>>,
    write_buf_dsize: usize,
    handshake_complete: bool,
}

enum TlsConnection {
    Client(ClientConnection),
    Server(ServerConnection),
}

impl TlsConnection {
    fn as_client(&self) -> Option<&ClientConnection> {
        match self {
            TlsConnection::Client(conn) => Some(conn),
            _ => None,
        }
    }

    fn as_client_mut(&mut self) -> Option<&mut ClientConnection> {
        match self {
            TlsConnection::Client(conn) => Some(conn),
            _ => None,
        }
    }

    fn as_server(&self) -> Option<&ServerConnection> {
        match self {
            TlsConnection::Server(conn) => Some(conn),
            _ => None,
        }
    }

    fn as_server_mut(&mut self) -> Option<&mut ServerConnection> {
        match self {
            TlsConnection::Server(conn) => Some(conn),
            _ => None,
        }
    }

    fn wants_read(&self) -> bool {
        match self {
            TlsConnection::Client(conn) => conn.wants_read(),
            TlsConnection::Server(conn) => conn.wants_read(),
        }
    }

    fn wants_write(&self) -> bool {
        match self {
            TlsConnection::Client(conn) => conn.wants_write(),
            TlsConnection::Server(conn) => conn.wants_write(),
        }
    }

    fn is_handshaking(&self) -> bool {
        match self {
            TlsConnection::Client(conn) => conn.is_handshaking(),
            TlsConnection::Server(conn) => conn.is_handshaking(),
        }
    }

    fn process_new_packets(&mut self) -> Result<(), rustls::Error> {
        match self {
            TlsConnection::Client(conn) => {
                conn.process_new_packets()?;
                Ok(())
            }
            TlsConnection::Server(conn) => {
                conn.process_new_packets()?;
                Ok(())
            }
        }
    }

    fn read_tls(&mut self, rd: &mut dyn Read) -> Result<usize, std::io::Error> {
        match self {
            TlsConnection::Client(ref mut conn) => conn.read_tls(rd),
            TlsConnection::Server(ref mut conn) => conn.read_tls(rd),
        }
    }

    fn write_tls(&mut self, wr: &mut dyn Write) -> Result<usize, std::io::Error> {
        match self {
            TlsConnection::Client(ref mut conn) => conn.write_tls(wr),
            TlsConnection::Server(ref mut conn) => conn.write_tls(wr),
        }
    }

    fn reader(&mut self) -> TlsReader {
        match self {
            TlsConnection::Client(conn) => TlsReader::Client(conn.reader()),
            TlsConnection::Server(conn) => TlsReader::Server(conn.reader()),
        }
    }

    fn writer(&mut self) -> TlsWriter {
        match self {
            TlsConnection::Client(conn) => TlsWriter::Client(conn.writer()),
            TlsConnection::Server(conn) => TlsWriter::Server(conn.writer()),
        }
    }
}

pub(crate) fn init_pymodule(module: &Bound<PyModule>) -> PyResult<()> {
    module.add_class::<SSLTransport>()?;

    Ok(())
}

enum TlsReader<'a> {
    Client(rustls::Reader<'a>),
    Server(rustls::Reader<'a>),
}

impl<'a> Read for TlsReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            TlsReader::Client(reader) => reader.read(buf),
            TlsReader::Server(reader) => reader.read(buf),
        }
    }
}

enum TlsWriter<'a> {
    Client(rustls::Writer<'a>),
    Server(rustls::Writer<'a>),
}

impl<'a> Write for TlsWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            TlsWriter::Client(writer) => writer.write(buf),
            TlsWriter::Server(writer) => writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            TlsWriter::Client(writer) => writer.flush(),
            TlsWriter::Server(writer) => writer.flush(),
        }
    }
}

#[pyclass(frozen, unsendable, module = "rloop._rloop")]
pub(crate) struct SSLTransport {
    pub fd: usize,
    pub lfd: Option<usize>,
    state: RefCell<SSLTransportState>,
    pyloop: Py<EventLoop>,
    // atomics
    closing: atomic::AtomicBool,
    paused: atomic::AtomicBool,
    water_hi: atomic::AtomicUsize,
    water_lo: atomic::AtomicUsize,
    weof: atomic::AtomicBool,
    // py protocol fields
    pub proto: Py<PyAny>,
    proto_buffered: bool,
    proto_paused: atomic::AtomicBool,
    protom_buf_get: Py<PyAny>,
    protom_conn_lost: Py<PyAny>,
    protom_recv_data: Py<PyAny>,
    // py extras
    extra: HashMap<String, Py<PyAny>>,
    sock: Py<SocketWrapper>,
}

impl SSLTransport {
    fn new(
        py: Python,
        pyloop: Py<EventLoop>,
        stream: TcpStream,
        tls_connection: TlsConnection,
        pyproto: Bound<PyAny>,
        socket_family: i32,
        lfd: Option<usize>,
    ) -> Self {
        let fd = stream.as_raw_fd() as usize;
        let state = SSLTransportState {
            stream,
            tls_connection,
            write_buf: VecDeque::new(),
            write_buf_dsize: 0,
            handshake_complete: false,
        };

        let wh = 1024 * 64;
        let wl = wh / 4;

        let mut proto_buffered = false;
        let protom_buf_get: Py<PyAny>;
        let protom_recv_data: Py<PyAny>;
        if pyproto.is_instance(asyncio_proto_buf(py).unwrap()).unwrap() {
            proto_buffered = true;
            protom_buf_get = pyproto.getattr(pyo3::intern!(py, "get_buffer")).unwrap().unbind();
            protom_recv_data = pyproto.getattr(pyo3::intern!(py, "buffer_updated")).unwrap().unbind();
        } else {
            protom_buf_get = py.None();
            protom_recv_data = pyproto.getattr(pyo3::intern!(py, "data_received")).unwrap().unbind();
        }
        let protom_conn_lost = pyproto.getattr(pyo3::intern!(py, "connection_lost")).unwrap().unbind();
        let proto = pyproto.unbind();

        Self {
            fd,
            lfd,
            state: RefCell::new(state),
            pyloop,
            closing: false.into(),
            paused: false.into(),
            water_hi: wh.into(),
            water_lo: wl.into(),
            weof: false.into(),
            proto,
            proto_buffered,
            proto_paused: false.into(),
            protom_buf_get,
            protom_conn_lost,
            protom_recv_data,
            extra: HashMap::new(),
            sock: SocketWrapper::from_fd(py, fd, socket_family, socket2::Type::STREAM, 0),
        }
    }

    pub(crate) fn from_py_client(
        py: Python,
        pyloop: &Py<EventLoop>,
        pysock: (i32, i32),
        protocol_factory: Py<PyAny>,
        server_hostname: Option<String>,
        ssl_context: Py<PyAny>,
    ) -> Result<Self> {
        let sock = unsafe { socket2::Socket::from_raw_fd(pysock.0) };
        _ = sock.set_nonblocking(true);
        let stdl: std::net::TcpStream = sock.into();
        let stream = TcpStream::from_std(stdl);

        // Create TLS client connection
        let config = Self::py_ssl_context_to_rustls_client_config(py, ssl_context)?;
        // TODO: Properly handle server_hostname parameter
        let server_name = rustls::pki_types::ServerName::IpAddress(rustls::pki_types::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST.into()));

        let conn = ClientConnection::new(config, server_name)?;
        let tls_connection = TlsConnection::Client(conn);

        let proto = protocol_factory.bind(py).call0()?;

        Ok(Self::new(py, pyloop.clone_ref(py), stream, tls_connection, proto, pysock.1, None))
    }

    pub(crate) fn from_py_server(
        py: Python,
        pyloop: &Py<EventLoop>,
        pysock: (i32, i32),
        protocol_factory: Py<PyAny>,
        ssl_context: Py<PyAny>,
    ) -> Result<Self> {
        let sock = unsafe { socket2::Socket::from_raw_fd(pysock.0) };
        _ = sock.set_nonblocking(true);
        let stdl: std::net::TcpStream = sock.into();
        let stream = TcpStream::from_std(stdl);

        // Create TLS server connection
        let config = Self::py_ssl_context_to_rustls_server_config(py, ssl_context)?;
        let conn = ServerConnection::new(config)?;
        let tls_connection = TlsConnection::Server(conn);

        let proto = protocol_factory.bind(py).call0()?;

        Ok(Self::new(py, pyloop.clone_ref(py), stream, tls_connection, proto, pysock.1, None))
    }

    fn py_ssl_context_to_rustls_client_config(py: Python, ssl_context: Py<PyAny>) -> Result<Arc<ClientConfig>> {
        // This is a simplified conversion - in practice, we'd need to extract
        // certificates, keys, and other settings from the Python SSLContext
        // For now, create a basic config that accepts any certificate
        let mut config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCertificate))
            .with_no_client_auth();

        // TODO: Properly extract settings from Python SSLContext
        // - Certificate verification settings
        // - Client certificates
        // - Cipher suites
        // - Protocol versions

        Ok(Arc::new(config))
    }

    fn py_ssl_context_to_rustls_server_config(py: Python, ssl_context: Py<PyAny>) -> Result<Arc<ServerConfig>> {
        // This is a simplified conversion - in practice, we'd need to extract
        // certificates, keys, and other settings from the Python SSLContext
        // For now, create a basic config
        let mut config = ServerConfig::builder()
            .with_no_client_auth();

        // TODO: Properly extract settings from Python SSLContext
        // - Server certificates and keys
        // - Client certificate requirements
        // - Cipher suites
        // - Protocol versions

        // Add a dummy certificate for testing
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
        let cert_der = cert.cert.der().to_vec();
        let key_der = cert.key_pair.serialize_der();

        let config = config.with_single_cert(vec![rustls::pki_types::CertificateDer::from(cert_der)],
                                           rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(key_der)))?;

        Ok(Arc::new(config))
    }

    pub(crate) fn attach(pyself: &Py<Self>, py: Python) -> PyResult<Py<PyAny>> {
        let rself = pyself.borrow(py);
        rself
            .proto
            .call_method1(py, pyo3::intern!(py, "connection_made"), (pyself.clone_ref(py),))?;
        Ok(rself.proto.clone_ref(py))
    }

    fn try_write(pyself: &Py<Self>, py: Python, data: &[u8]) -> PyResult<()> {
        let rself = pyself.borrow(py);

        if rself.weof.load(atomic::Ordering::Relaxed) {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Cannot write after EOF"));
        }
        if data.is_empty() {
            return Ok(());
        }

        let mut state = rself.state.borrow_mut();

        // If handshake is not complete, buffer the data
        if !state.handshake_complete {
            state.write_buf.push_back(data.into());
            state.write_buf_dsize += data.len();
            return Ok(());
        }

        // Try to write through TLS
        let mut writer = state.tls_connection.writer();
        match writer.write(data) {
            Ok(written) if written == data.len() => {
                // All data written to TLS, now try to flush to socket
                Self::flush_tls_to_socket(&mut state)?;
            }
            Ok(written) => {
                // Partial write, buffer the rest
                let remaining = &data[written..];
                state.write_buf.push_back(remaining.into());
                state.write_buf_dsize += remaining.len();
                Self::flush_tls_to_socket(&mut state)?;
            }
            Err(_) => {
                // Buffer the data for later
                state.write_buf.push_back(data.into());
                state.write_buf_dsize += data.len();
            }
        }

        Ok(())
    }

    fn flush_tls_to_socket(state: &mut SSLTransportState) -> Result<(), std::io::Error> {
        loop {
            match state.tls_connection.write_tls(&mut state.stream) {
                Ok(0) => break, // No more data to write
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn proto_pause(pyself: &Py<Self>, py: Python) {
        let rself = pyself.borrow(py);
        if let Err(err) = rself.proto.call_method0(py, pyo3::intern!(py, "pause_writing")) {
            let err_ctx = LogExc::transport(
                err,
                "protocol.pause_writing() failed".into(),
                rself.proto.clone_ref(py),
                pyself.clone_ref(py).into_any(),
            );
            _ = rself.pyloop.get().log_exception(py, err_ctx);
        }
    }

    fn proto_resume(pyself: &Py<Self>, py: Python) {
        let rself = pyself.borrow(py);
        if let Err(err) = rself.proto.call_method0(py, pyo3::intern!(py, "resume_writing")) {
            let err_ctx = LogExc::transport(
                err,
                "protocol.resume_writing() failed".into(),
                rself.proto.clone_ref(py),
                pyself.clone_ref(py).into_any(),
            );
            _ = rself.pyloop.get().log_exception(py, err_ctx);
        }
    }

    #[inline]
    fn close_from_read_handle(&self, py: Python, event_loop: &EventLoop) -> bool {
        if self
            .closing
            .compare_exchange(false, true, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_err()
        {
            return false;
        }

        if !self.state.borrow_mut().write_buf.is_empty() {
            return false;
        }

        event_loop.ssl_stream_rem(self.fd, Interest::WRITABLE);
        _ = self.protom_conn_lost.call1(py, (py.None(),));
        true
    }
}

#[pymethods]
impl SSLTransport {
    #[pyo3(signature = (name, default = None))]
    fn get_extra_info(&self, py: Python, name: &str, default: Option<Py<PyAny>>) -> Option<Py<PyAny>> {
        match name {
            "socket" => Some(self.sock.clone_ref(py).into_any()),
            "sockname" => self.sock.call_method0(py, pyo3::intern!(py, "getsockname")).ok(),
            "peername" => self.sock.call_method0(py, pyo3::intern!(py, "getpeername")).ok(),
            "sslcontext" => Some(self.extra.get("sslcontext").unwrap_or(&py.None()).clone_ref(py)),
            "server_hostname" => self.extra.get("server_hostname").map(|v| v.clone_ref(py)),
            _ => self.extra.get(name).map(|v| v.clone_ref(py)).or(default),
        }
    }

    fn is_closing(&self) -> bool {
        self.closing.load(atomic::Ordering::Relaxed)
    }

    fn close(&self, py: Python) {
        if self
            .closing
            .compare_exchange(false, true, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let event_loop = self.pyloop.get();
        event_loop.ssl_stream_rem(self.fd, Interest::READABLE);
        if self.state.borrow().write_buf_dsize == 0 {
            self.call_conn_lost(py, None);
        }
    }

    fn set_protocol(&self, _protocol: Py<PyAny>) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SSLTransport protocol cannot be changed",
        ))
    }

    fn get_protocol(&self, py: Python) -> Py<PyAny> {
        self.proto.clone_ref(py)
    }

    fn is_reading(&self) -> bool {
        !self.closing.load(atomic::Ordering::Relaxed) && !self.paused.load(atomic::Ordering::Relaxed)
    }

    fn pause_reading(&self) {
        if self.closing.load(atomic::Ordering::Relaxed) {
            return;
        }
        if self
            .paused
            .compare_exchange(false, true, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        self.pyloop.get().ssl_stream_rem(self.fd, Interest::READABLE);
    }

    fn resume_reading(&self) {
        if self.closing.load(atomic::Ordering::Relaxed) {
            return;
        }
        if self
            .paused
            .compare_exchange(true, false, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        self.pyloop.get().ssl_stream_add(self.fd, Interest::READABLE);
    }

    #[pyo3(signature = (high = None, low = None))]
    fn set_write_buffer_limits(pyself: Py<Self>, py: Python, high: Option<usize>, low: Option<usize>) -> PyResult<()> {
        let wh = match high {
            None => match low {
                None => 1024 * 64,
                Some(v) => v * 4,
            },
            Some(v) => v,
        };
        let wl = match low {
            None => wh / 4,
            Some(v) => v,
        };

        if wh < wl {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "high must be >= low must be >= 0",
            ));
        }

        let rself = pyself.borrow(py);
        rself.water_hi.store(wh, atomic::Ordering::Relaxed);
        rself.water_lo.store(wl, atomic::Ordering::Relaxed);

        Ok(())
    }

    fn get_write_buffer_size(&self) -> usize {
        self.state.borrow().write_buf_dsize
    }

    fn get_write_buffer_limits(&self) -> (usize, usize) {
        (
            self.water_lo.load(atomic::Ordering::Relaxed),
            self.water_hi.load(atomic::Ordering::Relaxed),
        )
    }

    fn write(pyself: Py<Self>, py: Python, data: Cow<[u8]>) -> PyResult<()> {
        Self::try_write(&pyself, py, &data)
    }

    fn writelines(pyself: Py<Self>, py: Python, data: &Bound<PyAny>) -> PyResult<()> {
        let pybytes = PyBytes::new(py, &[0; 0]);
        let pybytesj = pybytes.call_method1(pyo3::intern!(py, "join"), (data,))?;
        let bytes = pybytesj.extract::<Cow<[u8]>>()?;
        Self::try_write(&pyself, py, &bytes)
    }

    fn write_eof(&self) {
        if self.closing.load(atomic::Ordering::Relaxed) {
            return;
        }
        if self
            .weof
            .compare_exchange(false, true, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        // TODO: Implement proper TLS shutdown
        // For now, just close the connection
        Python::with_gil(|py| self.close(py));
    }

    fn can_write_eof(&self) -> bool {
        true
    }

    fn abort(&self, py: Python) {
        if self.state.borrow().write_buf_dsize > 0 {
            self.pyloop.get().ssl_stream_rem(self.fd, Interest::WRITABLE);
        }
        if self
            .closing
            .compare_exchange(false, true, atomic::Ordering::Relaxed, atomic::Ordering::Relaxed)
            .is_ok()
        {
            self.pyloop.get().ssl_stream_rem(self.fd, Interest::READABLE);
        }
        self.call_conn_lost(py, None);
    }

    fn call_conn_lost(&self, py: Python, err: Option<PyObject>) {
        _ = self.protom_conn_lost.call1(py, (err,));
        self.pyloop.get().ssl_stream_close(py, self.fd);
    }
}

pub(crate) struct SSLReadHandle {
    pub fd: usize,
}

impl SSLReadHandle {
    fn do_handshake(&self, py: Python, transport: &SSLTransport) -> Result<bool, Box<dyn std::error::Error>> {
        // Read data from socket into TLS
        {
            let mut state = transport.state.borrow_mut();
            match state.tls_connection.read_tls(&mut state.stream) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) => return Err(Box::new(e)),
            }
        }

        // Process the TLS packets
        {
            let mut state = transport.state.borrow_mut();
            state.tls_connection.process_new_packets()?;
        }

        // Check if handshake is complete
        {
            let mut state = transport.state.borrow_mut();
            if !state.tls_connection.is_handshaking() && !state.handshake_complete {
                state.handshake_complete = true;

                // Send any buffered data now that handshake is complete
                Self::flush_buffered_data(&mut state)?;

                return Ok(true);
            }
        }

        Ok(false)
    }

    fn flush_buffered_data(state: &mut SSLTransportState) -> Result<(), std::io::Error> {
        if !state.write_buf.is_empty() {
            let mut writer = state.tls_connection.writer();
            while let Some(data) = state.write_buf.pop_front() {
                writer.write_all(&data)?;
                state.write_buf_dsize -= data.len();
            }
            drop(writer); // Release the writer before flushing
            Self::flush_tls_to_socket(state)?;
        }
        Ok(())
    }

    fn flush_tls_to_socket(state: &mut SSLTransportState) -> Result<(), std::io::Error> {
        loop {
            match state.tls_connection.write_tls(&mut state.stream) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn recv_data(&self, py: Python, transport: &SSLTransport, buf: &mut [u8]) -> (Option<Py<PyAny>>, bool) {
        let mut state = transport.state.borrow_mut();

        if !state.handshake_complete {
            return (None, false);
        }

        let mut reader = state.tls_connection.reader();
        match reader.read(buf) {
            Ok(0) => (None, true), // EOF
            Ok(read) => {
                let rbuf = &buf[..read];
                let pydata = unsafe { PyBytes::from_ptr(py, rbuf.as_ptr(), read) };
                (Some(pydata.into_any().unbind()), false)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (None, false),
            Err(_) => (None, true), // Error, treat as EOF
        }
    }

    fn recv_eof(&self, py: Python, event_loop: &EventLoop, transport: &SSLTransport) -> bool {
        event_loop.ssl_stream_rem(self.fd, Interest::READABLE);
        if let Ok(pyr) = transport.proto.call_method0(py, pyo3::intern!(py, "eof_received"))
            && let Ok(true) = pyr.is_truthy(py)
        {
            return false;
        }
        transport.close_from_read_handle(py, event_loop)
    }
}

impl Handle for SSLReadHandle {
    fn run(&self, py: Python, event_loop: &EventLoop, state: &mut EventLoopRunState) {
        debug!("SSLReadHandle::run called for fd {}", self.fd);
        let pytransport = event_loop.get_ssl_transport(self.fd, py);
        let transport = pytransport.borrow(py);

        // First, try to complete handshake if needed
        if let Err(_) = self.do_handshake(py, &transport) {
            let err = pyo3::exceptions::PyRuntimeError::new_err("SSL handshake failed");
            transport.call_conn_lost(py, Some(err.into_pyobject(py).unwrap().into_any().unbind()));
            return;
        }

        // Then try to read data
        let (data, eof) = self.recv_data(py, &transport, &mut state.read_buf);

        if let Some(data) = data {
            _ = transport.protom_recv_data.call1(py, (data,));
        }

        if eof {
            if self.recv_eof(py, event_loop, &transport) {
                event_loop.ssl_stream_close(py, self.fd);
            }
        }
    }
}

pub(crate) struct SSLWriteHandle {
    pub fd: usize,
}

impl SSLWriteHandle {
    fn flush_pending(&self, transport: &SSLTransport) -> Result<(), std::io::Error> {
        let mut state = transport.state.borrow_mut();

        // Try to flush TLS data to socket
        loop {
            match state.tls_connection.write_tls(&mut state.stream) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

impl Handle for SSLWriteHandle {
    fn run(&self, py: Python, event_loop: &EventLoop, _state: &mut EventLoopRunState) {
        let pytransport = event_loop.get_ssl_transport(self.fd, py);
        let transport = pytransport.borrow(py);

        if let Err(_) = self.flush_pending(&transport) {
            let err = pyo3::exceptions::PyRuntimeError::new_err("SSL write failed");
            transport.call_conn_lost(py, Some(err.into_pyobject(py).unwrap().into_any().unbind()));
            event_loop.ssl_stream_close(py, self.fd);
            return;
        }

        // Check if we need to continue writing
        let state = transport.state.borrow();
        if state.tls_connection.wants_write() {
            return; // Keep writable interest
        }

        event_loop.ssl_stream_rem(self.fd, Interest::WRITABLE);
    }
}

// Dummy certificate verifier that accepts any certificate
#[derive(Debug)]
struct AcceptAnyCertificate;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer,
        _intermediates: &[rustls::pki_types::CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA1,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::ECDSA_SHA1_Legacy,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}
