use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::task::JoinSet;

use crate::config::Config;
use crate::core::{dedupe, filter, fusion, gates, normalize, relevance, score};
use crate::engines::{Engine, EngineError, EngineOutput, RawItem};
use crate::model::{
    FilterStat, Metadata, ResultItem, RoutingInfo, SearchData, SearchRequest, SourceStat,
};

/// Per-host cap so one site cannot fill the whole page.
const MAX_PER_DOMAIN: usize = 3;

pub struct Orchestrator {
    config: Config,
    engines: Vec<Arc<dyn Engine>>,
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
        Self { config, engines }
    }

    pub fn engine_ids(&self) -> Vec<&'static str> {
        self.engines.iter().map(|e| e.id()).collect()
    }

    /// Engine selection. M0 is intentionally simple: pick the language-appropriate
    /// primary set and fire the other concurrently as the fallback tier. The
    /// signal table (routing.json) replaces this in M1 behind the same interface.
    pub fn plan(&self, req: &SearchRequest) -> Plan {
        let prefer_cn = gates::is_cjk_query(&req.query);
        let intent = match &req.domain {
            Some(d) if d != "general" => d.clone(),
            _ if prefer_cn => "general_cn".to_string(),
            _ => "general_en".to_string(),
        };
        let domain_filter = req.domain.as_deref().filter(|d| *d != "general");

        let mut primary = Vec::new();
        let mut secondary = Vec::new();
        for engine in &self.engines {
            if let Some(domain) = domain_filter {
                if !engine.domains().contains(&domain) {
                    continue;
                }
            }
            let is_cn_engine = engine.id() == "so360";
            if is_cn_engine == prefer_cn {
                primary.push(engine.clone());
            } else {
                secondary.push(engine.clone());
            }
        }

        // Explicit domain matched nothing: fall back to general engines instead of
        // returning an empty result set.
        if primary.is_empty() && secondary.is_empty() {
            if let Some(general) = self.engines.iter().find(|e| e.domains().contains(&"general")) {
                primary.push(general.clone());
            }
        }

        Plan { intent, primary, secondary }
    }

    pub async fn search(&self, req: &SearchRequest) -> Result<SearchData, String> {
        let started = Instant::now();
        let max_results = req
            .max_results
            .unwrap_or(self.config.default_max_results)
            .clamp(1, self.config.max_results_limit);
        let terms = relevance::Terms::build(&req.query);
        let plan = self.plan(req);

        let mut tasks: Vec<(Arc<dyn Engine>, Duration)> = Vec::new();
        for engine in &plan.primary {
            tasks.push((engine.clone(), self.config.tier_a_timeout));
        }
        for engine in &plan.secondary {
            tasks.push((engine.clone(), self.config.tier_b_timeout));
        }
        tasks.truncate(self.config.max_upstream_requests.max(1));

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
        if runs.is_empty() {
            return Err("no engine could be scheduled".to_string());
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
                    let pass = output
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

        Ok(SearchData {
            metadata: Metadata {
                total_results: results.len(),
                search_time_ms: started.elapsed().as_millis(),
                cached: false,
                routing,
                sources,
                filters: filter_stats,
            },
            results,
        })
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
        let plan = orch.plan(&req("tokio select usage"));
        assert_eq!(plan.intent, "general_en");
        assert_eq!(plan.primary[0].id(), "bing");
    }
}
