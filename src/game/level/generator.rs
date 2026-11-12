//! Procedural level construction (SDD 3.8).
//!
//! Builds terrain, navigation anchors, and entity populations for SDD contract 19.
//! Stateless between calls; Game decides when a level is needed.

use rand::{Rng, RngExt, seq::SliceRandom};

use crate::game::{
    enemy::{Enemy, EnemyType},
    item::{Item, catalog, choose_loot},
    level::{
        Level,
        map::{Map, TileType},
    },
    position::Position,
};

////////////////////////////////////////////////////////////////////////////////
// TERRAIN GENERATION SETTINGS
////////////////////////////////////////////////////////////////////////////////

const MAP_WIDTH: usize = 80;
const MAP_HEIGHT: usize = 21;
const ROOM_ATTEMPTS: usize = 30;
const RECT_MIN_SIZE: usize = 4;
const RECT_MAX_SIZE: usize = 10;
const CIRCLE_MIN_RADIUS: usize = 2;
const CIRCLE_MAX_RADIUS: usize = 6;
/// Widest possible room, shared with the player visibility radius.
pub(super) const MAX_ROOM_WIDTH: usize = if 2 * RECT_MAX_SIZE > 2 * CIRCLE_MAX_RADIUS + 1 {
    2 * RECT_MAX_SIZE
} else {
    2 * CIRCLE_MAX_RADIUS + 1
};
const DOOR_CHANCE: f64 = 0.2;
const MAX_DOORS: usize = 6;
const ROCK_ROOM_CHANCE: f64 = 0.35;
const WATER_ROOM_CHANCE: f64 = 0.4;
const WATER_MIN_DIAMETER: usize = 9;
const WATER_PLACEMENT_ATTEMPTS: usize = 12;

////////////////////////////////////////////////////////////////////////////////
// LEVEL GENERATION
////////////////////////////////////////////////////////////////////////////////

/// Builds dungeon floors.
pub struct LevelGenerator;

impl LevelGenerator {
    /// Builds a connected level with a safe start, and distant stairs.
    /// Game supplies the depth and RNG; generation retains no session state.
    pub fn generate_level(depth: u32, rng: &mut impl Rng) -> Level {
        let (mut map, starting_room) = generate_layout(depth, rng);
        let player_start = starting_room.center();
        let stairs_down = explore(&map, player_start).0;
        map.set(stairs_down, TileType::StairsDown);
        let mut level = Level::new(
            map,
            depth,
            player_start,
            stairs_down,
            starting_room.positions(),
        );
        populate(&mut level, rng);
        level
    }
}

////////////////////////////////////////////////////////////////////////////////
// POPULATION RULES AND SELECTION
////////////////////////////////////////////////////////////////////////////////

/// Generation tuning ordered from weakest to strongest by current combat stats.
struct EnemySpawnProfile {
    kind: EnemyType,
    minimum_depth: u32,
    difficulty: u32,
}

const ENEMY_SPAWN_PROFILES: [EnemySpawnProfile; 3] = [
    EnemySpawnProfile {
        kind: EnemyType::Slime,
        minimum_depth: 0,
        difficulty: 1,
    },
    EnemySpawnProfile {
        kind: EnemyType::Ghost,
        minimum_depth: 2,
        difficulty: 2,
    },
    EnemySpawnProfile {
        kind: EnemyType::Goblin,
        minimum_depth: 4,
        difficulty: 3,
    },
];

impl EnemySpawnProfile {
    /// Stronger species gain weight faster after unlocking; weak species remain possible.
    fn weight(&self, depth: u32) -> u64 {
        if depth < self.minimum_depth {
            return 0;
        }
        let depths_since_unlock = u64::from(depth - self.minimum_depth) + 1;
        depths_since_unlock.pow(self.difficulty - 1)
    }
}

