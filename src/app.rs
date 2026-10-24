use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::orchestrator::Orchestrator;
use crate::engines::{bing::Bing, so360::So360, Engine};

/// Shared state for the HTTP layer and the MCP server.
pub struct AppState {
    pub config: Config,
    pub orchestrator: Orchestrator,
    pub started_at: Instant,
}

/// Engine roster. Overseas engines are intentionally absent while `intl_enabled`
/// is false: no client, no request, no health entry, no cost.
pub fn build_orchestrator(config: &Config) -> anyhow::Result<Orchestrator> {
    let client = crate::engines::build_client(config)?;
    let mut roster: Vec<Arc<dyn Engine>> = Vec::new();
    roster.push(Arc::new(So360::new(client.clone())));
    roster.push(Arc::new(Bing::new(client)));

    if config.intl_enabled {
        tracing::warn!("intl_enabled=true but no overseas adapter is implemented yet; ignoring");
    }
    Ok(Orchestrator::new(config.clone(), roster))
}

pub fn build_state(config: Config) -> anyhow::Result<Arc<AppState>> {
    let orchestrator = build_orchestrator(&config)?;
    Ok(Arc::new(AppState { config, orchestrator, started_at: Instant::now() }))
}
