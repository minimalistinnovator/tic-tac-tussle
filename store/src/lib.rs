//! Store crate - the heart of the event sourcing hexagonal architecture

pub mod board;
pub mod commands;
pub mod decider;
pub mod errors;
pub mod events;
pub mod ports;
pub mod simulation;
pub mod state;
pub mod store;

pub use errors::TicTacTussleError;

pub use commands::GameCommand;

pub use state::{GameId, GameState, PlayerId, PlayerPair, Stage, Symbol, Tile};

pub use events::{EndGameReason, GameEvent, GameEventEnvelope};

pub use board::{is_full, winning_symbol};

pub use decider::GameDecider;

pub use simulation::SimulationHarness;

pub use ports::{EventPublisher, NetworkBroadcaster};
