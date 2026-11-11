//! Grid coordinates shared by Player, Enemy, floor items, and the Map.

/// An (x, y) tile coordinate on the dungeon map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    /// Creates a position at (x, y)
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// The position shifted by (dx, dy), or `None` if x or y would go below 0
    pub fn offset(self, dx: isize, dy: isize) -> Option<Self> {
        let x = self.x.checked_add_signed(dx)?;
        let y = self.y.checked_add_signed(dy)?;
        Some(Self::new(x, y))
    }

    /// Number of moves to reach `other` if diagonal moves are allowed
    pub fn distance(self, other: Position) -> usize {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn distance_counts_diagonal_as_one_move() {
        let a = Position::new(0, 0);
        let b = Position::new(3, 4);
        assert_eq!(a.distance(b), 4);
        assert_eq!(b.distance(a), 4);
    }

    #[test]
    fn distance_to_self_is_zero() {
        let a = Position::new(5, 5);
        assert_eq!(a.distance(a), 0);
    }

    #[test]
    fn offset_moves_in_all_directions() {
        let a = Position::new(5, 5);
        assert_eq!(a.offset(1, 0), Some(Position::new(6, 5)));
        assert_eq!(a.offset(-1, 0), Some(Position::new(4, 5)));
        assert_eq!(a.offset(0, -1), Some(Position::new(5, 4)));
        assert_eq!(a.offset(0, 1), Some(Position::new(5, 6)));
    }

    #[test]
    fn offset_past_top_left_edge_is_none() {
        let a = Position::new(0, 0);
        assert_eq!(a.offset(-1, 0), None);
        assert_eq!(a.offset(0, -1), None);
    }
}