/// Selects a species by its depth-dependent weight, excluding locked species.
fn choose_enemy(depth: u32, rng: &mut impl Rng) -> EnemyType {
    let total: u64 = ENEMY_SPAWN_PROFILES
        .iter()
        .map(|profile| profile.weight(depth))
        .sum();
    let mut roll = rng.random_range(0..total);
    for profile in &ENEMY_SPAWN_PROFILES {
        let weight = profile.weight(depth);
        if roll < weight {
            return profile.kind;
        }
        roll -= weight;
    }
    unreachable!("slimes are available at every depth")
}

/// Filters the current catalog to items unlocked at this dungeon depth.
fn available_items(depth: u32) -> Vec<Item> {
    // Catalog levels are one-based; dungeon depths are zero-based.
    let catalog_level = depth.saturating_add(1);
    catalog()
        .filter(|item| item.level <= catalog_level)
        .collect()
}

/// Returns the enemy and item targets before limiting them to available space.
fn target_counts(depth: u32) -> (usize, usize) {
    // Cap density and arithmetic before converting to platform-sized counts.
    let enemies = 4 + depth.min(16) as usize;
    let items = 3 + (depth / 3).min(5) as usize;
    (enemies, items)
}

////////////////////////////////////////////////////////////////////////////////
// ENTITY PLACEMENT
////////////////////////////////////////////////////////////////////////////////

/// Populates only eligible, unoccupied tiles using the caller's RNG for every choice.
fn populate(level: &mut Level, rng: &mut impl Rng) {
    let mut candidates: Vec<_> = level
        .get_map()
        .positions()
        .filter(|&position| {
            level.is_spawn_tile(position)
                && level.enemy_at(position).is_none()
                && level.ground_items_at(position).next().is_none()
        })
        .collect();
    candidates.shuffle(rng);
    let (enemy_count, item_count) = target_counts(level.depth());
    let items = available_items(level.depth());

    // Reserve item space before placing enemies so a cramped floor can supply both.
    let reserved_items = if items.is_empty() {
        0
    } else {
        item_count.min(candidates.len() / 2)
    };
    let enemy_count = enemy_count.min(candidates.len() - reserved_items);
    for _ in 0..enemy_count {
        let position = candidates
            .pop()
            .expect("enemy count is bounded by available tiles");
        let enemy = Enemy::new(choose_enemy(level.depth(), rng), position);
        assert!(
            level.insert_enemy(enemy).is_ok(),
            "validated enemy placement failed"
        );
    }
    // Remaining candidates supply items without overlapping any initial enemy.
    if items.is_empty() {
        return;
    }
    for position in candidates.into_iter().take(item_count) {
        let item = choose_loot(&items, rng)
            .expect("eligible floor loot exists")
            .roll_spawn(level.depth(), rng);
        assert!(
            level.insert_ground_item(item, position).is_ok(),
            "validated item placement failed"
        );
    }
}

////////////////////////////////////////////////////////////////////////////////
// DUNGEON LAYOUT
////////////////////////////////////////////////////////////////////////////////

/// Carves rooms and connecting corridors; the first accepted room owns the start.
fn generate_layout(depth: u32, rng: &mut impl Rng) -> (Map, Room) {
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
    (map, rooms.remove(0))
}

////////////////////////////////////////////////////////////////////////////////
// CONNECTIVITY AND STAIR PLACEMENT
////////////////////////////////////////////////////////////////////////////////

