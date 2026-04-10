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
