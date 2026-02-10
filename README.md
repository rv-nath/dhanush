# dhanush

High-performance HTTP load testing tool built in Rust. Supports **multi-URL groups with weighted request distribution**, running multiple groups in parallel. Use quick CLI mode for single-URL tests or YAML config files for complex multi-group scenarios.

## Features

- **Weighted request distribution** -- assign weights to endpoints so traffic follows realistic patterns
- **Multi-group parallel execution** -- test different endpoint groups concurrently with independent configs
- **Ramp-up support** -- gradually add connections over a configurable period instead of slamming all at once
- **Duration or count mode** -- run for a time period (`-d 30s`) or a fixed number of requests (`-n 10000`)
- **HdrHistogram latency tracking** -- accurate percentile reporting (p50/p90/p95/p99) with microsecond precision
- **Per-group and overall stats** -- separate results for each group plus an aggregated summary
- **Text and JSON output** -- human-friendly colored terminal output or structured JSON for pipelines
- **Progress bars** -- real-time multi-line progress display (one bar per group)
- **Configurable concurrency** -- tune tokio worker threads (`-t`) and async connections per group (`-c`)
- **YAML config** -- define complex test scenarios in a config file with global defaults and per-group overrides

## Installation

```bash
cargo build --release
# Binary at ./target/release/dhanush
```

## Quick Start

```bash
# Simple GET test: 10 connections for 10 seconds
dhanush http://localhost:8080/api

# 100 connections for 30 seconds
dhanush -c 100 -d 30s http://localhost:8080/api

# Fixed request count
dhanush -c 50 -n 10000 http://localhost:8080/api

# POST with headers and body
dhanush -c 100 -d 10s -m POST \
    -H "Content-Type: application/json" \
    -b '{"key":"value"}' \
    http://localhost:8080/api

# With ramp-up: add connections linearly over 5 seconds
dhanush -c 100 -d 30s --ramp-up 5s http://localhost:8080/api

# Multi-group mode via YAML config
dhanush --config loadtest.yaml

# JSON output
dhanush -c 10 -d 5s --output-format json http://localhost:8080/api
```

## CLI Reference

```
Usage: dhanush [OPTIONS] [URL]

Arguments:
  [URL]  Target URL (for single-URL mode)

Options:
  -c, --connections <N>        Concurrent connections per group [default: 10]
  -d, --duration <DURATION>    Test duration, e.g. "10s", "1m" [default: 10s]
  -n, --num-requests <N>       Total requests (count mode, overrides duration)
  -t, --threads <N>            Tokio worker threads [default: num_cpus]
  -m, --method <METHOD>        HTTP method [default: GET]
  -H, --header <K: V>          HTTP header (repeatable)
  -b, --body <BODY>            Request body
  -r, --ramp-up <DURATION>     Ramp-up time to gradually add connections
      --timeout <DURATION>     Request timeout [default: 5s]
      --insecure               Disable TLS certificate verification
      --output-format <FMT>    Output format: text or json [default: text]
      --no-progress            Disable progress bars
      --config <PATH>          YAML config file for multi-group mode
  -h, --help                   Print help
  -V, --version                Print version
```

## YAML Config

For multi-group scenarios, define a YAML config file:

```yaml
# Global defaults (can be overridden per-group)
defaults:
  connections: 100
  duration: "30s"
  timeout: "5s"
  ramp-up: "3s"

groups:
  - name: "API endpoints"
    connections: 200
    duration: "60s"
    ramp-up: "10s"       # override global ramp-up
    endpoints:
      - url: "http://api.example.com/users"
        method: GET
        weight: 50
      - url: "http://api.example.com/orders"
        method: POST
        headers:
          Content-Type: "application/json"
        body: '{"item":"widget"}'
        weight: 30
      - url: "http://api.example.com/health"
        weight: 20

  - name: "Static assets"
    connections: 50
    endpoints:
      - url: "http://cdn.example.com/style.css"
        weight: 60
      - url: "http://cdn.example.com/app.js"
        weight: 40
```

Weights control request distribution within a group. In the example above, the API group sends ~50% of requests to `/users`, ~30% to `/orders`, and ~20% to `/health`.

