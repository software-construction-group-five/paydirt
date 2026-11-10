//! The current dungeon floor (SDD 3.4).
//!
//! Level owns terrain and navigation anchors.

pub mod generator;
pub mod map;

use crate::game::position::Position;
use map::Map;

/// One generated dungeon floor, including navigation anchors.
pub struct Level {
    map: Map,
    depth: u32,
    player_start: Position,
    stairs_down: Position,
}

impl Level {
    // Construction stays inside this module so generation establishes the map,
    // anchors, and placement invariants together.
    fn new(map: Map, depth: u32, player_start: Position, stairs_down: Position) -> Self {
        Self {
            map,
            depth,
            player_start,
            stairs_down,
        }
    }

    /// Provides read-only terrain access (contract 8).
    pub fn get_map(&self) -> &Map {
        &self.map
    }

    /// Checks terrain bounds and traversability (contract 9).
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
    pub fn stairs_down(&self) -> Position {
        self.stairs_down
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
        let level = Level::new(map, 2, start, exit);
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
