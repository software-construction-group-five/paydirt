# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

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
start closed. Closed doors block movement; Game must handle opening them. Circular rooms occasionally contain one or two impassable
rocks (`O`, configurable as `tiles.rock` in `config/theme.toml`). Rocks preserve
room centers, the starting safe area, and connectivity when doors are opened.
Some larger circular rooms (diameter at least nine tiles) contain rounded water
blobs, rendered as `~` (`tiles.water`). Water is impassable. Entire pools are
rejected if they would seal a route or cover protected starting/entrance tiles.
Rocks, water, and doors are excluded from enemy and item spawn locations.
Stairs and entity spawn reservations are chosen after these decorations.

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
