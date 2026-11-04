/// All data types for Hyperliquid WebSocket and REST interactions.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Hyperliquid WebSocket inbound ──────────────────────────────────────────

/// A single price level on the order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsLevel {
    /// Price as string (e.g. "50000.0")
    pub px: String,
    /// Size as string (e.g. "0.123")
    pub sz: String,
    /// Number of orders at this level
    pub n: u32,
}

/// Raw L2 book update from Hyperliquid WebSocket.
/// levels[0] = bids (descending), levels[1] = asks (ascending)
#[derive(Debug, Clone, Deserialize)]
pub struct WsBook {
    pub coin: String,
    pub levels: [Vec<WsLevel>; 2],
    pub time: u64,
}

/// A single trade update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsTrade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub hash: String,
    pub time: u64,
    pub tid: u64,
}

/// Best bid / ask update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsBbo {
    pub coin: String,
    pub time: u64,
    pub bbo: [Option<WsLevel>; 2],
}

/// Generic WebSocket envelope from Hyperliquid.
#[derive(Debug, Deserialize)]
pub struct WsEnvelope {
    pub channel: String,
    pub data: serde_json::Value,
}

// ─── Hyperliquid Info endpoint responses ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllMids {
    pub mids: HashMap<String, String>,
}

/// Response from POST /info { "type": "spotDeployAuction" }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotDeployAuction {
    pub states: Vec<SpotDeployState>,
    #[serde(rename = "gasAuction")]
    pub gas_auction: GasAuction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotDeployState {
    pub token: u32,
    pub spec: TokenSpec,
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
    pub spots: Vec<u32>,
    #[serde(rename = "maxSupply")]
    pub max_supply: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSpec {
    pub name: String,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u8,
    #[serde(rename = "weiDecimals")]
    pub wei_decimals: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasAuction {
    #[serde(rename = "startTimeSeconds")]
    pub start_time_seconds: u64,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: u64,
    #[serde(rename = "startGas")]
    pub start_gas: String,
    #[serde(rename = "currentGas")]
    pub current_gas: Option<String>,
    #[serde(rename = "endGas")]
    pub end_gas: Option<String>,
}

// ─── Clean output types for Flutter ─────────────────────────────────────────

/// A single clean order book level for Flutter consumption.
#[derive(Debug, Clone, Serialize)]
pub struct BookLevel {
    /// Numeric price
    pub price: f64,
    /// Numeric size
    pub size: f64,
    /// Number of orders
    pub orders: u32,
    /// Cumulative size from top of book
    pub cumulative: f64,
}

/// Full clean order book snapshot for Flutter.
#[derive(Debug, Clone, Serialize)]
pub struct CleanOrderBook {
    pub coin: String,
    pub timestamp_ms: u64,
    pub mid_price: Option<f64>,
    pub spread: Option<f64>,
    pub spread_pct: Option<f64>,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub updated_at: DateTime<Utc>,
}

/// Aggregated market summary.
#[derive(Debug, Clone, Serialize)]
pub struct MarketSummary {
    pub coin: String,
    pub mid_price: Option<f64>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
    pub bid_depth: f64,
    pub ask_depth: f64,
    pub last_update_ms: u64,
}

/// Combined response for Flutter market list endpoint.
#[derive(Debug, Serialize)]
pub struct MarketsResponse {
    pub markets: Vec<MarketSummary>,
    pub all_mids: HashMap<String, f64>,
}

// ─── App configuration ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Hyperliquid WS URL
    pub hl_ws_url: String,
    /// Hyperliquid Info REST URL
    pub hl_info_url: String,
    /// Coins to subscribe to
    pub coins: Vec<String>,
    /// Max book depth to serve
    pub max_depth: usize,
    /// HTTP port for Flutter API
    pub port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hl_ws_url: "wss://api.hyperliquid.xyz/ws".into(),
            hl_info_url: "https://api.hyperliquid.xyz/info".into(),
            coins: vec!["BTC".into(), "ETH".into(), "SOL".into()],
            max_depth: 20,
            port: 9090,
        }
    }
}
