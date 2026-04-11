//! Event definitions for the Tic-Tac-Tussle game.
//!
//! Events represent something that has happened in the game world. They are the
//! result of a successfully processed command.

use crate::{GameId, PlayerId};
use bincode_next::serde::{decode_from_slice, encode_to_vec};
use bincode_next::{Decode, Encode, config};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reasons why a game session might end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum EndGameReason {
    /// A player won the game.
    PlayerWon {
        /// The ID of the winning player.
        winner: PlayerId,
    },
    /// The game ended in a draw (board is full).
    Draw,
    /// A player left the game, causing it to end prematurely.
    PlayerLeft {
        /// The ID of the player who left.
        player_id: PlayerId,
    },
}

/// The various things that can happen during a game session.
#[derive(Debug, Clone, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum GameEvent {
    /// A player has successfully joined the game.
    PlayerJoined {
        /// The ID of the player who joined.
        player_id: PlayerId,
        /// The name the player used.
        name: String,
    },
    /// The game has started.
    GameStarted {
        /// The ID of the player who will make the first move.
        goes_first: PlayerId,
    },
    /// A tile was placed on the board.
    TilePlaced {
        /// The ID of the player who placed the tile.
        player_id: PlayerId,
        /// The board index (0-8) where the tile was placed.
        at: usize,
    },
    /// A player has left the game.
    PlayerLeft {
        /// The ID of the player who left.
        player_id: PlayerId,
    },
    /// The game has ended.
    GameEnded {
        /// The reason the game ended.
        reason: EndGameReason,
    },
}

/// A wrapper for a `GameEvent` that includes metadata for persistence and synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEventEnvelope {
    /// A unique identifier for this specific event.
    pub id: Uuid,
    /// The ID of the command that triggered this event, if applicable.
    pub command_id: Option<Uuid>,
    /// A monotonically increasing sequence number for ordering events within a game.
    pub sequence: u64,
    /// The ID of the game this event belongs to.
    pub game_id: GameId,
    /// The timestamp when the event occurred.
    pub occurred_at: DateTime<Utc>,
    /// The actual event data.
    pub event: GameEvent,
}

impl GameEventEnvelope {
    /// Creates a new `GameEventEnvelope` with a fresh UUID and current timestamp.
    pub fn new(game_id: GameId, sequence: u64, event: GameEvent, command_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            command_id,
            sequence,
            game_id,
            occurred_at: Utc::now(),
            event,
        }
    }

    /// Serializes the envelope into a byte vector using `bincode`.
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        encode_to_vec(self, config::standard()).map_err(|e| anyhow::anyhow!("encode: {e}"))
    }

    /// Deserializes a byte slice into a `GameEventEnvelope` using `bincode`.
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        decode_from_slice(bytes, config::standard())
            .map(|(v, _)| v)
            .map_err(|e| anyhow::anyhow!("decode: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serialization_roundtrip() {
        let gid = GameId::new();
        let event = GameEvent::PlayerJoined {
            player_id: PlayerId(1),
            name: "Alice".to_string(),
        };
        let envelope = GameEventEnvelope::new(gid, 1, event, None);

        let bytes = envelope.to_bytes().unwrap();
        let decoded = GameEventEnvelope::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.game_id, gid);
        assert_eq!(decoded.sequence, 1);
        if let GameEvent::PlayerJoined { player_id, name } = decoded.event {
            assert_eq!(player_id, PlayerId(1));
            assert_eq!(name, "Alice");
        } else {
            panic!("Event type mismatch");
        }
    }
}
