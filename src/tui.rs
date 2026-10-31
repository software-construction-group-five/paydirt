//! Subsystem: Terminal Interface.
//!
//! Reads game state and draws it with ASCII via ratatui; turns key presses into Actions.
//! Never mutates game state. Provides SDD subsystem contracts:
//! 3. Input Interpretation
//! 4. Game State Rendering

pub mod input;
pub mod screens;
pub mod theme;

/// Draws the game to the terminal (SDD 3.9).
///
/// Contracts:
/// - 20 RenderGameState: render(&Game) dispatches to the active screen
/// - 21 InterpretInput: read_input() -> Option<Action> (see `input`)
pub struct Renderer {
    // TODO: terminal handle, theme: Theme, keybinds: Keybinds
}
