//! All game domain events.
//!
//! These are the *only* way state advances. An event is an immutable
//! fact about something that has already happened.

use bincode_next::error::{DecodeError, EncodeError};
use bincode_next::{Decode, Encode, config, decode_from_slice, encode_to_vec};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A player's unique identifier - derived from renet `ClientId`
pub type PlayerId = u64;

/// Reasons a game can end
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum EndGameReason {
    /// A player disconnected; the opponent wins by default
    PlayerLeft { player_id: PlayerId },
    /// Someone filled three in a row
    PlayerWon { player_id: PlayerId },
    /// Board full, nobody won
    Draw,
}

/// Every possible game event.
///
/// Variants are ordered loosely by the game lifecycle:
/// lobby → start → play → end.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum GameEvent {
    /// A new player joined the lobby.
    PlayerJoined { player_id: PlayerId, name: String },
    /// A player disconnected from the lobby before the game started.
    PlayerDisconnected { player_id: PlayerId },
    /// The game has started; `goes_first` makes the first move.
    BeginGame { goes_first: PlayerId },
    /// A player placed their piece on `at` (0-based, row-major, 3×3 grid).
    PlaceTile { player_id: PlayerId, at: usize },
    /// The game has ended for `reason`.
    EndGame { reason: EndGameReason },
}

/// An envelope that wraps a [`GameEvent`] with metadata required for the
/// event log / broker.
///
/// This is the unit that gets serialised into Redpanda topics so that the
/// full history is replayable.
#[derive(Debug, Clone, Encode, Decode)]
pub struct GameEventEnvelope {
    /// Globally unique event identifier (useful for idempotency)
    #[bincode(with_serde)]
    pub id: Uuid,
    /// Monotonically increasing sequence number within a game session
    pub sequence: u64,
    /// The game session this event belongs to.
    #[bincode(with_serde)]
    pub game_id: Uuid,
    /// Wall-clock timestamp when the event was produced by the server
    #[bincode(with_serde)]
    pub occurred_at: DateTime<Utc>,
    /// The actual domain event
    pub event: GameEvent,
}

impl GameEventEnvelope {
    /// Wrap a [`GameEvent`] with a freshly generated id and the current time.
    pub fn new(game_id: Uuid, sequence: u64, event: GameEvent) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence,
            game_id,
            occurred_at: Utc::now(),
            event,
        }
    }

    /// Serialize to bytes (for Redpanda / renet wire format).
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        encode_to_vec(self, config::standard())
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (decoded, _len): (Self, usize) = decode_from_slice(bytes, config::standard())?;
        Ok(decoded)
    }
}
