//! Domain error types for the Tic-Tac-Tussle game.

use crate::state::{GameId, PlayerId, Stage};
use thiserror::Error;

/// Error type for the Tic-Tac-Tussle game domain.
#[derive(Debug, Error)]
pub enum TicTacTussleError {
    /// Error returned when a command is sent to the wrong game session.
    #[error("command for game {expected}, but this is game {actual}")]
    WrongGame {
        /// The game ID the command was intended for.
        expected: GameId,
        /// The game ID of the current session.
        actual: GameId,
    },
    /// Error returned when an action is performed in the wrong stage of the game.
    #[error("expected stage {expected:?}, actual {actual:?}")]
    WrongStage {
        /// The required stage for the action.
        expected: Stage,
        /// The current stage of the game.
        actual: Stage,
    },
    /// Error returned when an action is performed by a player who is not in the game.
    #[error("player {0} is not in this game")]
    UnknownPlayer(PlayerId),
    /// Error returned when a player tries to join a game they are already in.
    #[error("player {0} has already joined")]
    AlreadyJoined(PlayerId),
    /// Error returned when a tile index is outside the valid range of 0 to 8.
    #[error("tile index {0} is out of range (0-8)")]
    TileOutOfRange(usize),
    /// Error returned when a player tries to place a tile in an already occupied slot.
    #[error("tile {0} is already occupied")]
    TileOccupied(usize),
    /// Error returned when a player tries to make a move when it's not their turn.
    #[error("it is not player {0}'s turn")]
    NotYourTurn(PlayerId),
}
