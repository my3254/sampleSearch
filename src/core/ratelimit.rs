//! Per-engine token buckets.
//!
//! Sized from measured quotas, not guesses: GitHub's anonymous search allows 10
//! requests/minute per IP (shared with every other GitHub tool on that machine),
//! Stack Exchange allows 300/day, arXiv asks for one request per three seconds.
//! Exceeding a bucket skips the engine for that query instead of getting the whole
//! IP throttled.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

struct Bucket {
    tokens: f32,
    capacity: f32,
    refill_per_sec: f32,
    last: Instant,
}

#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<&'static str, Bucket>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take one token for `id`, refilling first. Returns false when the engine is over
    /// budget and should be skipped.
    pub fn try_acquire(&self, id: &'static str, capacity: f32, refill_secs: u64) -> bool {
        let capacity = capacity.max(1.0);
        let refill_per_sec = capacity / refill_secs.max(1) as f32;
        let now = Instant::now();
        let Ok(mut buckets) = self.buckets.lock() else {
            return true;
        };
        let bucket = buckets.entry(id).or_insert(Bucket {
            tokens: capacity,
            capacity,
            refill_per_sec,
            last: now,
        });
        // Providers can change their quota between runs; keep the newest sizing.
        bucket.capacity = capacity;
        bucket.refill_per_sec = refill_per_sec;
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f32();
        bucket.tokens = (bucket.tokens + elapsed * bucket.refill_per_sec).min(bucket.capacity);
        bucket.last = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Tokens left, for `/health` style reporting.
    pub fn available(&self, id: &'static str) -> Option<f32> {
        self.buckets.lock().ok()?.get(id).map(|b| b.tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spends_tokens_then_refuses() {
        let limiter = RateLimiter::new();
        assert!(limiter.try_acquire("gh", 2.0, 60));
        assert!(limiter.try_acquire("gh", 2.0, 60));
        assert!(!limiter.try_acquire("gh", 2.0, 60));
    }

    #[test]
    fn engines_have_independent_buckets() {
        let limiter = RateLimiter::new();
        assert!(limiter.try_acquire("a", 1.0, 60));
        assert!(!limiter.try_acquire("a", 1.0, 60));
        assert!(limiter.try_acquire("b", 1.0, 60));
    }
}
