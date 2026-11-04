//! Subsystem: Terminal Interface.
//!
//! Reads game state and draws it with ASCII via ratatui; turns key presses into Actions.
//! Never mutates game state. Provides SDD subsystem contracts:
//! 3. Input Interpretation
//! 4. Game State Rendering

pub mod input;
pub mod screens;
pub mod theme;

use ratatui::Frame;

use crate::game::level::map::Map;
use theme::Theme;

/// Draws the game to the terminal (SDD 3.9).
///
/// Contracts:
/// - 20 RenderGameState: render(&Game) dispatches to the active screen
/// - 21 InterpretInput: read_input() -> Option<Action> (see `input`)
pub struct Renderer {
    theme: Theme,
    // TODO: keybinds: Keybinds
}

impl Renderer {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    /// Draws the current level.
    /// TODO: take &Game and dispatch on game.screen once Game is implemented.
    pub fn render(&self, frame: &mut Frame, map: &Map, depth: usize) {
        screens::playing::draw_level(frame, map, depth, &self.theme);
    }
}
