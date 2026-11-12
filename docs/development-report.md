# Paydirt Development Report

**Date:** November 11, 2026  
**Code reviewed:** `feature/combat-turns`, commit `0bf82de`  
**Package:** `paydirt` 0.1.0, Rust edition 2024

## Project overview

Paydirt is a local, single-player terminal roguelike about Paydirt Pete exploring a mine. Players navigate procedurally generated floors, encounter enemies, collect equipment and consumables, and descend through increasingly populated levels. The interface uses configurable ASCII characters and colors.

The current branch integrates the core gameplay loop: starting a session, movement, exploration, doors, combat, inventory management, floor transitions, death, and restart. Further work concerns final item content, balance, manual acceptance checks, and documentation and release review. The implementation reviewed here should not be interpreted as an approved release or a merge into `main`.

## Technology and repository organization

The project is a Cargo package with a library and an executable. Its direct dependencies are Ratatui for terminal rendering, Crossterm for terminal events, Rand for random generation and combat rolls, and Serde and TOML for configuration parsing.

| Location | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Package metadata, dependency declarations, and resolved dependency versions. |
| `src/lib.rs` | Exposes the game and terminal-interface subsystems for reuse and testing. |
| `src/main.rs` | Loads configuration, manages terminal lifecycle, runs the event loop, and builds display views. |
| `src/game.rs` | Central controller, screen navigation, session ownership, player actions, and turn scheduling. |
| `src/game/` | Player, enemies, combat, statistics, items, inventory, equipment, levels, and supporting types. |
| `src/tui.rs`, `src/tui/` | Borrowed display models, renderer, screen layouts, themes, and input mapping. |
| `config/` | Editable theme and screen-specific keybindings. |
| `docs/` | Software Design Document, Coding Standards, SCM Plan, and this development report. |
| `README.md` | Running instructions, controls, implemented interfaces, and gameplay rules. |
| `TODO.html` | Completed capabilities, remaining tasks, and validation baseline. |

Tests are embedded alongside their implementation in Rust test modules, with a documentation test in the TUI module.

## Architecture and ownership

The code follows the SDD’s two-subsystem design: **Game Logic and Management** and **Terminal Interface**. Gameplay does not depend on Ratatui or Crossterm. The terminal subsystem depends on game types, while the executable coordinates both sides.

`Game` owns the current screen, player, active level, random-number generator, message log, selections, help origin, and final session summary. `Player` contains position, statistics, inventory, and equipment through composition. `Level` owns terrain, enemies, and ground items. An item has no position of its own; a ground item pairs an owned item with a position.

This structure preserves a single ownership path for each runtime object. Picking up, equipping, dropping, or swapping an item transfers the original copy, preserving custom damage dice and effects. The renderer borrows display data and does not move actors, change health, generate floors, or advance turns.

A normal application cycle is:

1. Build a borrowed `RenderView` from the controller’s state.
2. Draw the active screen through `Renderer`.
3. Translate a terminal key into a screen-specific `Action`.
4. Process the action through `Game::process_action()`.
5. Advance simulation if the action spent a turn, then render the updated state.

## Game controller and session lifecycle

`src/game.rs` implements the main coordination layer. Supporting `action.rs` and `screen.rs` define the commands and screen states shared with the interface.

The application begins at the main menu without generating a floor. Starting a game generates depth zero, creates a full-health player with an equipped Pickaxe, initializes visibility, and clears previous session information. The controller supports Playing, Inventory, Paused, GameOver, and Help screens in addition to MainMenu.

Help remembers the underlying screen so closing the overlay restores its inventory selection or history position. Pause and help freeze gameplay. Returning to the main menu clears the active session; restarting creates new player and floor state.

## Levels, generation, and entity storage

`src/game/level.rs` provides the active floor and validated entity operations. `level/map.rs` implements a bounded tile grid, and `level/generator.rs` constructs terrain and populations using a caller-supplied RNG.

Generated maps are 80 columns by 21 rows. The generator places rectangular and circular rooms, connects them with corridors, and adds closed doors at eligible room entrances. Circular rooms become more common with depth and may contain rocks or water. Obstacle placement protects navigation, room centers, and starting areas. Stairs are placed at a distant reachable location after terrain decoration.

Initial populations use shuffled eligible floor positions without replacement. Enemies and items start on distinct tiles outside the entire starting room, excluding stairs and blocked terrain. Enemy targets are `4 + min(depth, 16)`; item targets are `3 + min(depth / 3, 5)`. Cramped floors reserve item space and place only what fits.

