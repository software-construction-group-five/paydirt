//! Collectible items (SDD 3.7).
//!
//! Contract 18 Get Specific Item: look up an item by index.

use crate::game::{
    combat::{DamageDice, Die},
    stats::{EffectKind, EffectSpec},
};

use rand::{Rng, RngExt};

/// Immutable equipment bonuses and reusable weapon effect data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CombatModifiers {
    damage: Option<DamageDice>,
    defense: i32,
    bonus_damage: u32,
    on_hit: Option<EffectSpec>,
}

impl CombatModifiers {
    /// Creates weapon damage dice and an optional on-hit effect.
    pub fn weapon(damage: DamageDice, on_hit: Option<EffectSpec>) -> Self {
        Self {
            damage: Some(damage),
            defense: 0,
            bonus_damage: 0,
            on_hit,
        }
    }

    /// Creates an armor modifier with nonnegative defense.
    pub fn armor(defense: i32) -> Self {
        assert!(defense >= 0, "Defense bonus cannot be negative.");
        Self {
            damage: None,
            defense,
            bonus_damage: 0,
            on_hit: None,
        }
    }

    /// Returns weapon damage dice, if present.
    pub fn damage(&self) -> Option<DamageDice> {
        self.damage
    }

    /// Flat damage added to each attack roll.
    pub fn bonus_damage(&self) -> u32 {
        self.bonus_damage
    }

    /// Damage expression shown in inventory and status, including any rare effect.
    pub fn damage_label(&self) -> Option<String> {
        self.damage.map(|dice| {
            let bonus = if self.bonus_damage == 0 {
                String::new()
            } else {
                format!("+{}", self.bonus_damage)
            };
            let effect = self.on_hit.as_ref().map_or(String::new(), |effect| {
                format!(" {}", effect.kind().map_or("Effect", EffectKind::name))
            });
            format!("{dice}{bonus}{effect}")
        })
    }

    /// Returns the additive defense bonus.
    pub fn defense(&self) -> i32 {
        self.defense
    }

    /// Borrows the reusable on-hit effect specification.
    pub fn on_hit(&self) -> Option<&EffectSpec> {
        self.on_hit.as_ref()
    }
}

/// The three item categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Weapon,
    Armor,
    Consumable,
}

/// An object containing the information for an item that may be collected during the game.
///
/// Knows the item's index, name, type (weapon, armor, or consumable),
/// and the game level it spawns on.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Index used to look up and catalog the item.
    pub index: u32,
    /// Name shown to the player.
    pub name: String,
    /// Whether the item is a weapon, armor, or consumable.
    pub kind: ItemType,
    /// The game level the item spawns on.
    pub level: u32,
    combat: CombatModifiers,
    healing: Option<i32>,
}

impl Item {
    /// Creates a new Item.
    pub fn new(index: u32, name: &str, kind: ItemType, level: u32) -> Item {
        Item {
            index,
            name: name.to_string(),
            kind,
            level,
            combat: CombatModifiers::default(),
            healing: None,
        }
    }

    /// Sets equipment data, rejecting modifiers incompatible with the category.
    pub fn with_combat(mut self, combat: CombatModifiers) -> Self {
        assert!(
            match self.kind {
                ItemType::Weapon => combat.defense == 0,
                ItemType::Armor => combat.damage.is_none() && combat.on_hit.is_none(),
                ItemType::Consumable => combat == CombatModifiers::default(),
            },
            "Combat modifiers must match the item category."
        );
        self.combat = combat;
        self
    }

    /// Borrows equipment data without creating or advancing a status effect.
    pub fn combat(&self) -> &CombatModifiers {
        &self.combat
    }

    /// Rolls a newly spawned weapon once; transfers must preserve this owned value.
    /// Each four depths adds one guaranteed damage; a random bonus ranges from zero
    /// through two plus that depth tier. Scaling is capped to keep arithmetic bounded.
    pub fn roll_spawn(mut self, depth: u32, rng: &mut impl Rng) -> Self {
        if self.kind != ItemType::Weapon {
            return self;
        }
        let Some(dice) = self.combat.damage else {
            return self;
        };
        let base_bonus = self.combat.bonus_damage;
        let tier = (depth / 4).min(1000);
        self.combat.bonus_damage = self
            .combat
            .bonus_damage
            .saturating_add(tier + rng.random_range(0..=2 + tier));
        if rng.random_range(0..100) == 0 {
            let kind = match rng.random_range(0..3) {
                0 => EffectKind::Bleed,
                1 => EffectKind::Weakness,
                _ => EffectKind::Lightning,
            };
            // Base damage excludes the randomized depth bonus and critical attack rolls.
            let power = ((dice.minimum() + dice.maximum()) / 2 + base_bonus)
                .div_ceil(4)
                .max(1) as i32;
            self.combat.on_hit = Some(EffectSpec::weapon(kind, power, 3));
        }
        self
    }

    /// Sets positive consumable healing without affecting equipment modifiers.
    pub fn with_healing(mut self, amount: i32) -> Self {
        assert!(
            self.kind == ItemType::Consumable && amount > 0,
            "Healing requires a consumable and positive HP."
        );
        self.healing = Some(amount);
        self
    }

    /// Healing supplied by this consumable, if it has a supported use effect.
    pub fn healing(&self) -> Option<i32> {
        self.healing
    }
}

// TODO(Robert): replace placeholder items with final item list.
// Catalog order defines the existing public item indices.
struct CatalogEntry {
    name: &'static str,
    kind: ItemType,
    level: u32,
    damage: Option<DamageDice>,
    defense: i32,
    healing: Option<i32>,
    bonus_damage: u32,
}

