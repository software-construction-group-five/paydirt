//! Visual theme: which character/color represents each tile, enemy, item, and the player.
//!
//! Loaded from config/theme.toml. Purely presentational.

use std::error::Error;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::game::level::map::TileType;

/// Glyph and color mappings.
#[derive(Deserialize)]
pub struct Theme {
    tiles: TileGlyphs,
    // TODO: enemies, items, player
}

/// One character per tile kind, read from the `[tiles]` table.
#[derive(Deserialize)]
struct TileGlyphs {
    wall: char,
    floor: char,
    door_open: char,
    door_closed: char,
    stairs_down: char,
}

impl Theme {
    /// Reads a theme from a TOML file such as `config/theme.toml`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let text = fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    /// The character drawn for a tile.
    pub fn tile(&self, kind: TileType) -> char {
        match kind {
            TileType::Wall => self.tiles.wall,
            TileType::Floor => self.tiles.floor,
            TileType::Door { open: true } => self.tiles.door_open,
            TileType::Door { open: false } => self.tiles.door_closed,
            TileType::StairsDown => self.tiles.stairs_down,
        }
    }
}
