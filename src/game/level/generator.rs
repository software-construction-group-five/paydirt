//! Procedural level construction (SDD 3.8).
//!
//! Contract 19 Generate Level: build a fully populated Level for a given depth.
//! Stateless between calls; Game decides when a level is needed.

/// Builds dungeon layouts and populates enemies and items.
pub struct LevelGenerator {
    // TODO: (stateless) or generation tuning parameters
}

// TODO: pub fn generate_level(depth: u32, rng: &mut _) -> Level
