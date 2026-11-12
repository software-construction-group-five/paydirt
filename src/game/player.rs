//! The player character (SDD 3.3).
//!
//! Contracts:
//! - 6 Player State Access: position, stats, inventory, equipped items
//! - 7 Player State Modification: set_position, update_health, update_equipped
//!
//! Player does not validate terrain or resolve combat; Game coordinates those.

use crate::game::combat::{DamageDice, Die};
use crate::game::equipment::{Equipment, EquipmentSlot};
use crate::game::inventory::Inventory;
use crate::game::item::{Item, ItemType};
use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};
use rand::Rng;

/// A rejected item action leaves ownership and player state unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemError {
    /// The selected entry or equipment slot is empty.
    Missing,
    /// The selected item cannot perform that action.
    WrongCategory,
    /// The item's carried category is full.
    Full,
    /// Healing would have no effect.
    FullHealth,
    /// A dead player cannot use items.
    Dead,
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Missing => "No item is selected.",
            Self::WrongCategory => "That item cannot be used for this action.",
            Self::Full => "That inventory category is full.",
            Self::FullHealth => "You are already at full health; the potion is kept.",
            Self::Dead => "You cannot use items after death.",
        })
    }
}

/// Represents and manages the player state.
pub struct Player {
    position: Position,
    stats: Stats,
    inventory: Inventory,
    equipment: Equipment,
}

impl Player {
    /// Transfers an existing floor or dropped item into the pack.
    pub fn receive_item(&mut self, item: Item) -> Result<(), Item> {
        self.inventory.insert_item(item)
    }

    /// Takes one carried entry without confusing duplicate catalog copies.
    pub fn take_carried(&mut self, index: usize) -> Option<Item> {
        self.inventory.remove_entry(index)
    }

    /// Takes an equipped item for a direct drop onto a validated floor tile.
    pub fn take_equipped(&mut self, slot: EquipmentSlot) -> Option<Item> {
        self.equipment.take(slot)
    }

    /// Swaps a carried weapon or armor into its slot, preserving both copies.
    pub fn equip_carried(&mut self, index: usize) -> Result<(), ItemError> {
        let item = self
            .inventory
            .items()
            .get(index)
            .ok_or(ItemError::Missing)?;
        if item.kind == ItemType::Consumable {
            return Err(ItemError::WrongCategory);
        }
        let item = self
            .inventory
            .remove_entry(index)
            .expect("selected entry exists");
        let displaced = self
            .equipment
            .equip(item)
            .expect("selected category is equipment");
        if let Some(displaced) = displaced {
            // Removing the selected item freed a space in this same category.
            self.inventory
                .insert_item(displaced)
                .expect("equipment swap freed category capacity");
        }
        Ok(())
    }

    /// Moves equipment back to the pack; a full category preserves both stores.
    pub fn unequip(&mut self, slot: EquipmentSlot) -> Result<(), ItemError> {
        let item = self.equipment.item(slot).ok_or(ItemError::Missing)?;
        if !self.inventory.can_accept(item.kind) {
            return Err(ItemError::Full);
        }
        let item = self.equipment.take(slot).expect("selected slot exists");
        self.inventory
            .insert_item(item)
            .expect("category capacity was checked");
        Ok(())
    }

