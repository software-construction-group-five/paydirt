//! Damage rules and outcomes shared by combat callers and Level (SDD 2, 10, 14).

use crate::game::item::{Item, ItemType};
use rand::{Rng, RngExt};

/// Rolls a 5% equipment drop, choosing uniformly from the highest unlocked tier.
/// Caller removes the enemy once before invoking this; all randomness is supplied.
pub(crate) fn equipment_drop(
    depth: u32,
    catalog: impl IntoIterator<Item = Item>,
    rng: &mut impl Rng,
) -> Option<Item> {
    if rng.random_range(0..20) != 0 {
        return None;
    }
    let mut eligible: Vec<_> = catalog
        .into_iter()
        .filter(|item| {
            matches!(item.kind, ItemType::Weapon | ItemType::Armor)
                && item.level <= depth.saturating_add(1)
        })
        .collect();
    let tier = eligible.iter().map(|item| item.level).max()?;
    eligible.retain(|item| item.level == tier);
    let index = rng.random_range(0..eligible.len());
    Some(eligible.swap_remove(index))
}

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
    fn seeded_drops_choose_only_the_highest_unlocked_equipment_tier() {
        use rand::{SeedableRng, rngs::StdRng};
        let catalog = || {
            vec![
                Item::new(10, "Old pick", ItemType::Weapon, 1),
                Item::new(11, "Iron pick", ItemType::Weapon, 3),
                Item::new(12, "Iron armor", ItemType::Armor, 3),
                Item::new(13, "Deep armor", ItemType::Armor, u32::MAX),
                Item::new(14, "Potion", ItemType::Consumable, u32::MAX),
            ]
        };
        for (depth, allowed) in [
            (0, vec![10]),
            (1, vec![10]),
            (2, vec![11, 12]),
            (u32::MAX, vec![13]),
        ] {
            let mut selected = std::collections::HashSet::new();
            let mut no_drop = false;
            for seed in 0..2000 {
                let drop = equipment_drop(depth, catalog(), &mut StdRng::seed_from_u64(seed));
                assert_eq!(
                    drop,
                    equipment_drop(depth, catalog(), &mut StdRng::seed_from_u64(seed))
                );
                if let Some(item) = drop {
                    assert!(allowed.contains(&item.index));
                    selected.insert(item.index);
                } else {
                    no_drop = true;
                }
            }
            assert!(no_drop);
            assert_eq!(selected, allowed.into_iter().collect());
        }
    }

    #[test]
    fn drops_handle_empty_locked_and_consumable_only_catalogs() {
        use rand::{SeedableRng, rngs::StdRng};
        for seed in 0..200 {
            for items in [
                vec![],
                vec![Item::new(0, "Locked", ItemType::Armor, 2)],
                vec![Item::new(1, "Potion", ItemType::Consumable, 1)],
            ] {
                assert!(equipment_drop(0, items, &mut StdRng::seed_from_u64(seed)).is_none());
            }
        }
    }

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