CLI flags (`-t`, `-n`, `-r`, `--output-format`, `--no-progress`) override config file values when both are provided.

## Output

### Text (default)

```
╔══════════════════════════════════════════════╗
║         dhanush load test results            ║
╚══════════════════════════════════════════════╝

── Group: API endpoints ──────────────────────
  Endpoints:
    [ 50%] GET  http://api.example.com/users
    [ 30%] POST http://api.example.com/orders
    [ 20%] GET  http://api.example.com/health

  Connections:   200    Duration:  60.01s

  Latency Distribution:
    p50     1.23ms
    p90     4.56ms
    p95     8.91ms
    p99    23.45ms
    max   156.78ms

  Latency Stats:
    Avg     2.34ms    Stdev     5.67ms
    Min   210.00us    Max     156.78ms

  Throughput:
    Requests/sec:    15,234.56
    Transfer/sec:       12.34 MB
    Total requests:   457,037
    Total transfer:   370.20 MB

  Status Codes:
    200:  456,982  (99.99%)
    500:       55  ( 0.01%)
  Errors:      12  ( 0.00%)

── Overall Summary ───────────────────────────
  Total requests:    587,037
  Total errors:           15
  Combined RPS:      20,456.78
  Combined transfer:    512.30 MB
```

### JSON (`--output-format json`)

```json
{
  "groups": [
    {
      "name": "default",
      "endpoints": [{ "url": "...", "method": "GET", "weight_pct": 100.0 }],
      "connections": 10,
      "duration_secs": 10.02,
      "latency": { "p50_us": 1230, "p90_us": 4560, "p95_us": 8910, "p99_us": 23450, "min_us": 210, "max_us": 156780, "mean_us": 2340.5, "stdev_us": 5670.2 },
      "throughput": { "requests_per_sec": 15234.56, "bytes_per_sec": 12940000, "total_requests": 457037, "total_bytes": 388169420 },
      "status_codes": { "200": 456982, "500": 55 },
      "errors": 12
    }
  ],
  "summary": { "total_requests": 587037, "total_errors": 15, "combined_rps": 20456.78, "total_bytes": 537190400 }
}
```

## Ramp-up

Without ramp-up, all connections fire simultaneously from the start. With `--ramp-up 10s` and `-c 100`, connections are added linearly over 10 seconds:

- Worker 0 starts at t=0s
- Worker 50 starts at t=~5s
- Worker 99 starts at t=~10s

Ramp-up time is included in the total test duration (not added on top). This is useful for avoiding thundering-herd effects and observing how your service behaves as load gradually increases.

## Mock Server

A zero-dependency Python mock server is included for testing:

```bash
python3 mock-server/server.py -p 8080
```

Endpoints: `/health`, `/users`, `/orders` (POST), `/slow`, `/large`, `/flaky` (80/20 success/error), `/echo` (POST), `/login` (POST), `/style.css`, `/app.js`, `/status`.

A sample multi-group config is at `mock-server/loadtest.yaml`.

## Architecture

```
CLI args / YAML config
        │
        ▼
  Config (Vec<GroupConfig>)
        │
   engine::run()
        │
   ┌────┼────┐        Groups run in parallel (tokio JoinSet)
   ▼    ▼    ▼
 Group Group Group     Each group has its own reqwest client pool
   │    │    │         N async worker tasks per group
   │    │    │         Weighted random URL selection (O(1) per request)
   ▼    ▼    ▼
 Stats Stats Stats     Per-worker HdrHistogram (zero lock contention)
   └────┼────┘
        ▼
  reporter output      Per-group sections + overall summary
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| clap | CLI argument parsing |
| tokio | Async runtime with configurable worker threads |
| reqwest | HTTP client with connection pooling, rustls, gzip/brotli |
| hdrhistogram | Latency percentile tracking |
| indicatif | Progress bars |
| colored | Colored terminal output |
| humantime | Duration string parsing |
| thiserror / anyhow | Error handling |
| serde / serde_yaml / serde_json | Config and output serialization |
| byte-unit | Human-friendly byte formatting |
| rand | Weighted random URL selection |

## License

MIT
