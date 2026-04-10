//! Event store abstraction + SimulationHarness.
//!
//! `EventStore` is the durable, ordered log of all `GameEventEnvelope`s.
//! `SimulationHarness` can replay any snapshot — this fulfils the hard
//! requirement that event sourcing must be usable for simulations.

use crate::errors::StoreError;
pub(crate) use crate::events::{GameEvent, GameEventEnvelope};
use crate::state::GameState;
use bincode_next::error::{DecodeError, EncodeError};
use bincode_next::{Decode, Encode, config, decode_from_slice, encode_to_vec};
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Debug)]
struct EventStoreInner {
    game_id: Uuid,
    events: Vec<GameEventEnvelope>,
}
#[derive(Debug, Clone)]
pub struct EventStore {
    inner: Arc<Mutex<EventStoreInner>>,
}

impl EventStore {
    pub fn new(game_id: Uuid) -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventStoreInner {
                game_id,
                events: Vec::new(),
            })),
        }
    }

    /// Append a domain event. Returns the envelope so callers can publish it
    /// to Redpanda.
    pub fn append(&self, event: GameEvent) -> GameEventEnvelope {
        let mut inner = self.inner.lock().expect("EventStore lock poisoned");
        let seq = inner.events.len() as u64;
        let envelope = GameEventEnvelope::new(inner.game_id, seq, event);
        debug!(seq, ?envelope.event, "EventStore: appended");
        inner.events.push(envelope.clone());
        envelope
    }

    pub fn snapshot(&self) -> Vec<GameEventEnvelope> {
        self.inner.lock().expect("poisoned").events.clone()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("poisoned").events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn game_id(&self) -> Uuid {
        self.inner.lock().expect("poisoned").game_id
    }
}

/// Replay an event log into a fresh `GameState`.
///
/// Usage example (simulation / AI lookahead):
/// ```no_run
/// # use store::{EventStore, SimulationHarness};
/// # use uuid::Uuid;
/// # let store = EventStore::new(Uuid::new_v4());
/// // Full replay
/// let harness = SimulationHarness::replay(&store).unwrap();
///
/// // Step by step
/// let mut h = SimulationHarness::replay_up_to(&store, 0).unwrap();
/// while h.step_forward(&store).unwrap() {
///     println!("{:?}", h.state().board);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SimulationHarness {
    state: GameState,
    replayed: usize,
}

impl SimulationHarness {
    /// Replay every event in `store` into a new `GameState`.
    pub fn replay(store: &EventStore) -> Result<Self, StoreError> {
        let events = store.snapshot();
        let mut state = GameState::default();
        for (seq, envelope) in events.iter().enumerate() {
            state
                .validate(&envelope.event)
                .map_err(|e| StoreError::ReplayFailed {
                    seq,
                    source: Box::new(e),
                })?;
            state.consume(&envelope.event);
        }
        debug!(
            replayed = events.len(),
            "SimulationHarness: replay complete"
        );
        Ok(Self {
            state,
            replayed: events.len(),
        })
    }

    /// Replay only the first `n` events (step-by-step simulation support).
    pub fn replay_up_to(store: &EventStore, n: usize) -> Result<Self, StoreError> {
        let events = store.snapshot();
        let take = n.min(events.len());
        let mut state = GameState::default();
        for (seq, envelope) in events.iter().take(take).enumerate() {
            state
                .validate(&envelope.event)
                .map_err(|e| StoreError::ReplayFailed {
                    seq,
                    source: Box::new(e),
                })?;
            state.consume(&envelope.event);
        }
        debug!(
            replayed = take,
            "SimulationHarness: partial replay complete"
        );
        Ok(Self {
            state,
            replayed: take,
        })
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn into_state(self) -> GameState {
        self.state
    }

    pub fn replayed_count(&self) -> usize {
        self.replayed
    }

    /// Advance the simulation by one event.
    /// Returns `Ok(false)` when no more events are available.
    pub fn step_forward(&mut self, store: &EventStore) -> Result<bool, StoreError> {
        let events = store.snapshot();
        if self.replayed >= events.len() {
            warn!("SimulationHarness: no more events to replay");
            return Ok(false);
        }
        let envelope = &events[self.replayed];
        self.state
            .validate(&envelope.event)
            .map_err(|e| StoreError::ReplayFailed {
                seq: self.replayed,
                source: Box::new(e),
            })?;
        self.state.consume(&envelope.event);
        self.replayed += 1;
        Ok(true)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Serializable snapshot (for Redpanda replay bootstrap)
// ─────────────────────────────────────────────────────────────────────────────

/// A serialisable snapshot of the full event log for persistence or bootstrap.
#[derive(Debug, Clone, Encode, Decode)]
pub struct EventLogSnapshot {
    #[bincode(with_serde)]
    pub game_id: Uuid,
    pub events: Vec<GameEventEnvelope>,
}

impl EventLogSnapshot {
    pub fn from_store(store: &EventStore) -> Self {
        Self {
            game_id: store.game_id(),
            events: store.snapshot(),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        encode_to_vec(&self, config::standard())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (decoded, _len): (Self, usize) = decode_from_slice(bytes, config::standard())?;
        Ok(decoded)
    }
}
