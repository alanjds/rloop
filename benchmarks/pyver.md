# RLoop benchmarks

## Python versions

Run at: Wed 10 Dec 2025, 16:23    
Environment: GHA Linux x86_64 (CPUs: 4)    
RLoop version: 0.2.0    

Comparison between different Python versions.    
The only test performed is the raw socket one.


### 1KB

| Python version | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| 3.10 | 258173 | 25817.3 | 0.036ms | 0.054ms | 0.01 |
| 3.11 | 261356 | 26135.6 | 0.036ms | 0.053ms | 0.01 |
| 3.12 | 259234 | 25923.4 | 0.036ms | 0.053ms | 0.011 |
| 3.13 | 246188 | 24618.8 | 0.038ms | 0.056ms | 0.011 |


### 10KB

| Python version | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| 3.10 | 224174 | 22417.4 | 0.043ms | 0.06ms | 0.015 |
| 3.11 | 225695 | 22569.5 | 0.043ms | 0.06ms | 0.01 |
| 3.12 | 218645 | 21864.5 | 0.044ms | 0.063ms | 0.011 |
| 3.13 | 219167 | 21916.7 | 0.044ms | 0.063ms | 0.01 |


### 100KB

| Python version | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- |
| 3.10 | 100039 | 10003.9 | 0.098ms | 0.126ms | 0.019 |
| 3.11 | 100805 | 10080.5 | 0.097ms | 0.123ms | 0.019 |
| 3.12 | 99941 | 9994.1 | 0.098ms | 0.126ms | 0.019 |
| 3.13 | 101069 | 10106.9 | 0.097ms | 0.125ms | 0.019 |


### 10KB VS other

| Python version | Loop | Total requests | Throughput | Mean latency | 99p latency | Latency stdev |
| --- | --- | --- | --- | --- | --- | --- |
| 3.10 | asyncio | 200726 | 20072.6 (89.5%) | 0.048ms | 0.069ms | 0.012 |
| 3.10 | rloop | 224174 | 22417.4 (100.0%) | 0.043ms | 0.06ms | 0.015 |
| 3.10 | uvloop | 218427 | 21842.7 (97.4%) | 0.044ms | 0.062ms | 0.011 |
| 3.11 | asyncio | 198269 | 19826.9 (87.8%) | 0.049ms | 0.069ms | 0.014 |
| 3.11 | rloop | 225695 | 22569.5 (100.0%) | 0.043ms | 0.06ms | 0.01 |
| 3.11 | uvloop | 221554 | 22155.4 (98.2%) | 0.044ms | 0.061ms | 0.011 |
| 3.12 | asyncio | 214634 | 21463.4 (98.2%) | 0.045ms | 0.067ms | 0.012 |
| 3.12 | rloop | 218645 | 21864.5 (100.0%) | 0.044ms | 0.063ms | 0.011 |
| 3.12 | uvloop | 201510 | 20151.0 (92.2%) | 0.048ms | 0.068ms | 0.014 |
| 3.13 | asyncio | 183617 | 18361.7 (83.8%) | 0.053ms | 0.075ms | 0.013 |
| 3.13 | rloop | 219167 | 21916.7 (100.0%) | 0.044ms | 0.063ms | 0.01 |
| 3.13 | uvloop | 203867 | 20386.7 (93.0%) | 0.047ms | 0.067ms | 0.013 |
