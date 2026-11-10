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
    widgets::{Paragraph, Wrap},
};

/// Draws gameplay above a full-width message log, with status beside wide maps.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let [body, help] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    frame.render_widget(Paragraph::new(view.help).style(theme.muted), help);
    let (map_area, status_area, log_area) = if body.width >= 90 && body.height >= 12 {
        let [content, log] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(5)]).areas(body);
        let [map, status] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(28)]).areas(content);
        (map, status, log)
    } else if body.height >= 16 {
        let [map, status, log] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(5),
            Constraint::Length(5),
        ])
        .areas(body);
        (map, status, log)
    } else {
        (body, Rect::default(), Rect::default())
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
    let focus = level
        .player
        .filter(|&pos| level.map.in_bounds(pos))
        .unwrap_or(Position::new(0, 0));
    let origin_x = focus
        .x
        .saturating_sub(usize::from(inner.width) / 2)
        .min(level.map.width.saturating_sub(usize::from(inner.width)));
    let origin_y = focus
        .y
        .saturating_sub(usize::from(inner.height) / 2)
        .min(level.map.height.saturating_sub(usize::from(inner.height)));
    for dy in 0..inner.height {
        for dx in 0..inner.width {
            let pos = Position::new(origin_x + usize::from(dx), origin_y + usize::from(dy));
            if let Some(tile) = level.map.get(pos) {
                paint(frame, inner.x + dx, inner.y + dy, theme.tile(tile.kind));
            }
        }
    }
    for entity in level.items.iter().chain(level.enemies) {
        draw_entity(
            frame,
            inner,
            (origin_x, origin_y),
            level,
            entity.position,
            entity.glyph,
        );
    }
    if let Some(position) = level.player {
        draw_entity(
            frame,
            inner,
            (origin_x, origin_y),
            level,
            position,
            theme.player,
        );
    }
}

fn draw_entity(
    frame: &mut Frame,
    area: Rect,
    origin: (usize, usize),
    level: &LevelView<'_>,
    pos: Position,
    glyph: Glyph,
) {
    if !level.map.in_bounds(pos) {
        return;
    }
    if let (Some(x), Some(y)) = (pos.x.checked_sub(origin.0), pos.y.checked_sub(origin.1))
        && x < usize::from(area.width)
        && y < usize::from(area.height)
    {
        paint(frame, area.x + x as u16, area.y + y as u16, glyph);
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

/// Draws the newest messages in chronological order, clipped to the panel height.
pub fn draw_messages(frame: &mut Frame, area: Rect, messages: &[&str], theme: &Theme) {
    let block = panel(" Message log ", theme);
    let height = usize::from(block.inner(area).height);
    let lines: Vec<Line<'_>> = if messages.is_empty() {
        vec![Line::from("No messages yet.")]
    } else {
        messages
            .iter()
            .skip(messages.len().saturating_sub(height))
            .map(|message| Line::from(*message))
            .collect()
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
    fn entity_layers_override_terrain_and_reject_out_of_bounds_positions() {
        let mut map = Map::new(4, 3);
        let position = Position::new(1, 1);
        map.set(position, TileType::Floor);
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
        let map = Map::new(100, 100);
        for position in [
            Position::new(0, 0),
            Position::new(99, 0),
            Position::new(0, 99),
            Position::new(99, 99),
        ] {
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
}
