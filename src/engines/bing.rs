//! Bing adapter: HTML first (has snippets), RSS as a parsing fallback.
//!
//! Measured (2026-10-24): HTML and RSS return the same result set; Bing tends to
//! collapse multi-term queries to an entity page (3/8 queries in the probe set),
//! which is why this engine is secondary and why the relevance gate exists.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use scraper::{Html, Selector};

use super::{clean_text, looks_blocked, Engine, EngineError, EngineOutput, RawItem};

const ENDPOINT: &str = "https://cn.bing.com/search";

pub struct Bing {
    client: reqwest::Client,
}

impl Bing {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn fetch(&self, query: &str, rss: bool, timeout: Duration) -> Result<(u16, String), EngineError> {
        let mut url = url::Url::parse(ENDPOINT).expect("static endpoint");
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("q", query);
            if rss {
                qp.append_pair("format", "rss");
            }
        }
        let resp = self
            .client
            .get(url)
            .header("accept-language", "zh-CN,zh;q=0.9,en;q=0.8")
            .timeout(timeout)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(map_reqwest)?;
        Ok((status, body))
    }
}

#[async_trait]
impl Engine for Bing {
    fn id(&self) -> &'static str {
        "bing"
    }

    fn weight(&self) -> f32 {
        0.8
    }

    fn domains(&self) -> &'static [&'static str] {
        &["general", "news", "finance", "legal", "academic"]
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
        timeout: Duration,
    ) -> Result<EngineOutput, EngineError> {
        let started = Instant::now();
        let (status, body) = self.fetch(query, false, timeout).await?;
        if status != 200 {
            return Err(EngineError::Upstream(status));
        }
        if let Some(reason) = looks_blocked(&body) {
            return Err(EngineError::Blocked(reason.to_string()));
        }

        let mut items = parse_html(&body, max_results);
        // Parsing fallback: the RSS channel survives markup changes to the SERP.
        if items.is_empty() {
            let (rss_status, rss_body) = self.fetch(query, true, timeout).await?;
            if rss_status == 200 {
                items = parse_rss(&rss_body, max_results);
            }
        }
        if items.is_empty() {
            return Err(EngineError::Parse("no results in html or rss".into()));
        }

        let parsed = items.len();
        Ok(EngineOutput {
            items,
            http_status: Some(status),
            parsed,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }
}

fn item_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("li.b_algo").expect("valid selector"))
}

fn link_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| Selector::parse("h2 a[href]").expect("valid selector"))
}

fn snippet_sel() -> &'static Selector {
    static S: OnceLock<Selector> = OnceLock::new();
    S.get_or_init(|| {
        Selector::parse("p[class*=\"b_lineclamp\"], .b_caption p, p").expect("valid selector")
    })
}

pub fn parse_html(body: &str, max_results: usize) -> Vec<RawItem> {
    let doc = Html::parse_document(body);
    let mut out = Vec::new();
    for el in doc.select(item_sel()) {
        if out.len() >= max_results {
            break;
        }
        let link = el.select(link_sel()).next();
        let title = link
            .as_ref()
            .map(|n| clean_text(&n.text().collect::<String>()))
            .unwrap_or_default();
        let url = link
            .and_then(|n| n.value().attr("href"))
            .map(str::to_string)
            .unwrap_or_default();
        if title.is_empty() || !url.starts_with("http") {
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
            source: "bing",
        });
    }
    out
}

/// Minimal RSS reader: `<item><title/><link/><description/></item>`.
/// Deliberately dependency-free — this is a fallback path, not a general XML parser.
pub fn parse_rss(body: &str, max_results: usize) -> Vec<RawItem> {
    let mut out = Vec::new();
    let mut rest = body;
    while out.len() < max_results {
        let Some(start) = rest.find("<item") else { break };
        let after = &rest[start..];
        let Some(end) = after.find("</item>") else { break };
        let block = &after[..end];
        rest = &after[end + "</item>".len()..];

        let title = clean_text(&tag_text(block, "title").unwrap_or_default());
        let url = clean_text(&tag_text(block, "link").unwrap_or_default());
        if title.is_empty() || !url.starts_with("http") {
            continue;
        }
        let snippet = clean_text(&tag_text(block, "description").unwrap_or_default());
        out.push(RawItem {
            title,
            url,
            snippet,
            rank: (out.len() + 1) as u16,
            source: "bing",
        });
    }
    out
}

fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let start = block.find(&open)?;
    let gt = block[start..].find('>')? + start + 1;
    let close = format!("</{}>", tag);
    let end = block[gt..].find(&close)? + gt;
    let raw = &block[gt..end];
    let raw = raw
        .trim_start_matches("<![CDATA[")
        .trim_end_matches("]]>")
        .trim();
    Some(unescape(raw))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn map_reqwest(e: reqwest::Error) -> EngineError {
    if e.is_timeout() {
        EngineError::Timeout(Duration::from_millis(0))
    } else {
        EngineError::Network(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"
      <ol>
        <li class="b_algo">
          <h2><a href="https://www.microsoft.com/zh-cn/investor/earnings">微软 2026 财年第三季度财报</a></h2>
          <div class="b_caption"><p class="b_lineclamp2">营收 829 亿美元，同比增长 18%。</p></div>
        </li>
        <li class="b_algo"><h2><a href="javascript:void(0)">不是结果</a></h2></li>
      </ol>"#;

    const RSS: &str = r#"<?xml version="1.0"?><rss><channel>
        <item><title>微软财报解读</title><link>https://example.com/a</link>
              <description><![CDATA[营收与利润均超预期]]></description></item>
        <item><title>无关</title><link>not-a-url</link></item>
      </channel></rss>"#;

    #[test]
    fn parses_serp_html() {
        let items = parse_html(HTML, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://www.microsoft.com/zh-cn/investor/earnings");
        assert!(items[0].snippet.contains("829"));
        assert_eq!(items[0].source, "bing");
    }

    #[test]
    fn parses_rss_fallback() {
        let items = parse_rss(RSS, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "微软财报解读");
        assert_eq!(items[0].snippet, "营收与利润均超预期");
    }

    #[test]
    fn unescapes_entities() {
        assert_eq!(unescape("a &amp; b &lt;c&gt;"), "a & b <c>");
    }
}
