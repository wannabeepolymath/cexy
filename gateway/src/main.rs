use actix_web::{web, App, HttpServer};
use engine::engine::Engine;
use std::sync::Mutex;

mod app_state;
mod handlers;
mod http_models;
mod parsing;

use app_state::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let bind_addr =
        std::env::var("GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let state = web::Data::new(AppState {
        engine: Mutex::new(Engine::new()),
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
