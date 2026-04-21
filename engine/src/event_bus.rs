//! Event consumer foundation.
//!
//! Every command processed by a shard produces an ordered stream of
//! [`Event`]s. Phase 6 introduces the path that will carry those events to
//! downstream consumers (journal / market data publisher / metrics). For
//! now there is a single logical consumer; its interface is the
//! [`EventConsumer`] trait so future consumers can plug in without touching
//! the shard or router code.
//!
//! ### Topology
//!
//! ```text
//! shard 0 --\                     +----------------+
//!            >--- Sender<Event> ->| consumer thread| -> EventConsumer
//! shard 1 --/                     +----------------+
//! ```
//!
//! Each shard thread holds its own clone of the bus's `Sender<Event>`. The
//! bus owns the single receiver and drives one consumer thread that calls
//! [`EventConsumer::consume`] for every event in FIFO order.
//!
//! ### Ordering guarantees
//!
//! - **Per shard**: preserved. A shard thread is the sole producer on its
//!   sender clone and sends events in the order it produced them, so a
//!   consumer sees all events from a given shard in the order that shard
//!   emitted them. Since an instrument lives on exactly one shard, this
//!   implies per-instrument ordering as well.
//! - **Across shards**: no guarantee. Events from different shards can
//!   interleave arbitrarily depending on channel scheduling.
//!
//! ### Backpressure
//!
//! The bus currently uses an unbounded channel. Queue-depth limits and a
//! backpressure policy are explicitly deferred to Phase 9.

use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::events::Event;

/// Cloneable publish handle to an [`EventBus`]. Re-exported so downstream
/// crates (e.g. the gateway) don't have to depend on `crossbeam-channel`
/// directly.
pub type EventSender = Sender<Event>;

/// Plug-in point for anything that wants to observe engine events.
///
/// The bus spawns a single consumer thread and feeds every incoming event
/// to one implementation of this trait. Implementations must be `Send`
/// because they are moved onto the consumer thread; they do not need to
/// be `Sync` because the bus never shares them across threads.
pub trait EventConsumer: Send {
    /// Called exactly once per event, from the consumer thread, in the
    /// order events arrive on the bus. Implementations must not panic on
    /// valid events; a panic will terminate the consumer thread and
    /// silently drop subsequent events.
    fn consume(&mut self, event: Event);
}

/// Writes every event to stderr. Useful as the default consumer while the
/// journal / market data publisher are still vaporware.
pub struct LoggingConsumer;

impl EventConsumer for LoggingConsumer {
    fn consume(&mut self, event: Event) {
        eprintln!(
            "[event] seq={} instrument={} {:?}",
            event.seq(),
            event.instrument_id(),
            event
        );
    }
}

/// Owns the event channel and the dedicated consumer thread that drains it.
///
/// Construct once at startup, hand `sender()` clones to every shard, then
/// keep the [`EventBus`] alive for the lifetime of the process. Dropping it
/// closes the channel, which causes the consumer thread to exit cleanly;
/// the drop then joins the thread.
pub struct EventBus {
    // `Option` so `Drop` can take the sender and close the channel before
    // joining the consumer thread.
    sender: Option<Sender<Event>>,
    join: Option<JoinHandle<()>>,
}

impl EventBus {
    /// Spawn a consumer thread that feeds every event to `consumer`.
    ///
    /// The thread is named `event-consumer` for easier debugging in thread
    /// dumps. The channel is unbounded; see the module docs for the
    /// backpressure caveat.
    pub fn new<C: EventConsumer + 'static>(consumer: C) -> Self {
        let (tx, rx) = unbounded::<Event>();
        let join = thread::Builder::new()
            .name("event-consumer".to_string())
            .spawn(move || run_consumer(rx, consumer))
            .expect("failed to spawn event-consumer thread");

        Self {
            sender: Some(tx),
            join: Some(join),
        }
    }

    /// Cloneable handle to the event channel.
    ///
    /// Every shard thread holds one of these and publishes its own events
    /// through it.
    pub fn sender(&self) -> EventSender {
        self.sender
            .as_ref()
            .expect("event bus sender taken before drop")
            .clone()
    }
}

impl Drop for EventBus {
    fn drop(&mut self) {
        // Closing the sender causes `for event in rx` in the consumer
        // thread to terminate on the next iteration.
        self.sender.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_consumer<C: EventConsumer>(rx: Receiver<Event>, mut consumer: C) {
    for event in rx {
        consumer.consume(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Test-only consumer that pushes every event into a shared `Vec`.
    pub struct VecConsumer {
        events: Arc<Mutex<Vec<Event>>>,
    }

    impl VecConsumer {
        pub fn new() -> (Self, Arc<Mutex<Vec<Event>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: Arc::clone(&events),
                },
                events,
            )
        }
    }

    impl EventConsumer for VecConsumer {
        fn consume(&mut self, event: Event) {
            self.events.lock().unwrap().push(event);
        }
    }

    /// Block until the shared vec reaches `expected` length, or bail after
    /// a generous timeout so a broken test can't hang CI.
    fn wait_for_len(events: &Arc<Mutex<Vec<Event>>>, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if events.lock().unwrap().len() >= expected {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!(
            "timed out waiting for {} events, got {}",
            expected,
            events.lock().unwrap().len()
        );
    }

    fn accepted(seq: u64, instrument_id: u32, order_id: u64) -> Event {
        use crate::orderbook::side::Side;
        Event::OrderAccepted {
            seq,
            instrument_id,
            order_id,
            side: Side::Buy,
            price: 100,
            quantity: 1,
        }
    }

    #[test]
    fn single_sender_delivers_events_in_order() {
        let (consumer, events) = VecConsumer::new();
        let bus = EventBus::new(consumer);
        let tx = bus.sender();

        for i in 0..5 {
            tx.send(accepted(i, 1, i)).unwrap();
        }
        drop(tx);
        wait_for_len(&events, 5);

        let got = events.lock().unwrap();
        let seqs: Vec<u64> = got.iter().map(|e| e.seq()).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn multiple_senders_preserve_per_sender_order() {
        // Two simulated shards each send an in-order stream. The bus does
        // not guarantee a specific interleave across senders, but events
        // from each sender must remain in their original order.
        let (consumer, events) = VecConsumer::new();
        let bus = EventBus::new(consumer);

        let tx_a = bus.sender();
        let tx_b = bus.sender();

        let handle_a = thread::spawn(move || {
            for i in 0..50 {
                tx_a.send(accepted(i, 1, i)).unwrap();
            }
        });
        let handle_b = thread::spawn(move || {
            for i in 0..50 {
                tx_b.send(accepted(i, 2, i)).unwrap();
            }
        });

        handle_a.join().unwrap();
        handle_b.join().unwrap();
        wait_for_len(&events, 100);

        let got = events.lock().unwrap();
        let mut seq_a = Vec::new();
        let mut seq_b = Vec::new();
        for ev in got.iter() {
            match ev.instrument_id() {
                1 => seq_a.push(ev.seq()),
                2 => seq_b.push(ev.seq()),
                other => panic!("unexpected instrument {other}"),
            }
        }
        assert_eq!(seq_a, (0..50).collect::<Vec<_>>());
        assert_eq!(seq_b, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn drop_closes_channel_and_joins_consumer() {
        let (consumer, events) = VecConsumer::new();
        let bus = EventBus::new(consumer);
        let tx = bus.sender();
        tx.send(accepted(0, 1, 1)).unwrap();
        drop(tx);
        // Dropping the bus should close the channel and join the thread
        // without hanging.
        drop(bus);
        let got = events.lock().unwrap();
        assert_eq!(got.len(), 1);
    }
}
