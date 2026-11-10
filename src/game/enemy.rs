//! Enemies (SDD 3.5).
//!
//! Contracts:
//! - 12 Get Current Enemy State: position, AI state, health
//! - 13 Enemy Update: update position and AI state
//! - 14 Combat Information: attack roll
//!
//! Decision-making lives in `ai`; Game applies the results.
mod position;
mod stats;


//use enemy::ai::{AiState, AiProfile, Intent};
// pub mod stats::Stats;
// pub mod position::Position;
// pub mod ai::AiState;
    
/// A single enemy instance in a Level.
pub struct Enemy {
    // TODO: id: usize, kind: EnemyType, position: Position, stats: Stats, ai: AiState
    kind: EnemyType,
    position: position::Position,
    stats: stats::Stats,
    ai: AiState,
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

    pub fn new(kind:EnemyType, position:position::Position, stats: stats::Stats, ai: AiState) -> Enemy{
        Enemy{
            kind,
            position,
            stats,
            ai,
        }
    }

    // - 12 Get Current Enemy State: position, AI state, health
    // returns enemyID as currently the file structure does not allow for returning the enemy (Contract 12, Protocol 1)
    pub fn get_enemy(self, _enemyID: i32) -> Enemy {//underscore because enemyID is currently unsused
        self
    }

    // - 13 Enemy Update: update position and AI state
    //(Contract 13, Protocol 1)
    pub fn update_position(&mut self, newPosition: position::Position){
        self.position = newPosition;
    }

    //(Contract 13, Protocol 2) 
    //CRC card lists method header as fn update_status(&mut self, statusCode: i32, effect: String) this is handled in ai state now
    pub fn update_status(&mut self){
        //add a method in ai state to check the different factors and update the ai state then call from here
        //self.ai.update(); 
    }

    // - 14 Combat Information: attack roll
    //(Contract 14, Protocol 1)
    pub fn get_roll(&self) -> i32{
        self.stats.attack()
    }
}

//Test Suggestions

//Test that get_roll returns the attack value from stats

//Test that get_enemy returns the correct enemy