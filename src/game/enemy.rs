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

use crate::game::combat::{DamageDice, Die};
use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};
use rand::Rng;
use std::fmt;

/// A single enemy instance in a Level.
pub struct Enemy {
    kind: EnemyType,
    position: Position,
    stats: Stats,
    ai: AiState,
    // The flee threshold is exclusive: a threshold of 9 means flee at 8 HP or less.
    flee_below_hp: i32,
}

/// Enemy species with base stats and damage dice; generation controls depth eligibility.
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
        let (max_hp, defense, flee_below_hp) = match kind {
            EnemyType::Goblin => (24, 2, 9),
            EnemyType::Ghost => (14, 1, 6),
            EnemyType::Slime => (8, 0, 0),
        };
        Self {
            kind,
            position,
            stats: Stats::new(max_hp, 0, defense),
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

    /// Species damage dice, independent of health and behavior state.
    pub fn damage_dice(&self) -> DamageDice {
        DamageDice::new(
            1,
            match self.kind {
                EnemyType::Slime => Die::D4,
                EnemyType::Ghost => Die::D6,
                EnemyType::Goblin => Die::D12,
            },
        )
    }

    /// Rolls an attack with the supplied RNG; defense is applied by combat.
    pub fn get_roll(&self, rng: &mut impl Rng) -> u32 {
        self.damage_dice()
            .roll(rng)
            .saturating_add(self.stats.attack() as u32)
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
        assert_eq!(enemy.tick_status_effects(), 8);
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
            (EnemyType::Slime, 8, 0, 0, 0),
            (EnemyType::Ghost, 14, 0, 1, 6),
            (EnemyType::Goblin, 24, 0, 2, 9),
        ] {
            let mut enemy = Enemy::new(kind, Position::new(1, 1));
            assert_eq!(
                (
                    enemy.stats().hp(),
                    enemy.stats().attack(),
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
    fn enemy_rolls_use_species_dice_and_the_supplied_rng() {
        use rand::{SeedableRng, rngs::StdRng};
        for (kind, maximum) in [
            (EnemyType::Slime, 4),
            (EnemyType::Ghost, 6),
            (EnemyType::Goblin, 12),
        ] {
            let enemy = Enemy::new(kind, Position::new(4, 3));
            let mut rng = StdRng::seed_from_u64(42);
            let mut replay = StdRng::seed_from_u64(42);
            let rolls: Vec<_> = (0..100).map(|_| enemy.get_roll(&mut rng)).collect();
            assert!(rolls.iter().all(|&roll| (1..=maximum).contains(&roll)));
            assert_eq!(
                rolls,
                (0..100)
                    .map(|_| enemy.get_roll(&mut replay))
                    .collect::<Vec<_>>()
            );
            assert!(rolls.windows(2).any(|pair| pair[0] != pair[1]));
        }
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
        a.take_damage(-16); // 24 - 16 = 8, below the exclusive flee threshold.
        assert_eq!(a.stats().hp(), 8);
        assert_matches!(a.ai(), AiState::Fleeing);
    }
    #[test]
    fn take_damage_and_update_status_changes_from_attacking_to_fleeing() {
        let mut enemy = Enemy::new(EnemyType::Goblin, Position::new(4, 3));

        // Simulate Game applying the AI's attacking decision.
        enemy.set_ai_state(AiState::Attacking);
        assert!(matches!(enemy.ai(), AiState::Attacking));

        // Goblin starts with 24 HP and flees below 9 HP.
        enemy.take_damage(-16);

        assert_eq!(enemy.stats().hp(), 8);
        assert!(matches!(enemy.ai(), AiState::Fleeing));
    }
}
