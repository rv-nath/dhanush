use hdrhistogram::Histogram;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::config::EndpointConfig;

pub struct WorkerStats {
    pub histogram: Histogram<u64>,
    pub status_codes: HashMap<u16, u64>,
    pub errors: u64,
    pub total_requests: u64,
    pub total_bytes: u64,
}

impl WorkerStats {
    pub fn new() -> Self {
        Self {
            histogram: Histogram::new_with_bounds(1, 60_000_000, 3).unwrap(),
            status_codes: HashMap::new(),
            errors: 0,
            total_requests: 0,
            total_bytes: 0,
        }
    }

    pub fn record_success(&mut self, latency_us: u64, status: u16, bytes: u64) {
        let _ = self.histogram.record(latency_us);
        *self.status_codes.entry(status).or_insert(0) += 1;
        self.total_requests += 1;
        self.total_bytes += bytes;
    }

    pub fn record_error(&mut self, latency_us: u64) {
        let _ = self.histogram.record(latency_us);
        self.errors += 1;
        self.total_requests += 1;
    }
}

#[derive(Debug, Serialize)]
pub struct GroupStats {
    pub total_requests: u64,
    pub total_bytes: u64,
    pub errors: u64,
    pub elapsed: Duration,
    pub status_codes: HashMap<u16, u64>,

    // Latency percentiles in microseconds
    pub p50_us: u64,
    pub p90_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
    pub min_us: u64,
    pub mean_us: f64,
    pub stdev_us: f64,

    // Throughput
    pub requests_per_sec: f64,
    pub bytes_per_sec: f64,
}

impl GroupStats {
    pub fn from_workers(workers: Vec<WorkerStats>, elapsed: Duration) -> Self {
        let mut merged = Histogram::new_with_bounds(1, 60_000_000, 3).unwrap();
        let mut status_codes: HashMap<u16, u64> = HashMap::new();
        let mut total_requests = 0u64;
        let mut total_bytes = 0u64;
        let mut errors = 0u64;

        for w in workers {
            merged.add(&w.histogram).ok();
            for (code, count) in w.status_codes {
                *status_codes.entry(code).or_insert(0) += count;
            }
            total_requests += w.total_requests;
            total_bytes += w.total_bytes;
            errors += w.errors;
        }

        let elapsed_secs = elapsed.as_secs_f64();
        let rps = if elapsed_secs > 0.0 {
            total_requests as f64 / elapsed_secs
        } else {
            0.0
        };
        let bps = if elapsed_secs > 0.0 {
            total_bytes as f64 / elapsed_secs
        } else {
            0.0
        };

        GroupStats {
            total_requests,
            total_bytes,
            errors,
            elapsed,
            status_codes,
            p50_us: merged.value_at_quantile(0.50),
            p90_us: merged.value_at_quantile(0.90),
            p95_us: merged.value_at_quantile(0.95),
            p99_us: merged.value_at_quantile(0.99),
            max_us: merged.max(),
            min_us: merged.min(),
            mean_us: merged.mean(),
            stdev_us: merged.stdev(),
            requests_per_sec: rps,
            bytes_per_sec: bps,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GroupResult {
    pub name: String,
    pub endpoints: Vec<EndpointSummary>,
    pub connections: usize,
    pub stats: GroupStats,
}

#[derive(Debug, Serialize, Clone)]
pub struct EndpointSummary {
    pub url: String,
    pub method: String,
    pub weight_pct: f64,
}

impl EndpointSummary {
    pub fn from_endpoints(endpoints: &[EndpointConfig]) -> Vec<Self> {
        let total_weight: u32 = endpoints.iter().map(|e| e.weight).sum();
        endpoints
            .iter()
            .map(|e| EndpointSummary {
                url: e.url.clone(),
                method: e.method.clone(),
                weight_pct: if total_weight > 0 {
                    (e.weight as f64 / total_weight as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub struct OverallStats {
    pub total_requests: u64,
    pub total_bytes: u64,
    pub total_errors: u64,
    pub combined_rps: f64,
    pub combined_bps: f64,
}

impl OverallStats {
    pub fn from_groups(results: &[GroupResult]) -> Self {
        let total_requests: u64 = results.iter().map(|r| r.stats.total_requests).sum();
        let total_bytes: u64 = results.iter().map(|r| r.stats.total_bytes).sum();
        let total_errors: u64 = results.iter().map(|r| r.stats.errors).sum();
        let combined_rps: f64 = results.iter().map(|r| r.stats.requests_per_sec).sum();
        let combined_bps: f64 = results.iter().map(|r| r.stats.bytes_per_sec).sum();

        OverallStats {
            total_requests,
            total_bytes,
            total_errors,
            combined_rps,
            combined_bps,
        }
    }
}
