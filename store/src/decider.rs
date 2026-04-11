//! GameDecider — the pure functional core of the game.
//!
//! This module implements the core business logic of Tic-Tac-Tussle using a pure functional
//! approach. It follows the "Decider" pattern:
//!
//! - `decide(state, cmd)`: Validates a command against the current state and produces a list of events.
//! - `evolve(state, event)`: Applies an event to the current state to produce a new state.
//! - `hydrate(events)`: Reconstructs the current state by applying a sequence of events from the beginning.
//!
//! All domain rules live here and only here. This code is side-effect free and can be tested
//! without any infrastructure or I/O.

use crate::board::{is_full, winning_symbol};
use crate::commands::GameCommand;
use crate::errors::TicTacTussleError;
use crate::events::EndGameReason;
use crate::events::GameEvent;
use crate::state::GameState;
use crate::state::{PlayerId, Stage, Symbol, Tile};

/// The central component for game logic.
pub struct GameDecider;

impl GameDecider {
    /// Decides which events should be produced in response to a command.
    ///
    /// This function is the "brain" of the game. it validates the command against the
    /// current `GameState` and returns a vector of `GameEvent`s if the command is valid,
    /// or a `TicTacTussleError` if it is not.
    pub fn decide(
        state: &GameState,
        cmd: &GameCommand,
    ) -> Result<Vec<GameEvent>, TicTacTussleError> {
        match cmd {
            GameCommand::JoinGame { player_id, name } => {
                // Validation: Must be in Lobby to join.
                Self::require_stage(state, Stage::Lobby)?;
                // Validation: Cannot join twice.
                if state.players[0] == *player_id || state.players[1] == *player_id {
                    return Err(TicTacTussleError::AlreadyJoined(*player_id));
                }

                // Event creation
                let join = GameEvent::PlayerJoined {
                    player_id: *player_id,
                    name: name.clone(),
                };

                // Speculatively evolve to see if both slots are now filled.
                let next = Self::evolve(state, &join);

                // Both slots filled means the second player just joined -> auto-start.
                // Note: players[0] is always X and always goes first.
                if next.players[1] != PlayerId(0) {
                    Ok(vec![
                        join,
                        GameEvent::GameStarted {
                            goes_first: next.players[0],
                        },
                    ])
                } else {
                    Ok(vec![join])
                }
            }
            GameCommand::PlaceTile { player_id, at } => {
                // Validation: Game must be in progress.
                Self::require_stage(state, Stage::InGame)?;
                // Validation: Must be the player's turn.
                if state.active_player_id != *player_id {
                    return Err(TicTacTussleError::NotYourTurn(*player_id));
                }
                // Validation: Tile index must be valid.
                if *at > 8 {
                    return Err(TicTacTussleError::TileOutOfRange(*at));
                }
                // Validation: Tile must be empty.
                if state.board[*at] != Tile::Empty {
                    return Err(TicTacTussleError::TileOccupied(*at));
                }

                let place = GameEvent::TilePlaced {
                    player_id: *player_id,
                    at: *at,
                };

                // Speculatively evolve to run win/draw detection on the resulting board.
                let next = Self::evolve(state, &place);
                let mut ev = vec![place];

                if winning_symbol(&next.board).is_some() {
                    ev.push(GameEvent::GameEnded {
                        reason: EndGameReason::PlayerWon { winner: *player_id },
                    });
                } else if is_full(&next.board) {
                    ev.push(GameEvent::GameEnded {
                        reason: EndGameReason::Draw,
                    });
                }
                Ok(ev)
            }
            GameCommand::LeaveGame { player_id } => {
                let mut ev = vec![GameEvent::PlayerLeft {
                    player_id: *player_id,
                }];
                // If the game was in progress, leaving causes it to end.
                if state.stage == Stage::InGame {
                    ev.push(GameEvent::GameEnded {
                        reason: EndGameReason::PlayerLeft {
                            player_id: *player_id,
                        },
                    });
                }
                Ok(ev)
            }
        }
    }

    /// Applies an event to a state to produce a new state.
    ///
    /// This function is infallible. It assumes the event is valid (as it should have
    /// been validated by `decide` before being persisted/emitted).
    pub fn evolve(state: &GameState, event: &GameEvent) -> GameState {
        let mut next = state.clone();
        match event {
            GameEvent::PlayerJoined { player_id, .. } => {
                // Fill first empty lobby slot (PlayerId(0) == sentinel for empty).
                if next.players[0] == PlayerId(0) {
                    next.players[0] = *player_id; // -> X
                } else {
                    next.players[1] = *player_id; // -> O
                }
            }
            GameEvent::GameStarted { goes_first } => {
                next.stage = Stage::InGame;
                next.active_player_id = *goes_first;
            }
            GameEvent::TilePlaced { player_id, at } => {
                // Symbol: players[0] = X, players[1] = O
                let symbol = if next.players[0] == *player_id {
                    Symbol::X
                } else {
                    Symbol::O
                };
                next.board[*at] = Tile::Occupied(symbol);
                // Rotate turn
                next.active_player_id = if next.players[0] == *player_id {
                    next.players[1]
                } else {
                    next.players[0]
                };
            }
            GameEvent::PlayerLeft { .. } => {}
            GameEvent::GameEnded { .. } => {
                next.stage = Stage::Ended;
            }
        }
        next
    }

