use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::orchestrator::Orchestrator;
use crate::engines::{
    arxiv::Arxiv, bing::Bing, crossref::Crossref, github::Github, hn::HackerNews, npm::Npm,
    pypi::PyPI, so360::So360, stackexchange::StackExchange, Engine,
};

/// Shared state for the HTTP layer and the MCP server.
pub struct AppState {
    pub config: Config,
    pub orchestrator: Orchestrator,
    pub started_at: Instant,
}

/// Engine roster.
///
/// Two categories, deliberately kept apart: general web engines scrape public result
/// pages (unauthenticated, rate-limited by the operator), while the vertical engines
/// use official APIs with published quotas. Overseas engines are absent entirely
/// while `intl_enabled` is false.
pub fn build_orchestrator(config: &Config) -> anyhow::Result<Orchestrator> {
    let client = crate::engines::build_client(config)?;
    let mut roster: Vec<Arc<dyn Engine>> = Vec::new();

    // General web (public result pages).
    roster.push(Arc::new(So360::new(client.clone())));
    roster.push(Arc::new(Bing::new(client.clone())));

    // Vertical, official APIs. Free; keys only raise quotas.
    roster.push(Arc::new(Github::new(client.clone())));
    roster.push(Arc::new(StackExchange::new(client.clone())));
    roster.push(Arc::new(HackerNews::new(client.clone())));
    roster.push(Arc::new(Arxiv::new(client.clone())));
    roster.push(Arc::new(Crossref::new(client.clone())));
    roster.push(Arc::new(Npm::new(client.clone())));
    roster.push(Arc::new(PyPI::new(client)));

    if config.intl_enabled {
        tracing::warn!("intl_enabled=true but no overseas adapter is implemented yet; ignoring");
    }
    Ok(Orchestrator::new(config.clone(), roster))
}

pub fn build_state(config: Config) -> anyhow::Result<Arc<AppState>> {
    let orchestrator = build_orchestrator(&config)?;
    Ok(Arc::new(AppState { config, orchestrator, started_at: Instant::now() }))
}
