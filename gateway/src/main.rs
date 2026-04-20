use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use engine::commands::{CancelOrderResult, ModifyOrderReject};
use engine::engine::Engine;
use engine::orderbook::order::Order;
use engine::orderbook::order_modify::OrderModify;
use engine::orderbook::order_type::OrderType;
use engine::orderbook::side::Side;
use engine::orderbook::types::{OrderId, Price, Quantity};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

struct AppState {
    engine: Mutex<Engine>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct CreateOrderRequest {
    order_id: OrderId,
    side: String,
    order_type: String,
    price: Option<Price>,
    quantity: Quantity,
}

#[derive(Deserialize)]
struct ModifyOrderRequest {
    order_id: OrderId,
    side: String,
    price: Price,
    quantity: Quantity,
}

#[derive(Serialize)]
struct OrderResult {
    trades: usize,
    best_bid: Option<Price>,
    best_ask: Option<Price>,
}

#[derive(Serialize)]
struct TopOfBookResponse {
    best_bid: Option<Price>,
    best_ask: Option<Price>,
}

#[derive(Serialize)]
struct Level {
    price: Price,
    quantity: Quantity,
}

#[derive(Serialize)]
struct OrderbookResponse {
    bids: Vec<Level>,
    asks: Vec<Level>,
}

fn parse_side(input: &str) -> Option<Side> {
    match input.trim().to_lowercase().as_str() {
        "buy" => Some(Side::Buy),
        "sell" => Some(Side::Sell),
        _ => None,
    }
}

fn parse_order_type(input: &str) -> Option<OrderType> {
    match input.trim().to_lowercase().as_str() {
        "market" => Some(OrderType::Market),
        "limit" | "gtc" | "goodtillcancel" | "good_till_cancel" => Some(OrderType::GoodTillCancel),
        "fak" | "ioc" | "fillandkill" | "fill_and_kill" => Some(OrderType::FillAndKill),
        "fok" | "fillorkill" | "fill_or_kill" => Some(OrderType::FillOrKill),
        "postonly" | "post_only" | "post-only" => Some(OrderType::PostOnly),
        _ => None,
    }
}

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ErrorResponse {
        error: message.to_string(),
    })
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse { status: "ok" })
}

async fn create_order(
    state: web::Data<AppState>,
    payload: web::Json<CreateOrderRequest>,
) -> impl Responder {
    let side = match parse_side(&payload.side) {
        Some(side) => side,
        None => return bad_request("invalid side, expected: buy | sell"),
    };

    let order_type = match parse_order_type(&payload.order_type) {
        Some(order_type) => order_type,
        None => {
            return bad_request(
                "invalid order_type, expected: market | limit | gtc | fok | fak | post_only",
            )
        }
    };

    if payload.quantity == 0 {
        return bad_request("quantity must be greater than 0");
    }

    let order = if order_type == OrderType::Market {
        Order::market(payload.order_id, side, order_type, payload.quantity)
    } else {
        let price = match payload.price {
            Some(price) => price,
            None => return bad_request("price is required for non-market orders"),
        };
        Order::new(
            payload.order_id,
            side,
            order_type,
            price,
            payload.quantity,
        )
    };

    let mut engine = state.engine.lock().unwrap();
    match engine.place_order(order) {
        Ok(success) => HttpResponse::Ok().json(OrderResult {
            trades: success.trades.len(),
            best_bid: engine.best_bid(),
            best_ask: engine.best_ask(),
        }),
        Err(reject) => HttpResponse::BadRequest().json(ErrorResponse {
            error: reject.to_string(),
        }),
    }
}

async fn modify_order(
    state: web::Data<AppState>,
    payload: web::Json<ModifyOrderRequest>,
) -> impl Responder {
    let side = match parse_side(&payload.side) {
        Some(side) => side,
        None => return bad_request("invalid side, expected: buy | sell"),
    };

    if payload.quantity == 0 {
        return bad_request("quantity must be greater than 0");
    }

    let modify = OrderModify::new(payload.order_id, side, payload.price, payload.quantity);
    let mut engine = state.engine.lock().unwrap();
    match engine.modify_order(modify) {
        Ok(success) => HttpResponse::Ok().json(OrderResult {
            trades: success.trades.len(),
            best_bid: engine.best_bid(),
            best_ask: engine.best_ask(),
        }),
        Err(ModifyOrderReject::OrderNotFound) => {
            HttpResponse::NotFound().json(ErrorResponse {
                error: "order not found".to_string(),
            })
        }
        Err(ModifyOrderReject::PlaceRejected(e)) => HttpResponse::BadRequest().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

async fn cancel_order(state: web::Data<AppState>, path: web::Path<OrderId>) -> impl Responder {
    let order_id = path.into_inner();
    let mut engine = state.engine.lock().unwrap();
    match engine.cancel_order(order_id) {
        CancelOrderResult::Cancelled => HttpResponse::Ok().json(TopOfBookResponse {
            best_bid: engine.best_bid(),
            best_ask: engine.best_ask(),
        }),
        CancelOrderResult::NotFound => HttpResponse::NotFound().json(ErrorResponse {
            error: "order not found".to_string(),
        }),
    }
}

async fn orderbook(state: web::Data<AppState>) -> impl Responder {
    let engine = state.engine.lock().unwrap();
    let info = engine.get_orderbook_state();
    let bids = info
        .get_bids()
        .iter()
        .map(|level| Level {
            price: level.price,
            quantity: level.quantity,
        })
        .collect();
    let asks = info
        .get_asks()
        .iter()
        .map(|level| Level {
            price: level.price,
            quantity: level.quantity,
        })
        .collect();
    HttpResponse::Ok().json(OrderbookResponse { bids, asks })
}

async fn top_of_book(state: web::Data<AppState>) -> impl Responder {
    let engine = state.engine.lock().unwrap();
    HttpResponse::Ok().json(TopOfBookResponse {
        best_bid: engine.best_bid(),
        best_ask: engine.best_ask(),
    })
}

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
            .route("/health", web::get().to(health))
            .service(
                web::scope("/api/v1")
                    .route("/order", web::post().to(create_order))
                    .route("/order/modify", web::post().to(modify_order))
                    .route("/order/{order_id}", web::delete().to(cancel_order))
                    .route("/orderbook", web::get().to(orderbook))
                    .route("/orderbook/top", web::get().to(top_of_book)),
            )
    })
    .bind(bind_addr)?
    .run()
    .await
}
