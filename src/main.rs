//! Generated-level viewer using the TUI rendering and configured input APIs.
//!
//! TODO(team): replace this viewer with Game's loop when the controller is ready.

use std::{io, path::Path};

use crossterm::event::{self, Event};
use paydirt::{
    game::{level::generator::LevelGenerator, screen::Screen},
    tui::{
        LevelView, RenderView, Renderer,
        input::{Keybinds, PreviewCommand},
        theme::Theme,
    },
};
use ratatui::DefaultTerminal;

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if matches!(args.as_slice(), [flag] if flag == "--help" || flag == "-h") {
        println!(
            "Usage: paydirt\n\nGenerated-level viewer. Loads config/theme.toml and config/keybinds.toml."
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: paydirt; use --help for details",
        ));
    }
    // Validate both configs before raw mode, independently of the launch directory.
    let config_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
    let renderer = Renderer::new(Theme::load(config_dir.join("theme.toml"))?);
    let keybinds = Keybinds::load(config_dir.join("keybinds.toml"))?;
    // Ratatui's panic hook also restores the terminal if drawing panics.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &renderer, &keybinds);
    ratatui::restore();
    result
}

/// Shows generated terrain and the start marker without implementing gameplay.
fn run(terminal: &mut DefaultTerminal, renderer: &Renderer, keybinds: &Keybinds) -> io::Result<()> {
    let mut rng = rand::rng();
    let mut level = LevelGenerator::generate_level(0, &mut rng);
    let help = keybinds.preview_help();
    loop {
        let mut level_view = LevelView::new(level.get_map());
        level_view.depth = Some(level.depth());
        level_view.player = Some(level.player_start());
        let view = RenderView {
            level: Some(level_view),
            help: &help,
            ..Default::default()
        };
        terminal.draw(|frame| renderer.render(frame, &Screen::Playing, &view))?;
        if let Event::Key(key) = event::read()? {
            match keybinds.preview_command(key) {
                Some(PreviewCommand::Quit) => return Ok(()),
                Some(PreviewCommand::Regenerate) => {
                    level = LevelGenerator::generate_level(level.depth(), &mut rng);
                }
                Some(PreviewCommand::NextLevel) => {
                    level =
                        LevelGenerator::generate_level(level.depth().saturating_add(1), &mut rng);
                }
                None => {}
            }
        }
    }
}
