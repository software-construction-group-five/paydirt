//! Dungeon viewport, player status, and recent session messages.

use super::panel;
use crate::{
    game::position::Position,
    tui::{
        LevelView, RenderView, StatusView,
        theme::{Glyph, Theme},
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

/// Draws gameplay above a full-width message log, with status beside wide maps.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let body = frame.area();
    // Reserve the bottom log first; collapse status before losing useful map space.
    let (content, log_area) = if !view.messages.is_empty() && body.height >= 7 {
        let log_height = if body.height >= 16 { 5 } else { 3 };
        let [content, log] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(log_height)]).areas(body);
        (content, log)
    } else {
        (body, Rect::default())
    };
    let has_status = view.status.health.is_some()
        || !view.status.effects.is_empty()
        || !view.status.equipment.is_empty();
    let (map_area, status_area) = if has_status && content.width >= 90 && content.height >= 7 {
        let [map, status] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(28)]).areas(content);
        (map, status)
    } else if has_status && content.height >= 10 {
        let [map, status] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(5)]).areas(content);
        (map, status)
    } else {
        (content, Rect::default())
    };
    if let Some(level) = &view.level {
        draw_level(frame, map_area, level, theme);
    } else {
        frame.render_widget(
            Paragraph::new("No level available.").block(panel(" Mine ", theme)),
            map_area,
        );
    }
    draw_status(frame, status_area, &view.status, theme);
    draw_messages(frame, log_area, view.messages, theme);
}

/// Draws a clipped map centered on the player, with items below enemies and player.
pub fn draw_level(frame: &mut Frame, area: Rect, level: &LevelView<'_>, theme: &Theme) {
    let title = level.depth.map_or_else(
        || " Mine ".to_owned(),
        |depth| format!(" Mine / Depth {depth} "),
    );
    let block = panel(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // Clear the viewport so unseen tiles cannot retain glyphs from an earlier frame.
    frame.render_widget(Clear, inner);
    let focus = level
        .player
        .filter(|&pos| level.map.in_bounds(pos))
        .or_else(|| level.focus.filter(|&pos| level.map.in_bounds(pos)))
        .unwrap_or(Position::new(0, 0));
    let origin_x = focus
        .x
        .saturating_sub(usize::from(inner.width) / 2)
        .min(level.map.width().saturating_sub(usize::from(inner.width)));
    let origin_y = focus
        .y
        .saturating_sub(usize::from(inner.height) / 2)
        .min(level.map.height().saturating_sub(usize::from(inner.height)));
    for dy in 0..usize::from(inner.height).min(level.map.height()) {
        for dx in 0..usize::from(inner.width).min(level.map.width()) {
            let pos = Position::new(origin_x + dx, origin_y + dy);
            if let Some(tile) = level.map.get(pos) {
                paint(
                    frame,
                    inner.x + dx as u16,
                    inner.y + dy as u16,
                    if tile.seen {
                        theme.tile(tile.kind)
                    } else {
                        theme.unseen
                    },
                );
            }
        }
    }
    let mut draw_entity = |pos: Position, glyph| {
        if level.map.is_visible(pos)
            && let (Some(x), Some(y)) = (pos.x.checked_sub(origin_x), pos.y.checked_sub(origin_y))
            && x < usize::from(inner.width)
            && y < usize::from(inner.height)
        {
            paint(frame, inner.x + x as u16, inner.y + y as u16, glyph);
        }
    };
    for entity in level.items.iter().chain(level.enemies) {
        draw_entity(entity.position, entity.glyph);
    }
    if let Some(position) = level.player {
        draw_entity(position, theme.player);
    }
}

fn paint(frame: &mut Frame, x: u16, y: u16, glyph: Glyph) {
    // One ASCII character per tile keeps viewport coordinates stable for custom themes.
    let symbol = if glyph.symbol.is_ascii_graphic() || glyph.symbol == ' ' {
        glyph.symbol
    } else {
        '?'
    };
    frame.buffer_mut()[(x, y)]
        .set_char(symbol)
        .set_style(glyph.style);
}

/// Draws available player data without inventing values for unfinished systems.
pub fn draw_status(frame: &mut Frame, area: Rect, status: &StatusView<'_>, theme: &Theme) {
    let health = status.health.map_or_else(
        || "Health: --".to_owned(),
        |(hp, max)| format!("Health: {hp} / {max}"),
    );
    let health_style = match status.health {
        Some((hp, max)) if max > 0 && hp <= max / 4 => theme.danger,
        _ => theme.text,
    };
    let effects = if status.effects.is_empty() {
        "--".to_owned()
    } else {
        status.effects.join(", ")
    };
    let equipment = if status.equipment.is_empty() {
        "--".to_owned()
    } else {
        status.equipment.join(", ")
    };
    let lines = vec![
        Line::from(Span::styled(health, health_style)),
        Line::from(format!("Effects: {effects}")),
        Line::from(format!("Equipment: {equipment}")),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.text)
            .wrap(Wrap { trim: false })
            .block(panel(" Pete / Status ", theme)),
        area,
    );
}

