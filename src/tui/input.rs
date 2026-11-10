//! Configurable keyboard interpretation (contract 21), independent of gameplay.

use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::Path,
};

use crate::game::{action::Action, screen::Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::Deserialize;

/// Configured keys grouped by action, preserving their display order.
struct Binding {
    action: Action,
    keys: Vec<(KeyCode, KeyModifiers)>,
    labels: Vec<String>,
    description: &'static str,
}

/// Commands supported only by the generated-level viewer, outside gameplay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewCommand {
    /// Generate a new layout at the current depth.
    Regenerate,
    /// Preview the next dungeon depth.
    NextLevel,
    /// Close the terminal viewer.
    Quit,
}

struct PreviewBinding {
    command: PreviewCommand,
    keys: Vec<(KeyCode, KeyModifiers)>,
    labels: Vec<String>,
}

/// Global bindings and screen-specific action mappings.
pub struct Keybinds {
    global: Vec<Binding>,
    screens: [Vec<Binding>; 5],
    preview: Vec<PreviewBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    global: BTreeMap<String, Vec<String>>,
    main_menu: BTreeMap<String, Vec<String>>,
    playing: BTreeMap<String, Vec<String>>,
    inventory: BTreeMap<String, Vec<String>>,
    paused: BTreeMap<String, Vec<String>>,
    game_over: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    preview: BTreeMap<String, Vec<String>>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Keybinds {
    /// Loads configured actions before terminal raw mode is enabled.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        fs::read_to_string(path)
            .and_then(|source| Self::parse(&source))
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
    }

    /// Parses screen-specific mappings, rejecting invalid or conflicting keys.
    pub fn parse(source: &str) -> io::Result<Self> {
        let config: Config = toml::from_str(source).map_err(|error| invalid(error.to_string()))?;
        let global = bindings(config.global)?;
        let mut screens = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for (index, table) in [
            config.main_menu,
            config.playing,
            config.inventory,
            config.paused,
            config.game_over,
        ]
        .into_iter()
        .enumerate()
        {
            screens[index] = bindings(table)?;
            let mut occupied = HashSet::new();
            for binding in global.iter().chain(&screens[index]) {
                for key in &binding.keys {
                    if !occupied.insert(*key) {
                        return Err(invalid(format!(
                            "duplicate key binding: {key:?} in screen {index}"
                        )));
                    }
                }
            }
        }
        let mut preview = Vec::new();
        let mut occupied: HashSet<_> = global
            .iter()
            .chain(&screens[1])
            .flat_map(|binding| binding.keys.iter().copied())
            .collect();
        for (name, labels) in config.preview {
            let command = match name.as_str() {
                "regenerate" => PreviewCommand::Regenerate,
                "next_level" => PreviewCommand::NextLevel,
                _ => return Err(invalid(format!("unknown preview command: {name}"))),
            };
            if labels.is_empty() {
                return Err(invalid(format!("{name} needs at least one key")));
            }
            let keys: Vec<_> = labels
                .iter()
                .map(|label| parse_key(label))
                .collect::<io::Result<_>>()?;
            for key in &keys {
                if !occupied.insert(*key) {
                    return Err(invalid(format!("duplicate preview key: {key:?}")));
                }
            }
            preview.push(PreviewBinding {
                command,
                keys,
                labels,
            });
        }
        Ok(Self {
            global,
            screens,
            preview,
        })
    }

    /// Interprets a key press for the active screen; unmapped/released keys are ignored.
    pub fn action(&self, screen: &Screen, key: KeyEvent) -> Option<&Action> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        self.global
            .iter()
            .chain(&self.screens[screen_index(screen)])
            .find(|binding| binding.keys.contains(&(key.code, key.modifiers)))
            .map(|binding| &binding.action)
    }

    /// Interprets viewer input without assigning gameplay meaning to preview commands.
    pub fn preview_command(&self, key: KeyEvent) -> Option<PreviewCommand> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        let key = (key.code, key.modifiers);
        if self
            .global
            .iter()
            .any(|binding| matches!(binding.action, Action::Quit) && binding.keys.contains(&key))
        {
            return Some(PreviewCommand::Quit);
        }
        self.preview
            .iter()
            .find(|binding| binding.keys.contains(&key))
            .map(|binding| binding.command)
    }

    /// Builds viewer hints from its configured commands and global quit keys.
    pub fn preview_help(&self) -> String {
        let mut hints: Vec<_> = self
            .preview
            .iter()
            .map(|binding| {
                let label = match binding.command {
                    PreviewCommand::Regenerate => "regenerate",
                    PreviewCommand::NextLevel => "deeper",
                    PreviewCommand::Quit => "quit",
                };
                format!("{}: {label}", binding.labels.join("/"))
            })
            .collect();
        for binding in &self.screens[1] {
            if matches!(binding.action, Action::OpenInventory | Action::Pause) {
                hints.push(format!(
                    "{}: {}",
                    binding.labels.join("/"),
                    binding.description
                ));
            }
        }
        for binding in &self.global {
            if matches!(binding.action, Action::Quit) {
                hints.push(format!("{}: quit", binding.labels.join("/")));
            }
        }
        hints.join("  ")
    }

    /// Builds control hints from the same mappings used to interpret input.
    pub fn help(&self, screen: &Screen) -> String {
        self.screens[screen_index(screen)]
            .iter()
            .chain(&self.global)
            .map(|binding| format!("{}: {}", binding.labels.join("/"), binding.description))
            .collect::<Vec<_>>()
            .join("  ")
    }
}

