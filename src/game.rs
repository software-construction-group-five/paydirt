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

/// Owns screen navigation and generated levels while gameplay systems are completed.
///
/// TODO(team): integrate Player, inventory contents, combat, turns, and MessageLog.
/// The TUI consumes this state without owning transitions or level construction.
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

    /// Processes menu, inventory, and pause navigation; unfinished gameplay actions
    /// leave state unchanged rather than simulating movement or combat.
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
                    self.level = Some(LevelGenerator::generate_level(0, &mut self.rng));
                    self.screen = Screen::Playing;
                }
                Action::Confirm => self.should_quit = true,
                _ => {}
            },
            Screen::Playing => match action {
                Action::OpenInventory => self.screen = Screen::Inventory,
                Action::Pause => self.screen = Screen::Paused,
                _ => {}
            },
            Screen::Inventory => {
                if matches!(action, Action::Back | Action::OpenInventory) {
                    self.screen = Screen::Playing;
                }
            }
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

    /// Regenerates the current preview floor only while the Playing screen is active.
    pub fn regenerate_preview(&mut self) {
        if matches!(self.screen, Screen::Playing)
            && !self.should_quit
            && let Some(level) = &self.level
        {
            self.level = Some(LevelGenerator::generate_level(level.depth(), &mut self.rng));
        }
    }

    /// Advances the preview depth without implementing gameplay progression rules.
    pub fn advance_preview(&mut self) {
        if matches!(self.screen, Screen::Playing)
            && !self.should_quit
            && let Some(level) = &self.level
        {
            self.level = Some(LevelGenerator::generate_level(
                level.depth().saturating_add(1),
                &mut self.rng,
            ));
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
    fn menu_inventory_and_pause_navigation_preserves_the_active_level() {
        let mut game = Game::new();
        assert!(matches!(game.screen(), Screen::MainMenu));
        assert!(game.level().is_none());
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        game.advance_preview();
        let start = game.level().unwrap().player_start();
        game.process_action(&Action::OpenInventory);
        assert!(matches!(game.screen(), Screen::Inventory));
        game.advance_preview();
        assert_eq!(game.level().unwrap().depth(), 1);
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::Playing));
        game.process_action(&Action::Pause);
        assert!(matches!(game.screen(), Screen::Paused));
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        assert_eq!(game.level().unwrap().player_start(), start);
        assert_eq!(game.level().unwrap().depth(), 1);
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
        game.regenerate_preview();
        assert!(game.level().is_none());
        game.process_action(&Action::MoveDown);
        assert_eq!(game.menu_selection(), 1);
        game.process_action(&Action::Confirm);
        assert!(game.should_quit());
        assert!(game.level().is_none());
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::OpenInventory);
        game.process_action(&Action::Quit);
        assert!(game.should_quit());
    }
}
