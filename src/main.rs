mod browser;
mod captcha;
mod config;
mod extractor;
mod stealth;
mod tls;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use browser::BrowserPool;
use config::Config;

#[derive(Clone)]
struct AppState {
    browser: Arc<BrowserPool>,
}

#[derive(Deserialize)]
struct FetchRequest {
    url: String,
    /// CSS selector to wait for before extracting content.
    wait_for: Option<String>,
    /// Timeout in milliseconds (default: 30000).
    timeout_ms: Option<u64>,
    /// Include links in the response (default: false).
    #[serde(default)]
    include_links: bool,
}

#[derive(Serialize)]
struct FetchResponse {
    url: String,
    title: String,
    markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<Vec<LinkResponse>>,
    token_estimate: usize,
}

#[derive(Serialize)]
struct LinkResponse {
    text: String,
    href: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn health() -> &'static str {
    "ok"
}

async fn fetch_page(
    State(state): State<AppState>,
    Json(req): Json<FetchRequest>,
) -> Result<Json<FetchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let result = state
        .browser
        .fetch(&req.url, req.wait_for.as_deref(), req.timeout_ms)
        .await
        .map_err(|e| {
            tracing::error!("Fetch error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    // Rough token estimate: ~4 chars per token
    let token_estimate = result.markdown.len() / 4;

    let links = if req.include_links {
        Some(
            result
                .links
                .into_iter()
                .map(|l| LinkResponse {
                    text: l.text,
                    href: l.href,
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(Json(FetchResponse {
        url: result.url,
        title: result.title,
        markdown: result.markdown,
        links,
        token_estimate,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "webfetch=info".into()),
        )
        .init();

    let config = Config::from_env();

    tracing::info!(
        "Starting webfetch on {}:{}",
        config.host,
        config.port
    );

    let browser = BrowserPool::new(&config).await.expect("Failed to launch browser");

    let state = AppState {
        browser: Arc::new(browser),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/fetch", post(fetch_page))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    tracing::info!("Listening on {}", addr);
    axum::serve(listener, app).await.expect("Server error");
}
