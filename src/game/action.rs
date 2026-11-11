//! Player actions recognized by the game.
//!
//! Produced by the Terminal Interface (tui::input) from key presses,
//! consumed by Game::process_action (contract 2). Game owns what each action means.

/// A recognized player action, independent of which key triggered it.
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Wait,
    Pause,
    Confirm,
    Back,
    Quit,
}

/// Whether an action used up a gameplay turn (Game Contract 2).
///
/// Moves and waits always spend a turn, even when the move is blocked.
/// Menus, pausing, and redraws never do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnResult {
    Spent,
    NotSpent,
}