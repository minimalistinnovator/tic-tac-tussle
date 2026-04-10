//! EventStore — append-only in-process event log.  Thread-safe via Arc<Mutex<_>>.

use crate::GameEvent;
use crate::events::GameEventEnvelope;
use crate::state::GameId;
use std::sync::{Arc, Mutex, MutexGuard};
use tracing::debug;

struct Inner {
    game_id: GameId,
    log: Vec<GameEventEnvelope>,
}

#[derive(Clone)]
pub struct EventStore {
    inner: Arc<Mutex<Inner>>,
}

impl EventStore {
    pub fn new(game_id: GameId) -> Self {
        EventStore {
            inner: Arc::new(Mutex::new(Inner {
                game_id,
                log: Vec::new(),
            })),
        }
    }

    pub fn events(&self) -> Vec<GameEvent> {
        self.lock().log.iter().map(|e| e.event.clone()).collect()
    }
    pub fn snapshot(&self) -> Vec<GameEventEnvelope> {
        self.lock().log.clone()
    }
    pub fn game_id(&self) -> GameId {
        self.lock().game_id
    }
    pub fn len(&self) -> usize {
        self.lock().log.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("EventStore poisoned")
    }

    pub fn append(&self, event: GameEvent) -> GameEventEnvelope {
        let mut g = self.lock();
        let seq = g.log.len() as u64;
        let gee = GameEventEnvelope::new(g.game_id, seq, event);
        debug!(seq, ?gee.event, "appended");
        g.log.push(gee.clone());
        gee
    }

    pub fn append_batch(&self, events: Vec<GameEvent>) -> Vec<GameEventEnvelope> {
        let mut g = self.lock();
        events
            .into_iter()
            .map(|ev| {
                let seq = g.log.len() as u64;
                let env = GameEventEnvelope::new(g.game_id, seq, ev);
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
