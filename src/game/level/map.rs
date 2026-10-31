//! Dungeon tile grid.
//!
//! Read by Level (contracts 8, 9) and Renderer::draw_level.

/// 2D grid of tiles.
pub struct Map {
    // TODO: width: usize, height: usize, tiles: Vec<Tile>
}

/// A single map cell.
pub struct Tile {
    // TODO: kind: TileType, explored: bool
}

/// Terrain kinds. Theme maps each to a display character.
pub enum TileType {
    Wall,
    Floor,
    StairsDown,
}
