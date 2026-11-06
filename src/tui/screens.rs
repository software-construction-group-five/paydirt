//! One drawing module per Screen variant.
//!
//! Renderer::render matches on game.screen and calls the corresponding module's draw fn.

pub mod game_over;
pub mod inventory;
pub mod main_menu;
pub mod paused;
pub mod playing;
