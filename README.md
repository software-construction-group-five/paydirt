# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

## Theme configuration

Characters and foreground colors are defined in `config/theme.toml`. The TUI
provides `Theme::load(path)` to read it. Backgrounds always use the terminal
default. Rocks use dark gray and water uses blue.

`cargo run` opens the main menu. Select New Game to generate and display a floor.
Esc pauses, Esc or Enter resumes, and m returns to the main menu. The launcher
loads both configuration files relative to the project root.

## Keybindings

Edit `config/keybinds.toml` to change controls. Each action accepts a list
of keys. Bindings are scoped by screen, with a global section for quit.

## Level generation

`LevelGenerator::generate_level(depth: u32, rng: &mut impl Rng) -> Level` returns
an owned dungeon floor. Game supplies the RNG and decides when to request a new
floor.

- `get_map()` and `is_walkable(position)` provide terrain access. Map dimensions
  are read through `width()`/`height()`; `positions()` and `neighbors(position)`
  provide bounded iteration.
- `depth()`, `player_start()`, and `stairs_down()` provide progression data.

Doors appear only at entrances where narrow hallways meet rooms, and always
start closed. Closed doors block movement; Game must handle opening them.
Circular rooms occasionally contain one or two impassable
rocks (`O`, configured in `[tiles.rock]` in `config/theme.toml`). Rocks preserve
room centers, the starting safe area, and connectivity when doors are opened.
Some larger circular rooms (diameter at least nine tiles) contain rounded water
blobs, rendered as `~` (`[tiles.water]`). Water is impassable. Entire pools are
rejected if they would seal a route or cover protected starting/entrance tiles.
Stairs are placed after these decorations.

## TUI integration

The application loads `Theme` and `Keybinds` before setting up the terminal. Resolve
both config paths relative to the project root rather than the working directory.
Inside the application loop:

- The game controller supplies the active `Screen` and a borrowed `RenderView`.
- `Renderer::render(frame, screen, view)` draws without changing game state.
- `Keybinds::action(screen, key_event)` translates input into an `Action`.
- The game controller processes that action and provides updated display data.

The TUI does not construct levels, move players, validate movement, advance turns,
or decide screen transitions. Optional entity slices in `LevelView` provide
rendering hooks for enemies and floor items as those systems are implemented.
