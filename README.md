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

## Entity placement

`Level` owns enemies with stable, floor-local `EnemyId`s and ground items as
existing `Item` objects paired with positions. Enemy insertion and movement require
walkable terrain without another enemy. Staying on the current tile succeeds.
`Game` must check the player's position before applying enemy movement; Level
does not own the player. `is_walkable()` checks terrain only.

Initial enemy and item spawns must pass `is_spawn_tile()`: only ordinary floor
outside the entire starting room is eligible. The generator retains the first
room's actual shape, including circular boundaries, rather than excluding a fixed
distance around the player. Stairs and blocked terrain are excluded. Generation
chooses distinct initial positions across enemies and items using the supplied RNG.
Populations are stored in Level; drawing them is the next implementation phase.

### Population tuning

Dungeon depth is zero-based. Species use their existing combat stats:

| Species | Minimum depth | Relative difficulty | Selection weight |
| --- | --- | --- | --- |
| Slime | 0 | 1 | 1 |
| Ghost | 2 | 2 | depth − 1 |
| Goblin | 4 | 3 | (depth − 3)² |

Species below their minimum depth have zero weight. Stronger species become more
common as depth increases, raising average difficulty while retaining variation.
Difficulty eventually approaches the strongest existing species; stats do not scale.
Profiles and placement rules live in `src/game/level/generator.rs`.

Each floor targets `4 + min(depth, 16)` enemies and
`3 + min(depth / 3, 5)` items. Candidates are shuffled once and consumed without
replacement. On cramped floors, reserve up to half the available tiles for items,
then place as many enemies and items as fit. Placement terminates with no retries.

Items are chosen uniformly from eligible entries in the current three-item catalog.
Catalog level one is available at dungeon depth zero; availability uses saturating
`depth + 1`. Identical depth and RNG seed reproduce terrain and population.

These spawn restrictions do not apply during gameplay: enemies may enter the
starting room and stairs. Dropped items may occupy any walkable tile and share
tiles with enemies or other items. Rejected insertions return ownership unchanged.
Ground-item indices identify current entries, not catalog items; removal shifts
later indices, so callers must rebuild selections after a removal.

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
