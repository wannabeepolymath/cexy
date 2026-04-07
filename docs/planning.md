# Planning: centralized exchange

This document describes a practical architecture for a high-performance centralized exchange (CEX).
The main design goal is simple:

`keep the matching path tiny, deterministic, in-memory, and replayable`

The matching engine should not block on databases, remote RPC, or ad hoc market-data formatting.

---

## Architecture summary

Mental model:

`client ingress -> auth/session -> pre-trade risk -> shard router -> matching engine -> durable journal -> async projections -> private/public delivery`

### Control plane vs data plane

- **Control plane**: users, instruments, fees, permissions, KYC/compliance config, historical queries.
- **Data plane**: order validation, balance reservation, matching, journaling, market-data fanout, execution reports.

The control plane can use regular databases and service boundaries.
The data plane should stay memory-first and append-only.

### Core rule

Do not put synchronous Postgres, Redis, or cross-service RPC in the per-order hot path.

---

## End-to-end flow

| Stage | Responsibility |
|------|------|
| **Gateway** | Accept REST / WebSocket / later FIX. Parse external requests into canonical commands such as `NewOrder`, `CancelOrder`, `ModifyOrder`. |
| **Session / auth** | Authenticate client, bind `account_id`, validate nonce / sequence, enforce rate limits and cancel-on-disconnect semantics. |
| **Pre-trade risk** | Use in-memory state to validate balances, margin, instrument constraints, price bands, position limits, STP, and kill switches. |
| **Router** | Route each command to exactly one engine shard, usually by `instrument_id` or market symbol. |
| **Matching engine** | Single writer per shard/book. Mutate the order book, update reserved funds / positions, and emit typed domain events. |
| **Journal** | Append every accepted command and resulting events to an ordered replayable log. |
| **Projection services** | Build user order history, balances, positions, fees, candles, surveillance streams, and analytics from the journal/event stream. |
| **Private delivery** | Send execution reports, rejects, cancels, and fills back to the originating user. |
| **Public delivery** | Publish trades, top-of-book, depth, and later richer feeds to external clients. |

### Recommended order of operations

For each incoming command:

1. Parse and authenticate.
2. Run pre-trade risk against cached state.
3. Assign or validate shard-local sequence ordering.
4. Mutate engine state.
5. Append command + results to the journal.
6. Acknowledge the client and publish downstream events.

The exact ordering of `ack` vs `journal append` depends on the durability target, but the rule must be explicit and consistent.

---

## Matching engine principles

- **Single writer per shard**: one thread or event loop owns each book partition to avoid lock contention.
- **Deterministic state transitions**: identical command stream should rebuild the same final state on replay.
- **Typed commands and events**: business logic should use structured commands/events, not loosely typed JSON blobs.
- **No side effects in the core loop**: the matcher should not directly send WebSocket frames, write SQL rows, or call third-party services.
- **Fast cancel path**: cancel latency matters as much as placement latency.
- **Backpressure isolation**: slow downstream consumers must never stall matching.

### Engine outputs

The engine should emit domain events such as:

- `OrderAccepted`
- `OrderRejected`
- `OrderCanceled`
- `OrderModified`
- `TradeExecuted`
- `BookTopUpdated`
- `DepthUpdated`

Downstream systems subscribe to these events instead of coupling themselves to engine internals.

---

## Sequencing, durability, and recovery

This is the part that makes an exchange trustworthy, not just fast.

### Sequencing

- Commands need an unambiguous processing order.
- Sequence numbers should be at least shard-local and monotonic.
- Market-data streams should include `sequence` so consumers can detect gaps.

### Journal

The journal is the source of truth for:

- crash recovery
- auditability
- deterministic replay
- downstream projection rebuilds

Journal properties:

- append-only
- ordered
- partitioned by shard or instrument group
- cheap sequential writes
- replayable from snapshots + delta log

### Recovery

Recovery should look like:

1. Load latest snapshot for the shard.
2. Replay journal entries after the snapshot.
3. Rebuild order books, reserved balances, and sequence counters.
4. Resume processing from the next valid sequence.

Snapshots are an optimization; the journal remains authoritative.

---

## Market data distribution

Market data must be produced downstream of the matching engine, not inside the book's inner loop as JSON or ad hoc I/O.

### Internal distribution options

| Path | Use |
|------|-----|
| **Shared memory (`mmap`) + SPSC ring** | Lowest latency on one host for colocated consumers and benchmark harnesses. |
| **TCP loopback (`127.0.0.1`)** | Better process isolation, easier framing/backpressure, simpler path for components that may later move off-host. |

A dedicated market-data publisher should receive canonical quote/trade events from the engine or from journal replay, then:

1. Write a fixed binary layout to the ring buffer.
2. Publish the same logical update over loopback TCP.

### Internal wire format

Prefer a compact binary message with fields like:

- `instrument_id`
- `sequence`
- `timestamp_ns`
- `best_bid_ticks`
- `best_ask_ticks`
- `last_trade_price_ticks`
- `last_trade_qty_lots`

JSON is acceptable for debugging or external APIs, but not for internal low-latency fanout.

