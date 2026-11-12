//! Subsystem: Game Logic & Management.
//!
//! Owns all game state and gameplay rules. Provides SDD subsystem contracts:
//! 1. Game Action Processing
//! 2. Game State Access

pub mod action;
pub mod combat;
pub mod enemy;
pub mod equipment;
pub mod inventory;
pub mod item;
pub mod level;
pub mod message_log;
pub mod player;
pub mod position;
pub mod screen;
pub mod stats;

use action::{Action, TurnResult};
use combat::{attack_damage, equipment_drop};
use enemy::ai::{self, AiProfile, Intent};
use equipment::{Equipment, EquipmentSlot};
use inventory::Inventory;
use level::{EnemyId, Level, generator::LevelGenerator};
use message_log::MessageLog;
use player::{ItemError, Player};
use rand::{SeedableRng, rngs::StdRng};
use screen::Screen;
use stats::{Stats, StatusEffect};

// Initial combat tuning; keep equipment bonuses separate from these base stats.
const STARTING_MAX_HP: i32 = 40;
const STARTING_ATTACK: i32 = 0;
const STARTING_DEFENSE: i32 = 1;
const MESSAGE_LOG_CAPACITY: usize = 50;

/// Stored results of a completed session; retained with its message history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSummary {
    /// Dungeon depth at the moment of death.
    pub final_depth: u32,
    /// Human-readable cause of death.
    pub outcome: String,
}

/// A selectable owned copy, independent of the item's catalog index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventorySelection {
    /// Current carried storage index; rebuilt after transfers.
    Carried(usize),
    /// Current equipment slot.
    Equipped(EquipmentSlot),
}

/// Owns screen navigation, the active dungeon floor, the player, and the message log.
pub struct Game {
    screen: Screen,
    menu_selection: usize,
    level: Option<Level>,
    player: Option<Player>,
    log: MessageLog,
    rng: StdRng,
    should_quit: bool,
    summary: Option<GameSummary>,
    inventory_selection: usize,
    history_offset: usize,
    help_origin: Option<Screen>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    /// Starts at the main menu without generating a level before New Game is chosen.
    pub fn new() -> Self {
        Self {
            screen: Screen::MainMenu,
            menu_selection: 0,
            level: None,
            player: None,
            log: MessageLog::new(MESSAGE_LOG_CAPACITY),
            rng: StdRng::from_rng(&mut rand::rng()),
            should_quit: false,
            summary: None,
            inventory_selection: 0,
            history_offset: 0,
            help_origin: None,
        }
    }

    /// Returns the active screen for rendering and key interpretation.
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Returns the highlighted main-menu row (New Game or Quit).
    pub fn menu_selection(&self) -> usize {
        self.menu_selection
    }

    /// Returns the active level, absent before a session starts.
    pub fn level(&self) -> Option<&Level> {
        self.level.as_ref()
    }

    /// Returns the active player, absent before a session starts (contract 6).
    pub fn player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Returns the current session's messages (Game State Access).
    pub fn message_log(&self) -> &MessageLog {
        &self.log
    }

    /// Borrows the final results, absent during an active or unstarted session.
    pub fn summary(&self) -> Option<&GameSummary> {
        self.summary.as_ref()
    }

    /// Selected inventory row, clamped after any item transfer.
    pub fn inventory_selection(&self) -> usize {
        self.inventory_selection
    }

    /// Lines scrolled back from the newest game-over message.
    pub fn history_offset(&self) -> usize {
        self.history_offset
    }

    /// Screen paused beneath the controls overlay, if help is open.
    pub fn help_origin(&self) -> Option<Screen> {
        self.help_origin
    }

    /// Carried entries followed by occupied weapon/armor slots, borrowing originals.
    pub fn inventory_rows(&self) -> Vec<(InventorySelection, &item::Item)> {
        self.inventory_entries().collect()
    }

    /// Shares selection order between display rows and item actions.
    fn inventory_entries(&self) -> impl Iterator<Item = (InventorySelection, &item::Item)> {
        self.player.iter().flat_map(|player| {
            let carried = player
                .get_inventory()
                .items()
                .iter()
                .enumerate()
                .map(|(index, item)| (InventorySelection::Carried(index), item));
            let equipped = [EquipmentSlot::Weapon, EquipmentSlot::Armor]
                .into_iter()
                .filter_map(move |slot| {
                    player
                        .get_equipped_items()
                        .item(slot)
                        .map(|item| (InventorySelection::Equipped(slot), item))
                });
            carried.chain(equipped)
        })
    }

    /// Reports whether the application loop should exit and restore the terminal.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Processes an action and advances enemies and effects once if a turn was
    /// spent. Accepted descent ticks only player effects before replacing the floor.
    /// Menu navigation, pause, and redraws do not advance combat.
    pub fn process_action(&mut self, action: &Action) -> TurnResult {
        if self.should_quit {
            return TurnResult::NotSpent;
        }
        if matches!(action, Action::Quit) {
            self.should_quit = true;
            return TurnResult::NotSpent;
        }
        if matches!(action, Action::Help) {
            if matches!(self.screen, Screen::Help) {
                self.close_help();
            } else {
                self.help_origin = Some(self.screen);
                self.screen = Screen::Help;
            }
            return TurnResult::NotSpent;
        }

        let result = match self.screen {
            Screen::MainMenu => {
                match action {
                    Action::MoveUp | Action::MoveDown => {
                        self.menu_selection = 1 - self.menu_selection
                    }
                    Action::Confirm if self.menu_selection == 0 => self.initialize_game(),
                    Action::Confirm => self.should_quit = true,
                    _ => {}
                }
                TurnResult::NotSpent
            }
            Screen::Playing => match action {
                Action::Pause => {
                    self.screen = Screen::Paused;
                    TurnResult::NotSpent
                }
                _ if self.level.is_none()
                    || !self
                        .player
                        .as_ref()
                        .is_some_and(|player| !player.get_stats().is_dead()) =>
                {
                    TurnResult::NotSpent
                }
                Action::MoveUp => self.move_player(0, -1),
                Action::MoveDown => self.move_player(0, 1),
                Action::MoveLeft => self.move_player(-1, 0),
                Action::MoveRight => self.move_player(1, 0),
                Action::Descend => {
                    if self.descend() {
                        return TurnResult::Spent;
                    }
                    TurnResult::Spent
                }
                Action::Wait => TurnResult::Spent,
                Action::OpenInventory => {
                    self.screen = Screen::Inventory;
                    self.inventory_selection = 0;
                    TurnResult::NotSpent
                }
                Action::Pickup => self.item_action(action),
                _ => TurnResult::NotSpent,
            },
            Screen::Inventory => match action {
                Action::Wait
                    if self
                        .player
                        .as_ref()
                        .is_some_and(|player| !player.get_stats().is_dead()) =>
                {
                    TurnResult::Spent
                }
                Action::Back | Action::OpenInventory => {
                    self.screen = Screen::Playing;
                    TurnResult::NotSpent
                }
                Action::MoveUp => {
                    self.inventory_selection = self.inventory_selection.saturating_sub(1);
                    TurnResult::NotSpent
                }
                Action::MoveDown => {
                    let last = self.inventory_len().saturating_sub(1);
                    self.inventory_selection = self.inventory_selection.saturating_add(1).min(last);
                    TurnResult::NotSpent
                }
                Action::Pickup
                | Action::UseItem
                | Action::EquipItem
                | Action::UnequipItem
                | Action::DropItem => self.item_action(action),
                _ => TurnResult::NotSpent,
            },
            Screen::Paused => {
                match action {
                    Action::Pause | Action::Confirm => self.screen = Screen::Playing,
                    Action::Back => self.return_to_menu(),
                    _ => {}
                }
                TurnResult::NotSpent
            }
            Screen::Help => {
                if matches!(action, Action::Back | Action::Confirm) {
                    self.close_help();
                }
                TurnResult::NotSpent
            }
            Screen::GameOver => {
                let last = self.log.history_lines().saturating_sub(1);
                match action {
                    Action::Confirm | Action::Back => self.return_to_menu(),
                    Action::HistoryUp => {
                        self.history_offset = self.history_offset.saturating_add(1).min(last)
                    }
                    Action::HistoryDown => {
                        self.history_offset = self.history_offset.saturating_sub(1)
                    }
                    Action::HistoryPageUp => {
                        self.history_offset = self.history_offset.saturating_add(10).min(last)
                    }
                    Action::HistoryPageDown => {
                        self.history_offset = self.history_offset.saturating_sub(10)
                    }
                    _ => {}
                }
                TurnResult::NotSpent
            }
        };

        if result == TurnResult::Spent {
            self.advance_turn();
        }
        result
    }

    fn close_help(&mut self) {
        self.screen = self.help_origin.take().unwrap_or(Screen::MainMenu);
    }

    /// Counts selectable rows without allocating display entries.
    fn inventory_len(&self) -> usize {
        self.player.as_ref().map_or(0, |player| {
            let equipment = player.get_equipped_items();
            player.get_inventory().items().len()
                + usize::from(equipment.weapon.is_some())
                + usize::from(equipment.armor.is_some())
        })
    }

