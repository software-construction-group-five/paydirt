//! Collectible items (SDD 3.7).
//!
//! Contract 18 Get Specific Item: look up an item by index.

use crate::game::{
    combat::{DamageDice, Die},
    stats::EffectSpec,
};

/// Immutable equipment bonuses and reusable weapon effect data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CombatModifiers {
    damage: Option<DamageDice>,
    defense: i32,
    on_hit: Option<EffectSpec>,
}

impl CombatModifiers {
    /// Creates weapon damage dice and an optional on-hit effect.
    pub fn weapon(damage: DamageDice, on_hit: Option<EffectSpec>) -> Self {
        Self {
            damage: Some(damage),
            defense: 0,
            on_hit,
        }
    }

    /// Creates an armor modifier with nonnegative defense.
    pub fn armor(defense: i32) -> Self {
        assert!(defense >= 0, "Defense bonus cannot be negative.");
        Self {
            damage: None,
            defense,
            on_hit: None,
        }
    }

    /// Returns weapon damage dice, if present.
    pub fn damage(&self) -> Option<DamageDice> {
        self.damage
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
}

// TODO(Robert): replace placeholder items with final item list.
// Catalog order defines the existing public item indices.
const CATALOG: [(&str, ItemType, u32, Option<DamageDice>, i32); 3] = [
    (
        "Pickaxe",
        ItemType::Weapon,
        1,
        Some(DamageDice::new(2, Die::D4)),
        0,
    ),
    ("Leather Armor", ItemType::Armor, 1, None, 1),
    ("Health Potion", ItemType::Consumable, 1, None, 0),
];

/// Iterates the complete catalog in index order, constructing independent items.
/// Generation uses this list so new catalog entries need no separate spawn list.
pub fn catalog() -> impl Iterator<Item = Item> {
    CATALOG
        .iter()
        .enumerate()
        .map(|(index, &entry)| catalog_item(index as u32, entry))
}

/// Accepts an item index and returns the matching Item (Contract 18).
///
/// Panics if no item has that index.
pub fn get_item(index: u32) -> Item {
    let entry = usize::try_from(index)
        .ok()
        .and_then(|index| CATALOG.get(index));
    catalog_item(
        index,
        *entry.unwrap_or_else(|| panic!("No item with index {index}")),
    )
}

fn catalog_item(
    index: u32,
    (name, kind, level, damage, defense): (&str, ItemType, u32, Option<DamageDice>, i32),
) -> Item {
    let combat = CombatModifiers {
        damage,
        defense,
        on_hit: None,
    };
    Item::new(index, name, kind, level).with_combat(combat)
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
        assert_eq!(items.len(), 3);
        for (index, item) in items.iter().enumerate() {
            assert_eq!(item.index, index as u32);
            assert_eq!(*item, get_item(item.index));
        }
        let mut carried = get_item(0);
        carried.name = "Modified pickaxe".to_owned();
        assert_eq!(catalog().next().unwrap().name, "Pickaxe");
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
