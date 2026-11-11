//! Items the player currently has equipped (weapon and armor slots).
//!
//! Modified via Player contract 7 (update_equipped).

use crate::game::item::{Item, ItemType};

/// Keeps track of the player's equipped items: one weapon slot and one armor slot.
///
/// Owned by Player. An empty slot is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Equipment {
    /// The equipped weapon, if any.
    pub weapon: Option<Item>,
    /// The equipped armor, if any.
    pub armor: Option<Item>,
}

impl Equipment {
    /// Creates an Equipment with both slots empty.
    pub fn new() -> Equipment {
        Equipment {
            weapon: None,
            armor: None,
        }
    }

    /// Equips a weapon or armor in its matching slot, replacing any item already there.
    ///
    /// Returns true if the item was equipped, or false if it is a consumable.
    pub fn equip(&mut self, item: Item) -> bool {
        match item.kind {
            ItemType::Weapon => {
                self.weapon = Some(item);
                true
            }
            ItemType::Armor => {
                self.armor = Some(item);
                true
            }
            // Consumables are used up, not worn, so they can't be equipped.
            ItemType::Consumable => false,
        }
    }

    /// Removes and returns the equipped weapon, or None if the slot is empty.
    pub fn unequip_weapon(&mut self) -> Option<Item> {
        self.weapon.take()
    }

    /// Removes and returns the equipped armor, or None if the slot is empty.
    pub fn unequip_armor(&mut self) -> Option<Item> {
        self.armor.take()
    }

    /// Returns a list of all equipped items.
    pub fn get_equipped_items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        if let Some(weapon) = &self.weapon {
            items.push(weapon.clone());
        }
        if let Some(armor) = &self.armor {
            items.push(armor.clone());
        }
        items
    }
}

// Clippy expects a Default implementation when a type has new() with no arguments.
impl Default for Equipment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Equipment;
    use crate::game::item::get_item;

    // TC EQ 11
    #[test]
    fn equip_puts_items_in_matching_slot() {
        let mut equipment = Equipment::new();
        assert!(equipment.equip(get_item(0))); // Pickaxe (weapon)
        assert!(equipment.equip(get_item(1))); // Leather Armor (armor)
        assert_eq!(equipment.weapon, Some(get_item(0)));
        assert_eq!(equipment.armor, Some(get_item(1)));
    }

    // TC EQ 12
    #[test]
    fn equip_rejects_consumable() {
        let mut equipment = Equipment::new();
        assert!(!equipment.equip(get_item(2))); // Health Potion (consumable)
        assert_eq!(equipment.weapon, None);
        assert_eq!(equipment.armor, None);
    }
}
