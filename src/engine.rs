use crate::config::{Config, GroupConfig, TestMode};
use crate::error::Result;
use crate::http_client::{build_client, RequestSelector};
use crate::stats::{EndpointSummary, GroupResult, GroupStats};
use crate::worker::run_worker;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub async fn run(config: &Config) -> Result<Vec<GroupResult>> {
    let multi_progress = if !config.no_progress {
        Some(MultiProgress::new())
    } else {
        None
    };

    let mut group_handles = JoinSet::new();

    for group_config in &config.groups {
        let gc = group_config.clone();
        let mp = multi_progress.clone();
        let no_progress = config.no_progress;

        group_handles.spawn(async move {
            run_group(&gc, mp, no_progress).await
        });
    }

    let mut results = Vec::new();
    while let Some(res) = group_handles.join_next().await {
        match res {
            Ok(Ok(group_result)) => results.push(group_result),
            Ok(Err(e)) => eprintln!("Group error: {}", e),
            Err(e) => eprintln!("Task join error: {}", e),
        }
    }

    // Sort results by group name for consistent output
    results.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(results)
}

async fn run_group(
    group: &GroupConfig,
    multi_progress: Option<MultiProgress>,
    no_progress: bool,
) -> Result<GroupResult> {
    let client = build_client(group)?;
    let selector = Arc::new(RequestSelector::new(&group.endpoints)?);
    let cancel = CancellationToken::new();
    let endpoint_summaries = EndpointSummary::from_endpoints(&group.endpoints);

    // Setup progress bar
    let pb = if !no_progress {
        let pb = match &group.test_mode {
            TestMode::Duration(d) => {
                let pb = ProgressBar::new(d.as_secs());
                pb.set_style(
                    ProgressStyle::with_template(
                        "{prefix:.cyan.bold} [{elapsed_precise}] {bar:30.green/dim} {pos}/{len}s {msg}"
                    )
                    .unwrap()
                    .progress_chars("##-"),
                );
                pb
            }
            TestMode::RequestCount(n) => {
                let pb = ProgressBar::new(*n);
                pb.set_style(
                    ProgressStyle::with_template(
                        "{prefix:.cyan.bold} [{elapsed_precise}] {bar:30.green/dim} {pos}/{len} reqs {msg}"
                    )
                    .unwrap()
                    .progress_chars("##-"),
                );
                pb
            }
        };
        pb.set_prefix(group.name.clone());
        if let Some(ref mp) = multi_progress {
            Some(mp.add(pb))
        } else {
            Some(pb)
        }
    } else {
        None
    };

    // Request counter for count mode
    let request_counter = match &group.test_mode {
        TestMode::RequestCount(_) => Some(Arc::new(AtomicU64::new(0))),
        _ => None,
    };

    let start = Instant::now();

    // Spawn workers -- with optional ramp-up
    let mut worker_handles = JoinSet::new();

    if let Some(ramp_up) = group.ramp_up {
        // Stagger worker spawns linearly over the ramp-up period.
        // Worker i launches at: ramp_up * i / connections
        // (worker 0 starts immediately, last worker starts just before ramp_up expires)
        let total = group.connections;
        let ramp_us = ramp_up.as_micros() as u64;

        for worker_id in 0..total {
            let delay_us = if total <= 1 {
                0
            } else {
                ramp_us * worker_id as u64 / (total as u64 - 1)
            };

            let client = client.clone();
            let selector = Arc::clone(&selector);
            let cancel = cancel.clone();
            let test_mode = group.test_mode.clone();
            let counter = request_counter.clone();

            worker_handles.spawn(async move {
                if delay_us > 0 {
                    tokio::time::sleep(Duration::from_micros(delay_us)).await;
                }
                run_worker(client, selector, cancel, test_mode, counter, worker_id).await
            });
        }
    } else {
        // No ramp-up: spawn all workers immediately
        for worker_id in 0..group.connections {
            let client = client.clone();
            let selector = Arc::clone(&selector);
            let cancel = cancel.clone();
            let test_mode = group.test_mode.clone();
            let counter = request_counter.clone();

            worker_handles.spawn(async move {
                run_worker(client, selector, cancel, test_mode, counter, worker_id).await
            });
        }
    }

    // Duration-based progress update and cancellation
    if let TestMode::Duration(duration) = &group.test_mode {
        let duration = *duration;
        let cancel = cancel.clone();
        let pb = pb.clone();

        tokio::spawn(async move {
            let start = Instant::now();
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let elapsed = start.elapsed();
                if let Some(ref pb) = pb {
                    pb.set_position(elapsed.as_secs().min(duration.as_secs()));
                }
                if elapsed >= duration {
                    cancel.cancel();
                    if let Some(ref pb) = pb {
                        pb.finish_with_message("done");
                    }
                    break;
                }
            }
        });
    } else {
        // Count mode: update progress bar periodically
        let counter = request_counter.clone();
        let pb = pb.clone();
        let cancel_clone = cancel.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if cancel_clone.is_cancelled() {
                    break;
                }
                if let Some(ref c) = counter {
                    let count = c.load(std::sync::atomic::Ordering::Relaxed);
                    if let Some(ref pb) = pb {
                        pb.set_position(count);
                    }
                }
            }
        });
    }

    // Collect worker results
    let mut worker_stats = Vec::new();
    while let Some(res) = worker_handles.join_next().await {
        match res {
            Ok(stats) => worker_stats.push(stats),
            Err(e) => eprintln!("Worker join error: {}", e),
        }
    }

    let elapsed = start.elapsed();

    // Cancel any remaining background tasks and finish progress
    cancel.cancel();
    if let Some(ref pb) = pb {
        match &group.test_mode {
            TestMode::RequestCount(_) => {
                if let Some(ref c) = request_counter {
                    pb.set_position(c.load(std::sync::atomic::Ordering::Relaxed));
                }
            }
            _ => {}
        }
        pb.finish_with_message("done");
    }

    let stats = GroupStats::from_workers(worker_stats, elapsed);

    Ok(GroupResult {
        name: group.name.clone(),
        endpoints: endpoint_summaries,
        connections: group.connections,
        stats,
    })
}
