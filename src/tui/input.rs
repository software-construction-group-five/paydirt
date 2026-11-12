//! Configurable keyboard interpretation (contract 21), independent of gameplay.

use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::Path,
};

use crate::game::{action::Action, screen::Screen};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::Deserialize;

type Key = (KeyCode, KeyModifiers);

struct Binding {
    action: Action,
    keys: Vec<Key>,
    hint: String,
}

/// Global bindings and screen-specific action mappings.
pub struct Keybinds {
    global: Vec<Binding>,
    screens: [Vec<Binding>; 6],
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
    #[serde(default = "default_help_bindings")]
    help: BTreeMap<String, Vec<String>>,
}

fn default_help_bindings() -> BTreeMap<String, Vec<String>> {
    [
        ("back".into(), vec!["esc".into()]),
        ("confirm".into(), vec!["enter".into()]),
    ]
    .into()
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
        let mut screens = std::array::from_fn(|_| Vec::new());
        for (index, table) in [
            config.main_menu,
            config.playing,
            config.inventory,
            config.paused,
            config.game_over,
            config.help,
        ]
        .into_iter()
        .enumerate()
        {
            screens[index] = bindings(table)?;
            check_keys(
                global
                    .iter()
                    .chain(&screens[index])
                    .flat_map(|binding| &binding.keys),
                &format!("screen {index}"),
            )?;
        }
        Ok(Self { global, screens })
    }

    /// Reads the next terminal event and maps a key press for the active screen
    /// to an action (contract 21).
    ///
    /// Call after terminal setup. Unmapped keys and other events return `None`;
    /// terminal read failures are returned to the caller.
    pub fn read_input(&self, screen: &Screen) -> io::Result<Option<&Action>> {
        match event::read()? {
            Event::Key(key) => Ok(self.action(screen, key)),
            _ => Ok(None),
        }
    }

    /// Interprets a key press for the active screen; unmapped/released keys are ignored.
    pub fn action(&self, screen: &Screen, key: KeyEvent) -> Option<&Action> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        let chord = key_chord(key.code, key.modifiers);
        self.global
            .iter()
            .chain(&self.screens[screen_index(screen)])
            .find(|binding| binding.keys.contains(&chord))
            .map(|binding| &binding.action)
    }

    /// Builds control hints from the same mappings used to interpret input.
    pub fn help(&self, screen: &Screen) -> String {
        self.screens[screen_index(screen)]
            .iter()
            .chain(&self.global)
            .map(|binding| binding.hint.as_str())
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// Builds the controls overlay from actual screen bindings, including remaps.
    pub fn controls(&self, screen: &Screen) -> String {
        let title = match screen {
            Screen::MainMenu => "MAIN MENU",
            Screen::Playing => "PLAYING",
            Screen::Inventory => "INVENTORY",
            Screen::Paused => "PAUSED",
            Screen::GameOver => "GAME OVER",
            Screen::Help => "HELP",
        };
        let controls = self.screens[screen_index(screen)]
            .iter()
            .chain(&self.global)
            .map(|binding| binding.hint.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let closing = self.screens[screen_index(&Screen::Help)]
            .iter()
            .map(|binding| binding.hint.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        format!("{title} CONTROLS\n\n{controls}\n\nClose help\n{closing}")
    }
}

fn screen_index(screen: &Screen) -> usize {
    match screen {
        Screen::MainMenu => 0,
        Screen::Playing => 1,
        Screen::Inventory => 2,
        Screen::Paused => 3,
        Screen::GameOver => 4,
        Screen::Help => 5,
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
                "descend" => (Action::Descend, "descend"),
                "help" => (Action::Help, "help"),
                "inventory" => (Action::OpenInventory, "inventory"),
                "pickup" => (Action::Pickup, "pick up"),
                "use_item" => (Action::UseItem, "use"),
                "equip_item" => (Action::EquipItem, "equip"),
                "unequip_item" => (Action::UnequipItem, "unequip"),
                "drop_item" => (Action::DropItem, "drop"),
                "history_up" => (Action::HistoryUp, "older"),
                "history_down" => (Action::HistoryDown, "newer"),
                "history_page_up" => (Action::HistoryPageUp, "older page"),
                "history_page_down" => (Action::HistoryPageDown, "newer page"),
                "pause" => (Action::Pause, "pause/resume"),
                "confirm" => (Action::Confirm, "confirm"),
                "back" => (Action::Back, "back"),
                "quit" => (Action::Quit, "quit"),
                _ => return Err(invalid(format!("unknown action: {name}"))),
            };
            binding(action, description, labels)
        })
        .collect()
}

