//! Draws the Playing screen.
//!
//! - draw_level: map tiles, floor items, enemies, player (contract 20, protocol 2)
//! - draw_status: health, effects, equipment, message log (contract 20, protocol 3)

use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::game::level::map::Map;
use crate::game::position::Position;
use crate::tui::theme::Theme;

/// Draws the map one character per tile, inside a bordered box.
/// TODO: take &Level and &Player, and draw floor items, enemies, and the player.
pub fn draw_level(frame: &mut Frame, map: &Map, depth: usize, theme: &Theme) {
    let lines: Vec<Line> = (0..map.height)
        .map(|y| {
            let row: String = (0..map.width)
                .map(|x| map.get(Position::new(x, y)).map_or(' ', |tile| theme.tile(tile.kind)))
                .collect();
            Line::from(row)
        })
        .collect();

    let title = format!(" Depth {depth} | r: regenerate  >: deeper  q: quit ");
    let block = Block::bordered().title(title);
    frame.render_widget(Paragraph::new(lines).block(block), frame.area());
}

// TODO: pub fn draw_status(frame: &mut Frame, player: &Player, log: &MessageLog)