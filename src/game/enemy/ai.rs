//! Enemy decision-making for SDD Contract 13 (Enemy Update).
//!
//! This module only proposes behavior. Game validates moves against Level,
//! applies state changes and attacks, and advances the gameplay turn.
//! Range and fleeing rules are supplied by the caller until team-approved
//! per-species profiles are available.

use crate::game::enemy::{AiState, Enemy};
use crate::game::position::Position;

/// Configurable behavior limits for an enemy species.
///
/// The health threshold must agree with the Enemy species definition.
#[derive(Debug, Clone, Copy)]
pub struct AiProfile {
    /// Largest player distance at which the enemy responds.
    pub detection_range: usize,
    /// Largest player distance at which the enemy requests an attack.
    pub attack_range: usize,
    /// The enemy flees when its HP is strictly below this value.
    pub flee_below_hp: i32,
}

/// An action the AI requests; Game decides whether it can happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Take no action.
    Wait,
    /// Attempt to move to the given tile.
    Move(Position),
    /// Request an attack against the player.
    Attack,
}

/// SDD Contract 13: propose the next AI state and action without mutation.
///
/// A dead or undetected enemy waits. A detected, low-health enemy flees.
/// Otherwise it attacks within range or approaches the player.
/// Game must validate the target tile using Level Contract 9 before movement.
pub fn decide(enemy: &Enemy, player_pos: Position, profile: AiProfile) -> (AiState, Intent) {
    if enemy.stats().is_dead() {
        return (AiState::Idle, Intent::Wait);
    }

    let enemy_pos = *enemy.position();
    let distance = enemy_pos.distance(player_pos);

    if distance > profile.detection_range {
        return (AiState::Idle, Intent::Wait);
    }

    if enemy.stats().hp() < profile.flee_below_hp {
        let intent = choose_step(enemy_pos, player_pos, true).map_or(Intent::Wait, Intent::Move);
        return (AiState::Fleeing, intent);
    }

    if distance <= profile.attack_range {
        return (AiState::Attacking, Intent::Attack);
    }

    let intent = choose_step(enemy_pos, player_pos, false).map_or(Intent::Wait, Intent::Move);
    (AiState::Attacking, intent)
}

/// Choose a cardinal step that increases or decreases player distance.
///
/// Only nonnegative coordinates are checked here. Game checks map bounds,
/// terrain, the player tile, and other enemies before applying the move.
fn choose_step(from: Position, player: Position, flee: bool) -> Option<Position> {
    let distance = |position: Position| {
        position
            .x
            .abs_diff(player.x)
            .saturating_add(position.y.abs_diff(player.y))
    };
    let initial = distance(from);
    let mut best: Option<(usize, Position)> = None;

    // Fixed order makes unit tests deterministic when two steps tie.
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let Some(candidate) = from.offset(dx, dy) else {
            continue;
        };
        let candidate_distance = distance(candidate);
        let improves = if flee {
            candidate_distance > initial
        } else {
            candidate_distance < initial
        };
        let is_better = match best {
            None => true,
            Some((best_distance, _)) if flee => candidate_distance > best_distance,
            Some((best_distance, _)) => candidate_distance < best_distance,
        };

        if improves && is_better {
            best = Some((candidate_distance, candidate));
        }
    }

    best.map(|(_, position)| position)
}

#[cfg(test)]
mod tests {
    use super::{AiProfile, Intent, decide};
    use crate::game::enemy::{AiState, Enemy, EnemyType};
    use crate::game::position::Position;

    fn goblin() -> Enemy {
        Enemy::new(EnemyType::Goblin, Position::new(3, 3))
    }

    fn profile() -> AiProfile {
        // Test configuration; species-wide ranges are not yet finalized.
        AiProfile {
            detection_range: 4,
            attack_range: 1,
            flee_below_hp: 10,
        }
    }

    #[test]
    fn outside_detection_range_waits() {
        let (state, intent) = decide(&goblin(), Position::new(12, 3), profile());
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn detected_enemy_approaches_player() {
        let (state, intent) = decide(&goblin(), Position::new(5, 3), profile());
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Move(Position::new(4, 3)));
    }

    #[test]
    fn adjacent_enemy_requests_attack() {
        let (state, intent) = decide(&goblin(), Position::new(4, 3), profile());
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn low_health_enemy_flees() {
        let mut enemy = goblin();
        enemy.take_damage(-16); // Goblin: 25 -> 9 HP, below threshold 10.
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile());
        assert!(matches!(state, AiState::Fleeing));
        assert_eq!(intent, Intent::Move(Position::new(2, 3)));
    }

    #[test]
    fn low_health_enemy_is_idle_outside_detection() {
        let mut enemy = goblin();
        enemy.take_damage(-16);
        let (state, intent) = decide(&enemy, Position::new(12, 3), profile());
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn healing_above_threshold_ends_flee_decision() {
        let mut enemy = goblin();
        enemy.take_damage(-16);
        enemy.take_damage(10); // 9 -> 19 HP.
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile());
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn dead_enemy_does_not_act() {
        let mut enemy = goblin();
        enemy.take_damage(-100);
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile());
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn slime_does_not_flee_at_one_hp() {
        let mut enemy = Enemy::new(EnemyType::Slime, Position::new(3, 3));
        enemy.take_damage(-4);
        let mut config = profile();
        config.flee_below_hp = 0;
        let (state, intent) = decide(&enemy, Position::new(4, 3), config);
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn flee_at_origin_does_not_underflow_coordinates() {
        let mut enemy = Enemy::new(EnemyType::Goblin, Position::new(0, 0));
        enemy.take_damage(-16);
        let (state, intent) = decide(&enemy, Position::new(0, 1), profile());
        assert!(matches!(state, AiState::Fleeing));
        assert_eq!(intent, Intent::Move(Position::new(1, 0)));
    }
}
