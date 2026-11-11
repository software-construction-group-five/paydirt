//! Screen drawing and shared layout helpers.

pub mod game_over;
pub mod inventory;
pub mod main_menu;
pub mod paused;
pub mod playing;

use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders},
};

pub(crate) fn panel<'a>(title: impl Into<ratatui::text::Line<'a>>, theme: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(theme.muted)
        .title_style(theme.accent)
}

pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