Example external/debug quote:

```json
{
  "instrument": "SOL_USDC",
  "bid": 2850.25,
  "ask": 2850.75,
  "sequence": 9123456,
  "timestamp_ns": 1234567890123
}
```

### Slow-consumer rule

If a market-data consumer falls behind, the publisher must drop, disconnect, or resync that consumer without blocking the engine.

---

## Data model guidance

Use human-readable forms at the API boundary, but use compact numeric representations in the engine.

### Prefer in the engine

- `instrument_id: u32` instead of symbol strings
- integer `price_ticks` instead of arbitrary decimals
- integer `qty_lots` instead of arbitrary decimals
- compact enums for side, time-in-force, and order type
- monotonic sequence counters

### Avoid in the hot path where possible

- repeated `String` cloning
- high-allocation message formats
- generic decimal math if integer ticks/lots are enough
- per-event JSON serialization

This keeps memory layout tighter, reduces allocations, and makes latency more predictable.

---

## Balance and risk model

Before an order is accepted:

1. Validate instrument rules.
2. Validate account state from in-memory risk state.
3. Reserve or lock required funds / margin.
4. Reject immediately if insufficient.

Important distinction:

- **Hot-path tradable state**: what the engine/risk layer uses to decide if an order can be accepted now.
- **Async ledger/reporting state**: user-visible history, statements, accounting projections, and reconciled balances.

The hot-path state must be local and deterministic.
The async ledger can be built from events.

---

## Order types roadmap

Implement order types in tiers so the engine stays small and correct first.

### Tier 1

- Limit / GTC
- Market
- Cancel
- Modify / replace

### Tier 2

- IOC / FAK
- FOK
- Post-only

### Tier 3

- Stop-market
- Stop-limit
- Reduce-only
- Iceberg
- Good-for-day or session-bounded orders

Notes:

- `Post-only` needs explicit maker-only rejection logic.
- `Stop-*` orders need trigger semantics separate from resting book logic.
- `Iceberg` orders need visible vs hidden quantity rules.
- `Reduce-only` matters mainly once leveraged products / derivatives exist.

---

## Suggested component boundaries

### Gateway

Responsibilities:

- REST/WebSocket/FIX sessions
- request parsing
- authentication and throttling
- translating external requests into internal commands

It should not own matching or authoritative balance state.

### Router

Responsibilities:

- shard lookup for `instrument_id`
- command forwarding to the correct engine shard
- minimal routing metadata

It is not the public HTTP API itself.

### Engine shard

Responsibilities:

- order book ownership
- reserved balance / margin state relevant to that shard
- command processing
- event emission
- snapshot / replay support

### Projection services

Examples:

- user order history
- balances and statements
- fees
- positions / PnL
- candles / klines
- surveillance
- analytics

These can use Postgres, object storage, OLAP systems, or caches because they are not on the critical matching path.

---

## Persistence notes

### Postgres

Good fit for:

- user profiles
- instruments and metadata
- permissions and fee schedules
- historical order query APIs
- accounting/reporting tables
- kline/materialized query data

Not a good fit for per-order synchronous matching decisions.

### Redis

Optional fit for:

- external cache
- session data
- non-critical pub/sub
- rate-limit counters

Do not make Redis the source of truth or mandatory hop for the matching hot path.

---

## Benchmarking plan

Benchmark at multiple levels.

### 1. Engine microbenchmarks

Measure isolated operations:

- place limit order
- cancel order
- modify order
- cross spread and match
- top-of-book read
- snapshot generation

Tooling:

- `cargo bench`
- `criterion`

### 2. Component benchmarks

Measure:

- gateway parse/validate throughput
- engine command throughput with in-memory queue
- journal append throughput
- market-data publisher throughput

### 3. End-to-end benchmarks

Measure full client-visible paths:

- submit order -> ack
- submit order -> fill
- submit order -> market-data update observed
- cancel -> cancel confirmation

### 4. Stress / soak tests

Run:

- cancel storms
- one hot symbol
- burst after idle
- slow consumer scenarios
- long-running load
- restart and replay timing

Always record:

- throughput
- p50 / p95 / p99 / p99.9
- CPU
- memory
- queue depths
- dropped/disconnected consumers

---

## Minimal internal structs (conceptual)

These are conceptual shapes, not final implementation choices.

```rust
struct Command {
    sequence: u64,
    account_id: u64,
    instrument_id: u32,
    payload: CommandPayload,
    timestamp_ns: u64,
}

enum CommandPayload {
    NewOrder(NewOrder),
    CancelOrder { order_id: u64 },
    ModifyOrder(ModifyOrder),
}

struct EngineShard {
    books: HashMap<u32, OrderBook>,
    risk_state: HashMap<u64, AccountRiskState>,
    next_sequence: u64,
}
```

The real implementation should prefer compact types and allocation-aware structures.

---

## Current implementation focus

Start with:

1. one process
2. one engine shard
3. one or a few instruments
4. deterministic journal + replay
5. basic order types only

Then add:

- snapshots
- richer market data
- multiple shards
- more order types
- external gateway scaling

Correctness and replayability come before distribution complexity.
