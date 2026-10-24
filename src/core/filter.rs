//! Precision filter chain. Every drop records a reason so `/v1/search` can report
//! what was thrown away and why — filtering you cannot measure is filtering you
//! cannot trust.

use std::collections::{BTreeMap, HashSet};

use crate::core::fusion::Candidate;
use crate::core::normalize;
use crate::core::relevance::Terms;

/// Hosts that produce "answer noise" rather than documents: machine translation,
/// document-preview aggregators, 360's own redirect wrappers.
const NOISE_HOSTS: &[&str] = &[
    "fanyi.so.com",
    "wenku.so.com",
    "www.so.com",
    "so.com",
    "translate.google.com",
    "zhidao.baidu.com",
];

/// Titles that are aggregation/portal chrome, not documents.
const AGGREGATOR_MARKERS: &[&str] = &[
    "的最新相关消息",
    "相关搜索",
    "相关推荐",
    "搜索结果",
    "- 360搜索",
    "文库",
    "下载大全",
    "专题聚合",
    "全部结果",
];

#[derive(Debug, Default)]
pub struct FilterReport {
    pub kept: Vec<Candidate>,
    pub dropped: BTreeMap<&'static str, usize>,
}

pub fn apply(terms: &Terms, items: Vec<Candidate>) -> FilterReport {
    let mut dropped: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut kept: Vec<Candidate> = Vec::new();
    let mut seen_snippets: HashSet<String> = HashSet::new();

    for candidate in items {
        let text = format!("{} {}", candidate.title, candidate.snippet);
        let reason: Option<&'static str> = if is_noise(&candidate.url) {
            Some("noise_host")
        } else if AGGREGATOR_MARKERS.iter().any(|m| candidate.title.contains(m)) {
            Some("aggregator_title")
        } else if is_portal_root(&candidate.url) && terms.coverage(&text) < 1.0 {
            // A site homepage can only answer a query that the homepage itself
            // covers entirely ("nvidia" yes, "nvidia 财报 2026" no).
            Some("portal_root")
        } else if !terms.passes(&text) {
            Some("low_match")
        } else if terms.matched(&candidate.title) == 0 {
            // Concept only in the snippet, none in the title: a generic container
            // page ("财务报告" for a "茅台 年报" query) or an unrelated mention.
            Some("title_mismatch")
        } else if is_entity_page(terms, &candidate.title) {
            // Brand-only title ("TOKIO - 知乎") for a multi-concept query.
            Some("entity_page")
        } else if terms.script_mismatch(&text) {
            Some("script_mismatch")
        } else if !seen_snippets.insert(snippet_key(&candidate.snippet)) {
            Some("dup_content")
        } else {
            None
        };

        match reason {
            Some(r) => *dropped.entry(r).or_insert(0) += 1,
            None => kept.push(candidate),
        }
    }

    FilterReport { kept, dropped }
}

pub fn is_noise(url: &str) -> bool {
    let host = normalize::domain_of(url);
    if host.is_empty() {
        return true;
    }
    NOISE_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// Root path with no meaningful query string.
pub fn is_portal_root(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else { return false };
    let path = parsed.path();
    (path.is_empty() || path == "/") && parsed.query().is_none()
}

fn snippet_key(snippet: &str) -> String {
    snippet
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .take(40)
        .collect()
}

/// Multi-concept query against a title that is just one concept plus a site suffix
/// ("TOKIO - 知乎", "NVIDIA_官网"): an entity page, not an answer.
fn is_entity_page(terms: &Terms, title: &str) -> bool {
    if terms.groups.len() < 2 || terms.matched(title) >= terms.required() {
        return false;
    }
    let lowered = title.trim().to_lowercase();
    let core = lowered
        .rsplit_once(" - ")
        .map(|(head, _)| head)
        .or_else(|| lowered.rsplit_once('|').map(|(head, _)| head))
        .or_else(|| lowered.rsplit_once('_').map(|(head, _)| head))
        .unwrap_or(&lowered)
        .trim();
    terms.groups.iter().any(|g| core == g.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, snippet: &str, url: &str) -> Candidate {
        Candidate {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
            source: "so360",
            best_rank: 1,
            sources: vec!["so360"],
            rrf: 1.0,
        }
    }

    #[test]
    fn flags_translation_onebox() {
        assert!(is_noise("https://fanyi.so.com/?src=onebox#tokio"));
        assert!(!is_noise("https://docs.rs/tokio/latest/tokio/macro.select.html"));
    }

    #[test]
    fn filters_aggregator_and_portal() {
        let terms = Terms::build("中国政府网 最新政策");
        let items = vec![
            cand("中国政府网_中央人民政府门户网站", "", "https://www.gov.cn/"),
            cand("大模型 推理 优化的最新相关消息", "聚合页", "https://news.example.com/x"),
            cand("政策_中国政府网", "最新政策文件与解读", "https://www.gov.cn/zhengce/"),
        ];
        let report = apply(&terms, items);
        assert_eq!(report.kept.len(), 1);
        let reasons: Vec<(&str, usize)> = report.dropped.iter().map(|(k, v)| (*k, *v)).collect();
        assert!(reasons.contains(&("portal_root", 1)));
        assert!(reasons.contains(&("aggregator_title", 1)));
    }

    #[test]
    fn drops_unrelated_entity_collapse() {
        let terms = Terms::build("nvidia 财报 2026");
        let items = vec![
            cand("人工智能计算领域的领导者 | NVIDIA", "", "https://www.nvidia.cn/"),
            cand("NVIDIA公布2026年财报:营收达215亿美元", "", "https://example.com/a"),
        ];
        let report = apply(&terms, items);
        assert_eq!(report.kept.len(), 1);
        assert_eq!(report.kept[0].title.contains("财报"), true);
    }

    #[test]
    fn dedupes_identical_snippets() {
        let terms = Terms::build("台风 上海");
        let items = vec![
            cand("台风路径 上海", "上海台风最新消息", "https://a.com/1"),
            cand("台风路径 上海 实时", "上海台风最新消息", "https://b.com/2"),
        ];
        let report = apply(&terms, items);
        assert_eq!(report.kept.len(), 1);
    }

    #[test]
    fn drops_generic_container_title() {
        // Snippet mentions 茅台, title is a generic "财务报告" container.
        let terms = Terms::build("贵州茅台 2025 年报");
        let items = vec![
            cand("财务报告", "贵州茅台2025年年度报告全文", "https://example.com/report"),
            cand("贵州茅台2025年年报出炉", "年报要点", "https://example.com/news"),
        ];
        let report = apply(&terms, items);
        assert_eq!(report.kept.len(), 1);
        assert_eq!(report.kept[0].title, "贵州茅台2025年年报出炉");
        assert_eq!(report.dropped.get("title_mismatch"), Some(&1));
    }

    #[test]
    fn drops_brand_only_entity_page() {
        let terms = Terms::build("tokio select 用法");
        let items = vec![
            cand("TOKIO - 知乎", "tokio select usage 相关讨论", "https://zhihu.com/q/1"),
            cand("Tokio教程之select_tokio::select!-CSDN博客", "", "https://blog.csdn.net/x"),
        ];
        let report = apply(&terms, items);
        assert_eq!(report.kept.len(), 1);
        assert_eq!(report.dropped.get("entity_page"), Some(&1));
    }
}
