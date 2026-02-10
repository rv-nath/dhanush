use crate::stats::{GroupResult, OverallStats};
use byte_unit::Byte;
use colored::*;
use serde::Serialize;

fn format_latency_us(us: u64) -> String {
    if us < 1_000 {
        format!("{:.2}us", us as f64)
    } else if us < 1_000_000 {
        format!("{:.2}ms", us as f64 / 1_000.0)
    } else {
        format!("{:.2}s", us as f64 / 1_000_000.0)
    }
}

fn format_latency_f64(us: f64) -> String {
    if us < 1_000.0 {
        format!("{:.2}us", us)
    } else if us < 1_000_000.0 {
        format!("{:.2}ms", us / 1_000.0)
    } else {
        format!("{:.2}s", us / 1_000_000.0)
    }
}

fn format_bytes(b: f64) -> String {
    let byte = Byte::from_f64(b).unwrap_or(Byte::from_u64(0));
    byte.get_appropriate_unit(byte_unit::UnitType::Decimal).to_string()
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub fn print_text_results(results: &[GroupResult]) {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════╗"
            .bright_yellow()
            .bold()
    );
    println!(
        "{}",
        "║         dhanush load test results            ║"
            .bright_yellow()
            .bold()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════╝"
            .bright_yellow()
            .bold()
    );

    for result in results {
        println!();
        println!(
            "{}",
            format!("── Group: {} ──────────────────────", result.name)
                .cyan()
                .bold()
        );

        // Endpoints
        println!("  {}:", "Endpoints".white().bold());
        for ep in &result.endpoints {
            println!(
                "    [{:>3.0}%] {:<4} {}",
                ep.weight_pct,
                ep.method.bright_green(),
                ep.url
            );
        }
        println!();

        // Connection info
        println!(
            "  {:<14} {}    {:<10} {}",
            "Connections:".white().bold(),
            result.connections,
            "Duration:".white().bold(),
            format!("{:.2?}", result.stats.elapsed),
        );
        println!();

        let stats = &result.stats;

        if stats.total_requests == 0 {
            println!("  {}", "No requests completed.".red());
            continue;
        }

        // Latency distribution
        println!("  {}:", "Latency Distribution".white().bold());
        println!("    {:<8}{}", "p50".dimmed(), format_latency_us(stats.p50_us));
        println!("    {:<8}{}", "p90".dimmed(), format_latency_us(stats.p90_us));
        println!("    {:<8}{}", "p95".dimmed(), format_latency_us(stats.p95_us));
        println!("    {:<8}{}", "p99".dimmed(), format_latency_us(stats.p99_us));
        println!("    {:<8}{}", "max".dimmed(), format_latency_us(stats.max_us));
        println!();

        // Latency stats
        println!("  {}:", "Latency Stats".white().bold());
        println!(
            "    {:<6}{:<14}{:<8}{}",
            "Avg".dimmed(),
            format_latency_f64(stats.mean_us),
            "Stdev".dimmed(),
            format_latency_f64(stats.stdev_us),
        );
        println!(
            "    {:<6}{:<14}{:<8}{}",
            "Min".dimmed(),
            format_latency_us(stats.min_us),
            "Max".dimmed(),
            format_latency_us(stats.max_us),
        );
        println!();

        // Throughput
        println!("  {}:", "Throughput".white().bold());
        println!(
            "    {:<20}{}",
            "Requests/sec:".dimmed(),
            format!("{:.2}", stats.requests_per_sec).bright_green(),
        );
        println!(
            "    {:<20}{}",
            "Transfer/sec:".dimmed(),
            format_bytes(stats.bytes_per_sec),
        );
        println!(
            "    {:<20}{}",
            "Total requests:".dimmed(),
            format_number(stats.total_requests),
        );
        println!(
            "    {:<20}{}",
            "Total transfer:".dimmed(),
            format_bytes(stats.total_bytes as f64),
        );
        println!();

        // Status codes
        println!("  {}:", "Status Codes".white().bold());
        let mut codes: Vec<_> = stats.status_codes.iter().collect();
        codes.sort_by_key(|(code, _)| *code);

        for (code, count) in &codes {
            let pct = **count as f64 / stats.total_requests as f64 * 100.0;
            let code_str = format!("{}", code);
            let color_code = match *code / 100 {
                2 => code_str.green(),
                3 => code_str.yellow(),
                4 => code_str.red(),
                5 => code_str.bright_red(),
                _ => code_str.white(),
            };
            println!(
                "    {}:  {:>8}  ({:>5.2}%)",
                color_code,
                format_number(**count),
                pct
            );
        }

        if stats.errors > 0 {
            let pct = stats.errors as f64 / stats.total_requests as f64 * 100.0;
            println!(
                "  {}  {:>8}  ({:>5.2}%)",
                "Errors:".red().bold(),
                format_number(stats.errors),
                pct,
            );
        }
    }

    // Overall summary
    if results.len() > 1 {
        let overall = OverallStats::from_groups(results);
        println!();
        println!(
            "{}",
            "── Overall Summary ───────────────────────────"
                .bright_yellow()
                .bold()
        );
        println!(
            "  {:<20}{}",
            "Total requests:".white().bold(),
            format_number(overall.total_requests),
        );
        println!(
            "  {:<20}{}",
            "Total errors:".white().bold(),
            format_number(overall.total_errors),
        );
        println!(
            "  {:<20}{}",
            "Combined RPS:".white().bold(),
            format!("{:.2}", overall.combined_rps).bright_green(),
        );
        println!(
            "  {:<20}{}",
            "Combined transfer:".white().bold(),
            format_bytes(overall.total_bytes as f64),
        );
    }

    println!();
}

