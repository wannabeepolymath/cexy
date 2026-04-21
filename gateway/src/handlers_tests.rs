#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web};
    use engine::engine::Engine;
    use engine::event_bus::{EventBus, EventConsumer};
    use engine::events::Event;
    use serde_json::{Value, json};
    use std::sync::Arc;

    use crate::app_state::AppState;
    use crate::engine_handle::MutexEngineHandle;
    use crate::handlers::configure;

    /// Discards every event. Keeps the consumer thread alive without
    /// polluting test output.
    struct NullConsumer;
    impl EventConsumer for NullConsumer {
        fn consume(&mut self, _event: Event) {}
    }

    fn test_event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(NullConsumer))
    }

    fn app_state() -> web::Data<AppState> {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        web::Data::new(AppState {
            engine: Arc::new(MutexEngineHandle::new(engine)),
            _event_bus: test_event_bus(),
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

    /// Smoke test the router-backed pipeline (main.rs's actual wiring).
    /// Proves that `Arc<dyn EngineHandle>` + [`crate::router::Router`] works
    /// inside the actix test server for both data-plane (place order) and
    /// control-plane (admin registration) requests.
    #[actix_web::test]
    async fn router_backed_state_drives_end_to_end_flow() {
        use crate::engine_handle::EngineHandle;
        use crate::router::Router;

        let event_bus = test_event_bus();
        let router = Router::new_with_events(2, event_bus.sender()).expect("router");
        assert!(router.register_instrument(1));
        let state: web::Data<AppState> = web::Data::new(AppState {
            engine: Arc::new(router),
            _event_bus: event_bus,
        });
        let app = test::init_service(App::new().app_data(state).configure(configure)).await;

        let place_req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 1,
                "account_id": 1,
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
        let body: Value = test::read_body_json(place_resp).await;
        assert_eq!(body["best_bid"], 100);
        assert!(body["best_ask"].is_null());

        // Instrument 2 still goes to a different shard via modulo, and
        // must surface UnknownInstrument because we never registered it.
        let reject_req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 2,
                "account_id": 1,
                "request_id": 1,
                "order_id": 2,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();
        let reject_resp = test::call_service(&app, reject_req).await;
        assert_eq!(reject_resp.status(), StatusCode::NOT_FOUND);

        // Register it via the admin endpoint and retry; now succeeds.
        let admin_req = test::TestRequest::post()
            .uri("/admin/instruments")
            .set_json(json!({"instrument_id": 2}))
            .to_request();
        let admin_resp = test::call_service(&app, admin_req).await;
        assert_eq!(admin_resp.status(), StatusCode::CREATED);

        let retry_req = test::TestRequest::post()
            .uri("/api/v1/order")
            .set_json(json!({
                "instrument_id": 2,
                "account_id": 1,
                "request_id": 2,
                "order_id": 2,
                "side": "buy",
                "order_type": "limit",
                "price": 100,
                "quantity": 10
            }))
            .to_request();
        let retry_resp = test::call_service(&app, retry_req).await;
        assert_eq!(retry_resp.status(), StatusCode::OK);
    }
}

