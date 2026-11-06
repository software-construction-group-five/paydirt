//! Paydirt binary entry point.
//!
//! Sets up the terminal, runs the main game loop, and restores the terminal on exit.
//! Loop shape: Renderer reads input (contract 21) -> Game::process_action (contract 2)
//! -> Renderer::render (contract 20).

fn main() {
    // TODO: init terminal, build Game + Renderer, run loop, restore terminal.
}
