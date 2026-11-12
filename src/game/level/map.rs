//! Dungeon tile grid.
//!
//! Read by Level (contracts 8, 9) and Renderer::draw_level.

use crate::game::position::Position;

// From a room's center, cover half the widest generated room in every direction.
pub(crate) const PLAYER_VISION_RADIUS: usize = super::generator::MAX_ROOM_WIDTH / 2;

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
    /// Whether this terrain stops sight. Water blocks movement but not vision.
    pub fn blocks_sight(self) -> bool {
        matches!(self, Self::Wall | Self::Rock | Self::Door { open: false })
    }

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
    /// Current terrain at this location.
    pub kind: TileType,
    /// Whether the player has ever seen this tile on the current floor.
    pub seen: bool,
    /// Whether this tile is in the player's current line of sight.
    pub visible: bool,
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
            seen: false,
            visible: false,
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

    ////////////////////////////////////////////////////////////////////////////////
    // LINE OF SIGHT AND EXPLORATION MEMORY
    ////////////////////////////////////////////////////////////////////////////////

    /// Whether a tile is currently visible. Out-of-bounds positions are hidden.
    pub fn is_visible(&self, pos: Position) -> bool {
        self.get(pos).is_some_and(|tile| tile.visible)
    }

    /// Recomputes sight from the player without forgetting previously seen tiles.
    /// Range is half the maximum generated room width, using Chebyshev distance.
    /// Call after player movement or sight-blocking
    /// terrain changes, before presenting state. An invalid origin clears current
    /// visibility, preserves memory, and returns false.
    pub fn update_visibility(&mut self, origin: Position) -> bool {
        self.update_visibility_with_radius(origin, PLAYER_VISION_RADIUS as f64)
    }

    /// Refreshes exploration with the player's current lamp radius.
    /// Terrain line of sight remains independent of lamp fuel for enemy AI.
    pub fn update_visibility_with_radius(&mut self, origin: Position, radius: f64) -> bool {
        // Rays read terrain only, so flags can be updated in place without
        // allocating a second grid for every movement or door interaction.
        for index in 0..self.tiles.len() {
            let position = Position::new(index % self.width, index / self.width);
            let visible = origin.distance(position) as f64 <= radius
                && self.has_line_of_sight(origin, position);
            let tile = &mut self.tiles[index];
            tile.visible = visible;
            tile.seen |= visible;
        }
        self.in_bounds(origin)
    }

    /// Tests a straight ray between tile centers, without changing exploration
    /// or applying the player's vision range. AI supplies its own detection range.
    /// Always reveals all eight adjacent tiles, even across blocked corners.
    /// Farther away, the first opaque tile is visible but hides tiles behind it.
    /// Beyond that neighborhood, rays crossing an exact grid corner require both
    /// side tiles to be transparent. Opaque targets also expose their near corner
    /// when the transparent tile immediately toward the viewer is in sight.
    /// Entities do not block sight. Either endpoint
    /// outside the map returns false.
    pub fn has_line_of_sight(&self, origin: Position, target: Position) -> bool {
        if !self.in_bounds(origin) || !self.in_bounds(target) {
            return false;
        }
        if origin.distance(target) <= 1 {
            return true;
        }
        if self.blocks_sight(target) {
            let toward = |from: usize, to: usize| match from.cmp(&to) {
                std::cmp::Ordering::Less => 1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => -1,
            };
            let near = target
                .offset(toward(target.x, origin.x), toward(target.y, origin.y))
                .unwrap();
            // Room corners need not have a clear ray to their centers. Seeing
            // the open tile against the near face reveals that boundary wall.
            // Only transparent tiles qualify, so this cannot see through walls.
            if !self.blocks_sight(near) && self.has_line_of_sight(origin, near) {
                return true;
            }
        }
        let mut position = origin;
        let dx = origin.x.abs_diff(target.x) as u128;
        let dy = origin.y.abs_diff(target.y) as u128;
        let step_x = if target.x > origin.x { 1 } else { -1 };
        let step_y = if target.y > origin.y { 1 } else { -1 };
        let (mut crossed_x, mut crossed_y) = (0_u128, 0_u128);

        while position != target {
            // Compare the next vertical/horizontal boundary crossing using integer
            // fractions, avoiding rounding differences between ray directions.
            let next_x = (2 * crossed_x + 1) * dy;
            let next_y = (2 * crossed_y + 1) * dx;
            match next_x.cmp(&next_y) {
                std::cmp::Ordering::Equal => {
                    let diagonal = position.offset(step_x, step_y).unwrap();
                    let horizontal = position.offset(step_x, 0).unwrap();
                    let vertical = position.offset(0, step_y).unwrap();
                    if self.blocks_sight(horizontal) || self.blocks_sight(vertical) {
                        return false;
                    }
                    position = diagonal;
                    crossed_x += 1;
                    crossed_y += 1;
                }
                std::cmp::Ordering::Less => {
                    position = position.offset(step_x, 0).unwrap();
                    crossed_x += 1;
                }
                std::cmp::Ordering::Greater => {
                    position = position.offset(0, step_y).unwrap();
                    crossed_y += 1;
                }
            }
            if position != target && self.blocks_sight(position) {
                return false;
            }
        }
        true
    }

    /// Treats tiles outside the map as opaque, including corner checks.
    fn blocks_sight(&self, pos: Position) -> bool {
        self.get(pos).is_none_or(|tile| tile.kind.blocks_sight())
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
    fn shrinking_lamp_hides_entities_without_revealing_or_forgetting_terrain() {
        let mut map = open_map(30, 30);
        let origin = Position::new(15, 15);
        map.update_visibility_with_radius(origin, 5.0);
        assert!(map.is_visible(Position::new(20, 15)));
        assert!(!map.get(Position::new(21, 15)).unwrap().seen);
        map.update_visibility_with_radius(origin, 1.0);
        assert!(!map.is_visible(Position::new(20, 15)));
        assert!(map.get(Position::new(20, 15)).unwrap().seen);
        assert!(map.is_visible(Position::new(16, 16)));
        assert!(map.has_line_of_sight(origin, Position::new(25, 15)));
    }

    fn open_map(width: usize, height: usize) -> Map {
        let mut map = Map::new(width, height);
        for y in 0..height {
            for x in 0..width {
                map.set(Position::new(x, y), TileType::Floor);
            }
        }
        map
    }

    #[test]
    fn sight_reveals_open_terrain_in_every_direction_within_range() {
        let mut map = open_map(11, 9);
        assert!(map.positions().all(|pos| !map.get(pos).unwrap().seen));
        let origin = Position::new(5, 4);
        assert!(map.update_visibility(origin));
        for pos in map.positions() {
            assert!(map.has_line_of_sight(origin, pos));
            assert!(map.has_line_of_sight(pos, origin));
            assert!(map.get(pos).unwrap().seen);
            assert!(map.is_visible(pos));
        }
    }

    #[test]
    fn player_vision_caps_floor_and_wall_visibility_and_preserves_memory() {
        let mut map = open_map(35, 35);
        let origin = Position::new(17, 17);
        let boundary = origin.offset(PLAYER_VISION_RADIUS as isize, 0).unwrap();
        let outside = boundary.offset(1, 0).unwrap();
        map.set(boundary, TileType::Wall);
        map.set(outside.offset(0, 1).unwrap(), TileType::Wall);
        map.update_visibility(origin);
        for position in map.positions() {
            let tile = map.get(position).unwrap();
            if origin.distance(position) > PLAYER_VISION_RADIUS {
                assert!(!tile.visible && !tile.seen, "{position:?}");
            }
        }
        assert!(map.is_visible(boundary));
        let diagonal = origin
            .offset(PLAYER_VISION_RADIUS as isize, PLAYER_VISION_RADIUS as isize)
            .unwrap();
        assert!(map.is_visible(diagonal));
        // Geometry queries stay independent of player range for enemy AI.
        assert!(map.has_line_of_sight(origin, Position::new(0, 17)));
        map.update_visibility(Position::new(0, 17));
        assert!(!map.is_visible(boundary));
        assert!(map.get(boundary).unwrap().seen);
    }

    #[test]
    fn open_room_reveals_every_boundary_wall_including_diagonal_corners() {
        for (width, height) in [(7, 7), (11, 7), (8, 6)] {
            let mut map = Map::new(width, height);
            for y in 1..height - 1 {
                for x in 1..width - 1 {
                    map.set(Position::new(x, y), TileType::Floor);
                }
            }
            let origin = Position::new(width / 2, height / 2);
            map.update_visibility(origin);
            for position in map.positions() {
                let tile = map.get(position).unwrap();
                assert!(
                    tile.visible && tile.seen,
                    "room {width}x{height}: {position:?}"
                );
            }
        }
    }

    #[test]
    fn diagonal_wall_endpoint_is_visible_without_revealing_tiles_behind_it() {
        let mut map = open_map(9, 9);
        let origin = Position::new(1, 1);
        let corner = Position::new(4, 4);
        for position in [corner, Position::new(4, 3), Position::new(3, 4)] {
            map.set(position, TileType::Wall);
        }
        let hidden_wall = Position::new(6, 6);
        map.set(hidden_wall, TileType::Wall);
        map.update_visibility(origin);
        assert!(map.is_visible(corner));
        assert!(map.get(corner).unwrap().seen);
        for position in [Position::new(5, 5), hidden_wall] {
            assert!(!map.is_visible(position));
            assert!(!map.get(position).unwrap().seen);
        }
    }

    #[test]
    fn opaque_tiles_are_visible_but_hide_tiles_behind_them() {
        for kind in [
            TileType::Wall,
            TileType::Rock,
            TileType::Door { open: false },
        ] {
            let mut map = open_map(7, 7);
            let origin = Position::new(3, 3);
            for (dx, dy) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (-1, -1),
                (-1, 1),
                (1, -1),
            ] {
                let blocker = origin.offset(dx, dy).unwrap();
                map.set(blocker, kind);
                assert!(map.has_line_of_sight(origin, blocker));
                assert!(!map.has_line_of_sight(origin, origin.offset(2 * dx, 2 * dy).unwrap()));
                map.set(blocker, TileType::Floor);
            }
        }
    }

    #[test]
    fn water_and_open_doors_transmit_sight_and_door_changes_refresh_visibility() {
        let mut map = open_map(5, 1);
        let origin = Position::new(0, 0);
        let door = Position::new(1, 0);
        let water = Position::new(2, 0);
        let target = Position::new(4, 0);
        map.set(door, TileType::Door { open: false });
        map.set(water, TileType::Water);
        map.update_visibility(origin);
        assert!(map.is_visible(door));
        assert!(!map.get(target).unwrap().seen);
        map.set(door, TileType::Door { open: true });
        map.update_visibility(origin);
        assert!(map.is_visible(target));
        assert!(!map.is_walkable(water));
        map.set(door, TileType::Door { open: false });
        map.update_visibility(origin);
        assert!(!map.is_visible(target));
        assert!(map.get(target).unwrap().seen);
    }

    #[test]
    fn moving_around_an_occluder_retains_seen_tiles_without_current_visibility() {
        let mut map = open_map(5, 3);
        map.set(Position::new(2, 1), TileType::Wall);
        let remembered = Position::new(0, 1);
        map.update_visibility(remembered);
        map.update_visibility(Position::new(4, 1));
        assert!(!map.is_visible(remembered));
        assert!(map.get(remembered).unwrap().seen);
        assert!(map.is_visible(Position::new(4, 1)));
    }

    #[test]
    fn adjacent_corners_are_visible_but_distant_peeking_is_blocked_and_sight_is_symmetric() {
        let mut map = open_map(5, 5);
        map.set(Position::new(1, 0), TileType::Wall);
        assert!(map.has_line_of_sight(Position::new(0, 0), Position::new(1, 1)));
        assert!(!map.has_line_of_sight(Position::new(0, 0), Position::new(2, 2)));
        map.set(Position::new(2, 2), TileType::Rock);
        for from in map.positions().filter(|&pos| !map.blocks_sight(pos)) {
            for to in map.positions().filter(|&pos| !map.blocks_sight(pos)) {
                assert_eq!(
                    map.has_line_of_sight(from, to),
                    map.has_line_of_sight(to, from),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn visibility_reveals_all_eight_neighbors_through_blockers_and_remembers_them() {
        let mut map = Map::new(5, 5);
        let origin = Position::new(2, 2);
        map.set(origin, TileType::Floor);
        map.set(Position::new(1, 2), TileType::Rock);
        map.set(Position::new(2, 1), TileType::Door { open: false });
        map.update_visibility(origin);
        for position in map.positions() {
            let tile = map.get(position).unwrap();
            assert_eq!(tile.visible, origin.distance(position) <= 1);
            assert_eq!(tile.seen, tile.visible);
        }
        map.update_visibility(Position::new(4, 4));
        let corner = map.get(Position::new(1, 1)).unwrap();
        assert!(corner.seen);
        assert!(!corner.visible);
        map.update_visibility(Position::new(0, 0));
        assert!(map.is_visible(Position::new(1, 1)));
        assert!(!map.has_line_of_sight(Position::new(0, 0), Position::new(usize::MAX, 0)));
    }

    #[test]
    fn invalid_origins_clear_current_sight_and_new_maps_have_no_memory() {
        let mut map = open_map(3, 3);
        map.update_visibility(Position::new(1, 1));
        assert!(!map.update_visibility(Position::new(usize::MAX, usize::MAX)));
        assert!(
            map.positions()
                .all(|pos| map.get(pos).unwrap().seen && !map.is_visible(pos))
        );
        assert!(!map.has_line_of_sight(Position::new(0, 0), Position::new(3, 0)));
        for mut empty in [Map::new(0, 0), Map::new(0, 5), Map::new(5, 0)] {
            assert!(!empty.update_visibility(Position::new(0, 0)));
        }
        let fresh = open_map(3, 3);
        assert!(fresh.positions().all(|pos| {
            let tile = fresh.get(pos).unwrap();
            !tile.seen && !tile.visible
        }));
    }

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
