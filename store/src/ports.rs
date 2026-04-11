use crate::events::GameEventEnvelope;
use crate::{CommandEnvelope, GameEvent};

pub enum BrokerMessage {
    Command(CommandEnvelope),
    EventBatch(Vec<GameEventEnvelope>),
}

pub trait EventPublisher: Send + Sync {
    fn publish(&self, msg: BrokerMessage) -> anyhow::Result<()>;
}

pub trait NetworkBroadcaster: Send + Sync {
    fn broadcast(&self, event: &GameEvent) -> anyhow::Result<()>;
    fn send_to(&self, client_id: u64, event: &GameEvent) -> anyhow::Result<()>;
}

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
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        Self(Box::new(f))
    }
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

pub struct NoopPublisher;
impl EventPublisher for NoopPublisher {
    fn publish(&self, _: BrokerMessage) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct NoopBroadcaster;
impl NetworkBroadcaster for NoopBroadcaster {
    fn broadcast(&self, _: &GameEvent) -> anyhow::Result<()> {
        Ok(())
    }
    fn send_to(&self, _: u64, _: &GameEvent) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CapturingPublisher {
    pub published: std::sync::Mutex<Vec<GameEventEnvelope>>,
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

/// Creates an AckHandle that sets a flag — use in sync unit tests, no Tokio needed.
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
