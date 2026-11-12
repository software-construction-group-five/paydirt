//! The player's carried items (SDD 3.6).
//!
//! Contracts:
//! - 15 Add Item (respects per-type maximums)
//! - 16 Remove Item
//! - 17 Return Inventory Content
//!
//! Limits: consumables max 5, weapons max 3, armor max 1.

use crate::game::item::{Item, ItemType, get_item};

/// Carried items, with a separate capacity limit for each type.
#[derive(Default)]
pub struct Inventory {
    items: Vec<Item>,
}

impl Inventory {
    /// Creates an empty inventory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a catalog item if its category has room (contract 15).
    /// Returns false when full. The index must exist in the Item catalog.
    /// Multiple copies of the same item each occupy one space.
    pub fn add_item(&mut self, index: u32) -> bool {
        self.insert_item(get_item(index)).is_ok()
    }

    /// Borrows carried entries in selection order; duplicate copies stay distinct.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Checks capacity for one additional item of this category.
    pub fn can_accept(&self, kind: ItemType) -> bool {
        let limit = match kind {
            ItemType::Consumable => 5,
            ItemType::Weapon => 3,
            ItemType::Armor => 1,
        };
        let count = self.items.iter().filter(|held| held.kind == kind).count();
        count < limit
    }

    /// Transfers an existing item, returning ownership unchanged when full.
    pub fn insert_item(&mut self, item: Item) -> Result<(), Item> {
        if !self.can_accept(item.kind) {
            return Err(item);
        }
        self.items.push(item);
        Ok(())
    }

    /// Removes a carried storage entry, independently of its catalog index.
    pub fn remove_entry(&mut self, index: usize) -> Option<Item> {
        if index >= self.items.len() {
            return None;
        }
        Some(self.items.remove(index))
    }

    /// Removes and returns the first copy with this catalog index (contract 16).
    /// Returns None if the item is not carried.
    pub fn remove_item(&mut self, index: u32) -> Option<Item> {
        let position = self.items.iter().position(|item| item.index == index)?;
        Some(self.items.remove(position))
    }

    /// Lists carried item names by category without changing inventory (contract 17).
    pub fn return_inventory(&self) -> String {
        if self.items.is_empty() {
            return String::from("Inventory is empty.");
        }
        let mut lines = Vec::new();
        for (kind, label) in [
            (ItemType::Weapon, "Weapons"),
            (ItemType::Armor, "Armor"),
            (ItemType::Consumable, "Consumables"),
        ] {
            let names: Vec<&str> = self
                .items
                .iter()
                .filter(|item| item.kind == kind)
                .map(|item| item.name.as_str())
                .collect();
            let contents = if names.is_empty() {
                String::from("none")
            } else {
                names.join(", ")
            };
            lines.push(format!("{label}: {contents}"));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::Inventory;

    #[test]
    fn owned_transfers_preserve_duplicate_entries_and_return_rejected_items() {
        use crate::game::item::get_item;
        let mut inventory = Inventory::new();
        let mut first = get_item(1);
        first.name = "Custom armor".into();
        inventory.insert_item(first.clone()).unwrap();
        let rejected = get_item(1);
        assert_eq!(inventory.insert_item(rejected.clone()), Err(rejected));
        assert_eq!(inventory.remove_entry(0), Some(first));
        let mut duplicate = get_item(0);
        duplicate.name = "Second copy".into();
        inventory.insert_item(get_item(0)).unwrap();
        inventory.insert_item(duplicate.clone()).unwrap();
        assert_eq!(inventory.remove_entry(1), Some(duplicate));
        assert_eq!(inventory.items(), &[get_item(0)]);
        assert!(inventory.remove_entry(usize::MAX).is_none());
    }

    #[test]
    fn add_item_enforces_each_category_limit_without_storing_extra_items() {
        for (index, limit) in [(0, 3), (1, 1), (2, 5)] {
            let mut inventory = Inventory::new();
            for _ in 0..limit {
                assert!(inventory.add_item(index));
            }
            assert!(!inventory.add_item(index));
            for _ in 0..limit {
                assert_eq!(inventory.remove_item(index).unwrap().index, index);
            }
            assert!(inventory.remove_item(index).is_none());
        }
    }

    #[test]
    fn full_category_does_not_block_other_categories() {
        let mut inventory = Inventory::new();
        assert!(inventory.add_item(1));
        assert!(!inventory.add_item(1));
        assert!(inventory.add_item(0));
        assert!(inventory.add_item(2));
        let contents = inventory.return_inventory();
        assert!(contents.contains("Pickaxe"));
        assert!(contents.contains("Leather Armor"));
        assert!(contents.contains("Health Potion"));
    }

    #[test]
    fn removing_one_copy_frees_space_and_preserves_the_other_items() {
        let mut inventory = Inventory::new();
        for _ in 0..3 {
            assert!(inventory.add_item(0));
        }
        assert!(inventory.add_item(2));
        assert_eq!(inventory.remove_item(0).unwrap().name, "Pickaxe");
        assert_eq!(inventory.return_inventory().matches("Pickaxe").count(), 2);
        assert!(inventory.return_inventory().contains("Health Potion"));
        assert!(inventory.add_item(0));
        assert!(!inventory.add_item(0));
    }

    #[test]
    fn removing_a_missing_item_preserves_contents() {
        let mut inventory = Inventory::new();
        assert!(inventory.remove_item(99).is_none());
        inventory.add_item(2);
        let before = inventory.return_inventory();
        assert!(inventory.remove_item(0).is_none());
        assert_eq!(inventory.return_inventory(), before);
    }

    #[test]
    fn contents_describe_empty_inventory_and_group_items_by_category() {
        let mut inventory = Inventory::new();
        assert!(inventory.return_inventory().contains("empty"));
        inventory.add_item(2);
        inventory.add_item(0);
        inventory.add_item(1);
        let contents = inventory.return_inventory();
        assert!(contents.contains("Weapons: Pickaxe"));
        assert!(contents.contains("Armor: Leather Armor"));
        assert!(contents.contains("Consumables: Health Potion"));
    }
}
