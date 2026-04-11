//! GameService — application-layer orchestrator.
//!
//! Wires: GameDecider (pure) + EventStore + EventPublisher + NetworkBroadcaster.
//!
//! Type-state Builder: every required field must be provided at compile time.

use anyhow::{Context, Result};
use store::store::EventStore;
use store::{BrokerMessage, CommandEnvelope, EventPublisher, GameDecider, NetworkBroadcaster};
use tracing::{debug, info, warn};

pub struct GameService {
    store: EventStore,
    publisher: Box<dyn EventPublisher>,
    broadcaster: Box<dyn NetworkBroadcaster>,
}

impl GameService {
    pub fn builder() -> Builder<(), (), ()> {
        Builder {
            store: (),
            publisher: (),
            broadcaster: (),
        }
    }

    /// Hydrate state → decide → append → publish → broadcast.
    pub fn handle(&mut self, cmd_env: CommandEnvelope) -> Result<()> {
        let state = GameDecider::hydrate(&self.store.events());
        let events =
            GameDecider::decide(&state, &cmd_env.command).map_err(|e| anyhow::anyhow!("{e}"))?;

        debug!(
            command_id = %cmd_env.id,
            produced = events.len(),
            "command accepted"
        );

        let envelopes = self.store.append_batch(events.clone(), Some(cmd_env.id));

        if let Err(e) = self.publisher.publish(BrokerMessage::EventBatch(envelopes)) {
            warn!(%e, "broker publish failed (non-fatal)");
        }

        for event in &events {
            self.broadcaster.broadcast(event).context("broadcast")?;
            info!(?event, "broadcasted");
        }
        Ok(())
    }

    /// Send full event history to a newly connected client.
    pub fn catch_up(&self, client_id: u64) -> Result<()> {
        for env in self.store.snapshot() {
            self.broadcaster
                .send_to(client_id, &env.event)
                .context("catch-up send_to")?;
        }
        debug!(%client_id, n = self.store.len(), "catch-up complete");
        Ok(())
    }

    pub fn publish(&self, msg: BrokerMessage) -> Result<()> {
        self.publisher.publish(msg).context("publish")
    }
}

// ── Type-state Builder ────────────────────────────────────────────────────────
pub struct Builder<S, P, B> {
    store: S,
    publisher: P,
    broadcaster: B,
}

impl<P, B> Builder<(), P, B> {
    pub fn store(self, s: EventStore) -> Builder<EventStore, P, B> {
        Builder {
            store: s,
            publisher: self.publisher,
            broadcaster: self.broadcaster,
        }
    }
}
impl<S, B> Builder<S, (), B> {
    pub fn publisher(
        self,
        p: impl EventPublisher + 'static,
    ) -> Builder<S, Box<dyn EventPublisher>, B> {
        Builder {
            store: self.store,
            publisher: Box::new(p),
            broadcaster: self.broadcaster,
        }
    }
}

impl<S, P> Builder<S, P, ()> {
    pub fn broadcaster(
        self,
        b: impl NetworkBroadcaster + 'static,
    ) -> Builder<S, P, Box<dyn NetworkBroadcaster>> {
        Builder {
            store: self.store,
            publisher: self.publisher,
            broadcaster: Box::new(b),
        }
    }
}

impl Builder<EventStore, Box<dyn EventPublisher>, Box<dyn NetworkBroadcaster>> {
    pub fn build(self) -> GameService {
        GameService {
            store: self.store,
            publisher: self.publisher,
            broadcaster: self.broadcaster,
        }
    }
}
