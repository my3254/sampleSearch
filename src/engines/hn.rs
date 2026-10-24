//! Hacker News via the Algolia API (free, no key, no published hard limit).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct HackerNews {
    client: reqwest::Client,
}

impl HackerNews {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for HackerNews {
    fn id(&self) -> &'static str {
        "hn"
    }

    fn weight(&self) -> f32 {
        0.7
    }

    fn domains(&self) -> &'static [&'static str] {
        &["code", "social_media"]
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
            "https://hn.algolia.com/api/v1/search",
            &[
                ("query", query.to_string()),
                ("hitsPerPage", max_results.to_string()),
                ("tags", "(story,comment)".to_string()),
            ],
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
    let Some(hits) = value.get("hits").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for hit in hits.iter().take(max_results) {
        let object_id = hit.get("objectID").and_then(Value::as_str).unwrap_or_default();
        let title = hit
            .get("title")
            .or_else(|| hit.get("story_title"))
            .and_then(Value::as_str)
            .map(clean_text)
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let url = hit
            .get("url")
            .or_else(|| hit.get("story_url"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://news.ycombinator.com/item?id={object_id}"));
        let points = hit.get("points").and_then(Value::as_u64).unwrap_or(0);
        let comments = hit.get("num_comments").and_then(Value::as_u64).unwrap_or(0);
        let text = hit
            .get("story_text")
            .or_else(|| hit.get("comment_text"))
            .and_then(Value::as_str)
            .map(clean_text)
            .unwrap_or_default();
        let mut snippet = format!("{points} points · {comments} comments");
        if !text.is_empty() {
            snippet.push_str(&format!(" · {}", text.chars().take(160).collect::<String>()));
        }
        out.push(RawItem {
            title,
            url,
            snippet,
            rank: (out.len() + 1) as u16,
            source: "hn",
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hits_with_fallback_url() {
        let value: Value = serde_json::from_str(
            r#"{"hits":[
                {"objectID":"1","title":"Show HN: async runtime","url":"https://example.com/a","points":120,"num_comments":44},
                {"objectID":"2","title":"Ask HN: which runtime?","points":3,"num_comments":7}]}"#,
        )
        .unwrap();
        let items = parse(&value, 5);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://example.com/a");
        assert!(items[0].snippet.contains("120 points"));
        assert_eq!(items[1].url, "https://news.ycombinator.com/item?id=2");
    }
}
