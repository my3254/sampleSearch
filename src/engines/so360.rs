//! 360 (so.com) HTML adapter — the primary Chinese general-purpose engine.
//!
//! Measured (2026-10-24): 8/8 relevance on the probe set, ~390ms, and the real
//! target URL is exposed as `data-mdurl` so no redirect following is needed.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use scraper::{Html, Selector};

use super::{clean_text, looks_blocked, Engine, EngineError, EngineOutput, RawItem};

const ENDPOINT: &str = "https://www.so.com/s";

pub struct So360 {
    client: reqwest::Client,
}

impl So360 {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

fn item_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("li.res-list").expect("valid selector"))
}

fn title_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("h3").expect("valid selector"))
}

fn snippet_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("p.res-desc, .res-comm-con, p").expect("valid selector"))
}

fn mdurl_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("[data-mdurl]").expect("valid selector"))
}

fn anchor_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("h3 a[href], a[href]").expect("valid selector"))
}

#[async_trait]
impl Engine for So360 {
    fn id(&self) -> &'static str {
        "so360"
    }

    fn weight(&self) -> f32 {
        1.0
    }

    fn domains(&self) -> &'static [&'static str] {
        &["general", "news", "legal", "social_media", "finance"]
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let mut url = url::Url::parse(ENDPOINT).expect("static endpoint");
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("q", query);
        }

        let resp = self
            .client
            .get(url)
            .header("accept-language", "zh-CN,zh;q=0.9")
            .header("accept", "text/html,application/xhtml+xml")
            .timeout(timeout)
            .send()
            .await
            .map_err(map_reqwest)?;

        let status = resp.status();
        let body = resp.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            return Err(EngineError::Upstream(status.as_u16()));
        }
        if let Some(reason) = looks_blocked(&body) {
            return Err(EngineError::Blocked(reason.to_string()));
        }

        let items = parse(&body, max_results);
        let parsed = items.len();
        if parsed == 0 {
            return Err(EngineError::Parse("no li.res-list blocks".into()));
        }

        Ok(EngineOutput {
            items,
            http_status: Some(status.as_u16()),
            parsed,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }
}

fn parse(body: &str, max_results: usize) -> Vec<RawItem> {
    let doc = Html::parse_document(body);
    let mut out = Vec::new();
    for el in doc.select(item_sel()) {
        if out.len() >= max_results {
            break;
        }
        let title = el
            .select(title_sel())
            .next()
            .map(|n| clean_text(&n.text().collect::<String>()))
            .filter(|t| !t.is_empty())
            .unwrap_or_default();

        let url = el
            .value()
            .attr("data-mdurl")
            .map(str::to_string)
            .or_else(|| {
                el.select(mdurl_sel())
                    .next()
                    .and_then(|n| n.value().attr("data-mdurl"))
                    .map(str::to_string)
            })
            .or_else(|| {
                el.select(anchor_sel())
                    .filter_map(|a| a.value().attr("href"))
                    .find(|h| h.starts_with("http"))
                    .map(str::to_string)
            })
            .unwrap_or_default();

        if title.is_empty() || url.is_empty() {
            continue;
        }
        let snippet = el
            .select(snippet_sel())
            .next()
            .map(|n| clean_text(&n.text().collect::<String>()))
            .unwrap_or_default();

        out.push(RawItem {
            title,
            url,
            snippet,
            rank: (out.len() + 1) as u16,
            source: "so360",
        });
    }
    out
}

fn map_reqwest(e: reqwest::Error) -> EngineError {
    if e.is_timeout() {
        EngineError::Timeout
    } else {
        EngineError::Network(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_measured_fixture_shape() {
        // Shape taken from the live page (2026-10-24): data-mdurl carries the target.
        let html = r#"
        <ul>
          <li class="res-list" data-mdurl="https://blog.csdn.net/x/article/details/1">
            <h3><a href="https://www.so.com/link?m=abc">大模型推理优化实践</a></h3>
            <p class="res-desc">从显存到吞吐，逐层拆解推理优化。</p>
          </li>
          <li class="res-list"><h3><a href="https://example.com/no-mdurl">无 mdurl 条目</a></h3></li>
        </ul>"#;
        let items = parse(html, 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://blog.csdn.net/x/article/details/1");
        assert_eq!(items[0].rank, 1);
        assert!(items[0].snippet.contains("显存"));
        assert_eq!(items[1].url, "https://example.com/no-mdurl");
    }
}
