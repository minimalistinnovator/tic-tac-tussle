use crate::state::{PlayerId, Stage};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TicTacTussleError {
    #[error("expected stage {expected:?}, actual {actual:?}")]
    WrongStage { expected: Stage, actual: Stage },
    #[error("player {0} is not in this game")]
    UnknownPlayer(PlayerId),
    #[error("player {0} has already joined")]
    AlreadyJoined(PlayerId),
    #[error("tile index {0} is out of range (0-8)")]
    TileOutOfRange(usize),
    #[error("tile {0} is already occupied")]
    TileOccupied(usize),
    #[error("it is not player {0}'s turn")]
    NotYourTurn(PlayerId),
}
