//! Player actions recognized by the game.
//!
//! Produced by the Terminal Interface (tui::input) from key presses,
//! consumed by Game::process_action (contract 2). Game owns what each action means.

/// A recognized player action, independent of which key triggered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Wait,
    /// Descend while standing on stairs, ticking only player effects before entry.
    Descend,
    /// Toggle the controls overlay without spending a turn.
    Help,
    /// Open the inventory without spending a turn.
    OpenInventory,
    /// Pick up the first item at the player's tile.
    Pickup,
    /// Use the selected carried consumable.
    UseItem,
    /// Equip the selected carried weapon or armor.
    EquipItem,
    /// Unequip the selected equipment slot.
    UnequipItem,
    /// Drop the selected carried or equipped item.
    DropItem,
    /// Scroll one history line toward older messages.
    HistoryUp,
    /// Scroll one history line toward newer messages.
    HistoryDown,
    /// Scroll ten history lines toward older messages.
    HistoryPageUp,
    /// Scroll ten history lines toward newer messages.
    HistoryPageDown,
    Pause,
    Confirm,
    Back,
    Quit,
}

/// Whether an action used up a gameplay turn (Game Contract 2).
///
/// Moves and waits always spend a turn, even when the move is blocked.
/// Accepted descent spends a turn with only player effects before floor replacement.
/// Menus, pausing, and redraws never do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnResult {
    Spent,
    NotSpent,
}