// JSON output types
#[derive(Serialize)]
struct JsonOutput {
    groups: Vec<JsonGroupResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<JsonSummary>,
}

#[derive(Serialize)]
struct JsonGroupResult {
    name: String,
    endpoints: Vec<JsonEndpoint>,
    connections: usize,
    duration_secs: f64,
    latency: JsonLatency,
    throughput: JsonThroughput,
    status_codes: std::collections::HashMap<String, u64>,
    errors: u64,
}

#[derive(Serialize)]
struct JsonEndpoint {
    url: String,
    method: String,
    weight_pct: f64,
}

#[derive(Serialize)]
struct JsonLatency {
    p50_us: u64,
    p90_us: u64,
    p95_us: u64,
    p99_us: u64,
    max_us: u64,
    min_us: u64,
    mean_us: f64,
    stdev_us: f64,
}

#[derive(Serialize)]
struct JsonThroughput {
    requests_per_sec: f64,
    bytes_per_sec: f64,
    total_requests: u64,
    total_bytes: u64,
}

#[derive(Serialize)]
struct JsonSummary {
    total_requests: u64,
    total_errors: u64,
    combined_rps: f64,
    total_bytes: u64,
}

pub fn print_json_results(results: &[GroupResult]) {
    let groups: Vec<JsonGroupResult> = results
        .iter()
        .map(|r| {
            let status_codes: std::collections::HashMap<String, u64> = r
                .stats
                .status_codes
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect();

            JsonGroupResult {
                name: r.name.clone(),
                endpoints: r
                    .endpoints
                    .iter()
                    .map(|e| JsonEndpoint {
                        url: e.url.clone(),
                        method: e.method.clone(),
                        weight_pct: e.weight_pct,
                    })
                    .collect(),
                connections: r.connections,
                duration_secs: r.stats.elapsed.as_secs_f64(),
                latency: JsonLatency {
                    p50_us: r.stats.p50_us,
                    p90_us: r.stats.p90_us,
                    p95_us: r.stats.p95_us,
                    p99_us: r.stats.p99_us,
                    max_us: r.stats.max_us,
                    min_us: r.stats.min_us,
                    mean_us: r.stats.mean_us,
                    stdev_us: r.stats.stdev_us,
                },
                throughput: JsonThroughput {
                    requests_per_sec: r.stats.requests_per_sec,
                    bytes_per_sec: r.stats.bytes_per_sec,
                    total_requests: r.stats.total_requests,
                    total_bytes: r.stats.total_bytes,
                },
                status_codes,
                errors: r.stats.errors,
            }
        })
        .collect();

    let summary = if results.len() > 1 {
        let overall = OverallStats::from_groups(results);
        Some(JsonSummary {
            total_requests: overall.total_requests,
            total_errors: overall.total_errors,
            combined_rps: overall.combined_rps,
            total_bytes: overall.total_bytes,
        })
    } else {
        None
    };

    let output = JsonOutput { groups, summary };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
