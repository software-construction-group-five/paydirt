//! Terminal presentation and input modules.
//!
//! Rendering consumes borrowed display data and never changes game state. The
//! display data bridges the unfinished gameplay types without depending on their
//! eventual storage or accessors. Input and terminal lifecycle belong to callers.
//!
//! Pass the current screen and available state to the renderer inside the caller's
//! terminal draw callback. Empty slices leave space for future entity systems.
//!
//! ```
//! use paydirt::{game::{level::map::Map, screen::Screen}, tui::{LevelView, RenderView, Renderer}};
//! use ratatui::{Terminal, backend::TestBackend};
//!
//! let map = Map::new(80, 21);
//! let view = RenderView { level: Some(LevelView::new(&map)), ..Default::default() };
//! let renderer = Renderer::default();
//! let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
//! terminal.draw(|frame| renderer.render(frame, &Screen::Playing, &view))?;
//! # Ok::<(), std::convert::Infallible>(())
//! ```

pub mod input;
pub mod screens;
pub mod theme;

use crate::game::{level::map::Map, position::Position, screen::Screen};
use ratatui::{Frame, widgets::Block};
use theme::{Glyph, Theme};

/// An entity's already resolved visual appearance and map position.
#[derive(Debug, Clone, Copy)]
pub struct MapEntity {
    /// Location in map coordinates.
    pub position: Position,
    /// Species/item-specific appearance chosen by the caller.
    pub glyph: Glyph,
}

/// Read-only map and optional entities, rendered in terrain/item/enemy/player order.
pub struct LevelView<'a> {
    /// Terrain to display; fog rendering must use each tile's seen/visible flags.
    pub map: &'a Map,
    /// Current dungeon depth, if known.
    pub depth: Option<u32>,
    /// Items on the floor, in drawing order.
    pub items: &'a [MapEntity],
    /// Enemies on the floor, in drawing order.
    pub enemies: &'a [MapEntity],
    /// Player location; also controls the viewport center.
    pub player: Option<Position>,
}

impl<'a> LevelView<'a> {
    /// Creates a terrain-only view ready for optional entities and depth.
    pub fn new(map: &'a Map) -> Self {
        Self {
            map,
            depth: None,
            items: &[],
            enemies: &[],
            player: None,
        }
    }
}

/// Optional player information, formatted by the caller from available state.
#[derive(Default)]
pub struct StatusView<'a> {
    /// Current and maximum HP, if available.
    pub health: Option<(i32, i32)>,
    /// Descriptions of active effects.
    pub effects: &'a [&'a str],
    /// Names of equipped items.
    pub equipment: &'a [&'a str],
}

/// One inventory row, independent of the game's eventual inventory storage.
pub struct InventoryEntry<'a> {
    /// Item name.
    pub name: &'a str,
    /// Display category, such as Weapon or Consumable.
    pub category: &'a str,
    /// Optional configured selection key.
    pub key: Option<char>,
    /// Whether this item is equipped.
    pub equipped: bool,
}

/// Presentation data for screen dispatch; absent gameplay data stays absent.
#[derive(Default)]
pub struct RenderView<'a> {
    /// Current map and entities, if a level is available.
    pub level: Option<LevelView<'a>>,
    /// Player status, if available.
    pub status: StatusView<'a>,
    /// Session messages in chronological order.
    pub messages: &'a [&'a str],
    /// Inventory entries in display order.
    pub inventory: &'a [InventoryEntry<'a>],
    /// Selected menu or inventory row; out-of-range values are clamped.
    pub selected: usize,
    /// Optional game-over explanation.
    pub outcome: Option<&'a str>,
    /// Optional final dungeon depth.
    pub final_depth: Option<u32>,
    /// Caller-supplied help reflecting actual configured controls.
    pub help: &'a str,
}

/// Renders the five game screens using a shared, customizable theme (contract 20).
#[derive(Default)]
pub struct Renderer {
    theme: Theme,
}

impl Renderer {
    /// Creates a renderer with explicit text styles and glyphs.
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    /// Draws one frame without mutating gameplay or reading terminal events.
    pub fn render(&self, frame: &mut Frame, screen: &Screen, view: &RenderView<'_>) {
        frame.render_widget(
            Block::default().style(ratatui::style::Style::reset()),
            frame.area(),
        );
        match screen {
            Screen::MainMenu => screens::main_menu::draw(frame, view, &self.theme),
            Screen::Playing => screens::playing::draw(frame, view, &self.theme),
            Screen::Inventory => screens::inventory::draw(frame, view, &self.theme),
            Screen::Paused => {
                screens::playing::draw(frame, view, &self.theme);
                screens::paused::draw(frame, &self.theme);
            }
            Screen::GameOver => screens::game_over::draw(frame, view, &self.theme),
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;

    pub(super) fn draw_screen(
        width: u16,
        height: u16,
        screen: Screen,
        view: &RenderView<'_>,
    ) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| Renderer::default().render(frame, &screen, view))
            .unwrap()
            .buffer
            .clone()
    }

    pub(super) fn text(buffer: &Buffer) -> String {
        buffer
            .content
            .chunks(usize::from(buffer.area.width).max(1))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn all_screens_handle_small_and_resized_terminals() {
        let map = Map::new(0, 0);
        let inventory = [InventoryEntry {
            name: "Pickaxe",
            category: "Weapon",
            key: Some('a'),
            equipped: true,
        }];
        for (width, height) in [(0, 0), (1, 1), (2, 2), (20, 8), (80, 24), (120, 35)] {
            let view = RenderView {
                level: Some(LevelView::new(&map)),
                inventory: &inventory,
                selected: usize::MAX,
                ..Default::default()
            };
            for screen in [
                Screen::MainMenu,
                Screen::Playing,
                Screen::Paused,
                Screen::Inventory,
                Screen::GameOver,
            ] {
                draw_screen(width, height, screen, &view);
            }
        }
    }
}
