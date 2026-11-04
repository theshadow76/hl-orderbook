/// Axum HTTP server exposing clean order book data for Flutter clients.
use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::order_book::BookStore;
use crate::types::MarketsResponse;

#[derive(Clone)]
pub struct ServerState {
    pub book_store: Arc<BookStore>,
    pub coins: Vec<String>,
}

#[derive(Deserialize)]
pub struct DepthQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
}

fn default_depth() -> usize {
    20
}

#[derive(Deserialize)]
pub struct LimitQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Build and return the axum Router.
pub fn build_router(state: ServerState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/orderbook/:coin", get(get_orderbook))
        .route("/trades/:coin", get(get_trades))
        .route("/markets", get(get_markets))
        .route("/mids", get(get_mids))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// GET /health
async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok", "service": "hl-orderbook"}))
}

/// GET /orderbook/:coin?depth=20
/// Returns a clean, Flutter-ready L2 order book snapshot.
async fn get_orderbook(
    Path(coin): Path<String>,
    Query(q): Query<DepthQuery>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    let upper = coin.to_uppercase();
    match state.book_store.get_clean_book(&upper, q.depth) {
        Some(book) => (StatusCode::OK, Json(json!(book))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("No book data yet for coin '{}'", upper),
                "available": state.coins,
            })),
        )
            .into_response(),
    }
}

/// GET /trades/:coin?limit=50
/// Returns the most recent trades for a coin.
async fn get_trades(
    Path(coin): Path<String>,
    Query(q): Query<LimitQuery>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    let upper = coin.to_uppercase();
    let trades = state.book_store.get_recent_trades(&upper, q.limit);
    if trades.is_empty() && !state.coins.contains(&upper) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("No trade data yet for coin '{}'", upper),
                "available": state.coins,
            })),
        )
            .into_response();
    }
    (StatusCode::OK, Json(json!({ "coin": upper, "trades": trades }))).into_response()
}

/// GET /markets
/// Returns a summary of all tracked markets + mid prices.
async fn get_markets(State(state): State<ServerState>) -> impl IntoResponse {
    let summaries = state.book_store.get_all_summaries();
    let all_mids: HashMap<String, f64> = summaries
        .iter()
        .filter_map(|s| s.mid_price.map(|m| (s.coin.clone(), m)))
        .collect();

    Json(json!(MarketsResponse {
        markets: summaries,
        all_mids,
    }))
}

/// GET /mids
/// Lightweight endpoint: just returns { "BTC": 50000.5, ... }
async fn get_mids(State(state): State<ServerState>) -> impl IntoResponse {
    let summaries = state.book_store.get_all_summaries();
    let mids: HashMap<String, Value> = summaries
        .iter()
        .map(|s| {
            (
                s.coin.clone(),
                s.mid_price
                    .map(|m| json!(m))
                    .unwrap_or(Value::Null),
            )
        })
        .collect();
    Json(json!(mids))
}

pub async fn serve(state: ServerState, port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    info!("hl-orderbook HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind HTTP port");
    axum::serve(listener, build_router(state))
        .await
        .expect("HTTP server crashed");
}
