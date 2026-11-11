//! Dungeon tile grid.
//!
//! Read by Level (contracts 8, 9) and Renderer::draw_level.

use crate::game::position::Position;

/// Terrain kinds. Theme maps each to a display character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileType {
    Wall,
    /// Impassable boulder placed inside a room.
    Rock,
    /// Impassable water filling part of a room.
    Water,
    Floor,
    Door {
        open: bool,
    },
    StairsDown,
}

impl TileType {
    /// Whether players and enemies can be on this type of tile.
    pub fn is_walkable(self) -> bool {
        match self {
            TileType::Wall | TileType::Rock | TileType::Water => false,
            TileType::Door { open } => open,
            TileType::Floor | TileType::StairsDown => true,
        }
    }
}

/// A single map cell.
#[derive(Debug, Clone, Copy)]
pub struct Tile {
    pub kind: TileType,
    pub explored: bool,
}

/// 2D grid of tiles.
pub struct Map {
    width: usize,
    height: usize,
    tiles: Vec<Tile>,
}

impl Map {
    /// Creates a map filled with walls.
    pub fn new(width: usize, height: usize) -> Self {
        let wall = Tile {
            kind: TileType::Wall,
            explored: false,
        };
        Self {
            width,
            height,
            tiles: vec![wall; width.checked_mul(height).expect("map dimensions overflow")],
        }
    }

    /// Returns the number of columns; dimensions cannot change after allocation.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the number of rows.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Visits every stored position in row order, including walls and obstacles.
    pub fn positions(&self) -> impl Iterator<Item = Position> + '_ {
        (0..self.tiles.len()).map(|index| Position::new(index % self.width, index / self.width))
    }

    /// Returns in-bounds cardinal neighbors without overflowing grid coordinates.
    pub fn neighbors(&self, pos: Position) -> impl Iterator<Item = Position> + '_ {
        let valid = self.in_bounds(pos);
        [
            (valid && pos.x > 0).then(|| Position::new(pos.x - 1, pos.y)),
            (valid && pos.y > 0).then(|| Position::new(pos.x, pos.y - 1)),
            (valid && pos.x + 1 < self.width).then(|| Position::new(pos.x + 1, pos.y)),
            (valid && pos.y + 1 < self.height).then(|| Position::new(pos.x, pos.y + 1)),
        ]
        .into_iter()
        .flatten()
    }

    /// Whether (x,y) lies inside the map.
    pub fn in_bounds(&self, pos: Position) -> bool {
        pos.x < self.width && pos.y < self.height
    }

    /// The tile at a position, or `None` if out of bounds.
    pub fn get(&self, pos: Position) -> Option<&Tile> {
        if self.in_bounds(pos) {
            Some(&self.tiles[self.index(pos)])
        } else {
            None
        }
    }

    /// Sets the tile kind at a position. Does nothing if out of bounds.
    pub fn set(&mut self, pos: Position, kind: TileType) {
        if self.in_bounds(pos) {
            let i = self.index(pos);
            self.tiles[i].kind = kind;
        }
    }

    /// Whether a position can be walked on. Out of bounds is blocked.
    pub fn is_walkable(&self, pos: Position) -> bool {
        self.get(pos).is_some_and(|tile| tile.kind.is_walkable())
    }

    /// Converts a validated position into its row-major storage index.
    fn index(&self, pos: Position) -> usize {
        pos.y * self.width + pos.x
    }
}

////////////////////////////////////////////////////////////////////////////////
// TESTS
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_changes_walkability() {
        let mut map = Map::new(5, 5);
        let pos = Position::new(2, 2);
        assert!(!map.is_walkable(pos), "new maps start as walls");

        map.set(pos, TileType::Floor);
        assert!(map.is_walkable(pos));

        map.set(pos, TileType::Door { open: false });
        assert!(!map.is_walkable(pos), "closed doors block movement");

        map.set(pos, TileType::Door { open: true });
        assert!(map.is_walkable(pos), "open doors allow movement");
    }

    #[test]
    fn out_of_bounds_is_blocked_and_ignored() {
        let mut map = Map::new(5, 5);
        let outside = Position::new(5, 0);

        map.set(outside, TileType::Floor);
        assert!(map.get(outside).is_none());
        assert!(!map.is_walkable(outside));
    }

    #[test]
    fn positions_and_neighbors_handle_empty_maps_edges_and_extreme_coordinates() {
        for map in [Map::new(0, 0), Map::new(0, 5), Map::new(5, 0)] {
            assert_eq!(map.positions().count(), 0);
            assert_eq!(map.neighbors(Position::new(0, 0)).count(), 0);
        }
        let map = Map::new(3, 2);
        assert_eq!((map.width(), map.height()), (3, 2));
        assert_eq!(map.positions().last(), Some(Position::new(2, 1)));
        assert_eq!(
            map.neighbors(Position::new(0, 0)).collect::<Vec<_>>(),
            [Position::new(1, 0), Position::new(0, 1)]
        );
        assert_eq!(
            map.neighbors(Position::new(usize::MAX, usize::MAX)).count(),
            0
        );
    }

    #[test]
    #[should_panic(expected = "map dimensions overflow")]
    fn overflowing_dimensions_are_rejected_before_allocation() {
        Map::new(usize::MAX, 2);
    }
}
