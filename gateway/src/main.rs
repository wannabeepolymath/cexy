use std::sync::Arc;

use actix_web::{App, HttpServer, web};
use app_state::AppState;
use config::GatewayConfig;
use engine::engine::Engine;
use engine_handle::MutexEngineHandle;

mod app_state;
mod config;
mod engine_handle;
mod handlers;
mod http_models;
mod parsing;

#[cfg(test)]
mod handlers_tests;

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

    let mut engine = Engine::new();
    for instrument_id in &config.instruments {
        engine.register_instrument(*instrument_id);
    }
    println!(
        "Gateway registered {} instrument(s): {:?}",
        config.instruments.len(),
        config.instruments
    );

    let handle: Arc<dyn engine_handle::EngineHandle> = Arc::new(MutexEngineHandle::new(engine));
    let state = web::Data::new(AppState { engine: handle });

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
