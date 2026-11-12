//! Game screens.
//!
//! Game knows the active Screen (contract 3); Renderer reads it to decide what to draw.

/// The active screen. Controls which actions are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    Playing,
    Inventory,
    Paused,
    GameOver,
    /// Controls overlay; closing it restores the previous screen.
    Help,
}
