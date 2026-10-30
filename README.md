# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

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
