# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

## Theme configuration

Characters and foreground colors are defined in `config/theme.toml`. The TUI
provides `Theme::load(path)` to read it. Backgrounds always use the terminal default.

The game controller and application loop are not implemented yet. `cargo run`
currently runs the entry-point stub; it does not launch a playable session.

## Keybindings

Edit `config/keybinds.toml` to change controls. Each action
accepts a list of keys, with separate sections for each screen and a global
section for bindings such as quit. For example, `move_right = ["right", "d"]`
can be changed to `move_right = ["l"]`. Control hints use the configured keys.

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
