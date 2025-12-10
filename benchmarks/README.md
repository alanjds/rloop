# RLoop benchmarks

Run at: Wed 10 Dec 2025, 16:21    
Environment: GHA Linux x86_64 (CPUs: 4)    
Python version: 3.13    
RLoop version: 0.2.0    

### Raw sockets

TCP echo server with raw sockets comparison using 1KB, 10KB and 100KB messages.


| Loop | Throughput (1KB) | Throughput (10KB) | Throughput (100KB) |
| --- | --- | --- | --- |
| asyncio | 14173.6 (82.2%) | 13179.7 (87.9%) | 8473.5 (116.1%) | 
| rloop | 17247.7 (100.0%) | 14992.0 (100.0%) | 7295.5 (100.0%) | 
| uvloop | 15724.5 (91.2%) | 14394.3 (96.0%) | 10226.8 (140.2%) | 


#### 1KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 141736 | 14173.6 (82.2%) | 0.068ms | 0.105ms | 0.012 |
| rloop | 172477 | 17247.7 (100.0%) | 0.056ms | 0.086ms | 0.01 |
| uvloop | 157245 | 15724.5 (91.2%) | 0.06ms | 0.1ms | 0.015 |


#### 10KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 131797 | 13179.7 (87.9%) | 0.073ms | 0.114ms | 0.015 |
| rloop | 149920 | 14992.0 (100.0%) | 0.064ms | 0.102ms | 0.013 |
| uvloop | 143943 | 14394.3 (96.0%) | 0.066ms | 0.107ms | 0.016 |


#### 100KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 84735 | 8473.5 (116.1%) | 0.115ms | 0.174ms | 0.023 |
| rloop | 72955 | 7295.5 (100.0%) | 0.134ms | 0.286ms | 0.032 |
| uvloop | 102268 | 10226.8 (140.2%) | 0.095ms | 0.145ms | 0.016 |


### Streams

TCP echo server with `asyncio` streams comparison using 1KB, 10KB and 100KB messages.


| Loop | Throughput (1KB) | Throughput (10KB) | Throughput (100KB) |
| --- | --- | --- | --- |
| asyncio | 13832.5 (82.4%) | 12898.9 (83.9%) | 5925.6 (80.1%) | 
| rloop | 16787.2 (100.0%) | 15367.3 (100.0%) | 7394.6 (100.0%) | 
| uvloop | 14869.0 (88.6%) | 13470.6 (87.7%) | 7029.4 (95.1%) | 


#### 1KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 138325 | 13832.5 (82.4%) | 0.068ms | 0.103ms | 0.013 |
| rloop | 167872 | 16787.2 (100.0%) | 0.056ms | 0.087ms | 0.01 |
| uvloop | 148690 | 14869.0 (88.6%) | 0.067ms | 0.094ms | 0.01 |


#### 10KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 128989 | 12898.9 (83.9%) | 0.076ms | 0.109ms | 0.012 |
| rloop | 153673 | 15367.3 (100.0%) | 0.064ms | 0.092ms | 0.01 |
| uvloop | 134706 | 13470.6 (87.7%) | 0.07ms | 0.104ms | 0.012 |


#### 100KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 59256 | 5925.6 (80.1%) | 0.165ms | 0.234ms | 0.035 |
| rloop | 73946 | 7394.6 (100.0%) | 0.134ms | 0.19ms | 0.024 |
| uvloop | 70294 | 7029.4 (95.1%) | 0.139ms | 0.202ms | 0.027 |


### Protocol

TCP echo server with `asyncio.Protocol` comparison using 1KB, 10KB and 100KB messages.


| Loop | Throughput (1KB) | Throughput (10KB) | Throughput (100KB) |
| --- | --- | --- | --- |
| asyncio | 17255.3 (81.4%) | 16841.2 (87.1%) | 11983.6 (94.3%) | 
| rloop | 21187.2 (100.0%) | 19335.0 (100.0%) | 12709.4 (100.0%) | 
| uvloop | 19376.5 (91.5%) | 18435.8 (95.3%) | 12295.4 (96.7%) | 


#### 1KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 172553 | 17255.3 (81.4%) | 0.056ms | 0.082ms | 0.008 |
| rloop | 211872 | 21187.2 (100.0%) | 0.044ms | 0.068ms | 0.006 |
| uvloop | 193765 | 19376.5 (91.5%) | 0.049ms | 0.073ms | 0.01 |


#### 10KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 168412 | 16841.2 (87.1%) | 0.056ms | 0.083ms | 0.009 |
| rloop | 193350 | 19335.0 (100.0%) | 0.05ms | 0.07ms | 0.008 |
| uvloop | 184358 | 18435.8 (95.3%) | 0.053ms | 0.078ms | 0.006 |


#### 100KB details

| Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| asyncio | 119836 | 11983.6 (94.3%) | 0.083ms | 0.114ms | 0.009 |
| rloop | 127094 | 12709.4 (100.0%) | 0.075ms | 0.107ms | 0.009 |
| uvloop | 122954 | 12295.4 (96.7%) | 0.077ms | 0.112ms | 0.011 |


### Other benchmarks

- [Python versions](./pyver.md)
