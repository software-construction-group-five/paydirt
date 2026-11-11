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
use level::{Level, generator::LevelGenerator};
use rand::{SeedableRng, rngs::StdRng};
use screen::Screen;

/// Owns screen navigation and the active dungeon floor.
pub struct Game {
    screen: Screen,
    menu_selection: usize,
    level: Option<Level>,
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
                Action::Confirm if self.menu_selection == 0 => {
                    let mut level = LevelGenerator::generate_level(0, &mut self.rng);
                    level.update_visibility(level.player_start());
                    self.level = Some(level);
                    self.screen = Screen::Playing;
                }
                Action::Confirm => self.should_quit = true,
                _ => {}
            },
            Screen::Playing => {
                if matches!(action, Action::Pause) {
                    self.screen = Screen::Paused;
                }
            }
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

    fn return_to_menu(&mut self) {
        self.screen = Screen::MainMenu;
        self.menu_selection = 0;
        self.level = None;
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
