use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::task::JoinSet;

use crate::config::Config;
use crate::core::ratelimit::RateLimiter;
use crate::core::{dedupe, filter, fusion, gates, normalize, relevance, score};
use crate::engines::{Engine, EngineError, EngineOutput, RawItem};
use crate::model::{
    FilterStat, Metadata, ResultItem, RoutingInfo, SearchData, SearchRequest, SourceStat,
};

/// Per-host cap so one site cannot fill the whole page.
const MAX_PER_DOMAIN: usize = 3;
/// Identical queries are served from memory: repeat traffic must not cost upstream quota.
const CACHE_TTL: Duration = Duration::from_secs(300);
const CACHE_MAX: usize = 256;
/// After a captcha/block response, stop calling that engine for a while.
const COOLDOWN_AFTER_BLOCK: Duration = Duration::from_secs(600);
/// When only one engine is healthy, it gets the whole budget — a slow but working
/// engine is better than a fast timeout that returns nothing.
const LONE_ENGINE_BUDGET: Duration = Duration::from_secs(12);

pub struct Orchestrator {
    config: Config,
    engines: Vec<Arc<dyn Engine>>,
    cache: Mutex<HashMap<String, (Instant, SearchData)>>,
    /// engine id → instant until which it is skipped (captcha / rate-limit backoff).
    cooldowns: Mutex<HashMap<&'static str, Instant>>,
    limiter: RateLimiter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    General,
    Code,
    Academic,
}

/// Cheap intent routing. Deliberately rule-based: it runs on every request and must
/// never add latency or depend on a model.
fn detect_intent(query: &str, domain: Option<&str>) -> Intent {
    match domain {
        Some("code") => return Intent::Code,
        Some("academic") => return Intent::Academic,
        _ => {}
    }
    let q = query.to_lowercase();
    const ACADEMIC: &[&str] = &[
        "arxiv", "doi", "paper", "论文", "文献", "综述", "citation", "journal", "conference", "et al",
    ];
    const CODE: &[&str] = &[
        "error", "exception", "traceback", "panic", "stack trace", "pip install", "npm install",
        "npm i ", "cargo ", "import ", "fn ", "undefined", "#issue", ".py", ".ts", ".rs", ".go",
        "select", "async", "await", "runtime", "compile", "debug", "docker", "sql", "http",
        "json", "regex", "python", "rust", "kotlin", "typescript", "deploy",
        "middleware", "endpoint", "webhook", "graphql", "grpc", "kafka", "redis", "postgres",
        "mysql", "lambda", "terraform", "kubernetes", "webpack", "vite", "pytest", "numpy",
        "pandas", "django", "flask", "fastapi", "express", "nextjs", "react", "vue", "svelte",
    ];
    if ACADEMIC.iter().any(|m| q.contains(m)) || has_arxiv_id(&q) {
        Intent::Academic
    } else if CODE.iter().any(|m| q.contains(m)) {
        Intent::Code
    } else if looks_like_package(&q) {
        // A bare package name ("requests", "python-dateutil") is a lookup, not a web query.
        Intent::Code
    } else {
        Intent::General
    }
}

fn looks_like_package(q: &str) -> bool {
    let trimmed = q.trim();
    trimmed.chars().count() >= 3
        && !trimmed.contains(char::is_whitespace)
        && trimmed.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `2401.12345`, `2401.1234v2` — an arXiv identifier in the query.
fn has_arxiv_id(q: &str) -> bool {
    q.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == 'v'))
        .any(|token| match token.split_once('.') {
            Some((left, right)) => {
                let right = right.trim_end_matches('v');
                left.len() == 4
                    && (4..=5).contains(&right.len())
                    && left.chars().all(|c| c.is_ascii_digit())
                    && right.chars().all(|c| c.is_ascii_digit())
            }
            None => false,
        })
}

pub struct Plan {
    pub intent: String,
    pub primary: Vec<Arc<dyn Engine>>,
    pub secondary: Vec<Arc<dyn Engine>>,
}

