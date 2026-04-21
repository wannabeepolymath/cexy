use std::sync::Arc;

use engine::event_bus::EventBus;

use crate::engine_handle::EngineHandle;

/// Shared state injected into every handler.
///
/// Holds a trait-object handle rather than a concrete engine so the
/// implementation (mutex-backed, router-backed, ...) can change without
/// touching the HTTP layer.
pub struct AppState {
    pub engine: Arc<dyn EngineHandle>,
    /// Kept alive for the lifetime of the server so the event consumer
    /// thread stays running. Handlers never read this; dropping it on
    /// shutdown is what closes the event channel and joins the consumer
    /// thread. `Arc` so it survives `web::Data` cloning into workers.
    #[allow(dead_code)]
    pub _event_bus: Arc<EventBus>,
}
