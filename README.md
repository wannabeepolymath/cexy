# exchange-project-rs

Experimental Rust exchange components: matching engine, orderbook, API gateway,
and benchmarks.

## Layout
- `engine/` matching engine + orderbook core
- `api-gateway/` HTTP gateway (Actix Web)
- `benches/` Criterion benchmarks + order-ops throughput runner
- `planning/` design notes and research

## Benchmarks
Run benchmarks from `benches/`.

### Criterion orderbook benchmarks
```
cargo bench --bench orderbook
```

Reports default to `benches/bench-orderbook-report-<unix>.md`. Override with:
```
ORDERBOOK_REPORT_OUTPUT=bench-orderbook-report.md cargo bench --bench orderbook
```

### Order-ops throughput runner
```
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

### Latest results (sample)
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
