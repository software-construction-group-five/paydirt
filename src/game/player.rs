//! The player character (SDD 3.3).
//!
//! Contracts:
//! - 6 Player State Access: position, stats, inventory, equipped items
//! - 7 Player State Modification: set_position, update_health, update_equipped
//!
//! Player does not validate terrain or resolve combat; Game coordinates those.

use crate::game::equipment::Equipment;
use crate::game::inventory::Inventory;
use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};

/// Represents and manages the player state.
pub struct Player {
    position: Position,
    stats: Stats,
    inventory: Inventory,
    equipment: Equipment,
}

impl Player {
    /// Creates the player. Called by Game when a new game starts (Game Contract 1)
    pub fn new(
        position: Position,
        stats: Stats,
        inventory: Inventory,
        equipment: Equipment,
    ) -> Self {
        Self {
            position,
            stats,
            inventory,
            equipment,
        }
    }

    /// The player's current position (Contract 6, Protocol 1)
    pub fn get_position(&self) -> Position {
        self.position
    }

    /// The player's health and status effects (Contract 6, Protocol 2)
    pub fn get_stats(&self) -> &Stats {
        &self.stats
    }

    /// The items the player is carrying (Contract 6, Protocol 3)
    pub fn get_inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The player's equipped weapon and armor (Contract 6, Protocol 4)
    pub fn get_equipped_items(&self) -> &Equipment {
        &self.equipment
    }

    /// Contract 7, Protocol 1. Game checks the tile is walkable first
    pub fn set_position(&mut self, position: Position) {
        self.position = position;
    }

    /// Contract 7, Protocol 2. Negative amount is damage, positive is healing
    pub fn update_health(&mut self, amount: i32, effect: Option<StatusEffect>) {
        self.stats.update_health(amount);
        if let Some(effect) = effect {
            self.stats.add_status_effect(effect);
        }
    }

    /// Applies status effect damage for one turn and returns the HP lost
    /// (private responsibility: calculate status effects)
    pub fn tick_status_effects(&mut self) -> i32 {
        self.stats.tick_status_effects()
    }

    // TODO(joseph): update_equipped (Contract 7, Protocol 3) once Equipment is done
}

#[cfg(test)]
mod tests {
    use super::{Equipment, Inventory, Player, Position, Stats, StatusEffect};

    // TODO(joseph): switch to Equipment::new() once it exists
    fn player() -> Player {
        Player::new(
            Position::new(1, 1),
            Stats::new(100, 15, 5),
            Inventory::new(),
            Equipment::new(),
        )
    }

    #[test]
    fn set_position_moves_player() {
        let mut p = player();
        p.set_position(Position::new(4, 2));
        assert_eq!(p.get_position(), Position::new(4, 2));
    }

    #[test]
    fn update_health_stays_between_zero_and_max() {
        let mut p = player();
        p.update_health(-25, None);
        assert_eq!(p.get_stats().hp(), 75);
        p.update_health(50, None);
        assert_eq!(p.get_stats().hp(), 100);
        p.update_health(-150, None);
        assert_eq!(p.get_stats().hp(), 0);
    }

    #[test]
    fn update_health_adds_effect() {
        let mut p = player();
        let poison = StatusEffect::new(String::from("Poisoned"), 5, 2);
        p.update_health(0, Some(poison));
        assert_eq!(p.tick_status_effects(), 5);
        assert_eq!(p.get_stats().hp(), 95);
    }
}
