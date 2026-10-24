pub mod arxiv;
pub mod bing;
pub mod crossref;
pub mod github;
pub mod hn;
pub mod npm;
pub mod pypi;
pub mod so360;
pub mod stackexchange;

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
    #[error("timeout")]
    Timeout,
    #[error("upstream status {0}")]
    Upstream(u16),
    #[error("parse failure: {0}")]
    Parse(String),
    #[error("blocked by upstream: {0}")]
    Blocked(String),
    #[error("rate limited by upstream")]
    RateLimited,
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
    /// Local politeness budget: (capacity, seconds to refill the whole bucket).
    /// Sized from each provider's published or measured anonymous quota.
    fn rate_limit(&self) -> (f32, u64) {
        (4.0, 60)
    }
    /// Per-engine timeout floor. Public result pages answer in ~300 ms; official APIs
    /// regularly take 3-5 s, so a single tier timeout cannot serve both.
    fn timeout_hint(&self) -> Duration {
        Duration::from_millis(1800)
    }
    /// Whether this engine can answer the query at all. Vertical engines narrow this
    /// (PyPI only answers package-shaped queries); general engines accept everything.
    fn supports(&self, _query: &str) -> bool {
        true
    }
    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError>;
}

/// Shared GET for JSON/XML APIs. Keeps adapters to parse + map only.
pub(crate) async fn fetch_body(
    client: &reqwest::Client,
    url: reqwest::Url,
    headers: &[(&str, String)],
    timeout: Duration,
) -> Result<(u16, String), EngineError> {
    let mut request = client.get(url).timeout(timeout);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = request.send().await.map_err(map_reqwest)?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(map_reqwest)?;
    Ok((status, body))
}

pub(crate) fn map_reqwest(error: reqwest::Error) -> EngineError {
    if error.is_timeout() {
        EngineError::Timeout
    } else {
        EngineError::Network(error.to_string())
    }
}

pub(crate) fn url_with(base: &str, params: &[(&str, String)]) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).ok()?;
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    Some(url)
}

pub fn build_client(config: &Config) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(UA)
        // Ceiling only: each call passes its own per-request timeout. A degraded
        // upstream can legitimately take >5 s, so the client must not cut first.
        .timeout(Duration::from_secs(20))
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
