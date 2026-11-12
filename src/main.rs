//! Menu-driven launcher using Game state and the TUI rendering/input APIs.
//!
//! Game owns navigation and level generation. The launcher forwards TUI actions.

use std::{io, path::Path};

use paydirt::{
    game::{
        Game, InventorySelection,
        item::{Item, ItemType},
        screen::Screen,
    },
    tui::{
        InventoryEntry, LevelView, RenderView, Renderer, StatusView, input::Keybinds, theme::Theme,
    },
};
use ratatui::DefaultTerminal;

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if matches!(args.as_slice(), [flag] if flag == "--help" || flag == "-h") {
        println!("Usage: paydirt\n\nLoads config/theme.toml and config/keybinds.toml.");
        return Ok(());
    }
    if !args.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: paydirt; use --help for details",
        ));
    }
    // Validate both configs before raw mode, independently of the launch directory.
    let config_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
    let renderer = Renderer::new(Theme::load(config_dir.join("theme.toml"))?);
    let keybinds = Keybinds::load(config_dir.join("keybinds.toml"))?;
    // Ratatui's panic hook also restores the terminal if drawing panics.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &renderer, &keybinds);
    ratatui::restore();
    result
}

/// Draws Game's active screen and forwards configured input to its controller.
fn run(terminal: &mut DefaultTerminal, renderer: &Renderer, keybinds: &Keybinds) -> io::Result<()> {
    let mut game = Game::new();
    while !game.should_quit() {
        let help = keybinds.controls(&game.help_origin().unwrap_or(*game.screen()));
        with_view(&game, renderer, &help, |view| {
            terminal.draw(|frame| renderer.render(frame, game.screen(), view))
        })?;
        if let Some(action) = keybinds.read_input(game.screen())? {
            game.process_action(action);
        }
    }
    Ok(())
}

/// Builds the display data from Game's current state and passes it to `draw`
/// (Player Contract 6 for position and health, Renderer Contract 20).
fn with_view<R>(
    game: &Game,
    renderer: &Renderer,
    help: &str,
    draw: impl FnOnce(&RenderView<'_>) -> R,
) -> R {
    let player = game.player();
    let player_position = player.map(|player| player.get_position());
    let enemies = game
        .level()
        .map(|level| renderer.enemy_entities(level))
        .unwrap_or_default();
    let items = game
        .level()
        .map(|level| renderer.item_entities(level))
        .unwrap_or_default();
    let log = if game.help_origin().unwrap_or(*game.screen()) == Screen::GameOver {
        game.message_log().history()
    } else {
        game.message_log().messages()
    };
    let messages: Vec<&str> = log.iter().map(String::as_str).collect();
    let level = game.level().map(|level| {
        let mut view = LevelView::new(level.get_map());
        view.depth = Some(level.depth());
        view.enemies = &enemies;
        view.items = &items;
        view.player = player_position;
        // The camera follows the player once they exist
        view.focus = player_position.or(Some(level.player_start()));
        view
    });
    let equipment: Vec<String> = player
        .into_iter()
        .flat_map(|player| {
            let equipment = player.get_equipped_items();
            equipment.weapon.iter().chain(equipment.armor.iter())
        })
        .map(item_label)
        .collect();
    let equipment_names: Vec<&str> = equipment.iter().map(String::as_str).collect();
    let effects: Vec<String> = player
        .into_iter()
        .flat_map(|player| player.get_stats().status_effects())
        .map(|effect| format!("{} ({})", effect.name(), effect.turns_remaining()))
        .collect();
    let effect_names: Vec<&str> = effects.iter().map(String::as_str).collect();
    let inventory_rows = game.inventory_rows();
    let inventory_labels: Vec<String> = inventory_rows
        .iter()
        .map(|(_, item)| item_label(item))
        .collect();
    let inventory: Vec<InventoryEntry<'_>> = inventory_rows
        .iter()
        .zip(&inventory_labels)
        .map(|((selection, item), name)| InventoryEntry {
            name,
            category: match item.kind {
                ItemType::Weapon => "Weapon",
                ItemType::Armor => "Armor",
                ItemType::Consumable => "Consumable",
            },
            key: None,
            equipped: matches!(selection, InventorySelection::Equipped(_)),
        })
        .collect();
    let status = StatusView {
        health: player.map(|player| (player.get_stats().hp(), player.get_stats().max_hp())),
        equipment: &equipment_names,
        effects: &effect_names,
    };
    let view = RenderView {
        level,
        status,
        messages: &messages,
        inventory: &inventory,
        selected: if game.help_origin().unwrap_or(*game.screen()) == Screen::Inventory {
            game.inventory_selection()
        } else {
            game.menu_selection()
        },
        history_offset: game.history_offset(),
        help_origin: game.help_origin(),
        help,
        outcome: game.summary().map(|summary| summary.outcome.as_str()),
        final_depth: game.summary().map(|summary| summary.final_depth),
    };
    draw(&view)
}

/// Formats the same weapon label in the status panel and inventory rows.
fn item_label(item: &Item) -> String {
    item.combat().damage().map_or_else(
        || item.name.clone(),
        |dice| format!("{} ({dice})", item.name),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use paydirt::game::{action::Action, screen::Screen};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn inventory_view_uses_equipped_weapon_dice_and_selection() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::OpenInventory);
        let renderer = Renderer::default();
        with_view(&game, &renderer, "", |view| {
            assert_eq!(view.inventory.len(), 1);
            assert_eq!(view.inventory[0].name, "Pickaxe (2d4)");
            assert!(view.inventory[0].equipped);
            assert_eq!(view.selected, 0);
            assert_eq!(view.status.health, Some((40, 40)));
        });
    }

    #[test]
    fn playing_screen_shows_the_player_and_health() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        let renderer = Renderer::default();
        let mut terminal = Terminal::new(TestBackend::new(120, 35)).unwrap();
        with_view(&game, &renderer, "", |view| {
            terminal.draw(|frame| renderer.render(frame, &Screen::Playing, view))
        })
        .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains('@'));
        assert!(screen.contains("Health: 40 / 40"));
        assert!(screen.contains("Pickaxe (2d4)"));

        let keybinds = Keybinds::parse(include_str!("../config/keybinds.toml")).unwrap();
        for code in [
            crossterm::event::KeyCode::Up,
            crossterm::event::KeyCode::Right,
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyCode::Char('.'),
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyCode::Enter,
        ] {
            let key = crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            let action = keybinds.action(game.screen(), key).unwrap();
            game.process_action(action);
            let position = game.player().unwrap().get_position();
            let level = game.level().unwrap();
            let help = keybinds.controls(&game.help_origin().unwrap_or(*game.screen()));
            with_view(&game, &renderer, &help, |view| {
                let level_view = view.level.as_ref().unwrap();
                assert_eq!(level_view.player, Some(position));
                assert_eq!(level_view.focus, Some(position));
                assert_eq!(level_view.enemies.len(), level.visible_enemies().count());
                assert_eq!(level_view.items.len(), level.visible_ground_items().count());
                assert!(
                    level_view
                        .enemies
                        .iter()
                        .chain(level_view.items)
                        .all(|entity| { level.get_map().is_visible(entity.position) })
                );
                assert_eq!(view.status.health, Some((40, 40)));
                terminal.draw(|frame| renderer.render(frame, game.screen(), view))
            })
            .unwrap();
            // The centered pause overlay can cover the player on some floors.
            if matches!(game.screen(), Screen::Playing) {
                assert_eq!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .filter(|cell| cell.symbol() == "@")
                        .count(),
                    1
                );
            }
        }
    }
}
