//! The parameter bus: lock-free UI ↔ engine communication.
//!
//! Two channels:
//!
//! - **UI → engine** is a bounded SPSC queue of [`EngineEvent`] that the
//!   UI fills with `try_send` (non-blocking) and the audio callback drains
//!   with `try_recv` at the top of each block. Sizing follows
//!   `docs/planning/03-architecture/design-patterns.md` §2.2 (`4096`).
//! - **Engine → UI** is an [`ArcSwap`]-backed single-slot snapshot that
//!   the audio callback stores into once per block and any number of UI
//!   readers can `load_full` non-blocking. Optimised pool of pre-allocated
//!   `Arc<ParamSnapshot>` slots is a later milestone — for M1 the audio
//!   callback allocates a fresh `Arc` per block. This is intentionally
//!   *outside* the DSP hot path: `Engine::process_stereo` itself does not
//!   allocate (covered by the C6 `assert_no_alloc` test).

use std::sync::Arc;

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::events::EngineEvent;
use crate::params::ParamSnapshot;

/// Default capacity for the UI → engine queue, in events.
///
/// The starting size from `design-patterns.md` §2.2. Revisit at M3 with
/// measurements (bursty MIDI + UI traffic).
pub const DEFAULT_UI_TO_ENGINE_CAPACITY: usize = 4096;

/// The UI-side handle for sending [`EngineEvent`]s to the engine.
///
/// `try_send` is non-blocking; if the queue is full or the receiver
/// has been dropped, the event is logged and discarded so the UI
/// thread never stalls. This matches the §2.2 policy.
#[derive(Clone)]
pub struct EngineEventSender {
    inner: Sender<EngineEvent>,
}

impl EngineEventSender {
    /// Attempts to send an event. Logs a warning and drops the event
    /// if the queue is full or the engine has gone away.
    pub fn send(&self, event: EngineEvent) {
        use crossbeam_channel::TrySendError;
        match self.inner.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                tracing::warn!("UI->engine queue full; dropping event");
            }
            Err(TrySendError::Disconnected(_)) => {
                // Engine gone — app is shutting down. Nothing to do.
            }
        }
    }
}

/// The engine-side handle. Lives on the audio thread; drained at the
/// top of each block via [`EngineEventReceiver::try_recv`].
pub struct EngineEventReceiver {
    inner: Receiver<EngineEvent>,
}

impl EngineEventReceiver {
    /// Returns the next queued event, or `None` if the queue is empty
    /// or the sender has been dropped. Never blocks.
    #[must_use]
    pub fn try_recv(&self) -> Option<EngineEvent> {
        self.inner.try_recv().ok()
    }
}

/// Atomic single-slot snapshot used to publish engine state to readers.
///
/// Cloneable cheaply (`Arc`); readers call [`load_snapshot`] to get
/// the latest published value.
pub type SnapshotSlot = Arc<ArcSwap<ParamSnapshot>>;

/// Loads the latest published snapshot. Non-blocking; safe from any
/// thread.
#[must_use]
pub fn load_snapshot(slot: &SnapshotSlot) -> Arc<ParamSnapshot> {
    slot.load_full()
}

/// Stores a new snapshot into the slot. Non-blocking. Called from the
/// audio thread once per block.
pub fn store_snapshot(slot: &SnapshotSlot, snapshot: ParamSnapshot) {
    slot.store(Arc::new(snapshot));
}

/// Creates a fresh parameter bus: an SPSC event queue plus a snapshot
/// slot seeded with [`ParamSnapshot::default`].
#[must_use]
pub fn new_param_bus() -> (EngineEventSender, EngineEventReceiver, SnapshotSlot) {
    let (tx, rx) = bounded::<EngineEvent>(DEFAULT_UI_TO_ENGINE_CAPACITY);
    let slot: SnapshotSlot = Arc::new(ArcSwap::from_pointee(ParamSnapshot::default()));
    (EngineEventSender { inner: tx }, EngineEventReceiver { inner: rx }, slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_recv_round_trip() {
        let (tx, rx, _slot) = new_param_bus();
        tx.send(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        let event = rx.try_recv().expect("event should be queued");
        assert!(matches!(event, EngineEvent::NoteOn { note_midi: 60, .. }));
        assert!(rx.try_recv().is_none(), "queue should be empty after drain");
    }

    #[test]
    fn store_and_load_snapshot() {
        let (_tx, _rx, slot) = new_param_bus();
        let initial = load_snapshot(&slot);
        assert_eq!(initial.pitch_offset_semis, 0.0);

        let new_snap = ParamSnapshot {
            pitch_offset_semis: 5.0,
            ..ParamSnapshot::default()
        };
        store_snapshot(&slot, new_snap);

        let latest = load_snapshot(&slot);
        assert_eq!(latest.pitch_offset_semis, 5.0);
    }

    #[test]
    fn send_when_disconnected_does_not_panic() {
        let (tx, rx, _slot) = new_param_bus();
        drop(rx);
        // Should log and return, not panic.
        tx.send(EngineEvent::NoteOff { note_midi: 60 });
    }
}
