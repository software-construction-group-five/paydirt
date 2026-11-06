//! Procedural level construction (SDD 3.8).
//!
//! Contract 19 Generate Level: build a fully populated Level for a given depth.
//! Stateless between calls; Game decides when a level is needed.

use rand::{Rng, RngExt};

use crate::game::level::map::{Map, TileType};
use crate::game::position::Position;

const MAP_WIDTH: usize = 80;
const MAP_HEIGHT: usize = 21;
const ROOM_ATTEMPTS: usize = 30;
const RECT_MIN_SIZE: usize = 4;
const RECT_MAX_SIZE: usize = 10;
const CIRCLE_MIN_RADIUS: usize = 2;
const CIRCLE_MAX_RADIUS: usize = 5;

/// Builds dungeon layouts and populates enemies and items.
pub struct LevelGenerator;

impl LevelGenerator {
    /// Builds a map of rectangular and circular rooms joined by L-shaped corridors.
    /// Circular rooms become more common as depth increases.
    pub fn generate_map(depth: usize, rng: &mut impl Rng) -> Map {
        let mut map = Map::new(MAP_WIDTH, MAP_HEIGHT);
        let mut rooms: Vec<Room> = Vec::new();

        // No circles on the first floor, 10% increase per floor until 80%
        let circle_chance = (depth as f64 * 0.1).clamp(0.0, 0.8);

        for _ in 0..ROOM_ATTEMPTS {
            let room = Room::random(rng.random_bool(circle_chance), rng);
            if rooms.iter().any(|other| room.intersects(other)) {
                continue;
            }
            room.carve(&mut map);
            if let Some(previous) = rooms.last() {
                carve_corridor(&mut map, previous.center(), room.center(), rng);
            }
            rooms.push(room);
        }
        map
    }
}

/// A room's dimension. Circular rooms fill a circle inside  a square box.
struct Room {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    circular: bool,
}

impl Room {
    /// A randomly sized and placed room, kept off the map's outer wall.
    fn random(circular: bool, rng: &mut impl Rng) -> Self {
        let (width, height) = if circular {
            let diameter = 2 * rng.random_range(CIRCLE_MIN_RADIUS..=CIRCLE_MAX_RADIUS) + 1;
            (diameter, diameter)
        } else {
            (
                2 * rng.random_range(RECT_MIN_SIZE..=RECT_MAX_SIZE),
                rng.random_range(RECT_MIN_SIZE..=RECT_MAX_SIZE),
            )
        };
        Self {
            x: rng.random_range(1..=MAP_WIDTH - width - 1),
            y: rng.random_range(1..=MAP_HEIGHT - height - 1),
            width,
            height,
            circular,
        }
    }

    fn center(&self) -> Position {
        Position::new(self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Whether the boxes overlap or touch, so rooms always have a wall between them
    fn intersects(&self, other: &Room) -> bool {
        self.x <= other.x + other.width
            && self.x + self.width >= other.x
            && self.y <= other.y + other.height
            && self.y + self.height >= other.y
    }

    /// Turns the room's tiles into floor
    fn carve(&self, map: &mut Map) {
        let center = self.center();
        let radius = self.width / 2;
        for y in self.y..self.y + self.height {
            for x in self.x..self.x + self.width {
                let (dx, dy) = (x.abs_diff(center.x), y.abs_diff(center.y));
                if !self.circular || dx * dx + dy * dy <= radius * radius {
                    map.set(Position::new(x, y), TileType::Floor);
                }
            }
        }
    }
}

/// Joins two points with an L-shaped hallway, randomly turning horizontally or vertically first.
fn carve_corridor(map: &mut Map, from: Position, to: Position, rng: &mut impl Rng) {
    let corner = if rng.random_bool(0.5) {
        Position::new(to.x, from.y)
    } else {
        Position::new(from.x, to.y)
    };
    carve_line(map, from, corner);
    carve_line(map, corner, to);
}

/// Turns a straight horizontal or vertical line of tiles into floor.
fn carve_line(map: &mut Map, a: Position, b: Position) {
    for y in a.y.min(b.y)..=a.y.max(b.y) {
        for x in a.x.min(b.x)..=a.x.max(b.x) {
            map.set(Position::new(x, y), TileType::Floor);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// TESTS
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// Generates maps for several depths and seeds, so circles are included.
    fn sample_maps() -> Vec<Map> {
        let mut maps = Vec::new();
        for depth in 0..10 {
            for seed in 0..20 {
                let mut rng = StdRng::seed_from_u64(seed);
                maps.push(LevelGenerator::generate_map(depth, &mut rng));
            }
        }
        maps
    }

    #[test]
    fn outer_edge_is_always_wall() {
        for map in sample_maps() {
            for x in 0..map.width {
                for y in 0..map.height {
                    let on_edge = x == 0 || y == 0 || x == map.width - 1 || y == map.height - 1;
                    if on_edge {
                        assert!(
                            !map.is_walkable(Position::new(x, y)),
                            "floor on edge at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_floor_tile_is_reachable() {
        for map in sample_maps() {
            let floors: Vec<Position> = (0..map.height)
                .flat_map(|y| (0..map.width).map(move |x| Position::new(x, y)))
                .filter(|&pos| map.is_walkable(pos))
                .collect();
            assert!(!floors.is_empty(), "map has no floor");

            // Flood fill from the first floor tile, stepping in four directions.
            let mut reached = std::collections::HashSet::from([floors[0]]);
            let mut frontier = vec![floors[0]];
            while let Some(pos) = frontier.pop() {
                let neighbors = [
                    pos.x.checked_sub(1).map(|x| Position::new(x, pos.y)),
                    pos.y.checked_sub(1).map(|y| Position::new(pos.x, y)),
                    Some(Position::new(pos.x + 1, pos.y)),
                    Some(Position::new(pos.x, pos.y + 1)),
                ];
                for next in neighbors.into_iter().flatten() {
                    if map.is_walkable(next) && reached.insert(next) {
                        frontier.push(next);
                    }
                }
            }
            assert_eq!(
                reached.len(),
                floors.len(),
                "some floor tiles are unreachable"
            );
        }
    }
}
