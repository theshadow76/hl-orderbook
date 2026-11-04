#!/usr/bin/env bash
# Measures two things against a running hl-orderbook on $PORT (default 9090):
#   1. Venue update rate: how many distinct l2Book snapshots arrive per coin per second
#      (sampled by polling timestamp_ms and counting changes).
#   2. HTTP read latency: p50 / p99 of GET /orderbook/BTC?depth=20 over N requests.
# Requires: curl, python3. No external benchmarking tools.
set -euo pipefail
PORT="${PORT:-9090}"
COINS="${COINS:-BTC ETH SOL}"
SECONDS_TO_SAMPLE="${SECONDS_TO_SAMPLE:-30}"
REQUESTS="${REQUESTS:-2000}"
BASE="http://127.0.0.1:${PORT}"

echo "== waiting for books =="
for i in $(seq 1 30); do
    if curl -sf "$BASE/orderbook/BTC" >/dev/null; then break; fi
    sleep 1
done

echo "== venue update rate (${SECONDS_TO_SAMPLE}s sample) =="
python3 - "$BASE" "$SECONDS_TO_SAMPLE" $COINS <<'EOF'
import json, sys, time, urllib.request
base, secs, coins = sys.argv[1], float(sys.argv[2]), sys.argv[3:]
last = {c: None for c in coins}; changes = {c: 0 for c in coins}; staleness = {c: [] for c in coins}
end = time.time() + secs
while time.time() < end:
    for c in coins:
        d = json.loads(urllib.request.urlopen(f"{base}/orderbook/{c}?depth=1").read())
        ts = d["timestamp_ms"]
        now_ms = time.time() * 1000
        staleness[c].append(now_ms - ts)
        if last[c] is not None and ts != last[c]:
            changes[c] += 1
        last[c] = ts
    time.sleep(0.01)
for c in coins:
    st = sorted(staleness[c])
    print(f"{c:4s} {changes[c]/secs:6.1f} snapshots/s   age of served book: p50 {st[len(st)//2]:.0f} ms  p99 {st[int(len(st)*0.99)]:.0f} ms")
EOF

echo "== HTTP read latency (${REQUESTS} sequential requests, depth=20) =="
python3 - "$BASE" "$REQUESTS" <<'EOF'
import sys, time, urllib.request
base, n = sys.argv[1], int(sys.argv[2])
lat = []
for _ in range(n):
    t = time.perf_counter()
    urllib.request.urlopen(f"{base}/orderbook/BTC?depth=20").read()
    lat.append((time.perf_counter() - t) * 1e6)
lat.sort()
print(f"p50 {lat[n//2]:.0f} us   p90 {lat[int(n*0.9)]:.0f} us   p99 {lat[int(n*0.99)]:.0f} us   max {lat[-1]:.0f} us   ({n/sum(lat)*1e6:.0f} req/s sequential)")
EOF

echo "== process =="
pid=$(pgrep -f 'target/release/hl-orderbook' | head -1 || true)
[ -n "$pid" ] && ps -o rss=,pcpu= -p "$pid" | awk '{printf "RSS %.1f MB   CPU %s%%\n", $1/1024, $2}'
