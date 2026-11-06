//! Game screens and their per-screen state.
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

/// State for the main menu (e.g. selected option).
pub struct MainMenuState {
    // TODO: selected: usize
}

/// State for the inventory screen (e.g. cursor position).
pub struct InventoryState {
    // TODO: selected: usize
}

/// Final results shown on the GameOver screen (contract 5).
pub struct GameSummary {
    // TODO: final_depth: u32, cause_of_death: String, final stats
}
