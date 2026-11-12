//! Damage rules and outcomes shared by combat callers and Level (SDD 2, 10, 14).

/// Calculates deterministic attack damage from nonnegative effective stats.
///
/// Defense cannot reduce an attack below one damage. Saturating subtraction
/// avoids overflow; equipment bonuses must already be included by the caller.
/// Panics if either stat is negative, matching Stats' construction invariants.
pub fn attack_damage(attack: i32, defense: i32) -> u32 {
    assert!(attack >= 0, "Attack cannot be negative.");
    assert!(defense >= 0, "Defense cannot be negative.");
    attack.saturating_sub(defense).max(1) as u32
}

/// Result of one direct-damage or effect update; does not remove the defender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageOutcome {
    /// Actual HP lost, capped at the defender's health before the update.
    pub hp_lost: i32,
    /// Whether the defender is dead afterward, including an already-dead target.
    pub is_dead: bool,
    /// Whether this update attached a fresh on-hit effect to a surviving target.
    pub effect_applied: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        equipment::Equipment, inventory::Inventory, item::get_item, player::Player,
        position::Position, stats::Stats,
    };

    #[test]
    fn damage_has_a_minimum_and_handles_extreme_valid_stats() {
        for (attack, defense, expected) in [
            (15, 5, 10),
            (0, 0, 1),
            (5, 5, 1),
            (0, i32::MAX, 1),
            (i32::MAX, 0, i32::MAX as u32),
            (i32::MAX, i32::MAX, 1),
        ] {
            assert_eq!(attack_damage(attack, defense), expected);
        }
    }

    #[test]
    fn equipped_armor_reduces_damage_for_each_species() {
        let mut equipment = Equipment::new();
        equipment.equip(get_item(1)).unwrap();
        let player = Player::new(
            Position::new(1, 1),
            Stats::new(200, 15, 5),
            Inventory::new(),
            equipment,
        );
        for (attack, unarmored, armored) in [(12, 7, 4), (18, 13, 10), (25, 20, 17)] {
            assert_eq!(
                attack_damage(attack, player.get_stats().defense()),
                unarmored
            );
            assert_eq!(attack_damage(attack, player.effective_defense()), armored);
        }
    }

    #[test]
    #[should_panic(expected = "Attack cannot be negative")]
    fn negative_attack_is_rejected() {
        attack_damage(-1, 0);
    }

    #[test]
    #[should_panic(expected = "Defense cannot be negative")]
    fn negative_defense_is_rejected() {
        attack_damage(0, -1);
    }
}
