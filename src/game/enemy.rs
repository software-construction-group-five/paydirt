//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Game schedules behavior updates; generation controls species selection.
//! Decision-making lives in `ai`; Game applies the results.

pub mod ai;

use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};
use std::fmt;

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

impl fmt::Display for EnemyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Goblin => "Goblin",
            Self::Ghost => "Ghost",
            Self::Slime => "Slime",
        })
    }
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
        let (max_hp, attack, defense, flee_below_hp) = match kind {
            EnemyType::Goblin => (100, 25, 5, 40),
            EnemyType::Ghost => (50, 18, 3, 25),
            EnemyType::Slime => (30, 12, 2, 0),
        };
        Self {
            kind,
            position,
            stats: Stats::new(max_hp, attack, defense),
            ai: AiState::Idle,
            flee_below_hp,
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

    /// Updates health-related AI behavior (SDD Contract 13).
    pub fn update_status(&mut self) {
        if self.stats.is_dead() {
            self.ai = AiState::Idle;
        } else if self.stats.hp() < self.flee_below_hp {
            self.ai = AiState::Fleeing;
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

    /// Applies one effect phase and updates health-based behavior, including death.
    /// Game schedules ticks and removes dead enemies through Level.
    pub fn tick_status_effects(&mut self) -> i32 {
        if self.stats.is_dead() {
            return 0;
        }
        let lost = self.stats.tick_status_effects();
        if lost > 0 {
            self.update_status();
        }
        lost
    }

    /// Returns the fixed base attack value; no random roll is performed.
    pub fn get_roll(&self) -> i32 {
        self.stats.attack()
    }

    /// Returns the species' fleeing threshold (SDD Contracts 12 and 13).
    pub fn flee_below_hp(&self) -> i32 {
        self.flee_below_hp
    }

    /// Stores the AI state selected for this turn (SDD Contract 13).
    pub fn set_ai_state(&mut self, state: AiState) {
        self.ai = if self.stats.is_dead() {
            AiState::Idle
        } else {
            state
        };
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
        assert!(matches!(enemy.ai(), AiState::Idle));
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
                enemy.set_ai_state(AiState::Attacking);
                enemy.take_damage(threshold - hp);
                assert_matches!(enemy.ai(), AiState::Attacking);
                enemy.take_damage(-1);
                assert_matches!(enemy.ai(), AiState::Fleeing);
            }
        }
    }
    #[test]
    fn get_roll_returns_correct_attack() {
        let a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        assert_eq!(a.get_roll(), 25);
    }

    #[test]
    fn update_status_does_not_wake_idle_enemy() {
        let mut enemy = Enemy::new(EnemyType::Goblin, Position::new(4, 3));

        enemy.update_status();

        assert!(matches!(enemy.ai(), AiState::Idle));
    }

    #[test]
    fn take_damage_and_update_status_changes_from_idle_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4, 3));
        a.take_damage(-61); // 100 - 61 = 39, below the exclusive flee threshold.
        assert_eq!(a.stats().hp(), 39);
        assert_matches!(a.ai(), AiState::Fleeing);
    }
    #[test]
    fn take_damage_and_update_status_changes_from_attacking_to_fleeing() {
        let mut enemy = Enemy::new(EnemyType::Goblin, Position::new(4, 3));

        // Simulate Game applying the AI's attacking decision.
        enemy.set_ai_state(AiState::Attacking);
        assert!(matches!(enemy.ai(), AiState::Attacking));

        // Goblin starts with 100 HP and flees below 40 HP.
        enemy.take_damage(-61);

        assert_eq!(enemy.stats().hp(), 39);
        assert!(matches!(enemy.ai(), AiState::Fleeing));
    }
}
