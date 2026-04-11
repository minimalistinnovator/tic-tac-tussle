//! GameService — the application-layer orchestrator.
//!
//! This service coordinates the interaction between the pure domain logic (`GameDecider`),
//! the persistence layer (`EventStore`), and the communication ports (`EventPublisher` and `NetworkBroadcaster`).
//!
//! It uses a Type-state Builder pattern to ensure that all required dependencies
//! are provided at compile time.

use anyhow::{Context, Result};
use store::store::EventStore;
use store::{BrokerMessage, CommandEnvelope, EventPublisher, GameDecider, NetworkBroadcaster};
use tracing::{debug, info, warn};

/// The primary service for managing a game session's lifecycle and command processing.
pub struct GameService {
    /// The local in-memory event store for the game session.
    store: EventStore,
    /// The publisher for sending events/commands to an external broker.
    publisher: Box<dyn EventPublisher>,
    /// The broadcaster for sending events to connected network clients.
    broadcaster: Box<dyn NetworkBroadcaster>,
}

impl GameService {
    /// Starts the construction of a `GameService` using the builder pattern.
    pub fn builder() -> Builder<(), (), ()> {
        Builder {
            store: (),
            publisher: (),
            broadcaster: (),
        }
    }

    /// Handles an incoming command envelope.
    ///
    /// The process involves:
    /// 1. Validating the game ID.
    /// 2. Hydrating the current state from the event store.
    /// 3. Deciding which events to produce based on the command and current state.
    /// 4. Appending the produced events to the event store.
    /// 5. Publishing the new events to the external broker.
    /// 6. Broadcasting the events to all connected clients.
    pub fn handle(&mut self, cmd_env: CommandEnvelope) -> Result<()> {
        if cmd_env.game_id != self.store.game_id() {
            return Err(anyhow::anyhow!(store::TicTacTussleError::WrongGame {
                expected: cmd_env.game_id,
                actual: self.store.game_id(),
            }));
        }

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

    /// Sends the full event history to a newly connected client so they can catch up.
    pub fn catch_up(&self, client_id: u64) -> Result<()> {
        for env in self.store.snapshot() {
            self.broadcaster
                .send_to(client_id, &env.event)
                .context("catch-up send_to")?;
        }
        debug!(%client_id, n = self.store.len(), "catch-up complete");
        Ok(())
    }

    /// Manually publishes a message to the external broker.
    pub fn publish(&self, msg: BrokerMessage) -> Result<()> {
        self.publisher.publish(msg).context("publish")
    }
}

// ── Type-state Builder ────────────────────────────────────────────────────────

/// A builder for `GameService` that uses type-states to track which fields have been set.
pub struct Builder<S, P, B> {
    store: S,
    publisher: P,
    broadcaster: B,
}

impl<P, B> Builder<(), P, B> {
    /// Sets the `EventStore` for the `GameService`.
    pub fn store(self, s: EventStore) -> Builder<EventStore, P, B> {
        Builder {
            store: s,
            publisher: self.publisher,
            broadcaster: self.broadcaster,
        }
    }
}
impl<S, B> Builder<S, (), B> {
    /// Sets the `EventPublisher` for the `GameService`.
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
    /// Sets the `NetworkBroadcaster` for the `GameService`.
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
    /// Finalizes the builder and returns a `GameService` instance.
    pub fn build(self) -> GameService {
        GameService {
            store: self.store,
            publisher: self.publisher,
            broadcaster: self.broadcaster,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::commands::GameCommand;
    use store::events::GameEvent;
    use store::ports::{CapturingPublisher, NoopBroadcaster};
    use store::state::{GameId, PlayerId};
    use store::store::EventStore;

    #[test]
    fn handle_join_command() {
        let gid = GameId::new();
        let store = EventStore::new(gid);
        let publisher = CapturingPublisher::default();
        let broadcaster = NoopBroadcaster;

        let mut service = GameService::builder()
            .store(store)
            .publisher(publisher)
            .broadcaster(broadcaster)
            .build();

        let cmd = GameCommand::JoinGame {
            player_id: PlayerId(1),
            name: "Alice".to_string(),
        };
        let env = store::CommandEnvelope::new(gid, cmd);

        service.handle(env).unwrap();

        assert_eq!(service.store.len(), 1);
        let events = service.store.events();
        assert!(
            matches!(events[0], GameEvent::PlayerJoined { player_id, .. } if player_id == PlayerId(1))
        );
    }

    #[test]
    fn handle_wrong_game_id() {
        let gid1 = GameId::new();
        let gid2 = GameId::new();
        let store = EventStore::new(gid1);

        let mut service = GameService::builder()
            .store(store)
            .publisher(CapturingPublisher::default())
            .broadcaster(NoopBroadcaster)
            .build();

        let cmd = GameCommand::JoinGame {
            player_id: PlayerId(1),
            name: "Alice".to_string(),
        };
        let env = store::CommandEnvelope::new(gid2, cmd);

        let res = service.handle(env);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("command for game"));
    }
}
