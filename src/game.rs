//! Subsystem: Game Logic & Management.
//!
//! Owns all game state and gameplay rules. Provides SDD subsystem contracts:
//! 1. Game Action Processing
//! 2. Game State Access

pub mod action;
pub mod combat;
pub mod enemy;
pub mod equipment;
pub mod inventory;
pub mod item;
pub mod level;
pub mod message_log;
pub mod player;
pub mod position;
pub mod screen;
pub mod stats;

use action::{Action, TurnResult};
use combat::{attack_damage, equipment_drop};
use equipment::Equipment;
use inventory::Inventory;
use level::{EnemyId, Level, generator::LevelGenerator};
use message_log::MessageLog;
use player::Player;
use rand::{SeedableRng, rngs::StdRng};
use screen::Screen;
use stats::Stats;

// TODO(joseph): confirm starting stats with the team
const STARTING_MAX_HP: i32 = 200;
const STARTING_ATTACK: i32 = 15;
const STARTING_DEFENSE: i32 = 5;
const MESSAGE_LOG_CAPACITY: usize = 50;

/// Owns screen navigation, the active dungeon floor, the player, and the message log.
pub struct Game {
    screen: Screen,
    menu_selection: usize,
    level: Option<Level>,
    player: Option<Player>,
    log: MessageLog,
    rng: StdRng,
    should_quit: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    /// Starts at the main menu without generating a level before New Game is chosen.
    pub fn new() -> Self {
        Self {
            screen: Screen::MainMenu,
            menu_selection: 0,
            level: None,
            player: None,
            log: MessageLog::new(MESSAGE_LOG_CAPACITY),
            rng: StdRng::from_rng(&mut rand::rng()),
            should_quit: false,
        }
    }

    /// Returns the active screen for rendering and key interpretation.
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Returns the highlighted main-menu row (New Game or Quit).
    pub fn menu_selection(&self) -> usize {
        self.menu_selection
    }

    /// Returns the active level, absent before a session starts.
    pub fn level(&self) -> Option<&Level> {
        self.level.as_ref()
    }

    /// Returns the active player, absent before a session starts (contract 6).
    pub fn player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Returns the current session's messages (Game State Access).
    pub fn message_log(&self) -> &MessageLog {
        &self.log
    }

    /// Reports whether the application loop should exit and restore the terminal.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Processes menu, pause, and movement actions, and reports whether a
    /// gameplay turn was spent (Game Contract 2).
    pub fn process_action(&mut self, action: &Action) -> TurnResult {
        if self.should_quit {
            return TurnResult::NotSpent;
        }
        if matches!(action, Action::Quit) {
            self.should_quit = true;
            return TurnResult::NotSpent;
        }
        match self.screen {
            Screen::MainMenu => match action {
                Action::MoveUp | Action::MoveDown => self.menu_selection = 1 - self.menu_selection,
                Action::Confirm if self.menu_selection == 0 => self.initialize_game(),
                Action::Confirm => self.should_quit = true,
                _ => {}
            },
            Screen::Playing => match action {
                Action::Pause => self.screen = Screen::Paused,
                Action::MoveUp => return self.move_player(0, -1),
                Action::MoveDown => return self.move_player(0, 1),
                Action::MoveLeft => return self.move_player(-1, 0),
                Action::MoveRight => return self.move_player(1, 0),
                Action::Wait => return TurnResult::Spent,
                _ => {}
            },
            Screen::Inventory => {}
            Screen::Paused => match action {
                Action::Pause | Action::Confirm => self.screen = Screen::Playing,
                Action::Back => self.return_to_menu(),
                _ => {}
            },
            Screen::GameOver => {
                if matches!(action, Action::Confirm | Action::Back) {
                    self.return_to_menu();
                }
            }
        }
        TurnResult::NotSpent
    }

    /// Starts a fresh session: new floor, new player at its start, empty log (contract 1).
    fn initialize_game(&mut self) {
        let mut level = LevelGenerator::generate_level(0, &mut self.rng);
        level.update_visibility(level.player_start());
        let stats = Stats::new(STARTING_MAX_HP, STARTING_ATTACK, STARTING_DEFENSE);
        self.player = Some(Player::new(
            level.player_start(),
            stats,
            Inventory::new(),
            Equipment::new(),
        ));
        self.level = Some(level);
        self.log.clear();
        self.screen = Screen::Playing;
    }

