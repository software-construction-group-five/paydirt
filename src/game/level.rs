//! The current dungeon floor (SDD 3.4).
//!
//! Level owns terrain, navigation anchors, enemies, and ground items.

pub mod generator;
pub mod map;

use std::collections::{BTreeMap, HashSet};

use crate::game::{enemy::Enemy, item::Item, position::Position};
use map::Map;

////////////////////////////////////////////////////////////////////////////////
// ENTITY TYPES
////////////////////////////////////////////////////////////////////////////////

/// Stable enemy identity within one Level. IDs are never reused on that floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnemyId(u64);

/// An owned item and its location on the floor, separate from carried items.
pub struct GroundItem {
    /// The actual item transferred when picked up or dropped.
    pub item: Item,
    /// The map tile holding the item.
    pub position: Position,
}

////////////////////////////////////////////////////////////////////////////////
// LEVEL STATE
////////////////////////////////////////////////////////////////////////////////

/// One generated dungeon floor, including navigation anchors and entity storage.
pub struct Level {
    map: Map,
    depth: u32,
    player_start: Position,
    stairs_down: Position,
    starting_room: HashSet<Position>,
    enemies: BTreeMap<EnemyId, Enemy>,
    next_enemy_id: Option<u64>,
    ground_items: Vec<GroundItem>,
}

