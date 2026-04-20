#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use engine::engine::Engine;
    use serde_json::{json, Value};
    use std::sync::Mutex;

    use crate::app_state::AppState;
    use crate::handlers::configure;

    fn app_state() -> web::Data<AppState> {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        web::Data::new(AppState {
            engine: Mutex::new(engine),
        })
    }

    #[actix_web::test]
    async fn create_order_rejects_invalid_identity_fields() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 0,
                "account_id": 42,
                "request_id": 1,
                "order_id": 1,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "instrument_id must be greater than 0");
    }

    #[actix_web::test]
    async fn modify_order_returns_bad_request_for_side_change() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;

        let place_req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 1,
                "account_id": 42,
                "request_id": 1,
                "order_id": 1,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();
        let place_resp = test::call_service(&app, place_req).await;
        assert_eq!(place_resp.status(), StatusCode::OK);

        let modify_req = test::TestRequest::post()
            .uri("/api/v1/order/modify")
            .set_json(json!({
                "instrument_id": 1,
                "account_id": 42,
                "request_id": 2,
                "order_id": 1,
                "side": "sell",
                "price": 101,
                "quantity": 10
            }))
            .to_request();
        let modify_resp = test::call_service(&app, modify_req).await;
        assert_eq!(modify_resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(modify_resp).await;
        assert_eq!(body["error"], "side change not allowed on modify");
    }

    #[actix_web::test]
    async fn create_order_returns_not_found_for_unregistered_instrument() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;
        let req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 99,
                "account_id": 42,
                "request_id": 1,
                "order_id": 1,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "unknown instrument: 99");
    }

    #[actix_web::test]
    async fn admin_register_instrument_creates_book() {
        let state = app_state();
        let app = test::init_service(App::new().app_data(state.clone()).configure(configure)).await;
        let req = test::TestRequest::post()
            .uri("/admin/instruments")
            .set_json(json!({"instrument_id": 7}))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["instrument_id"], 7);
        assert_eq!(body["created"], true);

        // Follow-up: placing an order on the newly-registered instrument should succeed.
        let place_req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 7,
                "account_id": 42,
                "request_id": 1,
                "order_id": 1,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();
        let place_resp = test::call_service(&app, place_req).await;
        assert_eq!(place_resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn admin_register_instrument_is_idempotent() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;
        // Instrument 1 is already registered by `app_state()`.
        let req = test::TestRequest::post()
            .uri("/admin/instruments")
            .set_json(json!({"instrument_id": 1}))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["instrument_id"], 1);
        assert_eq!(body["created"], false);
    }

    #[actix_web::test]
    async fn admin_register_instrument_rejects_zero() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;
        let req = test::TestRequest::post()
            .uri("/admin/instruments")
            .set_json(json!({"instrument_id": 0}))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "instrument_id must be greater than 0");
    }

    #[actix_web::test]
    async fn cancel_order_rejects_invalid_identity_fields() {
        let app = test::init_service(App::new().app_data(app_state()).configure(configure)).await;

        let req = test::TestRequest::delete()
            .uri("/api/v1/order/1?instrument_id=1&account_id=0&request_id=1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "account_id must be greater than 0");
    }
}

