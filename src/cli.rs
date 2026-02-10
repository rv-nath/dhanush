use clap::Parser;

/// dhanush - High-performance HTTP load testing tool
#[derive(Parser, Debug)]
#[command(name = "dhanush", version, about)]
pub struct CliArgs {
    /// Target URL (for single-URL mode)
    pub url: Option<String>,

    /// Number of concurrent connections per group
    #[arg(short = 'c', long = "connections", default_value = "10")]
    pub connections: usize,

    /// Test duration (e.g. "10s", "1m", "2m30s")
    #[arg(short = 'd', long = "duration", default_value = "10s")]
    pub duration: String,

    /// Total number of requests (switches to count mode, overrides duration)
    #[arg(short = 'n', long = "num-requests")]
    pub num_requests: Option<u64>,

    /// Number of tokio worker threads
    #[arg(short = 't', long = "threads")]
    pub threads: Option<usize>,

    /// HTTP method
    #[arg(short = 'm', long = "method", default_value = "GET")]
    pub method: String,

    /// HTTP headers (repeatable, format: "Key: Value")
    #[arg(short = 'H', long = "header")]
    pub headers: Vec<String>,

    /// Request body
    #[arg(short = 'b', long = "body")]
    pub body: Option<String>,

    /// Request timeout (e.g. "5s", "30s")
    #[arg(long = "timeout", default_value = "5s")]
    pub timeout: String,

    /// Disable TLS certificate verification
    #[arg(long = "insecure", default_value = "false")]
    pub insecure: bool,

    /// Output format: text or json
    #[arg(long = "output-format", default_value = "text")]
    pub output_format: String,

    /// Disable progress display
    #[arg(long = "no-progress", default_value = "false")]
    pub no_progress: bool,

    /// Ramp-up time to gradually add connections (e.g. "5s", "30s")
    #[arg(short = 'r', long = "ramp-up")]
    pub ramp_up: Option<String>,

    /// Path to YAML config file for multi-group mode
    #[arg(long = "config")]
    pub config: Option<String>,
}