impl Level {
    // Construction stays inside this module so generation establishes the map,
    // anchors, and placement invariants together.
    fn new(
        map: Map,
        depth: u32,
        player_start: Position,
        stairs_down: Position,
        starting_room: impl IntoIterator<Item = Position>,
    ) -> Self {
        Self {
            map,
            depth,
            player_start,
            stairs_down,
            starting_room: starting_room.into_iter().collect(),
            enemies: BTreeMap::new(),
            next_enemy_id: Some(0),
            ground_items: Vec::new(),
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // TERRAIN AND PROGRESSION ACCESS
    ////////////////////////////////////////////////////////////////////////////////

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

    /// Opens the closed door at `position`. Returns false, changing nothing,
    /// if there is no closed door there (supports Game Contract 2).
    /// Game refreshes visibility afterward, since an open door no longer blocks sight.
    pub fn open_door(&mut self, position: Position) -> bool {
        match self.map.get(position).map(|tile| tile.kind) {
            Some(map::TileType::Door { open: false }) => {
                self.map.set(position, map::TileType::Door { open: true });
                true
            }
            _ => false,
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // PLAYER VISIBILITY
    ////////////////////////////////////////////////////////////////////////////////

    /// Refreshes tile visibility from the player while retaining seen terrain.
    /// Game calls this on floor entry, movement, and sight-blocking terrain changes.
    /// Invalid positions clear current sight and return false without losing memory.
    pub fn update_visibility(&mut self, player_position: Position) -> bool {
        self.map.update_visibility(player_position)
    }

    /// Returns currently visible enemies, using their current tile's visibility.
    /// Seen terrain alone does not reveal entities; IDs remain floor-local.
    pub fn visible_enemies(&self) -> impl Iterator<Item = (EnemyId, &Enemy)> {
        self.enemies()
            .filter(|(_, enemy)| self.map.is_visible(*enemy.position()))
    }

    /// Returns currently visible ground items with their original storage indices.
    /// Filtering never changes indices used to select or remove an item.
    pub fn visible_ground_items(&self) -> impl Iterator<Item = (usize, &GroundItem)> {
        self.ground_items
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.map.is_visible(entry.position))
    }

    ////////////////////////////////////////////////////////////////////////////////
    // SPAWN ELIGIBILITY
    ////////////////////////////////////////////////////////////////////////////////

    /// Whether terrain permits an initial enemy or item spawn (contract 19).
    /// Excludes the entire starting room and stairs, but does not check occupancy.
    /// Runtime movement and item drops use different, less restrictive rules.
    pub fn is_spawn_tile(&self, position: Position) -> bool {
        !self.starting_room.contains(&position)
            && self
                .map
                .get(position)
                .is_some_and(|tile| tile.kind == map::TileType::Floor)
    }

    ////////////////////////////////////////////////////////////////////////////////
    // ENEMY STORAGE AND MOVEMENT
    ////////////////////////////////////////////////////////////////////////////////

    /// Iterates enemies in insertion-ID order without allowing unchecked movement.
    pub fn enemies(&self) -> impl Iterator<Item = (EnemyId, &Enemy)> {
        self.enemies.iter().map(|(&id, enemy)| (id, enemy))
    }

    /// Looks up an enemy by its floor-local ID; removed or unknown IDs are absent.
    pub fn enemy(&self, id: EnemyId) -> Option<&Enemy> {
        self.enemies.get(&id)
    }

    /// Finds the enemy occupying a tile, independently of terrain walkability.
    pub fn enemy_at(&self, position: Position) -> Option<EnemyId> {
        self.enemies()
            .find(|(_, enemy)| *enemy.position() == position)
            .map(|(id, _)| id)
    }

    /// Inserts an enemy on walkable, unoccupied terrain and assigns a fresh ID.
    /// Returns ownership on rejection, including ID exhaustion. Game checks player
    /// occupancy; generation applies starting-room and stairs restrictions separately.
    pub fn insert_enemy(&mut self, enemy: Enemy) -> Result<EnemyId, Enemy> {
        let position = *enemy.position();
        if !self.is_walkable(position) || self.enemy_at(position).is_some() {
            return Err(enemy);
        }
        let Some(next) = self.next_enemy_id else {
            return Err(enemy);
        };
        let id = EnemyId(next);
        self.next_enemy_id = next.checked_add(1);
        self.enemies.insert(id, enemy);
        Ok(id)
    }

    /// Moves a known enemy to walkable terrain free of other enemies.
    /// Staying on its current tile succeeds. Rejection leaves all state unchanged;
    /// Game must check the player position before requesting movement.
    pub fn move_enemy(&mut self, id: EnemyId, position: Position) -> bool {
        if !self.is_walkable(position) || self.enemy_at(position).is_some_and(|other| other != id) {
            return false;
        }
        let Some(enemy) = self.enemies.get_mut(&id) else {
            return false;
        };
        enemy.update_position(position);
        true
    }

    /// Removes and returns an enemy without changing any other enemy's ID.
    pub fn remove_enemy(&mut self, id: EnemyId) -> Option<Enemy> {
        self.enemies.remove(&id)
    }

    ////////////////////////////////////////////////////////////////////////////////
    // GROUND ITEM STORAGE
    ////////////////////////////////////////////////////////////////////////////////

    /// Borrows ground entries in insertion order. Indices change after removal.
    pub fn ground_items(&self) -> &[GroundItem] {
        &self.ground_items
    }

    /// Finds ground entries at a tile, with indices usable for immediate removal.
    /// Multiple items may share a tile, including a tile occupied by an enemy.
    pub fn ground_items_at(
        &self,
        position: Position,
    ) -> impl Iterator<Item = (usize, &GroundItem)> {
        self.ground_items
            .iter()
            .enumerate()
            .filter(move |(_, entry)| entry.position == position)
    }

    /// Transfers an existing item onto walkable terrain; returns it on rejection.
    pub fn insert_ground_item(&mut self, item: Item, position: Position) -> Result<(), Item> {
        if !self.is_walkable(position) {
            return Err(item);
        }
        self.ground_items.push(GroundItem { item, position });
        Ok(())
    }

    /// Removes the selected entry and transfers ownership of its original item.
    /// Unknown indices leave storage unchanged; later indices shift after removal.
    pub fn remove_ground_item(&mut self, index: usize) -> Option<GroundItem> {
        if index >= self.ground_items.len() {
            return None;
        }
        Some(self.ground_items.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{enemy::EnemyType, item::get_item};
    use map::TileType;

    #[test]
    fn entity_visibility_follows_current_tiles_and_preserves_item_indices() {
        let mut map = Map::new(7, 3);
        for x in 0..7 {
            map.set(Position::new(x, 1), TileType::Floor);
        }
        let door = Position::new(3, 1);
        map.set(door, TileType::Door { open: false });
        let start = Position::new(1, 1);
        let hidden = Position::new(5, 1);
        let mut level = Level::new(map, 0, start, Position::new(6, 1), [start]);
        let visible_enemy = level
            .insert_enemy(Enemy::new(EnemyType::Slime, start))
            .ok()
            .unwrap();
        let hidden_enemy = level
            .insert_enemy(Enemy::new(EnemyType::Ghost, hidden))
            .ok()
            .unwrap();
        level.insert_ground_item(get_item(0), hidden).unwrap();
        level.insert_ground_item(get_item(2), start).unwrap();
        assert_eq!(level.visible_enemies().count(), 0);
        level.update_visibility(start);
        assert_eq!(
            level
                .visible_enemies()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            [visible_enemy]
        );
        assert_eq!(
            level
                .visible_ground_items()
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
            [1]
        );
        // Entities moving out of sight disappear immediately without caching visibility.
        assert!(level.move_enemy(visible_enemy, Position::new(4, 1)));
        assert_eq!(level.visible_enemies().count(), 0);
        level.update_visibility(hidden);
        assert!(level.get_map().get(start).unwrap().seen);
        assert!(!level.get_map().is_visible(start));
        assert_eq!(level.visible_enemies().count(), 2);
        assert!(level.visible_enemies().any(|(id, _)| id == hidden_enemy));
        assert_eq!(
            level
                .visible_ground_items()
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(level.ground_items().len(), 2);
        level.map.set(door, TileType::Door { open: true });
        level.update_visibility(start);
        assert_eq!(level.visible_ground_items().count(), 2);
    }

    fn empty_level() -> Level {
        let mut map = Map::new(5, 5);
        for x in 1..4 {
            map.set(Position::new(x, 1), TileType::Floor);
        }
        map.set(Position::new(1, 2), TileType::Door { open: false });
        map.set(Position::new(2, 2), TileType::Rock);
        map.set(Position::new(3, 2), TileType::Water);
        map.set(Position::new(3, 1), TileType::StairsDown);
        Level::new(
            map,
            0,
            Position::new(1, 1),
            Position::new(3, 1),
            [Position::new(1, 1)],
        )
    }

    fn insert(level: &mut Level, x: usize) -> EnemyId {
        level
            .insert_enemy(Enemy::new(EnemyType::Goblin, Position::new(x, 1)))
            .unwrap_or_else(|_| panic!("test enemy placement failed"))
    }

    #[test]
    fn enemy_ids_survive_removal_and_are_never_reused() {
        let mut level = empty_level();
        assert_eq!(level.enemies().count(), 0);
        assert!(level.ground_items().is_empty());
        let first = insert(&mut level, 1);
        let second = insert(&mut level, 2);
        assert_eq!(level.remove_enemy(first).unwrap().stats().hp(), 100);
        assert!(level.enemy(first).is_none());
        assert!(level.remove_enemy(first).is_none());
        assert!(!level.move_enemy(first, Position::new(3, 1)));
        assert_eq!(
            level.enemy(second).unwrap().position(),
            &Position::new(2, 1)
        );
        let third = insert(&mut level, 1);
        assert_ne!(first, third);
        assert_eq!(
            level.enemies().map(|(id, _)| id).collect::<Vec<_>>(),
            [second, third]
        );
        assert_eq!(level.enemy_at(Position::new(1, 1)), Some(third));
    }

    #[test]
    fn invalid_enemy_placements_and_moves_preserve_storage() {
        let mut level = empty_level();
        let first = insert(&mut level, 1);
        let second = insert(&mut level, 2);
        for position in [
            Position::new(0, 0),
            Position::new(5, 1),
            Position::new(1, 2),
            Position::new(2, 2),
            Position::new(3, 2),
            Position::new(2, 1),
        ] {
            let rejected = level
                .insert_enemy(Enemy::new(EnemyType::Ghost, position))
                .err()
                .unwrap();
            assert_eq!(*rejected.position(), position);
            assert!(!level.move_enemy(first, position));
            assert_eq!(*level.enemy(first).unwrap().position(), Position::new(1, 1));
        }
        assert_eq!(level.enemies().count(), 2);
        assert!(level.is_walkable(Position::new(2, 1)));
        assert!(level.move_enemy(first, Position::new(1, 1)));
        assert!(level.move_enemy(first, Position::new(3, 1)));
        assert!(level.enemy_at(Position::new(1, 1)).is_none());
        assert_eq!(level.enemy_at(Position::new(3, 1)), Some(first));
        assert_eq!(level.enemy_at(Position::new(2, 1)), Some(second));
    }

    #[test]
    fn enemy_id_exhaustion_returns_ownership_without_reusing_ids() {
        let mut level = empty_level();
        level.next_enemy_id = Some(u64::MAX);
        let last = insert(&mut level, 1);
        level.remove_enemy(last);
        assert!(
            level
                .insert_enemy(Enemy::new(EnemyType::Slime, Position::new(2, 1)))
                .is_err()
        );
        assert_eq!(level.enemies().count(), 0);
    }

    #[test]
    fn ground_entries_preserve_distinct_copies_and_original_item_data() {
        let mut level = empty_level();
        insert(&mut level, 1);
        let position = Position::new(1, 1);
        let mut first = get_item(0);
        first.name = "Worn pickaxe".to_owned();
        first.level = 7;
        let mut second = get_item(0);
        second.name = "Polished pickaxe".to_owned();
        level.insert_ground_item(first.clone(), position).unwrap();
        level.insert_ground_item(second.clone(), position).unwrap();
        assert_eq!(
            level
                .ground_items_at(position)
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(level.ground_items_at(Position::new(4, 4)).count(), 0);
        assert!(level.remove_ground_item(usize::MAX).is_none());
        assert_eq!(level.ground_items().len(), 2);
        let removed = level.remove_ground_item(1).unwrap();
        assert_eq!(removed.item, second);
        assert_eq!(removed.position, position);
        assert_eq!(level.remove_ground_item(0).unwrap().item, first);
        assert!(level.remove_ground_item(0).is_none());
        assert!(level.ground_items().is_empty());
    }

    #[test]
    fn rejected_ground_insertion_returns_the_existing_item() {
        let mut level = empty_level();
        for position in [
            Position::new(0, 0),
            Position::new(5, 5),
            Position::new(1, 2),
            Position::new(2, 2),
            Position::new(3, 2),
        ] {
            let mut item = get_item(2);
            item.name = "Custom potion".to_owned();
            assert_eq!(level.insert_ground_item(item.clone(), position), Err(item));
        }
        assert!(level.ground_items().is_empty());
    }

    #[test]
    fn spawn_restrictions_do_not_block_runtime_stairs_or_start_room_access() {
        let mut level = empty_level();
        let start = level.player_start();
        let stairs = level.stairs_down();
        assert!(!level.is_spawn_tile(start));
        assert!(!level.is_spawn_tile(stairs));
        assert!(level.is_spawn_tile(Position::new(2, 1)));
        assert!(!level.is_spawn_tile(Position::new(usize::MAX, 0)));
        for position in [
            Position::new(0, 0),
            Position::new(1, 2),
            Position::new(2, 2),
            Position::new(3, 2),
        ] {
            assert!(!level.is_spawn_tile(position));
        }
        let id = insert(&mut level, 3);
        assert!(level.move_enemy(id, start));
        assert!(level.move_enemy(id, stairs));
        level.insert_ground_item(get_item(0), start).unwrap();
        level.insert_ground_item(get_item(2), stairs).unwrap();
    }

    #[test]
    fn removing_ground_entry_shifts_indices_without_losing_other_items() {
        let mut level = empty_level();
        let position = Position::new(2, 1);
        for index in 0..3 {
            level.insert_ground_item(get_item(index), position).unwrap();
        }
        assert_eq!(level.remove_ground_item(0).unwrap().item, get_item(0));
        assert_eq!(
            level
                .ground_items_at(position)
                .map(|(index, entry)| (index, entry.item.index))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 2)]
        );
        assert_eq!(level.remove_ground_item(1).unwrap().item, get_item(2));
        assert_eq!(level.ground_items()[0].item, get_item(1));
    }

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
        let level = Level::new(map, 2, start, exit, [start]);
        assert!(level.is_walkable(start));
        assert!(level.is_walkable(exit));
        assert!(level.is_walkable(Position::new(1, 2)));
        assert!(!level.is_walkable(Position::new(2, 1)));
        assert!(!level.is_walkable(Position::new(0, 0)));
        assert!(!level.is_walkable(Position::new(2, 2)));
        assert!(!level.is_walkable(Position::new(3, 2)));
        assert!(!level.is_walkable(Position::new(5, 5)));
    }

    #[test]
    fn open_door_opens_only_closed_doors() {
        let mut map = Map::new(3, 1);
        let floor = Position::new(0, 0);
        let door = Position::new(1, 0);
        map.set(floor, TileType::Floor);
        map.set(door, TileType::Door { open: false });
        let mut level = Level::new(map, 0, floor, floor, [floor]);
        assert!(!level.is_walkable(door));
        assert!(level.open_door(door));
        assert!(level.is_walkable(door));
        assert!(!level.open_door(door));
        assert!(!level.open_door(floor));
        assert!(!level.open_door(Position::new(5, 5)));
    }
}
