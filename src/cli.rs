//! Command-line surface.
//!
//! Contract: one binary, three ways in — `serve` (HTTP), `mcp` (stdio JSON-RPC),
//! and one-shot `search`/`health` (shell). Agents that cannot speak MCP can always
//! fall back to `samplesearch search "..."`, and both paths share the same orchestrator.

use anyhow::bail;

use crate::app;
use crate::config::Config;
use crate::model::SearchRequest;

pub const HELP: &str = r#"samplesearch — self-hosted search for AI agents

USAGE
  samplesearch [serve]                 Start the HTTP API on 127.0.0.1:8787
  samplesearch mcp                     MCP server over stdio (JSON-RPC 2.0)
  samplesearch search "query" [opts]   One-shot search, prints the JSON envelope
  samplesearch health                  Print engine roster as JSON
  samplesearch doc                     Print the agent-facing usage spec
  samplesearch help                    This text

SEARCH OPTIONS
  -m, --max <1-10>   max_results (default 10)
      --explain      include per-feature score contributions
  -h, --help         help

ENV
  SAMPLESEARCH_BIND, SAMPLESEARCH_TIER_A_TIMEOUT_MS, SAMPLESEARCH_TIER_B_TIMEOUT_MS,
  SAMPLESEARCH_MAX_UPSTREAM, SAMPLESEARCH_DEFAULT_MAX, SAMPLESEARCH_INTL, SAMPLESEARCH_PROXY
"#;

pub const DOC: &str = r#"# samplesearch usage spec (for AI agents)

Prefer the MCP tools when the `samplesearch` MCP server is configured; otherwise use
the CLI. Both return the same data.

## MCP tools
- search          { query, max_results?: 1..10, explain?: bool }
- batch_search    { queries: string[] (1..5), max_results?: 1..10 }
- health          {}

## CLI
  samplesearch search "大模型 推理 优化" --max 5
  samplesearch search "600519 财报" -m 3 --explain

Output is the AnySearch-compatible envelope:
  { "code": 0, "data": { "results": [...], "metadata": {...} } }

## Reading the result
- `data.results[].title/url/snippet` — what you cite.
- `data.metadata.routing` — intent, chosen engines, `degraded` engines (failed the
  relevance gate; their items carry low weight).
- `data.metadata.filters` — why candidates were dropped (noise_host, low_match, …).
  Non-empty is normal; it means precision filtering did its job.
- `data.metadata.total_results` can be below max_results by design: we do not pad
  with irrelevant hits.

## Rules
- Always cite the `url`; never present snippets as your own knowledge.
- If `code != 0` or results are empty, say so instead of inventing an answer.
- For freshness-sensitive questions, add a time word to the query (e.g. "2026 最新").
"#;

pub async fn search(args: &[String]) -> anyhow::Result<()> {
    let mut query_parts: Vec<String> = Vec::new();
    let mut max_results: Option<usize> = None;
    let mut explain = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-m" | "--max" | "--max-results" => {
                i += 1;
                max_results = args.get(i).and_then(|v| v.parse().ok());
                if max_results.is_none() {
                    bail!("--max needs a number between 1 and 10");
                }
            }
            "--explain" => explain = true,
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(());
            }
            flag if flag.starts_with('-') => bail!("unknown flag {flag}"),
            word => query_parts.push(word.to_string()),
        }
        i += 1;
    }

    let query = query_parts.join(" ");
    if query.trim().is_empty() {
        bail!("missing query, e.g. samplesearch search \"tokio select 用法\"");
    }

    let config = Config::from_env();
    let orchestrator = app::build_orchestrator(&config)?;
    let request = SearchRequest {
        query,
        max_results,
        zone: None,
        language: None,
        domain: None,
        sub_domain: None,
        params: None,
        explain: Some(explain),
        debug: None,
    };

    match orchestrator.search(&request).await {
        Ok(data) => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "code": 0, "message": "success", "data": data
            }))?);
            Ok(())
        }
        Err(message) => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "code": 502, "message": message
            }))?);
            std::process::exit(3);
        }
    }
}

pub async fn health() -> anyhow::Result<()> {
    let config = Config::from_env();
    let orchestrator = app::build_orchestrator(&config)?;
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "engines": orchestrator.engine_ids(),
        "intl_enabled": config.intl_enabled,
        "upstream_request_cap": config.max_upstream_requests,
        "bind": config.bind,
    }))?);
    Ok(())
}

/// Shared by `mcp` and `serve` for consistent validation.
pub fn validate(request: &SearchRequest, limit: usize) -> Result<(), String> {
    if request.query.trim().is_empty() || request.query.chars().count() > 512 {
        return Err("query must be 1..=512 characters".to_string());
    }
    if let Some(max) = request.max_results {
        if max == 0 || max > limit {
            return Err(format!("max_results must be 1..={limit}"));
        }
    }
    Ok(())
}
