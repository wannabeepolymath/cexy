# exchange-project-rs

(work in progress)

Experimental Rust exchange components: matching engine, orderbook, API gateway,
and benchmarks.

## Layout
- `engine/` matching engine + orderbook core
- `gateway/` HTTP gateway (Actix Web)
- `benches/` Criterion benchmarks + order-ops throughput runner
- `planning/` design notes and research

Each of `engine/`, `gateway/`, and `benches/` is an independent Cargo package
(there is no workspace root). Run `cargo` commands from within the relevant
package directory.

## Quickstart

### Build everything
```
cd engine   && cargo build
cd gateway  && cargo build
cd benches  && cargo build --release
```

### Run all tests
```
cd engine   && cargo test
cd gateway  && cargo test
```

### Check benches compile
```
cd benches && cargo check --benches
```

### Lint / format
```
cd engine   && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
cd gateway  && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
cd benches  && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
```

## Gateway

### Configuration
The gateway registers its initial set of instruments at boot from a JSON
config file whose path is given by the `GATEWAY_CONFIG` environment variable.
When the variable is unset, the gateway falls back to a built-in dev default
of a single instrument with `instrument_id = 1`.

Example `gateway.json`:
```json
{
  "instruments": [1, 2, 3]
}
```

Validation rules applied at startup:
- `instruments` must be non-empty.
- `instrument_id == 0` is rejected.

### Environment variables
| Variable | Default | Purpose |
| --- | --- | --- |
| `GATEWAY_BIND` | `127.0.0.1:8080` | Listen address |
| `GATEWAY_CONFIG` | _unset_ | Path to JSON config file |

### Run locally
```
cd gateway
cargo run                                       # dev default: instrument 1
GATEWAY_CONFIG=./gateway.json cargo run         # with config file
GATEWAY_BIND=0.0.0.0:9090 cargo run --release   # custom bind addr
```

### HTTP endpoints

#### Health
```
GET /health
  200 OK   { "status": "ok" }
```

#### Public API (`/api/v1`)
```
POST /api/v1/order
  body: {
    "instrument_id": u32,   // must be > 0 and registered
    "account_id":    u64,   // must be > 0
    "request_id":    u64,   // must be > 0
    "order_id":      u64,
    "side":          "buy" | "sell",
    "order_type":    "market" | "limit" | "gtc" | "fak" | "fok" | "post_only",
    "price":         u64?,  // required unless order_type == "market"
    "quantity":      u32
  }
  200 OK    { "trades": usize, "best_bid": u64?, "best_ask": u64? }
  400 Bad Request   - invalid side / order_type / quantity / price / identity
  404 Not Found     - instrument not registered
```

```
POST /api/v1/order/modify
  body: {
    "instrument_id": u32,
    "account_id":    u64,
    "request_id":    u64,
    "order_id":      u64,
    "side":          "buy" | "sell",
    "price":         u64,
    "quantity":      u32
  }
  200 OK    { "trades": usize, "best_bid": u64?, "best_ask": u64? }
  400 Bad Request   - side change not allowed, or place-rejection on the modified order
  404 Not Found     - order not found, or instrument not registered
```

```
DELETE /api/v1/order/{order_id}?instrument_id=&account_id=&request_id=
  200 OK    { "best_bid": u64?, "best_ask": u64? }
  400 Bad Request   - invalid identity params
  404 Not Found     - order not found, or instrument not registered
```

```
GET /api/v1/orderbook?instrument_id=
  200 OK    {
              "bids": [{"price": u64, "quantity": u32}, ...],  // best first
              "asks": [{"price": u64, "quantity": u32}, ...]   // best first
            }
  400 Bad Request   - instrument_id missing or 0
  404 Not Found     - instrument not registered
```

```
GET /api/v1/orderbook/top?instrument_id=
  200 OK    { "best_bid": u64?, "best_ask": u64? }
  400 Bad Request   - instrument_id missing or 0
  404 Not Found     - instrument not registered
```

#### Admin (`/admin`)
```
POST /admin/instruments
  body: { "instrument_id": u32 }
  201 Created  { "instrument_id": u32, "created": true }   // newly registered
  200 OK       { "instrument_id": u32, "created": false }  // already registered (idempotent)
  400 Bad Request   - instrument_id == 0
```

