//! Write-model game state — NO history field.
//! Contains only what GameDecider needs to validate the next command.

use bevy::prelude::Resource;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── New type identifiers ───────────────────────────────────────────────────────

/// Opaque player identifier (mirrors renet ClientId).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Encode, Decode,
)]
pub struct PlayerId(pub u64);

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Player({})", self.0)
    }
}

/// Opaque game-session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode, Serialize, Deserialize)]
pub struct GameId(#[bincode(with_serde)] pub Uuid);

impl GameId {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum Symbol {
    X,
    O,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Encode, Decode, Serialize, Deserialize)]
pub enum Tile {
    #[default]
    Empty,
    Occupied(Symbol),
}

// ── Lifecycle stage ───────────────────────────────────────────────────────────

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Encode, Decode, Serialize, Deserialize,
)]
pub enum Stage {
    #[default]
    Lobby,
    InGame,
    Ended,
}

/// Read model: names, symbols, opponent lookup.
/// NOT stored in GameState. Built by server/client from PlayerJoined events.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct PlayerPair {
    pub x: (PlayerId, String), // first joiner → always X
    pub o: (PlayerId, String), // second joiner → always O
}

impl PlayerPair {
    pub fn new(x: PlayerId, x_name: String, o: PlayerId, o_name: String) -> Self {
        Self {
            x: (x, x_name),
            o: (o, o_name),
        }
    }

    pub fn opponent_of(&self, id: PlayerId) -> PlayerId {
        if self.x.0 == id { self.o.0 } else { self.x.0 }
    }

    pub fn symbol_of(&self, id: PlayerId) -> Symbol {
        if self.x.0 == id { Symbol::X } else { Symbol::O }
    }

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

// ── GameState ─────────────────────────────────────────────────────────────────

/// Board layout (row-major):  0|1|2 / 3|4|5 / 6|7|8
#[derive(Debug, Clone, Default, Encode, Decode, Resource)]
pub struct GameState {
    pub stage: Stage,
    pub board: [Tile; 9],
    pub active_player_id: PlayerId,
    pub players: [PlayerId; 2],
}