/// Draws the newest message lines in chronological order, bounded by panel height.
pub fn draw_messages(frame: &mut Frame, area: Rect, messages: &[&str], theme: &Theme) {
    let lines = messages
        .iter()
        .rev()
        .flat_map(|message| message.lines().rev());
    draw_message_lines(frame, area, lines, messages.is_empty(), theme);
}

/// Draws newest-first logical lines, collecting only what fits in the panel.
/// Lines are already split so interior blank lines retain their scroll positions.
pub(super) fn draw_message_lines<'a>(
    frame: &mut Frame,
    area: Rect,
    lines: impl Iterator<Item = &'a str>,
    empty: bool,
    theme: &Theme,
) {
    let block = panel(" Message log ", theme);
    let height = usize::from(block.inner(area).height);
    let lines: Vec<Line<'_>> = if empty {
        vec![Line::from("No messages yet.")]
    } else {
        let mut lines: Vec<_> = lines.take(height).map(Line::from).collect();
        lines.reverse();
        lines
    };
    frame.render_widget(Paragraph::new(lines).style(theme.text).block(block), area);
}

#[cfg(test)]
mod tests {
    use crate::{
        game::{
            level::map::{Map, TileType},
            position::Position,
            screen::Screen,
        },
        tui::{
            LevelView, MapEntity, RenderView,
            tests::{draw_screen, text},
            theme::Glyph,
        },
    };

    #[test]
    fn camera_focus_keeps_the_start_visible_without_a_player_on_small_terminals() {
        let mut map = Map::new(100, 30);
        let start = Position::new(85, 25);
        map.set(start, TileType::StairsDown);
        map.update_visibility(start);
        let mut level = LevelView::new(&map);
        level.focus = Some(start);
        // Invalid entity data must not override a valid camera focus.
        level.player = Some(Position::new(usize::MAX, usize::MAX));
        let view = RenderView {
            level: Some(level),
            ..Default::default()
        };
        for (width, height) in [(15, 10), (40, 20), (100, 30)] {
            let output = text(&draw_screen(width, height, Screen::Playing, &view));
            assert!(output.contains('>'));
            assert!(!output.contains('@'));
        }
    }

    #[test]
    fn redraws_remove_hidden_entities_and_clear_a_previous_floor() {
        use crate::tui::Renderer;
        use ratatui::{Terminal, backend::TestBackend};

        let mut map = Map::new(7, 3);
        for x in 0..7 {
            map.set(Position::new(x, 1), TileType::Floor);
        }
        map.set(Position::new(3, 1), TileType::Door { open: false });
        let left = Position::new(1, 1);
        let enemy = [MapEntity {
            position: left,
            glyph: Glyph::new('s'),
        }];
        let renderer = Renderer::default();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        for (origin, expected) in [(left, "s"), (Position::new(5, 1), ".")] {
            map.update_visibility(origin);
            let mut level = LevelView::new(&map);
            level.enemies = &enemy;
            let view = RenderView {
                level: Some(level),
                ..Default::default()
            };
            let buffer = &terminal
                .draw(|frame| renderer.render(frame, &Screen::Playing, &view))
                .unwrap()
                .buffer;
            assert_eq!(buffer[(2, 2)].symbol(), expected);
        }
        let fresh_floor = Map::new(7, 3);
        let view = RenderView {
            level: Some(LevelView::new(&fresh_floor)),
            ..Default::default()
        };
        let buffer = &terminal
            .draw(|frame| renderer.render(frame, &Screen::Playing, &view))
            .unwrap()
            .buffer;
        assert_eq!(buffer[(2, 2)].symbol(), ".");
    }

