//! PyPI package lookup.
//!
//! PyPI has no public search API any more (XML-RPC search was removed), so this
//! engine deliberately handles *package lookups only*: it fires when the query is a
//! single package-shaped token and returns that package's metadata. It reports a miss
//! instead of pretending to search the index.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, Engine, EngineError, EngineOutput, RawItem};

pub struct PyPI {
    client: reqwest::Client,
}

impl PyPI {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

fn package_shaped(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.contains(char::is_whitespace) || trimmed.chars().count() < 2 {
        return false;
    }
    trimmed.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[async_trait]
impl Engine for PyPI {
    fn id(&self) -> &'static str {
        "pypi"
    }

    fn weight(&self) -> f32 {
        0.8
    }

    fn domains(&self) -> &'static [&'static str] {
        &["code"]
    }

    fn rate_limit(&self) -> (f32, u64) {
        (30.0, 60)
    }
    fn timeout_hint(&self) -> Duration {
        Duration::from_millis(6_000)
    }

    fn supports(&self, query: &str) -> bool {
        package_shaped(query)
    }

    async fn search(
        &self,
        query: &str,
        _max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let name = query.trim();
        let url = reqwest::Url::parse(&format!("https://pypi.org/pypi/{}/json", name))
            .map_err(|e| EngineError::Parse(e.to_string()))?;

        let (status, body) = fetch_body(&self.client, url, &[("accept", "application/json".into())], timeout).await?;
        if status == 404 {
            return Err(EngineError::Parse(format!("no PyPI package named {name}")));
        }
        if status != 200 {
            return Err(EngineError::Upstream(status));
        }
        let value: Value =
            serde_json::from_str(&body).map_err(|e| EngineError::Parse(e.to_string()))?;
        let items = parse(&value, name);
        let parsed = items.len();
        Ok(EngineOutput { items, http_status: Some(status), parsed, elapsed_ms: started.elapsed().as_millis() })
    }
}

fn parse(value: &Value, fallback_name: &str) -> Vec<RawItem> {
    let info = value.get("info").unwrap_or(value);
    let name = info.get("name").and_then(Value::as_str).unwrap_or(fallback_name);
    let version = info.get("version").and_then(Value::as_str).unwrap_or("");
    let summary = info
        .get("summary")
        .and_then(Value::as_str)
        .map(clean_text)
        .unwrap_or_default();
    if name.is_empty() {
        return Vec::new();
    }
    let mut snippet = if version.is_empty() { String::new() } else { format!("v{version}") };
    if !summary.is_empty() {
        if !snippet.is_empty() {
            snippet.push_str(" · ");
        }
        snippet.push_str(&summary);
    }
    vec![RawItem {
        title: if version.is_empty() { name.to_string() } else { format!("{name} {version}") },
        url: format!("https://pypi.org/project/{name}/"),
        snippet,
        rank: 1,
        source: "pypi",
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_package_shaped_queries() {
        assert!(package_shaped("requests"));
        assert!(package_shaped("python-dateutil"));
        assert!(!package_shaped("how to parse json"));
        assert!(!package_shaped("a"));
    }

    #[test]
    fn parses_package_info() {
        let value: Value = serde_json::from_str(
            r#"{"info":{"name":"requests","version":"2.34.2","summary":"Python HTTP for Humans."}}"#,
        )
        .unwrap();
        let items = parse(&value, "requests");
        assert_eq!(items[0].title, "requests 2.34.2");
        assert_eq!(items[0].url, "https://pypi.org/project/requests/");
        assert!(items[0].snippet.contains("HTTP"));
    }
}
