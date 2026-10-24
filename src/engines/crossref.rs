//! Crossref scholarly metadata (free; a `mailto` puts the request in the polite pool).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct Crossref {
    client: reqwest::Client,
}

impl Crossref {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for Crossref {
    fn id(&self) -> &'static str {
        "crossref"
    }

    fn weight(&self) -> f32 {
        0.9
    }

    fn domains(&self) -> &'static [&'static str] {
        &["academic"]
    }

    fn rate_limit(&self) -> (f32, u64) {
        (20.0, 60)
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
        let mailto = std::env::var("SAMPLESEARCH_CONTACT").unwrap_or_else(|_| "samplesearch@example.com".into());
        let url = url_with(
            "https://api.crossref.org/works",
            &[
                ("query", query.to_string()),
                ("rows", max_results.to_string()),
                ("select", "title,URL,abstract,issued,container-title,author".to_string()),
                ("mailto", mailto),
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
        if items.is_empty() {
            return Err(EngineError::Parse("no crossref works".into()));
        }
        let parsed = items.len();
        Ok(EngineOutput { items, http_status: Some(status), parsed, elapsed_ms: started.elapsed().as_millis() })
    }
}

fn parse(value: &Value, max_results: usize) -> Vec<RawItem> {
    let Some(items) = value.pointer("/message/items").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.iter().take(max_results) {
        let title = item
            .get("title")
            .and_then(Value::as_array)
            .and_then(|t| t.first())
            .and_then(Value::as_str)
            .map(|t| clean_text(&strip_tags(t)))
            .unwrap_or_default();
        let url = item.get("URL").and_then(Value::as_str).unwrap_or_default();
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let year = item
            .pointer("/issued/date-parts/0/0")
            .and_then(Value::as_u64)
            .map(|y| y.to_string())
            .unwrap_or_default();
        let venue = item
            .get("container-title")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let abstract_text = item
            .get("abstract")
            .and_then(Value::as_str)
            .map(|a| clean_text(&strip_tags(a)))
            .unwrap_or_default();

        let mut snippet = String::new();
        if !year.is_empty() {
            snippet.push_str(&year);
        }
        if !venue.is_empty() {
            if !snippet.is_empty() {
                snippet.push_str(" · ");
            }
            snippet.push_str(venue);
        }
        if !abstract_text.is_empty() {
            snippet.push_str(&format!(" · {}", abstract_text.chars().take(180).collect::<String>()));
        }
        out.push(RawItem {
            title,
            url: url.to_string(),
            snippet,
            rank: (out.len() + 1) as u16,
            source: "crossref",
        });
    }
    out
}

/// Crossref abstracts arrive as JATS XML fragments.
fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_works_and_strips_jats() {
        let value: Value = serde_json::from_str(
            r#"{"message":{"items":[
                {"title":["Attention Is All You Need"],"URL":"https://doi.org/10.1/x",
                 "issued":{"date-parts":[[2017]]},"container-title":["NeurIPS"],
                 "abstract":"<jats:p>We propose the Transformer.</jats:p>"}]}}"#,
        )
        .unwrap();
        let items = parse(&value, 3);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Attention Is All You Need");
        assert!(items[0].snippet.contains("2017"));
        assert!(items[0].snippet.contains("Transformer"));
        assert!(!items[0].snippet.contains('<'));
    }
}
