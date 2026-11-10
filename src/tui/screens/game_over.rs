//! Final session summary presentation.

use super::{centered, panel};
use crate::tui::{RenderView, theme::Theme};
use ratatui::{Frame, text::Line, widgets::Paragraph};

/// Draws the available outcome and final depth without assuming a cause of death.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let depth = view.final_depth.map_or_else(
        || "Final depth: --".to_owned(),
        |depth| format!("Final depth: {depth}"),
    );
    let lines = vec![
        Line::from("GAME OVER").style(theme.danger),
        Line::from(""),
        Line::from(view.outcome.unwrap_or("The expedition has ended.")),
        Line::from(depth),
        Line::from(""),
        Line::from(view.help).style(theme.muted),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .centered()
            .style(theme.text)
            .block(panel(" Expedition results ", theme)),
        centered(frame.area(), 64, 12),
    );
}
