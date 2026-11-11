//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Game schedules behavior updates; generation controls species selection.
use crate::game::position::Position;
use crate::game::stats::Stats;

/// A single enemy instance in a Level.
pub struct Enemy {
    kind: EnemyType,
    position: Position,
    stats: Stats,
    ai: AiState,
    // The flee threshold is exclusive: a threshold of 10 means flee at 9 HP or less.
    flee_below_hp: i32,
}

/// Enemy species with fixed base stats; generation controls depth eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyType {
    Goblin,
    Ghost,
    Slime,
}

/// Current enemy behavior; Game decides when enemies act.
#[derive(Debug)]
pub enum AiState {
    Attacking,
    Fleeing,
    Idle,
}

impl Enemy {
    /// Creates an idle enemy with its species' base combat stats.
    pub fn new(kind: EnemyType, position: Position) -> Enemy {
        match kind {
            EnemyType::Goblin => Enemy {
                kind,
                position,
                stats: Stats::new(25, 3, 5),
                ai: AiState::Idle,
                flee_below_hp: 10,
            },
            EnemyType::Ghost => Enemy {
                kind,
                position,
                stats: Stats::new(10, 2, 3),
                ai: AiState::Idle,
                flee_below_hp: 5,
            },
            EnemyType::Slime => Enemy {
                kind,
                position,
                stats: Stats::new(5, 1, 2),
                ai: AiState::Idle,
                flee_below_hp: 0, //slimes are too dumb to run
            },
        }
    }

    /// Returns the enemy species.
    pub fn kind(&self) -> &EnemyType {
        &self.kind
    }

    /// Returns the current map position.
    pub fn position(&self) -> &Position {
        &self.position
    }

    /// Borrows health, attack, defense, and status effects.
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Returns the current behavior state.
    pub fn ai(&self) -> &AiState {
        &self.ai
    }

    /// Sets a position already validated by Level and Game.
    pub fn update_position(&mut self, new_position: Position) {
        self.position = new_position;
    }

    /// Updates behavior using the existing health threshold rules.
    pub fn update_status(&mut self) {
        match self.ai {
            AiState::Attacking => {
                self.ai = if self.stats.hp() >= self.flee_below_hp {
                    AiState::Attacking
                } else {
                    AiState::Fleeing
                }
            } // continue attacking unless below a certain hp
            AiState::Fleeing => self.ai = AiState::Fleeing, //continue fleeing
            AiState::Idle => {
                self.ai = if self.stats.hp() >= self.flee_below_hp {
                    AiState::Attacking
                } else {
                    AiState::Fleeing
                }
            }
        }
    }

    /// Applies a signed health change (negative damages, positive heals), then updates state.
    pub fn take_damage(&mut self, amount: i32) {
        self.stats.update_health(amount);
        self.update_status();
    }

    /// Returns the fixed base attack value; no random roll is performed.
    pub fn get_roll(&self) -> i32 {
        self.stats.attack()
    }
}

////////////////////////////////////////////////////////////////////////////////
// TESTS
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::{AiState, Enemy, EnemyType, Position};
    use std::assert_matches;
    //Test that get_roll returns the attack value from stats
    #[test]
    fn get_roll_returns_correct_attack() {
        let a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        assert_eq!(a.get_roll(), 3);
    }
    //Test that update_status changes ai Attacking
    #[test]
    fn update_status_changes_to_attacking() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        a.update_status();
        assert_matches!(a.ai(), AiState::Attacking);
    }
    //Test that take damage bellow a certain level runs update_status which then changes ai to Fleeing
    #[test]
    fn take_damage_and_update_status_changes_from_idle_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        a.take_damage(-16); //25-16=9 which is when Goblin should first begin to flee
        assert_eq!(a.stats().hp(), 9); //check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing); //check ai state correctly updated
    }
    //Test that take damage bellow a certain level runs update_status which then changes ai to Fleeing
    #[test]
    fn take_damage_and_update_status_changes_from_attacking_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        a.update_status(); //change enemy state from Idle to Attacking
        assert_matches!(a.ai(), AiState::Attacking); //confirm it switches to attacking
        a.take_damage(-16); //25-16=9 which is when Goblin should first begin to flee
        assert_eq!(a.stats().hp(), 9); //check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing); //check ai state correctly updated to fleeing
    }
}
