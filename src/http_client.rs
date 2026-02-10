use crate::config::{EndpointConfig, GroupConfig};
use crate::error::Result;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RequestTemplate {
    pub url: String,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Option<String>,
}

pub struct RequestSelector {
    templates: Vec<RequestTemplate>,
    dist: WeightedIndex<u32>,
}

impl RequestSelector {
    pub fn new(endpoints: &[EndpointConfig]) -> Result<Self> {
        let weights: Vec<u32> = endpoints.iter().map(|e| e.weight).collect();
        let dist = WeightedIndex::new(&weights).map_err(|e| {
            crate::error::DhanushError::Config(format!("Invalid weights: {}", e))
        })?;

        let templates: Vec<RequestTemplate> = endpoints
            .iter()
            .map(|ep| {
                let method = Method::from_str(&ep.method).unwrap_or(Method::GET);
                let mut headers = HeaderMap::new();
                for (k, v) in &ep.headers {
                    if let (Ok(name), Ok(val)) = (
                        HeaderName::from_str(k),
                        HeaderValue::from_str(v),
                    ) {
                        headers.insert(name, val);
                    }
                }
                RequestTemplate {
                    url: ep.url.clone(),
                    method,
                    headers,
                    body: ep.body.clone(),
                }
            })
            .collect();

        Ok(Self { templates, dist })
    }

    pub fn select(&self) -> &RequestTemplate {
        let mut rng = rand::thread_rng();
        let idx = self.dist.sample(&mut rng);
        &self.templates[idx]
    }

}

pub fn build_client(group: &GroupConfig) -> Result<Client> {
    let mut builder = Client::builder()
        .timeout(group.timeout)
        .pool_max_idle_per_host(group.connections)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true);

    if group.insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder.build().map_err(Into::into)
}