fn binding(action: Action, description: &str, labels: Vec<String>) -> io::Result<Binding> {
    if labels.is_empty() {
        return Err(invalid(format!("{description} needs at least one key")));
    }
    let keys = labels
        .iter()
        .map(|label| parse_key(label))
        .collect::<io::Result<_>>()?;
    Ok(Binding {
        action,
        keys,
        hint: format!("{}: {description}", labels.join("/")),
    })
}

fn check_keys<'a>(keys: impl Iterator<Item = &'a Key>, scope: &str) -> io::Result<()> {
    let mut occupied = HashSet::new();
    for key in keys {
        if !occupied.insert(*key) {
            return Err(invalid(format!("duplicate key {key:?} in {scope}")));
        }
    }
    Ok(())
}

// Terminals differ in whether they report Shift for an already shifted character.
fn key_chord(code: KeyCode, modifiers: KeyModifiers) -> Key {
    match code {
        KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => (KeyCode::BackTab, modifiers),
        KeyCode::BackTab => (code, modifiers | KeyModifiers::SHIFT),
        KeyCode::Char(character) if "!@#$%^&*()_+{}|:\"<>?~".contains(character) => {
            (code, modifiers - KeyModifiers::SHIFT)
        }
        KeyCode::Char(character) if character.is_ascii_uppercase() => {
            (code, modifiers | KeyModifiers::SHIFT)
        }
        KeyCode::Char(character) if modifiers.contains(KeyModifiers::SHIFT) => {
            (KeyCode::Char(character.to_ascii_uppercase()), modifiers)
        }
        _ => (code, modifiers),
    }
}