    /// Hydrates a `GameState` from a history of events.
    ///
    /// This is used to reconstruct the state of a game from its event log.
    pub fn hydrate(events: &[GameEvent]) -> GameState {
        events
            .iter()
            .fold(GameState::default(), |s, e| Self::evolve(&s, e))
    }

    /// Helper function to ensure the game is in the expected stage.
    #[inline]
    fn require_stage(
        state: &GameState,
        expected: crate::state::Stage,
    ) -> Result<(), TicTacTussleError> {
        if state.stage != expected {
            return Err(TicTacTussleError::WrongStage {
                expected,
                actual: state.stage,
            });
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PlayerId;
    use crate::state::Stage;

    fn pid(n: u64) -> PlayerId {
        PlayerId(n)
    }

    fn started() -> (GameState, PlayerId, PlayerId) {
        let s0 = GameState::default();
        let e1 = GameDecider::decide(
            &s0,
            &GameCommand::JoinGame {
                player_id: pid(1),
                name: "Alice".into(),
            },
        )
        .unwrap();
        let s1 = GameDecider::hydrate(&e1);
        let e2 = GameDecider::decide(
            &s1,
            &GameCommand::JoinGame {
                player_id: pid(2),
                name: "Bob".into(),
            },
        )
        .unwrap();
        let all: Vec<_> = e1.into_iter().chain(e2).collect();
        let s2 = GameDecider::hydrate(&all);
        (s2.clone(), s2.players[0], s2.players[1])
    }

    #[test]
    fn two_joins_starts_game() {
        let (s, _, _) = started();
        assert_eq!(s.stage, Stage::InGame);
    }

    #[test]
    fn x_goes_first() {
        let (s, x, _) = started();
        assert_eq!(s.active_player_id, x);
    }

    #[test]
    fn wrong_turn_rejected() {
        let (s, _, o) = started();
        assert!(matches!(
            GameDecider::decide(
                &s,
                &GameCommand::PlaceTile {
                    player_id: o,
                    at: 0
                }
            ),
            Err(TicTacTussleError::NotYourTurn(_))
        ));
    }

    #[test]
    fn occupied_tile_rejected() {
        let (s, x, o) = started();
        let evs = GameDecider::decide(
            &s,
            &GameCommand::PlaceTile {
                player_id: x,
                at: 4,
            },
        )
        .unwrap();
        let s2 = evs.iter().fold(s, |st, e| GameDecider::evolve(&st, e));
        assert!(matches!(
            GameDecider::decide(
                &s2,
                &GameCommand::PlaceTile {
                    player_id: o,
                    at: 4
                }
            ),
            Err(TicTacTussleError::TileOccupied(4))
        ));
    }

    #[test]
    fn win_ends_game() {
        let (mut s, x, o) = started();
        // X wins top row: 0,1,2
        for (p, at) in [(x, 0), (o, 3), (x, 1), (o, 4), (x, 2)] {
            let evs =
                GameDecider::decide(&s, &GameCommand::PlaceTile { player_id: p, at }).unwrap();
            s = evs.iter().fold(s, |st, e| GameDecider::evolve(&st, e));
        }
        assert_eq!(s.stage, Stage::Ended);
    }

    #[test]
    fn draw_ends_game() {
        let (mut s, x, o) = started();
        // No-win fill: X=0,2,5,7,8  O=1,3,4,6
        for (p, at) in [
            (x, 0),
            (o, 1),
            (x, 2),
            (o, 3),
            (x, 5),
            (o, 4),
            (x, 7),
            (o, 6),
            (x, 8),
        ] {
            let evs =
                GameDecider::decide(&s, &GameCommand::PlaceTile { player_id: p, at }).unwrap();
            s = evs.iter().fold(s, |st, e| GameDecider::evolve(&st, e));
        }
        assert_eq!(s.stage, Stage::Ended);
    }

    #[test]
    fn player_leaves_lobby() {
        let state = GameState::default();
        let p1 = PlayerId(1);
        let events = GameDecider::decide(
            &state,
            &GameCommand::JoinGame {
                player_id: p1,
                name: "Alice".to_string(),
            },
        )
        .unwrap();
        let state = GameDecider::evolve(&state, &events[0]);

        let leave_events =
            GameDecider::decide(&state, &GameCommand::LeaveGame { player_id: p1 }).unwrap();
        assert_eq!(leave_events.len(), 1);
        assert!(matches!(leave_events[0], GameEvent::PlayerLeft { player_id } if player_id == p1));

        // Ensure it didn't end the game because it wasn't InGame
        assert!(
            !leave_events
                .iter()
                .any(|e| matches!(e, GameEvent::GameEnded { .. }))
        );
    }

    #[test]
    fn player_leaves_ingame_ends_game() {
        let (state, p1, _p2) = started();
        let leave_events =
            GameDecider::decide(&state, &GameCommand::LeaveGame { player_id: p1 }).unwrap();

        assert_eq!(leave_events.len(), 2);
        assert!(matches!(leave_events[0], GameEvent::PlayerLeft { player_id } if player_id == p1));
        assert!(
            matches!(leave_events[1], GameEvent::GameEnded { reason: EndGameReason::PlayerLeft { player_id: pid } } if pid == p1)
        );
    }
}
