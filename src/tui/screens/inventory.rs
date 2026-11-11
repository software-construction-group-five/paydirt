//! Read-only inventory list with selection and equipment markers.

use super::panel;
use crate::tui::{RenderView, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{List, ListItem, ListState, Paragraph},
};

/// Draws inventory rows and scrolls to the selected item using local widget state.
pub fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme) {
    let [body, help] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    let block = panel(" Inventory ", theme);
    if view.inventory.is_empty() {
        frame.render_widget(Paragraph::new("Your pack is empty.").block(block), body);
    } else {
        let rows: Vec<ListItem<'_>> = view
            .inventory
            .iter()
            .map(|entry| {
                let key = entry
                    .key
                    .map_or_else(String::new, |key| format!("[{key}] "));
                let equipped = if entry.equipped { " (equipped)" } else { "" };
                ListItem::new(format!(
                    "{key}{} / {}{equipped}",
                    entry.name, entry.category
                ))
            })
            .collect();
        let mut state = ListState::default().with_selected(Some(view.selected.min(rows.len() - 1)));
        frame.render_stateful_widget(
            List::new(rows)
                .block(block)
                .style(theme.text)
                .highlight_style(theme.accent)
                .highlight_symbol("> "),
            body,
            &mut state,
        );
    }
    frame.render_widget(Paragraph::new(view.help).style(theme.muted), help);
}

#[cfg(test)]
mod tests {
    use crate::{
        game::screen::Screen,
        tui::{
            InventoryEntry, RenderView,
            tests::{draw_screen, text},
        },
    };

    #[test]
    fn inventory_scrolls_to_selection_and_marks_equipment() {
        let inventory: Vec<_> = (0..30)
            .map(|_| InventoryEntry {
                name: "Potion",
                category: "Consumable",
                key: None,
                equipped: false,
            })
            .chain([InventoryEntry {
                name: "Pickaxe",
                category: "Weapon",
                key: Some('z'),
                equipped: true,
            }])
            .collect();
        let view = RenderView {
            inventory: &inventory,
            selected: 30,
            ..Default::default()
        };
        let output = text(&draw_screen(60, 8, Screen::Inventory, &view));
        assert!(output.contains("> [z] Pickaxe / Weapon (equipped)"));
    }
}
