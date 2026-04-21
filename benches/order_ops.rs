use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::commands::{Command, CommandOutput, InstrumentId};
use engine::engine::Engine;
use engine::orderbook::order::Order;
use engine::orderbook::order_type::OrderType;
use engine::orderbook::side::Side;
use engine::orderbook::types::{OrderId, Price, Quantity};

const BENCH_INSTRUMENT_ID: InstrumentId = 1;

fn engine_place_order(engine: &mut Engine, order: Order) {
    match engine
        .execute(Command::PlaceOrder {
            instrument_id: BENCH_INSTRUMENT_ID,
            account_id: 1,
            request_id: 1,
            order,
        })
        .expect("registered instrument routes")
        .output
    {
        CommandOutput::PlaceOrder(result) => {
            result.expect("bench place_order must succeed");
        }
        _ => unreachable!("execute must return PlaceOrder output for PlaceOrder command"),
    }
}

const BASE_PRICE: Price = 10_000;
const DEFAULT_DURATION_MS: u64 = 5_000;
const DEFAULT_WARMUP_MS: u64 = 500;
const DEFAULT_THREADS: usize = 1;
const DEFAULT_SEED_LEVELS: usize = 10;
const DEFAULT_SEED_ORDERS_PER_LEVEL: usize = 1;
const DEFAULT_SEED_QTY: Quantity = 1_000_000_000;
const DEFAULT_ORDER_QTY: Quantity = 1;
const DEFAULT_PRICE_STEP: Price = 1;
const THREAD_ID_STRIDE: OrderId = 1_000_000_000;

#[derive(Debug, Clone)]
struct Config {
    duration: Duration,
    warmup: Duration,
    threads: usize,
    seed_levels: usize,
    seed_orders_per_level: usize,
    seed_qty: Quantity,
    order_qty: Quantity,
    price_step: Price,
    output_path: PathBuf,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let default_output = PathBuf::from(format!("bench-operations-results-{now}.md"));
        let duration_ms = parse_arg_u64(&args, "duration-ms").unwrap_or(DEFAULT_DURATION_MS);
        let warmup_ms = parse_arg_u64(&args, "warmup-ms").unwrap_or(DEFAULT_WARMUP_MS);
        let threads = parse_arg_usize(&args, "threads").unwrap_or(DEFAULT_THREADS);
        let seed_levels = parse_arg_usize(&args, "seed-levels").unwrap_or(DEFAULT_SEED_LEVELS);
        let seed_orders_per_level = parse_arg_usize(&args, "seed-orders")
            .unwrap_or(DEFAULT_SEED_ORDERS_PER_LEVEL);
        let seed_qty = parse_arg_u32(&args, "seed-qty").unwrap_or(DEFAULT_SEED_QTY);
        let order_qty = parse_arg_u32(&args, "order-qty").unwrap_or(DEFAULT_ORDER_QTY);
        let price_step = parse_arg_i32(&args, "price-step").unwrap_or(DEFAULT_PRICE_STEP);
        let output_path = parse_arg_value(&args, "output")
            .map(PathBuf::from)
            .unwrap_or(default_output);

        Self {
            duration: Duration::from_millis(duration_ms),
            warmup: Duration::from_millis(warmup_ms),
            threads: threads.max(1),
            seed_levels: seed_levels.max(1),
            seed_orders_per_level: seed_orders_per_level.max(1),
            seed_qty,
            order_qty,
            price_step,
            output_path,
        }
    }
}