    /// Performs one item attempt; success and rejection share the combat boundary.
    fn item_action(&mut self, action: &Action) -> TurnResult {
        if !self
            .player
            .as_ref()
            .is_some_and(|player| !player.get_stats().is_dead())
        {
            return TurnResult::NotSpent;
        }
        if matches!(action, Action::Pickup) {
            self.pick_up_item();
        } else {
            let selected = self
                .inventory_entries()
                .nth(self.inventory_selection)
                .map(|(selection, item)| (selection, item.name.clone()));
            let message = match (action, selected) {
                (Action::EquipItem, Some((InventorySelection::Carried(index), name))) => self
                    .player
                    .as_mut()
                    .unwrap()
                    .equip_carried(index)
                    .map(|_| format!("You equip {name}.")),
                (Action::UnequipItem, Some((InventorySelection::Equipped(slot), name))) => self
                    .player
                    .as_mut()
                    .unwrap()
                    .unequip(slot)
                    .map(|_| format!("You unequip {name}.")),
                (Action::UseItem, Some((InventorySelection::Carried(index), name))) => self
                    .player
                    .as_mut()
                    .unwrap()
                    .use_carried(index)
                    .map(|healed| format!("You use {name} and restore {healed} HP.")),
                (Action::DropItem, Some((selection, name))) => self
                    .drop_item(selection)
                    .map(|_| format!("You drop {name}.")),
                (_, None) => Err(ItemError::Missing),
                _ => Err(ItemError::WrongCategory),
            };
            self.log
                .add_message(message.unwrap_or_else(|error| error.to_string()));
        }
        self.inventory_selection = self
            .inventory_selection
            .min(self.inventory_len().saturating_sub(1));
        TurnResult::Spent
    }

