//! Collectible items (SDD 3.7).
//!
//! Contract 18 Get Specific Item: look up an item by index.

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
}

impl Item {
    /// Creates a new Item.
    pub fn new(index: u32, name: &str, kind: ItemType, level: u32) -> Item {
        Item {
            index,
            name: name.to_string(),
            kind,
            level,
        }
    }
}

// TODO(Robert): replace placeholder items with final item list.
// Catalog order defines the existing public item indices.
const CATALOG: [(&str, ItemType, u32); 3] = [
    ("Pickaxe", ItemType::Weapon, 1),
    ("Leather Armor", ItemType::Armor, 1),
    ("Health Potion", ItemType::Consumable, 1),
];

/// Iterates the complete catalog in index order, constructing independent items.
/// Generation uses this list so new catalog entries need no separate spawn list.
pub fn catalog() -> impl Iterator<Item = Item> {
    CATALOG
        .iter()
        .enumerate()
        .map(|(index, &(name, kind, level))| Item::new(index as u32, name, kind, level))
}

/// Accepts an item index and returns the matching Item (Contract 18).
///
/// Panics if no item has that index.
pub fn get_item(index: u32) -> Item {
    let entry = usize::try_from(index)
        .ok()
        .and_then(|index| CATALOG.get(index));
    let &(name, kind, level) = entry.unwrap_or_else(|| panic!("No item with index {index}"));
    Item::new(index, name, kind, level)
}

#[cfg(test)]
mod tests {
    use super::{ItemType, catalog, get_item};

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
