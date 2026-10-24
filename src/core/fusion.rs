use std::collections::HashMap;

use crate::core::normalize;
use crate::engines::RawItem;

/// A cross-source candidate: same canonical URL collapsed into one entry.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: &'static str,
    pub best_rank: u16,
    pub sources: Vec<&'static str>,
    pub rrf: f32,
}

/// Reciprocal Rank Fusion: `score += w_src / (k + rank)`.
///
/// Rank-based (not score-based) on purpose: upstream scores are incomparable, and
/// a new engine needs no calibration to participate.
pub fn fuse(lists: Vec<(f32, Vec<RawItem>)>, k: f32) -> Vec<Candidate> {
    let mut merged: HashMap<String, Candidate> = HashMap::new();
    for (weight, items) in lists {
        for item in items {
            let key = normalize::canonical(&item.url).unwrap_or_else(|| item.url.clone());
            let contribution = weight / (k + item.rank as f32);
            match merged.get_mut(&key) {
                Some(existing) => {
                    existing.rrf += contribution;
                    if !existing.sources.contains(&item.source) {
                        existing.sources.push(item.source);
                    }
                    if item.rank < existing.best_rank {
                        existing.best_rank = item.rank;
                    }
                    if item.snippet.chars().count() > existing.snippet.chars().count() {
                        existing.snippet = item.snippet.clone();
                    }
                }
                None => {
                    merged.insert(
                        key,
                        Candidate {
                            title: item.title.clone(),
                            url: item.url.clone(),
                            snippet: item.snippet.clone(),
                            source: item.source,
                            best_rank: item.rank,
                            sources: vec![item.source],
                            rrf: contribution,
                        },
                    );
                }
            }
        }
    }
    let mut out: Vec<Candidate> = merged.into_values().collect();
    out.sort_by(|a, b| {
        b.rrf
            .partial_cmp(&a.rrf)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.best_rank.cmp(&b.best_rank))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str, rank: u16, source: &'static str) -> RawItem {
        RawItem {
            title: format!("t{rank}"),
            url: url.to_string(),
            snippet: String::new(),
            rank,
            source,
        }
    }

    #[test]
    fn merges_same_url_across_engines() {
        let a = vec![item("https://x.com/p?utm_source=a", 1, "so360"), item("https://x.com/q", 2, "so360")];
        let b = vec![item("https://x.com/p", 2, "bing")];
        let fused = fuse(vec![(1.0, a), (0.8, b)], 60.0);
        assert_eq!(fused.len(), 2);
        let shared = fused.iter().find(|c| c.url.contains("/p")).unwrap();
        assert_eq!(shared.sources.len(), 2);
        assert!((shared.rrf - (1.0 / 61.0 + 0.8 / 62.0)).abs() < 1e-6);
    }
}
