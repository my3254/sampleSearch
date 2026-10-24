use std::time::Duration;

/// Runtime configuration. Everything is env-overridable so the binary stays
/// deployable without a config file in M0 (file-based routing/scoring lands in M1).
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    /// Hard cutoff for one engine call (per tier).
    pub tier_a_timeout: Duration,
    pub tier_b_timeout: Duration,
    /// Upper bound on upstream requests per search (architecture: <= 4).
    pub max_upstream_requests: usize,
    pub default_max_results: usize,
    pub max_results_limit: usize,
    /// Overseas engines (ddg/brave/wikipedia). Defaults to false: no request is
    /// even constructed while disabled.
    pub intl_enabled: bool,
    pub proxy: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8787".into(),
            tier_a_timeout: Duration::from_millis(1500),
            tier_b_timeout: Duration::from_millis(2000),
            max_upstream_requests: 4,
            default_max_results: 10,
            max_results_limit: 10,
            intl_enabled: false,
            proxy: None,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let d = Self::default();
        let env_usize = |k: &str, v: usize| {
            std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(v)
        };
        Self {
            bind: std::env::var("MYSEARCH_BIND").unwrap_or(d.bind),
            tier_a_timeout: Duration::from_millis(
                env_usize("MYSEARCH_TIER_A_TIMEOUT_MS", d.tier_a_timeout.as_millis() as usize) as u64,
            ),
            tier_b_timeout: Duration::from_millis(
                env_usize("MYSEARCH_TIER_B_TIMEOUT_MS", d.tier_b_timeout.as_millis() as usize) as u64,
            ),
            max_upstream_requests: env_usize("MYSEARCH_MAX_UPSTREAM", d.max_upstream_requests),
            default_max_results: env_usize("MYSEARCH_DEFAULT_MAX", d.default_max_results)
                .min(d.max_results_limit),
            max_results_limit: d.max_results_limit,
            intl_enabled: std::env::var("MYSEARCH_INTL").map(|v| v == "1" || v == "true").unwrap_or(false),
            proxy: std::env::var("MYSEARCH_PROXY").ok().filter(|s| !s.is_empty()),
        }
    }
}
