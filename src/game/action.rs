//! Player actions recognized by the game.
//!
//! Produced by the Terminal Interface (tui::input) from key presses,
//! consumed by Game::process_action (contract 2). Game owns what each action means.

/// A recognized player action, independent of which key triggered it.
pub enum Action {
    MoveUp,
    MoveDown,
    Pause,
    Confirm,
    Back,
    Quit,
}