const CATALOG: [CatalogEntry; 4] = [
    CatalogEntry {
        name: "Pickaxe",
        kind: ItemType::Weapon,
        level: 1,
        damage: Some(DamageDice::new(2, Die::D4)),
        defense: 0,
        healing: None,
        bonus_damage: 0,
    },
    CatalogEntry {
        name: "Leather Armor",
        kind: ItemType::Armor,
        level: 1,
        damage: None,
        defense: 1,
        healing: None,
        bonus_damage: 0,
    },
    CatalogEntry {
        name: "Health Potion",
        kind: ItemType::Consumable,
        level: 1,
        damage: None,
        defense: 0,
        healing: Some(12),
        bonus_damage: 0,
    },
    CatalogEntry {
        name: "Shovel",
        kind: ItemType::Weapon,
        level: 1,
        damage: Some(DamageDice::new(2, Die::D4)),
        defense: 0,
        healing: None,
        bonus_damage: 2,
    },
];

impl CatalogEntry {
    fn item(&self, index: u32) -> Item {
        let combat = CombatModifiers {
            damage: self.damage,
            defense: self.defense,
            bonus_damage: self.bonus_damage,
            on_hit: None,
        };
        let item = Item::new(index, self.name, self.kind, self.level).with_combat(combat);
        if let Some(healing) = self.healing {
            item.with_healing(healing)
        } else {
            item
        }
    }
}

/// Iterates the complete catalog in index order, constructing independent items.
/// Generation uses this list so new catalog entries need no separate spawn list.
pub fn catalog() -> impl Iterator<Item = Item> {
    CATALOG
        .iter()
        .enumerate()
        .map(|(index, entry)| entry.item(index as u32))
}

/// Accepts an item index and returns the matching Item (Contract 18).
///
/// Panics if no item has that index.
pub fn get_item(index: u32) -> Item {
    let entry = usize::try_from(index)
        .ok()
        .and_then(|index| CATALOG.get(index));
    entry
        .unwrap_or_else(|| panic!("No item with index {index}"))
        .item(index)
}

#[cfg(test)]
mod tests {
    use super::{ItemType, catalog, get_item};

    #[test]
    fn catalog_modifiers_match_initial_equipment_balance() {
        assert_eq!(
            get_item(0).combat().damage(),
            Some(super::DamageDice::new(2, super::Die::D4))
        );
        assert_eq!(get_item(1).combat().defense(), 1);
        assert_eq!(get_item(2).combat(), &super::CombatModifiers::default());
        assert!(catalog().all(|item| item.combat().on_hit().is_none()));
    }

    #[test]
    #[should_panic(expected = "Combat modifiers must match the item category")]
    fn armor_rejects_weapon_effects() {
        get_item(1).with_combat(super::CombatModifiers::weapon(
            super::DamageDice::new(1, super::Die::D4),
            Some(crate::game::stats::EffectSpec::new("Poison".into(), 2, 3)),
        ));
    }

    #[test]
    fn catalog_lookup_preserves_indices_and_returns_independent_items() {
        let items: Vec<_> = catalog().collect();
        assert_eq!(items.len(), 4);
        for (index, item) in items.iter().enumerate() {
            assert_eq!(item.index, index as u32);
            assert_eq!(*item, get_item(item.index));
        }
        let mut carried = get_item(0);
        carried.name = "Modified pickaxe".to_owned();
        assert_eq!(catalog().next().unwrap().name, "Pickaxe");
    }

    #[test]
    fn spawned_weapons_scale_replay_and_rarely_receive_each_effect() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut effects = [0; 3];
        let mut bonuses = std::collections::HashSet::new();
        for seed in 0..10000 {
            let rolled = get_item(3).roll_spawn(12, &mut StdRng::seed_from_u64(seed));
            assert_eq!(
                rolled,
                get_item(3).roll_spawn(12, &mut StdRng::seed_from_u64(seed))
            );
            assert!((5..=10).contains(&rolled.combat().bonus_damage()));
            bonuses.insert(rolled.combat().bonus_damage());
            if let Some(effect) = rolled.combat().on_hit() {
                let index = match effect.kind().unwrap() {
                    super::EffectKind::Bleed => 0,
                    super::EffectKind::Weakness => 1,
                    super::EffectKind::Lightning => 2,
                };
                effects[index] += 1;
                assert_eq!(effect.power(), 2);
            }
            let deep = get_item(0).roll_spawn(u32::MAX, &mut StdRng::seed_from_u64(seed));
            assert!((1000..=2002).contains(&deep.combat().bonus_damage()));
            assert_eq!(
                get_item(1),
                get_item(1).roll_spawn(20, &mut StdRng::seed_from_u64(seed))
            );
        }
        assert_eq!(bonuses.len(), 6);
        assert!(effects.iter().all(|&count| count > 10));
        assert!((50..150).contains(&effects.iter().sum::<usize>()));
        assert_eq!(get_item(3).combat().damage_label().unwrap(), "2d4+2");
        assert_eq!(get_item(0).combat().damage_label().unwrap(), "2d4");
    }

    // TC IT 09
    #[test]
    fn get_item_returns_matching_item() {
        let item = get_item(2);
        assert_eq!(item.index, 2);
        assert_eq!(item.name, "Health Potion");
        assert_eq!(item.kind, ItemType::Consumable);
        assert_eq!(item.level, 1);
    }

    // TC IT 10
    #[test]
    #[should_panic(expected = "No item with index 99")]
    fn get_item_with_bad_index_panics() {
        get_item(99);
    }
}
