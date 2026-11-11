//! Presentation styles shared by all terminal screens.

use std::{fs, io, path::Path};

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

use crate::game::level::map::TileType;

/// One ASCII glyph and its terminal style.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    /// Character occupying one map cell.
    pub symbol: char,
    /// Foreground, background, and emphasis for the cell.
    pub style: Style,
}

impl Glyph {
    /// Creates a display glyph using the terminal default colors.
    pub fn new(symbol: char) -> Self {
        Self {
            symbol,
            style: Style::default(),
        }
    }
}

/// Shared text styles and terrain glyphs; callers may customize every field.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Panel borders and secondary text.
    pub muted: Style,
    /// Titles and selected entries.
    pub accent: Style,
    /// Primary text.
    pub text: Style,
    /// Critical results and warnings.
    pub danger: Style,
    /// Wall appearance.
    pub wall: Glyph,
    /// Floor appearance.
    pub floor: Glyph,
    /// Impassable rock appearance.
    pub rock: Glyph,
    /// Impassable water appearance.
    pub water: Glyph,
    /// Open door appearance.
    pub door_open: Glyph,
    /// Closed door appearance.
    pub door_closed: Glyph,
    /// Downstairs appearance.
    pub stairs_down: Glyph,
    /// Default player appearance.
    pub player: Glyph,
}

impl Default for Theme {
    fn default() -> Self {
        Self::parse(include_str!("../../config/theme.toml"))
            .expect("the bundled theme must be valid")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeConfig {
    ui: UiConfig,
    tiles: TileConfig,
    player: GlyphConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiConfig {
    text: String,
    muted: String,
    accent: String,
    danger: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TileConfig {
    wall: GlyphConfig,
    floor: GlyphConfig,
    rock: GlyphConfig,
    water: GlyphConfig,
    door_open: GlyphConfig,
    door_closed: GlyphConfig,
    stairs_down: GlyphConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlyphConfig {
    symbol: char,
    color: String,
}

fn foreground(value: &str, field: &str) -> io::Result<Style> {
    let color = value.parse::<Color>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid color for {field}: {value:?}"),
        )
    })?;
    Ok(Style::default().fg(color))
}

impl GlyphConfig {
    fn resolve(self, field: &str) -> io::Result<Glyph> {
        if !self.symbol.is_ascii_graphic() && self.symbol != ' ' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{field}.symbol must be one printable ASCII character"),
            ));
        }
        Ok(Glyph {
            symbol: self.symbol,
            style: foreground(&self.color, &format!("{field}.color"))?,
        })
    }
}

