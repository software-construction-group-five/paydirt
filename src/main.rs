//! Paydirt binary entry point.
//!
//! Sets up the terminal, runs the main game loop, and restores the terminal on exit.
//! Loop shape: Renderer reads input (contract 21) -> Game::process_action (contract 2)
//! -> Renderer::render (contract 20).

use paydirt::game::level::generator::LevelGenerator;
use paydirt::tui::Renderer;
use paydirt::tui::theme::Theme;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

const THEME_PATH: &str = "config/theme.toml";

fn main() -> std::io::Result<()> {
    // Loaded before the terminal is taken over, so errors print normally.
    let theme = match Theme::load(THEME_PATH) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!("Could not load {THEME_PATH}: {error}");
            std::process::exit(1);
        }
    };

    // Enters raw mode and the alternate screen, and installs a panic hook
    // that restores the terminal if the game crashes.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, Renderer::new(theme));
    ratatui::restore();
    result
}

/// Temporary loop for viewing generated levels until Game is implemented.
fn run(terminal: &mut DefaultTerminal, renderer: Renderer) -> std::io::Result<()> {
    // TODO: move into Game once it owns the RNG.
    let mut rng = rand::rng();
    let mut depth = 0;
    let mut level = LevelGenerator::generate_level(depth, &mut rng);

    loop {
        terminal.draw(|frame| renderer.render(frame, level.get_map(), level.depth() as usize))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('r') => level = LevelGenerator::generate_level(depth, &mut rng),
                KeyCode::Char('>') => {
                    depth += 1;
                    level = LevelGenerator::generate_level(depth, &mut rng);
                }
                _ => {}
            },
            _ => {}
        }
    }
}
