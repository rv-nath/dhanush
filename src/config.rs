use crate::cli::CliArgs;
use crate::error::{DhanushError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum TestMode {
    Duration(Duration),
    RequestCount(u64),
}

#[derive(Debug, Clone)]
pub struct EndpointConfig {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub weight: u32,
}

#[derive(Debug, Clone)]
pub struct GroupConfig {
    pub name: String,
    pub endpoints: Vec<EndpointConfig>,
    pub connections: usize,
    pub test_mode: TestMode,
    pub timeout: Duration,
    pub insecure: bool,
    pub ramp_up: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub groups: Vec<GroupConfig>,
    pub threads: usize,
    pub output_format: OutputFormat,
    pub no_progress: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
}

// YAML deserialization types
#[derive(Deserialize, Debug)]
struct YamlConfig {
    defaults: Option<YamlDefaults>,
    groups: Vec<YamlGroup>,
}

#[derive(Deserialize, Debug)]
struct YamlDefaults {
    connections: Option<usize>,
    duration: Option<String>,
    #[serde(rename = "num-requests")]
    num_requests: Option<u64>,
    #[allow(dead_code)]
    threads: Option<usize>,
    timeout: Option<String>,
    insecure: Option<bool>,
    #[serde(rename = "ramp-up")]
    ramp_up: Option<String>,
}

#[derive(Deserialize, Debug)]
struct YamlGroup {
    name: String,
    connections: Option<usize>,
    duration: Option<String>,
    #[serde(rename = "num-requests")]
    num_requests: Option<u64>,
    timeout: Option<String>,
    insecure: Option<bool>,
    #[serde(rename = "ramp-up")]
    ramp_up: Option<String>,
    endpoints: Vec<YamlEndpoint>,
}

#[derive(Deserialize, Debug)]
struct YamlEndpoint {
    url: String,
    method: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
    weight: Option<u32>,
}

fn parse_duration_str(s: &str) -> Result<Duration> {
    humantime::parse_duration(s).map_err(|e| DhanushError::Config(format!("Invalid duration '{}': {}", s, e)))
}

fn parse_headers(header_strings: &[String]) -> Result<HashMap<String, String>> {
    let mut headers = HashMap::new();
    for h in header_strings {
        let parts: Vec<&str> = h.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(DhanushError::Config(format!(
                "Invalid header format '{}', expected 'Key: Value'",
                h
            )));
        }
        headers.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
    }
    Ok(headers)
}

impl Config {
    pub fn from_cli(args: &CliArgs) -> Result<Self> {
        let output_format = match args.output_format.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "text" => OutputFormat::Text,
            other => {
                return Err(DhanushError::Config(format!(
                    "Unknown output format '{}', expected 'text' or 'json'",
                    other
                )));
            }
        };

        let threads = args.threads.unwrap_or_else(num_cpus::get);

        if let Some(config_path) = &args.config {
            let content = std::fs::read_to_string(config_path)?;
            let yaml_config: YamlConfig = serde_yaml::from_str(&content)?;
            let groups = Self::build_groups_from_yaml(yaml_config, args)?;
            Ok(Config {
                groups,
                threads,
                output_format,
                no_progress: args.no_progress,
            })
        } else {
            let url = args.url.as_ref().ok_or_else(|| {
                DhanushError::Config("URL is required in single-URL mode. Provide a URL or use --config".into())
            })?;

            let headers = parse_headers(&args.headers)?;
            let test_mode = if let Some(n) = args.num_requests {
                TestMode::RequestCount(n)
            } else {
                TestMode::Duration(parse_duration_str(&args.duration)?)
            };

            let endpoint = EndpointConfig {
                url: url.clone(),
                method: args.method.to_uppercase(),
                headers,
                body: args.body.clone(),
                weight: 1,
            };

            let ramp_up = match &args.ramp_up {
                Some(s) => Some(parse_duration_str(s)?),
                None => None,
            };

            let group = GroupConfig {
                name: "default".to_string(),
                endpoints: vec![endpoint],
                connections: args.connections,
                test_mode,
                timeout: parse_duration_str(&args.timeout)?,
                insecure: args.insecure,
                ramp_up,
            };

            Ok(Config {
                groups: vec![group],
                threads,
                output_format,
                no_progress: args.no_progress,
            })
        }
    }

    fn build_groups_from_yaml(yaml: YamlConfig, cli: &CliArgs) -> Result<Vec<GroupConfig>> {
        let defaults = yaml.defaults.unwrap_or(YamlDefaults {
            connections: None,
            duration: None,
            num_requests: None,
            threads: None,
            timeout: None,
            insecure: None,
            ramp_up: None,
        });

        let default_connections = defaults.connections.unwrap_or(10);
        let default_duration = defaults.duration.as_deref().unwrap_or("10s");
        let default_timeout = defaults.timeout.as_deref().unwrap_or("5s");
        let default_insecure = defaults.insecure.unwrap_or(false);
        let default_ramp_up = &defaults.ramp_up;

        let mut groups = Vec::new();

        for yg in yaml.groups {
            let connections = yg.connections.unwrap_or(default_connections);

            let test_mode = if let Some(n) = cli.num_requests {
                // CLI override
                TestMode::RequestCount(n)
            } else if let Some(n) = yg.num_requests {
                TestMode::RequestCount(n)
            } else if let Some(ref dur) = yg.duration {
                TestMode::Duration(parse_duration_str(dur)?)
            } else if defaults.num_requests.is_some() {
                TestMode::RequestCount(defaults.num_requests.unwrap())
            } else {
                TestMode::Duration(parse_duration_str(default_duration)?)
            };

            let timeout_str = yg.timeout.as_deref().unwrap_or(default_timeout);
            let insecure = yg.insecure.unwrap_or(default_insecure);

            // ramp-up: CLI override > group > defaults
            let ramp_up = if let Some(ref s) = cli.ramp_up {
                Some(parse_duration_str(s)?)
            } else if let Some(ref s) = yg.ramp_up {
                Some(parse_duration_str(s)?)
            } else if let Some(ref s) = default_ramp_up {
                Some(parse_duration_str(s)?)
            } else {
                None
            };

            let mut endpoints = Vec::new();
            for ye in yg.endpoints {
                let ep = EndpointConfig {
                    url: ye.url,
                    method: ye.method.unwrap_or_else(|| "GET".to_string()).to_uppercase(),
                    headers: ye.headers.unwrap_or_default(),
                    body: ye.body,
                    weight: ye.weight.unwrap_or(1),
                };
                endpoints.push(ep);
            }

            if endpoints.is_empty() {
                return Err(DhanushError::Config(format!(
                    "Group '{}' has no endpoints",
                    yg.name
                )));
            }

            let total_weight: u32 = endpoints.iter().map(|e| e.weight).sum();
            if total_weight == 0 {
                return Err(DhanushError::Config(format!(
                    "Group '{}' has zero total weight",
                    yg.name
                )));
            }

            groups.push(GroupConfig {
                name: yg.name,
                endpoints,
                connections,
                test_mode,
                timeout: parse_duration_str(timeout_str)?,
                insecure,
                ramp_up,
            });
        }

        if groups.is_empty() {
            return Err(DhanushError::Config("Config file has no groups".into()));
        }

        Ok(groups)
    }
}
