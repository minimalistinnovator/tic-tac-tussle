use crate::events::GameEvent;
use crate::state::GameState;
use crate::{GameCommand, GameDecider, TicTacTussleError};

#[derive(Debug, Clone)]
pub struct SimulationHarness {
    log: Vec<GameEvent>,
    cursor: usize,
    state: GameState,
}

impl SimulationHarness {
    pub fn from_log(log: Vec<GameEvent>) -> Self {
        let state = GameDecider::hydrate(&log);
        let cursor = log.len();
        Self { log, cursor, state }
    }

    pub fn from_log_up_to(log: Vec<GameEvent>, n: usize) -> Self {
        let take = n.min(log.len());
        let state = GameDecider::hydrate(&log[..take]);
        Self {
            log,
            cursor: take,
            state,
        }
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn is_exhausted(&self) -> bool {
        self.cursor >= self.log.len()
    }

    pub fn step_forward(&mut self) -> bool {
        if self.is_exhausted() {
            return false;
        }
        self.state = GameDecider::evolve(&self.state, &self.log[self.cursor]);
        self.cursor += 1;
        true
    }
    pub fn rewind_to(&mut self, n: usize) {
        self.cursor = n.min(self.log.len());
        self.state = GameDecider::hydrate(&self.log[..self.cursor]);
    }

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

    pub fn run_to_end(&mut self) -> &GameState {
        while self.step_forward() {}
        &self.state
    }
}
