use crate::core::fusion::Candidate;
use crate::core::normalize;
use crate::core::relevance::Terms;
use crate::model::FeatureHit;

/// Feature weights. Mirrors `config/scoring.json` (file-driven form lands in M1).
#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub rrf: f32,
    /// Concept coverage over title+snippet.
    pub coverage: f32,
    /// Concept coverage over the title alone.
    pub title: f32,
    /// Whole query appears contiguously.
    pub phrase: f32,
    pub authority: f32,
    pub length: f32,
}

impl Default for Weights {
    fn default() -> Self {
        Self { rrf: 1.0, coverage: 0.8, title: 0.5, phrase: 0.4, authority: 0.3, length: -0.1 }
    }
}

#[derive(Debug, Clone)]
pub struct Scored {
    pub candidate: Candidate,
    pub score: f32,
    pub explain: Vec<FeatureHit>,
}

const AUTHORITY: &[&str] = &[
    "gov.cn",
    "edu.cn",
    "github.com",
    "stackoverflow.com",
    "docs.rs",
    "arxiv.org",
    "npmjs.com",
    "pypi.org",
    "microsoft.com",
    "openai.com",
    "anthropic.com",
    "apple.com",
];

fn authority(url: &str) -> f32 {
    let domain = normalize::domain_of(url);
    if domain.is_empty() {
        return 0.0;
    }
    if AUTHORITY.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}"))) {
        1.0
    } else {
        0.0
    }
}

/// Snippets around 80-200 chars carry the most signal.
fn length_score(snippet: &str) -> f32 {
    let len = snippet.chars().count() as f32;
    let distance = if len < 80.0 { 80.0 - len } else if len > 200.0 { len - 200.0 } else { 0.0 };
    (1.0 - distance / 300.0).clamp(0.0, 1.0)
}

pub fn score_all(terms: &Terms, candidates: Vec<Candidate>, w: &Weights) -> Vec<Scored> {
    let max_rrf = candidates.iter().map(|c| c.rrf).fold(0.0_f32, f32::max).max(1e-6);
    let denom: f32 = (w.rrf.abs()
        + w.coverage.abs()
        + w.title.abs()
        + w.phrase.abs()
        + w.authority.abs()
        + w.length.abs())
    .max(1e-6);

    let mut out: Vec<Scored> = candidates
        .into_iter()
        .map(|c| {
            let text = format!("{} {}", c.title, c.snippet);
            let features = [
                ("rrf", (c.rrf / max_rrf).clamp(0.0, 1.0), w.rrf),
                ("coverage", terms.coverage(&text), w.coverage),
                ("title_coverage", terms.coverage(&c.title), w.title),
                ("phrase", if terms.phrase_hit(&text) { 1.0 } else { 0.0 }, w.phrase),
                ("authority", authority(&c.url), w.authority),
                ("length", length_score(&c.snippet), w.length),
            ];
            let mut score = 0.0_f32;
            let mut explain = Vec::with_capacity(features.len());
            for (feature, value, weight) in features {
                let contribution = value * weight / denom;
                score += contribution;
                explain.push(FeatureHit {
                    feature: feature.to_string(),
                    value,
                    weight,
                    contribution,
                });
            }
            Scored { candidate: c, score, explain }
        })
        .collect();

    // Deterministic: score desc, then best rank asc, then url asc.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.candidate.best_rank.cmp(&b.candidate.best_rank))
            .then_with(|| a.candidate.url.cmp(&b.candidate.url))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, snippet: &str, url: &str, rrf: f32) -> Candidate {
        Candidate {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
            source: "so360",
            best_rank: 1,
            sources: vec!["so360"],
            rrf,
        }
    }

    #[test]
    fn on_topic_authoritative_wins() {
        let terms = Terms::build("rust async runtime");
        let items = vec![
            cand("无关页面", "内容与查询无关", "https://spam.com/x", 1.0),
            cand(
                "Rust async runtime internals",
                "A deep dive into the async runtime",
                "https://docs.rs/tokio",
                0.9,
            ),
        ];
        let ranked = score_all(&terms, items, &Weights::default());
        assert!(ranked[0].candidate.url.contains("docs.rs"));
        assert!(ranked[0].explain.iter().any(|e| e.feature == "authority" && e.value == 1.0));
    }

    #[test]
    fn phrase_hit_raises_score() {
        let terms = Terms::build("大模型 推理");
        let items = vec![
            cand("大模型推理优化实践", "", "https://a.com/1", 1.0),
            cand("大模型 与 推理 的若干话题", "", "https://b.com/2", 1.0),
        ];
        let ranked = score_all(&terms, items, &Weights::default());
        assert!(ranked[0].candidate.url.contains("a.com"));
    }

    #[test]
    fn scores_are_normalized() {
        let terms = Terms::build("a b");
        let ranked = score_all(&terms, vec![cand("a b", "a b", "https://x.com/1", 1.0)], &Weights::default());
        assert!(ranked[0].score <= 1.0 && ranked[0].score > 0.0);
    }
}
