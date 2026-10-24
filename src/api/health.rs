use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::AppState;

/// `GET /health` — engine roster plus the deployment switches that matter.
pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": state.started_at.elapsed().as_secs(),
        "engines": state.orchestrator.engine_ids(),
        "intl_enabled": state.config.intl_enabled,
        "upstream_request_cap": state.config.max_upstream_requests,
    }))
}