fn parse_arg_value(args: &[String], key: &str) -> Option<String> {
    let flag = format!("--{key}");
    let prefix = format!("--{key}=");
    for (idx, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
        if arg == &flag {
            if let Some(value) = args.get(idx + 1) {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn parse_arg_u64(args: &[String], key: &str) -> Option<u64> {
    parse_arg_value(args, key)?.parse().ok()
}

fn parse_arg_usize(args: &[String], key: &str) -> Option<usize> {
    parse_arg_value(args, key)?.parse().ok()
}

fn parse_arg_u32(args: &[String], key: &str) -> Option<u32> {
    parse_arg_value(args, key)?.parse().ok()
}

fn parse_arg_i32(args: &[String], key: &str) -> Option<i32> {
    parse_arg_value(args, key)?.parse().ok()
}

fn make_limit_order(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
    Order::new(id, side, OrderType::GoodTillCancel, price, quantity)
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
            engine_place_order(engine, make_limit_order(next_id, side, price, quantity));
            next_id += 1;
        }
    }
    next_id
}

#[derive(Debug, Clone)]
struct Snapshot {
    best_bid: Option<Price>,
    best_ask: Option<Price>,
    spread: Option<Price>,
    mid_price: Option<f64>,
    total_orders: usize,
    bid_levels: usize,
    ask_levels: usize,
    total_bid_qty: u64,
    total_ask_qty: u64,
}

fn snapshot_engine(engine: &Engine) -> Snapshot {
    let best_bid = engine.best_bid(BENCH_INSTRUMENT_ID);
    let best_ask = engine.best_ask(BENCH_INSTRUMENT_ID);
    let spread = match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => Some(ask - bid),
        _ => None,
    };
    let mid_price = match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => Some((bid as f64 + ask as f64) / 2.0),
        _ => None,
    };
    let state = engine
        .get_orderbook_state(BENCH_INSTRUMENT_ID)
        .expect("bench instrument is registered");
    let total_bid_qty = state
        .get_bids()
        .iter()
        .map(|level| level.quantity as u64)
        .sum();
    let total_ask_qty = state
        .get_asks()
        .iter()
        .map(|level| level.quantity as u64)
        .sum();

    Snapshot {
        best_bid,
        best_ask,
        spread,
        mid_price,
        total_orders: engine.order_count(BENCH_INSTRUMENT_ID).unwrap_or(0),
        bid_levels: state.get_bids().len(),
        ask_levels: state.get_asks().len(),
        total_bid_qty,
        total_ask_qty,
    }
}

fn run_workers(engine: Arc<Mutex<Engine>>, duration: Duration, config: &Config) -> u64 {
    let mut handles = Vec::with_capacity(config.threads);
    for thread_idx in 0..config.threads {
        let engine = Arc::clone(&engine);
        let order_qty = config.order_qty;
        let price_step = config.price_step;
        handles.push(thread::spawn(move || {
            let end = Instant::now() + duration;
            let mut order_id = THREAD_ID_STRIDE * (thread_idx as OrderId + 1);
            let mut side = if thread_idx % 2 == 0 { Side::Buy } else { Side::Sell };
            let buy_price = BASE_PRICE + price_step;
            let sell_price = BASE_PRICE - price_step;
            let mut ops = 0_u64;

            while Instant::now() < end {
                let price = if side == Side::Buy { buy_price } else { sell_price };
                let order = Order::new(order_id, side, OrderType::FillAndKill, price, order_qty);
                {
                    let mut engine = engine.lock().expect("engine mutex poisoned");
                    engine_place_order(&mut engine, order);
                }
                order_id += 1;
                side = match side {
                    Side::Buy => Side::Sell,
                    Side::Sell => Side::Buy,
                };
                ops += 1;
            }
            ops
        }));
    }

    handles
        .into_iter()
        .map(|handle| handle.join().unwrap_or(0))
        .sum()
}