    /// Moves the player one tile if the target is on the map and walkable;
    /// otherwise the player stays put (Game Contract 2, using Level Contract 9
    /// and Player Contract 7). Moving into a closed door opens it instead, and
    /// the player stays put. Bumping an enemy attacks without moving.
    /// Updates line of sight after a move. Always spends the turn, even when blocked.
    fn move_player(&mut self, dx: isize, dy: isize) -> TurnResult {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_mut()) else {
            return TurnResult::Spent;
        };
        let Some(target) = player.get_position().offset(dx, dy) else {
            return TurnResult::Spent;
        };
        // Bumping a closed door opens it; walking through takes another turn
        if level.open_door(target) {
            // An open door no longer blocks sight, so refresh what the player sees
            level.update_visibility(player.get_position());
            return TurnResult::Spent;
        }
        if !level.is_walkable(target) {
            return TurnResult::Spent;
        }
        if let Some(id) = level.enemy_at(target) {
            self.attack_enemy(id);
            return TurnResult::Spent;
        }
        player.set_position(target);
        level.update_visibility(target);
        TurnResult::Spent
    }

    /// Resolves a validated cardinal bump attack without advancing other actors.
    fn attack_enemy(&mut self, id: EnemyId) {
        let (Some(player), Some(level)) = (self.player.as_ref(), self.level.as_mut()) else {
            return;
        };
        let Some(enemy) = level.enemy(id) else {
            return;
        };
        if player.get_stats().is_dead()
            || !player
                .get_position()
                .is_cardinal_neighbor(*enemy.position())
        {
            return;
        }
        if enemy.stats().is_dead() {
            self.defeat_enemy(id);
            return;
        }
        let defender = format!("{:?} ({id:?})", enemy.kind());
        let damage = attack_damage(player.effective_attack(), enemy.stats().defense());
        let effect = player.weapon_effect();
        let effect_name = effect.as_ref().map(|effect| effect.name().to_owned());
        let Some(outcome) = level.damage_enemy(id, damage, effect) else {
            return;
        };
        self.log.add_message(format!(
            "You hit {defender} for {} damage.",
            outcome.hp_lost
        ));
        if outcome.effect_applied
            && let Some(name) = effect_name
        {
            self.log
                .add_message(format!("You apply {name} to {defender}."));
        }
        if outcome.is_dead {
            self.defeat_enemy(id);
        }
    }

    /// Removes a defeated enemy once and resolves its equipment drop.
    /// Effect deaths will use this same path when turn scheduling is connected.
    fn defeat_enemy(&mut self, id: EnemyId) {
        let Some(level) = self.level.as_mut() else {
            return;
        };
        let Some(enemy) = level.enemy(id) else {
            return;
        };
        if !enemy.stats().is_dead() {
            return;
        }
        let position = *enemy.position();
        // Level insertion and movement guarantee walkable death tiles. Check
        // before removing ownership so an invariant failure cannot lose loot.
        assert!(
            level.is_walkable(position),
            "enemy death tile must be walkable"
        );
        let Some(enemy) = level.remove_enemy(id) else {
            return;
        };
        self.log
            .add_message(format!("You defeat {:?} ({id:?}).", enemy.kind()));
        if let Some(item) = equipment_drop(level.depth(), item::catalog(), &mut self.rng) {
            let message = format!("{:?} ({id:?}) drops {}.", enemy.kind(), item.name);
            level
                .insert_ground_item(item, position)
                .expect("validated enemy death tile accepts ground items");
            self.log.add_message(message);
        }
    }

    fn return_to_menu(&mut self) {
        self.screen = Screen::MainMenu;
        self.menu_selection = 0;
        self.level = None;
        self.player = None;
        self.log.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combat_game(
        attack: i32,
        target_hp: u32,
        effect: Option<stats::EffectSpec>,
    ) -> (Game, EnemyId, Action) {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        while level.remove_ground_item(0).is_some() {}
        let start = level.player_start();
        let (target, action) = [
            (start.offset(1, 0), Action::MoveRight),
            (start.offset(-1, 0), Action::MoveLeft),
            (start.offset(0, 1), Action::MoveDown),
            (start.offset(0, -1), Action::MoveUp),
        ]
        .into_iter()
        .find_map(|(position, action)| {
            position
                .filter(|&p| level.is_walkable(p))
                .map(|p| (p, action))
        })
        .unwrap();
        let id = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, target))
            .ok()
            .unwrap();
        level.damage_enemy(id, 100 - target_hp, None);
        let mut equipment = Equipment::new();
        if let Some(effect) = effect {
            equipment
                .equip(
                    item::get_item(0).with_combat(item::CombatModifiers::weapon(3, Some(effect))),
                )
                .unwrap();
        }
        game.player = Some(Player::new(
            start,
            Stats::new(200, attack, 5),
            Inventory::new(),
            equipment,
        ));
        (game, id, action)
    }

    #[test]
    fn bump_attack_spends_one_turn_without_moving_or_advancing_other_actors() {
        let (mut game, id, action) = combat_game(15, 100, None);
        let start = game.player().unwrap().get_position();
        let level = game.level.as_mut().unwrap();
        let other_position = level
            .get_map()
            .positions()
            .find(|&p| level.is_walkable(p) && p != start && level.enemy_at(p).is_none())
            .unwrap();
        let other = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Slime, other_position))
            .ok()
            .unwrap();
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), start);
        assert_eq!(game.player().unwrap().get_stats().hp(), 200);
        let level = game.level().unwrap();
        assert_eq!(level.enemy(id).unwrap().stats().hp(), 90);
        assert_eq!(level.enemy(other).unwrap().stats().hp(), 30);
        assert_eq!(*level.enemy(other).unwrap().position(), other_position);
        assert_eq!(
            game.message_log().history(),
            &[format!("You hit Goblin ({id:?}) for 10 damage.")]
        );
    }

    #[test]
    fn weapon_bonus_and_fresh_effects_apply_without_automatic_ticks() {
        let (mut game, id, action) =
            combat_game(15, 100, Some(stats::EffectSpec::new("Poison".into(), 7, 3)));
        for expected_hp in [87, 74] {
            assert_eq!(game.process_action(&action), TurnResult::Spent);
            assert_eq!(
                game.level().unwrap().enemy(id).unwrap().stats().hp(),
                expected_hp
            );
        }
        let effects = game
            .level()
            .unwrap()
            .enemy(id)
            .unwrap()
            .stats()
            .status_effects();
        assert_eq!(effects.len(), 2);
        assert!(effects.iter().all(|effect| effect.turns_remaining() == 3));
        let log = game.message_log().history();
        assert_eq!(log.len(), 4);
        assert!(log[0].contains("13 damage"));
        assert!(log[1].contains("apply Poison"));
    }

    #[test]
    fn lethal_bump_removes_only_target_and_requires_another_action_to_enter_tile() {
        let (mut game, id, action) =
            combat_game(15, 4, Some(stats::EffectSpec::new("Poison".into(), 7, 3)));
        let start = game.player().unwrap().get_position();
        let target = *game.level().unwrap().enemy(id).unwrap().position();
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert_eq!(game.player().unwrap().get_position(), start);
        let log = game.message_log().history();
        assert!(log[0].contains("4 damage"));
        assert!(log[1].contains("defeat Goblin"));
        assert!(!log.iter().any(|line| line.contains("Poison")));
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), target);
    }

    #[test]
    fn attacks_reject_distant_diagonal_missing_dead_player_and_paused_requests() {
        let (mut game, id, action) = combat_game(15, 100, None);
        let target = *game.level().unwrap().enemy(id).unwrap().position();
        for origin in [
            target.offset(1, 1).unwrap(),
            target.offset(2, 0).unwrap(),
            target,
        ] {
            game.player.as_mut().unwrap().set_position(origin);
            game.attack_enemy(id);
            assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 100);
        }
        let start = game.level().unwrap().player_start();
        game.player.as_mut().unwrap().set_position(start);
        game.process_action(&Action::Pause);
        assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 100);
        game.process_action(&Action::Pause);
        game.player.as_mut().unwrap().update_health(-200, None);
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 100);
        game.level.as_mut().unwrap().remove_enemy(id);
        game.attack_enemy(id);
        assert!(game.message_log().history().is_empty());
    }

    #[test]
    fn defeat_rolls_once_and_drops_owned_equipment_at_the_death_tile() {
        use rand::RngExt;
        let mut dropped = false;
        let mut skipped = false;
        for seed in 0..200 {
            let (mut game, id, action) = combat_game(150, 100, None);
            let position = *game.level().unwrap().enemy(id).unwrap().position();
            // Existing items can share the death tile with the new equipment.
            game.level
                .as_mut()
                .unwrap()
                .insert_ground_item(item::get_item(2), position)
                .unwrap();
            game.rng = StdRng::seed_from_u64(seed);
            let mut expected_rng = StdRng::seed_from_u64(seed);
            let expected = equipment_drop(0, item::catalog(), &mut expected_rng);
            game.process_action(&action);
            let items = game.level().unwrap().ground_items();
            if let Some(expected) = expected {
                dropped = true;
                assert_eq!(items.len(), 2);
                assert_eq!(items[1].item, expected);
                assert_eq!(items[1].position, position);
                assert!(
                    game.message_log()
                        .history()
                        .last()
                        .unwrap()
                        .contains("drops")
                );
            } else {
                skipped = true;
                assert_eq!(items.len(), 1);
            }
            let before_log = game.message_log().history().to_vec();
            let count = items.len();
            game.defeat_enemy(id);
            assert_eq!(game.level().unwrap().ground_items().len(), count);
            assert_eq!(game.message_log().history(), before_log);
            assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
        }
        assert!(dropped && skipped);
    }

    #[test]
    fn defeat_path_rejects_living_enemies_and_accepts_effect_deaths() {
        let (mut game, id, _) = combat_game(15, 1, None);
        game.defeat_enemy(id);
        assert!(game.level().unwrap().enemy(id).is_some());
        assert!(game.message_log().history().is_empty());
        let level = game.level.as_mut().unwrap();
        level.damage_enemy(id, 0, Some(stats::StatusEffect::new("Poison".into(), 5, 1)));
        assert!(level.tick_enemy_effects(id).unwrap().is_dead);
        game.defeat_enemy(id);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert!(game.message_log().history()[0].contains("defeat"));
    }

    #[test]
    fn menu_and_pause_navigation_preserves_the_active_level() {
        let mut game = Game::new();
        assert!(matches!(game.screen(), Screen::MainMenu));
        assert!(game.level().is_none());
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        let start = game.level().unwrap().player_start();
        assert!(game.level().unwrap().get_map().is_visible(start));
        assert!(game.level().unwrap().get_map().get(start).unwrap().seen);
        game.process_action(&Action::Pause);
        assert!(matches!(game.screen(), Screen::Paused));
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        assert_eq!(game.level().unwrap().player_start(), start);
        assert_eq!(game.level().unwrap().depth(), 0);
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::MainMenu));
        assert!(game.level().is_none());
        game.process_action(&Action::Confirm);
        assert_eq!(game.level().unwrap().depth(), 0);
    }

    #[test]
    fn new_game_places_player_at_level_start() {
        let mut game = Game::new();
        assert!(game.player().is_none());
        game.process_action(&Action::Confirm);
        let start = game.level().unwrap().player_start();
        assert!(game.level().unwrap().is_walkable(start));
        let player = game.player().unwrap();
        assert_eq!(player.get_position(), start);
        assert_eq!(player.get_stats().hp(), STARTING_MAX_HP);
        assert!(game.message_log().messages().is_empty());
    }

    #[test]
    fn returning_to_menu_clears_the_session() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(game.player().is_none());
        assert!(game.message_log().messages().is_empty());
    }

    #[test]
    fn moves_only_onto_walkable_tiles() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let start = game.player().unwrap().get_position();
        for (action, dx, dy) in [
            (Action::MoveUp, 0, -1),
            (Action::MoveDown, 0, 1),
            (Action::MoveLeft, -1, 0),
            (Action::MoveRight, 1, 0),
        ] {
            game.player.as_mut().unwrap().set_position(start);
            let expected = start
                .offset(dx, dy)
                .filter(|&target| {
                    let level = game.level().unwrap();
                    level.is_walkable(target) && level.enemy_at(target).is_none()
                })
                .unwrap_or(start);
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), expected);
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position) == map.has_line_of_sight(expected, position)
            }));

            if expected != start {
                // Exercise the merge boundary: terrain permits this move, but
                // Level's enemy occupancy must prevent the player entering it.
                game.player.as_mut().unwrap().set_position(start);
                game.level.as_mut().unwrap().update_visibility(start);
                let enemy = enemy::Enemy::new(enemy::EnemyType::Slime, expected);
                let id = game
                    .level
                    .as_mut()
                    .unwrap()
                    .insert_enemy(enemy)
                    .ok()
                    .unwrap();
                let seen: Vec<_> = game
                    .level()
                    .unwrap()
                    .get_map()
                    .positions()
                    .map(|position| game.level().unwrap().get_map().get(position).unwrap().seen)
                    .collect();
                assert_eq!(game.process_action(&action), TurnResult::Spent);
                assert_eq!(game.player().unwrap().get_position(), start);
                let level = game.level().unwrap();
                assert_eq!(level.enemy_at(expected), Some(id));
                assert_eq!(level.enemy(id).unwrap().stats().hp(), 17);
                assert!(
                    level
                        .get_map()
                        .positions()
                        .zip(seen)
                        .all(|(position, seen)| {
                            let tile = level.get_map().get(position).unwrap();
                            tile.seen == seen
                                && tile.visible
                                    == level.get_map().has_line_of_sight(start, position)
                        })
                );
                game.level.as_mut().unwrap().remove_enemy(id);
            }
        }
    }

    #[test]
    fn walking_into_a_wall_stops_the_player() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        // Rooms are enclosed, so walking left long enough always hits something
        // solid (or an enemy, which also blocks)
        for _ in 0..100 {
            game.process_action(&Action::MoveLeft);
        }
        let level = game.level().unwrap();
        let stopped = game.player().unwrap().get_position();
        assert!(level.is_walkable(stopped));
        assert!(
            stopped
                .offset(-1, 0)
                .is_none_or(|left| !level.is_walkable(left) || level.enemy_at(left).is_some())
        );
    }

    #[test]
    fn movement_is_ignored_while_paused() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        let start = game.player().unwrap().get_position();
        game.process_action(&Action::Pause);
        for action in [
            Action::MoveUp,
            Action::MoveDown,
            Action::MoveLeft,
            Action::MoveRight,
        ] {
            game.process_action(&action);
        }
        assert_eq!(game.player().unwrap().get_position(), start);
    }

    #[test]
    fn moves_and_waits_spend_a_turn_even_when_blocked() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        for _ in 0..100 {
            assert_eq!(game.process_action(&Action::MoveLeft), TurnResult::Spent);
        }
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
    }

    #[test]
    fn menus_and_pausing_do_not_spend_a_turn() {
        let mut game = Game::new();
        assert_eq!(game.process_action(&Action::MoveDown), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::MoveUp), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Confirm), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::MoveLeft), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
    }

    #[test]
    fn bumping_a_closed_door_opens_it_without_moving() {
        use level::map::TileType;
        // Not every floor has a door, so try new games until one does
        for seed in 0..50 {
            let mut game = Game::new();
            game.rng = StdRng::seed_from_u64(seed);
            game.process_action(&Action::Confirm);
            let level = game.level().unwrap();
            let map = level.get_map();
            let found = map.positions().find_map(|door| {
                if !matches!(map.get(door)?.kind, TileType::Door { open: false }) {
                    return None;
                }
                [
                    (Action::MoveRight, -1, 0),
                    (Action::MoveLeft, 1, 0),
                    (Action::MoveDown, 0, -1),
                    (Action::MoveUp, 0, 1),
                ]
                .into_iter()
                .find_map(|(action, dx, dy)| {
                    let beside = door.offset(dx, dy)?;
                    (level.is_walkable(beside) && level.enemy_at(beside).is_none())
                        .then_some((door, beside, action))
                })
            });
            let Some((door, beside, action)) = found else {
                continue;
            };
            game.player.as_mut().unwrap().set_position(beside);
            game.level.as_mut().unwrap().update_visibility(beside);
            let seen: Vec<_> = game
                .level()
                .unwrap()
                .get_map()
                .positions()
                .filter(|&position| game.level().unwrap().get_map().get(position).unwrap().seen)
                .collect();
            assert_eq!(game.process_action(&action), TurnResult::Spent);
            assert_eq!(game.player().unwrap().get_position(), beside);
            assert!(game.level().unwrap().is_walkable(door));
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position) == map.has_line_of_sight(beside, position)
            }));
            assert!(seen.iter().all(|&position| map.get(position).unwrap().seen));
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), door);
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position) == map.has_line_of_sight(door, position)
            }));
            return;
        }
        panic!("no closed door found in 50 generated levels");
    }

    #[test]
    fn quit_menu_entry_and_global_quit_end_the_session() {
        let mut game = Game::new();
        assert!(game.level().is_none());
        game.process_action(&Action::MoveDown);
        assert_eq!(game.menu_selection(), 1);
        game.process_action(&Action::Confirm);
        assert!(game.should_quit());
        assert!(game.level().is_none());
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::Quit);
        assert!(game.should_quit());
    }
}
