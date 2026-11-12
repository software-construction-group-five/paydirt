//! Items the player currently has equipped (weapon and armor slots).
//!
//! Modified via Player contract 7 (update_equipped).

use crate::game::item::{Item, ItemType};

/// A specific equipment slot, distinct from an inventory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentSlot {
    /// Equipped weapon.
    Weapon,
    /// Equipped armor.
    Armor,
}

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
    /// Borrows the item in a selected slot.
    pub fn item(&self, slot: EquipmentSlot) -> Option<&Item> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.as_ref(),
            EquipmentSlot::Armor => self.armor.as_ref(),
        }
    }

    /// Takes ownership of the selected slot's item.
    pub fn take(&mut self, slot: EquipmentSlot) -> Option<Item> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.take(),
            EquipmentSlot::Armor => self.armor.take(),
        }
    }
    /// Creates an Equipment with both slots empty.
    pub fn new() -> Equipment {
        Equipment {
            weapon: None,
            armor: None,
        }
    }

    /// Equips a weapon or armor in its matching slot, replacing any item already there.
    ///
    /// Returns the displaced item on success, or the rejected consumable on error.
    /// The caller can return either item to inventory without losing ownership.
    pub fn equip(&mut self, item: Item) -> Result<Option<Item>, Item> {
        match item.kind {
            ItemType::Weapon => Ok(self.weapon.replace(item)),
            ItemType::Armor => Ok(self.armor.replace(item)),
            // Consumables are used up, not worn, so they can't be equipped.
            ItemType::Consumable => Err(item),
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
        assert_eq!(equipment.equip(get_item(0)), Ok(None)); // Pickaxe (weapon)
        assert_eq!(equipment.equip(get_item(1)), Ok(None)); // Leather Armor (armor)
        assert_eq!(equipment.weapon, Some(get_item(0)));
        assert_eq!(equipment.armor, Some(get_item(1)));
    }

    // TC EQ 12
    #[test]
    fn equip_rejects_consumable() {
        let mut equipment = Equipment::new();
        assert_eq!(equipment.equip(get_item(2)), Err(get_item(2))); // Health Potion
        assert_eq!(equipment.weapon, None);
        assert_eq!(equipment.armor, None);
    }

    #[test]
    fn replacement_returns_previous_item_and_preserves_other_slot() {
        let mut equipment = Equipment::new();
        equipment.equip(get_item(0)).unwrap();
        equipment.equip(get_item(1)).unwrap();

        for index in [0, 1] {
            let mut replacement = get_item(index);
            replacement.name = format!("Replacement {index}");
            assert_eq!(
                equipment.equip(replacement.clone()),
                Ok(Some(get_item(index)))
            );
            if index == 0 {
                assert_eq!(equipment.weapon, Some(replacement));
                assert_eq!(equipment.armor, Some(get_item(1)));
            } else {
                assert_eq!(equipment.armor, Some(replacement));
                assert_eq!(equipment.weapon.as_ref().unwrap().name, "Replacement 0");
            }
        }
        let before = equipment.clone();
        assert_eq!(equipment.equip(get_item(2)), Err(get_item(2)));
        assert_eq!(equipment, before);
    }
}
