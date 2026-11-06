//! Paydirt library crate root.
//!
//! Exposes the two SDD subsystems so they can be unit/integration tested without a terminal:
//! - `game`: Game Logic & Management (never depends on ratatui/crossterm)
//! - `tui`:  Terminal Interface (depends on `game`, never the reverse)

pub mod game;
pub mod tui;
