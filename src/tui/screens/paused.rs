//! Pause overlay over the current dungeon view.

use super::{centered, panel};
use crate::tui::{RenderView, theme::Theme};
use ratatui::{
    Frame,
    layout::Alignment,
    widgets::{Clear, Paragraph, Wrap},
};

/// Draws PAUSED followed by a left-aligned overview of the current session.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let depth = view
        .level
        .as_ref()
        .and_then(|level| level.depth)
        .map_or_else(|| "--".to_owned(), |depth| depth.to_string());
    let turns = view
        .status
        .turns_and_fuel
        .map_or_else(|| "--".to_owned(), |(turns, _)| turns.to_string());
    let text = format!(
        "PAUSED\n\nDepth: {depth}\nTurns: {turns}\nEnemies slain: {}\nItems acquired: {}",
        view.session_stats.enemies_slain, view.session_stats.items_acquired,
    );
    draw_overlay(frame, theme, " Paused ", &text, Alignment::Left);
}

/// Shared centered panel with explicit text alignment for pause and controls.
pub fn draw_overlay(
    frame: &mut Frame,
    theme: &Theme,
    title: &str,
    text: &str,
    alignment: Alignment,
) {
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
            .alignment(alignment)
            .wrap(Wrap { trim: false })
            .style(theme.accent)
            .block(panel(title, theme)),
        area,
    );
}
