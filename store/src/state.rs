//! Write-model game state.
//!
//! This module contains the state of the game used by the `GameDecider` to validate commands
//! and produce events. It follows the "Write Model" pattern where history is omitted,
//! and only current state required for logic is maintained.

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── New type identifiers ───────────────────────────────────────────────────────

/// A unique identifier for a player.
///
/// This is typically an opaque `u64` (mirroring `renet::ClientId` for networking).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Encode, Decode,
)]
pub struct PlayerId(pub u64);

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Player({})", self.0)
    }
}

/// A unique identifier for a specific game session.
///
/// Uses a UUID internally for global uniqueness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode, Serialize, Deserialize)]
pub struct GameId(#[bincode(with_serde)] pub Uuid);

impl GameId {
    /// Creates a new, randomly generated `GameId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for GameId {
    fn default() -> Self {
        Self::new()
    }
}
impl std::fmt::Display for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Board tile ────────────────────────────────────────────────────────────────

/// The two symbols available in a Tic-Tac-Toe game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum Symbol {
    /// The "X" symbol, typically assigned to the first player who joins.
    X,
    /// The "O" symbol, typically assigned to the second player who joins.
    O,
}

/// Represents the state of a single cell on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Encode, Decode, Serialize, Deserialize)]
pub enum Tile {
    /// The tile has not been played yet.
    #[default]
    Empty,
    /// The tile has been marked by a player with the specified symbol.
    Occupied(Symbol),
}

// ── Lifecycle stage ───────────────────────────────────────────────────────────

/// The possible lifecycle stages of a game session.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Encode, Decode, Serialize, Deserialize,
)]
pub enum Stage {
    /// Waiting for players to join.
    #[default]
    Lobby,
    /// The game is currently in progress.
    InGame,
    /// The game has finished (win, draw, or player left).
    Ended,
}

/// A pair of players associated with their respective symbols and names.
///
/// This structure acts as a read-model used by both the server and client to lookup
/// player information based on their ID. It is built from `PlayerJoined` events.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct PlayerPair {
    /// The first player to join, always assigned the 'X' symbol.
    pub x: (PlayerId, String),
    /// The second player to join, always assigned the 'O' symbol.
    pub o: (PlayerId, String),
}

impl PlayerPair {
    /// Creates a new pair of players.
    pub fn new(x: PlayerId, x_name: String, o: PlayerId, o_name: String) -> Self {
        Self {
            x: (x, x_name),
            o: (o, o_name),
        }
    }

    /// Returns the ID of the opponent for the given player.
    pub fn opponent_of(&self, id: PlayerId) -> PlayerId {
        if self.x.0 == id { self.o.0 } else { self.x.0 }
    }

    /// Returns the symbol assigned to the given player.
    pub fn symbol_of(&self, id: PlayerId) -> Symbol {
        if self.x.0 == id { Symbol::X } else { Symbol::O }
    }

    /// Returns the name of the given player, if they are part of this pair.
    pub fn name_of(&self, id: PlayerId) -> Option<&str> {
        if self.x.0 == id {
            Some(&self.x.1)
        } else if self.o.0 == id {
            Some(&self.o.1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_pair_symbol_lookup() {
        let p1 = PlayerId(1);
        let p2 = PlayerId(2);
        let pair = PlayerPair::new(p1, "Alice".to_string(), p2, "Bob".to_string());

        assert_eq!(pair.symbol_of(p1), Symbol::X);
        assert_eq!(pair.symbol_of(p2), Symbol::O);
    }

    #[test]
    fn player_pair_opponent_lookup() {
        let p1 = PlayerId(1);
        let p2 = PlayerId(2);
        let pair = PlayerPair::new(p1, "Alice".to_string(), p2, "Bob".to_string());

        assert_eq!(pair.opponent_of(p1), p2);
        assert_eq!(pair.opponent_of(p2), p1);
    }

    #[test]
    fn player_pair_name_lookup() {
        let p1 = PlayerId(1);
        let p2 = PlayerId(2);
        let p3 = PlayerId(3);
        let pair = PlayerPair::new(p1, "Alice".to_string(), p2, "Bob".to_string());

        assert_eq!(pair.name_of(p1), Some("Alice"));
        assert_eq!(pair.name_of(p2), Some("Bob"));
        assert_eq!(pair.name_of(p3), None);
    }

    #[test]
    fn game_id_generation() {
        let g1 = GameId::new();
        let g2 = GameId::new();
        assert_ne!(g1, g2);
    }
}

// ── GameState ─────────────────────────────────────────────────────────────────

/// The state of a Tic-Tac-Toe game.
///
/// This structure holds all the information required to determine the validity of the next command.
/// The board layout is row-major:
/// ```text
///  0 | 1 | 2
/// -----------
///  3 | 4 | 5
/// -----------
///  6 | 7 | 8
/// ```
#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize)]
pub struct GameState {
    /// The current stage of the game's lifecycle.
    pub stage: Stage,
    /// The 3x3 board represented as a flat array of 9 tiles.
    pub board: [Tile; 9],
    /// The ID of the player whose turn it is.
    pub active_player_id: PlayerId,
    /// The IDs of the two players in the game (0 is X, 1 is O).
    pub players: [PlayerId; 2],
}
