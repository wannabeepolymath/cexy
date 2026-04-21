# Orderbook Benchmark Report
Generated at (unix): 1776722091
Target dir: `/var/folders/vp/_gvcxwbx3rg9cz0193_n4kr00000gn/T/cursor-sandbox-cache/b76dda3eb1a50496db968cbd4a81ab7d/cargo-target`
Source: `/var/folders/vp/_gvcxwbx3rg9cz0193_n4kr00000gn/T/cursor-sandbox-cache/b76dda3eb1a50496db968cbd4a81ab7d/cargo-target/criterion`

## Benchmarks
| Group | Benchmark | Mean (ns) | Median (ns) | Ops/sec (mean) |
| --- | --- | --- | --- | --- |
| orderbook | add_limit_order | 443.81 | 430.88 | 2253238.69 |
| orderbook | cancel_order | 488.74 | 442.68 | 2046063.52 |
| orderbook | modify_order | 498.41 | 478.92 | 2006380.13 |
| orderbook | cross_spread_match | 5342.97 | 5343.78 | 187161.85 |
| orderbook | top_of_book_read | 2.01 | 2.01 | 496715080.87 |
| orderbook | snapshot_levels | 8519.00 | 8505.54 | 117384.67 |
| engine | place_order | 464.67 | 461.96 | 2152081.98 |
| engine | get_orderbook_state | 8766.14 | 8707.70 | 114075.25 |