    #[test]
    fn fog_hides_unseen_terrain_and_entities_but_keeps_remembered_terrain() {
        let mut map = Map::new(7, 3);
        for x in 0..7 {
            map.set(Position::new(x, 1), TileType::Floor);
        }
        map.set(Position::new(3, 1), TileType::Door { open: false });
        let left = Position::new(1, 1);
        let right = Position::new(5, 1);
        let items = [MapEntity {
            position: left,
            glyph: Glyph::new('!'),
        }];
        let enemies = [MapEntity {
            position: right,
            glyph: Glyph::new('s'),
        }];
        map.update_visibility(left);
        {
            let mut level = LevelView::new(&map);
            level.items = &items;
            level.enemies = &enemies;
            let view = RenderView {
                level: Some(level),
                ..Default::default()
            };
            let buffer = draw_screen(20, 10, Screen::Playing, &view);
            assert_eq!(buffer[(2, 2)].symbol(), "!");
            assert_eq!(buffer[(4, 2)].symbol(), "+");
            assert_eq!(buffer[(6, 2)].symbol(), ".");
            assert_eq!(
                buffer[(6, 2)].fg,
                ratatui::style::Color::Rgb(0x1b, 0x30, 0x33)
            );
        }
        map.update_visibility(right);
        let mut level = LevelView::new(&map);
        level.items = &items;
        level.enemies = &enemies;
        let view = RenderView {
            level: Some(level),
            ..Default::default()
        };
        for screen in [Screen::Playing, Screen::Paused] {
            let buffer = draw_screen(40, 20, screen, &view);
            assert_eq!(buffer[(2, 2)].symbol(), ".");
            assert_eq!(buffer[(6, 2)].symbol(), "s");
        }
        assert!(map.get(left).unwrap().seen);
        assert!(!map.get(left).unwrap().visible);
    }

    #[test]
    fn entity_layers_override_terrain_and_reject_out_of_bounds_positions() {
        let mut map = Map::new(4, 3);
        let position = Position::new(1, 1);
        map.set(position, TileType::Floor);
        map.update_visibility(position);
        let items = [MapEntity {
            position,
            glyph: Glyph::new('!'),
        }];
        let enemies = [
            MapEntity {
                position,
                glyph: Glyph::new('g'),
            },
            MapEntity {
                position: Position::new(99, 99),
                glyph: Glyph::new('X'),
            },
        ];
        let mut view = RenderView {
            level: Some(LevelView::new(&map)),
            ..Default::default()
        };
        view.level.as_mut().unwrap().items = &items;
        assert_eq!(
            draw_screen(20, 10, Screen::Playing, &view)[(2, 2)].symbol(),
            "!"
        );
        view.level.as_mut().unwrap().enemies = &enemies;
        let buffer = draw_screen(20, 10, Screen::Playing, &view);
        assert_eq!(buffer[(2, 2)].symbol(), "g");
        assert!(!text(&buffer).contains('X'));
        view.level.as_mut().unwrap().player = Some(position);
        assert_eq!(
            draw_screen(20, 10, Screen::Playing, &view)[(2, 2)].symbol(),
            "@"
        );
        assert!(matches!(map.get(position).unwrap().kind, TileType::Floor));
    }

    #[test]
    fn viewport_keeps_player_visible_at_each_map_corner() {
        let mut map = Map::new(100, 100);
        for position in [
            Position::new(0, 0),
            Position::new(99, 0),
            Position::new(0, 99),
            Position::new(99, 99),
        ] {
            map.update_visibility(position);
            let mut level = LevelView::new(&map);
            level.player = Some(position);
            let view = RenderView {
                level: Some(level),
                ..Default::default()
            };
            assert!(text(&draw_screen(15, 10, Screen::Playing, &view)).contains('@'));
        }
    }

    #[test]
    fn message_log_keeps_newest_messages_in_chronological_order() {
        let messages: Vec<_> = (0..30).map(|n| format!("message {n:02}")).collect();
        let borrowed: Vec<_> = messages.iter().map(String::as_str).collect();
        let view = RenderView {
            messages: &borrowed,
            ..Default::default()
        };
        let output = text(&draw_screen(100, 24, Screen::Playing, &view));
        assert!(!output.contains("message 00"));
        assert!(output.find("message 28").unwrap() < output.find("message 29").unwrap());
    }

    #[test]
    fn short_terminals_keep_the_latest_multiline_message_at_the_bottom() {
        let view = RenderView {
            messages: &["old message", "first line\nlatest line"],
            ..Default::default()
        };
        let output = text(&draw_screen(60, 12, Screen::Playing, &view));
        assert!(output.contains("Message log"));
        assert!(output.contains("latest line"));
        assert!(!output.contains("first line") && !output.contains("old message"));
    }
}
