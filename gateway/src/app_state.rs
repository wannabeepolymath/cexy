use engine::engine::Engine;
use std::sync::Mutex;

pub struct AppState {
    pub engine: Mutex<Engine>,
}
