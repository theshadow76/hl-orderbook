/// In-memory order book store: maintains the latest L2 snapshot per coin.
use std::collections::VecDeque;
use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use tracing::debug;

use crate::types::{BookLevel, CleanOrderBook, MarketSummary, WsBook, WsLevel, WsTrade};

const MAX_RECENT_TRADES: usize = 200;

/// Internal book state for one coin.
struct CoinBook {
    bids: Vec<WsLevel>,
    asks: Vec<WsLevel>,
    timestamp_ms: u64,
}

/// Trade ring-buffer for one coin.
struct TradeBuf {
    trades: VecDeque<WsTrade>,
}

impl TradeBuf {
    fn new() -> Self {
        Self {
            trades: VecDeque::with_capacity(MAX_RECENT_TRADES),
        }
    }
    fn push(&mut self, trade: WsTrade) {
        if self.trades.len() >= MAX_RECENT_TRADES {
            self.trades.pop_back();
        }
        self.trades.push_front(trade);
    }
}

/// Thread-safe store for all coin books and recent trades.
pub struct BookStore {
    books: DashMap<String, Arc<RwLock<CoinBook>>>,
    trades: DashMap<String, Arc<RwLock<TradeBuf>>>,
    max_depth: usize,
}

impl BookStore {
    pub fn new(max_depth: usize) -> Self {
        Self {
            books: DashMap::new(),
            trades: DashMap::new(),
            max_depth,
        }
    }

    /// Apply a full L2 snapshot from Hyperliquid WS (always a full replace).
    pub fn apply_update(&self, book: WsBook) {
        let coin = book.coin.clone();
        debug!(
            "Book update {} — bids:{} asks:{}",
            coin,
            book.levels[0].len(),
            book.levels[1].len()
        );
        let entry = self
            .books
            .entry(coin)
            .or_insert_with(|| {
                Arc::new(RwLock::new(CoinBook {
                    bids: vec![],
                    asks: vec![],
                    timestamp_ms: 0,
                }))
            })
            .clone();

        let mut guard = entry.write();
        guard.bids = book.levels[0].clone();
        guard.asks = book.levels[1].clone();
        guard.timestamp_ms = book.time;
    }

    /// Record a recent trade.
    pub fn record_trade(&self, trade: WsTrade) {
        let coin = trade.coin.clone();
        let entry = self
            .trades
            .entry(coin)
            .or_insert_with(|| Arc::new(RwLock::new(TradeBuf::new())))
            .clone();
        entry.write().push(trade);
    }

    /// Get a clean, Flutter-ready order book snapshot for a coin.
    pub fn get_clean_book(&self, coin: &str, depth: usize) -> Option<CleanOrderBook> {
        let entry = self.books.get(coin)?;
        let guard = entry.read();

        let effective_depth = depth.min(self.max_depth);

        let bids = build_levels(&guard.bids, effective_depth, true);
        let asks = build_levels(&guard.asks, effective_depth, false);

        let best_bid = bids.first().map(|l| l.price);
        let best_ask = asks.first().map(|l| l.price);

        let mid_price = match (best_bid, best_ask) {
            (Some(b), Some(a)) => Some((b + a) / 2.0),
            _ => None,
        };
        let spread = match (best_bid, best_ask) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        };
        let spread_pct = match (spread, mid_price) {
            (Some(s), Some(m)) if m > 0.0 => Some((s / m) * 100.0),
            _ => None,
        };

        Some(CleanOrderBook {
            coin: coin.to_owned(),
            timestamp_ms: guard.timestamp_ms,
            mid_price,
            spread,
            spread_pct,
            bids,
            asks,
            updated_at: Utc::now(),
        })
    }

    /// Get a market summary for all tracked coins.
    pub fn get_all_summaries(&self) -> Vec<MarketSummary> {
        self.books
            .iter()
            .map(|entry| {
                let guard = entry.value().read();
                let best_bid = guard
                    .bids
                    .first()
                    .and_then(|l| l.px.parse::<f64>().ok());
                let best_ask = guard
                    .asks
                    .first()
                    .and_then(|l| l.px.parse::<f64>().ok());
                let mid_price = match (best_bid, best_ask) {
                    (Some(b), Some(a)) => Some((b + a) / 2.0),
                    _ => None,
                };
                let spread = match (best_bid, best_ask) {
                    (Some(b), Some(a)) => Some(a - b),
                    _ => None,
                };
                let bid_depth: f64 = guard
                    .bids
                    .iter()
                    .take(self.max_depth)
                    .filter_map(|l| l.sz.parse::<f64>().ok())
                    .sum();
                let ask_depth: f64 = guard
                    .asks
                    .iter()
                    .take(self.max_depth)
                    .filter_map(|l| l.sz.parse::<f64>().ok())
                    .sum();

                MarketSummary {
                    coin: entry.key().clone(),
                    mid_price,
                    best_bid,
                    best_ask,
                    spread,
                    bid_depth,
                    ask_depth,
                    last_update_ms: guard.timestamp_ms,
                }
            })
            .collect()
    }

    /// Get recent trades for a coin (newest first).
    pub fn get_recent_trades(&self, coin: &str, limit: usize) -> Vec<WsTrade> {
        self.trades
            .get(coin)
            .map(|entry| {
                entry
                    .read()
                    .trades
                    .iter()
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Convert raw WsLevel list into clean BookLevel list with cumulative sizes.
fn build_levels(raw: &[WsLevel], depth: usize, _is_bids: bool) -> Vec<BookLevel> {
    let mut cumulative = 0.0_f64;
    raw.iter()
        .take(depth)
        .filter_map(|l| {
            let price = l.px.parse::<f64>().ok()?;
            let size = l.sz.parse::<f64>().ok()?;
            cumulative += size;
            Some(BookLevel {
                price,
                size,
                orders: l.n,
                cumulative,
            })
        })
        .collect()
}
