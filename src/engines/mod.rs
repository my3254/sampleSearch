pub mod bing;
pub mod so360;

use std::time::Duration;

use async_trait::async_trait;

use crate::config::Config;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// One normalized upstream hit. Source-local shape, no scoring yet.
#[derive(Debug, Clone)]
pub struct RawItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub rank: u16,
    pub source: &'static str,
}

#[derive(Debug)]
pub struct EngineOutput {
    pub items: Vec<RawItem>,
    pub http_status: Option<u16>,
    pub parsed: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("upstream status {0}")]
    Upstream(u16),
    #[error("parse failure: {0}")]
    Parse(String),
    #[error("blocked by upstream: {0}")]
    Blocked(String),
    #[error("network error: {0}")]
    Network(String),
}

impl EngineError {
    /// Errors that mean "this engine is not worth retrying right now".
    pub fn is_hard(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }
}

#[async_trait]
pub trait Engine: Send + Sync {
    fn id(&self) -> &'static str;
    /// RRF source weight (see architecture: fusion weights, degraded => *0.3).
    fn weight(&self) -> f32;
    /// Domains this engine can answer; `general` for web search engines.
    fn domains(&self) -> &'static [&'static str];
    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError>;
}

pub fn build_client(config: &Config) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(config.tier_b_timeout + Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::limited(3))
        .gzip(true);
    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    Ok(builder.build()?)
}

/// Collapse whitespace and drop zero-width junk; scrapers already decode entities.
pub fn clean_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
            }
            last_space = true;
        } else if ch == '\u{200b}' || ch == '\u{feff}' {
            continue;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

/// Detect the "safe verification" / captcha shells that mean we must not retry.
pub fn looks_blocked(body: &str) -> Option<&'static str> {
    const MARKERS: [(&str, &str); 5] = [
        ("安全验证", "baidu/bing security verification page"),
        ("captcha", "captcha page"),
        ("unusual traffic", "rate-limit page"),
        ("请输入验证码", "captcha prompt"),
        ("访问过于频繁", "frequency limit page"),
    ];
    let lower = body.to_lowercase();
    MARKERS
        .iter()
        .find(|(needle, _)| body.contains(needle) || lower.contains(&needle.to_lowercase()))
        .map(|(_, reason)| *reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_collapses_whitespace() {
        assert_eq!(clean_text("  a \n\t b  "), "a b");
    }

    #[test]
    fn blocked_markers_are_detected() {
        assert!(looks_blocked("<title>百度安全验证</title>").is_some());
        assert!(looks_blocked("please solve this CAPTCHA").is_some());
        assert!(looks_blocked("<h1>normal page</h1>").is_none());
    }
}