fn main() {
    let config = Config::from_args();
    let engine = {
        let mut engine = Engine::new();
        engine.register_instrument(BENCH_INSTRUMENT_ID);
        Arc::new(Mutex::new(engine))
    };

    let run_started_at = SystemTime::now();

    {
        let mut engine = engine.lock().expect("engine mutex poisoned");
        let next_id = seed_engine(
            &mut engine,
            1,
            Side::Buy,
            config.seed_levels,
            config.seed_orders_per_level,
            BASE_PRICE - config.price_step,
            -config.price_step,
            config.seed_qty,
        );
        seed_engine(
            &mut engine,
            next_id,
            Side::Sell,
            config.seed_levels,
            config.seed_orders_per_level,
            BASE_PRICE + config.price_step,
            config.price_step,
            config.seed_qty,
        );
    }

    let initial_snapshot = {
        let engine = engine.lock().expect("engine mutex poisoned");
        snapshot_engine(&engine)
    };

    if config.warmup > Duration::ZERO {
        let _ = run_workers(Arc::clone(&engine), config.warmup, &config);
    }

    let total_ops = run_workers(Arc::clone(&engine), config.duration, &config);
    let final_snapshot = {
        let engine = engine.lock().expect("engine mutex poisoned");
        snapshot_engine(&engine)
    };
    let ops_per_sec = total_ops as f64 / config.duration.as_secs_f64();
    let run_started_secs = run_started_at
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    println!("Orders placed: {total_ops}");
    println!("Duration (ms): {}", config.duration.as_millis());
    println!("Orders placed/sec: {:.2}", ops_per_sec);
    println!("Threads: {}", config.threads);
    println!(
        "Seed: levels={} orders_per_level={} qty={}",
        config.seed_levels, config.seed_orders_per_level, config.seed_qty
    );
    println!(
        "Order qty: {} | Price step: {}",
        config.order_qty, config.price_step
    );
    if config.threads > 1 {
        println!("Note: threads share a single engine via Mutex.");
    }

    let report = format!(
        "# Order Ops Benchmark Report\n\
Generated at (unix): {run_started_secs}\n\n\
## Configuration\n\
- Duration (ms): {}\n\
- Warmup (ms): {}\n\
- Threads: {}\n\
- Seed levels: {}\n\
- Seed orders/level: {}\n\
- Seed qty: {}\n\
- Order qty: {}\n\
- Price step: {}\n\n\
## Performance\n\
- Orders placed: {total_ops}\n\
- Orders placed/sec: {:.2}\n\n\
## Orderbook State\n\
| Metric | Initial | Final |\n\
| --- | --- | --- |\n\
| Best bid | {} | {} |\n\
| Best ask | {} | {} |\n\
| Spread | {} | {} |\n\
| Mid price | {} | {} |\n\
| Total orders | {} | {} |\n\
| Bid price levels | {} | {} |\n\
| Ask price levels | {} | {} |\n\
| Total bid qty | {} | {} |\n\
| Total ask qty | {} | {} |\n",
        config.duration.as_millis(),
        config.warmup.as_millis(),
        config.threads,
        config.seed_levels,
        config.seed_orders_per_level,
        config.seed_qty,
        config.order_qty,
        config.price_step,
        ops_per_sec,
        format_opt_price(initial_snapshot.best_bid),
        format_opt_price(final_snapshot.best_bid),
        format_opt_price(initial_snapshot.best_ask),
        format_opt_price(final_snapshot.best_ask),
        format_opt_price(initial_snapshot.spread),
        format_opt_price(final_snapshot.spread),
        format_opt_f64(initial_snapshot.mid_price),
        format_opt_f64(final_snapshot.mid_price),
        initial_snapshot.total_orders,
        final_snapshot.total_orders,
        initial_snapshot.bid_levels,
        final_snapshot.bid_levels,
        initial_snapshot.ask_levels,
        final_snapshot.ask_levels,
        initial_snapshot.total_bid_qty,
        final_snapshot.total_bid_qty,
        initial_snapshot.total_ask_qty,
        final_snapshot.total_ask_qty,
    );

    if let Err(err) = fs::write(&config.output_path, report) {
        eprintln!(
            "Failed to write report to {}: {}",
            config.output_path.display(),
            err
        );
    } else {
        println!("Report written to {}", config.output_path.display());
    }
}

fn format_opt_price(value: Option<Price>) -> String {
    match value {
        Some(price) => price.to_string(),
        None => "N/A".to_string(),
    }
}

fn format_opt_f64(value: Option<f64>) -> String {
    match value {
        Some(price) => format!("{:.2}", price),
        None => "N/A".to_string(),
    }
}
