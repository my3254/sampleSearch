use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::AppState;

/// `GET /health` — engine roster, local quota left per engine, and the switches that
/// change behaviour. Operators use this to tell "no results" from "out of budget".
pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let engines: Vec<serde_json::Value> = state
        .orchestrator
        .quota_snapshot()
        .into_iter()
        .map(|(id, tokens)| json!({ "name": id, "tokens_left": tokens }))
        .collect();

    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": state.started_at.elapsed().as_secs(),
        "engines": engines,
        "intl_enabled": state.config.intl_enabled,
        "upstream_request_cap": state.config.max_upstream_requests,
    }))
}
