//! The player character (SDD 3.3).
//!
//! Contracts:
//! - 6 Player State Access: position, stats, inventory, equipped items
//! - 7 Player State Modification: set_position, update_health, update_equipped
//!
//! Player does not validate terrain or resolve combat; Game coordinates those.

use crate::game::combat::{DamageDice, Die};
use crate::game::equipment::Equipment;
use crate::game::inventory::Inventory;
use crate::game::item::ItemType;
use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};
use rand::Rng;

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

    /// Weapon damage dice, falling back to 1d4 when no weapon is equipped.
    pub fn damage_dice(&self) -> DamageDice {
        self.equipment
            .weapon
            .as_ref()
            .filter(|item| item.kind == ItemType::Weapon)
            .and_then(|item| item.combat().damage())
            .unwrap_or(DamageDice::new(1, Die::D4))
    }

    /// Rolls damage once; Stats attack is an optional flat bonus (zero at start).
    pub fn roll_attack(&self, rng: &mut impl Rng) -> u32 {
        self.damage_dice()
            .roll(rng)
            .saturating_add(self.stats.attack() as u32)
    }

    /// Returns base defense plus the equipped armor bonus, without mutation.
    pub fn effective_defense(&self) -> i32 {
        self.stats.defense().saturating_add(
            self.equipment
                .armor
                .as_ref()
                .filter(|item| item.kind == ItemType::Armor)
                .map_or(0, |item| item.combat().defense()),
        )
    }

    /// Creates a fresh weapon effect; Game applies it after a valid nonlethal hit.
    pub fn weapon_effect(&self) -> Option<StatusEffect> {
        self.equipment
            .weapon
            .as_ref()
            .filter(|item| item.kind == ItemType::Weapon)?
            .combat()
            .on_hit()
            .map(|spec| spec.instantiate())
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
}

#[cfg(test)]
mod tests {
    use super::{Equipment, Inventory, Player, Position, Stats, StatusEffect};

    #[test]
    fn equipment_queries_are_repeatable_and_follow_unequipping() {
        use crate::game::item::{CombatModifiers, get_item};
        let mut p = player();
        p.equipment.equip(get_item(0)).unwrap();
        p.equipment.equip(get_item(1)).unwrap();
        for _ in 0..3 {
            assert_eq!(p.damage_dice(), super::DamageDice::new(2, super::Die::D4));
            assert_eq!(p.effective_defense(), 6);
            assert_eq!((p.stats.attack(), p.stats.defense()), (15, 5));
        }
        p.equipment
            .equip(get_item(1).with_combat(CombatModifiers::armor(i32::MAX)))
            .unwrap();
        assert_eq!(p.effective_defense(), i32::MAX);
        p.equipment.unequip_weapon();
        p.equipment.unequip_armor();
        assert_eq!(p.damage_dice(), super::DamageDice::new(1, super::Die::D4));
        assert_eq!(p.effective_defense(), 5);
    }

    #[test]
    fn weapon_hits_create_independent_enemy_effect_durations() {
        use crate::game::{
            enemy::{AiState, Enemy, EnemyType},
            item::{CombatModifiers, get_item},
            stats::EffectSpec,
        };
        let mut p = player();
        let weapon = get_item(0).with_combat(CombatModifiers::weapon(
            super::DamageDice::new(2, super::Die::D4),
            Some(EffectSpec::new("Poison".into(), 2, 3)),
        ));
        p.equipment.equip(weapon).unwrap();
        let mut enemy = Enemy::new(EnemyType::Ghost, Position::new(2, 1));
        enemy.add_status_effect(p.weapon_effect().unwrap());
        assert_eq!(enemy.stats().hp(), 14);
        assert_eq!(enemy.tick_status_effects(), 2);
        enemy.add_status_effect(p.weapon_effect().unwrap());
        assert_eq!(enemy.stats().status_effects()[0].turns_remaining(), 2);
        assert_eq!(enemy.stats().status_effects()[1].turns_remaining(), 3);
        assert_eq!(enemy.tick_status_effects(), 4);
        assert_eq!(enemy.tick_status_effects(), 4);
        assert!(matches!(enemy.ai(), AiState::Fleeing));
        assert_eq!(enemy.tick_status_effects(), 2);
        assert!(enemy.stats().status_effects().is_empty());
        assert_eq!(p.weapon_effect().unwrap().turns_remaining(), 3);
    }

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
