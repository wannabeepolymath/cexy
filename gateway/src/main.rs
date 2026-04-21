use std::sync::Arc;

use actix_web::{App, HttpServer, web};
use app_state::AppState;
use config::GatewayConfig;
use engine::event_bus::{EventBus, LoggingConsumer};
use engine_handle::EngineHandle;
use router::Router;

mod app_state;
mod config;
mod engine_handle;
mod handlers;
mod http_models;
mod parsing;
mod router;

#[cfg(test)]
mod handlers_tests;

/// Default number of shard threads the gateway spins up when no override is
/// provided. Chosen small (2) to make concurrency behaviour easy to reason
/// about and benchmark until we have numbers that justify more.
const DEFAULT_SHARD_COUNT: u16 = 2;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let bind_addr =
        std::env::var("GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let config = match GatewayConfig::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to load gateway config: {err}");
            std::process::exit(1);
        }
    };

    // Start the event bus before the router so the router can hand its
    // sender clones to every shard at spawn time. The bus is kept alive
    // inside `AppState` for the lifetime of the HTTP server: dropping it
    // would close the event channel and stop the consumer thread.
    let event_bus = EventBus::new(LoggingConsumer);

    let router = match Router::new_with_events(DEFAULT_SHARD_COUNT, event_bus.sender()) {
        Ok(router) => router,
        Err(err) => {
            eprintln!("Failed to build router: {err}");
            std::process::exit(1);
        }
    };
    for instrument_id in &config.instruments {
        router.register_instrument(*instrument_id);
    }
    println!(
        "Gateway router: {} shard(s), {} instrument(s) registered: {:?}",
        router.shard_count(),
        config.instruments.len(),
        config.instruments
    );

    let handle: Arc<dyn EngineHandle> = Arc::new(router);
    let state = web::Data::new(AppState {
        engine: handle,
        _event_bus: Arc::new(event_bus),
    });

    println!("Gateway listening on http://{bind_addr}");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(handlers::configure)
    })
    .bind(bind_addr)?
    .run()
    .await
}
