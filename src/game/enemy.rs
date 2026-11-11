//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Decision-making lives in `ai`; Game applies the results.
use crate::game::position::Position;
use crate::game::stats::Stats;
    
/// A single enemy instance in a Level.
pub struct Enemy {
    // TODO: id: usize, kind: EnemyType, position: Position, stats: Stats, ai: AiState
    kind: EnemyType,
    position: Position,
    stats: Stats,
    ai: AiState,
    flee_below_hp: i32,//non inclusive. aka if it is set to 10 flees at 9 and below
}

/// Enemy species. Each maps to an AiProfile that drives divergent behavior.
pub enum EnemyType {
	// TODO: Come up with some enemy types
    Goblin,
    Ghost,
    Slime,
}
#[derive(Debug)]
pub enum AiState {
    Attacking, 
    Fleeing,
    Idle,
}


impl Enemy{
    //Creates Enemy

    pub fn new(kind:EnemyType, position:Position) -> Enemy{
        match kind {
                EnemyType::Goblin => Enemy{
                    kind,
                    position,
                    stats : Stats::new(25, 3, 5),
                    ai:AiState::Idle,
                    flee_below_hp: 10,
                },
                EnemyType::Ghost => Enemy{
                    kind,
                    position,
                    stats : Stats::new(10, 2, 3),
                    ai:AiState::Idle,
                    flee_below_hp: 5,
                },
                EnemyType::Slime => Enemy{
                    kind,
                    position,
                    stats : Stats::new(5, 1, 2),
                    ai:AiState::Idle,
                    flee_below_hp: 0,//slimes are too dumb to run
                },
        }
    }

    // - 12 Get Current Enemy State: kind, position, stats, AI state
    //(Contract 12, Protocol 1-4)
    pub fn kind(&self) -> &EnemyType { &self.kind }
    pub fn position(&self) -> &Position { &self.position }
    pub fn stats(&self) -> &Stats { &self.stats }
    pub fn ai(&self) -> &AiState { &self.ai}


    // - 13 Enemy Update: update position and AI state
    //(Contract 13, Protocol 1)
    pub fn update_position(&mut self, new_position: Position){
        self.position = new_position;
    }

    //(Contract 13, Protocol 2) 
    //CRC card lists method header as fn update_status(&mut self, statusCode: i32, effect: String) this has changed to simplify
    pub fn update_status(&mut self){
        match self.ai{
            AiState::Attacking => self.ai = if self.stats.hp() >= self.flee_below_hp { AiState::Attacking } else { AiState::Fleeing },// continue attacking unless below a certain hp
            AiState::Fleeing => self.ai = AiState::Fleeing,//continue fleeing
            AiState::Idle => self.ai = if self.stats.hp() >= self.flee_below_hp { AiState::Attacking } else { AiState::Fleeing },
        }
    }
    //(Contract 13, Protocol 3)
    //added to implement checking status change if damage goes below fleeing threshold
    pub fn take_damage(&mut self, amount:i32){
        self.stats.update_health(amount);
        self.update_status();
    }

    // - 14 Combat Information: attack roll
    //(Contract 14, Protocol 1)
    pub fn get_roll(&self) -> i32{
        self.stats.attack()
    }
}

//Test Suggestions
#[cfg(test)]
mod tests {
    use std::assert_matches;
    use super::{Enemy, EnemyType, Position, AiState};
    //Test that get_roll returns the attack value from stats
    #[test]
    fn get_roll_returns_correct_attack() {
        let a = Enemy::new(EnemyType::Goblin, Position::new(4,3));
        assert_eq!(a.get_roll(), 3);
    }
    //Test that update_status changes ai Attacking 
    #[test]
    fn update_status_changes_to_attacking() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4,3));
        a.update_status();
        assert_matches!(a.ai(), AiState::Attacking);
    }
    //Test that take damage bellow a certain level runs update_status which then changes ai to Fleeing 
    #[test]
    fn take_damage_and_update_status_changes_from_idle_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4,3));
        a.take_damage(-16);//25-16=9 which is when Goblin should first begin to flee
        assert_eq!(a.stats().hp(),9);//check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing);//check ai state correctly updated
    }
    //Test that take damage bellow a certain level runs update_status which then changes ai to Fleeing 
    #[test]
    fn take_damage_and_update_status_changes_from_attacking_to_fleeing() {
        let mut a = Enemy::new(EnemyType::Goblin, Position::new(4,3));
        a.update_status();//change enemy state from Idle to Attacking
        assert_matches!(a.ai(), AiState::Attacking);//confirm it switches to attacking
        a.take_damage(-16);//25-16=9 which is when Goblin should first begin to flee
        assert_eq!(a.stats().hp(),9);//check hp correctly updated
        assert_matches!(a.ai(), AiState::Fleeing);//check ai state correctly updated to fleeing
    }
}