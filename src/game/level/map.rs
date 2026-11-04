//! Dungeon tile grid.
//!
//! Read by Level (contracts 8, 9) and Renderer::draw_level.

use crate::game::position::Position;

/// Terrain kinds. Theme maps each to a display character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileType {
    Wall,
    Floor,
    Door { open: bool },
    StairsDown,
}

impl TileType {
    /// Whether players and enemies can be on this type of tile.
    pub fn is_walkable(self) -> bool {
        match self {
            TileType::Wall => false,
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
    pub width: usize,
    pub height: usize,
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
            tiles: vec![wall; width * height],
        }
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

    /// serialize Position into a single
    fn index(&self, pos: Position) -> usize {
        pos.y * self.width + pos.x
    }
}
