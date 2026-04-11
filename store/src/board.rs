//! Board logic for Tic-Tac-Toe.
//!
//! This module provides functions to check for win conditions and whether
//! the board is full.

use crate::state::Symbol;
use crate::state::Tile;

/// All possible winning patterns (3-in-a-row) on a 3x3 grid.
/// The indices refer to the row-major flat array representation.
const WIN_PATTERNS: [[usize; 3]; 8] = [
    [0, 1, 2], //rows
    [3, 4, 5], //rows
    [6, 7, 8], //rows
    [0, 3, 6], //columns
    [1, 4, 7], //columns
    [2, 5, 8], //columns
    [0, 4, 8], //diagonals
    [2, 4, 6], //diagonals
];

/// Checks the board for a winning pattern.
///
/// Returns `Some(Symbol)` of the winner if a win is found, or `None` otherwise.
pub fn winning_symbol(board: &[Tile; 9]) -> Option<Symbol> {
    WIN_PATTERNS.iter().find_map(|&[a, b, c]| {
        if let Tile::Occupied(symbol) = board[a] {
            if board[b] == Tile::Occupied(symbol) && board[c] == Tile::Occupied(symbol) {
                return Some(symbol);
            }
        }
        None
    })
}

/// Checks if the board is completely filled with tiles.
pub fn is_full(board: &[Tile; 9]) -> bool {
    board.iter().all(|&tile| tile != Tile::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(cells: &[(usize, Symbol)]) -> [Tile; 9] {
        let mut board = [Tile::Empty; 9];
        for &(i, s) in cells {
            board[i] = Tile::Occupied(s);
        }
        board
    }
    #[test]
    fn row_win() {
        assert_eq!(
            winning_symbol(&b(&[(0, Symbol::X), (1, Symbol::X), (2, Symbol::X)])),
            Some(Symbol::X)
        );
    }
    #[test]
    fn col_win() {
        assert_eq!(
            winning_symbol(&b(&[(0, Symbol::O), (3, Symbol::O), (6, Symbol::O)])),
            Some(Symbol::O)
        );
    }
    #[test]
    fn diag_win() {
        assert_eq!(
            winning_symbol(&b(&[(0, Symbol::X), (4, Symbol::X), (8, Symbol::X)])),
            Some(Symbol::X)
        );
    }
    #[test]
    fn no_win() {
        assert_eq!(winning_symbol(&b(&[(0, Symbol::X), (1, Symbol::O)])), None);
    }
    #[test]
    fn full() {
        assert!(is_full(&[Tile::Occupied(Symbol::X); 9]));
    }
    #[test]
    fn not_full() {
        assert!(!is_full(&b(&[(0, Symbol::X)])));
    }
}
