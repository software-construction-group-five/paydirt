//! Combat and health statistics shared by Player and Enemy.
//Derived from Player Contract 7 - Update Player Health

/// A damaging effect. Durations are measured in gameplay turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEffect {
    name: String,
    damage_per_turn: i32,
    turns_remaining: u32,
}

impl StatusEffect {
    /// Creates an effect with nonnegative damage and at least one turn.
    pub fn new(name: String, damage_per_turn: i32, turns: u32) -> Self {
        assert!(damage_per_turn >= 0, "Effect damage cannot be negative.");
        assert!(turns > 0, "Effect duration must be positive.");
        Self {
            name,
            damage_per_turn,
            turns_remaining: turns,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn damage_per_turn(&self) -> i32 {
        self.damage_per_turn
    }
    pub fn turns_remaining(&self) -> u32 {
        self.turns_remaining
    }
}

/// Health, combat values, and at most three active status effects.
pub struct Stats {
    hp: i32,
    max_hp: i32,
    attack: i32,
    defense: i32,
    status_effects: Vec<StatusEffect>,
}

impl Stats {
    pub fn new(max_hp: i32, attack: i32, defense: i32) -> Self {
        assert!(max_hp > 0, "Maximum health must be positive.");
        assert!(attack >= 0, "Attack cannot be negative.");
        assert!(defense >= 0, "Defense cannot be negative.");
        Self {
            hp: max_hp,
            max_hp,
            attack,
            defense,
            status_effects: Vec::new(),
        }
    }

    pub fn hp(&self) -> i32 {
        self.hp
    }
    pub fn max_hp(&self) -> i32 {
        self.max_hp
    }
    pub fn attack(&self) -> i32 {
        self.attack
    }
    pub fn defense(&self) -> i32 {
        self.defense
    }
    pub fn status_effects(&self) -> &[StatusEffect] {
        &self.status_effects
    }

    /// Positive amounts heal; negative amounts damage. Health stays in bounds.
    pub fn update_health(&mut self, amount: i32) {
        self.hp = self.hp.saturating_add(amount).clamp(0, self.max_hp);
    }

    /// Adds an independent effect, replacing the oldest at the three-effect limit.
    /// Effects with the same name stack as separate entries.
    pub fn add_status_effect(&mut self, effect: StatusEffect) {
        if self.status_effects.len() == 3 {
            self.status_effects.remove(0);
        }
        self.status_effects.push(effect);
    }

    /// Applies one turn of damage and removes effects after their final tick.
    /// Game must call this once per completed gameplay turn, not per redraw.
    /// Damage bypasses defense. Returns actual HP lost, limited by remaining HP.
    pub fn tick_status_effects(&mut self) -> i32 {
        let old_hp = self.hp;
        let mut damage: i32 = 0;
        for effect in &mut self.status_effects {
            damage = damage.saturating_add(effect.damage_per_turn);
            effect.turns_remaining -= 1;
        }
        self.update_health(-damage);
        self.status_effects
            .retain(|effect| effect.turns_remaining > 0);
        old_hp - self.hp
    }

    pub fn is_dead(&self) -> bool {
        self.hp == 0
    }
}

#[cfg(test)]
mod tests {
    use super::{Stats, StatusEffect};

    fn stats() -> Stats {
        Stats::new(100, 15, 5)
    }
    fn effect(name: &str, damage: i32, turns: u32) -> StatusEffect {
        StatusEffect::new(String::from(name), damage, turns)
    }

    #[test]
    fn new_stats_start_at_full_health() {
        let s = stats();
        assert_eq!(
            (s.hp(), s.max_hp(), s.attack(), s.defense()),
            (100, 100, 15, 5)
        );
        assert!(s.status_effects().is_empty());
        assert!(!s.is_dead());
    }

    #[test]
    fn damage_reduces_health() {
        let mut s = stats();
        s.update_health(-25);
        assert_eq!(s.hp(), 75);
    }

    #[test]
    fn healing_cannot_exceed_max_health() {
        let mut s = stats();
        s.update_health(-25);
        s.update_health(50);
        assert_eq!(s.hp(), 100);
    }

    #[test]
    fn fatal_damage_stops_at_zero_health() {
        let mut s = stats();
        s.update_health(-150);
        assert_eq!(s.hp(), 0);
        assert!(s.is_dead());
    }

    #[test]
    fn poison_damages_for_exactly_three_turns() {
        let mut s = stats();
        s.add_status_effect(effect("Poisoned", 5, 3));
        assert_eq!(s.hp(), 100); // Adding an effect does not tick it.
        assert_eq!(s.status_effects()[0].damage_per_turn(), 5);
        for remaining in [2, 1] {
            assert_eq!(s.tick_status_effects(), 5);
            assert_eq!(s.status_effects()[0].turns_remaining(), remaining);
        }
        assert_eq!(s.tick_status_effects(), 5);
        assert_eq!(s.hp(), 85);
        assert!(s.status_effects().is_empty());
        assert_eq!(s.tick_status_effects(), 0);
        assert_eq!(s.hp(), 85);
    }

    #[test]
    fn multiple_effects_tick_and_expire_independently() {
        let mut s = stats();
        s.add_status_effect(effect("Poisoned", 5, 3));
        s.add_status_effect(effect("Burning", 3, 1));
        assert_eq!(s.tick_status_effects(), 8);
        assert_eq!(s.hp(), 92);
        assert_eq!(s.status_effects().len(), 1);
        assert_eq!(s.status_effects()[0].name(), "Poisoned");
        assert_eq!(s.tick_status_effects(), 5);
        assert_eq!(s.hp(), 87);
    }

    #[test]
    fn fourth_effect_replaces_oldest() {
        let mut s = stats();
        for name in ["Poisoned", "Burning", "Slowed", "Weakened"] {
            s.add_status_effect(effect(name, 1, 2));
        }
        let names: Vec<&str> = s.status_effects().iter().map(|e| e.name()).collect();
        assert_eq!(names, vec!["Burning", "Slowed", "Weakened"]);
        assert_eq!(s.tick_status_effects(), 3);
    }

    #[test]
    fn effect_damage_can_kill_and_returns_actual_health_lost() {
        let mut s = stats();
        s.update_health(-98);
        s.add_status_effect(effect("Poisoned", 5, 1));
        assert_eq!(s.tick_status_effects(), 2);
        assert!(s.is_dead());
    }

    #[test]
    fn extreme_effect_damage_does_not_overflow() {
        let mut s = stats();
        s.add_status_effect(effect("Poisoned", i32::MAX, 1));
        s.add_status_effect(effect("Burning", i32::MAX, 1));
        assert_eq!(s.tick_status_effects(), 100);
        assert_eq!(s.hp(), 0);
    }

    #[test]
    fn empty_effect_list_does_nothing() {
        let mut s = stats();
        assert_eq!(s.tick_status_effects(), 0);
        assert_eq!(s.hp(), 100);
    }

    #[test]
    #[should_panic(expected = "Maximum health must be positive.")]
    fn zero_max_health_is_rejected() {
        Stats::new(0, 15, 5);
    }

    #[test]
    #[should_panic(expected = "Attack cannot be negative.")]
    fn negative_attack_is_rejected() {
        Stats::new(100, -1, 5);
    }

    #[test]
    #[should_panic(expected = "Defense cannot be negative.")]
    fn negative_defense_is_rejected() {
        Stats::new(100, 15, -1);
    }

    #[test]
    #[should_panic(expected = "Effect damage cannot be negative.")]
    fn negative_effect_damage_is_rejected() {
        effect("Poisoned", -1, 3);
    }

    #[test]
    #[should_panic(expected = "Effect duration must be positive.")]
    fn zero_effect_duration_is_rejected() {
        effect("Poisoned", 5, 0);
    }
}
