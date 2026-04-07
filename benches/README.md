# Benchmarks

This folder contains the orderbook benchmarks and the order-ops throughput runner.

## Requirements
- Rust toolchain (`cargo`)

## Run the Criterion benchmarks
From `benches/`:

```
cargo bench --bench orderbook
```

Report output:
- Default: `bench-orderbook-report-<unix>.md` in `benches/`
- Custom: set `ORDERBOOK_REPORT_OUTPUT`

```
ORDERBOOK_REPORT_OUTPUT=bench-orderbook-report.md cargo bench --bench orderbook
```

## Run the order-ops throughput runner
From `benches/`:

```
cargo run --release --bin order_ops
```

Optional flags:
- `--duration-ms` (default: 5000)
- `--warmup-ms` (default: 500)
- `--threads` (default: 1)
- `--seed-levels` (default: 10)
- `--seed-orders` (default: 1)
- `--seed-qty` (default: 1000000000)
- `--order-qty` (default: 1)
- `--price-step` (default: 1)
- `--output` (default: `bench-operations-results-<unix>.md`)

Example:

```
cargo run --release --bin order_ops -- \
  --duration-ms 5000 \
  --threads 4 \
  --seed-levels 10 \
  --seed-orders 1 \
  --seed-qty 1000000000 \
  --order-qty 1 \
  --price-step 1 \
  --output bench-operations-results.md
```
