//! Procedural level construction (SDD 3.8).
//!
//! Contract 19 Generate Level: construct a Level for a given depth.
//! Entity population is deferred while Enemy and Item remain scaffolding.
//! Stateless between calls; Game decides when a level is needed.

use std::collections::VecDeque;

use rand::{Rng, RngExt, seq::SliceRandom};

use crate::game::level::{
    Level, SpawnPlan,
    map::{Map, TileType},
};
use crate::game::position::Position;

const MAP_WIDTH: usize = 80;
const MAP_HEIGHT: usize = 21;
const ROOM_ATTEMPTS: usize = 30;
const RECT_MIN_SIZE: usize = 4;
const RECT_MAX_SIZE: usize = 10;
const CIRCLE_MIN_RADIUS: usize = 2;
const CIRCLE_MAX_RADIUS: usize = 6;
const DOOR_CHANCE: f64 = 0.2;
const MAX_DOORS: usize = 6;
const ROCK_ROOM_CHANCE: f64 = 0.35;
const WATER_ROOM_CHANCE: f64 = 0.4;
const WATER_MIN_DIAMETER: usize = 9;
const WATER_PLACEMENT_ATTEMPTS: usize = 12;

/// Builds dungeon floors and reserves placements for enemies and items.
pub struct LevelGenerator;

impl LevelGenerator {
    /// Builds a connected level with a safe start, distant stairs, and spawn plans.
    /// Game supplies the depth and RNG; generation retains no session state.
    /// Entity collections remain empty until Enemy and Item constructors are merged.
    pub fn generate_level(depth: u32, rng: &mut impl Rng) -> Level {
        let (mut map, player_start) = generate_layout(depth, rng);
        let stairs_down = farthest_floor(&map, player_start);
        map.set(stairs_down, TileType::StairsDown);
        let pending_spawns = plan_spawns(&map, depth, player_start, stairs_down, rng);
        Level::new(map, depth, player_start, stairs_down, pending_spawns)
    }
}

/// Carves rooms and connecting corridors; the first accepted room owns the start.
fn generate_layout(depth: u32, rng: &mut impl Rng) -> (Map, Position) {
    let mut map = Map::new(MAP_WIDTH, MAP_HEIGHT);
    let mut rooms: Vec<Room> = Vec::new();

    // No circles on the first floor, 10% increase per floor until 80%
    let circle_chance = (f64::from(depth) * 0.1).clamp(0.0, 0.8);

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
    // The first attempt is always accepted because the room list starts empty.
    let player_start = rooms[0].center();
    add_doors(&mut map, &rooms, rng);
    add_water(&mut map, &rooms, player_start, rng);
    add_rocks(&mut map, &rooms, player_start, rng);
    (map, player_start)
}

/// Finds a distant exit along routes that can be used after opening doors.
fn farthest_floor(map: &Map, start: Position) -> Position {
    explore(map, start).0
}

/// Returns the most distant floor and connected tile count with doors openable.
/// This topology check excludes rocks, water, and walls without changing doors.
fn explore(map: &Map, start: Position) -> (Position, usize) {
    let mut reached = vec![false; map.width * map.height];
    reached[start.y * map.width + start.x] = true;
    let mut frontier = VecDeque::from([start]);
    let mut farthest = start;
    let mut count = 0;
    while let Some(position) = frontier.pop_front() {
        count += 1;
        if map
            .get(position)
            .is_some_and(|tile| tile.kind == TileType::Floor)
        {
            farthest = position;
        }
        let neighbors = [
            position
                .x
                .checked_sub(1)
                .map(|x| Position::new(x, position.y)),
            position
                .y
                .checked_sub(1)
                .map(|y| Position::new(position.x, y)),
            position
                .x
                .checked_add(1)
                .map(|x| Position::new(x, position.y)),
            position
                .y
                .checked_add(1)
                .map(|y| Position::new(position.x, y)),
        ];
        for next in neighbors.into_iter().flatten() {
            if connected_terrain(map, next) && !reached[next.y * map.width + next.x] {
                reached[next.y * map.width + next.x] = true;
                frontier.push_back(next);
            }
        }
    }
    (farthest, count)
}

