use crate::config::TestMode;
use crate::http_client::RequestSelector;
use crate::stats::WorkerStats;
use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

pub async fn run_worker(
    client: Client,
    selector: Arc<RequestSelector>,
    cancel: CancellationToken,
    test_mode: TestMode,
    request_counter: Option<Arc<AtomicU64>>,
    _worker_id: usize,
) -> WorkerStats {
    let mut stats = WorkerStats::new();

    loop {
        // Check termination condition
        match &test_mode {
            TestMode::Duration(_) => {
                if cancel.is_cancelled() {
                    break;
                }
            }
            TestMode::RequestCount(max) => {
                if let Some(ref counter) = request_counter {
                    let prev = counter.fetch_add(1, Ordering::Relaxed);
                    if prev >= *max {
                        break;
                    }
                }
            }
        }

        let template = selector.select();
        let start = Instant::now();

        let mut request = client.request(template.method.clone(), &template.url);
        request = request.headers(template.headers.clone());

        if let Some(ref body) = template.body {
            request = request.body(body.clone());
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                match response.bytes().await {
                    Ok(bytes) => {
                        let latency = start.elapsed().as_micros() as u64;
                        stats.record_success(latency, status, bytes.len() as u64);
                    }
                    Err(_) => {
                        let latency = start.elapsed().as_micros() as u64;
                        stats.record_error(latency);
                    }
                }
            }
            Err(_) => {
                let latency = start.elapsed().as_micros() as u64;
                stats.record_error(latency);
            }
        }
    }

    stats
}
