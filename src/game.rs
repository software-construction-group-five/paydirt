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

use action::Action;
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

    /// Returns the active player, absent before a session starts (Game State Access).
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

    /// Processes menu and pause navigation.
    pub fn process_action(&mut self, action: &Action) {
        if self.should_quit {
            return;
        }
        if matches!(action, Action::Quit) {
            self.should_quit = true;
            return;
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
                Action::MoveUp => self.move_player(0, -1),
                Action::MoveDown => self.move_player(0, 1),
                Action::MoveLeft => self.move_player(-1, 0),
                Action::MoveRight => self.move_player(1, 0),
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
    }

    /// Starts a fresh session: new floor, new player at its start, empty log (Game Contract 1, InitializeGame).
    fn initialize_game(&mut self) {
        let level = LevelGenerator::generate_level(0, &mut self.rng);
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
    /// and Player Contract 7).
    fn move_player(&mut self, dx: isize, dy: isize) {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_ref()) else {
            return;
        };
        let Some(target) = player.get_position().offset(dx, dy) else {
            return;
        };
        if !level.is_walkable(target) {
            return;
        }
        // TODO(joseph): once level-entities adds enemy lookup, send moves into an
        // enemy to combat instead of moving (Level Contract 10)
        player.set_position(target);
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
                .filter(|&target| game.level().unwrap().is_walkable(target))
                .unwrap_or(start);
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), expected);
        }
    }

    #[test]
    fn walking_into_a_wall_stops_the_player() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        // Rooms are enclosed, so walking left long enough always hits something solid
        for _ in 0..100 {
            game.process_action(&Action::MoveLeft);
        }
        let level = game.level().unwrap();
        let stopped = game.player().unwrap().get_position();
        assert!(level.is_walkable(stopped));
        assert!(
            stopped
                .offset(-1, 0)
                .is_none_or(|left| !level.is_walkable(left))
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
