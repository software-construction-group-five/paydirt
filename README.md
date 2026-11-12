# Paydirt — v1.0

A turn-based terminal roguelike about Paydirt Pete’s daring escape from the mine.
Explore generated dungeon floors, fight mine creatures, collect better equipment,
and manage your lamp as you descend deeper.

## Getting started

You need Rust and Cargo with support for the Rust 2024 edition, plus an interactive
terminal. Build and launch from the repository:

```sh
cargo run --release
```

Select **New Game** and press Enter. Press `?` on any screen to see its current
controls, or `q` / Ctrl+C to quit.

Configuration files are loaded from the source project’s `config/` directory,
whose location is recorded at build time. Keep that directory available when
running the compiled binary. Invalid configuration is reported before the
application takes over the terminal.

## What’s in v1.0

- Procedural floors with rectangular and circular rooms, corridors, doors,
  rocks, water, and stairs to the next depth.
- Fog of war with remembered terrain and lamp-dependent visibility.
- Slimes, ghosts, and goblins that detect, approach, attack, and flee.
- Dice-based combat, depth-scaled weapons, five helmet tiers, and rare Bleed,
  Weakness, and Lightning weapon effects.
- Inventory management, equipment swaps, healing potions, lamp fuel, and enemy drops.
- Main menu, pause overview, contextual controls, and a game-over screen with
  scrollable message history.
- Configurable keybindings, glyphs, and foreground colors.

## Default controls

| Screen | Keys | Action |
| --- | --- | --- |
| Any | `?` | Open or close contextual controls |
| Any | `q` / Ctrl+C | Quit |
| Main menu | Up/Down or `w`/`s`, Enter | Select and confirm |
| Playing | Arrow keys or WASD | Move; bump an enemy to attack |
| Playing | `.` | Wait one turn |
| Playing | `g` | Pick up an item at your position |
| Playing | `i` | Open inventory |
| Playing | `>` | Descend while standing on stairs |
| Playing | Esc | Pause |
| Inventory | Up/Down or `w`/`s` | Select an item or equipped slot |
| Inventory | `e` / `r` | Equip / unequip |
| Inventory | `u` / `d` | Use a consumable / drop an item |
| Inventory | `g` / `.` | Pick up / wait |
| Inventory | Esc / `i` | Return to play |
| Paused | Esc / Enter | Resume |
| Paused | `m` | Return to main menu |
| Game over | Up/Down, PageUp/PageDown | Scroll message history |
| Game over | Enter | Return to main menu |
| Controls | `?` / Esc / Enter | Return to the previous screen |

## Playing the game

### Exploration and turns

Movement is cardinal. Bumping a closed door opens it; moving through it takes
another action. Rocks and water are impassable. The starting room is free of
initial enemies and loot, and closed doors block distant sight.

Movement attempts, attacks, waiting, and inventory action attempts spend a turn,
even if blocked or rejected. Enemies act after your action, then status effects
tick. Opening menus, browsing inventory, pausing, reading controls, and scrolling
history spend no turns. Inventory actions can expose you to enemy attacks.

Standing on stairs does not descend automatically: press `>`. Successful descent
preserves health, equipment, inventory, effects, remaining fuel, and elapsed turns.
Player effects tick during the transition; enemies do not act on either floor
for a successful descent. Depth starts at zero.

### Lamp and visibility

You start with **200 turns of lamp fuel**. Each spent turn consumes one fuel.
Sight shrinks from a radius of 10 tiles at full fuel to 5 at 50 fuel and 1 at zero.
An empty lamp does not stop play. Explored terrain stays remembered, but enemies
and items appear only in current sight.

Pick up **Lamp Fuel**, select it in inventory, and press `u` to refill. A successful
refill leaves 199 fuel after the action’s cost. Each new game supplies one extra
fuel item; later floors supply one when fuel is below 150 before descent.

### Combat and equipment

Pete starts with **40 HP**, base defense of 1, and a rolled **2d4 Pickaxe**.
Shovels deal **2d4+2** base damage. Weapons receive a random depth-based bonus when
created and retain their rolls through transfers and floor changes. Each new
weapon has a 1% chance of Bleed, Weakness, or Lightning.

Helmets progress from **Rusty (+1)** through **Lead (+2)**, **Iron (+3)**,
**Steel (+4)**, and **Pete’s (+5)**, unlocking at depths 0, 3, 6, 9, and 12.
Enemies become more numerous and stronger species appear as you descend.
Defeated enemies can drop healing potions and equipment.

Carry up to **three weapons, one armor item, and five consumables**; equipped
items are separate. Equipping a carried item returns displaced equipment to the
pack. Health Potions restore up to **12 HP**. Using a potion at full health or fuel
at a full lamp keeps the item, but the attempt still spends a turn.

Death ends the run and shows the final depth, cause of death, and full session
message history. Returning to the menu or starting a new game resets the session.

## Configuration

- [`config/keybinds.toml`](config/keybinds.toml): per-screen bindings and global
  help/quit bindings. Use lists for multiple keys, `shift+x` or `X` for uppercase,
  and `plus` for `+`. Equivalent duplicate bindings are rejected.
- [`config/theme.toml`](config/theme.toml): terrain, enemy, and item glyphs and
  foreground colors. Backgrounds use the terminal default. `[tiles.unseen]`
  controls unexplored cells; an empty symbol leaves them blank.

The controls menu reflects configured bindings. Missing optional theme entries
use defaults.

## Development and validation

The game controller owns turns, combat, inventory, progression, and screen state.
Level generation and entity storage live under `src/game/level/`; terminal
rendering and input live under `src/tui/`. Rendering does not advance simulation.
See the [development report](docs/development-report.md) for project background.

Run the checks before review (offline commands require cached dependencies):

```sh
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
```

For a terminal smoke test, check New Game, movement, door opening, combat,
inventory actions, stairs, help, pause/resume, return to menu, restart, resizing,
and quit. Check that hidden entities stay hidden and that menus spend no turns.

This README describes the v1.0 release scope. The Cargo package metadata currently
reports version `0.1.0`.
