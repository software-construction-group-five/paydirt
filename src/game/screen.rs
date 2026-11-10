//! Game screens.
//!
//! Game knows the active Screen (contract 3); Renderer reads it to decide what to draw.

/// The active screen. Controls which actions are valid.
pub enum Screen {
    MainMenu,
    Playing,
    Inventory,
    Paused,
    GameOver,
}
