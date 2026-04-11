//! EventStore — an append-only in-process event log.
//!
//! This module provides a thread-safe, in-memory storage for game events.
//! It is used to maintain the history of a single game session.

use crate::GameEvent;
use crate::events::GameEventEnvelope;
use crate::state::GameId;
use std::sync::{Arc, Mutex, MutexGuard};
use tracing::debug;
use uuid::Uuid;

/// Internal state of the `EventStore`.
struct Inner {
    /// The ID of the game this store is associated with.
    game_id: GameId,
    /// The actual list of event envelopes.
    log: Vec<GameEventEnvelope>,
}

/// A thread-safe, clonable handle to an event log.
#[derive(Clone)]
pub struct EventStore {
    inner: Arc<Mutex<Inner>>,
}

impl EventStore {
    /// Creates a new, empty `EventStore` for the given `GameId`.
    pub fn new(game_id: GameId) -> Self {
        EventStore {
            inner: Arc::new(Mutex::new(Inner {
                game_id,
                log: Vec::new(),
            })),
        }
    }

    /// Returns all events in the store as a flat vector of `GameEvent`s.
    pub fn events(&self) -> Vec<GameEvent> {
        self.lock().log.iter().map(|e| e.event.clone()).collect()
    }

    /// Returns a full snapshot of the event log, including envelopes.
    pub fn snapshot(&self) -> Vec<GameEventEnvelope> {
        self.lock().log.clone()
    }

    /// Returns the `GameId` associated with this store.
    pub fn game_id(&self) -> GameId {
        self.lock().game_id
    }

    /// Returns the number of events in the store.
    pub fn len(&self) -> usize {
        self.lock().log.len()
    }

    /// Returns `true` if the store contains no events.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Helper to acquire the inner mutex lock.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("EventStore poisoned")
    }

    /// Appends a new event to the store.
    ///
    /// Returns the created `GameEventEnvelope`.
    pub fn append(&self, event: GameEvent, command_id: Option<Uuid>) -> GameEventEnvelope {
        let mut g = self.lock();
        let seq = g.log.len() as u64;
        let gee = GameEventEnvelope::new(g.game_id, seq, event, command_id);
        debug!(seq, ?gee.event, "appended");
        g.log.push(gee.clone());
        gee
    }

    /// Appends a batch of events to the store.
    ///
    /// Returns a vector of created `GameEventEnvelope`s.
    pub fn append_batch(
        &self,
        events: Vec<GameEvent>,
        command_id: Option<Uuid>,
    ) -> Vec<GameEventEnvelope> {
        let mut g = self.lock();
        events
            .into_iter()
            .map(|ev| {
                let seq = g.log.len() as u64;
                let env = GameEventEnvelope::new(g.game_id, seq, ev, command_id);
                debug!(seq, ?env.event, "batch-appended");
                g.log.push(env.clone());
                env
            })
            .collect()
    }
}

impl std::fmt::Debug for EventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let g = self.lock();
        f.debug_struct("EventStore")
            .field("game_id", &g.game_id)
            .field("len", &g.log.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameEvent;

    #[test]
    fn test_event_store_lifecycle() {
        let game_id = GameId::new();
        let store = EventStore::new(game_id);

        assert_eq!(store.game_id(), game_id);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        let event1 = GameEvent::GameStarted {
            goes_first: crate::state::PlayerId(1),
        };

        let env1 = store.append(event1.clone(), None);
        assert_eq!(env1.event, event1);
        assert_eq!(env1.sequence, 0);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        let event2 = GameEvent::TilePlaced {
            player_id: crate::state::PlayerId(1),
            at: 0,
        };
        let event3 = GameEvent::TilePlaced {
            player_id: crate::state::PlayerId(2),
            at: 1,
        };

        let batch = store.append_batch(vec![event2.clone(), event3.clone()], None);
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].sequence, 1);
        assert_eq!(batch[1].sequence, 2);
        assert_eq!(store.len(), 3);

        let all_events = store.events();
        assert_eq!(all_events.len(), 3);
        assert_eq!(all_events[0], event1);

        let snapshot = store.snapshot();
        assert_eq!(snapshot.len(), 3);
        assert_eq!(snapshot[2].event, event3);

        let debug_str = format!("{:?}", store);
        assert!(debug_str.contains("EventStore"));
        assert!(debug_str.contains("game_id"));
    }
}
