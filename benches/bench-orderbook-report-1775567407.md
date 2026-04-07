# Orderbook Benchmark Report
Generated at (unix): 1775567407
Target dir: `target`
Source: `target/criterion`

## Benchmarks
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
