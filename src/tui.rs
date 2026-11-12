//! Terminal presentation and input modules.
//!
//! Rendering consumes borrowed display data and never changes game state. The
//! display data bridges the unfinished gameplay types without depending on their
//! eventual storage or accessors. Input and terminal lifecycle belong to callers.
//!
//! Pass the current screen and available state to the renderer inside the caller's
//! terminal draw callback. Entity builders resolve visible level populations through
//! the renderer theme; drawing also enforces tile visibility.
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

use crate::game::{
    level::{Level, map::Map},
    position::Position,
    screen::Screen,
};
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
    /// Camera focus when a player position is not yet supplied.
    pub focus: Option<Position>,
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
            focus: None,
        }
    }
}

/// Optional player information, formatted by the caller from available state.
#[derive(Default)]
pub struct StatusView<'a> {
    /// Elapsed session turns and remaining lamp fuel.
    pub turns_and_fuel: Option<(u64, u32)>,
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
    /// Cumulative session results for the pause overview.
    pub session_stats: crate::game::SessionStats,
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
    /// Number of history lines scrolled back from the newest game-over message.
    pub history_offset: usize,
    /// Screen underneath the help overlay, preserving its display selection.
    pub help_origin: Option<Screen>,
    /// Controls overlay text generated from configured bindings.
    pub help: &'a str,
}

/// Renders game screens and overlays using a shared theme (contract 20).
#[derive(Default)]
pub struct Renderer {
    theme: Theme,
}

impl Renderer {
    /// Creates a renderer with explicit text styles and glyphs.
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    /// Builds display entries for enemies currently in sight, without changing state.
    pub fn enemy_entities(&self, level: &Level) -> Vec<MapEntity> {
        level
            .visible_enemies()
            .map(|(_, enemy)| MapEntity {
                position: *enemy.position(),
                glyph: self.theme.enemy(enemy.kind()),
            })
            .collect()
    }

    /// Builds display entries for ground items currently in sight, in storage order.
    pub fn item_entities(&self, level: &Level) -> Vec<MapEntity> {
        level
            .visible_ground_items()
            .map(|(_, entry)| MapEntity {
                position: entry.position,
                glyph: self.theme.item(&entry.item.kind),
            })
            .collect()
    }

    /// Draws one frame without mutating gameplay or reading terminal events.
    pub fn render(&self, frame: &mut Frame, screen: &Screen, view: &RenderView<'_>) {
        frame.render_widget(
            Block::default().style(ratatui::style::Style::reset()),
            frame.area(),
        );
        let background = if matches!(screen, Screen::Help) {
            view.help_origin.as_ref().unwrap_or(&Screen::Playing)
        } else {
            screen
        };
        match background {
            Screen::MainMenu => screens::main_menu::draw(frame, view, &self.theme),
            Screen::Playing | Screen::Help => screens::playing::draw(frame, view, &self.theme),
            Screen::Inventory => screens::inventory::draw(frame, view, &self.theme),
            Screen::Paused => {
                screens::playing::draw(frame, view, &self.theme);
                screens::paused::draw(frame, view, &self.theme);
            }
            Screen::GameOver => screens::game_over::draw(frame, view, &self.theme),
        }
        if matches!(screen, Screen::Help) {
            screens::paused::draw_overlay(
                frame,
                &self.theme,
                " Help ",
                view.help,
                ratatui::layout::Alignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;

    #[test]
    fn controls_only_appear_in_help_using_the_pause_overlay_style() {
        let view = RenderView {
            help: "CONTROLS\nx: wait",
            help_origin: Some(Screen::Playing),
            ..Default::default()
        };
        for screen in [
            Screen::Playing,
            Screen::Inventory,
            Screen::GameOver,
            Screen::MainMenu,
            Screen::Paused,
        ] {
            assert!(!text(&draw_screen(80, 24, screen, &view)).contains("x: wait"));
        }
        let help = draw_screen(80, 24, Screen::Help, &view);
        let pause = draw_screen(80, 24, Screen::Paused, &view);
        assert!(text(&help).contains("x: wait"));
        assert!(text(&pause).contains("PAUSED"));
        assert!(text(&pause).contains("Enemies slain: 0"));
        assert_eq!(help[(20, 9)].fg, pause[(20, 8)].fg);
        let map = crate::game::level::map::Map::new(1, 1);
        let mut level = LevelView::new(&map);
        level.depth = Some(7);
        let overview = RenderView {
            level: Some(level),
            status: StatusView {
                turns_and_fuel: Some((56, 144)),
                ..Default::default()
            },
            session_stats: crate::game::SessionStats {
                enemies_slain: 12,
                items_acquired: 34,
            },
            ..Default::default()
        };
        let pause = draw_screen(80, 24, Screen::Paused, &overview);
        for (y, label) in [
            (9, "PAUSED"),
            (11, "Depth: 7"),
            (12, "Turns: 56"),
            (13, "Enemies slain: 12"),
            (14, "Items acquired: 34"),
        ] {
            let row: String = (21..59).map(|x| pause[(x, y)].symbol()).collect();
            assert!(row.starts_with(label));
        }
    }

    #[test]
    fn generated_populations_reach_the_renderer_with_custom_glyphs_and_visibility() {
        use crate::game::level::generator::LevelGenerator;
        use rand::{SeedableRng, rngs::StdRng};
        use ratatui::style::Color;

        let mut level = LevelGenerator::generate_level(8, &mut StdRng::seed_from_u64(42));
        let mut theme = Theme::default();
        for glyph in [&mut theme.goblin, &mut theme.ghost, &mut theme.slime] {
            glyph.symbol = 'E';
            glyph.style = ratatui::style::Style::default().fg(Color::Red);
        }
        for glyph in [&mut theme.weapon, &mut theme.armor, &mut theme.consumable] {
            glyph.symbol = 'I';
            glyph.style = ratatui::style::Style::default().fg(Color::Cyan);
        }
        let renderer = Renderer::new(theme);
        assert!(renderer.enemy_entities(&level).is_empty());
        assert!(renderer.item_entities(&level).is_empty());
        let positions: Vec<_> = level
            .enemies()
            .map(|(_, enemy)| *enemy.position())
            .chain(level.ground_items().iter().map(|entry| entry.position))
            .collect();
        assert!(!positions.is_empty());
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        for position in positions {
            level.update_visibility(position);
            let enemies = renderer.enemy_entities(&level);
            let items = renderer.item_entities(&level);
            assert_eq!(enemies.len(), level.visible_enemies().count());
            assert_eq!(items.len(), level.visible_ground_items().count());
            let mut view = LevelView::new(level.get_map());
            view.enemies = &enemies;
            view.items = &items;
            let view = RenderView {
                level: Some(view),
                ..Default::default()
            };
            let buffer = &terminal
                .draw(|frame| renderer.render(frame, &Screen::Playing, &view))
                .unwrap()
                .buffer;
            for entity in enemies.iter().chain(&items) {
                let cell = &buffer[(
                    (entity.position.x + 1) as u16,
                    (entity.position.y + 1) as u16,
                )];
                assert_eq!(cell.symbol(), entity.glyph.symbol.to_string());
                assert_eq!(cell.fg, entity.glyph.style.fg.unwrap());
            }
        }
    }

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
                Screen::Help,
            ] {
                draw_screen(width, height, screen, &view);
            }
        }
    }
}
