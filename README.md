# hl-orderbook

A small Rust service that keeps live L2 order books and recent trades for a set of Hyperliquid perpetual markets and serves them as clean JSON over HTTP.

It is the market-data edge of [ChipaX](https://exchange.chipatrade.com/trade/BTC), a live perpetuals exchange. One instance holds one WebSocket to the venue for all subscribed coins; every web and mobile client reads from this service instead of opening its own venue connection.

```
Hyperliquid WS ──l2Book / trades──▶ ws_client ──▶ BookStore (DashMap + RwLock per coin)
                                                  │
                                       broadcast::channel<BookEvent>
                                                  │
                           axum ──▶ /orderbook/:coin  /trades/:coin  /markets  /mids  /health
```

## What it does

- **Subscribes** to `l2Book` and `trades` for each coin in `HL_COINS` over a single WebSocket (`tokio-tungstenite`), with a 30 s application-level ping.
- **Reconnects forever** with exponential backoff (1 s doubling to a 30 s cap) and resubscribes on every new connection. A venue hiccup never kills the process.
- **Stores** the latest book per coin behind a `DashMap<String, Arc<RwLock<CoinBook>>>`, so readers on different coins never contend and a slow HTTP reader never blocks ingestion. Hyperliquid sends full snapshots on every `l2Book` message, so updates are a replace, not a delta merge.
- **Keeps** the last 200 trades per coin in a ring buffer (`VecDeque`).
- **Normalizes** the venue's string prices and sizes into numeric levels with cumulative depth, mid price, spread and spread percentage, capped at `HL_MAX_DEPTH`.
- **Broadcasts** a `BookUpdated(coin)` event on a `tokio::sync::broadcast` channel so new consumers (SSE, recorders, alerting) can attach without touching the ingestion path.
- **Serves** JSON over `axum` with permissive CORS and request tracing.

## Endpoints

| Route | Returns |
|---|---|
| `GET /orderbook/:coin?depth=20` | Bids and asks with `price`, `size`, `orders`, `cumulative`; plus `mid_price`, `spread`, `spread_pct`, venue `timestamp_ms` and local `updated_at`. |
| `GET /trades/:coin?limit=50` | Most recent trades, newest first. |
| `GET /markets` | Summary for every tracked coin: best bid/ask, mid, spread, bid and ask depth, last update. |
| `GET /mids` | `{ "BTC": 67842.5, ... }` only. |
| `GET /health` | Liveness. |

Unknown coins return `404` with the list of tracked coins.

## Run

```bash
cargo run --release
# or
docker build -t hl-orderbook . && docker run -p 9090:9090 -e HL_COINS=BTC,ETH,SOL hl-orderbook
```

```bash
curl -s localhost:9090/orderbook/BTC?depth=5 | jq
```

### Configuration

| Variable | Default | Meaning |
|---|---|---|
| `HL_COINS` | `BTC,ETH,SOL` | Comma-separated coins to subscribe to |
| `HL_MAX_DEPTH` | `20` | Maximum levels served per side |
| `HL_ORDERBOOK_PORT` | `9090` | HTTP port |
| `HL_TESTNET` | `false` | Use the Hyperliquid testnet endpoints |
| `HL_WS_URL` / `HL_INFO_URL` | mainnet | Override venue URLs |
| `RUST_LOG` | `info` | `tracing` filter |

## Benchmarks

Measured with [`bench/bench.sh`](bench/bench.sh) against mainnet, three coins, default depth, on a laptop. See [`bench/RESULTS.md`](bench/RESULTS.md) for the raw numbers from the latest run and how they were taken.

## Layout

```
src/main.rs         config from env, wiring, spawns the WS task, runs axum
src/ws_client.rs    connect, subscribe, ping, reconnect loop, message dispatch
src/order_book.rs   BookStore: per-coin book state, trade ring buffer, normalization
src/http_server.rs  axum router and handlers
src/types.rs        venue wire types and clean output types
bench/              load script and results
```

## Design notes

- **Why a full replace per message?** Hyperliquid's `l2Book` channel sends the complete top-of-book snapshot on every update rather than deltas, so there are no sequence numbers to track and no gap recovery to implement. The trade-off is bandwidth, which is fine for 20 levels and a handful of coins.
- **Why per-coin locks?** A single global lock would serialize ingestion for BTC behind a slow reader of SOL. `DashMap` shards the map and `parking_lot::RwLock` keeps the per-coin critical section tiny: two `Vec` assignments.
- **Why a broadcast channel nobody listens to yet?** It costs nothing when there are no receivers and it means the next feature (server-sent events to browsers) does not need to touch the WebSocket code.
- **What it does not do:** persistence, multiple venues, delta books, or authentication. Those live elsewhere in the ChipaX stack.

## License

MIT. See [LICENSE](LICENSE).
