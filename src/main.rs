/// ChipaX — Hyperliquid L2 Order Book Service
///
/// Subscribes to Hyperliquid WebSocket for real-time L2 books and trades,
/// maintains in-memory state, and serves clean JSON for Flutter clients.
mod http_server;
mod order_book;
mod types;
mod ws_client;

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use http_server::{serve, ServerState};
use order_book::BookStore;
use types::AppConfig;
use ws_client::WsClient;

#[tokio::main]
async fn main() {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Tracing — default to info, override with RUST_LOG env var
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer())
        .init();

    let config = build_config();
    info!(
        "hl-orderbook starting — coins: {:?}, port: {}",
        config.coins, config.port
    );

    let book_store = Arc::new(BookStore::new(config.max_depth));

    // Broadcast channel — allows future SSE endpoints to subscribe to book events
    let (event_tx, _event_rx) = broadcast::channel::<ws_client::BookEvent>(1024);

    // Spawn the WebSocket subscriber
    let ws_store = Arc::clone(&book_store);
    let ws_coins = config.coins.clone();
    let ws_url = config.hl_ws_url.clone();
    let ws_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let client = WsClient::new(ws_url, ws_coins, ws_store, ws_event_tx);
        client.run_forever().await;
    });

    // Run the HTTP server (blocks until process exits)
    let server_state = ServerState {
        book_store,
        coins: config.coins.clone(),
    };
    serve(server_state, config.port).await;
}

fn build_config() -> AppConfig {
    let testnet = std::env::var("HL_TESTNET")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    let hl_ws_url = if testnet {
        "wss://api.hyperliquid-testnet.xyz/ws".into()
    } else {
        std::env::var("HL_WS_URL")
            .unwrap_or_else(|_| "wss://api.hyperliquid.xyz/ws".into())
    };

    let hl_info_url = if testnet {
        "https://api.hyperliquid-testnet.xyz/info".into()
    } else {
        std::env::var("HL_INFO_URL")
            .unwrap_or_else(|_| "https://api.hyperliquid.xyz/info".into())
    };

    // Comma-separated list of coins, default BTC,ETH,SOL
    let coins: Vec<String> = std::env::var("HL_COINS")
        .unwrap_or_else(|_| "BTC,ETH,SOL".into())
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    let max_depth: usize = std::env::var("HL_MAX_DEPTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);

    let port: u16 = std::env::var("HL_ORDERBOOK_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9090);

    AppConfig {
        hl_ws_url,
        hl_info_url,
        coins,
        max_depth,
        port,
    }
}
