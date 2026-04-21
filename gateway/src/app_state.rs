use std::sync::Arc;

use crate::engine_handle::EngineHandle;

/// Shared state injected into every handler.
///
/// Holds a trait-object handle rather than a concrete engine so the
/// implementation (mutex-backed, router-backed, ...) can change without
/// touching the HTTP layer.
pub struct AppState {
    pub engine: Arc<dyn EngineHandle>,
}
