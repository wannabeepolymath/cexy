use actix_web::{web, App, HttpServer};
use engine::engine::Engine;
use std::sync::Mutex;

mod app_state;
mod handlers;
mod http_models;
mod parsing;

#[cfg(test)]
mod handlers_tests;

use app_state::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let bind_addr =
        std::env::var("GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    // TODO(phase-1/step-6): load the registered instrument set from config.
    // For now we boot with a single default instrument so the gateway is usable.
    let mut engine = Engine::new();
    engine.register_instrument(1);

    let state = web::Data::new(AppState {
        engine: Mutex::new(engine),
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
