//! Command definitions for the Tic-Tac-Tussle game.
//!
//! Commands represent the intent of a player to change the state of the game.

use crate::state::{GameId, PlayerId};
use bincode_next::{Decode, Encode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The various actions a player can take in the game.
#[derive(Debug, Clone, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum GameCommand {
    /// A player wants to join a game.
    JoinGame {
        /// The unique ID of the player joining.
        player_id: PlayerId,
        /// The name the player wants to use.
        name: String,
    },
    /// A player wants to place a tile on the board.
    PlaceTile {
        /// The ID of the player making the move.
        player_id: PlayerId,
        /// The index on the board (0-8) where the tile should be placed.
        at: usize,
    },
    /// A player wants to leave the game.
    LeaveGame {
        /// The ID of the player leaving.
        player_id: PlayerId,
    },
}

/// A wrapper for a `GameCommand` that includes metadata for tracking and auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    /// A unique identifier for this specific command instance.
    pub id: Uuid,
    /// The ID of the game this command is targeted at.
    pub game_id: GameId,
    /// The timestamp when the command was created.
    pub occurred_at: DateTime<Utc>,
    /// The actual command being issued.
    pub command: GameCommand,
}

impl CommandEnvelope {
    /// Creates a new `CommandEnvelope` with a fresh UUID and current timestamp.
    pub fn new(game_id: GameId, command: GameCommand) -> Self {
        Self {
            id: Uuid::new_v4(),
            game_id,
            occurred_at: Utc::now(),
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_creation() {
        let gid = GameId::new();
        let cmd = GameCommand::JoinGame {
            player_id: PlayerId(123),
            name: "Tester".to_string(),
        };
        let env = CommandEnvelope::new(gid, cmd);

        assert_eq!(env.game_id, gid);
        if let GameCommand::JoinGame { player_id, .. } = env.command {
            assert_eq!(player_id, PlayerId(123));
        } else {
            panic!("Wrong command type");
        }
    }
}
