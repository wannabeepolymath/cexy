use criterion::{black_box, BatchSize, Criterion};
use engine::engine::Engine;
use engine::orderbook::order::Order;
use engine::orderbook::order_modify::OrderModify;
use engine::orderbook::order_type::OrderType;
use engine::orderbook::orderbook::Orderbook;
use engine::orderbook::side::Side;
use engine::orderbook::types::{OrderId, Price, Quantity};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_PRICE: Price = 10_000;
const PRICE_STEP: Price = 1;
const ORDER_QTY: Quantity = 10;

const ORDERBOOK_BENCH_SPECS: &[BenchSpec] = &[
    BenchSpec {
        group: "orderbook",
        name: "add_limit_order",
    },
    BenchSpec {
        group: "orderbook",
        name: "cancel_order",
    },
    BenchSpec {
        group: "orderbook",
        name: "modify_order",
    },
    BenchSpec {
        group: "orderbook",
        name: "cross_spread_match",
    },
    BenchSpec {
        group: "orderbook",
        name: "top_of_book_read",
    },
    BenchSpec {
        group: "orderbook",
        name: "snapshot_levels",
    },
    BenchSpec {
        group: "engine",
        name: "place_order",
    },
    BenchSpec {
        group: "engine",
        name: "get_orderbook_state",
    },
];

#[derive(Clone, Copy)]
struct BenchSpec {
    group: &'static str,
    name: &'static str,
}

#[derive(Deserialize)]
struct EstimatesFile {
    mean: EstimateStats,
    median: EstimateStats,
}

#[derive(Deserialize)]
struct EstimateStats {
    point_estimate: f64,
}

fn make_limit_order(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
    Order::new(id, side, OrderType::GoodTillCancel, price, quantity)
}

fn seed_book(
    book: &mut Orderbook,
    mut next_id: OrderId,
    side: Side,
    levels: usize,
    orders_per_level: usize,
    start_price: Price,
    price_step: Price,
    quantity: Quantity,
) -> OrderId {
    for level in 0..levels {
        let price = start_price + price_step * level as Price;
        for _ in 0..orders_per_level {
            book
                .add_order(make_limit_order(next_id, side, price, quantity))
                .unwrap();
            next_id += 1;
        }
    }
    next_id
}

fn seed_engine(
    engine: &mut Engine,
    mut next_id: OrderId,
    side: Side,
    levels: usize,
    orders_per_level: usize,
    start_price: Price,
    price_step: Price,
    quantity: Quantity,
) -> OrderId {
    for level in 0..levels {
        let price = start_price + price_step * level as Price;
        for _ in 0..orders_per_level {
            engine
                .place_order(make_limit_order(next_id, side, price, quantity))
                .unwrap();
            next_id += 1;
        }
    }
    next_id
}

