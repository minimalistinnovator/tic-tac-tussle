//! Simulation harness for replaying and branching game history.
//!
//! This module provides a way to step through an existing event log,
//! inspect the state at any point, and explore "what-if" scenarios by
//! branching with new commands.

use crate::events::GameEvent;
use crate::state::GameState;
use crate::{GameCommand, GameDecider, TicTacTussleError};

/// A harness for simulating and replaying game sessions.
#[derive(Debug, Clone)]
pub struct SimulationHarness {
    /// The full log of events.
    log: Vec<GameEvent>,
    /// The current position in the log.
    cursor: usize,
    /// The state of the game at the current cursor position.
    state: GameState,
}

impl SimulationHarness {
    /// Creates a harness from a complete event log, positioned at the end.
    pub fn from_log(log: Vec<GameEvent>) -> Self {
        let state = GameDecider::hydrate(&log);
        let cursor = log.len();
        Self { log, cursor, state }
    }

    /// Creates a harness from an event log, positioned at index `n`.
    pub fn from_log_up_to(log: Vec<GameEvent>, n: usize) -> Self {
        let take = n.min(log.len());
        let state = GameDecider::hydrate(&log[..take]);
        Self {
            log,
            cursor: take,
            state,
        }
    }

    /// Returns the current `GameState`.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns `true` if the cursor is at the end of the log.
    pub fn is_exhausted(&self) -> bool {
        self.cursor >= self.log.len()
    }

    /// Moves the cursor forward by one event, updating the state.
    ///
    /// Returns `false` if the log is already exhausted.
    pub fn step_forward(&mut self) -> bool {
        if self.is_exhausted() {
            return false;
        }
        self.state = GameDecider::evolve(&self.state, &self.log[self.cursor]);
        self.cursor += 1;
        true
    }

    /// Rewinds or fast-forwards the cursor to index `n`.
    pub fn rewind_to(&mut self, n: usize) {
        self.cursor = n.min(self.log.len());
        self.state = GameDecider::hydrate(&self.log[..self.cursor]);
    }

    /// Simulates applying a command to the current state.
    ///
    /// Returns the resulting events and the new state if the command is valid,
    /// without modifying the harness itself.
    pub fn branch_with_command(
        &self,
        cmd: &GameCommand,
    ) -> Result<(Vec<GameEvent>, GameState), TicTacTussleError> {
        let events = GameDecider::decide(&self.state, cmd)?;
        let new_state = events
            .iter()
            .fold(self.state.clone(), |s, e| GameDecider::evolve(&s, e));
        Ok((events, new_state))
    }

    /// Replays the remaining events in the log.
    pub fn run_to_end(&mut self) -> &GameState {
        while self.step_forward() {}
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::GameEvent;
    use crate::state::PlayerId;

    #[test]
    fn simulation_harness_step_through() {
        let p1 = PlayerId(1);
        let p2 = PlayerId(2);
        let log = vec![
            GameEvent::PlayerJoined {
                player_id: p1,
                name: "Alice".to_string(),
            },
            GameEvent::PlayerJoined {
                player_id: p2,
                name: "Bob".to_string(),
            },
            GameEvent::GameStarted { goes_first: p1 },
        ];

        let mut harness = SimulationHarness::from_log_up_to(log.clone(), 0);
        assert_eq!(harness.cursor(), 0);
        assert_eq!(harness.state().players[0], PlayerId(0));

        harness.step_forward();
        assert_eq!(harness.cursor(), 1);
        assert_eq!(harness.state().players[0], p1);

        harness.run_to_end();
        assert_eq!(harness.cursor(), 3);
        assert_eq!(harness.state().players[1], p2);
        assert_eq!(harness.state().active_player_id, p1);
    }

    #[test]
    fn simulation_harness_branching() {
        let p1 = PlayerId(1);
        let p2 = PlayerId(2);
        let log = vec![
            GameEvent::PlayerJoined {
                player_id: p1,
                name: "Alice".to_string(),
            },
            GameEvent::PlayerJoined {
                player_id: p2,
                name: "Bob".to_string(),
            },
            GameEvent::GameStarted { goes_first: p1 },
        ];

        let harness = SimulationHarness::from_log(log);
        let cmd = GameCommand::PlaceTile {
            player_id: p1,
            at: 0,
        };

        let (events, next_state) = harness.branch_with_command(&cmd).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], GameEvent::TilePlaced { at: 0, .. }));
        assert_eq!(next_state.active_player_id, p2);

        // Original harness remains unchanged
        assert_eq!(harness.state().active_player_id, p1);
    }
}
