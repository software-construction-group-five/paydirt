//! Collectible items (SDD 3.7).
//!
//! Contract 18 Get Specific Item: look up an item by index.

/// An item with a name, type, spawn level, and catalog index.
pub struct Item {
    // TODO: index: u32, name: String, kind: ItemType, level: u32
}

/// The three item categories.
pub enum ItemType {
    Weapon,
    Armor,
    Consumable,
}
