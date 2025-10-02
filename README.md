# Wakanda-LB
Solution of [this](https://codingchallenges.fyi/challenges/challenge-load-balancer/) coding challenge.
A lightweight HTTP load balancer built with Rust, designed for high throughput and low latency. It distributes incoming HTTP requests across multiple backend servers using configurable routing strategies.

---

## Build the Project

```bash
cargo build --release
```

# Run Tests
```bash
cargo test
```

# Start the Load Balancer
```bash
./load-balancer \
  --port 3000 \
  --target-servers http://localhost:9000,http://localhost:9001 \
  --routing-policy round-robin
```

# CLI Options
```bash
load-balancer [OPTIONS]

Options:
  -p, --port <PORT>                             Port to listen on [default: 3000]
  -t, --target-servers <SERVERS>                Comma-separated list of backend servers
                                                Example: http://server1:8000,http://server2:8000
  -r, --routing-policy <POLICY>                 Load balancing strategy [default: round-robin]
                                                Possible values:
                                                - round-robin: Distribute requests evenly
                                                - random:      Random server selection
  --target-servers-health-path <PATH>           Path to check backend server health [default: /health]
  --health-checker-polling-seconds <SECONDS>    Polling interval for health checks in seconds [default: 10]
  -h, --help                                    Print help
  -V, --version                                 Print version

```

# Run a Full Containerized Mock Environment
You can start a full mock environment with dummy backend servers using Docker:
```bash
make all
```
This will launch:
- Wakanda-LB on port 3000
- Two dummy backend servers on ports 9000 and 9001

# Mock Environment Architecture
```bash
    ┌─────────────┐
    │   Client    │
    └──────┬──────┘
           │           ← HTTP Request
           ▼
  ┌─────────────────┐
  │   Wakanda-LB    │
  │   (Port 3000)   │
  │                 │
  │  ┌───────────┐  │
  │  │  Routing  │  │  ← Round Robin / Random
  │  │  Strategy │  │
  │  └───────────┘  │
  └────┬────────┬───┘
       │        │
       ▼        ▼
┌─────────┐ ┌─────────┐
│ Backend │ │ Backend │
│ Server  │ │ Server  │
│  :9000  │ │  :9001  │
└─────────┘ └─────────┘

```

# Run benchmark
Add how many times
```bash
./benchmark.sh <max_request_count> <url>
```
