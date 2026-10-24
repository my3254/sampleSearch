//! Minimal MCP server over stdio (JSON-RPC 2.0).
//!
//! Implemented directly instead of pulling an SDK: the surface this service needs
//! is `initialize` / `tools/list` / `tools/call` / `ping`, and stdio must stay
//! byte-clean (all logging goes to stderr).

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::app::AppState;
use crate::model::{SearchData, SearchRequest};

const SUPPORTED_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
const DEFAULT_VERSION: &str = "2024-11-05";

pub async fn run_stdio(state: Arc<AppState>) -> anyhow::Result<()> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle(trimmed, &state).await {
            let mut payload = serde_json::to_string(&response)?;
            payload.push('\n');
            stdout.write_all(payload.as_bytes()).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

async fn handle(line: &str, state: &Arc<AppState>) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => return Some(error(Value::Null, -32700, &format!("parse error: {err}"))),
    };
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_VERSION);
            let version = if SUPPORTED_VERSIONS.contains(&requested) { requested } else { DEFAULT_VERSION };
            tracing::info!(version, "mcp initialize");
            Some(result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "mysearch", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Self-hosted search. Use `search` with natural language; results include url+snippet and a metadata block explaining engine routing and filtering."
                }),
            ))
        }
        // Notifications carry no id and must not be answered.
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => Some(result(id, json!({}))),
        "tools/list" => Some(result(id, json!({ "tools": tool_defs() }))),
        "tools/call" => {
            let output = call_tool(&params, state).await;
            Some(result(id, output))
        }
        other => Some(error(
            id.unwrap_or(Value::Null),
            -32601,
            &format!("method not found: {other}"),
        )),
    }
}

fn tool_defs() -> Value {
    json!([
        {
            "name": "search",
            "description": "Search the web through the self-hosted mysearch service. Returns ranked results with title, url and snippet, plus metadata describing which engines ran and why results were filtered.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural-language query, Chinese or English" },
                    "max_results": { "type": "integer", "minimum": 1, "maximum": 10, "description": "How many results (default 10)" },
                    "explain": { "type": "boolean", "description": "Include per-feature score contributions" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "batch_search",
            "description": "Run 2-5 independent queries and return each result set. Use for multi-angle questions or comparing several entities.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1,
                        "maxItems": 5,
                        "description": "Queries to run in parallel"
                    },
                    "max_results": { "type": "integer", "minimum": 1, "maximum": 10 }
                },
                "required": ["queries"]
            }
        },
        {
            "name": "health",
            "description": "Report which search engines are enabled and whether overseas engines are switched on.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

async fn call_tool(params: &Value, state: &Arc<AppState>) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match name {
        "search" => search_tool(&args, state).await,
        "batch_search" => batch_tool(&args, state).await,
        "health" => {
            let text = format!(
                "engines: {}\nintl_enabled: {}\nupstream_request_cap: {}",
                state.orchestrator.engine_ids().join(", "),
                state.config.intl_enabled,
                state.config.max_upstream_requests
            );
            tool_ok(
                text,
                json!({
                    "engines": state.orchestrator.engine_ids(),
                    "intl_enabled": state.config.intl_enabled,
                    "version": env!("CARGO_PKG_VERSION")
                }),
            )
        }
        other => tool_error(&format!("unknown tool: {other}")),
    }
}

async fn search_tool(args: &Value, state: &Arc<AppState>) -> Value {
    let Some(query) = args.get("query").and_then(Value::as_str) else {
        return tool_error("missing required argument: query");
    };
    let request = SearchRequest {
        query: query.to_string(),
        max_results: args.get("max_results").and_then(Value::as_u64).map(|v| v as usize),
        zone: None,
        language: None,
        domain: None,
        sub_domain: None,
        params: None,
        explain: args.get("explain").and_then(Value::as_bool),
        debug: None,
    };
    run_one(request, state).await
}

async fn batch_tool(args: &Value, state: &Arc<AppState>) -> Value {
    let Some(list) = args.get("queries").and_then(Value::as_array) else {
        return tool_error("missing required argument: queries (array of strings)");
    };
    if list.is_empty() || list.len() > 5 {
        return tool_error("queries must contain 1..=5 items");
    }
    let max_results = args.get("max_results").and_then(Value::as_u64).map(|v| v as usize);
    let mut sections = Vec::new();
    let mut structured = Vec::new();
    for item in list {
        let Some(query) = item.as_str() else {
            return tool_error("every item in queries must be a string");
        };
        let request = SearchRequest {
            query: query.to_string(),
            max_results,
            zone: None,
            language: None,
            domain: None,
            sub_domain: None,
            params: None,
            explain: None,
            debug: None,
        };
        match run_search(request, state).await {
            Ok(data) => {
                sections.push(format!("## {query}\n{}", format_markdown(&data)));
                structured.push(serde_json::to_value(&data).unwrap_or(Value::Null));
            }
            Err(message) => {
                sections.push(format!("## {query}\nsearch failed: {message}"));
            }
        }
    }
    tool_ok(sections.join("\n"), json!({ "batch": structured }))
}

async fn run_one(request: SearchRequest, state: &Arc<AppState>) -> Value {
    match run_search(request, state).await {
        Ok(data) => {
            let text = format_markdown(&data);
            let structured = serde_json::to_value(&data).unwrap_or(Value::Null);
            tool_ok(text, structured)
        }
        Err(message) => tool_error(&format!("search failed: {message}")),
    }
}

async fn run_search(request: SearchRequest, state: &Arc<AppState>) -> Result<SearchData, String> {
    if let Err(message) = crate::cli::validate(&request, state.config.max_results_limit) {
        return Err(message);
    }
    state.orchestrator.search(&request).await
}

fn format_markdown(data: &SearchData) -> String {
    let mut out = String::new();
    if data.results.is_empty() {
        out.push_str("No results survived the relevance filters. Try broader or rephrased keywords.\n");
    }
    for (index, item) in data.results.iter().enumerate() {
        out.push_str(&format!("{}. {}\n   {}\n", index + 1, item.title, item.url));
        if !item.snippet.is_empty() {
            let snippet: String = item.snippet.chars().take(220).collect();
            out.push_str(&format!("   {snippet}\n"));
        }
    }
    let sources: Vec<String> = data
        .metadata
        .sources
        .iter()
        .map(|s| format!("{}={}", s.name, s.status))
        .collect();
    out.push_str(&format!(
        "\n({} results, {} ms; sources: {}",
        data.metadata.total_results,
        data.metadata.search_time_ms,
        sources.join(" ")
    ));
    if !data.metadata.routing.degraded.is_empty() {
        out.push_str(&format!("; degraded: {}", data.metadata.routing.degraded.join(",")));
    }
    if !data.metadata.filters.is_empty() {
        let filters: Vec<String> = data
            .metadata
            .filters
            .iter()
            .map(|f| format!("{}={}", f.reason, f.count))
            .collect();
        out.push_str(&format!("; filtered: {}", filters.join(" ")));
    }
    out.push_str(")\n");
    out
}

fn tool_ok(text: String, structured: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false
    })
}

fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

fn result(id: Option<Value>, value: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": value })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
