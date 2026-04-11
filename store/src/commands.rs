use crate::state::PlayerId;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Encode, Decode, Serialize, Deserialize)]
pub enum GameCommand {
    JoinGame { player_id: PlayerId, name: String },
    PlaceTile { player_id: PlayerId, at: usize },
    LeaveGame { player_id: PlayerId },
}
