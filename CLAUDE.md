# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Dhanush is a high-performance HTTP load testing tool written in Rust. It supports multi-URL groups, weighted request distribution, ramp-up periods, and parallel group execution with HdrHistogram-based latency tracking.

## Build & Run Commands

```bash
cargo build                          # Debug build
cargo build --release                # Optimized release build
cargo run -- <URL> [options]         # Run in single-URL mode
cargo run -- --config <yaml>         # Run with YAML config file
```

There are currently no automated tests (`cargo test` will find none).

## Mock Server (for manual testing)

```bash
python3 mock-server/server.py -p 8080       # Start mock server
cargo run -- http://127.0.0.1:8080/users -c 10 -d 5s   # Simple load test
cargo run -- --config mock-server/loadtest.yaml          # Multi-group test
```

## Architecture

The execution pipeline flows linearly:

**CLI (clap)** → **Config (YAML/CLI merge)** → **Engine (group orchestration)** → **Workers (per-connection request loops)** → **Stats (HdrHistogram aggregation)** → **Reporter (text/JSON output)**

### Key modules

- **`cli.rs`** — Clap-derived CLI argument definitions
- **`config.rs`** — Parses YAML configs, merges with CLI flags, builds `GroupConfig` structs. Supports `TestMode::Duration` and `TestMode::RequestCount`
- **`engine.rs`** — Spawns groups in parallel via `JoinSet`. Each group creates its own reqwest client pool, `RequestSelector`, and worker tasks. Ramp-up staggers worker spawn times linearly
- **`worker.rs`** — Async loop: select weighted endpoint → send request → record latency/status. Each worker owns its own `WorkerStats` (no shared lock contention)
- **`http_client.rs`** — `RequestSelector` uses `WeightedIndex` for O(1) random endpoint selection. `build_client()` configures connection pooling and TLS
- **`stats.rs`** — `WorkerStats` holds per-worker HdrHistogram (1μs–60s range). `GroupStats::from_workers()` merges histograms and computes percentiles (p50/p90/p95/p99). `OverallStats` sums across groups
- **`reporter.rs`** — Text mode uses `colored` crate with box-drawing. JSON mode serializes nested structs via serde
- **`error.rs`** — `thiserror`-derived error enum wrapping config, HTTP, IO, and YAML errors

### Design decisions

- Per-worker histograms avoid lock contention; merged only after test completion
- Ramp-up time is included in total test duration (not added on top)
- Termination uses `CancellationToken` (duration mode) or `AtomicU64` counter (request-count mode)
- reqwest client pool is configured with `max_idle_per_host = connections` and TCP_NODELAY
- Tokio runtime worker threads default to CPU count, configurable via `-t`
