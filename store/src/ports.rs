use crate::GameEvent;
use crate::events::GameEventEnvelope;

pub trait EventPublisher: Send + Sync {
    fn publish_batch(&self, envelopes: Vec<GameEventEnvelope>) -> anyhow::Result<()>;
}

pub trait NetworkBroadcaster: Send + Sync {
    fn broadcast(&self, event: &GameEvent) -> anyhow::Result<()>;
    fn send_to(&self, client_id: u64, event: &GameEvent) -> anyhow::Result<()>;
}

// ── Test doubles ──────────────────────────────────────────────────────────────

pub struct NoopPublisher;
impl EventPublisher for NoopPublisher {
    fn publish_batch(&self, _: Vec<GameEventEnvelope>) -> anyhow::Result<()> {
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
}
impl EventPublisher for CapturingPublisher {
    fn publish_batch(&self, envelopes: Vec<GameEventEnvelope>) -> anyhow::Result<()> {
        self.published.lock().expect("poisoned").extend(envelopes);
        Ok(())
    }
}
