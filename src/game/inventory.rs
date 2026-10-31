//! The player's carried items (SDD 3.6).
//!
//! Contracts:
//! - 15 Add Item (respects per-type maximums)
//! - 16 Remove Item
//! - 17 Return Inventory Content
//!
//! Limits: consumables max 5, weapons max 3, armor max 1.

/// Carried items grouped by type.
pub struct Inventory {
    // TODO: consumables: Vec<Item>, weapons: Vec<Item>, armor: Option<Item>
}
