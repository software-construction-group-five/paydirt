//! Menu-driven launcher using Game state and the TUI rendering/input APIs.
//!
//! Game owns navigation and level generation; the launcher owns terminal events.

use std::{io, path::Path};

use crossterm::event::{self, Event};
use paydirt::{
    game::{Game, screen::Screen},
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
            "Usage: paydirt\n\nMenu and generated-level preview. Loads config/theme.toml and config/keybinds.toml."
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

/// Draws Game's active screen and forwards configured input to its controller.
fn run(terminal: &mut DefaultTerminal, renderer: &Renderer, keybinds: &Keybinds) -> io::Result<()> {
    let mut game = Game::new();
    while !game.should_quit() {
        let help = if matches!(game.screen(), Screen::Playing) {
            keybinds.preview_help()
        } else {
            keybinds.help(game.screen())
        };
        let level = game.level().map(|level| {
            let mut view = LevelView::new(level.get_map());
            view.depth = Some(level.depth());
            view.player = Some(level.player_start());
            view
        });
        let view = RenderView {
            level,
            selected: game.menu_selection(),
            help: &help,
            ..Default::default()
        };
        terminal.draw(|frame| renderer.render(frame, game.screen(), &view))?;
        if let Event::Key(key) = event::read()? {
            if let Some(action) = keybinds.action(game.screen(), key) {
                game.process_action(action);
            } else if matches!(game.screen(), Screen::Playing) {
                match keybinds.preview_command(key) {
                    Some(PreviewCommand::Regenerate) => game.regenerate_preview(),
                    Some(PreviewCommand::NextLevel) => game.advance_preview(),
                    None => {}
                }
            }
        }
    }
    Ok(())
}
