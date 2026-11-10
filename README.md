# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

## Theme configuration

Characters and foreground colors are defined in `config/theme.toml`. The TUI
provides `Theme::load(path)` to read it. Backgrounds always use the terminal
default. Rocks use dark gray and water uses blue.

`cargo run` opens the main menu with Pete and the ASCII title. Select New Game
to generate a level, or Quit to exit. In the level view, `i` opens inventory and
`Esc` pauses. Inventory is empty until item handling is integrated. `Esc`/Enter
resumes from pause; `m` returns to the main menu. Controls are configurable.
The level view displays the starting player marker, depth, and terrain; movement
and combat remain unimplemented.
The preview loads both config files relative to the project root, so it can also
be launched from another working directory.

## Keybindings

Edit `config/keybinds.toml` to change controls. Each action
accepts a list of keys, with separate sections for each screen and a global
section for bindings such as quit. For example, `move_right = ["right", "d"]`
can be changed to `move_right = ["l"]`. Control hints use the configured keys.

The `[preview]` section configures `regenerate` (default `r`) and `next_level`
(default `>`). The preview also honors the global quit keys (`q` and `ctrl+c`).
Preview commands are active only in the level view, not in menus or inventory.

## Level generation

`LevelGenerator::generate_level(depth: u32, rng: &mut impl Rng) -> Level` returns
an owned dungeon floor. Game supplies the RNG and decides when to request a new
floor. The existing map preview now renders the returned Level's map.

- `get_map()` and `is_walkable(position)` provide terrain access.
- `depth()`, `player_start()`, and `stairs_down()` provide progression data.
- `get_enemies()` and `get_floor_items()` expose entity collections.
- `pending_spawns()` contains reserved enemy and item positions. These never
  overlap each other, the stairs, or the safe area around the player start.

Enemy and Item are still stubs on this branch, so their collections remain empty.
Once their constructors and catalog are merged, add population inside the level
module using the reserved positions and the caller's RNG. Store each Enemy with
its position and each ground Item as `(Item, Position)`, then clear the consumed
spawn plans. Spawn counts are provisional and depth-scaled with fixed caps.
Player construction and level transitions remain the Game controller's job.

Doors appear only at entrances where narrow hallways meet rooms, and always
start closed. Closed doors block movement; Game must handle opening them.
Circular rooms occasionally contain one or two impassable
rocks (`O`, configured in `[tiles.rock]` in `config/theme.toml`). Rocks preserve
room centers, the starting safe area, and connectivity when doors are opened.
Some larger circular rooms (diameter at least nine tiles) contain rounded water
blobs, rendered as `~` (`[tiles.water]`). Water is impassable. Entire pools are
rejected if they would seal a route or cover protected starting/entrance tiles.
Rocks, water, and doors are excluded from enemy and item spawn locations.
Stairs and entity spawn reservations are chosen after these decorations.

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

## Paydirt Struct Hierarchy

**Game** - the game (you lost btw). Keeps track of and orchestrates the whole shebang

```
Game
├── Screen (enum)
│   ├── MainMenu ── MainMenuState
│   ├── Playing
│   ├── Inventory ── InventoryState
│   ├── Paused
│   └── GameOver ── GameSummary
├── Player
│   ├── Position
│   ├── Stats
│   ├── Inventory
│   │   └── Item (many) ── ItemType
│   └── Equipment
│       └── Item (weapon, armor slots)
├── Level
│   ├── Map
│   │   └── Tile (many) ── TileType
│   ├── Enemy (many)
│   │   ├── Position
│   │   ├── Stats
│   │   ├── EnemyType
│   │   └── AiState (sleeping, idle, chasing, fleeing)
│   ├── Item + Position (on floor)
│   └── depth (int)
├── Action (enum)
├── MessageLog
└── RNG

LevelGenerator (creates `Level`)
Theme (maps characters to Tiles, enemies, and player) 
Renderer (reads `Game` state and displays it with Ratatui)
```

## Comments and Clarifications

- Combat and turns will be handled with functions, as they do not need to store any data really.
- Every time you see `Item` up there it's the same thing, just it gets used a lot.
- If you don't know what an enum is, look it up.
- `Action` is used to map keys to actions, so that a config can be used to change keybinds.
