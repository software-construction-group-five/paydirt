//! Subsystem: Game Logic & Management.
//!
//! Owns all game state and gameplay rules. Provides SDD subsystem contracts:
//! 1. Game Action Processing
//! 2. Game State Access

pub mod action;
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

/// Main controller of Paydirt (SDD 3.2).
///
/// Contracts:
/// - 1 InitializeGame: create Player, request starting Level
/// - 2 ProcessAction: apply player Action, advance turn, check game over / level complete
/// - 3 ChangeScreen: transition between Screen variants
/// - 4 AdvanceLevel: request next Level, preserve Player
/// - 5 EndGame: build GameSummary, switch to GameOver
pub struct Game {
    // TODO: screen: Screen, player: Player, level: Level, depth: u32,
    //       log: MessageLog, rng: <seeded RNG>
}
