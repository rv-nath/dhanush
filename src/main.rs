mod cli;
mod config;
mod engine;
mod error;
mod http_client;
mod reporter;
mod stats;
mod worker;

use clap::Parser;
use cli::CliArgs;
use colored::*;
use config::{Config, OutputFormat, TestMode};

fn main() {
    let args = CliArgs::parse();

    let config = match Config::from_cli(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Print test summary before running
    if config.output_format == OutputFormat::Text {
        print_test_summary(&config);
    }

    // Build tokio runtime with configurable thread count
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.threads)
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("{} Failed to build tokio runtime: {}", "Error:".red().bold(), e);
            std::process::exit(1);
        });

    let results = runtime.block_on(async {
        engine::run(&config).await
    });

    match results {
        Ok(results) => match config.output_format {
            OutputFormat::Text => reporter::print_text_results(&results),
            OutputFormat::Json => reporter::print_json_results(&results),
        },
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    }
}

fn print_test_summary(config: &Config) {
    println!();
    println!(
        "{}",
        "  dhanush - HTTP load testing tool"
            .bright_yellow()
            .bold()
    );
    println!(
        "  {} worker threads",
        config.threads.to_string().bright_green()
    );
    println!(
        "  {} group(s) configured",
        config.groups.len().to_string().bright_green()
    );

    for group in &config.groups {
        let mode_str = match &group.test_mode {
            TestMode::Duration(d) => format!("{:?} duration", d),
            TestMode::RequestCount(n) => format!("{} requests", n),
        };
        let ramp_str = match &group.ramp_up {
            Some(d) => format!(", ramp-up {:?}", d),
            None => String::new(),
        };
        println!(
            "  Group {}: {} connections, {}{}, {} endpoint(s)",
            group.name.cyan().bold(),
            group.connections.to_string().bright_green(),
            mode_str,
            ramp_str,
            group.endpoints.len().to_string().bright_green(),
        );
    }
    println!();
}