Species unlock at zero-based depths 0, 2, and 4 for Slime, Ghost, and Goblin. Depth-dependent weights increasingly favor stronger species. Enemy statistics remain fixed by species. Catalog items unlock when their level is no greater than saturating `depth + 1`. Identical depth and RNG seed reproduce terrain and populations.

Enemies receive stable, floor-local `EnemyId` values that are never reused within that floor. Level validates enemy insertion and movement against terrain and other enemies; Game additionally protects the player’s tile. Ground items retain ownership of their original `Item` values and may share runtime tiles with other items or enemies. Removing an item shifts subsequent storage indices, so selections must be rebuilt.

## Visibility and exploration

Each map tile records current visibility and persistent exploration memory. Visibility is recalculated at floor entry, after movement, and after opening a door. Previously seen terrain remains available for rendering.

Sight has no distance limit. Walls, rocks, and closed doors block it; water, floor, stairs, and open doors transmit it. The first opaque tile remains visible. All eight neighboring cells are visible, while more distant rays cannot pass diagonally through blocked corners.

Entities appear only in current sight. Remembered terrain does not reveal enemies or items that have moved outside visibility. Rendering applies an additional visibility check even when callers supply custom entity views. Drawing order is terrain, ground items, enemies, and player.

## Player statistics, combat, and effects

`player.rs` manages player state and item operations. `stats.rs` provides health, base combat values, and status effects. `combat.rs` defines supported dice, defense calculations, combat outcomes, and equipment-drop selection.

| Actor | Maximum HP | Damage dice | Base defense |
| --- | ---: | --- | ---: |
| Player with starting Pickaxe | 40 | 2d4 | 1 |
| Slime | 8 | 1d4 | 0 |
| Ghost | 14 | 1d6 | 1 |
| Goblin | 24 | 1d12 | 2 |

Unarmed player damage is 1d4. Supported die sizes are d4, d6, and d12. Starting flat attack modifiers are zero. Armor adds to effective defense without rewriting base statistics. Attack damage is `max(1, rolled damage − effective defense)`, and messages report actual HP lost.

Moving into a cardinally adjacent enemy performs a bump attack without moving the player. Direct kills remove enemies before they can retaliate. Direct and effect kills share one defeat path, preventing duplicate removal or drops. Each defeated enemy has a 5% chance to drop equipment chosen uniformly from the highest unlocked catalog tier.

Weapon-effect specifications create independent status-effect instances. Actors can retain up to three effects; adding another replaces the oldest. Effects bypass defense, tick once per relevant turn, and expire after their final tick. The controller logs aggregate damage and individual expirations. The effect machinery is implemented, although the current catalog’s Pickaxe has no on-hit effect.

## Turn scheduling and enemy behavior

The controller uses one shared boundary for ordinary spent actions:

1. Resolve the player action and remove direct kills.
2. Process surviving enemies in stable ID order.
3. Tick player effects; stop immediately if the player dies.
4. Tick surviving enemy effects and resolve resulting defeats.

Movement, waiting, attacking, opening doors, and inventory attempts spend turns, including rejected attempts. Menu navigation, inventory selection, pause, help, history scrolling, and redraws do not. Newly applied weapon effects tick in the same turn, so an enemy killed by an effect may act before its effect phase. Lethal enemy damage stops later enemy actions and effect processing.

`enemy/ai.rs` separates decision-making from execution. AI proposes Wait, Move, or Attack; Game validates and applies the result.

| Species | Detection range, Chebyshev tiles | Flee when HP is below |
| --- | ---: | ---: |
| Slime | 3 | 0 |
| Ghost | 6 | 6 |
| Goblin | 5 | 9 |

Detection requires range and terrain line of sight. Healthy detected enemies approach, injured enemies flee, and undetected enemies return to Idle without pursuit memory. Movement chooses a legal cardinal step that improves Manhattan distance, with deterministic tie-breaking. Attacks require cardinal adjacency. Current AI uses local decisions without full pathfinding or door opening.

## Items, inventory, and equipment

`item.rs` defines item categories and immutable combat/healing data. `inventory.rs` stores carried copies, while `equipment.rs` provides weapon and armor slots.

| Current catalog item | Behavior |
| --- | --- |
| Pickaxe | Weapon dealing 2d4 damage. |
| Leather Armor | Adds one defense while equipped. |
| Health Potion | Restores up to 12 HP, capped at maximum health. |