/// Returns the most distant floor and connected tile count with doors openable.
/// This topology check excludes rocks, water, and walls without changing doors.
fn explore(map: &Map, start: Position) -> (Position, usize) {
    let mut reached = vec![false; map.width() * map.height()];
    reached[start.y * map.width() + start.x] = true;
    let mut frontier = Vec::with_capacity(reached.len());
    frontier.push(start);
    let mut farthest = start;
    let mut count = 0;
    while count < frontier.len() {
        let position = frontier[count];
        count += 1;
        if map
            .get(position)
            .is_some_and(|tile| tile.kind == TileType::Floor)
        {
            farthest = position;
        }
        for next in map.neighbors(position) {
            if connected_terrain(map, next) && !reached[next.y * map.width() + next.x] {
                reached[next.y * map.width() + next.x] = true;
                frontier.push(next);
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

////////////////////////////////////////////////////////////////////////////////
// DOOR PLACEMENT
////////////////////////////////////////////////////////////////////////////////

/// Adds spaced, closed doors where a narrow hallway meets a room boundary.
fn add_doors(map: &mut Map, rooms: &[Room], rng: &mut impl Rng) {
    let mut candidates = Vec::new();
    for y in 1..map.height() - 1 {
        for x in 1..map.width() - 1 {
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

////////////////////////////////////////////////////////////////////////////////
// OBSTACLE PLACEMENT
////////////////////////////////////////////////////////////////////////////////

/// Keeps decoration away from room centers, the player, and narrow entrances.
fn obstacle_site(map: &Map, room: &Room, start: Position, position: Position) -> bool {
    room.contains(position)
        && position != room.center()
        && position
            .x
            .abs_diff(start.x)
            .max(position.y.abs_diff(start.y))
            > 2
        && map
            .get(position)
            .is_some_and(|tile| tile.kind == TileType::Floor)
        && map
            .neighbors(position)
            .all(|next| map.get(next).unwrap().kind == TileType::Floor)
}

/// Applies a whole obstacle or restores its floor tiles if it seals another route.
fn try_obstacle(
    map: &mut Map,
    tiles: &[Position],
    kind: TileType,
    start: Position,
    connected: &mut usize,
) -> bool {
    for &position in tiles {
        map.set(position, kind);
    }
    if explore(map, start).1 == *connected - tiles.len() {
        *connected -= tiles.len();
        return true;
    }
    for &position in tiles {
        map.set(position, TileType::Floor);
    }
    false
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
            if pool
                .iter()
                .all(|&position| obstacle_site(map, room, start, position))
                && try_obstacle(map, &pool, TileType::Water, start, &mut connected_count)
            {
                break;
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
        let mut candidates: Vec<_> = room
            .positions()
            .filter(|&position| obstacle_site(map, room, start, position))
            .collect();
        candidates.shuffle(rng);
        let target = rng.random_range(1..=2);
        let mut placed = 0;
        for position in candidates {
            if placed == target {
                break;
            }
            if try_obstacle(map, &[position], TileType::Rock, start, &mut walkable_count) {
                placed += 1;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// ROOM GEOMETRY
////////////////////////////////////////////////////////////////////////////////

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

    /// Visits the room's actual shape rather than its bounding-box corners.
    fn positions(&self) -> impl Iterator<Item = Position> + '_ {
        (self.y..self.y + self.height)
            .flat_map(|y| (self.x..self.x + self.width).map(move |x| Position::new(x, y)))
            .filter(|&position| self.contains(position))
    }

    /// Turns the room's tiles into floor.
    fn carve(&self, map: &mut Map) {
        for position in self.positions() {
            map.set(position, TileType::Floor);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// CORRIDOR CARVING
////////////////////////////////////////////////////////////////////////////////

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
    use std::collections::HashSet;

    ////////////////////////////////////////////////////////////////////////////////
    // POPULATION TESTS
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn populations_are_reproducible_valid_and_depth_gated() {
        for depth in [0, 1, 2, 3, 4, 8, 20, u32::MAX] {
            for seed in 0..20 {
                let first = LevelGenerator::generate_level(depth, &mut StdRng::seed_from_u64(seed));
                let second =
                    LevelGenerator::generate_level(depth, &mut StdRng::seed_from_u64(seed));
                let snapshot = |level: &Level| {
                    level
                        .enemies()
                        .map(|(id, enemy)| {
                            (id, *enemy.kind(), *enemy.position(), enemy.stats().hp())
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(snapshot(&first), snapshot(&second));
                let item_snapshot = |level: &Level| {
                    level
                        .ground_items()
                        .iter()
                        .map(|entry| (entry.position, entry.item.clone()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(item_snapshot(&first), item_snapshot(&second));
                let mut occupied = HashSet::new();
                for (_, enemy) in first.enemies() {
                    assert!(first.is_spawn_tile(*enemy.position()));
                    assert!(occupied.insert(*enemy.position()));
                    let profile = ENEMY_SPAWN_PROFILES
                        .iter()
                        .find(|profile| profile.kind == *enemy.kind())
                        .unwrap();
                    assert!(depth >= profile.minimum_depth);
                }
                for entry in first.ground_items() {
                    assert!(first.is_spawn_tile(entry.position));
                    assert!(occupied.insert(entry.position));
                    assert!(entry.item.level <= depth.saturating_add(1));
                }
                let (enemies, items) = target_counts(depth);
                assert_eq!(first.enemies().count(), enemies);
                assert_eq!(first.ground_items().len(), items);
            }
        }
    }

    #[test]
    fn cramped_and_empty_floors_stop_when_candidates_run_out() {
        for free_tiles in 0..5 {
            let mut map = Map::new(8, 3);
            let start = Position::new(1, 1);
            map.set(start, TileType::Floor);
            for x in 2..2 + free_tiles {
                map.set(Position::new(x, 1), TileType::Floor);
            }
            let mut level = Level::new(map, 0, start, Position::new(7, 1), [start]);
            populate(&mut level, &mut StdRng::seed_from_u64(7));
            assert_eq!(
                level.enemies().count() + level.ground_items().len(),
                free_tiles
            );
            let count = level.enemies().count();
            populate(&mut level, &mut StdRng::seed_from_u64(8));
            assert_eq!(level.enemies().count(), count);
        }
    }

    #[test]
    fn average_enemy_difficulty_increases_across_seeded_floors() {
        let mut previous_average = 0.0;
        for depth in [0, 2, 4, 8, 20] {
            let (mut total, mut count) = (0_u64, 0_u64);
            for seed in 0..100 {
                let level = LevelGenerator::generate_level(depth, &mut StdRng::seed_from_u64(seed));
                for (_, enemy) in level.enemies() {
                    total += u64::from(
                        ENEMY_SPAWN_PROFILES
                            .iter()
                            .find(|profile| profile.kind == *enemy.kind())
                            .unwrap()
                            .difficulty,
                    );
                    count += 1;
                }
            }
            let average = total as f64 / count as f64;
            assert!(
                average > previous_average,
                "difficulty did not rise at depth {depth}"
            );
            previous_average = average;
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // STARTING ROOM AND SPAWN ELIGIBILITY TESTS
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn spawn_tiles_exclude_the_whole_starting_room_for_both_shapes() {
        for circular in [false, true] {
            let room = Room {
                x: 1,
                y: 1,
                width: 9,
                height: 9,
                circular,
            };
            let mut map = Map::new(14, 12);
            room.carve(&mut map);
            let nearby_corridor = Position::new(10, 5);
            map.set(nearby_corridor, TileType::Floor);
            let stairs = Position::new(11, 5);
            map.set(stairs, TileType::StairsDown);
            // A circular room's bounding-box corner is outside its actual shape.
            let corner = Position::new(1, 1);
            map.set(corner, TileType::Floor);
            let level = Level::new(map, 0, room.center(), stairs, room.positions());
            let distant_room_tile = Position::new(5, 1);
            assert!(distant_room_tile.distance(room.center()) > 2);
            assert!(!level.is_spawn_tile(distant_room_tile));
            for position in room.positions() {
                assert!(!level.is_spawn_tile(position));
            }
            assert_eq!(level.is_spawn_tile(corner), circular);
            assert!(level.is_spawn_tile(nearby_corridor));
            assert!(!level.is_spawn_tile(stairs));
        }
    }

    #[test]
    fn generated_levels_retain_their_starting_room_spawn_exclusion() {
        for depth in [0, 8] {
            for seed in 0..20 {
                let (_, room) = generate_layout(depth, &mut StdRng::seed_from_u64(seed));
                let level = LevelGenerator::generate_level(depth, &mut StdRng::seed_from_u64(seed));
                for position in room.positions() {
                    assert!(!level.is_spawn_tile(position));
                }
                assert!(
                    level
                        .get_map()
                        .positions()
                        .any(|position| level.is_spawn_tile(position))
                );
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // TERRAIN AND NAVIGATION TESTS
    ////////////////////////////////////////////////////////////////////////////////

    /// Generates levels for several depths and seeds, so circles are included.
    fn sample_levels() -> impl Iterator<Item = Level> {
        (0..10).flat_map(|depth| {
            (0..20).map(move |seed| {
                LevelGenerator::generate_level(depth, &mut StdRng::seed_from_u64(seed))
            })
        })
    }

    #[test]
    fn generated_levels_preserve_navigation_invariants() {
        let (mut doors, mut rocks, mut water) = (0, 0, 0);
        for level in sample_levels() {
            let map = level.get_map();
            let start = level.player_start();
            let exit = level.stairs_down();
            assert_ne!(start, exit);
            assert_eq!(map.get(start).unwrap().kind, TileType::Floor);
            assert_eq!(map.get(exit).unwrap().kind, TileType::StairsDown);
            let (mut connected_tiles, mut stairs) = (0, 0);
            for position in map.positions() {
                let (x, y) = (position.x, position.y);
                let kind = map.get(position).unwrap().kind;
                if x == 0 || y == 0 || x + 1 == map.width() || y + 1 == map.height() {
                    assert_eq!(kind, TileType::Wall);
                }
                if kind.is_walkable() || matches!(kind, TileType::Door { .. }) {
                    connected_tiles += 1;
                }
                match kind {
                    TileType::StairsDown => stairs += 1,
                    TileType::Door { open } => {
                        doors += 1;
                        assert!(!open);
                        assert!(!level.is_walkable(position));
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
                    TileType::Rock | TileType::Water => {
                        if kind == TileType::Rock {
                            rocks += 1;
                        } else {
                            water += 1;
                        }
                        assert!(level.depth() > 0);
                        assert!(!level.is_walkable(position));
                        assert!(x.abs_diff(start.x).max(y.abs_diff(start.y)) > 2);
                    }
                    _ => {}
                }
            }
            assert_eq!(stairs, 1);
            // Independent flood fill: verify all terrain is reachable after opening doors.
            let mut reached = std::collections::HashSet::from([start]);
            let mut frontier = vec![start];
            while let Some(pos) = frontier.pop() {
                for next in [
                    pos.x.checked_sub(1).map(|x| Position::new(x, pos.y)),
                    pos.y.checked_sub(1).map(|y| Position::new(pos.x, y)),
                    Some(Position::new(pos.x + 1, pos.y)),
                    Some(Position::new(pos.x, pos.y + 1)),
                ]
                .into_iter()
                .flatten()
                {
                    let passable = map.get(next).is_some_and(|tile| {
                        tile.kind.is_walkable() || matches!(tile.kind, TileType::Door { .. })
                    });
                    if passable && reached.insert(next) {
                        frontier.push(next);
                    }
                }
            }
            assert_eq!(reached.len(), connected_tiles);
            assert!(reached.contains(&exit));
        }
        assert!(doors > 0 && rocks > 0 && water > 0);
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
            for y in 0..map.height() {
                for x in 0..map.width() {
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
                for y in 0..map.height() {
                    for x in 0..map.width() {
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
            let water: std::collections::HashSet<_> = map
                .positions()
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

    #[test]
    fn rejected_obstacles_restore_floor_and_connected_count() {
        let mut map = Map::new(7, 5);
        let start = Position::new(1, 2);
        carve_line(&mut map, start, Position::new(5, 2));
        let bridge = Position::new(3, 2);
        let mut connected = explore(&map, start).1;
        assert!(!try_obstacle(
            &mut map,
            &[bridge],
            TileType::Rock,
            start,
            &mut connected
        ));
        assert_eq!(map.get(bridge).unwrap().kind, TileType::Floor);
        assert_eq!(connected, 5);
        // A parallel path makes the same obstacle safe to keep.
        carve_line(&mut map, Position::new(1, 1), Position::new(5, 1));
        connected = explore(&map, start).1;
        assert!(try_obstacle(
            &mut map,
            &[bridge],
            TileType::Rock,
            start,
            &mut connected
        ));
        assert_eq!(connected, 9);
    }
}
