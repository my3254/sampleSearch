mod api;
mod app;
mod cli;
mod config;
mod core;
mod engines;
mod mcp;
mod model;

pub use app::AppState;

use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // reqwest is built with `rustls-no-provider`: install a crypto provider explicitly,
    // otherwise the first TLS handshake panics.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("serve");

    // Logs go to stderr in every mode: `mcp` owns stdout for JSON-RPC. One-shot CLI
    // commands stay quiet by default — their caller reads stdout, and a stderr full of
    // WARN lines reads like an error in any tool output that merges the two.
    let default_filter = match command {
        "serve" | "mcp" => "info,samplesearch=debug,tower_http=warn",
        _ => "error",
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_target(false)
        .init();

    match command {
        "serve" => serve().await,
        "mcp" => {
            let state = app::build_state(Config::from_env())?;
            tracing::info!("mcp server ready on stdio");
            mcp::run_stdio(state).await
        }
        "search" => cli::search(&args[1..]).await,
        "health" => cli::health().await,
        "doc" => {
            println!("{}", cli::DOC);
            Ok(())
        }
        "help" | "-h" | "--help" => {
            println!("{}", cli::HELP);
            Ok(())
        }
        other => {
            eprintln!("unknown command: {other}\n\n{}", cli::HELP);
            std::process::exit(2);
        }
    }
}

async fn serve() -> anyhow::Result<()> {
    let config = Config::from_env();
    let addr: std::net::SocketAddr = config.bind.parse()?;
    let state = app::build_state(config)?;

    let router = Router::new()
        .route("/health", get(api::health::health))
        .route("/v1/search", post(api::search::search))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "samplesearch http listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