fn screen_index(screen: &Screen) -> usize {
    match screen {
        Screen::MainMenu => 0,
        Screen::Playing => 1,
        Screen::Inventory => 2,
        Screen::Paused => 3,
        Screen::GameOver => 4,
    }
}

fn bindings(table: BTreeMap<String, Vec<String>>) -> io::Result<Vec<Binding>> {
    table
        .into_iter()
        .map(|(name, labels)| {
            let (action, description) = match name.as_str() {
                "move_up" => (Action::MoveUp, "up"),
                "move_down" => (Action::MoveDown, "down"),
                "move_left" => (Action::MoveLeft, "left"),
                "move_right" => (Action::MoveRight, "right"),
                "wait" => (Action::Wait, "wait"),
                "pick_up" => (Action::PickUp, "pick up"),
                "open_inventory" => (Action::OpenInventory, "inventory"),
                "pause" => (Action::Pause, "pause/resume"),
                "confirm" => (Action::Confirm, "confirm"),
                "back" => (Action::Back, "back"),
                "quit" => (Action::Quit, "quit"),
                _ => return Err(invalid(format!("unknown action: {name}"))),
            };
            if labels.is_empty() {
                return Err(invalid(format!("{name} needs at least one key")));
            }
            let keys = labels
                .iter()
                .map(|label| parse_key(label))
                .collect::<io::Result<_>>()?;
            Ok(Binding {
                action,
                keys,
                labels,
                description,
            })
        })
        .collect()
}

fn parse_key(label: &str) -> io::Result<(KeyCode, KeyModifiers)> {
    let parts: Vec<_> = label.split('+').collect();
    let mut modifiers = KeyModifiers::NONE;
    for modifier in &parts[..parts.len() - 1] {
        modifiers |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return Err(invalid(format!("invalid modifier in key: {label:?}"))),
        };
    }
    let name = parts[parts.len() - 1];
    let code = match name.to_ascii_lowercase().as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "plus" => KeyCode::Char('+'),
        _ if name.chars().count() == 1 => {
            let mut character = name.chars().next().unwrap();
            if !character.is_ascii_graphic() {
                return Err(invalid(format!("invalid key: {label:?}")));
            }
            if character.is_ascii_uppercase() {
                modifiers |= KeyModifiers::SHIFT;
            }
            if modifiers.contains(KeyModifiers::SHIFT) {
                character = character.to_ascii_uppercase();
            }
            KeyCode::Char(character)
        }
        _ => return Err(invalid(format!("invalid key: {label:?}"))),
    };
    Ok((code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONFIG: &str = include_str!("../../config/keybinds.toml");

    #[test]
    fn remapping_changes_actions_and_help_and_preserves_screen_context() {
        let config = CONFIG.replace("[\"right\", \"d\"]", "[\"l\"]");
        let bindings = Keybinds::parse(&config).unwrap();
        assert!(matches!(
            bindings.action(
                &Screen::Playing,
                KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)
            ),
            Some(Action::MoveRight)
        ));
        assert!(
            bindings
                .action(
                    &Screen::Playing,
                    KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)
                )
                .is_none()
        );
        assert!(bindings.help(&Screen::Playing).contains("l: right"));
        assert!(matches!(
            bindings.action(
                &Screen::Playing,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            ),
            Some(Action::Pause)
        ));
        assert!(matches!(
            bindings.action(
                &Screen::Inventory,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            ),
            Some(Action::Back)
        ));
        assert!(matches!(
            bindings.action(
                &Screen::MainMenu,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
            ),
            Some(Action::Quit)
        ));
    }

    #[test]
    fn rejects_bad_keys_actions_and_conflicts() {
        for source in [
            CONFIG.replace("\"ctrl+c\"", "\"banana\""),
            CONFIG.replace("move_up =", "teleport ="),
            CONFIG.replace("[\"right\", \"d\"]", "[\"q\"]"),
            CONFIG.replace("[\"right\", \"d\"]", "[]"),
        ] {
            assert!(Keybinds::parse(&source).is_err());
        }
    }

    #[test]
    fn preview_commands_are_configurable_and_separate_from_game_actions() {
        let source = CONFIG.replace("regenerate = [\"r\"]", "regenerate = [\"f\"]");
        let bindings = Keybinds::parse(&source).unwrap();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(
            bindings.preview_command(key(KeyCode::Char('f'))),
            Some(PreviewCommand::Regenerate)
        );
        assert_eq!(bindings.preview_command(key(KeyCode::Char('r'))), None);
        assert_eq!(
            bindings.preview_command(key(KeyCode::Char('>'))),
            Some(PreviewCommand::NextLevel)
        );
        assert_eq!(
            bindings.preview_command(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(PreviewCommand::Quit)
        );
        assert!(bindings.preview_help().contains("f: regenerate"));
        assert!(
            bindings
                .action(&Screen::Playing, key(KeyCode::Char('f')))
                .is_none()
        );
        assert_eq!(bindings.preview_command(key(KeyCode::Char('w'))), None);
        assert_eq!(
            bindings.preview_command(KeyEvent::new_with_kind(
                KeyCode::Char('f'),
                KeyModifiers::NONE,
                KeyEventKind::Release
            )),
            None
        );
        for source in [
            CONFIG.replace("regenerate = [\"r\"]", "regenerate = [\"q\"]"),
            CONFIG.replace("regenerate = [\"r\"]", "regenerate = [\">\"]"),
            CONFIG.replace("regenerate =", "teleport ="),
        ] {
            assert!(Keybinds::parse(&source).is_err());
        }
    }
}
