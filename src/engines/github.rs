//! GitHub repository search. Anonymous quota: 10 requests/minute per IP;
//! a free personal access token raises it to 30/minute.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct Github {
    client: reqwest::Client,
}

impl Github {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for Github {
    fn id(&self) -> &'static str {
        "github"
    }

    fn weight(&self) -> f32 {
        0.9
    }

    fn domains(&self) -> &'static [&'static str] {
        &["code"]
    }

    /// 10/min anonymous, 30/min with a token. Keep headroom for other tools on the
    /// same IP, which share this budget.
    fn rate_limit(&self) -> (f32, u64) {
        (6.0, 60)
    }
    fn timeout_hint(&self) -> Duration {
        Duration::from_millis(8_000)
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let url = url_with(
            "https://api.github.com/search/repositories",
            &[
                ("q", query.to_string()),
                ("per_page", max_results.to_string()),
                ("sort", "stars".to_string()),
            ],
        )
        .ok_or_else(|| EngineError::Parse("invalid url".into()))?;

        let mut headers = vec![
            ("accept", "application/vnd.github+json".to_string()),
            ("x-github-api-version", "2022-11-28".to_string()),
        ];
        if let Ok(token) = std::env::var("SAMPLESEARCH_GITHUB_TOKEN") {
            if !token.is_empty() {
                headers.push(("authorization", format!("Bearer {token}")));
            }
        }

        let (status, body) = fetch_body(&self.client, url, &headers, timeout).await?;
        if status == 403 || status == 429 {
            return Err(EngineError::RateLimited);
        }
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
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in items.iter().take(max_results) {
        let name = entry.get("full_name").and_then(Value::as_str).unwrap_or_default();
        let url = entry.get("html_url").and_then(Value::as_str).unwrap_or_default();
        if name.is_empty() || url.is_empty() {
            continue;
        }
        let stars = entry.get("stargazers_count").and_then(Value::as_u64).unwrap_or(0);
        let language = entry.get("language").and_then(Value::as_str).unwrap_or("");
        let description = entry.get("description").and_then(Value::as_str).unwrap_or("");
        let mut snippet = format!("★{stars}");
        if !language.is_empty() {
            snippet.push_str(&format!(" · {language}"));
        }
        if !description.is_empty() {
            snippet.push_str(&format!(" · {}", clean_text(description)));
        }
        out.push(RawItem {
            title: name.to_string(),
            url: url.to_string(),
            snippet,
            rank: (out.len() + 1) as u16,
            source: "github",
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repository_items() {
        let value: Value = serde_json::from_str(
            r#"{"total_count":1,"items":[
                {"full_name":"tokio-rs/tokio","html_url":"https://github.com/tokio-rs/tokio",
                 "stargazers_count":28000,"language":"Rust","description":"A runtime for async apps"}]}"#,
        )
        .unwrap();
        let items = parse(&value, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "tokio-rs/tokio");
        assert!(items[0].snippet.contains("Rust"));
        assert_eq!(items[0].source, "github");
    }
}
