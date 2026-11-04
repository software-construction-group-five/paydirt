//! Grid coordinates shared by Player, Enemy, floor items, and the Map.

/// An (x, y) tile coordinate on the dungeon map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// The position shifted by (dx, dy)
    pub fn offset(self, dx: usize, dy: usize) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
}
