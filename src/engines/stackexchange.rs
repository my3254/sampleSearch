//! Stack Exchange search. Anonymous quota is 300 requests/day per IP (measured);
//! a free app key raises it to 10,000/day. The local bucket is sized for the
//! anonymous case so the service degrades instead of getting throttled.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct StackExchange {
    client: reqwest::Client,
}

impl StackExchange {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for StackExchange {
    fn id(&self) -> &'static str {
        "stackexchange"
    }

    fn weight(&self) -> f32 {
        0.8
    }

    fn domains(&self) -> &'static [&'static str] {
        &["code"]
    }

    /// 1 token per 5 minutes ≈ 288 calls/day, just under the anonymous 300/day.
    fn rate_limit(&self) -> (f32, u64) {
        (2.0, 600)
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
        let mut params = vec![
            ("order", "desc".to_string()),
            ("sort", "relevance".to_string()),
            ("q", query.to_string()),
            ("site", "stackoverflow".to_string()),
            ("pagesize", max_results.to_string()),
        ];
        if let Ok(key) = std::env::var("SAMPLESEARCH_SE_KEY") {
            if !key.is_empty() {
                params.push(("key", key));
            }
        }
        let url = url_with("https://api.stackexchange.com/2.3/search/advanced", &params)
            .ok_or_else(|| EngineError::Parse("invalid url".into()))?;

        let (status, body) = fetch_body(&self.client, url, &[], timeout).await?;
        if status == 400 || status == 429 {
            return Err(EngineError::RateLimited);
        }
        if status != 200 {
            return Err(EngineError::Upstream(status));
        }
        let value: Value =
            serde_json::from_str(&body).map_err(|e| EngineError::Parse(e.to_string()))?;
        if value
            .get("quota_remaining")
            .and_then(Value::as_i64)
            .is_some_and(|remaining| remaining <= 0)
        {
            return Err(EngineError::RateLimited);
        }
        let items = parse(&value, max_results);
        if items.is_empty() {
            return Err(EngineError::Parse("no stackoverflow questions".into()));
        }
        let parsed = items.len();
        Ok(EngineOutput { items, http_status: Some(status), parsed, elapsed_ms: started.elapsed().as_millis() })
    }
}

fn parse(value: &Value, max_results: usize) -> Vec<RawItem> {
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.iter().take(max_results) {
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .map(|t| clean_text(&unescape(t)))
            .unwrap_or_default();
        let url = item.get("link").and_then(Value::as_str).unwrap_or_default();
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let score = item.get("score").and_then(Value::as_i64).unwrap_or(0);
        let answers = item.get("answer_count").and_then(Value::as_i64).unwrap_or(0);
        let answered = item.get("is_answered").and_then(Value::as_bool).unwrap_or(false);
        let tags = item
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let mut snippet = format!("score {score} · {answers} answers");
        if answered {
            snippet.push_str(" · accepted");
        }
        if !tags.is_empty() {
            snippet.push_str(&format!(" · {tags}"));
        }
        out.push(RawItem {
            title,
            url: url.to_string(),
            snippet,
            rank: (out.len() + 1) as u16,
            source: "stackexchange",
        });
    }
    out
}

fn unescape(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_questions_with_metadata() {
        let value: Value = serde_json::from_str(
            r#"{"quota_remaining":299,"items":[
                {"title":"How to use tokio::select!?","link":"https://stackoverflow.com/q/1",
                 "score":12,"answer_count":3,"is_answered":true,"tags":["rust","tokio","async"]}]}"#,
        )
        .unwrap();
        let items = parse(&value, 3);
        assert_eq!(items.len(), 1);
        assert!(items[0].title.contains("tokio::select!"));
        assert!(items[0].snippet.contains("accepted"));
        assert!(items[0].snippet.contains("rust, tokio"));
    }
}
