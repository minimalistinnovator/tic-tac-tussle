use crate::state::PlayerId;
use bincode_next::{Decode, Encode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum GameCommand {
    JoinGame { player_id: PlayerId, name: String },
    PlaceTile { player_id: PlayerId, at: usize },
    LeaveGame { player_id: PlayerId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub command: GameCommand,
}

impl CommandEnvelope {
    pub fn new(command: GameCommand) -> Self {
        Self {
            id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            command,
        }
    }
}