    /// Takes the first ground entry at the player's tile after checking capacity.
    fn pick_up_item(&mut self) {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_mut()) else {
            return;
        };
        let Some((index, entry)) = level.ground_items_at(player.get_position()).next() else {
            self.log
                .add_message("There is nothing here to pick up.".to_owned());
            return;
        };
        if !player.get_inventory().can_accept(entry.item.kind) {
            self.log.add_message(ItemError::Full.to_string());
            return;
        }
        let entry = level
            .remove_ground_item(index)
            .expect("selected ground entry exists");
        let message = format!("You pick up {}.", entry.item.name);
        player
            .receive_item(entry.item)
            .expect("carried capacity was checked");
        self.log.add_message(message);
    }

    /// Transfers a selected original item directly onto the validated player tile.
    fn drop_item(&mut self, selection: InventorySelection) -> Result<(), ItemError> {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_mut()) else {
            return Err(ItemError::Missing);
        };
        let position = player.get_position();
        if !level.is_walkable(position) {
            return Err(ItemError::WrongCategory);
        }
        let item = match selection {
            InventorySelection::Carried(index) => player.take_carried(index),
            InventorySelection::Equipped(slot) => player.take_equipped(slot),
        }
        .ok_or(ItemError::Missing)?;
        level
            .insert_ground_item(item, position)
            .expect("player drop tile was validated");
        Ok(())
    }

    /// Attempts a floor transition. Accepted stairs spend a turn with only player
    /// effects; rejected attempts use the ordinary combat boundary.
    fn descend(&mut self) -> bool {
        let (Some(player), Some(level)) = (self.player.as_ref(), self.level.as_ref()) else {
            return false;
        };
        if player.get_position() != level.stairs_down() {
            self.log.add_message("There are no stairs here.".to_owned());
            return false;
        }
        let Some(depth) = level.depth().checked_add(1) else {
            self.log
                .add_message("You cannot descend any farther.".to_owned());
            return false;
        };
        if self.advance_player_effects() {
            return true;
        }
        let mut level = LevelGenerator::generate_level(depth, &mut self.rng);
        let start = level.player_start();
        self.player
            .as_mut()
            .expect("active player survives descent")
            .set_position(start);
        level.update_visibility(start);
        self.level = Some(level);
        self.log
            .add_message(format!("You descend to depth {depth}."));
        true
    }

    /// Ordinary spent-turn boundary: enemies, player effects, then enemy effects.
    fn advance_turn(&mut self) {
        if self.finish_if_dead("You died.".to_owned()) {
            return;
        }
        self.advance_enemy_turns();
        if matches!(self.screen, Screen::GameOver) {
            return;
        }
        self.advance_effects();
    }

    /// Applies one decision per living enemy in stable insertion-ID order.
    fn advance_enemy_turns(&mut self) {
        let Some(player_pos) = self
            .player
            .as_ref()
            .filter(|player| !player.get_stats().is_dead())
            .map(Player::get_position)
        else {
            return;
        };
        let Some(level) = self.level.as_ref() else {
            return;
        };

        let enemy_ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in enemy_ids {
            let Some(level) = self.level.as_ref() else {
                return;
            };
            let Some(enemy) = level.enemy(id).filter(|enemy| !enemy.stats().is_dead()) else {
                continue;
            };
            let origin = *enemy.position();
            let profile = AiProfile::for_enemy(enemy);
            let can_see_player = origin.distance(player_pos) <= profile.detection_range
                && level.get_map().has_line_of_sight(origin, player_pos);
            let (state, intent) =
                ai::decide(enemy, player_pos, profile, can_see_player, |target| {
                    target != player_pos
                        && level.is_walkable(target)
                        && level.enemy_at(target).is_none()
                });
            let level = self
                .level
                .as_mut()
                .expect("active floor remains during enemy phase");
            level.set_enemy_ai_state(id, state);
            match intent {
                Intent::Move { target } => {
                    // Game enforces actor collision and one-step movement;
                    // Level revalidates terrain and enemy occupancy.
                    if target != player_pos && origin.is_cardinal_neighbor(target) {
                        level.move_enemy(id, target);
                    }
                }
                Intent::Attack => {
                    self.enemy_attack(id);
                    if matches!(self.screen, Screen::GameOver) {
                        return;
                    }
                }
                Intent::Wait => {}
            }
        }
    }

    /// Revalidates an enemy attack and stops the session immediately on death.
    fn enemy_attack(&mut self, id: EnemyId) {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_ref()) else {
            return;
        };
        let Some(enemy) = level.enemy(id) else {
            return;
        };
        if player.get_stats().is_dead()
            || enemy.stats().is_dead()
            || !player
                .get_position()
                .is_cardinal_neighbor(*enemy.position())
        {
            return;
        }
        let kind = *enemy.kind();
        let damage = attack_damage(enemy.get_roll(&mut self.rng), player.effective_defense());
        let old_hp = player.get_stats().hp();
        player.update_health(-(damage.min(i32::MAX as u32) as i32), None);
        let lost = old_hp - player.get_stats().hp();
        self.log
            .add_message(format!("{kind} hits you for {lost} damage."));
        self.finish_if_dead(format!("You were defeated by {kind}."));
    }

    /// Ticks each survivor once. A player effect death skips all enemy effects.
    fn advance_effects(&mut self) {
        if self.advance_player_effects() {
            return;
        }

        self.advance_enemy_effects();
    }

    /// Ticks player effects and returns whether they ended the session.
    fn advance_player_effects(&mut self) -> bool {
        let Some(player) = self.player.as_mut() else {
            return false;
        };
        let effects = player.get_stats().status_effects().to_vec();
        let lost = player.tick_status_effects();
        self.log_effect_tick("You", &effects, lost);
        let names = effects
            .iter()
            .map(StatusEffect::name)
            .collect::<Vec<_>>()
            .join(", ");
        self.finish_if_dead(format!("You died from status effects: {names}."))
    }

    /// Ticks surviving enemies on the current floor in stable ID order.
    fn advance_enemy_effects(&mut self) {
        let Some(level) = self.level.as_ref() else {
            return;
        };
        let enemy_ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in enemy_ids {
            let Some(level) = self.level.as_mut() else {
                return;
            };
            let Some(enemy) = level.enemy(id).filter(|enemy| !enemy.stats().is_dead()) else {
                continue;
            };
            let kind = enemy.kind().to_string();
            let effects = enemy.stats().status_effects().to_vec();
            let Some(outcome) = level.tick_enemy_effects(id) else {
                continue;
            };
            self.log_effect_tick(&kind, &effects, outcome.hp_lost);
            if outcome.is_dead {
                self.defeat_enemy(id);
            }
        }
    }

    /// Logs aggregate effect damage and each independently expiring effect.
    fn log_effect_tick(&mut self, actor: &str, effects: &[StatusEffect], lost: i32) {
        if lost > 0 {
            let names = effects
                .iter()
                .map(StatusEffect::name)
                .collect::<Vec<_>>()
                .join(", ");
            let verb = if actor == "You" { "take" } else { "takes" };
            self.log.add_message(format!(
                "{actor} {verb} {lost} damage from status effects ({names})."
            ));
        }
        for effect in effects
            .iter()
            .filter(|effect| effect.turns_remaining() == 1)
        {
            self.log
                .add_message(format!("{actor}: {} expires.", effect.name()));
        }
    }

    /// Records results once and preserves the log until the session is cleared.
    fn finish_if_dead(&mut self, outcome: String) -> bool {
        if !self
            .player
            .as_ref()
            .is_some_and(|player| player.get_stats().is_dead())
        {
            return false;
        }
        if self.summary.is_none() {
            self.history_offset = 0;
            self.help_origin = None;
            let final_depth = self.level.as_ref().map_or(0, Level::depth);
            self.log.add_message(outcome.clone());
            self.summary = Some(GameSummary {
                final_depth,
                outcome,
            });
        }
        self.screen = Screen::GameOver;
        true
    }

    /// Starts a fresh session: new floor, new player at its start, empty log (contract 1).
    fn initialize_game(&mut self) {
        let mut level = LevelGenerator::generate_level(0, &mut self.rng);
        level.update_visibility(level.player_start());
        let stats = Stats::new(STARTING_MAX_HP, STARTING_ATTACK, STARTING_DEFENSE);
        let mut equipment = Equipment::new();
        equipment
            .equip(item::get_item(0))
            .expect("starting pickaxe is a weapon");
        self.player = Some(Player::new(
            level.player_start(),
            stats,
            Inventory::new(),
            equipment,
        ));
        self.level = Some(level);
        self.log.clear();
        self.summary = None;
        self.inventory_selection = 0;
        self.history_offset = 0;
        self.help_origin = None;
        self.screen = Screen::Playing;
    }

    /// Moves the player one tile if the target is on the map and walkable;
    /// otherwise the player stays put (Game Contract 2, using Level Contract 9
    /// and Player Contract 7). Moving into a closed door opens it instead, and
    /// the player stays put. Bumping an enemy attacks without moving.
    /// Updates line of sight after a move. Always spends the turn, even when blocked.
    fn move_player(&mut self, dx: isize, dy: isize) -> TurnResult {
        let (Some(player), Some(level)) = (self.player.as_mut(), self.level.as_mut()) else {
            return TurnResult::Spent;
        };
        let Some(target) = player.get_position().offset(dx, dy) else {
            return TurnResult::Spent;
        };
        // Bumping a closed door opens it; walking through takes another turn
        if level.open_door(target) {
            // An open door no longer blocks sight, so refresh what the player sees
            level.update_visibility(player.get_position());
            return TurnResult::Spent;
        }
        if !level.is_walkable(target) {
            return TurnResult::Spent;
        }
        if let Some(id) = level.enemy_at(target) {
            self.attack_enemy(id);
            return TurnResult::Spent;
        }
        player.set_position(target);
        level.update_visibility(target);
        TurnResult::Spent
    }

    /// Resolves a validated cardinal bump attack without advancing other actors.
    fn attack_enemy(&mut self, id: EnemyId) {
        let (Some(player), Some(level)) = (self.player.as_ref(), self.level.as_mut()) else {
            return;
        };
        let Some(enemy) = level.enemy(id) else {
            return;
        };
        if player.get_stats().is_dead()
            || !player
                .get_position()
                .is_cardinal_neighbor(*enemy.position())
        {
            return;
        }
        if enemy.stats().is_dead() {
            self.defeat_enemy(id);
            return;
        }
        let defender = enemy.kind().to_string();
        let damage = attack_damage(player.roll_attack(&mut self.rng), enemy.stats().defense());
        let effect = player.weapon_effect();
        let effect_name = effect.as_ref().map(|effect| effect.name().to_owned());
        let Some(outcome) = level.damage_enemy(id, damage, effect) else {
            return;
        };
        self.log.add_message(format!(
            "You hit {defender} for {} damage.",
            outcome.hp_lost
        ));
        if outcome.effect_applied
            && let Some(name) = effect_name
        {
            self.log
                .add_message(format!("You apply {name} to {defender}."));
        }
        if outcome.is_dead {
            self.defeat_enemy(id);
        }
    }

    /// Removes a defeated enemy once and resolves its equipment drop.
    /// Both direct attacks and effect deaths use this path.
    fn defeat_enemy(&mut self, id: EnemyId) {
        let Some(level) = self.level.as_mut() else {
            return;
        };
        let Some(enemy) = level.enemy(id) else {
            return;
        };
        if !enemy.stats().is_dead() {
            return;
        }
        let position = *enemy.position();
        // Level insertion and movement guarantee walkable death tiles. Check
        // before removing ownership so an invariant failure cannot lose loot.
        assert!(
            level.is_walkable(position),
            "enemy death tile must be walkable"
        );
        let Some(enemy) = level.remove_enemy(id) else {
            return;
        };
        self.log
            .add_message(format!("You defeat {}.", enemy.kind()));
        if let Some(item) = equipment_drop(level.depth(), item::catalog(), &mut self.rng) {
            let message = format!("{} drops {}.", enemy.kind(), item.name);
            level
                .insert_ground_item(item, position)
                .expect("validated enemy death tile accepts ground items");
            self.log.add_message(message);
        }
    }

    fn return_to_menu(&mut self) {
        self.screen = Screen::MainMenu;
        self.menu_selection = 0;
        self.level = None;
        self.player = None;
        self.log.clear();
        self.summary = None;
        self.inventory_selection = 0;
        self.history_offset = 0;
        self.help_origin = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descent_preserves_player_items_and_ticks_only_player_effects() {
        let (mut game, _, _) = combat_game(0, 24, None);
        let stairs = game.level().unwrap().stairs_down();
        let player = game.player.as_mut().unwrap();
        player.set_position(stairs);
        player.receive_item(item::get_item(2)).unwrap();
        player.update_health(-10, Some(StatusEffect::new("Poison".into(), 2, 3)));
        let hp = player.get_stats().hp();
        let equipment = player.get_equipped_items().clone();
        let inventory = player.get_inventory().items().to_vec();
        // An adjacent enemy must never get an action during descent.
        let ids: Vec<_> = game.level().unwrap().enemies().map(|(id, _)| id).collect();
        for id in ids {
            game.level.as_mut().unwrap().remove_enemy(id);
        }
        let adjacent = game
            .level()
            .unwrap()
            .get_map()
            .neighbors(stairs)
            .find(|&p| game.level().unwrap().is_walkable(p))
            .unwrap();
        game.level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, adjacent))
            .ok()
            .unwrap();
        game.rng = StdRng::seed_from_u64(901);
        let mut expected_rng = StdRng::seed_from_u64(901);
        let expected = LevelGenerator::generate_level(1, &mut expected_rng);
        assert_eq!(game.process_action(&Action::Descend), TurnResult::Spent);
        let player = game.player().unwrap();
        let level = game.level().unwrap();
        assert_eq!(level.depth(), 1);
        assert_eq!(player.get_position(), expected.player_start());
        assert_eq!(player.get_stats().hp(), hp - 2);
        assert_eq!(player.get_stats().status_effects()[0].turns_remaining(), 2);
        assert_eq!(player.get_equipped_items(), &equipment);
        assert_eq!(player.get_inventory().items(), inventory);
        assert!(level.get_map().is_visible(player.get_position()));
        assert!(!game.log.history().iter().any(|m| m.contains("hits you")));
        use rand::RngExt;
        assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
    }

    #[test]
    fn lethal_descent_effect_keeps_old_floor_and_skips_enemy_effects_and_rng() {
        let (mut game, id, _) = combat_game(0, 24, None);
        let stairs = game.level().unwrap().stairs_down();
        game.player.as_mut().unwrap().set_position(stairs);
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Poison".into(), i32::MAX, 1)));
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Burn".into(), 1, 2)),
        );
        game.rng = StdRng::seed_from_u64(901);
        let mut expected_rng = StdRng::seed_from_u64(901);
        assert_eq!(game.process_action(&Action::Descend), TurnResult::Spent);
        assert_eq!(*game.screen(), Screen::GameOver);
        assert_eq!(game.summary().unwrap().final_depth, 0);
        assert_eq!(game.level().unwrap().depth(), 0);
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            2
        );
        assert_eq!(game.process_action(&Action::Descend), TurnResult::NotSpent);
        use rand::RngExt;
        assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
    }

    #[test]
    fn rejected_descent_runs_one_normal_turn() {
        let (mut game, _, _) = combat_game(0, 24, None);
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 2)));
        assert_eq!(game.process_action(&Action::Descend), TurnResult::Spent);
        assert_eq!(game.level().unwrap().depth(), 0);
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            1
        );
        assert_eq!(
            game.log
                .history()
                .iter()
                .filter(|m| m.contains("hits you"))
                .count(),
            1
        );
    }

    #[test]
    fn maximum_depth_rejects_descent_without_overflow() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.level = Some(LevelGenerator::generate_level(u32::MAX, &mut game.rng));
        let stairs = game.level().unwrap().stairs_down();
        game.player.as_mut().unwrap().set_position(stairs);
        assert_eq!(game.process_action(&Action::Descend), TurnResult::Spent);
        assert_eq!(game.level().unwrap().depth(), u32::MAX);
        assert!(game.log.history().iter().any(|m| m.contains("farther")));
    }

    #[test]
    fn stepping_onto_stairs_waits_for_explicit_descent() {
        let (mut game, id, _) = combat_game(0, 24, None);
        game.level.as_mut().unwrap().remove_enemy(id);
        let stairs = game.level().unwrap().stairs_down();
        let (position, action) = [
            (-1, 0, Action::MoveRight),
            (1, 0, Action::MoveLeft),
            (0, -1, Action::MoveDown),
            (0, 1, Action::MoveUp),
        ]
        .into_iter()
        .find_map(|(dx, dy, action)| {
            stairs
                .offset(dx, dy)
                .filter(|&p| game.level().unwrap().is_walkable(p))
                .map(|p| (p, action))
        })
        .unwrap();
        game.player.as_mut().unwrap().set_position(position);
        game.process_action(&action);
        assert_eq!(game.player().unwrap().get_position(), stairs);
        assert_eq!(game.level().unwrap().depth(), 0);
        game.process_action(&Action::Pause);
        assert_eq!(game.process_action(&Action::Descend), TurnResult::NotSpent);
        game.process_action(&Action::Confirm);
        game.process_action(&Action::Descend);
        assert_eq!(game.level().unwrap().depth(), 1);
    }

    #[test]
    fn help_freezes_combat_and_restores_play_inventory_or_pause() {
        use rand::RngExt;
        for origin in [Screen::Playing, Screen::Inventory, Screen::Paused] {
            for close in [Action::Help, Action::Back, Action::Confirm] {
                let (mut game, id, _) = combat_game(0, 24, None);
                game.player
                    .as_mut()
                    .unwrap()
                    .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 2)));
                game.level.as_mut().unwrap().damage_enemy(
                    id,
                    0,
                    Some(StatusEffect::new("Timer".into(), 0, 2)),
                );
                match origin {
                    Screen::Inventory => {
                        game.process_action(&Action::OpenInventory);
                    }
                    Screen::Paused => {
                        game.process_action(&Action::Pause);
                    }
                    _ => {}
                }
                let position = game.player().unwrap().get_position();
                let enemy_position = *game.level().unwrap().enemy(id).unwrap().position();
                game.rng = StdRng::seed_from_u64(7);
                assert_eq!(game.process_action(&Action::Help), TurnResult::NotSpent);
                assert_eq!(*game.screen(), Screen::Help);
                assert_eq!(game.help_origin(), Some(origin));
                for action in [
                    Action::Wait,
                    Action::MoveUp,
                    Action::UseItem,
                    Action::Pickup,
                    Action::Pause,
                    Action::HistoryUp,
                ] {
                    assert_eq!(game.process_action(&action), TurnResult::NotSpent);
                }
                assert_eq!(game.process_action(&close), TurnResult::NotSpent);
                assert_eq!(*game.screen(), origin);
                assert_eq!(game.help_origin(), None);
                assert_eq!(game.player().unwrap().get_position(), position);
                assert_eq!(game.player().unwrap().get_stats().hp(), 200);
                assert_eq!(
                    game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
                    2
                );
                assert_eq!(
                    *game.level().unwrap().enemy(id).unwrap().position(),
                    enemy_position
                );
                assert_eq!(
                    game.level()
                        .unwrap()
                        .enemy(id)
                        .unwrap()
                        .stats()
                        .status_effects()[0]
                        .turns_remaining(),
                    2
                );
                assert!(game.log.history().is_empty());
                assert_eq!(
                    game.rng.random::<u64>(),
                    StdRng::seed_from_u64(7).random::<u64>()
                );
            }
        }
        let mut game = Game::new();
        game.process_action(&Action::MoveDown);
        game.process_action(&Action::Help);
        game.process_action(&Action::Confirm);
        assert_eq!(*game.screen(), Screen::MainMenu);
        assert_eq!(game.menu_selection(), 1);
        assert!(game.player().is_none());
    }

    #[test]
    fn inventory_wait_spends_a_turn_without_moving_or_changing_items() {
        let (mut game, _, _) = combat_game(0, 24, None);
        let position = game.player().unwrap().get_position();
        let equipment = game.player().unwrap().get_equipped_items().clone();
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 2)));
        game.process_action(&Action::OpenInventory);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let player = game.player().unwrap();
        assert_eq!(player.get_position(), position);
        assert_eq!(player.get_equipped_items(), &equipment);
        assert!(player.get_inventory().items().is_empty());
        assert_eq!(player.get_stats().status_effects()[0].turns_remaining(), 1);
        assert_eq!(
            game.message_log()
                .history()
                .iter()
                .filter(|message| message.contains("hits you"))
                .count(),
            1
        );
    }

    #[test]
    fn inventory_transfers_originals_and_each_item_action_spends_one_turn() {
        let (mut game, enemy, _) = combat_game(0, 24, None);
        game.level.as_mut().unwrap().remove_enemy(enemy);
        let position = game.player().unwrap().get_position();
        let custom = item::get_item(0).with_combat(item::CombatModifiers::weapon(
            combat::DamageDice::new(1, combat::Die::D12),
            Some(stats::EffectSpec::new("Burning".into(), 1, 2)),
        ));
        game.level
            .as_mut()
            .unwrap()
            .insert_ground_item(custom.clone(), position)
            .unwrap();
        game.level
            .as_mut()
            .unwrap()
            .insert_ground_item(item::get_item(2), position)
            .unwrap();
        game.player
            .as_mut()
            .unwrap()
            .update_health(-20, Some(StatusEffect::new("Timer".into(), 0, 20)));
        assert_eq!(game.process_action(&Action::Pickup), TurnResult::Spent);
        assert_eq!(game.process_action(&Action::Pickup), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_inventory().items()[0], custom);
        assert_eq!(
            game.process_action(&Action::OpenInventory),
            TurnResult::NotSpent
        );
        assert_eq!(game.process_action(&Action::EquipItem), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().get_equipped_items().weapon,
            Some(custom.clone())
        );
        assert_eq!(
            game.player().unwrap().weapon_effect().unwrap().name(),
            "Burning"
        );
        assert_eq!(game.process_action(&Action::UseItem), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_stats().hp(), 192);
        assert_eq!(game.process_action(&Action::MoveDown), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::UnequipItem), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().damage_dice(),
            combat::DamageDice::new(1, combat::Die::D4)
        );
        assert!(game.player().unwrap().weapon_effect().is_none());
        assert_eq!(game.process_action(&Action::DropItem), TurnResult::Spent);
        assert_eq!(game.level().unwrap().ground_items()[0].item, custom);
        assert_eq!(game.level().unwrap().ground_items()[0].position, position);
        assert_eq!(game.inventory_selection(), 0);
        assert_eq!(game.process_action(&Action::EquipItem), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().damage_dice(),
            combat::DamageDice::new(2, combat::Die::D4)
        );
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            13
        );
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::Playing));
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            13
        );
    }

    #[test]
    fn capacity_and_full_health_failures_keep_items_and_advance_combat_in_inventory() {
        let (mut game, id, _) = combat_game(0, 24, None);
        let position = game.player().unwrap().get_position();
        game.player
            .as_mut()
            .unwrap()
            .receive_item(item::get_item(1))
            .unwrap();
        game.level
            .as_mut()
            .unwrap()
            .insert_ground_item(item::get_item(1), position)
            .unwrap();
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 5)));
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Timer".into(), 0, 5)),
        );
        assert_eq!(game.process_action(&Action::Pickup), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().get_inventory().items(),
            &[item::get_item(1)]
        );
        assert_eq!(
            game.level().unwrap().ground_items()[0].item,
            item::get_item(1)
        );
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            4
        );
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            4
        );
        game.process_action(&Action::OpenInventory);
        let hp = game.player().unwrap().get_stats().hp();
        assert_eq!(game.process_action(&Action::UseItem), TurnResult::Spent);
        assert!(game.player().unwrap().get_stats().hp() < hp);
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            3
        );
        // A full-health use fails before the enemy attacks and keeps the potion.
        game.player.as_mut().unwrap().update_health(200, None);
        game.player
            .as_mut()
            .unwrap()
            .receive_item(item::get_item(2))
            .unwrap();
        game.inventory_selection = 1;
        assert_eq!(game.process_action(&Action::UseItem), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().get_inventory().items()[1],
            item::get_item(2)
        );
        assert!(
            game.message_log()
                .history()
                .iter()
                .any(|line| line.contains("potion is kept"))
        );
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            2
        );
    }

    #[test]
    fn empty_pack_actions_fail_safely_and_equipped_items_can_be_dropped_and_recovered() {
        let (mut game, id, _) = combat_game(0, 24, None);
        game.level.as_mut().unwrap().remove_enemy(id);
        game.process_action(&Action::OpenInventory);
        assert_eq!(game.process_action(&Action::DropItem), TurnResult::Spent);
        assert!(game.inventory_rows().is_empty());
        assert_eq!(game.inventory_selection(), 0);
        for action in [
            Action::UseItem,
            Action::EquipItem,
            Action::UnequipItem,
            Action::DropItem,
        ] {
            assert_eq!(game.process_action(&action), TurnResult::Spent);
        }
        assert_eq!(game.process_action(&Action::Pickup), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().get_inventory().items(),
            &[item::get_item(0)]
        );
        assert_eq!(game.process_action(&Action::EquipItem), TurnResult::Spent);
        assert_eq!(
            game.player().unwrap().get_equipped_items().weapon,
            Some(item::get_item(0))
        );
    }

    #[test]
    fn item_action_death_enters_game_over_and_keeps_the_result_message() {
        let (mut game, _, _) = combat_game(0, 24, None);
        game.player.as_mut().unwrap().update_health(-199, None);
        game.process_action(&Action::OpenInventory);
        assert_eq!(game.process_action(&Action::UseItem), TurnResult::Spent);
        assert!(matches!(game.screen(), Screen::GameOver));
        let log = game.message_log().history();
        assert!(log[0].contains("cannot be used"));
        assert!(log[1].contains("hits you for 1"));
        assert!(game.summary().is_some());
    }

    #[test]
    fn game_over_history_scrolls_full_multiline_history_without_turns_and_resets() {
        use rand::RngExt;
        let (mut game, _, _) = combat_game(0, 24, None);
        game.log
            .add_message("oldest first line\noldest second line".into());
        for index in 0..60 {
            game.log.add_message(format!("Message {index}"));
        }
        game.player.as_mut().unwrap().update_health(-199, None);
        game.process_action(&Action::Wait);
        let history = game.log.history().to_vec();
        let summary = game.summary().unwrap().clone();
        assert!(
            !game
                .log
                .messages()
                .iter()
                .any(|line| line.contains("oldest"))
        );
        assert_eq!(game.history_offset(), 0);
        game.rng = StdRng::seed_from_u64(7);
        assert_eq!(
            game.process_action(&Action::HistoryPageUp),
            TurnResult::NotSpent
        );
        assert_eq!(game.history_offset(), 10);
        game.process_action(&Action::Help);
        game.process_action(&Action::HistoryPageUp);
        assert_eq!(game.history_offset(), 10);
        game.process_action(&Action::Back);
        assert_eq!(*game.screen(), Screen::GameOver);
        assert_eq!(game.history_offset(), 10);
        for _ in 0..100 {
            game.process_action(&Action::HistoryUp);
        }
        assert_eq!(game.history_offset(), 63);
        game.process_action(&Action::HistoryDown);
        assert_eq!(game.history_offset(), 62);
        for _ in 0..10 {
            game.process_action(&Action::HistoryPageDown);
        }
        assert_eq!(game.history_offset(), 0);
        assert_eq!(game.summary(), Some(&summary));
        assert_eq!(game.log.history(), history);
        assert_eq!(game.player().unwrap().get_stats().hp(), 0);
        assert_eq!(
            game.rng.random::<u64>(),
            StdRng::seed_from_u64(7).random::<u64>()
        );
        game.process_action(&Action::HistoryUp);
        game.process_action(&Action::Confirm);
        assert_eq!(game.history_offset(), 0);
        assert_eq!(game.inventory_selection(), 0);
        assert!(game.log.history().is_empty());
        game.process_action(&Action::Confirm);
        assert!(game.player().unwrap().get_inventory().items().is_empty());
        assert!(game.summary().is_none());
    }

    fn another_adjacent_enemy(game: &mut Game, kind: enemy::EnemyType) -> EnemyId {
        let start = game.player().unwrap().get_position();
        let level = game.level.as_mut().unwrap();
        let position = level
            .get_map()
            .neighbors(start)
            .find(|&position| level.is_walkable(position) && level.enemy_at(position).is_none())
            .unwrap();
        level
            .insert_enemy(enemy::Enemy::new(kind, position))
            .ok()
            .unwrap()
    }

    #[test]
    fn lethal_enemy_attack_stops_later_enemies_and_all_effect_ticks() {
        use rand::RngExt;
        let (mut game, first, _) = combat_game(0, 24, None);
        let second = another_adjacent_enemy(&mut game, enemy::EnemyType::Slime);
        game.player
            .as_mut()
            .unwrap()
            .update_health(-199, Some(StatusEffect::new("Poison".into(), 1, 2)));
        game.level.as_mut().unwrap().damage_enemy(
            second,
            0,
            Some(StatusEffect::new("Burning".into(), 1, 2)),
        );
        let mut expected_rng = StdRng::seed_from_u64(7);
        game.level()
            .unwrap()
            .enemy(first)
            .unwrap()
            .get_roll(&mut expected_rng);
        game.rng = StdRng::seed_from_u64(7);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        assert!(matches!(game.screen(), Screen::GameOver));
        assert_eq!(game.player().unwrap().get_stats().hp(), 0);
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            2
        );
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(second)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            2
        );
        assert_eq!(
            game.message_log().history(),
            [
                "Goblin hits you for 1 damage.",
                "You were defeated by Goblin."
            ]
        );
        assert_eq!(
            game.summary().unwrap().final_depth,
            game.level().unwrap().depth()
        );
        assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
        let summary = game.summary().unwrap().clone();
        assert_eq!(game.process_action(&Action::Wait), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::MoveUp), TurnResult::NotSpent);
        assert_eq!(game.summary(), Some(&summary));
        assert_eq!(game.message_log().history().len(), 2);
        game.process_action(&Action::Confirm);
        assert!(game.summary().is_none());
        assert!(game.message_log().history().is_empty());
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        assert_eq!(game.player().unwrap().get_stats().hp(), 40);
        assert!(
            game.player()
                .unwrap()
                .get_stats()
                .status_effects()
                .is_empty()
        );
        assert!(game.summary().is_none());
    }

    #[test]
    fn player_effect_death_skips_enemy_effects_and_retains_the_lethal_messages() {
        let (mut game, id, _) = combat_game(0, 24, None);
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Poison".into(), i32::MAX, 2)));
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Burning".into(), i32::MAX, 2)),
        );
        game.process_action(&Action::Wait);
        assert!(matches!(game.screen(), Screen::GameOver));
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            2
        );
        let log = game.message_log().history();
        assert!(log[0].contains("Goblin hits you"));
        assert!(log[1].contains("damage from status effects (Poison)"));
        assert_eq!(log[2], "You died from status effects: Poison.");
        assert_eq!(game.summary().unwrap().outcome, log[2]);
    }

    #[test]
    fn enemy_effect_death_follows_its_action_and_uses_the_shared_drop_path_once() {
        use rand::RngExt;
        let (mut game, id, _) = combat_game(0, 24, None);
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Burning".into(), i32::MAX, 1)),
        );
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        game.level()
            .unwrap()
            .enemy(id)
            .unwrap()
            .get_roll(&mut expected_rng);
        let drop = equipment_drop(0, item::catalog(), &mut expected_rng);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert_eq!(
            game.level().unwrap().ground_items().len(),
            usize::from(drop.is_some())
        );
        let log = game.message_log().history();
        assert!(log[0].contains("Goblin hits you"));
        assert_eq!(
            log[1],
            "Goblin takes 24 damage from status effects (Burning)."
        );
        assert_eq!(log[2], "Goblin: Burning expires.");
        assert_eq!(log[3], "You defeat Goblin.");
        let history = log.to_vec();
        game.process_action(&Action::Wait);
        assert_eq!(game.message_log().history(), history);
        assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
    }

    #[test]
    fn direct_kill_prevents_retaliation_and_removed_enemies_never_tick() {
        let (mut game, id, action) = combat_game(150, 24, None);
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Poison".into(), 1, 2)),
        );
        let hp = game.player().unwrap().get_stats().hp();
        game.process_action(&action);
        assert_eq!(game.player().unwrap().get_stats().hp(), hp);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert!(
            !game
                .message_log()
                .history()
                .iter()
                .any(|line| line.contains("hits you") || line.contains("status effects"))
        );
    }

    #[test]
    fn menus_and_pause_do_not_tick_effects_or_roll_attacks() {
        use rand::RngExt;
        let (mut game, id, _) = combat_game(0, 24, None);
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Poison".into(), 1, 2)));
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Burning".into(), 1, 2)),
        );
        game.rng = StdRng::seed_from_u64(7);
        for action in [
            Action::Confirm,
            Action::Pause,
            Action::Wait,
            Action::MoveUp,
            Action::Pause,
        ] {
            assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        }
        assert_eq!(game.player().unwrap().get_stats().hp(), 200);
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            2
        );
        assert_eq!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()[0]
                .turns_remaining(),
            2
        );
        assert_eq!(
            game.rng.random::<u64>(),
            StdRng::seed_from_u64(7).random::<u64>()
        );
        assert!(game.message_log().history().is_empty());
    }

    #[test]
    fn multiple_enemies_attack_in_id_order_once_then_player_and_enemy_effects_tick() {
        let (mut game, first, _) = combat_game(0, 24, None);
        let second = another_adjacent_enemy(&mut game, enemy::EnemyType::Slime);
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Poison".into(), 2, 1)));
        game.level.as_mut().unwrap().damage_enemy(
            first,
            0,
            Some(StatusEffect::new("Burning".into(), 1, 2)),
        );
        game.level.as_mut().unwrap().damage_enemy(
            second,
            0,
            Some(StatusEffect::new("Cold".into(), 1, 2)),
        );
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        let first_damage = attack_damage(
            game.level()
                .unwrap()
                .enemy(first)
                .unwrap()
                .get_roll(&mut expected_rng),
            5,
        );
        let second_damage = attack_damage(
            game.level()
                .unwrap()
                .enemy(second)
                .unwrap()
                .get_roll(&mut expected_rng),
            5,
        );
        game.process_action(&Action::Wait);
        assert_eq!(
            game.player().unwrap().get_stats().hp(),
            200 - first_damage as i32 - second_damage as i32 - 2
        );
        assert_eq!(game.level().unwrap().enemy(first).unwrap().stats().hp(), 23);
        assert_eq!(game.level().unwrap().enemy(second).unwrap().stats().hp(), 7);
        assert_eq!(
            game.message_log().history(),
            [
                format!("Goblin hits you for {first_damage} damage."),
                format!("Slime hits you for {second_damage} damage."),
                "You take 2 damage from status effects (Poison).".to_owned(),
                "You: Poison expires.".to_owned(),
                "Goblin takes 1 damage from status effects (Burning).".to_owned(),
                "Slime takes 1 damage from status effects (Cold).".to_owned(),
            ]
        );
    }

    #[test]
    fn enemy_attack_uses_equipped_defense_and_rejects_invalid_targets_without_rolling() {
        use rand::RngExt;
        for armored in [false, true] {
            let (mut game, id, _) = combat_game(0, 24, None);
            let start = game.player().unwrap().get_position();
            let mut equipment = Equipment::new();
            if armored {
                equipment.equip(item::get_item(1)).unwrap();
            }
            game.player = Some(Player::new(
                start,
                Stats::new(40, 0, 1),
                Inventory::new(),
                equipment,
            ));
            game.rng = StdRng::seed_from_u64(7);
            let mut expected_rng = StdRng::seed_from_u64(7);
            let roll = game
                .level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .get_roll(&mut expected_rng);
            let damage = attack_damage(roll, if armored { 2 } else { 1 });
            game.process_action(&Action::Wait);
            assert_eq!(game.player().unwrap().get_stats().hp(), 40 - damage as i32);
            let enemy_position = *game.level().unwrap().enemy(id).unwrap().position();
            game.player
                .as_mut()
                .unwrap()
                .set_position(enemy_position.offset(1, 1).unwrap());
            game.enemy_attack(id);
            game.player
                .as_mut()
                .unwrap()
                .set_position(enemy_position.offset(2, 0).unwrap());
            game.enemy_attack(id);
            game.level.as_mut().unwrap().remove_enemy(id);
            game.enemy_attack(id);
            assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
            assert_eq!(game.message_log().history().len(), 1);
        }
    }

    #[test]
    fn dead_player_cannot_move_wait_or_open_doors_but_can_leave_the_session() {
        use level::map::TileType;
        let (mut game, _, _) = combat_game(0, 24, None);
        let level = game.level().unwrap();
        let (door, start, bump) = level
            .get_map()
            .positions()
            .filter(|&p| level.get_map().get(p).unwrap().kind == TileType::Door { open: false })
            .find_map(|door| {
                [
                    (1, 0, Action::MoveRight),
                    (-1, 0, Action::MoveLeft),
                    (0, 1, Action::MoveDown),
                    (0, -1, Action::MoveUp),
                ]
                .into_iter()
                .find_map(|(dx, dy, action)| {
                    let start = door.offset(-dx, -dy)?;
                    level.is_walkable(start).then_some((door, start, action))
                })
            })
            .expect("seeded floor contains a closed door with a walkable entrance");
        let enemies: Vec<_> = level
            .enemies()
            .map(|(id, enemy)| (id, *enemy.position()))
            .collect();
        let player = game.player.as_mut().unwrap();
        player.set_position(start);
        player.update_health(-200, None);
        for action in [
            bump,
            Action::Wait,
            Action::MoveLeft,
            Action::MoveRight,
            Action::MoveUp,
            Action::MoveDown,
        ] {
            assert_eq!(game.process_action(&action), TurnResult::NotSpent);
            assert_eq!(game.player().unwrap().get_position(), start);
        }
        let level = game.level().unwrap();
        assert_eq!(
            level.get_map().get(door).unwrap().kind,
            TileType::Door { open: false }
        );
        assert_eq!(
            level
                .enemies()
                .map(|(id, enemy)| (id, *enemy.position()))
                .collect::<Vec<_>>(),
            enemies
        );
        assert!(game.message_log().history().is_empty());
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::MainMenu));
    }

    fn combat_game(
        attack: i32,
        target_hp: u32,
        effect: Option<stats::EffectSpec>,
    ) -> (Game, EnemyId, Action) {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        while level.remove_ground_item(0).is_some() {}
        let start = level.player_start();
        let (target, action) = [
            (start.offset(1, 0), Action::MoveRight),
            (start.offset(-1, 0), Action::MoveLeft),
            (start.offset(0, 1), Action::MoveDown),
            (start.offset(0, -1), Action::MoveUp),
        ]
        .into_iter()
        .find_map(|(position, action)| {
            position
                .filter(|&p| level.is_walkable(p))
                .map(|p| (p, action))
        })
        .unwrap();
        let id = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, target))
            .ok()
            .unwrap();
        level.damage_enemy(id, 24 - target_hp, None);
        let mut equipment = Equipment::new();
        equipment.equip(item::get_item(0)).unwrap();
        if let Some(effect) = effect {
            equipment
                .equip(item::get_item(0).with_combat(item::CombatModifiers::weapon(
                    combat::DamageDice::new(2, combat::Die::D4),
                    Some(effect),
                )))
                .unwrap();
        }
        game.player = Some(Player::new(
            start,
            Stats::new(200, attack, 5),
            Inventory::new(),
            equipment,
        ));
        (game, id, action)
    }

    #[test]
    fn bump_attack_runs_one_counterattack_without_moving_the_player() {
        let (mut game, id, action) = combat_game(0, 24, None);
        let start = game.player().unwrap().get_position();
        let level = game.level.as_mut().unwrap();
        let other_position = level
            .get_map()
            .positions()
            .find(|&p| level.is_walkable(p) && p.distance(start) > 6 && level.enemy_at(p).is_none())
            .unwrap();
        let other = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Slime, other_position))
            .ok()
            .unwrap();
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        let damage = attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 2);
        let counter = attack_damage(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .get_roll(&mut expected_rng),
            5,
        );
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), start);
        assert_eq!(
            game.player().unwrap().get_stats().hp(),
            200 - counter as i32
        );
        let level = game.level().unwrap();
        assert_eq!(level.enemy(id).unwrap().stats().hp(), 24 - damage as i32);
        assert_eq!(level.enemy(other).unwrap().stats().hp(), 8);
        assert_eq!(*level.enemy(other).unwrap().position(), other_position);
        assert_eq!(
            game.message_log().history(),
            &[
                format!("You hit Goblin for {damage} damage."),
                format!("Goblin hits you for {counter} damage.")
            ]
        );
    }

    #[test]
    fn new_weapon_effects_tick_that_turn_and_duplicates_expire_independently() {
        let (mut game, id, action) =
            combat_game(0, 24, Some(stats::EffectSpec::new("Poison".into(), 1, 3)));
        game.rng = StdRng::seed_from_u64(7);
        let mut expected_rng = StdRng::seed_from_u64(7);
        let mut expected_hp = 24;
        for active_effects in 1..=2 {
            let damage = attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 2);
            expected_hp -= damage as i32 + active_effects;
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .get_roll(&mut expected_rng);
            assert_eq!(game.process_action(&action), TurnResult::Spent);
            assert_eq!(
                game.level().unwrap().enemy(id).unwrap().stats().hp(),
                expected_hp
            );
        }
        let effects = game
            .level()
            .unwrap()
            .enemy(id)
            .unwrap()
            .stats()
            .status_effects();
        assert_eq!(effects.len(), 2);
        assert_eq!(
            effects
                .iter()
                .map(StatusEffect::turns_remaining)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        let log = game.message_log().history();
        assert_eq!(log.len(), 8);
        assert!(log[0].contains("hit Goblin"));
        assert!(
            log.iter()
                .all(|line| !line.contains('#') && !line.contains("EnemyId"))
        );
        assert!(log[1].contains("apply Poison"));
        game.process_action(&Action::Wait);
        let effects = game
            .level()
            .unwrap()
            .enemy(id)
            .unwrap()
            .stats()
            .status_effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].turns_remaining(), 1);
        game.process_action(&Action::Wait);
        assert!(
            game.level()
                .unwrap()
                .enemy(id)
                .unwrap()
                .stats()
                .status_effects()
                .is_empty()
        );
        assert_eq!(
            game.message_log()
                .history()
                .iter()
                .filter(|line| line.as_str() == "Goblin: Poison expires.")
                .count(),
            2
        );
    }

    #[test]
    fn lethal_bump_removes_only_target_and_requires_another_action_to_enter_tile() {
        let (mut game, id, action) =
            combat_game(15, 4, Some(stats::EffectSpec::new("Poison".into(), 7, 3)));
        let start = game.player().unwrap().get_position();
        let target = *game.level().unwrap().enemy(id).unwrap().position();
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert_eq!(game.player().unwrap().get_position(), start);
        let log = game.message_log().history();
        assert!(log[0].contains("4 damage"));
        assert!(log[1].contains("defeat Goblin"));
        assert!(!log.iter().any(|line| line.contains("Poison")));
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), target);
    }

    #[test]
    fn attacks_reject_distant_diagonal_missing_dead_player_and_paused_requests() {
        let (mut game, id, action) = combat_game(0, 24, None);
        let target = *game.level().unwrap().enemy(id).unwrap().position();
        for origin in [
            target.offset(1, 1).unwrap(),
            target.offset(2, 0).unwrap(),
            target,
        ] {
            game.player.as_mut().unwrap().set_position(origin);
            game.attack_enemy(id);
            assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        }
        let start = game.level().unwrap().player_start();
        game.player.as_mut().unwrap().set_position(start);
        game.process_action(&Action::Pause);
        assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        game.process_action(&Action::Pause);
        game.player.as_mut().unwrap().update_health(-200, None);
        assert_eq!(game.process_action(&action), TurnResult::NotSpent);
        assert_eq!(game.level().unwrap().enemy(id).unwrap().stats().hp(), 24);
        game.level.as_mut().unwrap().remove_enemy(id);
        game.attack_enemy(id);
        assert!(game.message_log().history().is_empty());
    }

    #[test]
    fn defeat_rolls_once_and_drops_owned_equipment_at_the_death_tile() {
        use rand::RngExt;
        let mut dropped = false;
        let mut skipped = false;
        for seed in 0..200 {
            let (mut game, id, action) = combat_game(150, 24, None);
            let position = *game.level().unwrap().enemy(id).unwrap().position();
            // Existing items can share the death tile with the new equipment.
            game.level
                .as_mut()
                .unwrap()
                .insert_ground_item(item::get_item(2), position)
                .unwrap();
            game.rng = StdRng::seed_from_u64(seed);
            let mut expected_rng = StdRng::seed_from_u64(seed);
            game.player().unwrap().roll_attack(&mut expected_rng);
            let expected = equipment_drop(0, item::catalog(), &mut expected_rng);
            game.process_action(&action);
            let items = game.level().unwrap().ground_items();
            if let Some(expected) = expected {
                dropped = true;
                assert_eq!(items.len(), 2);
                assert_eq!(items[1].item, expected);
                assert_eq!(items[1].position, position);
                assert!(
                    game.message_log()
                        .history()
                        .last()
                        .unwrap()
                        .contains("drops")
                );
            } else {
                skipped = true;
                assert_eq!(items.len(), 1);
            }
            let before_log = game.message_log().history().to_vec();
            let count = items.len();
            game.defeat_enemy(id);
            assert_eq!(game.level().unwrap().ground_items().len(), count);
            assert_eq!(game.message_log().history(), before_log);
            assert_eq!(game.rng.random::<u64>(), expected_rng.random::<u64>());
        }
        assert!(dropped && skipped);
    }

    #[test]
    fn defeat_path_rejects_living_enemies_and_accepts_effect_deaths() {
        let (mut game, id, _) = combat_game(0, 1, None);
        game.defeat_enemy(id);
        assert!(game.level().unwrap().enemy(id).is_some());
        assert!(game.message_log().history().is_empty());
        let level = game.level.as_mut().unwrap();
        level.damage_enemy(id, 0, Some(stats::StatusEffect::new("Poison".into(), 5, 1)));
        assert!(level.tick_enemy_effects(id).unwrap().is_dead);
        game.defeat_enemy(id);
        assert!(game.level().unwrap().enemy(id).is_none());
        assert!(game.message_log().history()[0].contains("defeat"));
    }

    #[test]
    fn menu_and_pause_navigation_preserves_the_active_level() {
        let mut game = Game::new();
        assert!(matches!(game.screen(), Screen::MainMenu));
        assert!(game.level().is_none());
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        let start = game.level().unwrap().player_start();
        assert!(game.level().unwrap().get_map().is_visible(start));
        assert!(game.level().unwrap().get_map().get(start).unwrap().seen);
        game.process_action(&Action::Pause);
        assert!(matches!(game.screen(), Screen::Paused));
        game.process_action(&Action::Confirm);
        assert!(matches!(game.screen(), Screen::Playing));
        assert_eq!(game.level().unwrap().player_start(), start);
        assert_eq!(game.level().unwrap().depth(), 0);
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(matches!(game.screen(), Screen::MainMenu));
        assert!(game.level().is_none());
        game.process_action(&Action::Confirm);
        assert_eq!(game.level().unwrap().depth(), 0);
    }

    #[test]
    fn new_game_places_player_at_level_start() {
        let mut game = Game::new();
        assert!(game.player().is_none());
        game.process_action(&Action::Confirm);
        let start = game.level().unwrap().player_start();
        assert!(game.level().unwrap().is_walkable(start));
        let player = game.player().unwrap();
        assert_eq!(player.get_position(), start);
        assert_eq!(player.get_stats().hp(), STARTING_MAX_HP);
        assert_eq!(player.get_stats().attack(), 0);
        assert_eq!(
            player.damage_dice(),
            combat::DamageDice::new(2, combat::Die::D4)
        );
        assert_eq!(
            player.get_equipped_items().weapon.as_ref().unwrap().name,
            "Pickaxe"
        );
        assert!(player.get_equipped_items().armor.is_none());
        assert!(game.message_log().messages().is_empty());
    }

    #[test]
    fn returning_to_menu_clears_the_session() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::Pause);
        game.process_action(&Action::Back);
        assert!(game.player().is_none());
        assert!(game.message_log().messages().is_empty());
    }

    #[test]
    fn moves_only_onto_walkable_tiles() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let start = game.player().unwrap().get_position();
        for (action, dx, dy) in [
            (Action::MoveUp, 0, -1),
            (Action::MoveDown, 0, 1),
            (Action::MoveLeft, -1, 0),
            (Action::MoveRight, 1, 0),
        ] {
            game.player.as_mut().unwrap().set_position(start);
            let expected = start
                .offset(dx, dy)
                .filter(|&target| {
                    let level = game.level().unwrap();
                    level.is_walkable(target) && level.enemy_at(target).is_none()
                })
                .unwrap_or(start);
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), expected);
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position)
                    == (expected.distance(position) <= level::map::PLAYER_VISION_RADIUS
                        && map.has_line_of_sight(expected, position))
            }));

            if expected != start {
                // Exercise the merge boundary: terrain permits this move, but
                // Level's enemy occupancy must prevent the player entering it.
                game.player.as_mut().unwrap().set_position(start);
                game.level.as_mut().unwrap().update_visibility(start);
                let enemy = enemy::Enemy::new(enemy::EnemyType::Slime, expected);
                let id = game
                    .level
                    .as_mut()
                    .unwrap()
                    .insert_enemy(enemy)
                    .ok()
                    .unwrap();
                let seen: Vec<_> = game
                    .level()
                    .unwrap()
                    .get_map()
                    .positions()
                    .map(|position| game.level().unwrap().get_map().get(position).unwrap().seen)
                    .collect();
                game.rng = StdRng::seed_from_u64(7);
                let mut expected_rng = StdRng::seed_from_u64(7);
                let damage =
                    attack_damage(game.player().unwrap().roll_attack(&mut expected_rng), 0);
                assert_eq!(game.process_action(&action), TurnResult::Spent);
                assert_eq!(game.player().unwrap().get_position(), start);
                let level = game.level().unwrap();
                if damage >= 8 {
                    assert!(level.enemy(id).is_none());
                } else {
                    assert_eq!(level.enemy_at(expected), Some(id));
                    assert_eq!(level.enemy(id).unwrap().stats().hp(), 8 - damage as i32);
                }
                assert!(
                    level
                        .get_map()
                        .positions()
                        .zip(seen)
                        .all(|(position, seen)| {
                            let tile = level.get_map().get(position).unwrap();
                            tile.seen == seen
                                && tile.visible
                                    == (start.distance(position)
                                        <= level::map::PLAYER_VISION_RADIUS
                                        && level.get_map().has_line_of_sight(start, position))
                        })
                );
                game.level.as_mut().unwrap().remove_enemy(id);
            }
        }
    }

    #[test]
    fn walking_into_a_wall_stops_the_player() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        // Rooms are enclosed, so walking left long enough always hits something
        // solid (or an enemy, which also blocks)
        for _ in 0..100 {
            game.process_action(&Action::MoveLeft);
        }
        let level = game.level().unwrap();
        let stopped = game.player().unwrap().get_position();
        assert!(level.is_walkable(stopped));
        assert!(
            stopped
                .offset(-1, 0)
                .is_none_or(|left| !level.is_walkable(left) || level.enemy_at(left).is_some())
        );
    }

    #[test]
    fn movement_is_ignored_while_paused() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        let start = game.player().unwrap().get_position();
        game.process_action(&Action::Pause);
        for action in [
            Action::MoveUp,
            Action::MoveDown,
            Action::MoveLeft,
            Action::MoveRight,
        ] {
            game.process_action(&action);
        }
        assert_eq!(game.player().unwrap().get_position(), start);
    }

    #[test]
    fn moves_and_waits_spend_a_turn_even_when_blocked() {
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 101)));
        for _ in 0..100 {
            assert_eq!(game.process_action(&Action::MoveLeft), TurnResult::Spent);
        }
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            1
        );
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        assert!(
            game.player()
                .unwrap()
                .get_stats()
                .status_effects()
                .is_empty()
        );
    }

    #[test]
    fn menus_and_pausing_do_not_spend_a_turn() {
        let mut game = Game::new();
        assert_eq!(game.process_action(&Action::MoveDown), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::MoveUp), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Confirm), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::MoveLeft), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
    }

    #[test]
    fn bumping_a_closed_door_opens_it_without_moving() {
        use level::map::TileType;
        // Not every floor has a door, so try new games until one does
        for seed in 0..50 {
            let mut game = Game::new();
            game.rng = StdRng::seed_from_u64(seed);
            game.process_action(&Action::Confirm);
            let level = game.level().unwrap();
            let map = level.get_map();
            let found = map.positions().find_map(|door| {
                if !matches!(map.get(door)?.kind, TileType::Door { open: false }) {
                    return None;
                }
                [
                    (Action::MoveRight, -1, 0),
                    (Action::MoveLeft, 1, 0),
                    (Action::MoveDown, 0, -1),
                    (Action::MoveUp, 0, 1),
                ]
                .into_iter()
                .find_map(|(action, dx, dy)| {
                    let beside = door.offset(dx, dy)?;
                    (level.is_walkable(beside) && level.enemy_at(beside).is_none())
                        .then_some((door, beside, action))
                })
            });
            let Some((door, beside, action)) = found else {
                continue;
            };
            game.player.as_mut().unwrap().set_position(beside);
            game.level.as_mut().unwrap().update_visibility(beside);
            let seen: Vec<_> = game
                .level()
                .unwrap()
                .get_map()
                .positions()
                .filter(|&position| game.level().unwrap().get_map().get(position).unwrap().seen)
                .collect();
            assert_eq!(game.process_action(&action), TurnResult::Spent);
            assert_eq!(game.player().unwrap().get_position(), beside);
            assert!(game.level().unwrap().is_walkable(door));
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position)
                    == (beside.distance(position) <= level::map::PLAYER_VISION_RADIUS
                        && map.has_line_of_sight(beside, position))
            }));
            assert!(seen.iter().all(|&position| map.get(position).unwrap().seen));
            game.process_action(&action);
            assert_eq!(game.player().unwrap().get_position(), door);
            let map = game.level().unwrap().get_map();
            assert!(map.positions().all(|position| {
                map.is_visible(position)
                    == (door.distance(position) <= level::map::PLAYER_VISION_RADIUS
                        && map.has_line_of_sight(door, position))
            }));
            return;
        }
        panic!("no closed door found in 50 generated levels");
    }

    #[test]
    fn waiting_advances_enemy_ai_but_pausing_does_not() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);

        // Remove generated enemies to test one deterministic AI actor.
        let ids: Vec<_> = game.level().unwrap().enemies().map(|(id, _)| id).collect();
        for id in ids {
            game.level.as_mut().unwrap().remove_enemy(id);
        }

        // Find three walkable tiles in one row; the enemy is two steps away.
        let (player_pos, expected_step, enemy_pos) = {
            let level = game.level().unwrap();
            level
                .get_map()
                .positions()
                .find_map(|player_pos| {
                    let step = player_pos.offset(1, 0)?;
                    let enemy_pos = player_pos.offset(2, 0)?;
                    (level.is_walkable(player_pos)
                        && level.is_walkable(step)
                        && level.is_walkable(enemy_pos))
                    .then_some((player_pos, step, enemy_pos))
                })
                .expect("generated level should contain three consecutive floor tiles")
        };
        game.player.as_mut().unwrap().set_position(player_pos);
        game.level.as_mut().unwrap().update_visibility(player_pos);
        let id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, enemy_pos))
            .ok()
            .unwrap();

        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::NotSpent);
        assert_eq!(
            *game.level().unwrap().enemy(id).unwrap().position(),
            enemy_pos
        );
        assert_eq!(game.process_action(&Action::Pause), TurnResult::NotSpent);

        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let moved = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*moved.position(), expected_step);
        assert!(matches!(moved.ai(), enemy::AiState::Attacking));
        assert_ne!(*moved.position(), player_pos);

        // After approaching, the next spent turn resolves one adjacent attack.
        let hp = game.player().unwrap().get_stats().hp();
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        assert_eq!(
            *game.level().unwrap().enemy(id).unwrap().position(),
            expected_step
        );
        assert!(game.player().unwrap().get_stats().hp() < hp);
    }

    #[test]
    fn closed_door_hides_player_from_ai_and_opening_it_allows_immediate_detection() {
        use level::map::TileType;
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);
        let level = game.level.as_mut().unwrap();
        let ids: Vec<_> = level.enemies().map(|(id, _)| id).collect();
        for id in ids {
            level.remove_enemy(id);
        }
        let (door, player_position, enemy_position, action) = level
            .get_map()
            .positions()
            .filter(|&p| level.get_map().get(p).unwrap().kind == TileType::Door { open: false })
            .find_map(|door| {
                [
                    (-1, 0, 1, 0, Action::MoveRight),
                    (0, -1, 0, 1, Action::MoveDown),
                ]
                .into_iter()
                .find_map(|(px, py, ex, ey, action)| {
                    let player = door.offset(px, py)?;
                    let enemy = door.offset(ex, ey)?;
                    (level.is_walkable(player) && level.is_walkable(enemy))
                        .then_some((door, player, enemy, action))
                })
            })
            .expect("seeded floor contains a closed corridor door");
        let id = level
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, enemy_position))
            .ok()
            .unwrap();
        level.set_enemy_ai_state(id, enemy::AiState::Attacking);
        game.player.as_mut().unwrap().set_position(player_position);
        level.update_visibility(player_position);
        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let enemy = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*enemy.position(), enemy_position);
        assert!(matches!(enemy.ai(), enemy::AiState::Idle));
        game.player
            .as_mut()
            .unwrap()
            .update_health(0, Some(StatusEffect::new("Timer".into(), 0, 2)));
        game.level.as_mut().unwrap().damage_enemy(
            id,
            0,
            Some(StatusEffect::new("Timer".into(), 0, 2)),
        );
        assert_eq!(game.process_action(&action), TurnResult::Spent);
        assert_eq!(game.player().unwrap().get_position(), player_position);
        let enemy = game.level().unwrap().enemy(id).unwrap();
        assert_eq!(*enemy.position(), door);
        assert!(matches!(enemy.ai(), enemy::AiState::Attacking));
        assert_eq!(enemy.stats().status_effects()[0].turns_remaining(), 1);
        assert_eq!(
            game.player().unwrap().get_stats().status_effects()[0].turns_remaining(),
            1
        );
    }

    #[test]
    fn game_ai_avoids_an_occupied_preferred_step() {
        let mut game = Game::new();
        game.rng = StdRng::seed_from_u64(42);
        game.process_action(&Action::Confirm);

        let ids: Vec<_> = game.level().unwrap().enemies().map(|(id, _)| id).collect();
        for id in ids {
            game.level.as_mut().unwrap().remove_enemy(id);
        }

        // The nearest horizontal step is occupied, but moving down is legal.
        let (from, blocked, alternative, player_pos) = {
            let level = game.level().unwrap();
            level
                .get_map()
                .positions()
                .find_map(|from| {
                    let blocked = from.offset(1, 0)?;
                    let alternative = from.offset(0, 1)?;
                    let player_pos = from.offset(2, 1)?;
                    (level.is_walkable(from)
                        && level.is_walkable(blocked)
                        && level.is_walkable(alternative)
                        && level.is_walkable(player_pos))
                    .then_some((from, blocked, alternative, player_pos))
                })
                .expect("generated level should contain the necessary walkable tiles")
        };

        game.player.as_mut().unwrap().set_position(player_pos);
        game.level.as_mut().unwrap().update_visibility(player_pos);
        let active_id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Goblin, from))
            .ok()
            .unwrap();
        let blocker_id = game
            .level
            .as_mut()
            .unwrap()
            .insert_enemy(enemy::Enemy::new(enemy::EnemyType::Slime, blocked))
            .ok()
            .unwrap();

        assert_eq!(game.process_action(&Action::Wait), TurnResult::Spent);
        let level = game.level().unwrap();
        assert_eq!(*level.enemy(active_id).unwrap().position(), alternative);
        // The diagonal neighbor now approaches cardinally instead of requesting
        // PR12's diagonal attack. It still cannot enter the player's tile.
        let blocker_position = *level.enemy(blocker_id).unwrap().position();
        assert!(blocked.is_cardinal_neighbor(blocker_position));
        assert_ne!(blocker_position, alternative);
        assert!(level.enemy_at(player_pos).is_none());
    }

    #[test]
    fn quit_menu_entry_and_global_quit_end_the_session() {
        let mut game = Game::new();
        assert!(game.level().is_none());
        game.process_action(&Action::MoveDown);
        assert_eq!(game.menu_selection(), 1);
        game.process_action(&Action::Confirm);
        assert!(game.should_quit());
        assert!(game.level().is_none());
        let mut game = Game::new();
        game.process_action(&Action::Confirm);
        game.process_action(&Action::Quit);
        assert!(game.should_quit());
    }
}
