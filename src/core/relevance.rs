//! Query → concept groups, and "does this result actually answer it".
//!
//! The earlier version counted CJK bigrams, which lets a result through when it
//! matches one common character pair. Precision needs *concept* matching: the
//! query is split into whitespace-delimited groups and a result must match most
//! of them, not one fragment of one of them.

use std::collections::HashSet;

use crate::core::gates::is_cjk;

/// Words that carry no retrieval signal; dropping them stops "用法" from being
/// counted as a matched concept.
const STOPWORDS: &[&str] = &[
    "用法", "教程", "怎么", "如何", "是什么", "推荐", "相关", "一下", "请问", "帮我", "告诉",
    "查询", "搜索", "介绍", "详解",
];

/// Modifiers that prefix a real concept ("最新政策" → "政策", "官方文档" → "文档").
/// Stripping them is what lets a Chinese query match titles that omit the modifier.
const MODIFIER_PREFIXES: &[&str] = &["最新", "最近", "常用", "热门", "免费", "官方", "相关", "全部"];

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub text: String,
    pub ascii: bool,
}

#[derive(Debug, Clone)]
pub struct Terms {
    pub groups: Vec<Group>,
    /// Majority-CJK query: results in the wrong script are usually junk.
    pub cjk: bool,
    /// Original query with single spaces, lowercased.
    pub phrase_spaced: String,
    /// Same, whitespace removed (catches CJK titles written without spaces).
    pub phrase_compact: String,
}

impl Terms {
    pub fn build(query: &str) -> Self {
        let lowered = query.trim().to_lowercase();
        let raw_groups: Vec<String> = lowered
            .split(|c: char| {
                c.is_whitespace()
                    || matches!(c, '，' | '。' | ',' | ';' | '；' | '、' | '|' | '/' | '？' | '?')
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut seen = HashSet::new();
        let mut groups: Vec<Group> = Vec::new();
        for text in &raw_groups {
            if STOPWORDS.contains(&text.as_str()) {
                continue;
            }
            let text = strip_modifier(text);
            if text.is_empty() || STOPWORDS.contains(&text.as_str()) || !seen.insert(text.clone()) {
                continue;
            }
            groups.push(Group { text: text.clone(), ascii: text.chars().all(|c| !is_cjk(c)) });
        }
        // Everything was a stopword: fall back to the raw query so the gate still works.
        if groups.is_empty() {
            let text = lowered.clone();
            groups.push(Group { text: text.clone(), ascii: text.chars().all(|c| !is_cjk(c)) });
        }

        let phrase_spaced = groups.iter().map(|g| g.text.as_str()).collect::<Vec<_>>().join(" ");
        let phrase_compact: String = phrase_spaced.chars().filter(|c| !c.is_whitespace()).collect();
        let cjk_chars = query.chars().filter(|c| is_cjk(*c)).count();
        let meaningful = query.chars().filter(|c| !c.is_whitespace()).count().max(1);
        let cjk = (cjk_chars as f32 / meaningful as f32) >= 0.2;

        Self { groups, cjk, phrase_spaced, phrase_compact }
    }

    /// How many groups must match: 1→1, 2→2, 3→2, 4→2, 5→3, …
    pub fn required(&self) -> usize {
        match self.groups.len() {
            0 => 0,
            1 => 1,
            2 => 2,
            n => (n + 1) / 2,
        }
    }

    pub fn matched(&self, text: &str) -> usize {
        let lower = text.to_lowercase();
        self.groups.iter().filter(|g| group_matches(g, &lower)).count()
    }

    pub fn coverage(&self, text: &str) -> f32 {
        if self.groups.is_empty() {
            return 0.0;
        }
        self.matched(text) as f32 / self.groups.len() as f32
    }

    /// Item-level relevance gate.
    pub fn passes(&self, text: &str) -> bool {
        if self.groups.is_empty() {
            return true;
        }
        self.matched(text) >= self.required()
    }

    /// Whole query appears contiguously (spaces optional). Strong precision signal.
    pub fn phrase_hit(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        if self.phrase_spaced.contains(' ') && lower.contains(&self.phrase_spaced) {
            return true;
        }
        self.phrase_compact.chars().count() >= 4 && lower.contains(&self.phrase_compact)
    }

    /// CJK query against a pure-ASCII result: almost always an unrelated page.
    pub fn script_mismatch(&self, text: &str) -> bool {
        self.cjk && !text.chars().any(is_cjk) && text.chars().any(|c| c.is_ascii_alphabetic())
    }
}

fn strip_modifier(text: &str) -> String {
    for prefix in MODIFIER_PREFIXES {
        if let Some(rest) = text.strip_prefix(prefix) {
            if rest.chars().count() >= 2 {
                return rest.to_string();
            }
        }
    }
    text.to_string()
}

fn group_matches(group: &Group, lower_text: &str) -> bool {
    if group.ascii {
        // Exact token match first (so "ai" does not match "said"), then substring
        // for longer groups ("vllm" inside "vllm-project").
        let tokens = lower_text
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .filter(|t| !t.is_empty());
        if tokens.clone().any(|t| t == group.text) {
            return true;
        }
        group.text.chars().count() >= 4 && lower_text.contains(&group.text)
    } else if lower_text.contains(&group.text) {
        true
    } else {
        // Chinese titles often omit characters inside a compound ("推理优化" vs
        // "推理 与 优化"), so accept a strong bigram overlap rather than requiring
        // the exact contiguous string.
        cjk_bigram_overlap(&group.text, lower_text) >= 0.6
    }
}

fn cjk_bigram_overlap(group: &str, text: &str) -> f32 {
    let chars: Vec<char> = group.chars().collect();
    if chars.len() < 3 {
        return if text.contains(group) { 1.0 } else { 0.0 };
    }
    let total = chars.len() - 1;
    let hits = chars.windows(2).filter(|w| text.contains(&w.iter().collect::<String>())).count();
    hits as f32 / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_groups_and_drops_stopwords() {
        let t = Terms::build("tokio select 用法");
        let texts: Vec<&str> = t.groups.iter().map(|g| g.text.as_str()).collect();
        assert_eq!(texts, vec!["tokio", "select"]);
        assert_eq!(t.required(), 2);
    }

    #[test]
    fn requires_most_concepts_not_one_bigram() {
        let t = Terms::build("大模型 推理 优化");
        assert_eq!(t.required(), 2);
        assert!(t.passes("大模型推理优化实践"));
        assert!(!t.passes("大（汉语文字）_百度百科"));
    }

    #[test]
    fn two_concept_query_needs_both() {
        let t = Terms::build("中国政府网 最新政策");
        assert!(!t.passes("中国政府网_中央人民政府门户网站"));
        assert!(t.passes("政策_中国政府网"));
    }

    #[test]
    fn ascii_tokens_do_not_match_inside_words() {
        let t = Terms::build("ai");
        assert!(!t.passes("said the model"));
        assert!(t.passes("AI 大模型"));
    }

    #[test]
    fn phrase_and_script_helpers() {
        let t = Terms::build("大模型 推理");
        assert!(t.phrase_hit("大模型推理优化"));
        assert!(t.script_mismatch("Large language model inference"));
        assert!(!t.script_mismatch("大模型推理优化"));
    }
}
