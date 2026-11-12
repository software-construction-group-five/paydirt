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
use crate::game::stats::StatusEffect;

pub mod ai;

/// A single enemy instance in a Level.
pub struct Enemy {
    kind: EnemyType,
    position: Position,
    stats: Stats,
    ai: AiState,
    // The flee threshold is exclusive: a threshold of 40 means flee at 39 HP or less.
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
                stats: Stats::new(100, 25, 5),
                ai: AiState::Idle,
                flee_below_hp: 40,
            },
            EnemyType::Ghost => Enemy {
                kind,
                position,
                stats: Stats::new(50, 18, 3),
                ai: AiState::Idle,
                flee_below_hp: 25,
            },
            EnemyType::Slime => Enemy {
                kind,
                position,
                stats: Stats::new(30, 12, 2),
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

    /// Attaches an effect to a living enemy without immediately ticking it.
    pub fn add_status_effect(&mut self, effect: StatusEffect) {
        if !self.stats.is_dead() {
            self.stats.add_status_effect(effect);
        }
    }

    /// Applies one effect phase and updates a survivor's health-based behavior.
    /// Game schedules ticks and removes dead enemies through Level.
    pub fn tick_status_effects(&mut self) -> i32 {
        if self.stats.is_dead() {
            return 0;
        }
        let lost = self.stats.tick_status_effects();
        if lost > 0 && !self.stats.is_dead() {
            self.update_status();
        }
        lost
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

    #[test]
    fn effects_use_shared_capacity_and_dead_enemies_do_not_receive_or_tick_effects() {
        use crate::game::stats::StatusEffect;
        let mut enemy = Enemy::new(EnemyType::Slime, Position::new(1, 1));
        for name in ["a", "b", "c", "d"] {
            enemy.add_status_effect(StatusEffect::new(name.into(), 20, 2));
        }
        let names: Vec<_> = enemy
            .stats()
            .status_effects()
            .iter()
            .map(|e| e.name())
            .collect();
        assert_eq!(names, ["b", "c", "d"]);
        assert_eq!(enemy.tick_status_effects(), 30);
        assert!(enemy.stats().is_dead());
        let effects = enemy.stats().status_effects().to_vec();
        enemy.add_status_effect(StatusEffect::new("new".into(), 1, 1));
        assert_eq!(enemy.tick_status_effects(), 0);
        assert_eq!(enemy.stats().status_effects(), effects);
    }

    #[test]
    fn species_use_scaled_health_attack_and_exclusive_flee_thresholds() {
        for (kind, hp, attack, defense, threshold) in [
            (EnemyType::Slime, 30, 12, 2, 0),
            (EnemyType::Ghost, 50, 18, 3, 25),
            (EnemyType::Goblin, 100, 25, 5, 40),
        ] {
            let mut enemy = Enemy::new(kind, Position::new(1, 1));
            assert_eq!(
                (
                    enemy.stats().hp(),
                    enemy.get_roll(),
                    enemy.stats().defense()
                ),
                (hp, attack, defense)
            );
            if threshold > 0 {
                enemy.take_damage(threshold - hp);
                assert_matches!(enemy.ai(), AiState::Attacking);
                enemy.take_damage(-1);
                assert_matches!(enemy.ai(), AiState::Fleeing);
            }
        }
    }
    //Test that get_roll returns the attack value from stats
    #[test]
    fn get_roll_returns_correct_attack() {
        let a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        assert_eq!(a.get_roll(), 25);
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
        a.take_damage(-61); // 100 - 61 = 39, below the exclusive flee threshold.
        assert_eq!(a.stats().hp(), 39); //check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing); //check ai state correctly updated
    }
    //Test that take damage bellow a certain level runs update_status which then changes ai to Fleeing
    #[test]
    fn take_damage_and_update_status_changes_from_attacking_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        a.update_status(); //change enemy state from Idle to Attacking
        assert_matches!(a.ai(), AiState::Attacking); //confirm it switches to attacking
        a.take_damage(-61); // 100 - 61 = 39, below the exclusive flee threshold.
        assert_eq!(a.stats().hp(), 39); //check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing); //check ai state correctly updated to fleeing
    }
}
