//! Subsystem: Game Logic & Management.
//!
//! Owns all game state and gameplay rules. Provides SDD subsystem contracts:
//! 1. Game Action Processing
//! 2. Game State Access

pub mod action;
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
use enemy::ai::{self, AiProfile, Intent};
use equipment::Equipment;
use inventory::Inventory;
use level::{Level, generator::LevelGenerator};
use message_log::MessageLog;
use player::Player;
use rand::{SeedableRng, rngs::StdRng};
use screen::Screen;
use stats::Stats;

// TODO(joseph): confirm starting stats with the team
const STARTING_MAX_HP: i32 = 100;
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
        let Some(player_pos) = self.player.as_ref().map(Player::get_position) else {
            return;
        };
        let Some(level) = self.level.as_mut() else {
            return;
        };

        let enemy_ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in enemy_ids {
            // Decision-making borrows the level immutably. Mutations happen
            // afterward, so Level remains the authority on valid moves.
            let Some((state, intent)) = level.enemy(id).map(|enemy| {
                let profile = AiProfile::for_enemy(enemy);
                ai::decide(enemy, player_pos, profile, |target| {
                    target != player_pos
                        && level.is_walkable(target)
                        && level.enemy_at(target).is_none()
                })
            }) else {
                continue;
            };

            level.set_enemy_ai_state(id, state);
            match intent {
                Intent::Move(target) => {
                    // Revalidate because earlier enemies already took turns.
                    if target != player_pos
                        && level.is_walkable(target)
                        && level.enemy_at(target).is_none()
                    {
                        level.move_enemy(id, target);
                    }
                }
                Intent::Attack => {
                    // TODO(combat-turns): resolve damage through Game's combat rules.
                    // Do not damage Player until combat integration is approved.
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
    /// the player stays put. Enemies block movement until combat exists.
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
        if level.enemy_at(target).is_some() {
            // TODO(joseph): attack the enemy once combat-turns is merged (Level Contract 10)
            return TurnResult::Spent;
        }
        player.set_position(target);
        level.update_visibility(target);
        TurnResult::Spent
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
        for _ in 0..50 {
            let mut game = Game::new();
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
                    level.is_walkable(beside).then_some((door, beside, action))
                })
            });
            let Some((door, beside, action)) = found else {
                continue;
            };
            game.player.as_mut().unwrap().set_position(beside);
            assert_eq!(game.process_action(&action), TurnResult::Spent);
            assert_eq!(game.player().unwrap().get_position(), beside);
            assert!(game.level().unwrap().is_walkable(door));
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), door);
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
        assert_eq!(*level.enemy(blocker_id).unwrap().position(), blocked);
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
