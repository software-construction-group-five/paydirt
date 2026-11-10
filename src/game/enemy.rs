//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Decision-making lives in `ai`; Game applies the results.
use crate::game::position::Position;
use crate::game::stats::{Stats, StatusEffect};
    
/// A single enemy instance in a Level.
pub struct Enemy {
    // TODO: id: usize, kind: EnemyType, position: Position, stats: Stats, ai: AiState
    kind: EnemyType,
    position: Position,
    stats: Stats,
    ai: AiState,
    flee_below_hp: i32,
}

/// Enemy species. Each maps to an AiProfile that drives divergent behavior.
pub enum EnemyType {
	// TODO: Come up with some enemy types
    Goblin,
    Ghost,
    Slime,
}

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
    pub fn update_position(&mut self, newPosition: Position){
        self.position = newPosition;
    }

    //(Contract 13, Protocol 2) 
    //CRC card lists method header as fn update_status(&mut self, statusCode: i32, effect: String) this is handled in ai state now
    pub fn update_status(&mut self){
        match self.ai{
            AiState::Attacking => self.ai = AiState::Attacking,// continue attacking
            AiState::Fleeing => self.ai = AiState::Fleeing,//continue fleeing
            AiState::Idle => self.ai = if self.stats.hp() >= self.flee_below_hp { AiState::Attacking } else { AiState::Fleeing },
        }
    }

    // - 14 Combat Information: attack roll
    //(Contract 14, Protocol 1)
    pub fn get_roll(&self) -> i32{
        self.stats.attack()
    }
}

//Test Suggestions

//Test that get_roll returns the attack value from stats

//Test that update_status changes ai to fleeing when actual hp below flee_below_hp