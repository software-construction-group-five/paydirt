//! Pause overlay over the current dungeon view.

use super::{centered, panel};
use crate::tui::theme::Theme;
use ratatui::{
    Frame,
    widgets::{Clear, Paragraph},
};

/// Draws a centered pause notice without changing the underlying game state.
pub fn draw(frame: &mut Frame, theme: &Theme) {
    let area = centered(frame.area(), 40, 5);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(
            "PAUSED
Take a breather, Pete.",
        )
        .centered()
        .style(theme.accent)
        .block(panel(" Paused ", theme)),
        area,
    );
}
