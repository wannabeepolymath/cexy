use actix_web::{web, HttpResponse, Responder};
use engine::commands::{
    CancelOrderResult, Command, CommandOutput, EngineError, ModifyOrderReject,
};
use engine::orderbook::order::Order;
use engine::orderbook::order_modify::OrderModify;
use engine::orderbook::order_type::OrderType;
use engine::orderbook::types::OrderId;

use crate::app_state::AppState;
use crate::http_models::{
    CancelOrderQuery, CreateOrderRequest, ErrorResponse, HealthResponse, InstrumentQuery, Level,
    ModifyOrderRequest, OrderResult, OrderbookResponse, TopOfBookResponse,
};
use crate::parsing::{parse_order_type, parse_side, validate_request_identity};

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ErrorResponse {
        error: message.to_string(),
    })
}

fn unknown_instrument(instrument_id: u32) -> HttpResponse {
    HttpResponse::NotFound().json(ErrorResponse {
        error: format!("unknown instrument: {instrument_id}"),
    })
}

pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse { status: "ok" })
}

pub async fn create_order(
    state: web::Data<AppState>,
    payload: web::Json<CreateOrderRequest>,
) -> impl Responder {
    if let Some(message) = validate_request_identity(
        payload.instrument_id,
        payload.account_id,
        payload.request_id,
    ) {
        return bad_request(message);
    }

    let side = match parse_side(&payload.side) {
        Some(side) => side,
        None => return bad_request("invalid side, expected: buy | sell"),
    };

    let order_type = match parse_order_type(&payload.order_type) {
        Some(order_type) => order_type,
        None => {
            return bad_request(
                "invalid order_type, expected: market | limit | gtc | fok | fak | post_only",
            );
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
    let instrument_id = payload.instrument_id;
    let result = engine.execute(Command::PlaceOrder {
        instrument_id,
        account_id: payload.account_id,
        request_id: payload.request_id,
        order,
    });
    match result {
        Ok(CommandOutput::PlaceOrder(Ok(success))) => HttpResponse::Ok().json(OrderResult {
            trades: success.trades.len(),
            best_bid: engine.best_bid(instrument_id),
            best_ask: engine.best_ask(instrument_id),
        }),
        Ok(CommandOutput::PlaceOrder(Err(reject))) => HttpResponse::BadRequest().json(ErrorResponse {
            error: reject.to_string(),
        }),
        Ok(_) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: "unexpected command output".to_string(),
        }),
        Err(EngineError::UnknownInstrument(id)) => unknown_instrument(id),
    }
}

pub async fn modify_order(
    state: web::Data<AppState>,
    payload: web::Json<ModifyOrderRequest>,
) -> impl Responder {
    if let Some(message) = validate_request_identity(
        payload.instrument_id,
        payload.account_id,
        payload.request_id,
    ) {
        return bad_request(message);
    }

    let side = match parse_side(&payload.side) {
        Some(side) => side,
        None => return bad_request("invalid side, expected: buy | sell"),
    };

    if payload.quantity == 0 {
        return bad_request("quantity must be greater than 0");
    }

    let modify = OrderModify::new(payload.order_id, side, payload.price, payload.quantity);
    let mut engine = state.engine.lock().unwrap();
    let instrument_id = payload.instrument_id;
    let result = engine.execute(Command::ModifyOrder {
        instrument_id,
        account_id: payload.account_id,
        request_id: payload.request_id,
        modify,
    });
    match result {
        Ok(CommandOutput::ModifyOrder(Ok(success))) => HttpResponse::Ok().json(OrderResult {
            trades: success.trades.len(),
            best_bid: engine.best_bid(instrument_id),
            best_ask: engine.best_ask(instrument_id),
        }),
        Ok(CommandOutput::ModifyOrder(Err(ModifyOrderReject::OrderNotFound))) => {
            HttpResponse::NotFound().json(ErrorResponse {
                error: "order not found".to_string(),
            })
        }
        Ok(CommandOutput::ModifyOrder(Err(ModifyOrderReject::SideChangeNotAllowed))) => {
            HttpResponse::BadRequest().json(ErrorResponse {
                error: "side change not allowed on modify".to_string(),
            })
        }
        Ok(CommandOutput::ModifyOrder(Err(ModifyOrderReject::PlaceRejected(e)))) => {
            HttpResponse::BadRequest().json(ErrorResponse {
                error: e.to_string(),
            })
        }
        Ok(_) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: "unexpected command output".to_string(),
        }),
        Err(EngineError::UnknownInstrument(id)) => unknown_instrument(id),
    }
}

pub async fn cancel_order(
    state: web::Data<AppState>,
    path: web::Path<OrderId>,
    query: web::Query<CancelOrderQuery>,
) -> impl Responder {
    let order_id = path.into_inner();
    if let Some(message) =
        validate_request_identity(query.instrument_id, query.account_id, query.request_id)
    {
        return bad_request(message);
    }

    let mut engine = state.engine.lock().unwrap();
    let instrument_id = query.instrument_id;
    let result = engine.execute(Command::CancelOrder {
        instrument_id,
        account_id: query.account_id,
        request_id: query.request_id,
        order_id,
    });
    match result {
        Ok(CommandOutput::CancelOrder(CancelOrderResult::Cancelled)) => {
            HttpResponse::Ok().json(TopOfBookResponse {
                best_bid: engine.best_bid(instrument_id),
                best_ask: engine.best_ask(instrument_id),
            })
        }
        Ok(CommandOutput::CancelOrder(CancelOrderResult::NotFound)) => {
            HttpResponse::NotFound().json(ErrorResponse {
                error: "order not found".to_string(),
            })
        }
        Ok(_) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: "unexpected command output".to_string(),
        }),
        Err(EngineError::UnknownInstrument(id)) => unknown_instrument(id),
    }
}

pub async fn orderbook(
    state: web::Data<AppState>,
    query: web::Query<InstrumentQuery>,
) -> impl Responder {
    if query.instrument_id == 0 {
        return bad_request("instrument_id must be greater than 0");
    }
    let engine = state.engine.lock().unwrap();
    let Some(info) = engine.get_orderbook_state(query.instrument_id) else {
        return unknown_instrument(query.instrument_id);
    };
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

pub async fn top_of_book(
    state: web::Data<AppState>,
    query: web::Query<InstrumentQuery>,
) -> impl Responder {
    if query.instrument_id == 0 {
        return bad_request("instrument_id must be greater than 0");
    }
    let engine = state.engine.lock().unwrap();
    if !engine.is_registered(query.instrument_id) {
        return unknown_instrument(query.instrument_id);
    }
    HttpResponse::Ok().json(TopOfBookResponse {
        best_bid: engine.best_bid(query.instrument_id),
        best_ask: engine.best_ask(query.instrument_id),
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health)).service(
        web::scope("/api/v1")
            .route("/order", web::post().to(create_order))
            .route("/order/modify", web::post().to(modify_order))
            .route("/order/{order_id}", web::delete().to(cancel_order))
            .route("/orderbook", web::get().to(orderbook))
            .route("/orderbook/top", web::get().to(top_of_book)),
    );
}
