//! Tic-Tac-Tussle Store
//!
//! This crate implements the core domain logic and state management for the Tic-Tac-Tussle game,
//! following the principles of Hexagonal Architecture and Event Sourcing.
//!
//! It is designed to be a pure functional core (in `decider.rs`) surrounded by
//! ports (in `ports.rs`) that define how it interacts with the outside world.

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

pub use commands::{CommandEnvelope, GameCommand};

pub use state::{GameId, GameState, PlayerId, PlayerPair, Stage, Symbol, Tile};

pub use events::{EndGameReason, GameEvent, GameEventEnvelope};

pub use board::{is_full, winning_symbol};

pub use decider::GameDecider;

pub use simulation::SimulationHarness;

pub use ports::{
    AckHandle, BrokerMessage, CapturingPublisher, EventPublisher, NetworkBroadcaster,
    NoopBroadcaster, NoopPublisher, test_ack,
};
