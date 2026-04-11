use anyhow::{Context, Result};
use bincode_next::{config, encode_to_vec};
use renet::{DefaultChannel, RenetServer};
use std::sync::{Arc, Mutex};
use store::{GameEvent, NetworkBroadcaster};

pub struct RenetBroadcaster {
    server: Arc<Mutex<RenetServer>>,
}

impl RenetBroadcaster {
    pub fn new(server: Arc<Mutex<RenetServer>>) -> Self {
        Self { server }
    }
}

impl NetworkBroadcaster for RenetBroadcaster {
    fn broadcast(&self, event: &GameEvent) -> Result<()> {
        let bytes = encode_to_vec(event, config::standard()).context("encode broadcast")?;
        self.server
            .lock()
            .expect("renet mutex")
            .broadcast_message(DefaultChannel::ReliableOrdered, bytes);
        Ok(())
    }

    fn send_to(&self, client_id: u64, event: &GameEvent) -> Result<()> {
        let bytes = encode_to_vec(event, config::standard()).context("encode send_to")?;
        self.server.lock().expect("renet mutex").send_message(
            client_id,
            DefaultChannel::ReliableOrdered,
            bytes,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renet::{ConnectionConfig, RenetServer};
    use store::state::PlayerId;

    #[test]
    fn test_renet_broadcaster() {
        let config = ConnectionConfig::default();
        let server = RenetServer::new(config);
        let shared_server = Arc::new(Mutex::new(server));
        let broadcaster = RenetBroadcaster::new(shared_server.clone());

        let event = GameEvent::TilePlaced {
            player_id: PlayerId(1),
            at: 4,
        };

        // Broadcast (should not panic)
        assert!(broadcaster.broadcast(&event).is_ok());

        // Send to specific client (should not panic even if client 123 doesn't exist)
        assert!(broadcaster.send_to(123, &event).is_ok());
    }
}
