//! Store crate - the heart of the event sourcing architecture
//!
//! Design goals:
//!     * Pure domain logic - zero I/O or networking
//!     * Every state mutation is driven by appending an event
//!     * The full game history is replayable for simulations / AI training
//!     * `GameState1 is deterministic: given the same event sequence, you always
//!          get the same game state

pub mod errors;
pub mod events;
pub mod state;
pub mod store;

pub use errors::StoreError;
pub use events::{EndGameReason, GameEvent, GameEventEnvelope};
pub use state::{GameState, Player, Stage, Tile};
pub use store::{EventStore, SimulationHarness};
