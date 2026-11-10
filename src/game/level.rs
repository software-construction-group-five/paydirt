//! The current dungeon floor (SDD 3.4).
//!
//! Level owns terrain and floor entities; Game controls turns and progression.

pub mod generator;
pub mod map;

use crate::game::{enemy::Enemy, item::Item, position::Position};
use map::Map;

/// Reserved, non-overlapping locations awaiting concrete entity constructors.
///
/// TODO(team): consume these positions when Enemy and Item construction is merged.
/// Enemy constructors receive their position; items remain paired with positions
/// in Level rather than storing terrain coordinates inside Item.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SpawnPlan {
    /// Enemy spawn locations, excluding the player start and stairs.
    pub enemies: Vec<Position>,
    /// Floor-item locations, excluding all other reserved locations.
    pub items: Vec<Position>,
}

/// One generated dungeon floor, including navigation anchors and entity storage.
pub struct Level {
    map: Map,
    depth: u32,
    player_start: Position,
    stairs_down: Position,
    enemies: Vec<Enemy>,
    floor_items: Vec<(Item, Position)>,
    pending_spawns: SpawnPlan,
}

impl Level {
    // Construction stays inside this module so generation establishes the map,
    // anchors, and placement invariants together.
    fn new(
        map: Map,
        depth: u32,
        player_start: Position,
        stairs_down: Position,
        pending_spawns: SpawnPlan,
    ) -> Self {
        Self {
            map,
            depth,
            player_start,
            stairs_down,
            enemies: Vec::new(),
            floor_items: Vec::new(),
            pending_spawns,
        }
    }

    /// Provides read-only terrain access (contract 8).
    pub fn get_map(&self) -> &Map {
        &self.map
    }

    /// Checks terrain bounds and traversability (contract 9).
    /// Entity occupancy is coordinated by Game when resolving movement.
    pub fn is_walkable(&self, position: Position) -> bool {
        self.map.is_walkable(position)
    }

    /// Returns the dungeon depth requested by Game.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Returns the safe starting position for placing the Player on this floor.
    pub fn player_start(&self) -> Position {
        self.player_start
    }

    /// Returns the exit reachable after opening intervening doors.
    /// Game handles door interactions and decides when to advance levels.
    pub fn stairs_down(&self) -> Position {
        self.stairs_down
    }

    /// Returns constructed enemies (contract 10); empty until factories are merged.
    pub fn get_enemies(&self) -> &[Enemy] {
        &self.enemies
    }

    /// Returns constructed ground items and their positions (contract 11).
    pub fn get_floor_items(&self) -> &[(Item, Position)] {
        &self.floor_items
    }

    /// Returns reserved placements for the future entity population step.
    pub fn pending_spawns(&self) -> &SpawnPlan {
        &self.pending_spawns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map::TileType;

    #[test]
    fn terrain_validation_rejects_walls_closed_doors_and_out_of_bounds() {
        let mut map = Map::new(5, 5);
        let start = Position::new(1, 1);
        let exit = Position::new(3, 3);
        map.set(start, TileType::Floor);
        map.set(Position::new(2, 2), TileType::Rock);
        map.set(Position::new(3, 2), TileType::Water);
        map.set(exit, TileType::StairsDown);
        map.set(Position::new(2, 1), TileType::Door { open: false });
        map.set(Position::new(1, 2), TileType::Door { open: true });
        let level = Level::new(map, 2, start, exit, SpawnPlan::default());
        assert!(level.is_walkable(start));
        assert!(level.is_walkable(exit));
        assert!(level.is_walkable(Position::new(1, 2)));
        assert!(!level.is_walkable(Position::new(2, 1)));
        assert!(!level.is_walkable(Position::new(0, 0)));
        assert!(!level.is_walkable(Position::new(2, 2)));
        assert!(!level.is_walkable(Position::new(3, 2)));
        assert!(!level.is_walkable(Position::new(5, 5)));
    }
}