### Example curl flow
```
# Register a new instrument at runtime
curl -s -X POST localhost:8080/admin/instruments \
  -H 'content-type: application/json' \
  -d '{"instrument_id": 7}'

# Place a limit buy
curl -s -X POST localhost:8080/api/v1/order \
  -H 'content-type: application/json' \
  -d '{
    "instrument_id": 7, "account_id": 42, "request_id": 1,
    "order_id": 1, "side": "buy", "order_type": "limit",
    "price": 100, "quantity": 10
  }'

# Top of book
curl -s 'localhost:8080/api/v1/orderbook/top?instrument_id=7'

# Full book snapshot
curl -s 'localhost:8080/api/v1/orderbook?instrument_id=7'

# Modify the order
curl -s -X POST localhost:8080/api/v1/order/modify \
  -H 'content-type: application/json' \
  -d '{
    "instrument_id": 7, "account_id": 42, "request_id": 2,
    "order_id": 1, "side": "buy", "price": 101, "quantity": 10
  }'

# Cancel it
curl -s -X DELETE \
  'localhost:8080/api/v1/order/1?instrument_id=7&account_id=42&request_id=3'

# Health
curl -s localhost:8080/health
```

## Engine

The engine is a library crate used by the gateway and benchmarks.

### Canonical command shape
```rust
use engine::commands::{Command, CommandOutput, EngineError};

let mut engine = engine::engine::Engine::new();
engine.register_instrument(1);

let result: Result<CommandOutput, EngineError> =
    engine.execute(Command::PlaceOrder { /* ... */ });
```

### Instrument lifecycle
- `Engine::register_instrument(id) -> bool` — idempotent; returns `true` on new registration.
- `Engine::is_registered(id) -> bool`.
- `Engine::execute(cmd)` returns `Err(EngineError::UnknownInstrument(id))` when routing to an unregistered book.

### Per-instrument accessors
All getters take an `InstrumentId` and return `Option`:
- `best_bid(id) -> Option<Price>`
- `best_ask(id) -> Option<Price>`
- `order_count(id) -> Option<usize>`
- `get_orderbook_state(id) -> Option<OrderbookLevelInfo>`

## Benchmarks

Run benchmarks from `benches/`.

### Criterion orderbook benchmarks
```
cd benches
cargo bench --bench orderbook
```

Reports default to `benches/bench-orderbook-report-<unix>.md`. Override with:
```
ORDERBOOK_REPORT_OUTPUT=bench-orderbook-report.md cargo bench --bench orderbook
```

### Order-ops throughput runner
```
cd benches
cargo run --release --bin order_ops -- \
  --duration-ms 5000 \
  --threads 1 \
  --seed-levels 10 \
  --seed-orders 1 \
  --seed-qty 1000000000 \
  --order-qty 1 \
  --price-step 1 \
  --output bench-operations-results.md
```

### Latest results (pre-multi-instrument baseline)
Orderbook benchmark report (`benches/bench-orderbook-report-1775567407.md`):
| Group | Benchmark | Mean (ns) | Median (ns) | Ops/sec (mean) |
| --- | --- | --- | --- | --- |
| orderbook | add_limit_order | 369.27 | 365.70 | 2708074.03 |
| orderbook | cancel_order | 361.09 | 360.44 | 2769360.98 |
| orderbook | modify_order | 404.38 | 403.43 | 2472918.04 |
| orderbook | cross_spread_match | 4853.07 | 4851.69 | 206055.17 |
| orderbook | top_of_book_read | 3.11 | 3.09 | 321213984.17 |
| orderbook | snapshot_levels | 8785.17 | 8784.37 | 113828.16 |
| engine | place_order | 357.15 | 357.74 | 2799970.88 |
| engine | get_orderbook_state | 8698.38 | 8684.44 | 114963.92 |

Order-ops throughput report (`benches/bench-operations-results-1775567441.md`):
| Metric | Value |
| --- | --- |
| Duration (ms) | 5000 |
| Warmup (ms) | 500 |
| Threads | 1 |
| Seed levels | 10 |
| Seed orders/level | 1 |
| Seed qty | 1000000000 |
| Order qty | 1 |
| Price step | 1 |
| Orders placed | 22425117 |
| Orders placed/sec | 4485023.40 |

Full reports live in `benches/`.
