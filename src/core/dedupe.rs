use std::collections::HashSet;

use crate::core::fusion::Candidate;

/// Drop near-duplicates. URL-level dedupe already happened in `fusion::fuse`
/// (canonical URL key); this catches titles that differ only in punctuation,
/// whitespace or case — a common pattern when two engines echo the same page.
pub fn dedupe(items: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen: HashSet<String> = HashSet::new();
    items
        .into_iter()
        .filter(|c| {
            let key = title_key(&c.title);
            key.is_empty() || seen.insert(key)
        })
        .collect()
}

fn title_key(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, url: &str) -> Candidate {
        Candidate {
            title: title.into(),
            url: url.into(),
            snippet: String::new(),
            source: "so360",
            best_rank: 1,
            sources: vec!["so360"],
            rrf: 1.0,
        }
    }

    #[test]
    fn drops_title_variants() {
        let items = vec![
            cand("大模型推理优化：从显存到吞吐", "https://a.com/1"),
            cand("大模型推理优化 从显存到吞吐", "https://b.com/2"),
            cand("完全不同的标题", "https://c.com/3"),
        ];
        assert_eq!(dedupe(items).len(), 2);
    }

    #[test]
    fn keeps_distinct_titles() {
        let items = vec![cand("甲", "https://a.com/1"), cand("乙", "https://b.com/2")];
        assert_eq!(dedupe(items).len(), 2);
    }
}
