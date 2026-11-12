//! Pause overlay over the current dungeon view.

use super::{centered, panel};
use crate::tui::theme::Theme;
use ratatui::{
    Frame,
    widgets::{Clear, Paragraph, Wrap},
};

/// Draws a centered pause notice without changing the underlying game state.
pub fn draw(frame: &mut Frame, theme: &Theme) {
    draw_overlay(frame, theme, " Paused ", "PAUSED\nTake a breather, Pete.");
}

/// Shared centered overlay for pause and controls, differing only in text.
pub fn draw_overlay(frame: &mut Frame, theme: &Theme, title: &str, text: &str) {
    let width = frame.area().width.min(40);
    let columns = usize::from(width.saturating_sub(2)).max(1);
    let lines = text
        .lines()
        .map(|line| line.chars().count().div_ceil(columns).max(1))
        .sum::<usize>();
    let height = lines.saturating_add(2).max(5).min(u16::MAX as usize) as u16;
    let area = centered(frame.area(), 40, height);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .centered()
            .wrap(Wrap { trim: false })
            .style(theme.accent)
            .block(panel(title, theme)),
        area,
    );
}
