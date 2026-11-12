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

/// Styles only the damage expression of an item label using its rare effect cue.
pub(crate) fn item_spans(
    label: &str,
    normal: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'_>> {
    use ratatui::{style::Color, text::Span};
    let Some(start) = label.rfind('(') else {
        return vec![Span::styled(label, normal)];
    };
    let Some(end) = label[start..].find(')').map(|end| start + end + 1) else {
        return vec![Span::styled(label, normal)];
    };
    let damage = &label[start..end];
    let color = if damage.ends_with(" Bleed)") {
        Some(Color::Red)
    } else if damage.ends_with(" Weakness)") {
        Some(Color::Rgb(180, 90, 255))
    } else if damage.ends_with(" Lightning)") {
        Some(Color::Rgb(0, 191, 255))
    } else {
        None
    };
    vec![
        Span::styled(&label[..start], normal),
        Span::styled(damage, color.map_or(normal, |color| normal.fg(color))),
        Span::styled(&label[end..], normal),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_damage_text_uses_the_effect_color() {
        use ratatui::style::{Color, Style};
        for (effect, color) in [
            ("Bleed", Color::Red),
            ("Weakness", Color::Rgb(180, 90, 255)),
            ("Lightning", Color::Rgb(0, 191, 255)),
        ] {
            let label = format!("Shovel (2d4+2 {effect})");
            let spans = super::item_spans(&label, Style::default().fg(Color::White));
            assert_eq!(spans[0].style.fg, Some(Color::White));
            assert_eq!(spans[1].style.fg, Some(color));
            assert_eq!(spans[1].content, format!("(2d4+2 {effect})"));
        }
    }
}
