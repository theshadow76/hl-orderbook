/// WebSocket client that subscribes to Hyperliquid L2 order books
/// and trades, maintaining live in-memory state.
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::order_book::BookStore;
use crate::types::{WsBook, WsEnvelope, WsTrade};

/// Outbound events broadcast to any subscribers (e.g. SSE handler).
#[derive(Debug, Clone)]
pub enum BookEvent {
    BookUpdated(String),
}

pub struct WsClient {
    url: String,
    coins: Vec<String>,
    book_store: Arc<BookStore>,
    event_tx: broadcast::Sender<BookEvent>,
}

impl WsClient {
    pub fn new(
        url: String,
        coins: Vec<String>,
        book_store: Arc<BookStore>,
        event_tx: broadcast::Sender<BookEvent>,
    ) -> Self {
        Self {
            url,
            coins,
            book_store,
            event_tx,
        }
    }

    /// Run forever with automatic reconnection.
    pub async fn run_forever(&self) {
        let mut backoff_secs: u64 = 1;
        loop {
            info!("Connecting to Hyperliquid WS: {}", self.url);
            match self.connect_and_stream().await {
                Ok(()) => {
                    info!("WS connection closed cleanly — reconnecting in {}s", backoff_secs);
                }
                Err(e) => {
                    error!("WS error: {:#} — reconnecting in {}s", e, backoff_secs);
                }
            }
            sleep(Duration::from_secs(backoff_secs)).await;
            // Exponential backoff capped at 30s
            backoff_secs = (backoff_secs * 2).min(30);
        }
    }

    async fn connect_and_stream(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (mut write, mut read) = ws_stream.split();

        // Subscribe to l2Book and trades for each coin
        for coin in &self.coins {
            let sub_book = json!({
                "method": "subscribe",
                "subscription": { "type": "l2Book", "coin": coin }
            });
            let sub_trades = json!({
                "method": "subscribe",
                "subscription": { "type": "trades", "coin": coin }
            });
            write.send(Message::Text(sub_book.to_string())).await?;
            write.send(Message::Text(sub_trades.to_string())).await?;
            debug!("Subscribed to l2Book + trades for {}", coin);
        }

        // Ping loop running concurrently
        let mut ping_interval = tokio::time::interval(Duration::from_secs(30));

        info!("WS subscriptions active for coins: {:?}", self.coins);
        // Reset backoff on successful connection is handled by caller
        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    let ping = json!({"method": "ping"});
                    if let Err(e) = write.send(Message::Text(ping.to_string())).await {
                        return Err(e.into());
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text).await;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            warn!("WS server sent Close frame");
                            return Ok(());
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }

    async fn handle_message(&self, text: &str) {
        let Ok(envelope) = serde_json::from_str::<WsEnvelope>(text) else {
            debug!("Non-envelope message: {}", &text[..text.len().min(120)]);
            return;
        };

        match envelope.channel.as_str() {
            "l2Book" => {
                if let Ok(book) = serde_json::from_value::<WsBook>(envelope.data) {
                    let coin = book.coin.clone();
                    self.book_store.apply_update(book);
                    let _ = self.event_tx.send(BookEvent::BookUpdated(coin));
                }
            }
            "trades" => {
                if let Ok(trades) = serde_json::from_value::<Vec<WsTrade>>(envelope.data) {
                    for trade in trades {
                        self.book_store.record_trade(trade);
                    }
                }
            }
            "pong" => {
                debug!("Heartbeat pong received");
            }
            other => {
                debug!("Unhandled channel: {}", other);
            }
        }
    }
}
