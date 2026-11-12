//! Final session summary presentation.

use super::{centered, panel, playing::draw_message_lines};
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
    let line_count = view
        .messages
        .iter()
        .map(|message| message.lines().count())
        .sum::<usize>();
    let offset = view.history_offset.min(line_count.saturating_sub(1));
    let history = view
        .messages
        .iter()
        .rev()
        .flat_map(|message| message.lines().rev())
        .skip(offset);
    let summary_area = if view.messages.is_empty() {
        area
    } else {
        let log_height = if area.height >= 16 {
            7
        } else if area.height >= 8 {
            area.height / 2
        } else {
            // Preserve at least a readable history line before the summary.
            area.height
        };
        let [summary, log] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(log_height)]).areas(area);
        draw_message_lines(frame, log, history, view.messages.is_empty(), theme);
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
    fn scrolling_reaches_every_multiline_history_line_on_short_terminals() {
        let messages = ["oldest\nsecond", "third", "newest"];
        for (offset, expected) in [
            (0, "newest"),
            (1, "third"),
            (2, "second"),
            (3, "oldest"),
            (usize::MAX, "oldest"),
        ] {
            let view = RenderView {
                history_offset: offset,
                messages: &messages,
                help: "up: older down: newer pageup: older page pagedown: newer page enter: menu",
                ..Default::default()
            };
            let output = text(&draw_screen(20, 4, Screen::GameOver, &view));
            assert!(output.contains(expected), "offset {offset}: {output}");
        }
    }

    #[test]
    fn scrolling_preserves_blank_history_lines() {
        let messages = ["oldest\n\nnewest"];
        let view = RenderView {
            messages: &messages,
            history_offset: 1,
            ..Default::default()
        };
        // A one-line history viewport should show the blank line itself.
        let output = text(&draw_screen(20, 3, Screen::GameOver, &view));
        assert!(!output.contains("oldest"));
        assert!(!output.contains("newest"));
        let output = text(&draw_screen(20, 4, Screen::GameOver, &view));
        assert!(output.contains("oldest"));
        assert!(!output.contains("newest"));
    }

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