struct EngineRun {
    id: &'static str,
    weight: f32,
    outcome: Result<EngineOutput, EngineError>,
    elapsed_ms: u128,
}

impl Orchestrator {
    pub fn new(config: Config, engines: Vec<Arc<dyn Engine>>) -> Self {
        Self {
            config,
            engines,
            cache: Mutex::new(HashMap::new()),
            cooldowns: Mutex::new(HashMap::new()),
            limiter: RateLimiter::new(),
        }
    }

    fn cache_key(req: &SearchRequest, max_results: usize) -> String {
        format!(
            "{}|{}|{}|{}",
            req.query.trim().to_lowercase(),
            max_results,
            req.domain.as_deref().unwrap_or(""),
            req.language.as_deref().unwrap_or("")
        )
    }

    fn cache_get(&self, key: &str) -> Option<SearchData> {
        let mut cache = self.cache.lock().ok()?;
        match cache.get(key) {
            Some((stored_at, data)) if stored_at.elapsed() < CACHE_TTL => {
                let mut data = data.clone();
                data.metadata.cached = true;
                Some(data)
            }
            Some(_) => {
                cache.remove(key);
                None
            }
            None => None,
        }
    }

    fn cache_put(&self, key: String, data: &SearchData) {
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= CACHE_MAX {
                cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
                if cache.len() >= CACHE_MAX {
                    cache.clear();
                }
            }
            cache.insert(key, (Instant::now(), data.clone()));
        }
    }

    fn cooling(&self) -> HashSet<&'static str> {
        let now = Instant::now();
        self.cooldowns
            .lock()
            .map(|map| {
                map.iter()
                    .filter(|(_, until)| **until > now)
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn mark_blocked(&self, id: &'static str) {
        if let Ok(mut map) = self.cooldowns.lock() {
            map.insert(id, Instant::now() + COOLDOWN_AFTER_BLOCK);
        }
        tracing::warn!(engine = id, cooldown_s = COOLDOWN_AFTER_BLOCK.as_secs(), "engine blocked upstream, cooling down");
    }

    pub fn engine_ids(&self) -> Vec<&'static str> {
        self.engines.iter().map(|e| e.id()).collect()
    }

    /// Remaining local tokens per engine, for `/health`. `None` means the engine has
    /// not been called yet in this process.
    pub fn quota_snapshot(&self) -> Vec<(&'static str, Option<f32>)> {
        self.engines.iter().map(|e| (e.id(), self.limiter.available(e.id()))).collect()
    }

    /// Engine selection: route by intent, then fall back to general web engines.
    ///
    /// Code and academic queries go to the official-API engines first, because those
    /// have published quotas and never answer with a captcha. Everything else uses the
    /// general web engines, with the other language's engine as the fallback tier.
    pub fn plan(&self, req: &SearchRequest) -> Plan {
        let prefer_cn = gates::is_cjk_query(&req.query);
        let intent = match detect_intent(&req.query, req.domain.as_deref()) {
            Intent::Academic => "academic",
            Intent::Code => "code",
            Intent::General if prefer_cn => "general_cn",
            Intent::General => "general_en",
        };

        let (mut primary, secondary): (Vec<Arc<dyn Engine>>, Vec<Arc<dyn Engine>>) = match intent {
            "academic" => (
                self.pick(&["arxiv", "crossref"], req),
                self.pick_general(prefer_cn, req).into_iter().take(1).collect(),
            ),
            "code" if looks_like_package(&req.query) => (
                self.pick(&["pypi", "github"], req),
                self.pick(&["npm", "stackexchange"], req).into_iter().take(2).collect(),
            ),
            "code" => (
                self.pick(&["github", "stackexchange"], req),
                self.pick(&["hn", "npm", "pypi"], req).into_iter().take(2).collect(),
            ),
            _ => (
                self.pick_general(prefer_cn, req).into_iter().take(1).collect(),
                self.pick_general(!prefer_cn, req).into_iter().take(1).collect(),
            ),
        };

        // Intent matched nothing usable (quota, capability): use general web engines.
        if primary.is_empty() && secondary.is_empty() {
            primary = self.pick_general(prefer_cn, req).into_iter().take(1).collect();
        }

        Plan { intent: intent.to_string(), primary, secondary }
    }

    fn pick(&self, ids: &[&str], req: &SearchRequest) -> Vec<Arc<dyn Engine>> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(engine) = self.engines.iter().find(|e| e.id() == *id) {
                if !req.domain.as_deref().is_some_and(|d| d != "general" && !engine.domains().contains(&d)) {
                    if engine.supports(&req.query) {
                        out.push(engine.clone());
                    }
                }
            }
        }
        out
    }

    fn pick_general(&self, cn: bool, req: &SearchRequest) -> Vec<Arc<dyn Engine>> {
        let wanted = if cn { "so360" } else { "bing" };
        self.engines
            .iter()
            .filter(|e| e.id() == wanted && e.supports(&req.query))
            .cloned()
            .collect()
    }

    pub async fn search(&self, req: &SearchRequest) -> Result<SearchData, String> {
        let started = Instant::now();
        let max_results = req
            .max_results
            .unwrap_or(self.config.default_max_results)
            .clamp(1, self.config.max_results_limit);
        let terms = relevance::Terms::build(&req.query);
        let plan = self.plan(req);

        // Repeat queries are free: no upstream traffic, no quota spent.
        let cache_key = Self::cache_key(req, max_results);
        if let Some(hit) = self.cache_get(&cache_key) {
            tracing::debug!(query = %req.query, "cache hit");
            return Ok(hit);
        }

        let cooling = self.cooling();
        let mut skipped: Vec<&'static str> = Vec::new();
        let mut tasks: Vec<(Arc<dyn Engine>, Duration)> = Vec::new();
        for (engine, timeout) in plan
            .primary
            .iter()
            .map(|e| (e, self.config.tier_a_timeout))
            .chain(plan.secondary.iter().map(|e| (e, self.config.tier_b_timeout)))
        {
            if cooling.contains(&engine.id()) {
                skipped.push(engine.id());
                continue;
            }
            let (capacity, refill_secs) = engine.rate_limit();
            if !self.limiter.try_acquire(engine.id(), capacity, refill_secs) {
                tracing::debug!(engine = engine.id(), "local quota spent, skipping this query");
                skipped.push(engine.id());
                continue;
            }
            tasks.push((engine.clone(), timeout.max(engine.timeout_hint())));
        }
        // Every engine is cooling down: try anyway rather than fail without asking.
        if tasks.is_empty() {
            for engine in &plan.primary {
                tasks.push((engine.clone(), self.config.tier_a_timeout));
            }
        }
        tasks.truncate(self.config.max_upstream_requests.max(1));
        // One engine left: give it the whole budget instead of the tier timeout.
        if tasks.len() == 1 {
            tasks[0].1 = LONE_ENGINE_BUDGET;
        }

        let mut set: JoinSet<EngineRun> = JoinSet::new();
        for (engine, timeout) in tasks {
            let query = req.query.clone();
            set.spawn(async move {
                let id = engine.id();
                let weight = engine.weight();
                let begin = Instant::now();
                let outcome = engine.search(&query, max_results, timeout).await;
                EngineRun { id, weight, outcome, elapsed_ms: begin.elapsed().as_millis() }
            });
        }

        let mut runs: Vec<EngineRun> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(run) => runs.push(run),
                Err(err) => tracing::warn!(%err, "engine task failed to join"),
            }
        }
        for run in &runs {
            if let Err(EngineError::Blocked(_)) = &run.outcome {
                self.mark_blocked(run.id);
            }
        }
        if runs.is_empty() {
            return Err(format!(
                "no engine could be scheduled (cooling down: {})",
                skipped.join(", ")
            ));
        }

        // Tier C: every engine failed. Retry the one that was not hard-blocked with the
        // full budget — a slow-but-working engine is exactly the case a fixed tier
        // timeout throws away.
        if runs.iter().all(|r| r.outcome.is_err()) {
            let retry_id = runs
                .iter()
                .find(|r| !matches!(r.outcome, Err(EngineError::Blocked(_))))
                .map(|r| r.id)
                .or_else(|| plan.primary.first().map(|e| e.id()));
            if let Some(id) = retry_id {
                let candidate = plan
                    .primary
                    .iter()
                    .chain(plan.secondary.iter())
                    .find(|e| e.id() == id)
                    .cloned();
                if let Some(engine) = candidate {
                    tracing::info!(engine = id, budget_ms = LONE_ENGINE_BUDGET.as_millis(), "retrying survivor with full budget");
                    let begin = Instant::now();
                    let outcome = engine.search(&req.query, max_results, LONE_ENGINE_BUDGET).await;
                    runs.retain(|r| r.id != id);
                    runs.push(EngineRun {
                        id,
                        weight: engine.weight(),
                        outcome,
                        elapsed_ms: begin.elapsed().as_millis(),
                    });
                }
            }
        }

        if runs.iter().all(|r| r.outcome.is_err()) {
            let details: Vec<String> = runs
                .iter()
                .map(|r| {
                    format!(
                        "{}: {}",
                        r.id,
                        r.outcome.as_ref().err().map(ToString::to_string).unwrap_or_default()
                    )
                })
                .collect();
            return Err(format!("all upstream engines failed ({})", details.join("; ")));
        }

        let mut degraded: Vec<String> = Vec::new();
        let mut sources: Vec<SourceStat> = Vec::new();
        let mut lists: Vec<(f32, Vec<RawItem>)> = Vec::new();

        for run in &runs {
            match &run.outcome {
                Ok(output) => {
                    // An exact arXiv identifier is a lookup, not a keyword query: the
                    // returned papers legitimately do not repeat the id in their titles.
                    let id_lookup = run.id == "arxiv" && has_arxiv_id(&req.query.to_lowercase());
                    let pass = id_lookup
                        || output
                            .items
                            .iter()
                            .take(3)
                            .any(|item| terms.passes(&format!("{} {}", item.title, item.snippet)));
                    if !pass {
                        degraded.push(run.id.to_string());
                        tracing::debug!(engine = run.id, "relevance gate failed, weight x0.3");
                    }
                    sources.push(SourceStat {
                        name: run.id.to_string(),
                        count: output.items.len(),
                        ms: output.elapsed_ms,
                        status: if pass { "ok".into() } else { "degraded".into() },
                        error: None,
                    });
                    lists.push((if pass { run.weight } else { run.weight * 0.3 }, output.items.clone()));
                }
                Err(err) => {
                    sources.push(SourceStat {
                        name: run.id.to_string(),
                        count: 0,
                        ms: run.elapsed_ms,
                        status: "error".into(),
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        let fused = fusion::fuse(lists, 60.0);
        let deduped = dedupe::dedupe(fused);

        // Precision chain: noise hosts → aggregator titles → portal roots →
        // concept coverage → script mismatch → duplicate content.
        let report = filter::apply(&terms, deduped.clone());
        let filter_stats: Vec<FilterStat> = report
            .dropped
            .iter()
            .map(|(reason, count)| FilterStat { reason: (*reason).to_string(), count: *count })
            .collect();

        // If the strict chain emptied the page, fall back to anything matching at
        // least one concept (still excluding known noise) rather than padding with junk.
        let candidates = if report.kept.is_empty() {
            deduped
                .into_iter()
                .filter(|c| {
                    let text = format!("{} {}", c.title, c.snippet);
                    terms.matched(&text) >= 1 && !filter::is_noise(&c.url)
                })
                .collect()
        } else {
            report.kept
        };

        let scored = score::score_all(&terms, candidates, &score::Weights::default());

        let mut per_domain: HashMap<String, usize> = HashMap::new();
        let mut selected = Vec::with_capacity(max_results);
        for item in scored {
            let domain = normalize::domain_of(&item.candidate.url);
            let slot = per_domain.entry(domain).or_insert(0);
            if *slot >= MAX_PER_DOMAIN {
                continue;
            }
            *slot += 1;
            selected.push(item);
            if selected.len() >= max_results {
                break;
            }
        }

        let explain_all = req.explain.unwrap_or(false);
        let results: Vec<ResultItem> = selected
            .into_iter()
            .map(|scored| {
                // Emit the canonical URL: tracking params stripped, fragment dropped.
                let url = normalize::canonical(&scored.candidate.url)
                    .unwrap_or_else(|| scored.candidate.url.clone());
                ResultItem {
                    title: scored.candidate.title,
                    url,
                    snippet: scored.candidate.snippet.clone(),
                    content: scored.candidate.snippet,
                    source: scored.candidate.source.to_string(),
                    score: (scored.score * 1000.0).round() / 1000.0,
                    explain: if explain_all { Some(scored.explain) } else { None },
                }
            })
            .collect();

        let routing = RoutingInfo {
            intent: plan.intent,
            primary: plan.primary.iter().map(|e| e.id().to_string()).collect(),
            secondary: plan.secondary.iter().map(|e| e.id().to_string()).collect(),
            degraded,
        };

        let data = SearchData {
            metadata: Metadata {
                total_results: results.len(),
                search_time_ms: started.elapsed().as_millis(),
                cached: false,
                routing,
                sources,
                filters: filter_stats,
            },
            results,
        };

        // Only successful, non-empty result sets are cached: caching a transient
        // failure would keep serving it for five minutes.
        if !data.results.is_empty() {
            self.cache_put(cache_key, &data);
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubEngine {
        name: &'static str,
        weight: f32,
        domains: &'static [&'static str],
    }

    #[async_trait::async_trait]
    impl Engine for StubEngine {
        fn id(&self) -> &'static str {
            self.name
        }
        fn weight(&self) -> f32 {
            self.weight
        }
        fn domains(&self) -> &'static [&'static str] {
            self.domains
        }
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
            _timeout: Duration,
        ) -> Result<EngineOutput, EngineError> {
            Ok(EngineOutput { items: Vec::new(), http_status: Some(200), parsed: 0, elapsed_ms: 0 })
        }
    }

    fn stub(name: &'static str, weight: f32) -> Arc<dyn Engine> {
        Arc::new(StubEngine { name, weight, domains: &["general"] })
    }

    fn req(query: &str) -> SearchRequest {
        SearchRequest {
            query: query.into(),
            max_results: None,
            zone: None,
            language: None,
            domain: None,
            sub_domain: None,
            params: None,
            explain: None,
            debug: None,
        }
    }

    #[test]
    fn prefers_chinese_engine_for_cjk_queries() {
        let orch = Orchestrator::new(Config::default(), vec![stub("so360", 1.0), stub("bing", 0.8)]);
        let plan = orch.plan(&req("大模型 推理 优化"));
        assert_eq!(plan.intent, "general_cn");
        assert_eq!(plan.primary[0].id(), "so360");
        assert_eq!(plan.secondary[0].id(), "bing");
    }

    #[test]
    fn prefers_bing_for_english_queries() {
        let orch = Orchestrator::new(Config::default(), vec![stub("so360", 1.0), stub("bing", 0.8)]);
        let plan = orch.plan(&req("weather in hangzhou"));
        assert_eq!(plan.intent, "general_en");
        assert_eq!(plan.primary[0].id(), "bing");
    }

    #[test]
    fn routes_developer_queries_to_vertical_engines() {
        let engines: Vec<Arc<dyn Engine>> = vec![
            stub("so360", 1.0),
            stub("bing", 0.8),
            stub("github", 0.9),
            stub("stackexchange", 0.8),
            stub("arxiv", 0.95),
            stub("crossref", 0.9),
        ];
        let orch = Orchestrator::new(Config::default(), engines);

        let code = orch.plan(&req("tokio select usage"));
        assert_eq!(code.intent, "code");
        assert_eq!(code.primary[0].id(), "github");

        let academic = orch.plan(&req("arxiv 2401.00001"));
        assert_eq!(academic.intent, "academic");
        assert_eq!(academic.primary[0].id(), "arxiv");
    }
}