All current entries are catalog level one. Carried limits are three weapons, one armor, and five consumables; equipped items are separate from those limits.

Inventory rows distinguish duplicate carried copies and equipped slots. Pickup takes the first ground item on the player’s tile after checking capacity. Equipping swaps the selected carried item into its slot and returns displaced equipment to the freed pack space. Unequipping checks capacity before changing ownership. Players can drop carried or equipped items directly onto their tile.

Successful potion use consumes exactly the selected copy. Full-health use retains the potion and still spends a turn. Failed transfers preserve ownership. Combat remains active during item attempts in the inventory screen, including attempts that fail.

## Progression, death, and message history

Players descend by pressing `>` while standing on stairs. Moving onto stairs alone does not transition. Depth increment is checked for overflow.

Accepted descent ticks player effects on the old floor before generating its replacement. A lethal tick prevents replacement and retains the old final depth. Survivors keep health, effects, inventory, and equipment and enter the next floor at its starting position with sight initialized. Neither floor runs an enemy phase or enemy-effect ticks during an accepted transition. Rejected descent runs an ordinary turn.

Lethal damage stores the final depth and cause, appends a death message, enters GameOver, and rejects further gameplay actions. `message_log.rs` retains the full session history while exposing the newest 50 entries for ordinary gameplay. GameOver supports line and page scrolling through full multiline history. Returning to the menu clears history and summary state.

## Terminal interface and configuration

The TUI defines `RenderView`, `LevelView`, `StatusView`, and inventory/entity display entries. Screen modules render the main menu, gameplay, inventory, pause/help overlays, and game-over results. Gameplay combines a player-centered map viewport, status information, and recent messages. Layouts adapt to available terminal space, with game-over history receiving priority on short terminals.

`theme.rs` loads configurable foreground colors and printable ASCII glyphs from `config/theme.toml`. Backgrounds use terminal defaults. Optional entity settings preserve compatibility with older themes. Unseen cells have a separate appearance that does not reveal hidden terrain.

`input.rs` parses global and screen-specific bindings from `config/keybinds.toml`, validates action names and keys, and rejects conflicts. The controls overlay derives its text from the actual configured bindings. Defaults include arrow keys or WASD for movement, `.` to wait, `i` for inventory, `g` for pickup, `>` for descent, `?` for help, and `q` or Ctrl+C to quit.

The launcher resolves configuration paths from the Cargo project root and validates both files before terminal initialization. It restores the terminal after the application loop returns; Ratatui supplies panic restoration support. Real-terminal cleanup checks remain part of manual validation.

## Testing and quality practices

At the reviewed commit, the following checks passed on November 11, 2026:

```sh
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
```

The test suite passed **193 tests**: 190 library tests, two executable tests, and one documentation test. Coverage includes generation reproducibility and connectivity, spawn restrictions, visibility, AI decisions, dice calculations, drops, effect timing, inventory ownership, rejected actions, descent and overflow, death handling, restart, help, configuration parsing, and rendering across small/resized test backends.

These results establish an automated baseline. They do not establish completion of a real-terminal playthrough, balance testing, release review, or customer acceptance.

Project standards call for default Rust formatting, Clippy checks, composition, single ownership, documented public APIs, descriptive commits, and feature-branch review. The SCM Plan specifies issue-linked pull requests, independent review, document review through Teams, and a configuration audit before baseline tagging.

## Remaining development work

The detailed task list is maintained in [TODO.html](../TODO.html). Remaining work is to:

- Agree on the final item catalog and implement approved additions, including any additional consumable behavior.
- Playtest early and deeper floors and tune combat, population, equipment, healing, and drops using recorded results.
- Review existing tests for gaps across a complete session and add focused deterministic integration coverage where needed.
- Record a full keyboard playthrough, configuration/theme checks, resize behavior, and terminal cleanup on normal and error exits.
- Reconcile older SDD signatures and ownership descriptions with the current implementation, then refresh developer documentation.
- Run checks after final changes and complete the project’s review, demonstration, and baseline audit process.

Current scope has local enemy movement, fixed species statistics, a three-item catalog, in-memory sessions, and downward progression without an implemented victory condition. Broader pathfinding, persistence, or a victory condition would require a scope decision rather than being assumed requirements.

## Project references

- [README and implemented interface notes](../README.md)
- [Completed work and remaining tasks](../TODO.html)
- [Software Design Document](sdd-report.pdf)
- [Coding Standards](Coding_Standards.pdf)
- [Software Configuration Management Plan](scm-paydirt.pdf)
