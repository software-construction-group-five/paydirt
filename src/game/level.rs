//! The current dungeon floor (SDD 3.4).
//!
//! Contracts:
//! - 8 Map Access
//! - 9 Position Validation (is_walkable)
//! - 10 Enemy Management
//! - 11 Ground Item Management

pub mod generator;
pub mod map;

/// One dungeon floor: map, enemies, floor items, and depth.
pub struct Level {
    // TODO: map: Map, enemies: Vec<Enemy>, floor_items: Vec<(Item, Position)>, depth: u32
}
