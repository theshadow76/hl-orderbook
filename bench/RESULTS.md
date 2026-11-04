# Benchmark results

Run on 4 November 2026 with `bench/bench.sh`, release build, Hyperliquid mainnet, `HL_COINS=BTC,ETH,SOL`, depth 20, on a Linux laptop in France. Single process, no tuning.

## Venue update rate and freshness (30 s sample)

| Coin | l2Book snapshots received | Age of served book, p50 | p99 |
|---|---|---|---|
| BTC | 0.2 / s | 2,964 ms | 5,836 ms |
| ETH | 0.2 / s | 2,964 ms | 5,836 ms |
| SOL | 0.2 / s | 2,965 ms | 5,836 ms |

**Reading this honestly:** the venue delivered one full snapshot every 3 to 5 seconds per coin during this sample. An independent Python WebSocket client run at the same time saw the same rate (5 messages in 15 s for BTC, venue timestamp to receipt p50 501 ms), so the cadence is the venue's, not the service's. The "age of served book" is therefore dominated by the interval between venue snapshots; the service's own cost between receiving a message and serving it is below the 10 ms polling resolution of the script. Identical numbers across coins are expected because Hyperliquid emits all three books on the same block cadence.

## HTTP read latency (2,000 sequential requests, `GET /orderbook/BTC?depth=20`)

| p50 | p90 | p99 | max | throughput |
|---|---|---|---|---|
| 185 µs | 258 µs | 341 µs | 13.2 ms | 4,875 req/s from one sequential client |

That includes Python's `urllib` overhead on the client side; the server-side cost is lower. The single 13 ms outlier coincided with a book write.

## Process

RSS 3.7 MB, CPU 0.0 % between messages, after 60 s of operation with three coins subscribed.

## Reproduce

```bash
cargo build --release
./target/release/hl-orderbook &
SECONDS_TO_SAMPLE=30 REQUESTS=2000 ./bench/bench.sh
```
