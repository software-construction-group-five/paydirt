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
use enemy::ai::{self, AiProfile, Intent};
use equipment::Equipment;
use inventory::Inventory;
use level::{EnemyId, Level, generator::LevelGenerator};
use message_log::MessageLog;
use player::Player;
use rand::{SeedableRng, rngs::StdRng};
use screen::Screen;
use stats::Stats;

// Initial combat tuning; keep equipment bonuses separate from these base stats.
const STARTING_MAX_HP: i32 = 40;
const STARTING_ATTACK: i32 = 0;
const STARTING_DEFENSE: i32 = 1;
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

    /// Processes a player action and advances enemy AI exactly once for each
    /// spent gameplay turn (Game Contract 2). Menu and pause actions do not
    /// advance enemies.
    pub fn process_action(&mut self, action: &Action) -> TurnResult {
        if self.should_quit {
            return TurnResult::NotSpent;
        }
        if matches!(action, Action::Quit) {
            self.should_quit = true;
            return TurnResult::NotSpent;
        }

        let result = match self.screen {
            Screen::MainMenu => {
                match action {
                    Action::MoveUp | Action::MoveDown => {
                        self.menu_selection = 1 - self.menu_selection
                    }
                    Action::Confirm if self.menu_selection == 0 => self.initialize_game(),
                    Action::Confirm => self.should_quit = true,
                    _ => {}
                }
                TurnResult::NotSpent
            }
            Screen::Playing => match action {
                Action::Pause => {
                    self.screen = Screen::Paused;
                    TurnResult::NotSpent
                }
                _ if self.level.is_none()
                    || !self
                        .player
                        .as_ref()
                        .is_some_and(|player| !player.get_stats().is_dead()) =>
                {
                    TurnResult::NotSpent
                }
                Action::MoveUp => self.move_player(0, -1),
                Action::MoveDown => self.move_player(0, 1),
                Action::MoveLeft => self.move_player(-1, 0),
                Action::MoveRight => self.move_player(1, 0),
                Action::Wait => TurnResult::Spent,
                _ => TurnResult::NotSpent,
            },
            Screen::Inventory => TurnResult::NotSpent,
            Screen::Paused => {
                match action {
                    Action::Pause | Action::Confirm => self.screen = Screen::Playing,
                    Action::Back => self.return_to_menu(),
                    _ => {}
                }
                TurnResult::NotSpent
            }
            Screen::GameOver => {
                if matches!(action, Action::Confirm | Action::Back) {
                    self.return_to_menu();
                }
                TurnResult::NotSpent
            }
        };

        if result == TurnResult::Spent {
            self.advance_enemy_turns();
        }
        result
    }

    /// Applies one AI decision per living enemy after a spent player turn.
    /// Level validates terrain and occupancy; combat-turns will execute Attack.
    /// Snapshotting IDs prevents enemy movements from changing turn order.
    fn advance_enemy_turns(&mut self) {
        let Some(player_pos) = self
            .player
            .as_ref()
            .filter(|player| !player.get_stats().is_dead())
            .map(Player::get_position)
        else {
            return;
        };
        let Some(level) = self.level.as_mut() else {
            return;
        };

        let enemy_ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in enemy_ids {
            let Some(enemy) = level.enemy(id).filter(|enemy| !enemy.stats().is_dead()) else {
                continue;
            };
            let origin = *enemy.position();
            let profile = AiProfile::for_enemy(enemy);
            let can_see_player = origin.distance(player_pos) <= profile.detection_range
                && level.get_map().has_line_of_sight(origin, player_pos);
            let (state, intent) =
                ai::decide(enemy, player_pos, profile, can_see_player, |target| {
                    target != player_pos
                        && level.is_walkable(target)
                        && level.enemy_at(target).is_none()
                });
            level.set_enemy_ai_state(id, state);
            match intent {
                Intent::Move { target } => {
                    // Game enforces actor collision and one-step movement;
                    // Level revalidates terrain and enemy occupancy.
                    if target != player_pos && origin.is_cardinal_neighbor(target) {
                        level.move_enemy(id, target);
                    }
                }
                Intent::Attack => {
                    // TODO(combat-turns): resolve damage through Game's combat rules.
                    // Enemy damage and effect scheduling are the next combat phase.
                }
                Intent::Wait => {}
            }
        }
    }

    /// Starts a fresh session: new floor, new player at its start, empty log (contract 1).
    fn initialize_game(&mut self) {
        let mut level = LevelGenerator::generate_level(0, &mut self.rng);
        level.update_visibility(level.player_start());
        let stats = Stats::new(STARTING_MAX_HP, STARTING_ATTACK, STARTING_DEFENSE);
        let mut equipment = Equipment::new();
        equipment
            .equip(item::get_item(0))
            .expect("starting pickaxe is a weapon");
        self.player = Some(Player::new(
            level.player_start(),
            stats,
            Inventory::new(),
            equipment,
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
        let defender = enemy.kind().to_string();
        let damage = attack_damage(player.roll_attack(&mut self.rng), enemy.stats().defense());
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
            .add_message(format!("You defeat {}.", enemy.kind()));
        if let Some(item) = equipment_drop(level.depth(), item::catalog(), &mut self.rng) {
            let message = format!("{} drops {}.", enemy.kind(), item.name);
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

    #[test]
    fn dead_player_cannot_move_wait_or_open_doors_but_can_leave_the_session() {
        use level::map::TileType;
        let (mut game, _, _) = combat_game(0, 24, None);
        let level = game.level().unwrap();
        let (door, start, bump) = level
            .get_map()
            .positions()
            .filter(|&p| level.get_map().get(p).unwrap().kind == TileType::Door { open: false })
            .find_map(|door| {
                [
                    (1, 0, Action::MoveRight),
                    (-1, 0, Action::MoveLeft),
                    (0, 1, Action::MoveDown),
                    (0, -1, Action::MoveUp),
                ]
                .into_iter()
                .find_map(|(dx, dy, action)| {
                    let start = door.offset(-dx, -dy)?;
                    level.is_walkable(start).then_some((door, start, action))
                })
            })
            .expect("seeded floor contains a closed door with a walkable entrance");
        let enemies: Vec<_> = level
            .enemies()
            .map(|(id, enemy)| (id, *enemy.position()))
            .collect();
        let player = game.player.as_mut().unwrap();
        player.set_position(start);
        player.update_health(-200, None);
        for action in [
            bump,
            Action::Wait,
            Action::MoveLeft,
            Action::MoveRight,
            Action::MoveUp,
            Action::MoveDown,
        ] {
            assert_eq!(game.process_action(&action), TurnResult::NotSpent);
            assert_eq!(game.player().unwrap().get_position(), start);
        }
        let level = game.level().unwrap();
        assert_eq!(
            level.get_map().get(door).unwrap().kind,
            TileType::Door { open: false }
        );
        assert_eq!(
            level
                .enemies()
                .map(|(id, enemy)| (id, *enemy.position()))
                .collect::<Vec<_>>(),
            enemies
        );
        assert!(game.message_log().history().is_empty());
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::MainMenu));
    }

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
        level.damage_enemy(id, 24 - target_hp, None);
        let mut equipment = Equipment::new();
        equipment.equip(item::get_item(0)).unwrap();
        if let Some(effect) = effect {
            equipment
                .equip(item::get_item(0).with_combat(item::CombatModifiers::weapon(
                    combat::DamageDice::new(2, combat::Die::D4),
                    Some(effect),
                )))
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
        let (mut game, id, action) = combat_game(0, 24, None);
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
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        let damage = attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 2);
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), start);
        assert_eq!(game.player().unwrap().get_stats().hp(), 200);
        let level = game.level().unwrap();
        assert_eq!(level.enemy(id).unwrap().stats().hp(), 24 - damage as i32);
        assert_eq!(level.enemy(other).unwrap().stats().hp(), 8);
        assert_eq!(*level.enemy(other).unwrap().position(), other_position);
        assert_eq!(
            game.message_log().history(),
            &[format!("You hit Goblin for {damage} damage.")]
        );
    }

    #[test]
    fn weapon_dice_and_fresh_effects_apply_without_automatic_ticks() {
        let (mut game, id, action) =
            combat_game(0, 24, Some(stats::EffectSpec::new("Poison".into(), 7, 3)));
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        let mut expected_hp = 24;
        for _ in 0..2 {
            let damage = attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 2);
            expected_hp -= damage as i32;
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
        assert!(log[0].contains("hit Goblin"));
        assert!(
            log.iter()
                .all(|line| !line.contains('#') && !line.contains("EnemyId"))
        );
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
        let (mut game, id, action) = combat_game(0, 24, None);
        let target = *game.level().unwrap().enemy(id).unwrap().position();
        for origin in [
            target.offset(1, 1).unwrap(),
            target.offset(2, 0).unwrap(),
            target,
        ] {
            game.player.as_mut().unwrap().set_position(origin);
            game.attack_enemy(id);
            assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        }
        let start = game.level().unwrap().player_start();
        game.player.as_mut().unwrap().set_position(start);
        game.process_action(&Action::Pause);
        assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        game.process_action(&Action::Pause);
        game.player.as_mut().unwrap().update_health(-200, None);
        assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
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
            let (mut game, id, action) = combat_game(150, 24, None);
            let position = *game.level().unwrap().enemy(id).unwrap().position();
            // Existing items can share the death tile with the new equipment.
            game.level
                .as_mut()
                .unwrap()
                .insert_ground_item(item::get_item(2), position)
                .unwrap();
            game.rng = StdRng::seed_from_u64(seed);
            let mut expected_rng = StdRng::seed_from_u64(seed);
            game.player().unwrap().roll_attack(&mut expected_rng);
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
        let (mut game, id, _) = combat_game(0, 1, None);
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
        assert_eq!(player.get_stats().attack(), 0);
        assert_eq!(
            player.damage_dice(),
            combat::DamageDice::new(2, combat::Die::D4)
        );
        assert_eq!(
            player.get_equipped_items().weapon.as_ref().unwrap().name,
            "Pickaxe"
        );
        assert!(player.get_equipped_items().armor.is_none());
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
                game.rng = StdRng::seed_from_u64(7);
                let mut expected_rng = StdRng::seed_from_u64(7);
                let damage =
                    attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 0);
                assert_eq!(game.process_action(&action), TurnResult::Spent);
                assert_eq!(game.player().unwrap().get_position(), start);
                let level = game.level().unwrap();
                if damage >= 8 {
                    assert!(level.enemy(id).is_none());
                } else {
                    assert_eq!(level.enemy_at(expected), Some(id));
                    assert_eq!(level.enemy(id).unwrap().stats().hp(), 8 - damage as i32);
                }
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
    fn waiting_advances_enemy_ai_but_pausing_does_not() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);

        // Remove generated enemies to test one deterministic AI actor.
        let ids: Vec<_> = game.level().unwrap().enemies().map(|(id, _)| id).collect();
        for id in ids {
            game.level.as_mut().unwrap().remove_enemy(id);
        }

        // Find three walkable tiles in one row; the enemy is two steps away.
        let (player_pos, expected_step, enemy_pos) = {
            let level = game.level().unwrap();
            level
                .get_map()
                .positions()
                .find_map(|player_pos| {
                    let step = player_pos.offset(1, 0)?;
                    let enemy_pos = player_pos.offset(2, 0)?;
                    (level.is_walkable(player_pos)
                        && level.is_walkable(step)
                        && level.is_walkable(enemy_pos))
                    .then_some((player_pos, step, enemy_pos))
                })
                .expect("generated level should contain three consecutive floor tiles")
        };
        game.player.as_mut().unwrap().set_position(player_pos);
        game.level.as_mut().unwrap().update_visibility(player_pos);
        let id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, enemy_pos))
            .ok()
            .unwrap();

        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::NotSpent);
        assert_eq!(
            *game.level().unwrap().enemy(id).unwrap().position(),
            enemy_pos
        );
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);

        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let moved = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*moved.position(), expected_step);
        assert!(matches!(moved.ai(), enemy::AiState::Attacking));
        assert_ne!(*moved.position(), player_pos);

        // Adjacent enemies request Attack but do not deal damage until the
        // separate combat-turns implementation is integrated.
        let hp = game.player().unwrap().get_stats().hp();
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        assert_eq!(
            *game.level().unwrap().enemy(id).unwrap().position(),
            expected_step
        );
        assert_eq!(game.player().unwrap().get_stats().hp(), hp);
    }

    #[test]
    fn closed_door_hides_player_from_ai_and_opening_it_allows_immediate_detection() {
        use level::map::TileType;
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        let (door, player_position, enemy_position, action) = level
            .get_map()
            .positions()
            .filter(|&p| level.get_map().get(p).unwrap().kind == TileType::Door { open: false })
            .find_map(|door| {
                [
                    (-1, 0, 1, 0, Action::MoveRight),
                    (0, -1, 0, 1, Action::MoveDown),
                ]
                .into_iter()
                .find_map(|(px, py, ex, ey, action)| {
                    let player = door.offset(px, py)?;
                    let enemy = door.offset(ex, ey)?;
                    (level.is_walkable(player) && level.is_walkable(enemy))
                        .then_some((door, player, enemy, action))
                })
            })
            .expect("seeded floor contains a closed corridor door");
        let id = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, enemy_position))
            .ok()
            .unwrap();
        level.set_enemy_ai_state(id, enemy::AiState::Attacking);
        game.player.as_mut().unwrap().set_position(player_position);
        level.update_visibility(player_position);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let enemy = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*enemy.position(), enemy_position);
        assert!(matches!(enemy.ai(), enemy::AiState::Idle));
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), player_position);
        let enemy = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*enemy.position(), door);
        assert!(matches!(enemy.ai(), enemy::AiState::Attacking));
    }

    #[test]
    fn game_ai_avoids_an_occupied_preferred_step() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);

        let ids: Vec<_> = game.level().unwrap().enemies().map(|(id, _)| id).collect();
        for id in ids {
            game.level.as_mut().unwrap().remove_enemy(id);
        }

        // The nearest horizontal step is occupied, but moving down is legal.
        let (from, blocked, alternative, player_pos) = {
            let level = game.level().unwrap();
            level
                .get_map()
                .positions()
                .find_map(|from| {
                    let blocked = from.offset(1, 0)?;
                    let alternative = from.offset(0, 1)?;
                    let player_pos = from.offset(2, 1)?;
                    (level.is_walkable(from)
                        && level.is_walkable(blocked)
                        && level.is_walkable(alternative)
                        && level.is_walkable(player_pos))
                    .then_some((from, blocked, alternative, player_pos))
                })
                .expect("generated level should contain the necessary walkable tiles")
        };

        game.player.as_mut().unwrap().set_position(player_pos);
        game.level.as_mut().unwrap().update_visibility(player_pos);
        let active_id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, from))
            .ok()
            .unwrap();
        let blocker_id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Slime, blocked))
            .ok()
            .unwrap();

        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let level = game.level().unwrap();
        assert_eq!(*level.enemy(active_id).unwrap().position(), alternative);
        // The diagonal neighbor now approaches cardinally instead of requesting
        // PR12's diagonal attack. It still cannot enter the player's tile.
        let blocker_position = *level.enemy(blocker_id).unwrap().position();
        assert!(blocked.is_cardinal_neighbor(blocker_position));
        assert_ne!(blocker_position, alternative);
        assert!(level.enemy_at(player_pos).is_none());
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