/// Structural traversal for generation; closed doors still block actual movement.
fn connected_terrain(map: &Map, position: Position) -> bool {
    map.get(position)
        .is_some_and(|tile| tile.kind.is_walkable() || matches!(tile.kind, TileType::Door { .. }))
}

/// Adds spaced, closed doors where a narrow hallway meets a room boundary.
fn add_doors(map: &mut Map, rooms: &[Room], rng: &mut impl Rng) {
    let mut candidates = Vec::new();
    for y in 1..map.height - 1 {
        for x in 1..map.width - 1 {
            let position = Position::new(x, y);
            if rooms.iter().any(|room| room.contains(position))
                || map.get(position).unwrap().kind != TileType::Floor
            {
                continue;
            }
            let wall = |x, y| map.get(Position::new(x, y)).unwrap().kind == TileType::Wall;
            let floor = |x, y| map.get(Position::new(x, y)).unwrap().kind == TileType::Floor;
            let in_room = |x, y| rooms.iter().any(|room| room.contains(Position::new(x, y)));
            // The door occupies the first hallway tile outside the room. Exactly
            // one side must be room floor, and the other must continue the hallway.
            if (wall(x, y - 1)
                && wall(x, y + 1)
                && floor(x - 1, y)
                && floor(x + 1, y)
                && (in_room(x - 1, y) != in_room(x + 1, y)))
                || (wall(x - 1, y)
                    && wall(x + 1, y)
                    && floor(x, y - 1)
                    && floor(x, y + 1)
                    && (in_room(x, y - 1) != in_room(x, y + 1)))
            {
                candidates.push(position);
            }
        }
    }
    candidates.shuffle(rng);
    let mut doors: Vec<Position> = Vec::new();
    for position in candidates {
        if doors.len() == MAX_DOORS {
            break;
        }
        if doors
            .iter()
            .any(|other| position.x.abs_diff(other.x) + position.y.abs_diff(other.y) < 5)
            || !rng.random_bool(DOOR_CHANCE)
        {
            continue;
        }
        map.set(position, TileType::Door { open: false });
        doors.push(position);
    }
}

/// Places a rounded pool in some large circular rooms, accepting the whole blob
/// only when dry routes and room entrances remain usable.
fn add_water(map: &mut Map, rooms: &[Room], start: Position, rng: &mut impl Rng) {
    let mut connected_count = explore(map, start).1;
    for room in rooms
        .iter()
        .filter(|room| room.circular && room.width >= WATER_MIN_DIAMETER)
    {
        if !rng.random_bool(WATER_ROOM_CHANCE) {
            continue;
        }
        for _ in 0..WATER_PLACEMENT_ATTEMPTS {
            let center = Position::new(
                rng.random_range(room.x + 1..room.x + room.width - 1),
                rng.random_range(room.y + 1..room.y + room.height - 1),
            );
            let radius = if room.width >= 11 {
                rng.random_range(1..=2)
            } else {
                1
            };
            let (dx, dy) = match rng.random_range(0..4) {
                0 => (1, 0),
                1 => (-1, 0),
                2 => (0, 1),
                _ => (0, -1),
            };
            let lobe = Position::new(
                center.x.checked_add_signed(dx).unwrap(),
                center.y.checked_add_signed(dy).unwrap(),
            );
            let pool = water_blob(center, radius, lobe);
            if !pool.iter().all(|&position| {
                room.contains(position)
                    && !rooms.iter().any(|room| room.center() == position)
                    && position.x.abs_diff(start.x).max(position.y.abs_diff(start.y)) > 2
                    // Preserve a dry rim and never replace doors or their approach tiles.
                    && [(position.x, position.y), (position.x - 1, position.y),
                        (position.x + 1, position.y), (position.x, position.y - 1),
                        (position.x, position.y + 1)].into_iter().all(|(x, y)|
                            map.get(Position::new(x, y)).is_some_and(|tile| tile.kind == TileType::Floor))
            }) { continue; }
            for &position in &pool {
                map.set(position, TileType::Water);
            }
            // Reject the entire pool rather than trimming it into scattered water tiles.
            if explore(map, start).1 == connected_count - pool.len() {
                connected_count -= pool.len();
                break;
            }
            for position in pool {
                map.set(position, TileType::Floor);
            }
        }
    }
}

