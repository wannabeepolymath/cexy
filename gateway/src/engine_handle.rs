//! Abstraction over how the gateway reaches the matching engine.
//!
//! The HTTP handlers do not talk to [`engine::engine::Engine`] directly.
//! They go through [`EngineHandle`], a narrow trait whose only requirement
//! is that every operation be atomic with respect to its own per-instrument
//! state. This keeps the gateway layer ignorant of whether the engine is
//! currently a single `Mutex<Engine>` ([`MutexEngineHandle`]) or, in a later
//! phase, a shard router.
//!
//! ### Why [`EngineReply`] bundles top-of-book
//!
//! The pre-trait code acquired the engine lock, ran `execute`, and then read
//! `best_bid` / `best_ask` under the same lock so the HTTP response reflected
//! the post-command book. A trait-based API with a separate `top_of_book`
//! call after `submit` would break that atomicity (another writer could move
//! the book between the two calls). We preserve it by returning the
//! top-of-book snapshot alongside the command output.
//!
//! [`engine::engine::Engine`]: engine::engine::Engine

use std::sync::Mutex;

use engine::commands::{Command, CommandOutput, EngineError, InstrumentId};
use engine::engine::Engine;
use engine::events::Events;
use engine::orderbook::level_info::OrderbookLevelInfo;
use engine::orderbook::types::Price;

/// Response bundle returned by [`EngineHandle::submit`].
///
/// Carries everything the HTTP layer needs for a single command so the
/// handle only pays one round-trip (one lock acquisition, or eventually
/// one shard channel hop).
#[derive(Debug)]
pub struct EngineReply {
    pub output: CommandOutput,
    /// Engine events produced by this command. Held on the reply so that a
    /// future event-consumer path (Phase 6) can drain them without another
    /// trip through the handle. Currently unread by the HTTP layer.
    #[allow(dead_code)]
    pub events: Events,
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
}

/// Narrow, object-safe interface the gateway uses to drive the engine.
///
/// All methods must be internally atomic per instrument. The contract does
/// not guarantee cross-instrument atomicity; callers needing that must not
/// assume it.
pub trait EngineHandle: Send + Sync {
    /// Run a command against the instrument it names, returning the output,
    /// the events it produced, and the post-command top-of-book.
    fn submit(&self, cmd: Command) -> Result<EngineReply, EngineError>;

    /// Register a new instrument. Returns `true` when a book was created,
    /// `false` on idempotent re-registration.
    fn register_instrument(&self, instrument_id: InstrumentId) -> bool;

    /// Full orderbook snapshot for the given instrument.
    /// `None` means the instrument is not registered.
    fn orderbook_snapshot(&self, instrument_id: InstrumentId) -> Option<OrderbookLevelInfo>;

    /// Top-of-book prices for the given instrument.
    ///
    /// Returns `None` when the instrument is not registered. The inner
    /// `Option`s are `None` when the respective side is empty.
    fn top_of_book(
        &self,
        instrument_id: InstrumentId,
    ) -> Option<(Option<Price>, Option<Price>)>;
}

/// [`EngineHandle`] implementation that serialises all access to a single
/// in-process [`Engine`] behind a [`Mutex`].
///
/// This is the pre-shard implementation: correct and simple, but all traffic
/// contends on one lock regardless of instrument. A router-backed handle
/// replaces this in a later phase without any handler changes.
pub struct MutexEngineHandle {
    engine: Mutex<Engine>,
}

impl MutexEngineHandle {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine: Mutex::new(engine),
        }
    }
}

impl EngineHandle for MutexEngineHandle {
    fn submit(&self, cmd: Command) -> Result<EngineReply, EngineError> {
        let instrument_id = cmd.instrument_id();
        let mut engine = self.engine.lock().unwrap();
        let result = engine.execute(cmd)?;
        let best_bid = engine.best_bid(instrument_id);
        let best_ask = engine.best_ask(instrument_id);
        Ok(EngineReply {
            output: result.output,
            events: result.events,
            best_bid,
            best_ask,
        })
    }

    fn register_instrument(&self, instrument_id: InstrumentId) -> bool {
        self.engine
            .lock()
            .unwrap()
            .register_instrument(instrument_id)
    }

    fn orderbook_snapshot(&self, instrument_id: InstrumentId) -> Option<OrderbookLevelInfo> {
        self.engine
            .lock()
            .unwrap()
            .get_orderbook_state(instrument_id)
    }

    fn top_of_book(
        &self,
        instrument_id: InstrumentId,
    ) -> Option<(Option<Price>, Option<Price>)> {
        let engine = self.engine.lock().unwrap();
        if !engine.is_registered(instrument_id) {
            return None;
        }
        Some((engine.best_bid(instrument_id), engine.best_ask(instrument_id)))
    }
}
