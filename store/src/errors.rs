//! Domain errors for the store crate
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Event validation failed: {reason}")]
    InvalidEvent { reason: String },

    #[error(
        "Game is not in the expected stage: \
    (expected: {expected:?}, actual: {actual:?})"
    )]
    WrongStage { expected: String, actual: String },
    #[error("Player {player_id} is not in the game")]
    UnknownPlayer { player_id: u64 },
    #[error("Tile index {index} is out of bounds (max: 8)")]
    TileOutOfBounds { index: usize },
    #[error("Tile index {index} is already occupied")]
    TileOccupied { index: usize },
    #[error("It is not the player ({player_id})'s turn")]
    NotYourTurn { player_id: u64 },
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode_next::error::EncodeError),
    #[error("De-serialization error: {0}")]
    DeSerialization(#[from] bincode_next::error::DecodeError),
    #[error("Replay error at event #{seq}: {source}")]
    ReplayFailed {
        seq: usize,
        #[source]
        source: Box<StoreError>,
    },
}