/// Unites overlapping disks into a connected rounded blob, without random holes.
fn water_blob(center: Position, radius: usize, lobe: Position) -> Vec<Position> {
    let mut tiles = Vec::new();
    for y in center
        .y
        .saturating_sub(radius)
        .min(lobe.y.saturating_sub(1))..=(center.y + radius).max(lobe.y + 1)
    {
        for x in center
            .x
            .saturating_sub(radius)
            .min(lobe.x.saturating_sub(1))..=(center.x + radius).max(lobe.x + 1)
        {
            let distance =
                |point: Position| x.abs_diff(point.x).pow(2) + y.abs_diff(point.y).pow(2);
            if distance(center) <= radius * radius || distance(lobe) <= 1 {
                tiles.push(Position::new(x, y));
            }
        }
    }
    tiles
}

/// Scatters one or two boulders in occasional circular rooms without sealing paths.
fn add_rocks(map: &mut Map, rooms: &[Room], start: Position, rng: &mut impl Rng) {
    let mut walkable_count = explore(map, start).1;
    for room in rooms.iter().filter(|room| room.circular) {
        if !rng.random_bool(ROCK_ROOM_CHANCE) {
            continue;
        }
        let mut candidates = Vec::new();
        for y in room.y + 1..room.y + room.height - 1 {
            for x in room.x + 1..room.x + room.width - 1 {
                let position = Position::new(x, y);
                if !room.contains(position)
                    || rooms.iter().any(|room| room.center() == position)
                    || x.abs_diff(start.x).max(y.abs_diff(start.y)) <= 2
                {
                    continue;
                }
                let interior = [(x, y), (x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .into_iter()
                    .all(|(x, y)| {
                        map.get(Position::new(x, y))
                            .is_some_and(|tile| tile.kind == TileType::Floor)
                    });
                if interior {
                    candidates.push(position);
                }
            }
        }
        candidates.shuffle(rng);
        let target = rng.random_range(1..=2);
        let mut placed = 0;
        for position in candidates {
            if placed == target {
                break;
            }
            map.set(position, TileType::Rock);
            // Test the final carved layout, including hallways that cross rooms.
            if explore(map, start).1 == walkable_count - 1 {
                walkable_count -= 1;
                placed += 1;
            } else {
                map.set(position, TileType::Floor);
            }
        }
    }
}

/// Samples distinct floor cells without retries or dependence on entity catalogs.
fn plan_spawns(
    map: &Map,
    depth: u32,
    start: Position,
    exit: Position,
    rng: &mut impl Rng,
) -> SpawnPlan {
    let mut available: Vec<_> = (0..map.height)
        .flat_map(|y| (0..map.width).map(move |x| Position::new(x, y)))
        .filter(|&position| {
            map.get(position).is_some_and(|tile| tile.kind == TileType::Floor)
                // Keep a small safe area around Pete, including diagonal neighbors.
                && position.x.abs_diff(start.x).max(position.y.abs_diff(start.y)) > 2
                && position != exit
        })
        .collect();
    available.shuffle(rng);
    // TODO(team): tune population counts and select species/catalog items after merge.
    // Cap depth scaling before conversion so even extreme depths stay bounded.
    let enemy_count = (3 + depth.min(9) as usize).min(available.len());
    let enemies = available.drain(..enemy_count).collect();
    let item_count = (2 + depth.min(6) as usize / 2).min(available.len());
    let items = available.drain(..item_count).collect();
    SpawnPlan { enemies, items }
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

    /// Whether a tile belongs to the room's carved shape.
    fn contains(&self, position: Position) -> bool {
        if position.x < self.x
            || position.x >= self.x + self.width
            || position.y < self.y
            || position.y >= self.y + self.height
        {
            return false;
        }
        let center = self.center();
        let dx = position.x.abs_diff(center.x);
        let dy = position.y.abs_diff(center.y);
        !self.circular || dx * dx + dy * dy <= (self.width / 2).pow(2)
    }

    /// Turns the room's tiles into floor
    fn carve(&self, map: &mut Map) {
        for y in self.y..self.y + self.height {
            for x in self.x..self.x + self.width {
                if self.contains(Position::new(x, y)) {
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

    /// Generates levels for several depths and seeds, so circles are included.
    fn sample_levels() -> Vec<Level> {
        let mut levels = Vec::new();
        for depth in 0..10 {
            for seed in 0..20 {
                let mut rng = StdRng::seed_from_u64(seed);
                levels.push(LevelGenerator::generate_level(depth, &mut rng));
            }
        }
        levels
    }

    #[test]
    fn outer_edge_is_always_wall() {
        for level in sample_levels() {
            let map = level.get_map();
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
    fn every_floor_tile_is_reachable_after_opening_doors() {
        for level in sample_levels() {
            let map = level.get_map();
            let floors: Vec<Position> = (0..map.height)
                .flat_map(|y| (0..map.width).map(move |x| Position::new(x, y)))
                .filter(|&pos| {
                    map.get(pos).is_some_and(|tile| {
                        tile.kind.is_walkable() || matches!(tile.kind, TileType::Door { .. })
                    })
                })
                .collect();
            assert!(!floors.is_empty(), "map has no floor");

            // Flood fill from the player start, including stairs and reserved spawns.
            let mut reached = std::collections::HashSet::from([level.player_start()]);
            let mut frontier = vec![level.player_start()];
            while let Some(pos) = frontier.pop() {
                let neighbors = [
                    pos.x.checked_sub(1).map(|x| Position::new(x, pos.y)),
                    pos.y.checked_sub(1).map(|y| Position::new(pos.x, y)),
                    Some(Position::new(pos.x + 1, pos.y)),
                    Some(Position::new(pos.x, pos.y + 1)),
                ];
                for next in neighbors.into_iter().flatten() {
                    let passable_after_opening = map.get(next).is_some_and(|tile| {
                        tile.kind.is_walkable() || matches!(tile.kind, TileType::Door { .. })
                    });
                    if passable_after_opening && reached.insert(next) {
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

    #[test]
    fn anchors_and_spawn_plans_are_safe_across_depths_and_seeds() {
        for level in sample_levels() {
            let map = level.get_map();
            let start = level.player_start();
            let exit = level.stairs_down();
            assert_ne!(start, exit);
            assert_eq!(map.get(start).unwrap().kind, TileType::Floor);
            assert_eq!(map.get(exit).unwrap().kind, TileType::StairsDown);
            let stairs_count = (0..map.height)
                .flat_map(|y| (0..map.width).map(move |x| Position::new(x, y)))
                .filter(|&position| map.get(position).unwrap().kind == TileType::StairsDown)
                .count();
            assert_eq!(stairs_count, 1);
            let plan = level.pending_spawns();
            assert!(!plan.enemies.is_empty());
            assert!(!plan.items.is_empty());
            let mut occupied = std::collections::HashSet::from([start, exit]);
            for &position in plan.enemies.iter().chain(&plan.items) {
                assert!(occupied.insert(position), "overlapping reserved placement");
                assert_eq!(map.get(position).unwrap().kind, TileType::Floor);
                assert!(
                    position
                        .x
                        .abs_diff(start.x)
                        .max(position.y.abs_diff(start.y))
                        > 2
                );
            }
            // Placeholder plans must not masquerade as constructed entity instances.
            assert!(level.get_enemies().is_empty());
            assert!(level.get_floor_items().is_empty());
        }
    }

    #[test]
    fn generation_preserves_depth_and_is_reproducible_even_at_maximum_depth() {
        for depth in [0, 1, 9, u32::MAX] {
            let mut first_rng = StdRng::seed_from_u64(42);
            let mut second_rng = StdRng::seed_from_u64(42);
            let first = LevelGenerator::generate_level(depth, &mut first_rng);
            let second = LevelGenerator::generate_level(depth, &mut second_rng);
            assert_eq!(first.depth(), depth);
            assert_eq!(first.player_start(), second.player_start());
            assert_eq!(first.stairs_down(), second.stairs_down());
            assert_eq!(first.pending_spawns(), second.pending_spawns());
            for y in 0..MAP_HEIGHT {
                for x in 0..MAP_WIDTH {
                    let position = Position::new(x, y);
                    assert_eq!(
                        first.get_map().get(position).unwrap().kind,
                        second.get_map().get(position).unwrap().kind
                    );
                }
            }
        }
    }

    #[test]
    fn decorations_include_closed_doors_and_nonwalkable_rocks() {
        let mut doors = 0;
        let mut rocks = 0;
        for level in sample_levels() {
            let map = level.get_map();
            for y in 1..map.height - 1 {
                for x in 1..map.width - 1 {
                    match map.get(Position::new(x, y)).unwrap().kind {
                        TileType::Door { open } => {
                            doors += 1;
                            assert!(!open, "generated doors must start closed");
                            assert!(!level.is_walkable(Position::new(x, y)));
                            let wall =
                                |x, y| map.get(Position::new(x, y)).unwrap().kind == TileType::Wall;
                            assert!(
                                (wall(x, y - 1)
                                    && wall(x, y + 1)
                                    && map.is_walkable(Position::new(x - 1, y))
                                    && map.is_walkable(Position::new(x + 1, y)))
                                    || (wall(x - 1, y)
                                        && wall(x + 1, y)
                                        && map.is_walkable(Position::new(x, y - 1))
                                        && map.is_walkable(Position::new(x, y + 1)))
                            );
                        }
                        TileType::Rock => {
                            rocks += 1;
                            assert!(level.depth() > 0, "first-floor rooms are rectangular");
                            let position = Position::new(x, y);
                            assert!(!level.is_walkable(position));
                            assert!(!level.pending_spawns().enemies.contains(&position));
                            assert!(!level.pending_spawns().items.contains(&position));
                        }
                        _ => {}
                    }
                }
            }
        }
        assert!(doors > 0, "no doors appeared across seeded levels");
        assert!(rocks > 0, "no rocks appeared across seeded levels");
    }

    #[test]
    fn rocks_stay_inside_circular_rooms_and_preserve_room_centers() {
        let rooms = [
            Room {
                x: 4,
                y: 4,
                width: 9,
                height: 9,
                circular: true,
            },
            Room {
                x: 20,
                y: 4,
                width: 10,
                height: 8,
                circular: false,
            },
        ];
        let mut total = 0;
        for seed in 0..40 {
            let mut map = Map::new(MAP_WIDTH, MAP_HEIGHT);
            for room in &rooms {
                room.carve(&mut map);
            }
            carve_line(&mut map, rooms[0].center(), rooms[1].center());
            let start = rooms[1].center();
            add_rocks(&mut map, &rooms, start, &mut StdRng::seed_from_u64(seed));
            for y in 0..map.height {
                for x in 0..map.width {
                    let position = Position::new(x, y);
                    if map.get(position).unwrap().kind == TileType::Rock {
                        total += 1;
                        assert!(rooms[0].contains(position));
                        assert!(!rooms[1].contains(position));
                    }
                }
            }
            for room in &rooms {
                assert_eq!(map.get(room.center()).unwrap().kind, TileType::Floor);
            }
        }
        assert!(total > 0);
    }

    #[test]
    fn doors_only_appear_at_room_hallway_intersections() {
        for circular in [false, true] {
            let rooms = [
                Room {
                    x: 4,
                    y: 4,
                    width: 9,
                    height: 9,
                    circular,
                },
                Room {
                    x: 20,
                    y: 4,
                    width: 10,
                    height: 8,
                    circular: false,
                },
            ];
            let entrances = [Position::new(13, 8), Position::new(19, 8)];
            let mut seen = std::collections::HashSet::new();
            for seed in 0..40 {
                let mut map = Map::new(MAP_WIDTH, MAP_HEIGHT);
                for room in &rooms {
                    room.carve(&mut map);
                }
                carve_line(&mut map, rooms[0].center(), rooms[1].center());
                add_doors(&mut map, &rooms, &mut StdRng::seed_from_u64(seed));
                for y in 0..map.height {
                    for x in 0..map.width {
                        let position = Position::new(x, y);
                        if let TileType::Door { open } = map.get(position).unwrap().kind {
                            assert!(
                                entrances.contains(&position),
                                "door inside room or middle of hallway: {position:?}"
                            );
                            assert!(!open);
                            seen.insert(position);
                        }
                    }
                }
                assert_eq!(map.get(Position::new(16, 8)).unwrap().kind, TileType::Floor);
            }
            assert_eq!(seen.len(), entrances.len());
        }
    }

    #[test]
    fn spawn_planning_excludes_rocks_water_closed_doors_and_stairs() {
        let mut map = Map::new(8, 8);
        let start = Position::new(1, 1);
        let exit = Position::new(6, 6);
        let floors = [Position::new(5, 1), Position::new(5, 2)];
        map.set(start, TileType::Floor);
        map.set(exit, TileType::StairsDown);
        map.set(Position::new(4, 1), TileType::Rock);
        map.set(Position::new(4, 3), TileType::Water);
        map.set(Position::new(4, 2), TileType::Door { open: false });
        for position in floors {
            map.set(position, TileType::Floor);
        }
        let plan = plan_spawns(&map, 10, start, exit, &mut StdRng::seed_from_u64(1));
        let reserved: std::collections::HashSet<_> =
            plan.enemies.iter().chain(&plan.items).copied().collect();
        assert_eq!(reserved, std::collections::HashSet::from(floors));
        for position in reserved {
            assert!(map.is_walkable(position));
        }
    }

    #[test]
    fn water_appears_in_generated_levels_and_never_receives_spawns() {
        let mut count = 0;
        for level in sample_levels() {
            let map = level.get_map();
            for y in 0..map.height {
                for x in 0..map.width {
                    let position = Position::new(x, y);
                    if map.get(position).unwrap().kind == TileType::Water {
                        count += 1;
                        assert!(level.depth() > 0);
                        assert!(!level.is_walkable(position));
                        assert!(!connected_terrain(map, position));
                        assert!(!level.pending_spawns().enemies.contains(&position));
                        assert!(!level.pending_spawns().items.contains(&position));
                        assert!(
                            x.abs_diff(level.player_start().x)
                                .max(y.abs_diff(level.player_start().y))
                                > 2
                        );
                    }
                }
            }
        }
        assert!(count > 0, "no pools generated across seeded levels");
    }

    #[test]
    fn pools_are_connected_and_only_appear_in_large_circular_rooms() {
        let rooms = [
            Room {
                x: 4,
                y: 4,
                width: 11,
                height: 11,
                circular: true,
            },
            Room {
                x: 25,
                y: 6,
                width: 7,
                height: 7,
                circular: true,
            },
            Room {
                x: 40,
                y: 5,
                width: 12,
                height: 8,
                circular: false,
            },
        ];
        let mut total = 0;
        for seed in 0..80 {
            let mut map = Map::new(MAP_WIDTH, MAP_HEIGHT);
            for room in &rooms {
                room.carve(&mut map);
            }
            carve_line(&mut map, rooms[0].center(), rooms[2].center());
            let start = rooms[2].center();
            let before = explore(&map, start).1;
            add_water(&mut map, &rooms, start, &mut StdRng::seed_from_u64(seed));
            let water: std::collections::HashSet<_> = (0..map.height)
                .flat_map(|y| (0..map.width).map(move |x| Position::new(x, y)))
                .filter(|&position| map.get(position).unwrap().kind == TileType::Water)
                .collect();
            assert_eq!(explore(&map, start).1, before - water.len());
            for room in &rooms {
                assert_eq!(map.get(room.center()).unwrap().kind, TileType::Floor);
            }
            if let Some(&first) = water.iter().next() {
                assert!(water.len() >= 5, "pool collapsed to scattered single tiles");
                total += 1;
                let mut reached = std::collections::HashSet::from([first]);
                let mut frontier = vec![first];
                while let Some(position) = frontier.pop() {
                    assert!(rooms[0].contains(position));
                    for next in [
                        Position::new(position.x - 1, position.y),
                        Position::new(position.x + 1, position.y),
                        Position::new(position.x, position.y - 1),
                        Position::new(position.x, position.y + 1),
                    ] {
                        if water.contains(&next) && reached.insert(next) {
                            frontier.push(next);
                        }
                    }
                }
                assert_eq!(reached, water);
            }
        }
        assert!(total > 0);
    }

    #[test]
    fn water_blob_has_a_rounded_solid_outline() {
        let tiles = water_blob(Position::new(5, 5), 1, Position::new(6, 5));
        let rows: Vec<_> = (4..=6)
            .map(|y| tiles.iter().filter(|position| position.y == y).count())
            .collect();
        assert_eq!(rows, [2, 4, 2]);
        assert_eq!(tiles.len(), 8);
    }
}
