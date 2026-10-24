//! arXiv Atom API. The manual asks for a single connection and roughly one request
//! every three seconds, so the local bucket is deliberately tiny.

use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::{clean_text, fetch_body, url_with, Engine, EngineError, EngineOutput, RawItem};

pub struct Arxiv {
    client: reqwest::Client,
}

impl Arxiv {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Engine for Arxiv {
    fn id(&self) -> &'static str {
        "arxiv"
    }

    fn weight(&self) -> f32 {
        0.95
    }

    fn domains(&self) -> &'static [&'static str] {
        &["academic"]
    }

    /// One request per three seconds, as documented in the arXiv API manual.
    fn rate_limit(&self) -> (f32, u64) {
        (1.0, 3)
    }
    fn timeout_hint(&self) -> Duration {
        Duration::from_millis(10_000)
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let url = url_with(
            "https://export.arxiv.org/api/query",
            &[
                ("search_query", format!("all:{query}")),
                ("start", "0".to_string()),
                ("max_results", max_results.to_string()),
            ],
        )
        .ok_or_else(|| EngineError::Parse("invalid url".into()))?;

        let (status, body) = fetch_body(&self.client, url, &[("accept", "application/atom+xml".into())], timeout).await?;
        if status == 429 {
            return Err(EngineError::RateLimited);
        }
        if status != 200 {
            return Err(EngineError::Upstream(status));
        }
        let items = parse(&body, max_results);
        // An empty result set is a legitimate answer; only transport/parse errors are Err.

        let parsed = items.len();
        Ok(EngineOutput { items, http_status: Some(status), parsed, elapsed_ms: started.elapsed().as_millis() })
    }
}

fn parse(body: &str, max_results: usize) -> Vec<RawItem> {
    let mut out = Vec::new();
    let mut rest = body;
    while out.len() < max_results {
        let Some(start) = rest.find("<entry") else { break };
        let after = &rest[start..];
        let Some(end) = after.find("</entry>") else { break };
        let block = &after[..end];
        rest = &after[end + "</entry>".len()..];

        let id = tag_text(block, "id").unwrap_or_default();
        let title = tag_text(block, "title").map(|t| clean_text(&t)).unwrap_or_default();
        if title.is_empty() || id.is_empty() {
            continue;
        }
        let summary = tag_text(block, "summary")
            .map(|s| clean_text(&s))
            .unwrap_or_default();
        let published = tag_text(block, "published").unwrap_or_default();
        let date = published.get(0..10).unwrap_or("");

        let mut snippet = String::new();
        if !date.is_empty() {
            snippet.push_str(date);
        }
        if !summary.is_empty() {
            if !snippet.is_empty() {
                snippet.push_str(" · ");
            }
            snippet.push_str(&summary.chars().take(200).collect::<String>());
        }
        out.push(RawItem {
            title,
            url: id,
            snippet,
            rank: (out.len() + 1) as u16,
            source: "arxiv",
        });
    }
    out
}

fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let gt = block[start..].find('>')? + start + 1;
    let close = format!("</{tag}>");
    let end = block[gt..].find(&close)? + gt;
    Some(block[gt..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_atom_entries() {
        let body = r#"<?xml version="1.0"?><feed>
          <entry>
            <id>http://arxiv.org/abs/2401.00001v1</id>
            <title>Scaling   Laws for
              Retrieval</title>
            <summary>We study retrieval at scale.</summary>
            <published>2024-01-02T00:00:00Z</published>
          </entry>
        </feed>"#;
        let items = parse(body, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "http://arxiv.org/abs/2401.00001v1");
        assert_eq!(items[0].title, "Scaling Laws for Retrieval");
        assert!(items[0].snippet.starts_with("2024-01-02"));
    }
}
