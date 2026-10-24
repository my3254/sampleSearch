//! npm registry package search (public, no key).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct Npm {
    client: reqwest::Client,
}

impl Npm {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for Npm {
    fn id(&self) -> &'static str {
        "npm"
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

    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let url = url_with(
            "https://registry.npmjs.org/-/v1/search",
            &[("text", query.to_string()), ("size", max_results.to_string())],
        )
        .ok_or_else(|| EngineError::Parse("invalid url".into()))?;

        let (status, body) = fetch_body(&self.client, url, &[], timeout).await?;
        if status != 200 {
            return Err(EngineError::Upstream(status));
        }
        let value: Value =
            serde_json::from_str(&body).map_err(|e| EngineError::Parse(e.to_string()))?;
        let items = parse(&value, max_results);
        // An empty result set is a legitimate answer; only transport/parse errors are Err.

        let parsed = items.len();
        Ok(EngineOutput { items, http_status: Some(status), parsed, elapsed_ms: started.elapsed().as_millis() })
    }
}

fn parse(value: &Value, max_results: usize) -> Vec<RawItem> {
    let Some(objects) = value.get("objects").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for object in objects.iter().take(max_results) {
        let package = object.get("package").unwrap_or(object);
        let name = package.get("name").and_then(Value::as_str).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let version = package.get("version").and_then(Value::as_str).unwrap_or("");
        let url = package
            .pointer("/links/npm")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://www.npmjs.com/package/{name}"));
        let description = package
            .get("description")
            .and_then(Value::as_str)
            .map(clean_text)
            .unwrap_or_default();
        let mut snippet = if version.is_empty() { String::new() } else { format!("v{version}") };
        if !description.is_empty() {
            if !snippet.is_empty() {
                snippet.push_str(" · ");
            }
            snippet.push_str(&description);
        }
        out.push(RawItem {
            title: if version.is_empty() { name.to_string() } else { format!("{name}@{version}") },
            url,
            snippet,
            rank: (out.len() + 1) as u16,
            source: "npm",
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_objects() {
        let value: Value = serde_json::from_str(
            r#"{"objects":[
                {"package":{"name":"react","version":"19.0.0","description":"UI library",
                            "links":{"npm":"https://www.npmjs.com/package/react"}}}]}"#,
        )
        .unwrap();
        let items = parse(&value, 3);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "react@19.0.0");
        assert_eq!(items[0].url, "https://www.npmjs.com/package/react");
        assert_eq!(items[0].source, "npm");
    }
}
