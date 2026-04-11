//! Ports for the Tic-Tac-Tussle game.
//!
//! Ports define the interfaces for external communication, such as message brokers
//! and network broadcasters. This is part of the Hexagonal Architecture.

use crate::events::GameEventEnvelope;
use crate::{CommandEnvelope, GameEvent};

/// A message that can be sent to or received from a message broker.
pub enum BrokerMessage {
    /// A command targeted at a game session.
    Command(CommandEnvelope),
    /// A batch of events produced by a game session.
    EventBatch(Vec<GameEventEnvelope>),
}

/// A trait for publishing messages to an external broker (e.g., Kafka/Redpanda).
pub trait EventPublisher: Send + Sync {
    /// Publishes a `BrokerMessage`.
    fn publish(&self, msg: BrokerMessage) -> anyhow::Result<()>;
}

/// A trait for broadcasting events over the network to connected clients.
pub trait NetworkBroadcaster: Send + Sync {
    /// Broadcasts an event to all connected clients.
    fn broadcast(&self, event: &GameEvent) -> anyhow::Result<()>;
    /// Sends an event to a specific client.
    fn send_to(&self, client_id: u64, event: &GameEvent) -> anyhow::Result<()>;
}

/// An opaque handle for acknowledging that a message has been processed.
pub struct AckHandle(Box<dyn FnOnce() + Send>);

// ── AckHandle ─────────────────────────────────────────────────────────────────

/// Opaque acknowledgement token handed to the game loop with every inbound
/// broker message.
///
/// Contract:
///   - Call `ack()` after the message has been fully processed (appended to
///     EventStore AND broadcast to clients).
///   - Dropping without calling `ack()` is intentional on failure paths —
///     the adapter interprets it as "processing failed" and leaves the
///     Kafka offset uncommitted so Redpanda redelivers the message.
///
/// The adapter constructs this by closing over its own signalling mechanism
/// (e.g. a tokio oneshot sender). The domain crate has no knowledge of that
/// mechanism — it sees only `FnOnce() + Send`.
impl AckHandle {
    /// Creates a new `AckHandle` from a closure.
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        Self(Box::new(f))
    }
    /// Acknowledges the successful processing of the message.
    pub fn ack(self) {
        (self.0)()
    }
}

impl std::fmt::Debug for AckHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AckHandle(<fn>)")
    }
}

// ── Test doubles ──────────────────────────────────────────────────────────────

/// A publisher that does nothing. Used for testing.
pub struct NoopPublisher;
impl EventPublisher for NoopPublisher {
    fn publish(&self, _: BrokerMessage) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A broadcaster that does nothing. Used for testing.
pub struct NoopBroadcaster;
impl NetworkBroadcaster for NoopBroadcaster {
    fn broadcast(&self, _: &GameEvent) -> anyhow::Result<()> {
        Ok(())
    }
    fn send_to(&self, _: u64, _: &GameEvent) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A publisher that captures all published messages for inspection in tests.
#[derive(Default)]
pub struct CapturingPublisher {
    /// The list of events captured.
    pub published: std::sync::Mutex<Vec<GameEventEnvelope>>,
    /// The list of commands captured.
    pub commands: std::sync::Mutex<Vec<CommandEnvelope>>,
}

impl EventPublisher for CapturingPublisher {
    fn publish(&self, msg: BrokerMessage) -> anyhow::Result<()> {
        match msg {
            BrokerMessage::Command(cmd) => {
                self.commands.lock().expect("poisoned").push(cmd);
            }
            BrokerMessage::EventBatch(envelopes) => {
                self.published.lock().expect("poisoned").extend(envelopes);
            }
        }
        Ok(())
    }
}

/// Creates an `AckHandle` that sets an atomic flag when acknowledged.
///
/// Useful for unit tests to verify that a message was acknowledged without needing
/// a full async environment.
///
/// ```
/// use store::ports::test_ack;
/// let (ack, was_acked) = test_ack();
/// ack.ack();
/// assert!(was_acked.load(std::sync::atomic::Ordering::SeqCst));
/// ```
pub fn test_ack() -> (AckHandle, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f = flag.clone();
    let handle = AckHandle::new(move || f.store(true, std::sync::atomic::Ordering::SeqCst));
    (handle, flag)
}
