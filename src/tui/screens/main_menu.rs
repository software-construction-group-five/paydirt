//! Main menu presentation; selection and actions remain owned by the caller.

use super::{centered, panel};
use crate::tui::{RenderView, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{List, ListItem, ListState, Paragraph},
};

// Original ASCII portrait: Pete's broad miner's hat, mustache, and work shirt.
const PETE: &str = r"         .--------.
        /   UTEP   \
    ___/____________\___
   /____________________\
        |  o    o  |
        |    /\    |
       /  /======\  \
       \  \______/  /
        '----..----'
       /|   PETE   |\
      / |__________| \
        /          \";

const TITLE: &str = r" ____   _ __   ______ ___ ____ _____
|  _ \ / \\ \ / /  _ \_ _|  _ \_   _|
| |_) / _ \\ V /| | | | || |_) || |
|  __/ ___ \| | | |_| | ||  _ < | |
|_| /_/   \_\_| |____/___|_| \_\|_|";

/// Draws Pete, the title, and selectable New Game / Quit entries.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let area = centered(frame.area(), 64, 24);
    let block = panel(" PAYDIRT ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let title_width = TITLE.lines().map(str::len).max().unwrap_or(0) as u16;
    let banner = inner.width >= title_width && inner.height >= 9;
    let title_height = if banner { 5 } else { 1 };
    // Preserve the full title and controls before making room for the portrait.
    let logo_height = if inner.height >= 12 + title_height + 4 && inner.width >= 34 {
        12
    } else {
        0
    };
    let gap = inner
        .height
        .saturating_sub(logo_height + title_height + 3)
        .min(1);
    let [logo, title, _, menu, help] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(title_height),
        Constraint::Length(gap),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .areas(inner);
    if logo_height > 0 {
        frame.render_widget(
            Paragraph::new(PETE).style(theme.player.style),
            centered(logo, 26, 12),
        );
    }
    if banner {
        frame.render_widget(
            Paragraph::new(TITLE).style(theme.accent),
            centered(title, title_width, title_height),
        );
    } else {
        frame.render_widget(
            Paragraph::new("PAYDIRT").centered().style(theme.accent),
            title,
        );
    }
    let entries = [ListItem::new("New Game"), ListItem::new("Quit")];
    let mut state = ListState::default().with_selected(Some(view.selected.min(1)));
    frame.render_stateful_widget(
        List::new(entries)
            .highlight_style(theme.accent)
            .highlight_symbol("> "),
        menu,
        &mut state,
    );
    frame.render_widget(Paragraph::new(view.help).style(theme.muted), help);
}
