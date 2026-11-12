//! Enemy decision-making for SDD Contract 13 (Enemy Update).
//!
//! AI proposes a state and intent without modifying Enemy, Player, or Level.
//! Game supplies movement validation, applies the chosen state and intent,
//! and controls turn progression and combat.

use crate::game::enemy::{AiState, Enemy, EnemyType};
use crate::game::position::Position;

/// Detection tuning for a species (SDD Contract 13).
///
/// Fleeing thresholds belong to `Enemy` and are read from that single source.
#[derive(Debug, Clone, Copy)]
pub struct AiProfile {
    /// Largest Chebyshev distance at which the enemy detects the player.
    pub detection_range: usize,
}

impl AiProfile {
    /// Returns the agreed default profile for an enemy species.
    ///
    /// Detection uses Chebyshev distance; attacks and movement remain cardinal.
    pub fn for_enemy(enemy: &Enemy) -> Self {
        match enemy.kind() {
            EnemyType::Goblin => Self { detection_range: 5 },
            EnemyType::Ghost => Self { detection_range: 6 },
            EnemyType::Slime => Self { detection_range: 3 },
        }
    }
}

/// An action requested by AI; Game decides whether and how to apply it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Take no action this enemy turn.
    Wait,
    /// Request movement to a specific, checked tile.
    Move {
        /// Requested destination in map coordinates.
        target: Position,
    },
    /// Request an attack against the player.
    Attack,
}

/// SDD Contract 13: choose the next behavior and action without mutation.
///
/// `can_move` is supplied by Game and must reject blocked terrain, occupied
/// tiles (including the player), and out-of-bounds destinations. The function
/// checks alternatives instead of repeatedly requesting an invalid first step.
/// Game must still validate the chosen destination when applying the intent.
/// `can_see_player` must come from Map's current terrain line-of-sight query,
/// including its adjacent-tile exception, rather than cached player-visible flags.
///
/// Behavior priorities: dead/out of sight/out of detection -> idle; detected and below the
/// species HP threshold -> flee; in attack range -> attack; otherwise approach.
/// If no *distance-improving* legal cardinal step exists, request `Wait`.
/// This is intentionally not full pathfinding or obstacle detouring.
pub fn decide<F>(
    enemy: &Enemy,
    player_pos: Position,
    profile: AiProfile,
    can_see_player: bool,
    can_move: F,
) -> (AiState, Intent)
where
    F: Fn(Position) -> bool,
{
    if enemy.stats().is_dead() {
        return (AiState::Idle, Intent::Wait);
    }

    let enemy_pos = *enemy.position();
    let distance = enemy_pos.distance(player_pos);

    if !can_see_player || distance > profile.detection_range {
        return (AiState::Idle, Intent::Wait);
    }

    if enemy.stats().hp() < enemy.flee_below_hp() {
        let intent = choose_step(enemy_pos, player_pos, true, &can_move)
            .map_or(Intent::Wait, |target| Intent::Move { target });
        return (AiState::Fleeing, intent);
    }

    if enemy_pos.is_cardinal_neighbor(player_pos) {
        return (AiState::Attacking, Intent::Attack);
    }

    let intent = choose_step(enemy_pos, player_pos, false, &can_move)
        .map_or(Intent::Wait, |target| Intent::Move { target });
    (AiState::Attacking, intent)
}

