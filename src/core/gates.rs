//! Query-level language detection.
//!
//! Term-level matching lives in `core::relevance`; this module only answers
//! "is this a Chinese query", which drives engine preference.

pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x3040..=0x30FF)
}

/// CJK share of the meaningful characters; >= 20% counts as a Chinese query.
pub fn is_cjk_query(query: &str) -> bool {
    let mut cjk = 0usize;
    let mut total = 0usize;
    for c in query.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() {
            continue;
        }
        total += 1;
        if is_cjk(c) {
            cjk += 1;
        }
    }
    total > 0 && (cjk as f32 / total as f32) >= 0.2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_chinese_queries() {
        assert!(is_cjk_query("大模型 推理 优化"));
        assert!(!is_cjk_query("tokio select usage"));
        assert!(is_cjk_query("vLLM 推理 加速"));
        assert!(is_cjk_query("600519 财报"));
    }
}
