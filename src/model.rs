use serde::{Deserialize, Serialize};

/// Incoming search request. Field names mirror the AnySearch contract.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub zone: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub sub_domain: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Debug aid: attach per-feature score contributions.
    #[serde(default)]
    pub explain: Option<bool>,
    /// Accept the query-string style switch too (`?explain=1` reaches us as a field
    /// only if the client puts it in the body; the HTTP layer maps the query flag).
    #[serde(default)]
    pub debug: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub content: String,
    pub source: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<Vec<FeatureHit>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureHit {
    pub feature: String,
    pub value: f32,
    pub weight: f32,
    pub contribution: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceStat {
    pub name: String,
    pub count: usize,
    pub ms: u128,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Why candidates were discarded. Filtering that cannot be measured cannot be trusted.
#[derive(Debug, Clone, Serialize)]
pub struct FilterStat {
    pub reason: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingInfo {
    pub intent: String,
    pub primary: Vec<String>,
    pub secondary: Vec<String>,
    pub degraded: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metadata {
    pub total_results: usize,
    pub search_time_ms: u128,
    pub cached: bool,
    pub routing: RoutingInfo,
    pub sources: Vec<SourceStat>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<FilterStat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchData {
    pub results: Vec<ResultItem>,
    pub metadata: Metadata,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub code: i32,
    pub message: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> Envelope<T> {
    pub fn ok(request_id: String, data: T) -> Self {
        Self { code: 0, message: "success".into(), request_id, data: Some(data) }
    }
}

impl Envelope<serde_json::Value> {
    /// AnySearch convention: non-zero `code` with a human readable message.
    pub fn err(code: i32, request_id: String, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), request_id, data: None }
    }
}
