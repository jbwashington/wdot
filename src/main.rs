mod behavior;
mod browser;
mod captcha;
mod config;
mod extractor;
mod osint;
mod reputation;
mod stealth;
mod tls;

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use behavior::BehaviorEngine;
use browser::BrowserPool;
use config::Config;
use osint::OsintEngine;
use reputation::ReputationMonitor;

#[derive(Clone)]
struct AppState {
    browser: Arc<BrowserPool>,
    osint: Arc<OsintEngine>,
    behavior: Arc<BehaviorEngine>,
    reputation: Arc<ReputationMonitor>,
}

// --- Fetch types ---

#[derive(Deserialize)]
struct FetchRequest {
    url: String,
    wait_for: Option<String>,
    timeout_ms: Option<u64>,
    #[serde(default)]
    include_links: bool,
    max_tokens: Option<usize>,
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

// --- OSINT types ---

#[derive(Deserialize)]
struct OsintRequest {
    target: String,
}

#[derive(Deserialize)]
struct DnsRequest {
    domain: String,
}

// --- Behavior types ---

#[derive(Deserialize)]
struct RecordRequest {
    name: String,
}

// --- Handlers ---

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

    let markdown = if req.max_tokens.is_some() {
        let max_chars = req.max_tokens.map(|t| t * 4);
        extractor::html_to_markdown(&result.raw_html, max_chars)
    } else {
        result.markdown
    };

    let token_estimate = markdown.len() / 4;

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
        markdown,
        links,
        token_estimate,
    }))
}

// --- OSINT handlers ---

async fn osint_scan(
    State(state): State<AppState>,
    Json(req): Json<OsintRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let report = state.osint.scan(&req.target).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(Json(serde_json::to_value(report).unwrap()))
}

async fn osint_emails(
    State(state): State<AppState>,
    Json(req): Json<OsintRequest>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let result = state
        .browser
        .fetch(&req.target, None, Some(30_000))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(Json(osint::emails::extract(&result.raw_html)))
}

async fn osint_tech(
    State(state): State<AppState>,
    Json(req): Json<OsintRequest>,
) -> Result<Json<Vec<osint::tech::Technology>>, (StatusCode, Json<ErrorResponse>)> {
    let result = state
        .browser
        .fetch(&req.target, None, Some(30_000))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(Json(osint::tech::extract_from_html(&result.raw_html)))
}

async fn osint_dns(
    Json(req): Json<DnsRequest>,
) -> Result<Json<osint::dns::DnsInfo>, (StatusCode, Json<ErrorResponse>)> {
    let info = osint::dns::lookup(&req.domain).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(Json(info))
}

// --- Behavior handlers ---

async fn behavior_record_start(
    State(state): State<AppState>,
    Json(req): Json<RecordRequest>,
) -> Json<serde_json::Value> {
    state.behavior.start_recording(req.name.clone()).await;
    Json(serde_json::json!({"status": "recording", "name": req.name}))
}

async fn behavior_record_stop(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    match state.behavior.stop_recording().await {
        Some(profile) => Json(serde_json::json!({
            "status": "saved",
            "name": profile.name,
        })),
        None => Json(serde_json::json!({"status": "not_recording"})),
    }
}

async fn behavior_profiles_list(
    State(state): State<AppState>,
) -> Json<Vec<String>> {
    Json(state.behavior.list_profiles())
}

async fn behavior_activate(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state.behavior.activate_profile(&name).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: e }),
        )
    })?;
    Ok(Json(serde_json::json!({"status": "activated", "name": name})))
}

async fn behavior_delete(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state.behavior.delete_profile(&name).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: e }),
        )
    })?;
    Ok(Json(serde_json::json!({"status": "deleted", "name": name})))
}

// --- Reputation handlers ---

async fn reputation_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let score = state.reputation.score().await;
    let adaptive = state.reputation.adaptive_state().await;
    Json(serde_json::json!({
        "score": score,
        "adaptive": adaptive,
    }))
}

async fn reputation_history(
    State(state): State<AppState>,
) -> Json<Vec<reputation::signals::SessionSignals>> {
    Json(state.reputation.history(50).await)
}

async fn reputation_reset(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    state.reputation.reset().await;
    Json(serde_json::json!({"status": "reset"}))
}

async fn reputation_config_get(
    State(state): State<AppState>,
) -> Json<reputation::adapter::AdaptiveConfig> {
    Json(state.reputation.get_config().await)
}

async fn reputation_config_update(
    State(state): State<AppState>,
    Json(config): Json<reputation::adapter::AdaptiveConfig>,
) -> Json<serde_json::Value> {
    state.reputation.update_config(config).await;
    Json(serde_json::json!({"status": "updated"}))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wdot=info".into()),
        )
        .init();

    let config = Config::from_env();

    tracing::info!("Starting wdot on {}:{}", config.host, config.port);
    tracing::info!("Data dir: {:?}", config.data_dir);

    // Create data directory
    std::fs::create_dir_all(&config.data_dir).ok();

    let reputation = Arc::new(ReputationMonitor::new(config.reputation_window));
    let behavior = Arc::new(BehaviorEngine::new(config.data_dir.clone()));

    // Activate behavior profile if configured
    if let Some(ref profile_name) = config.behavior_profile {
        if let Err(e) = behavior.activate_profile(profile_name).await {
            tracing::warn!("Could not load behavior profile '{}': {}", profile_name, e);
        } else {
            tracing::info!("Behavior profile '{}' activated", profile_name);
        }
    }

    let browser = BrowserPool::new(
        &config,
        Some(reputation.clone()),
        Some(behavior.clone()),
    )
    .await
    .expect("Failed to launch browser");

    let browser = Arc::new(browser);
    let osint = Arc::new(OsintEngine::new(browser.clone()));

    let state = AppState {
        browser,
        osint,
        behavior,
        reputation,
    };

    let app = Router::new()
        // Core
        .route("/health", get(health))
        .route("/fetch", post(fetch_page))
        // OSINT
        .route("/osint/scan", post(osint_scan))
        .route("/osint/emails", post(osint_emails))
        .route("/osint/tech", post(osint_tech))
        .route("/osint/dns", post(osint_dns))
        // Behavior
        .route("/behavior/record/start", post(behavior_record_start))
        .route("/behavior/record/stop", post(behavior_record_stop))
        .route("/behavior/profiles", get(behavior_profiles_list))
        .route("/behavior/profiles/:name/activate", post(behavior_activate))
        .route("/behavior/profiles/:name", delete(behavior_delete))
        // Reputation
        .route("/reputation", get(reputation_status))
        .route("/reputation/history", get(reputation_history))
        .route("/reputation/reset", post(reputation_reset))
        .route("/reputation/config", get(reputation_config_get))
        .route("/reputation/config", put(reputation_config_update))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    tracing::info!("Listening on {}", addr);
    axum::serve(listener, app).await.expect("Server error");
}