    /// Consumes exactly one healing item on success and returns actual HP restored.
    pub fn use_carried(&mut self, index: usize) -> Result<i32, ItemError> {
        if self.stats.is_dead() {
            return Err(ItemError::Dead);
        }
        let item = self
            .inventory
            .items()
            .get(index)
            .ok_or(ItemError::Missing)?;
        if item.kind != ItemType::Consumable {
            return Err(ItemError::WrongCategory);
        }
        let healing = item.healing().ok_or(ItemError::WrongCategory)?;
        if self.stats.hp() == self.stats.max_hp() {
            return Err(ItemError::FullHealth);
        }
        self.inventory
            .remove_entry(index)
            .expect("selected consumable exists");
        let old_hp = self.stats.hp();
        self.stats.update_health(healing);
        Ok(self.stats.hp() - old_hp)
    }
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
    fn full_pack_weapon_swaps_preserve_original_dice_and_effects() {
        use super::{EquipmentSlot, ItemError};
        use crate::game::{
            combat::{DamageDice, Die},
            item::{CombatModifiers, get_item},
            stats::EffectSpec,
        };
        let mut p = player();
        let original = get_item(0);
        p.equipment.equip(original.clone()).unwrap();
        let custom = get_item(0).with_combat(CombatModifiers::weapon(
            DamageDice::new(1, Die::D12),
            Some(EffectSpec::new("Burning".into(), 2, 2)),
        ));
        p.receive_item(custom.clone()).unwrap();
        p.receive_item(get_item(0)).unwrap();
        p.receive_item(get_item(0)).unwrap();
        p.equip_carried(0).unwrap();
        assert_eq!(p.equipment.weapon, Some(custom));
        assert_eq!(p.inventory.items().len(), 3);
        assert_eq!(p.inventory.items()[2], original);
        assert_eq!(p.damage_dice(), DamageDice::new(1, Die::D12));
        assert_eq!(p.weapon_effect().unwrap().name(), "Burning");
        let before = p.inventory.items().to_vec();
        assert_eq!(p.unequip(EquipmentSlot::Weapon), Err(ItemError::Full));
        assert_eq!(p.inventory.items(), before);
        assert_eq!(p.damage_dice(), DamageDice::new(1, Die::D12));
        p.take_carried(0);
        p.unequip(EquipmentSlot::Weapon).unwrap();
        assert_eq!(p.damage_dice(), DamageDice::new(1, Die::D4));
        assert!(p.weapon_effect().is_none());
    }

    #[test]
    fn potion_use_preserves_rejections_and_consumes_only_the_selected_copy() {
        use super::ItemError;
        use crate::game::item::get_item;
        let mut p = player();
        p.receive_item(get_item(2)).unwrap();
        let custom = get_item(2).with_healing(3);
        p.receive_item(custom.clone()).unwrap();
        assert_eq!(p.use_carried(0), Err(ItemError::FullHealth));
        assert_eq!(p.inventory.items().len(), 2);
        p.update_health(-5, None);
        assert_eq!(p.use_carried(1), Ok(3));
        assert_eq!(p.inventory.items(), &[get_item(2)]);
        assert_eq!(p.use_carried(0), Ok(2));
        assert!(p.inventory.items().is_empty());
        assert_eq!(p.use_carried(0), Err(ItemError::Missing));
        p.receive_item(get_item(0)).unwrap();
        assert_eq!(p.use_carried(0), Err(ItemError::WrongCategory));
        p.update_health(-100, None);
        assert_eq!(p.use_carried(0), Err(ItemError::Dead));
        assert_eq!(p.inventory.items(), &[get_item(0)]);
    }

    #[test]
    fn armor_swaps_change_effective_defense_and_full_unequip_is_atomic() {
        use super::{EquipmentSlot, ItemError};
        use crate::game::item::{CombatModifiers, get_item};
        let mut p = player();
        p.receive_item(get_item(1)).unwrap();
        p.equip_carried(0).unwrap();
        assert_eq!(p.effective_defense(), 6);
        let better = get_item(1).with_combat(CombatModifiers::armor(4));
        p.receive_item(better.clone()).unwrap();
        p.equip_carried(0).unwrap();
        assert_eq!(p.effective_defense(), 9);
        assert_eq!(p.inventory.items(), &[get_item(1)]);
        assert_eq!(p.unequip(EquipmentSlot::Armor), Err(ItemError::Full));
        assert_eq!(p.equipment.armor, Some(better.clone()));
        assert_eq!(p.inventory.items(), &[get_item(1)]);
        p.take_carried(0);
        p.unequip(EquipmentSlot::Armor).unwrap();
        assert_eq!(p.inventory.items(), &[better]);
        assert_eq!(p.effective_defense(), 5);
        assert_eq!(p.stats.defense(), 5);
    }

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