/// Select the best *legal* cardinal step toward or away from the player.
///
/// Manhattan distance ranks the four candidate steps. Fixed candidate order
/// provides deterministic ties; invalid candidates are skipped before ranking.
fn choose_step<F>(from: Position, player: Position, flee: bool, can_move: &F) -> Option<Position>
where
    F: Fn(Position) -> bool,
{
    let distance = |position: Position| {
        position
            .x
            .abs_diff(player.x)
            .saturating_add(position.y.abs_diff(player.y))
    };
    let initial = distance(from);
    let mut best: Option<(usize, Position)> = None;

    // Prefer left, right, up, then down when legal candidates tie.
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let Some(candidate) = from.offset(dx, dy) else {
            continue;
        };
        if candidate == player || !can_move(candidate) {
            continue;
        }

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
        AiProfile::for_enemy(&goblin())
    }

    #[test]
    fn terrain_sight_blocks_detection_through_walls_rocks_and_closed_doors() {
        use crate::game::level::map::{Map, TileType};
        let enemy = Enemy::new(EnemyType::Goblin, Position::new(0, 1));
        let player = Position::new(2, 1);
        for (terrain, detected) in [
            (TileType::Wall, false),
            (TileType::Rock, false),
            (TileType::Door { open: false }, false),
            (TileType::Door { open: true }, true),
            (TileType::Water, true),
        ] {
            let mut map = Map::new(3, 3);
            for x in 0..3 {
                map.set(Position::new(x, 1), TileType::Floor);
            }
            map.set(Position::new(1, 1), terrain);
            // No visibility refresh: AI reads geometry, not exploration memory.
            let (state, intent) = decide(
                &enemy,
                player,
                AiProfile::for_enemy(&enemy),
                map.has_line_of_sight(*enemy.position(), player),
                |target| map.is_walkable(target),
            );
            assert_eq!(matches!(state, AiState::Attacking), detected);
            if !detected {
                assert_eq!(intent, Intent::Wait);
            }
            assert!(!map.is_visible(player));
        }
    }

    #[test]
    fn loss_of_sight_ends_chase_or_flee_and_adjacent_corners_remain_visible() {
        use crate::game::level::map::{Map, TileType};
        for state in [AiState::Attacking, AiState::Fleeing] {
            let mut enemy = goblin();
            if matches!(state, AiState::Fleeing) {
                enemy.take_damage(-61);
            }
            enemy.set_ai_state(state);
            let (state, intent) = decide(&enemy, Position::new(4, 3), profile(), false, |_| true);
            assert!(matches!(state, AiState::Idle));
            assert_eq!(intent, Intent::Wait);
        }
        let mut map = Map::new(3, 3);
        let enemy = Enemy::new(EnemyType::Goblin, Position::new(0, 0));
        let player = Position::new(1, 1);
        map.set(*enemy.position(), TileType::Floor);
        map.set(player, TileType::Floor);
        let (state, intent) = decide(
            &enemy,
            player,
            AiProfile::for_enemy(&enemy),
            map.has_line_of_sight(*enemy.position(), player),
            |target| map.is_walkable(target),
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Wait); // Both cardinal approach tiles are blocked.
    }

    #[test]
    fn outside_detection_range_waits() {
        let (state, intent) = decide(&goblin(), Position::new(12, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn detected_enemy_approaches_player() {
        let (state, intent) = decide(&goblin(), Position::new(5, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(4, 3)
            }
        );
    }

    #[test]
    fn adjacent_enemy_requests_attack() {
        let (state, intent) = decide(&goblin(), Position::new(4, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn low_health_enemy_flees() {
        let mut enemy = goblin();
        enemy.take_damage(-61); // Goblin: 100 -> 39 HP; flee below 40.
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Fleeing));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(2, 3)
            }
        );
    }

    #[test]
    fn low_health_enemy_is_idle_outside_detection() {
        let mut enemy = goblin();
        enemy.take_damage(-61);
        let (state, intent) = decide(&enemy, Position::new(12, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn healing_above_threshold_ends_flee_decision() {
        let mut enemy = goblin();
        enemy.take_damage(-61);
        enemy.take_damage(10); // 39 -> 49 HP.
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn dead_enemy_does_not_act() {
        let mut enemy = goblin();
        enemy.take_damage(-100);
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile(), true, |_| true);
        assert!(matches!(state, AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn slime_does_not_flee_at_one_hp() {
        let mut enemy = Enemy::new(EnemyType::Slime, Position::new(3, 3));
        enemy.take_damage(-29);
        let (state, intent) = decide(
            &enemy,
            Position::new(4, 3),
            AiProfile::for_enemy(&enemy),
            true,
            |_| true,
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn flee_at_origin_does_not_underflow_coordinates() {
        let mut enemy = Enemy::new(EnemyType::Goblin, Position::new(0, 0));
        enemy.take_damage(-61);
        let (state, intent) = decide(&enemy, Position::new(0, 1), profile(), true, |_| true);
        assert!(matches!(state, AiState::Fleeing));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(1, 0)
            }
        );
    }

    #[test]
    fn species_profiles_have_agreed_detection_and_flee_values() {
        let goblin_profile = AiProfile::for_enemy(&goblin());
        assert_eq!(goblin_profile.detection_range, 5);
        assert_eq!(goblin().flee_below_hp(), 40);

        let ghost = Enemy::new(EnemyType::Ghost, Position::new(3, 3));
        let ghost_profile = AiProfile::for_enemy(&ghost);
        assert_eq!(ghost_profile.detection_range, 6);
        assert_eq!(ghost.flee_below_hp(), 25);

        let slime = Enemy::new(EnemyType::Slime, Position::new(3, 3));
        let slime_profile = AiProfile::for_enemy(&slime);
        assert_eq!(slime_profile.detection_range, 3);
        assert_eq!(slime.flee_below_hp(), 0);
    }

    #[test]
    fn enemy_detects_player_exactly_at_range_limit() {
        let enemy = goblin();
        let (state, intent) = decide(
            &enemy,
            Position::new(8, 3), // Exactly five tiles away.
            AiProfile::for_enemy(&enemy),
            true,
            |_| true,
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(4, 3)
            }
        );
    }

    #[test]
    fn diagonal_adjacent_player_requires_a_cardinal_step_before_attacking() {
        let enemy = goblin();
        let (state, intent) = decide(
            &enemy,
            Position::new(4, 4),
            AiProfile::for_enemy(&enemy),
            true,
            |_| true,
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(4, 3)
            }
        );
    }

    #[test]
    fn enemy_does_not_flee_at_exact_threshold() {
        let mut enemy = goblin();
        enemy.take_damage(-60); // 100 -> 40 HP; threshold is strictly below 40.
        let (state, intent) = decide(
            &enemy,
            Position::new(4, 3),
            AiProfile::for_enemy(&enemy),
            true,
            |_| true,
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Attack);
    }

    #[test]
    fn same_distance_can_be_detected_by_ghost_but_not_slime() {
        let ghost = Enemy::new(EnemyType::Ghost, Position::new(3, 3));
        let slime = Enemy::new(EnemyType::Slime, Position::new(3, 3));
        let player_pos = Position::new(8, 3);

        let (ghost_state, ghost_intent) = decide(
            &ghost,
            player_pos,
            AiProfile::for_enemy(&ghost),
            true,
            |_| true,
        );
        assert!(matches!(ghost_state, AiState::Attacking));
        assert_eq!(
            ghost_intent,
            Intent::Move {
                target: Position::new(4, 3)
            }
        );

        let (slime_state, slime_intent) = decide(
            &slime,
            player_pos,
            AiProfile::for_enemy(&slime),
            true,
            |_| true,
        );
        assert!(matches!(slime_state, AiState::Idle));
        assert_eq!(slime_intent, Intent::Wait);
    }

    #[test]
    fn blocked_first_approach_tries_another_legal_step() {
        // From (3,3), both (4,3) and (3,4) approach the player at (5,5).
        // The first is blocked but the second is a legal alternative.
        let blocked = Position::new(4, 3);
        let (state, intent) = decide(
            &goblin(),
            Position::new(5, 5),
            profile(),
            true,
            |candidate| candidate != blocked,
        );
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(3, 4)
            }
        );
    }

    #[test]
    fn all_improving_moves_blocked_means_wait() {
        let (state, intent) = decide(&goblin(), Position::new(5, 5), profile(), true, |_| false);
        assert!(matches!(state, AiState::Attacking));
        assert_eq!(intent, Intent::Wait);
    }

    #[test]
    fn fleeing_uses_alternative_when_preferred_tile_blocked() {
        let mut enemy = goblin();
        enemy.take_damage(-61);
        let blocked = Position::new(2, 3);
        let (state, intent) = decide(&enemy, Position::new(4, 3), profile(), true, |candidate| {
            candidate != blocked
        });
        assert!(matches!(state, AiState::Fleeing));
        assert_eq!(
            intent,
            Intent::Move {
                target: Position::new(3, 2)
            }
        );
    }

    #[test]
    fn chosen_ai_state_can_be_applied_to_the_enemy() {
        let mut enemy = goblin();
        let (state, intent) = decide(&enemy, Position::new(12, 3), profile(), true, |_| true);
        enemy.set_ai_state(state);
        assert!(matches!(enemy.ai(), AiState::Idle));
        assert_eq!(intent, Intent::Wait);
    }
}
