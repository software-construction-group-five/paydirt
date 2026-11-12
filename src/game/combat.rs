//! Damage rules and outcomes shared by combat callers and Level (SDD 2, 10, 14).

use crate::game::item::{Item, ItemType, catalog, choose_loot, get_item};
use rand::{Rng, RngExt};
use std::fmt;

/// Supported damage dice; other die sizes cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Die {
    /// Four-sided die.
    D4,
    /// Six-sided die.
    D6,
    /// Twelve-sided die.
    D12,
}

impl Die {
    /// Number of equally likely faces.
    pub const fn sides(self) -> u32 {
        match self {
            Self::D4 => 4,
            Self::D6 => 6,
            Self::D12 => 12,
        }
    }
}

/// A positive number of identical damage dice, such as 2d4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageDice {
    count: u8,
    die: Die,
}

impl DamageDice {
    /// Creates a pool of 1–255 supported dice. Panics for zero dice.
    pub const fn new(count: u8, die: Die) -> Self {
        assert!(count > 0, "Damage requires at least one die.");
        Self { count, die }
    }

    /// Rolls every die independently with the caller's RNG.
    pub fn roll(self, rng: &mut impl Rng) -> u32 {
        (0..self.count)
            .map(|_| rng.random_range(1..=self.die.sides()))
            .sum()
    }

    /// Minimum possible total.
    pub const fn minimum(self) -> u32 {
        self.count as u32
    }

    /// Maximum possible total.
    pub const fn maximum(self) -> u32 {
        self.count as u32 * self.die.sides()
    }
}

impl fmt::Display for DamageDice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d{}", self.count, self.die.sides())
    }
}

const EQUIPMENT_DROP_ONE_IN: u32 = 20;
const HEALTH_POTION_DROP_ONE_IN: u32 = 5;

/// Rolls equipment and an independent 20% health potion drop for one defeat.
/// Both may drop on the same tile; fuel never drops from enemies.
pub(crate) fn enemy_drops(depth: u32, rng: &mut impl Rng) -> impl Iterator<Item = Item> {
    let equipment = equipment_drop(depth, catalog(), rng);
    let potion = (rng.random_range(0..HEALTH_POTION_DROP_ONE_IN) == 0).then(|| get_item(2));
    [equipment, potion].into_iter().flatten()
}

/// Rolls a 5% equipment drop, with 5% of successful drops allocated to armor.
/// Weapons and armor each use their highest unlocked category tier.
/// Caller removes the enemy once before invoking this; all randomness is supplied.
pub(crate) fn equipment_drop(
    depth: u32,
    catalog: impl IntoIterator<Item = Item>,
    rng: &mut impl Rng,
) -> Option<Item> {
    if rng.random_range(0..EQUIPMENT_DROP_ONE_IN) != 0 {
        return None;
    }
    let unlocked_level = depth.saturating_add(1);
    let mut eligible: Vec<_> = catalog
        .into_iter()
        .filter(|item| {
            matches!(item.kind, ItemType::Weapon | ItemType::Armor) && item.level <= unlocked_level
        })
        .collect();
    let weapon_tier = eligible
        .iter()
        .filter(|item| item.kind == ItemType::Weapon)
        .map(|item| item.level)
        .max();
    eligible.retain(|item| item.kind == ItemType::Armor || Some(item.level) == weapon_tier);
    choose_loot(&eligible, rng).map(|item| item.roll_spawn(depth, rng))
}

/// Applies defense to a rolled attack total, with a minimum of one damage.
///
/// Roll exactly once using Player::roll_attack or Enemy::get_roll, then pass that
/// total here. Panics for negative defense, matching Stats' invariants.
pub fn attack_damage(roll: u32, defense: i32) -> u32 {
    assert!(defense >= 0, "Defense cannot be negative.");
    roll.saturating_sub(defense as u32).max(1)
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
    fn enemy_drops_include_potions_and_preserve_equipment_rarity() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(42);
        let (mut potions, mut equipment, mut combined) = (0, 0, 0);
        for _ in 0..40000 {
            let drops: Vec<_> = enemy_drops(0, &mut rng).collect();
            combined += usize::from(drops.len() == 2);
            for item in drops {
                assert!(!item.is_lamp_fuel());
                if item.kind == ItemType::Consumable {
                    assert_eq!(item, get_item(2));
                    potions += 1;
                } else {
                    equipment += 1;
                }
            }
        }
        assert!((7500..8500).contains(&potions));
        assert!((1700..2300).contains(&equipment));
        assert!(combined > 0);
    }

    #[test]
    fn helmets_remain_rare_drops_after_unlocking_new_tiers() {
        use rand::{SeedableRng, rngs::StdRng};
        for depth in [0_u32, 3, 6, 9, 12, u32::MAX] {
            let mut rng = StdRng::seed_from_u64(51);
            let (mut drops, mut helmets, mut weapons) = (0, 0, 0);
            for _ in 0..40000 {
                if let Some(item) = equipment_drop(depth, crate::game::item::catalog(), &mut rng) {
                    drops += 1;
                    if item.kind == ItemType::Armor {
                        helmets += 1;
                        assert_eq!(item.combat().defense(), 1 + (depth / 3).min(4) as i32);
                    } else {
                        weapons += 1;
                    }
                }
            }
            assert!((1700..2300).contains(&drops));
            assert!((60..140).contains(&helmets));
            assert!(weapons > helmets * 10);
        }
    }

    #[test]
    fn supported_dice_roll_independent_faces_with_seeded_replay() {
        use rand::{SeedableRng, rngs::StdRng};
        for die in [Die::D4, Die::D6, Die::D12] {
            for count in [1, 2, 3, u8::MAX] {
                let dice = DamageDice::new(count, die);
                let mut rng = StdRng::seed_from_u64(42);
                let mut replay = StdRng::seed_from_u64(42);
                for _ in 0..100 {
                    let roll = dice.roll(&mut rng);
                    let individual_faces: u32 = (0..count)
                        .map(|_| replay.random_range(1..=die.sides()))
                        .sum();
                    assert_eq!(roll, individual_faces);
                    assert!((dice.minimum()..=dice.maximum()).contains(&roll));
                }
                assert_eq!(dice.to_string(), format!("{count}d{}", die.sides()));
            }
        }
    }

    #[test]
    #[should_panic(expected = "Damage requires at least one die")]
    fn zero_dice_are_rejected() {
        DamageDice::new(0, Die::D4);
    }

    #[test]
    fn seeded_drops_choose_highest_unlocked_tier_per_category() {
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
            (u32::MAX, vec![11, 13]),
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
            (u32::MAX, 0, u32::MAX),
            (i32::MAX as u32, i32::MAX, 1),
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
            Stats::new(40, 0, 1),
            Inventory::new(),
            equipment,
        );
        for (attack, unarmored, armored) in [(4, 3, 2), (6, 5, 4), (12, 11, 10)] {
            assert_eq!(
                attack_damage(attack, player.get_stats().defense()),
                unarmored
            );
            assert_eq!(attack_damage(attack, player.effective_defense()), armored);
        }
    }

    #[test]
    #[should_panic(expected = "Defense cannot be negative")]
    fn negative_defense_is_rejected() {
        attack_damage(0, -1);
    }
}
