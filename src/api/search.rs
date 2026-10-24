use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::api::new_request_id;
use crate::model::{Envelope, SearchRequest};
use crate::AppState;

/// `POST /v1/search` — AnySearch-compatible envelope.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Response {
    let request_id = new_request_id();
    let query = req.query.trim().to_string();

    if query.is_empty() || query.chars().count() > 512 {
        return failure(StatusCode::BAD_REQUEST, 400, request_id, "query must be 1..=512 characters");
    }
    if let Some(max) = req.max_results {
        if max == 0 || max > state.config.max_results_limit {
            return failure(
                StatusCode::BAD_REQUEST,
                400,
                request_id,
                format!("max_results must be 1..={}", state.config.max_results_limit),
            );
        }
    }

    let request = SearchRequest { query, ..req };
    tracing::debug!(query = %request.query, "search");
    match state.orchestrator.search(&request).await {
        Ok(data) => (StatusCode::OK, Json(Envelope::ok(request_id, data))).into_response(),
        Err(message) => {
            tracing::warn!(%message, "search failed");
            failure(StatusCode::BAD_GATEWAY, 502, request_id, message)
        }
    }
}

fn failure(
    status: StatusCode,
    code: i32,
    request_id: String,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(Envelope::<serde_json::Value>::err(code, request_id, message)),
    )
        .into_response()
}