fn bench_orderbook_add_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook");
    group.bench_function("add_limit_order", |b| {
        b.iter_batched(
            || {
                let mut book = Orderbook::new();
                let next_id = seed_book(
                    &mut book,
                    1,
                    Side::Buy,
                    20,
                    5,
                    BASE_PRICE,
                    -PRICE_STEP,
                    ORDER_QTY,
                );
                (book, next_id)
            },
            |(mut book, order_id)| {
                let order = make_limit_order(order_id, Side::Buy, BASE_PRICE - 25, ORDER_QTY);
                black_box(book.add_order(order).unwrap());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_orderbook_cancel(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook");
    group.bench_function("cancel_order", |b| {
        b.iter_batched(
            || {
                let mut book = Orderbook::new();
                let target_id = 1;
                book.add_order(make_limit_order(
                    target_id,
                    Side::Buy,
                    BASE_PRICE,
                    ORDER_QTY,
                ))
                .unwrap();
                seed_book(
                    &mut book,
                    target_id + 1,
                    Side::Buy,
                    20,
                    5,
                    BASE_PRICE - 1,
                    -PRICE_STEP,
                    ORDER_QTY,
                );
                (book, target_id)
            },
            |(mut book, target_id)| {
                black_box(book.cancel_order(target_id));
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_orderbook_modify(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook");
    group.bench_function("modify_order", |b| {
        b.iter_batched(
            || {
                let mut book = Orderbook::new();
                let target_id = 1;
                book.add_order(make_limit_order(
                    target_id,
                    Side::Buy,
                    BASE_PRICE,
                    ORDER_QTY,
                ))
                .unwrap();
                seed_book(
                    &mut book,
                    target_id + 1,
                    Side::Buy,
                    20,
                    5,
                    BASE_PRICE - 1,
                    -PRICE_STEP,
                    ORDER_QTY,
                );
                let modify = OrderModify::new(
                    target_id,
                    Side::Buy,
                    BASE_PRICE - 5,
                    ORDER_QTY + 5,
                );
                (book, modify)
            },
            |(mut book, modify)| {
                black_box(book.modify_order(modify).unwrap());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_orderbook_cross_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook");
    group.bench_function("cross_spread_match", |b| {
        b.iter_batched(
            || {
                let mut book = Orderbook::new();
                let next_id = seed_book(
                    &mut book,
                    1,
                    Side::Sell,
                    10,
                    10,
                    BASE_PRICE + 1,
                    PRICE_STEP,
                    ORDER_QTY,
                );
                let aggressive = make_limit_order(
                    next_id,
                    Side::Buy,
                    BASE_PRICE + 10,
                    ORDER_QTY * 50,
                );
                (book, aggressive)
            },
            |(mut book, aggressive)| {
                black_box(book.add_order(aggressive).unwrap());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_orderbook_top_of_book(c: &mut Criterion) {
    let mut book = Orderbook::new();
    let next_id = seed_book(
        &mut book,
        1,
        Side::Buy,
        50,
        5,
        BASE_PRICE,
        -PRICE_STEP,
        ORDER_QTY,
    );
    seed_book(
        &mut book,
        next_id,
        Side::Sell,
        50,
        5,
        BASE_PRICE + 1,
        PRICE_STEP,
        ORDER_QTY,
    );

    let mut group = c.benchmark_group("orderbook");
    group.bench_function("top_of_book_read", |b| {
        b.iter(|| {
            black_box(book.best_bid());
            black_box(book.best_ask());
        });
    });
    group.finish();
}

fn bench_orderbook_snapshot(c: &mut Criterion) {
    let mut book = Orderbook::new();
    let next_id = seed_book(
        &mut book,
        1,
        Side::Buy,
        80,
        8,
        BASE_PRICE,
        -PRICE_STEP,
        ORDER_QTY,
    );
    seed_book(
        &mut book,
        next_id,
        Side::Sell,
        80,
        8,
        BASE_PRICE + 1,
        PRICE_STEP,
        ORDER_QTY,
    );

    let mut group = c.benchmark_group("orderbook");
    group.bench_function("snapshot_levels", |b| {
        b.iter(|| {
            black_box(book.get_order_infos());
        });
    });
    group.finish();
}

fn bench_engine_place_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine");
    group.bench_function("place_order", |b| {
        b.iter_batched(
            || {
                let mut engine = Engine::new();
                let next_id = seed_engine(
                    &mut engine,
                    1,
                    Side::Buy,
                    20,
                    5,
                    BASE_PRICE,
                    -PRICE_STEP,
                    ORDER_QTY,
                );
                let order = make_limit_order(next_id, Side::Buy, BASE_PRICE - 25, ORDER_QTY);
                (engine, order)
            },
            |(mut engine, order)| {
                black_box(engine.place_order(order).unwrap());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_engine_get_state(c: &mut Criterion) {
    let mut engine = Engine::new();
    let next_id = seed_engine(
        &mut engine,
        1,
        Side::Buy,
        80,
        8,
        BASE_PRICE,
        -PRICE_STEP,
        ORDER_QTY,
    );
    seed_engine(
        &mut engine,
        next_id,
        Side::Sell,
        80,
        8,
        BASE_PRICE + 1,
        PRICE_STEP,
        ORDER_QTY,
    );

    let mut group = c.benchmark_group("engine");
    group.bench_function("get_orderbook_state", |b| {
        b.iter(|| {
            black_box(engine.get_orderbook_state());
        });
    });
    group.finish();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_orderbook_add_limit(&mut c);
    bench_orderbook_cancel(&mut c);
    bench_orderbook_modify(&mut c);
    bench_orderbook_cross_spread(&mut c);
    bench_orderbook_top_of_book(&mut c);
    bench_orderbook_snapshot(&mut c);
    bench_engine_place_order(&mut c);
    bench_engine_get_state(&mut c);
    c.final_summary();

    let output_path = report_output_path();
    if let Err(err) = write_report(&output_path) {
        eprintln!("Failed to write benchmark report: {err}");
    } else {
        println!("Benchmark report written to {}", output_path.display());
    }
}

fn report_output_path() -> PathBuf {
    if let Ok(path) = env::var("ORDERBOOK_REPORT_OUTPUT") {
        return PathBuf::from(path);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    PathBuf::from(format!("bench-orderbook-report-{now}.md"))
}

fn write_report(output_path: &Path) -> Result<(), String> {
    let target_dir = target_dir();
    let criterion_dir = target_dir.join("criterion");
    let generated = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut rows = String::new();
    for spec in ORDERBOOK_BENCH_SPECS {
        if let Some(estimates) = load_estimates(&criterion_dir, spec) {
            let mean_sec = estimates.mean.point_estimate;
            let median_sec = estimates.median.point_estimate;
            rows.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                spec.group,
                spec.name,
                format_ns(mean_sec),
                format_ns(median_sec),
                format_ops(mean_sec)
            ));
        } else {
            rows.push_str(&format!(
                "| {} | {} | N/A | N/A | N/A |\n",
                spec.group, spec.name
            ));
        }
    }

    let report = format!(
        "# Orderbook Benchmark Report\n\
Generated at (unix): {generated}\n\
Target dir: `{}`\n\
Source: `{}`\n\n\
## Benchmarks\n\
| Group | Benchmark | Mean (ns) | Median (ns) | Ops/sec (mean) |\n\
| --- | --- | --- | --- | --- |\n\
{rows}",
        target_dir.display(),
        criterion_dir.display(),
    );

    fs::write(output_path, report).map_err(|err| err.to_string())
}

fn target_dir() -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target"))
}

fn load_estimates(criterion_dir: &Path, spec: &BenchSpec) -> Option<EstimatesFile> {
    let path = criterion_dir
        .join(spec.group)
        .join(spec.name)
        .join("new")
        .join("estimates.json");
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn format_ns(nanoseconds: f64) -> String {
    if nanoseconds.is_finite() && nanoseconds > 0.0 {
        format!("{:.2}", nanoseconds)
    } else {
        "N/A".to_string()
    }
}

fn format_ops(nanoseconds: f64) -> String {
    if nanoseconds.is_finite() && nanoseconds > 0.0 {
        format!("{:.2}", 1_000_000_000.0 / nanoseconds)
    } else {
        "N/A".to_string()
    }
}
