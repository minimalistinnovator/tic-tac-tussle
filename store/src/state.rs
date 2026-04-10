//! Pure game-state reducer.
//!
//! `GameState` is a *projection* build by folding a sequence of `GameEvent`s.
//! It will never perform I/O operations; it can be used in simulations,
//! AI training, replays, and unit tests without any external dependencies.
//!

use crate::errors::StoreError;
use crate::events::{EndGameReason, GameEvent, PlayerId};
use bincode_next::{Decode, Encode};
use std::collections::HashMap;

/// What can occupy a board cell.
#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode, Default)]
pub enum Tile {
    #[default]
    Empty,
    /// First player's piece (assigned on `PlayerJoined` order).
    X,
    /// Second player's piece (assigned on `PlayerJoined` order).
    O,
}

/// Per-player data stored in the game state.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Player {
    pub name: String,
    /// The piece this player places on the board.
    pub piece: Tile,
}

/// Lifecycle stage of a game session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode, Default)]
pub enum Stage {
    #[default]
    Lobby,
    InGame,
    Ended,
}

/// The entire observable state of a TicTacToe game at a point in time.
///
/// Derive `Clone` so simulations can branch from any snapshot.
#[derive(Debug, Clone, Default, Encode, Decode)]
pub struct GameState {
    pub stage: Stage,
    /// The 3×3 board stored in row-major order (index 0..8).
    pub board: [Tile; 9],
    /// Player whose turn it currently is.
    pub active_player_id: PlayerId,
    /// All players indexed by their id.
    pub players: HashMap<PlayerId, Player>,
    /// Append-only event log — this is what makes it event-sourced.
    pub history: Vec<GameEvent>,
}

impl GameState {
    /// Returns `Ok(())` if `event` is legal given the current state.
    pub fn validate(&self, event: &GameEvent) -> Result<(), StoreError> {
        match event {
            GameEvent::PlayerJoined { player_id, .. } => {
                if self.players.contains_key(player_id) {
                    return Err(StoreError::InvalidEvent {
                        reason: format!("Player {player_id} already joined."),
                    });
                }
                if self.stage != Stage::Lobby {
                    return Err(StoreError::WrongStage {
                        expected: "Lobby".into(),
                        actual: format!("{:?}", self.stage),
                    });
                }
                Ok(())
            }
            GameEvent::PlayerDisconnected { player_id } => {
                if !self.players.contains_key(player_id) {
                    return Err(StoreError::UnknownPlayer {
                        player_id: *player_id,
                    });
                }
                Ok(())
            }
            GameEvent::BeginGame { goes_first } => {
                if self.stage != Stage::Lobby {
                    return Err(StoreError::WrongStage {
                        expected: "Lobby".into(),
                        actual: format!("{:?}", self.stage),
                    });
                }
                if !self.players.contains_key(goes_first) {
                    return Err(StoreError::UnknownPlayer {
                        player_id: *goes_first,
                    });
                }
                Ok(())
            }
            GameEvent::PlaceTile { player_id, at } => {
                if self.stage != Stage::InGame {
                    return Err(StoreError::WrongStage {
                        expected: "InGame".into(),
                        actual: format!("{:?}", self.stage),
                    });
                }
                if !self.players.contains_key(player_id) {
                    return Err(StoreError::UnknownPlayer {
                        player_id: *player_id,
                    });
                }
                if self.active_player_id != *player_id {
                    return Err(StoreError::NotYourTurn {
                        player_id: *player_id,
                    });
                }
                if *at > 8 {
                    return Err(StoreError::TileOutOfBounds { index: *at });
                }
                if self.board[*at] != Tile::Empty {
                    return Err(StoreError::TileOccupied { index: *at });
                }
                Ok(())
            }
            GameEvent::EndGame { reason } => {
                match reason {
                    EndGameReason::PlayerWon { .. } | EndGameReason::Draw => {
                        if self.stage != Stage::InGame {
                            return Err(StoreError::WrongStage {
                                expected: "InGame".into(),
                                actual: format!("{:?}", self.stage),
                            });
                        }
                    }
                    EndGameReason::PlayerLeft { player_id } => {
                        if !self.players.contains_key(player_id) {
                            return Err(StoreError::UnknownPlayer {
                                player_id: *player_id,
                            });
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Apply a **pre-validated** event, mutating state and recording it in
    /// `history`.
    ///
    /// # Panics
    /// Assumes the caller has already called `validate`.
    pub fn consume(&mut self, event: &GameEvent) {
        match event {
            GameEvent::PlayerJoined { player_id, name } => {
                let piece = if self.players.is_empty() {
                    Tile::X
                } else {
                    Tile::O
                };
                self.players.insert(
                    *player_id,
                    Player {
                        name: name.clone(),
                        piece,
                    },
                );
            }
            GameEvent::PlayerDisconnected { player_id } => {
                self.players.remove(player_id);
            }
            GameEvent::BeginGame { goes_first } => {
                self.active_player_id = *goes_first;
                self.stage = Stage::InGame;
            }
            GameEvent::PlaceTile { player_id, at } => {
                let piece = self.players[player_id].piece;
                self.board[*at] = piece;
                self.active_player_id = self
                    .players
                    .keys()
                    .copied()
                    .find(|&id| id != *player_id)
                    .unwrap_or(*player_id);
            }
            GameEvent::EndGame { .. } => {
                self.stage = Stage::Ended;
            }
        }
        self.history.push(event.clone());
    }

    /// Convenience: validate then consume.
    pub fn dispatch(&mut self, event: &GameEvent) -> Result<(), StoreError> {
        self.validate(event)?;
        self.consume(event);
        Ok(())
    }

    /// Returns the `PlayerId` of the winner, if any.
    pub fn determine_winner(&self) -> Option<PlayerId> {
        const WINS: [[usize; 3]; 8] = [
            [0, 1, 2],
            [3, 4, 5],
            [6, 7, 8], // rows
            [0, 3, 6],
            [1, 4, 7],
            [2, 5, 8], // cols
            [0, 4, 8],
            [2, 4, 6], // diagonals
        ];
        for triple in &WINS {
            let [a, b, c] = *triple;
            let ta = self.board[a];
            if ta != Tile::Empty && ta == self.board[b] && ta == self.board[c] {
                return self
                    .players
                    .iter()
                    .find(|(_, p)| p.piece == ta)
                    .map(|(&id, _)| id);
            }
        }
        None
    }

    /// Returns `true` if the board is full (draw condition).
    pub fn is_draw(&self) -> bool {
        self.board.iter().all(|t| *t != Tile::Empty)
    }

    /// Helper used by the client to look up the piece for a given player.
    pub fn get_player_tile(&self, player_id: &PlayerId) -> Option<Tile> {
        self.players.get(player_id).map(|p| p.piece)
    }
}
