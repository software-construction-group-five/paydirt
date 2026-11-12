//! AI decision contract. Game validates and applies each intent once.
//!
//! Behavior uses the single `enemy::AiState` enum. Decision selection belongs to
//! enemy-ai; intents do not apply damage or advance turns themselves.

use crate::game::position::Position;

/// What one enemy requests; the caller supplies its floor-local EnemyId.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Take no action this turn.
    Wait,
    /// Request a tile, subject to Game and Level collision validation.
    Move {
        /// Requested destination in map coordinates.
        target: Position,
    },
    /// Request a cardinally adjacent attack against the active player.
    Attack,
}