fn parse_key(label: &str) -> io::Result<Key> {
    let mut parts = label.split('+');
    let name = parts.next_back().unwrap_or_default();
    let mut modifiers = KeyModifiers::NONE;
    for modifier in parts {
        modifiers |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return Err(invalid(format!("invalid modifier in key: {label:?}"))),
        };
    }
    let code = match name.to_ascii_lowercase().as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
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
    Ok(key_chord(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_and_shifted_punctuation_match_terminal_variants() {
        let source = include_str!("../../config/keybinds.toml");
        for label in ["X", "shift+x", "!", "plus", "shift+plus", "@", "_"] {
            let bindings = Keybinds::parse(
                &source.replace("wait = [\".\"]", &format!("wait = [\"{label}\"]")),
            )
            .unwrap();
            let character = match label {
                "X" | "shift+x" => 'X',
                "plus" | "shift+plus" => '+',
                _ => label.chars().next().unwrap(),
            };
            for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
                assert_eq!(
                    bindings.action(
                        &Screen::Playing,
                        KeyEvent::new(KeyCode::Char(character), modifiers)
                    ),
                    Some(&Action::Wait),
                    "binding {label:?}, modifiers {modifiers:?}"
                );
            }
            assert!(
                bindings
                    .action(
                        &Screen::Playing,
                        KeyEvent::new(KeyCode::Char(character), KeyModifiers::ALT)
                    )
                    .is_none()
            );
        }
        assert!(
            Keybinds::parse(&source.replace("wait = [\".\"]", "wait = [\"plus\", \"shift+plus\"]"))
                .is_err()
        );
    }

    #[test]
    fn help_opens_with_shifted_or_unshifted_question_mark_and_uses_remapped_controls() {
        let source =
            include_str!("../../config/keybinds.toml").replace("wait = [\".\"]", "wait = [\"x\"]");
        let bindings = Keybinds::parse(&source).unwrap();
        for screen in [
            Screen::MainMenu,
            Screen::Playing,
            Screen::Inventory,
            Screen::Paused,
            Screen::GameOver,
            Screen::Help,
        ] {
            for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
                assert_eq!(
                    bindings.action(&screen, KeyEvent::new(KeyCode::Char('?'), modifiers)),
                    Some(&Action::Help)
                );
            }
        }
        assert_eq!(
            bindings.action(
                &Screen::Help,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            ),
            Some(&Action::Back)
        );
        let controls = bindings.controls(&Screen::Inventory);
        assert!(controls.contains("INVENTORY CONTROLS"));
        assert!(controls.contains("x: wait"));
        assert!(!controls.contains(".: wait"));
        assert!(controls.contains("Close help\nesc: back\nenter: confirm"));
    }

    #[test]
    fn inventory_and_history_controls_are_scoped_and_match_help() {
        let bindings = Keybinds::parse(include_str!("../../config/keybinds.toml")).unwrap();
        for (screen, key, expected) in [
            (Screen::Playing, KeyCode::Char('i'), Action::OpenInventory),
            (Screen::Playing, KeyCode::Char('g'), Action::Pickup),
            (Screen::Playing, KeyCode::Char('.'), Action::Wait),
            (Screen::Inventory, KeyCode::Char('.'), Action::Wait),
            (Screen::Playing, KeyCode::Char('>'), Action::Descend),
            (Screen::Inventory, KeyCode::Char('e'), Action::EquipItem),
            (Screen::Inventory, KeyCode::Char('r'), Action::UnequipItem),
            (Screen::Inventory, KeyCode::Char('u'), Action::UseItem),
            (Screen::Inventory, KeyCode::Char('d'), Action::DropItem),
            (Screen::Inventory, KeyCode::Esc, Action::Back),
            (Screen::GameOver, KeyCode::Up, Action::HistoryUp),
            (Screen::GameOver, KeyCode::Down, Action::HistoryDown),
            (Screen::GameOver, KeyCode::PageUp, Action::HistoryPageUp),
            (Screen::GameOver, KeyCode::PageDown, Action::HistoryPageDown),
        ] {
            assert_eq!(
                bindings.action(&screen, KeyEvent::new(key, KeyModifiers::NONE)),
                Some(&expected)
            );
        }
        assert!(bindings.help(&Screen::Inventory).contains("u: use"));
        assert!(
            bindings
                .help(&Screen::GameOver)
                .contains("pageup: older page")
        );
        assert_eq!(
            bindings.action(
                &Screen::Playing,
                KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)
            ),
            None
        );
    }
    const CONFIG: &str = include_str!("../../config/keybinds.toml");

    #[test]
    fn remapping_changes_actions_and_help_and_preserves_screen_context() {
        let config = CONFIG.replace("[\"up\", \"w\"]", "[\"l\"]");
        let bindings = Keybinds::parse(&config).unwrap();
        assert!(matches!(
            bindings.action(
                &Screen::MainMenu,
                KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)
            ),
            Some(Action::MoveUp)
        ));
        assert!(
            bindings
                .action(
                    &Screen::MainMenu,
                    KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE)
                )
                .is_none()
        );
        assert!(bindings.help(&Screen::MainMenu).contains("l: up"));
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
    fn playing_keys_map_to_movement_and_wait_and_appear_in_help() {
        let bindings = Keybinds::parse(CONFIG).unwrap();
        let press =
            |code| bindings.action(&Screen::Playing, KeyEvent::new(code, KeyModifiers::NONE));
        assert!(matches!(press(KeyCode::Up), Some(Action::MoveUp)));
        assert!(matches!(press(KeyCode::Down), Some(Action::MoveDown)));
        assert!(matches!(press(KeyCode::Left), Some(Action::MoveLeft)));
        assert!(matches!(press(KeyCode::Right), Some(Action::MoveRight)));
        assert!(matches!(press(KeyCode::Char('.')), Some(Action::Wait)));
        let help = bindings.help(&Screen::Playing);
        for hint in ["left", "right", "wait"] {
            assert!(help.contains(hint));
        }
    }

    #[test]
    fn rejects_bad_keys_actions_and_conflicts() {
        for source in [
            CONFIG.replace("\"ctrl+c\"", "\"banana\""),
            CONFIG.replace("move_up =", "teleport ="),
            CONFIG.replace("[\"up\", \"w\"]", "[\"q\"]"),
            CONFIG.replace("[\"up\", \"w\"]", "[]"),
        ] {
            assert!(Keybinds::parse(&source).is_err());
        }
    }

    #[test]
    fn greater_than_matches_with_or_without_shift_and_detects_alias_conflicts() {
        let source = CONFIG.to_owned();
        let bindings = Keybinds::parse(&source).unwrap();
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            assert!(matches!(
                bindings.action(
                    &Screen::Playing,
                    KeyEvent::new(KeyCode::Char('>'), modifiers)
                ),
                Some(Action::Descend)
            ));
        }
        assert!(Keybinds::parse(&source.replace("[\">\"]", "[\">\", \"shift+>\"]")).is_err());
    }

    #[test]
    fn shift_tab_matches_backtab_events_and_detects_alias_conflicts() {
        let source = CONFIG.replace("move_up = [\"up\", \"w\"]", "move_up = [\"shift+tab\"]");
        let bindings = Keybinds::parse(&source).unwrap();
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            assert!(matches!(
                bindings.action(
                    &Screen::MainMenu,
                    KeyEvent::new(KeyCode::BackTab, modifiers)
                ),
                Some(Action::MoveUp)
            ));
        }
        assert!(
            Keybinds::parse(&source.replace("[\"shift+tab\"]", "[\"shift+tab\", \"backtab\"]"))
                .is_err()
        );
        assert!(matches!(
            bindings.action(
                &Screen::MainMenu,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
            ),
            Some(Action::Quit)
        ));
    }
}