impl Theme {
    /// Loads characters and foreground colors, reporting the path on failure.
    /// Background settings and unknown fields are rejected.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let result = fs::read_to_string(path).and_then(|source| Self::parse(&source));
        result.map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
    }

    /// Parses a complete theme; every style retains the terminal default background.
    pub fn parse(source: &str) -> io::Result<Self> {
        let config: ThemeConfig = toml::from_str(source)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(Self {
            text: foreground(&config.ui.text, "ui.text")?,
            muted: foreground(&config.ui.muted, "ui.muted")?,
            accent: foreground(&config.ui.accent, "ui.accent")?.add_modifier(Modifier::BOLD),
            danger: foreground(&config.ui.danger, "ui.danger")?.add_modifier(Modifier::BOLD),
            wall: config.tiles.wall.resolve("tiles.wall")?,
            floor: config.tiles.floor.resolve("tiles.floor")?,
            rock: config.tiles.rock.resolve("tiles.rock")?,
            water: config.tiles.water.resolve("tiles.water")?,
            door_open: config.tiles.door_open.resolve("tiles.door_open")?,
            door_closed: config.tiles.door_closed.resolve("tiles.door_closed")?,
            stairs_down: config.tiles.stairs_down.resolve("tiles.stairs_down")?,
            player: config.player.resolve("player")?,
        })
    }

    /// Resolves terrain into a glyph without changing the map.
    pub fn tile(&self, kind: TileType) -> Glyph {
        match kind {
            TileType::Wall => self.wall,
            TileType::Floor => self.floor,
            TileType::Rock => self.rock,
            TileType::Water => self.water,
            TileType::Door { open: true } => self.door_open,
            TileType::Door { open: false } => self.door_closed,
            TileType::StairsDown => self.stairs_down,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use a fixed monochrome fixture so tests do not depend on the active palette.
    const CONFIG: &str = r##"
[ui]
text = "reset"
muted = "reset"
accent = "reset"
danger = "reset"
[tiles.wall]
symbol = "#"
color = "reset"
[tiles.floor]
symbol = "."
color = "reset"
[tiles.rock]
symbol = "O"
color = "reset"
[tiles.water]
symbol = "~"
color = "reset"
[tiles.door_open]
symbol = "/"
color = "reset"
[tiles.door_closed]
symbol = "+"
color = "reset"
[tiles.stairs_down]
symbol = ">"
color = "reset"
[player]
symbol = "@"
color = "reset"
"##;

    #[test]
    fn configured_characters_and_foreground_colors_reach_the_renderer() {
        use crate::game::{level::map::Map, position::Position};
        use crate::{
            game::screen::Screen,
            tui::{LevelView, RenderView, Renderer},
        };
        use ratatui::{Terminal, backend::TestBackend};

        let source = CONFIG
            .replace("symbol = \"@\"", "symbol = \"P\"")
            .replace("color = \"reset\"", "color = \"#123456\"")
            .replace("accent = \"reset\"", "accent = \"yellow\"");
        let theme = Theme::parse(&source).unwrap();
        assert_eq!(theme.accent.fg, Some(Color::Yellow));
        assert_eq!(theme.player.style.fg, Some(Color::Rgb(0x12, 0x34, 0x56)));
        let map = Map::new(3, 3);
        let mut level = LevelView::new(&map);
        level.player = Some(Position::new(1, 1));
        let view = RenderView {
            level: Some(level),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let renderer = Renderer::new(theme);
        let buffer = &terminal
            .draw(|frame| renderer.render(frame, &Screen::Playing, &view))
            .unwrap()
            .buffer;
        assert_eq!(buffer[(2, 2)].symbol(), "P");
        assert_eq!(buffer[(2, 2)].fg, Color::Rgb(0x12, 0x34, 0x56));
        assert!(buffer.content.iter().all(|cell| cell.bg == Color::Reset));
    }

    #[test]
    fn rejects_invalid_colors_glyphs_and_background_configuration() {
        for source in [
            CONFIG.replace("text = \"reset\"", "text = \"not-a-color\""),
            CONFIG.replace("symbol = \"@\"", "symbol = \"PP\""),
            CONFIG.replace("symbol = \"@\"", "symbol = \"界\""),
            CONFIG.replace("[ui]", "[ui]\nbackground = \"black\""),
            format!("background = \"black\"\n{CONFIG}"),
            format!("{CONFIG}\nbackground = \"black\""),
        ] {
            assert_eq!(
                Theme::parse(&source).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn default_theme_only_sets_foregrounds() {
        let theme = Theme::default();
        for style in [
            theme.text,
            theme.muted,
            theme.accent,
            theme.danger,
            theme.wall.style,
            theme.floor.style,
            theme.rock.style,
            theme.water.style,
            theme.door_open.style,
            theme.door_closed.style,
            theme.stairs_down.style,
            theme.player.style,
        ] {
            assert_eq!(style.bg, None);
        }
    }

    #[test]
    fn load_errors_include_the_configuration_path() {
        let path = "missing-theme-for-paydirt-test.toml";
        let error = Theme::load(path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains(path));
    }

    #[test]
    fn rocks_and_water_render_with_configured_glyphs_and_foregrounds() {
        use crate::{
            game::{level::map::Map, position::Position, screen::Screen},
            tui::{LevelView, RenderView, Renderer},
        };
        use ratatui::{Terminal, backend::TestBackend};
        let theme = Theme::parse(include_str!("../../config/theme.toml")).unwrap();
        assert_eq!(theme.tile(TileType::Rock).symbol, 'O');
        assert_eq!(theme.tile(TileType::Rock).style.fg, Some(Color::DarkGray));
        assert_eq!(theme.tile(TileType::Water).symbol, '~');
        assert_eq!(theme.tile(TileType::Water).style.fg, Some(Color::Blue));
        let mut map = Map::new(5, 5);
        map.set(Position::new(1, 1), TileType::Rock);
        map.set(Position::new(2, 1), TileType::Water);
        let view = RenderView {
            level: Some(LevelView::new(&map)),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let renderer = Renderer::new(theme);
        let buffer = &terminal
            .draw(|frame| renderer.render(frame, &Screen::Playing, &view))
            .unwrap()
            .buffer;
        assert_eq!(buffer[(2, 2)].symbol(), "O");
        assert_eq!(buffer[(2, 2)].fg, Color::DarkGray);
        assert_eq!(buffer[(3, 2)].symbol(), "~");
        assert_eq!(buffer[(3, 2)].fg, Color::Blue);
        assert!(buffer.content.iter().all(|cell| cell.bg == Color::Reset));
    }
}
