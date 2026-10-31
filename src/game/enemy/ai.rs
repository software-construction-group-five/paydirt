//! Enemy decision-making.
//!
//! AiState = which mode an enemy is in (shared by all types).
//! EnemyType -> AiProfile = when it changes mode and how it acts in each mode.
//! `decide()` is read-only and returns an Intent; Game::advance_turn applies it
//! via Level (contract 9) and Player (contract 7).

/// Behavioral mode of an enemy.
pub enum AiState {
    Sleeping,
    Idle,
    Chasing,
    Fleeing,
}

/// Per-EnemyType tuning (sight range, wake chance, flee threshold, movement style).
pub struct AiProfile {
    // TODO: sight_range: i32, wake_chance: f32, flee_below_hp: f32, movement: Movement
}

/// What an enemy wants to do this turn.
pub enum Intent {
    Wait,
    Move, // TODO: carry target Position
    Attack,
}

// TODO: pub fn decide(enemy: &Enemy, player_pos: Position, level: &Level, rng: &mut _)
//           -> (AiState, Intent)
