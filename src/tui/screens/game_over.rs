//! Final session summary presentation.

use super::{centered, panel, playing::draw_messages};
use crate::tui::{RenderView, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    text::Line,
    widgets::{Paragraph, Wrap},
};

/// Draws the available outcome and final depth without assuming a cause of death.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let area = frame.area();
    let summary_area = if view.messages.is_empty() {
        area
    } else {
        let log_height = if area.height >= 16 {
            7
        } else {
            area.height / 2
        };
        let [summary, log] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(log_height)]).areas(area);
        draw_messages(frame, log, view.messages, theme);
        summary
    };
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
            .wrap(Wrap { trim: false })
            .style(theme.text)
            .block(panel(" Expedition results ", theme)),
        centered(summary_area, 64, 12),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::screen::Screen,
        tui::tests::{draw_screen, text},
    };

    #[test]
    fn results_show_cause_depth_and_the_lethal_message() {
        let view = RenderView {
            outcome: Some("You were defeated by Slime."),
            final_depth: Some(3),
            messages: &[
                "Slime hits you for 1 damage.",
                "You were defeated by Slime.",
            ],
            ..Default::default()
        };
        let output = text(&draw_screen(100, 30, Screen::GameOver, &view));
        for expected in [
            "GAME OVER",
            "Final depth: 3",
            "Slime hits you for 1 damage.",
            "You were defeated by Slime.",
        ] {
            assert!(output.contains(expected));
        }
        for (width, height) in [(0, 0), (1, 1), (10, 4), (30, 10)] {
            draw_screen(width, height, Screen::GameOver, &view);
        }
    }
}
