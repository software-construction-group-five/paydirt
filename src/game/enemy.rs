//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Decision-making lives in `ai`; Game applies the results.
mod enemy{
    pub mod ai;
}
mod position;
mod stats;


//use enemy::ai::{AiState, AiProfile, Intent};
// pub mod stats::Stats;
// pub mod position::Position;
// pub mod ai::AiState;
    
/// A single enemy instance in a Level.
pub struct Enemy {
    // TODO: id: usize, kind: EnemyType, position: Position, stats: Stats, ai: AiState
    id: i32,
    kind: EnemyType,
    position: position::Position,
    stats: stats::Stats,
    ai: enemy::ai::AiState,
}

/// Enemy species. Each maps to an AiProfile that drives divergent behavior.
pub enum EnemyType {
	// TODO: Come up with some enemy types
}
