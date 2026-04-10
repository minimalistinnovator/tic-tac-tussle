use crate::{GameId, PlayerId};
use bincode_next::serde::{decode_from_slice, encode_to_vec};
use bincode_next::{Decode, Encode, config};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum EndGameReason {
    PlayerWon { winner: PlayerId },
    Draw,
    PlayerLeft { player_id: PlayerId },
}

#[derive(Debug, Clone, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum GameEvent {
    PlayerJoined { player_id: PlayerId, name: String },
    GameStarted { goes_first: PlayerId },
    TilePlaced { player_id: PlayerId, at: usize },
    PlayerLeft { player_id: PlayerId },
    GameEnded { reason: EndGameReason },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEventEnvelope {
    pub id: Uuid,
    pub sequence: u64,
    pub game_id: GameId,
    pub occurred_at: DateTime<Utc>,
    pub event: GameEvent,
}

impl GameEventEnvelope {
    pub fn new(game_id: GameId, sequence: u64, event: GameEvent) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence,
            game_id,
            occurred_at: Utc::now(),
            event,
        }
    }
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        encode_to_vec(self, config::standard()).map_err(|e| anyhow::anyhow!("encode: {e}"))
    }
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        decode_from_slice(bytes, config::standard())
            .map(|(v, _)| v)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))
    }
}
