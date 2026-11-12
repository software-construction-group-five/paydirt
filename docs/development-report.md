# Paydirt Development Report

**Date:** November 11, 2026  
**Code reviewed:** `feature/new-items`, working tree based on `34b9813`, including turn counting, lamp fuel, and conditional fuel generation<br>
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

This structure preserves a single ownership path for each runtime object. Picking up, equipping, dropping, or swapping an item transfers the original copy, preserving damage dice, rolled bonuses, and effects. The renderer borrows display data and does not move actors, change health, generate floors, or advance turns.

A normal application cycle is:

1. Build a borrowed `RenderView` from the controller’s state.
2. Draw the active screen through `Renderer`.
3. Translate a terminal key into a screen-specific `Action`.
4. Process the action through `Game::process_action()`.
5. Advance simulation if the action spent a turn, then render the updated state.

## Game controller and session lifecycle

`src/game.rs` implements the main coordination layer. Supporting `action.rs` and `screen.rs` define the commands and screen states shared with the interface.

The application begins at the main menu without generating a floor. Starting a game generates depth zero, creates a full-health player with an equipped Pickaxe, initializes visibility, and clears previous session information. The controller supports Playing, Inventory, Paused, GameOver, and Help screens in addition to MainMenu.

The pause panel shows PAUSED followed by a left-aligned overview of current depth, elapsed turns, cumulative enemies slain, and cumulative successful ground-item pickups. Direct and effect kills count once; reacquired dropped items count again, while starting gear and failed pickups do not. Counts persist across floors and reset with the session.

Help remembers the underlying screen so closing the overlay restores its inventory selection or history position. Pause and help freeze gameplay. Returning to the main menu clears the active session; restarting creates new player and floor state.

## Levels, generation, and entity storage

`src/game/level.rs` provides the active floor and validated entity operations. `level/map.rs` implements a bounded tile grid, and `level/generator.rs` constructs terrain and populations using a caller-supplied RNG.

Generated maps are 80 columns by 21 rows. The generator places rectangular and circular rooms, connects them with corridors, and adds closed doors at eligible room entrances. Circular rooms become more common with depth and may contain rocks or water. Obstacle placement protects navigation, room centers, and starting areas. Stairs are placed at a distant reachable location after terrain decoration.

Initial populations use shuffled eligible floor positions without replacement. Enemies and items start on distinct tiles outside the entire starting room, excluding stairs and blocked terrain. Enemy targets are `4 + min(depth, 16)`; item targets are `3 + min(depth / 3, 5)`. Cramped floors reserve item space and place only what fits.

Species unlock at zero-based depths 0, 2, and 4 for Slime, Ghost, and Goblin. Depth-dependent weights increasingly favor stronger species. Enemy statistics remain fixed by species. Catalog items unlock when their level is no greater than saturating `depth + 1`. Identical depth and RNG seed reproduce terrain and populations.

Enemies receive stable, floor-local `EnemyId` values that are never reused within that floor. Level validates enemy insertion and movement against terrain and other enemies; Game additionally protects the player’s tile. Ground items retain ownership of their original `Item` values and may share runtime tiles with other items or enemies. Removing an item shifts subsequent storage indices, so selections must be rebuilt.

## Visibility and exploration

Each map tile records current visibility and persistent exploration memory. Visibility is recalculated at floor entry and after every spent action, including waits and inventory attempts. Previously seen terrain remains available for rendering.

Player sight uses a fuel-dependent Chebyshev radius: linearly from 10 to 5 tiles as fuel decreases from 200 to 50, then from 5 to 1 as fuel decreases to zero. Tile centers outside the continuous radius are hidden. Game refreshes sight after burning fuel on every spent action, preventing movement or doors from revealing terrain at a stale radius. Previously seen terrain remains remembered beyond that range.

Walls, rocks, and closed doors block sight; water, floor, stairs, and open doors transmit it. The first opaque tile remains visible. All eight neighboring cells are visible, while more distant rays cannot pass diagonally through blocked corners. Exposed diagonal wall boundaries are revealed from visible transparent tiles immediately toward the viewer. This fixes missing boundary walls when viewing an open room without revealing terrain behind obstacles. The player’s range cap also applies to these boundary walls.

`Map::has_line_of_sight()` remains a terrain geometry query without the player’s lamp range cap. Player visibility applies the cap in `update_visibility()`, while enemy detection retains its own species range.

Entities appear only in current sight. Remembered terrain does not reveal enemies or items that have moved outside visibility. Rendering applies an additional visibility check even when callers supply custom entity views. Drawing order is terrain, ground items, enemies, and player.

## Player statistics, combat, and effects

`player.rs` manages player state and item operations. `stats.rs` provides health, base combat values, and status effects. `combat.rs` defines supported dice, defense calculations, combat outcomes, and equipment-drop selection.

| Actor | Maximum HP | Damage dice | Base defense |
| --- | ---: | --- | ---: |
| Player with starting Pickaxe | 40 | 2d4 plus rolled bonus | 1 |
| Slime | 8 | 1d4 | 0 |
| Ghost | 14 | 1d6 | 1 |
| Goblin | 24 | 1d12 | 2 |

Unarmed player damage is 1d4. Supported die sizes are d4, d6, and d12. Base actor attack modifiers are zero; equipped weapons add their rolled flat damage bonus. Weakness subtracts its power from the outgoing attack roll before defense. Armor adds to effective defense without rewriting base statistics. Attack damage is `max(1, weakened roll including weapon bonus − effective defense)`, and messages report actual HP lost.

Moving into a cardinally adjacent enemy performs a bump attack without moving the player. Direct kills remove enemies before they can retaliate. Direct and effect kills share one defeat path, preventing duplicate removal or drops. Each defeated enemy has a 5% chance to drop equipment. Successful drops select a helmet 5% of the time and a weapon 95% of the time, giving a 0.25% helmet chance per kill. Selection uses the highest unlocked tier within each category, preventing higher-level helmets from displacing weapon drops. Weapon drops roll their bonus and rare effect at the current floor depth.

Each newly spawned weapon has a **1%** chance to receive one on-hit effect, chosen equally from Bleed, Weakness, and Lightning. Effect power is one quarter of average base weapon damage, including the Shovel’s base bonus, rounded up with a minimum of one. It excludes the randomized depth bonus and the current attack roll; both current weapons have effect power two.

| Effect | Damage-text color | Behavior |
| --- | --- | --- |
| Bleed | Red | Deals fixed damage each turn for three effect phases, bypassing defense. |
| Weakness | Purple | Reduces outgoing attack rolls by fixed power for three effect phases; ordinary defense and minimum-one damage still apply. |
| Lightning | Electric blue | Immediately deals fixed damage to other living enemies within Chebyshev distance two of the struck target and with terrain line of sight from the impact. |

Bleed and Weakness attach only to surviving targets and create independent status instances. Actors can retain up to three active effects; adding another replaces the oldest. Repeated effects stack independently and expire after their final tick. The controller logs aggregate effect damage and individual expirations. Weakness applies before an enemy’s retaliation on the hit turn.

Lightning also triggers when the primary hit kills. Splash bypasses defense, does not chain, and does not occupy a status slot or run later ticks. Splash kills use the shared immediate removal and once-only equipment-drop path.

## Turn scheduling and enemy behavior

The controller uses one shared boundary for ordinary spent actions:

1. Resolve the player action and remove direct kills.
2. Process surviving enemies in stable ID order.
3. Tick player effects; stop immediately if the player dies.
4. Tick surviving enemy effects and resolve resulting defeats.

Movement, waiting, attacking, opening doors, and inventory attempts spend turns, including rejected attempts. Menu navigation, inventory selection, pause, help, history scrolling, and redraws do not. Newly applied Bleed and Weakness tick in the same turn, so an enemy killed by Bleed may act before its effect phase. Lightning resolves during the player action, removing splash kills before the enemy phase. Lethal enemy damage stops later enemy actions and effect processing.

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
| Pickaxe | Base weapon damage 2d4, plus a rolled bonus. |
| Shovel | Base weapon damage 2d4+2, plus a rolled bonus. |
| Rusty Helmet | +1 defense; first available at depth 0 (catalog level 1). Replaces Leather Armor at the same index and stats. |
| Lead Helmet | +2 defense; first available at depth 3 (catalog level 4). |
| Iron Helmet | +3 defense; first available at depth 6 (catalog level 7). |
| Steel Helmet | +4 defense; first available at depth 9 (catalog level 10). |
| Pete's Helmet | +5 defense; first available at depth 12 (catalog level 13). |
| Health Potion | Restores up to 12 HP, capped at maximum health. |
| Lamp Fuel | Refills the lamp to 200; separate guaranteed floor supply, excluded from random loot and drops. |

New weapons roll once when created as starting equipment, generated floor loot, or enemy drops. With `tier = min(depth / 4, 1000)`, the added bonus is `tier + random(0..=2 + tier)`, inclusive. A Pickaxe therefore starts at 2d4 plus zero to two damage at depth zero; a Shovel adds its base +2 as well. The cap keeps scaling bounded at extreme depths. Attack dice continue to roll independently on each hit, while transfers and descent preserve the original weapon’s rolled data.

Pickaxe, Shovel, Health Potion, and Rusty Helmet remain catalog level one. Floor loot selects helmets with a fixed 5% probability; the remaining 95% is split uniformly among the weapons and potion. Only the best unlocked helmet tier spawns, keeping armor rare as the catalog grows and avoiding obsolete helmet loot. Helmets have fixed bonuses and use the existing armor slot. The maximum helmet bonus is +5, giving 6 total defense with the player's base defense of 1. Carried limits are three weapons, one armor, and five consumables; equipped items are separate from those limits.

Inventory rows distinguish duplicate carried copies and equipped slots. Pickup takes the first ground item on the player’s tile after checking capacity. Equipping swaps the selected carried item into its slot and returns displaced equipment to the freed pack space. Unequipping checks capacity before changing ownership. Players can drop carried or equipped items directly onto their tile.

Successful potion use consumes exactly the selected copy. Full-health use retains the potion and still spends a turn. Failed transfers preserve ownership. Combat remains active during item attempts in the inventory screen, including attempts that fail.

## Turn counter and lamp lifecycle

Player Stats starts with zero elapsed turns and 200 lamp fuel. Each spent action
increments the saturating counter and reduces fuel by one, including rejected
attempts and lethal actions. Accepted descent counts once through its special
transition path. Menus, help, pause, selection, and history remain free. At zero
fuel, the player retains one-tile sight and can continue playing.

Lamp Fuel uses the existing pickup and consumable-use controls and shares the
five-consumable capacity. Successful use consumes the selected copy, restores
fuel to 200, and then burns one for the action (199 remains). A full lamp keeps
the item on rejection, while the attempt still spends a turn. Health and elapsed
turns are preserved. Both gameplay and inventory status show turns and fuel.

Generation reserves one extra fuel spawn before ordinary population placement,
with no random fuel spawns or enemy fuel drops. The starting floor always has
one. On descent, Game requests one if fuel was below 150 before the action;
exactly 150 or more requests none. Fuel normally uses a distinct eligible tile
outside the starting room; a floor without eligible tiles uses the safe player
start to preserve the guarantee. Regular item targets are unchanged, with fuel
additional to them. Survivors retain the counter and remaining fuel on descent;
new sessions reset both.

The historical SDD PDF remains the original reviewed design. Its contracts 6–7,
18–20 now additionally correspond to these Stats fields, fuel use, the appended
catalog index 8, conditional generation, and lamp-aware borrowed status views.
The README documents the current interfaces pending the formal SDD review.

## Progression, death, and message history

Players descend by pressing `>` while standing on stairs. Moving onto stairs alone does not transition. Depth increment is checked for overflow.

Accepted descent ticks player effects on the old floor before generating its replacement. A lethal tick prevents replacement and retains the old final depth. Survivors keep health, effects, inventory, and equipment and enter the next floor at its starting position with sight initialized. Neither floor runs an enemy phase or enemy-effect ticks during an accepted transition. Rejected descent runs an ordinary turn.

Lethal damage stores the final depth and cause, appends a death message, enters GameOver, and rejects further gameplay actions. `message_log.rs` retains the full session history while exposing the newest 50 entries for ordinary gameplay. GameOver supports line and page scrolling through full multiline history. Returning to the menu clears history and summary state.

## Terminal interface and configuration

The TUI defines `RenderView`, `LevelView`, `StatusView`, and inventory/entity display entries. Screen modules render the main menu, gameplay, inventory, pause/help overlays, and game-over results. Gameplay combines a player-centered map viewport, status information, and recent messages. Layouts adapt to available terminal space, with game-over history receiving priority on short terminals. Inventory and equipped status labels include the full damage expression and any rare effect name, such as `Shovel (2d4+4 Weakness)`. Only the parenthesized damage text receives the effect color.

`theme.rs` loads configurable foreground colors and printable ASCII glyphs from `config/theme.toml`. Backgrounds use terminal defaults. Optional entity settings preserve compatibility with older themes. Unseen cells have a separate appearance that does not reveal hidden terrain.

`input.rs` parses global and screen-specific bindings from `config/keybinds.toml`, validates action names and keys, and rejects conflicts. The controls overlay derives its text from the actual configured bindings. Defaults include arrow keys or WASD for movement, `.` to wait, `i` for inventory, `g` for pickup, `>` for descent, `?` for help, and `q` or Ctrl+C to quit.

The launcher resolves configuration paths from the Cargo project root and validates both files before terminal initialization. It restores the terminal after the application loop returns; Ratatui supplies panic restoration support. Real-terminal cleanup checks remain part of manual validation.

## Testing and quality practices

For the reviewed working tree, the following checks passed on November 11, 2026:

```sh
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
```

The test suite passed **215 tests**: 212 library tests, two executable tests, and one documentation test. Coverage includes generation reproducibility and connectivity, spawn restrictions, visibility, AI decisions, dice calculations, drops, effect timing, inventory ownership, rejected actions, descent and overflow, death handling, restart, help, configuration parsing, and rendering across small/resized test backends.

Three new visibility regressions cover diagonal boundary walls in square and rectangular rooms, hidden terrain behind diagonal walls, and the vision-range boundary for floors and walls with exploration memory retained after moving away. Movement and door-opening tests now verify visibility against both terrain line of sight and the player’s lamp range cap.

Six additional weapon/effect regressions cover deterministic depth-scaled rolls and rare-effect distribution, Bleed and Weakness duration, Weakness damage reduction on the hit turn, Lightning splash and kill handling, rolled-weapon ownership through transfers, and damage-text colors. Launcher assertions now accommodate randomized starting weapon labels and wrapping effect names.

Six lamp regressions cover the fuel curve and saturation, spent/free actions and reset, the pre-descent 150 threshold, full-lamp rejection and refueling ownership, exact conditional generation across depths and seeds, and shrinking visibility with exploration memory retained.

Three helmet regressions verify catalog names, stats and equipped defense, depth unlock boundaries and floor-loot rarity, and equipment-drop rarity across all helmet tiers and maximum depth. Existing inventory tests now expect Rusty Helmet.

These results establish an automated baseline. They do not establish completion of a real-terminal playthrough, balance testing, release review, or customer acceptance.

Project standards call for default Rust formatting, Clippy checks, composition, single ownership, documented public APIs, descriptive commits, and feature-branch review. The SCM Plan specifies issue-linked pull requests, independent review, document review through Teams, and a configuration audit before baseline tagging.

## Follow-up code review and maintenance pass

The follow-up pass reviewed controller scheduling, combat/effects, inventory transfers,
generation, visibility, configuration, and terminal presentation against the README,
this report, and the design and coding standards documents.

- Fixed uppercase bindings that failed when terminals omitted the Shift flag, and
  extended shifted punctuation normalization beyond `>` and `?`. Equivalent aliases
  now share conflict detection; Ctrl and Alt remain distinct.
- Fixed premature integer rounding in rare-effect power for supported custom dice
  pools. For example, 5d4 averages 12.5 damage and now yields power 4 rather than 3.
  The catalog's Pickaxe and Shovel still yield power 2.
- Removed the two temporary loot-selection vectors while preserving pool ordering,
  category probabilities, and RNG call order. Added empty, fuel-only, and single-category
  coverage alongside the existing distribution tests.
- Skipped effect snapshots and log formatting for actors without active effects.
  Existing timing, death, and descent tests cover the shared turn boundary.
- Reused the underlying screen when building launcher views and corrected stale TUI
  ownership comments.

The input and effect-power regressions failed before the fixes and passed afterward.
These changes do not include a performance benchmark or a real-terminal acceptance
playthrough; automated rendering checks use Ratatui's test backend.

## Remaining development work

The detailed task list is maintained in [TODO.html](../TODO.html). Remaining work is to:

- Review the expanded nine-item catalog and decide whether further content or consumable behavior belongs in this increment. Shovel, weapon rolls, Bleed, Weakness, and Lightning, and five helmet tiers are implemented.
- Playtest early and deeper floors and tune combat, depth-scaled weapon bonuses, the 1% effect chance, effect power/duration/radius, helmet unlock depths and rarity, population, healing, and drops using recorded results.
- Review existing tests for gaps across a complete session and add focused deterministic integration coverage where needed.
- Record a full keyboard playthrough, configuration/theme checks, resize behavior, and terminal cleanup on normal and error exits.
- Reconcile older SDD signatures and ownership descriptions with the current implementation, then refresh developer documentation.
- Run checks after final changes and complete the project’s review, demonstration, and baseline audit process.

Current scope has local enemy movement, fixed species statistics, a nine-item catalog, in-memory sessions, and downward progression without an implemented victory condition. Broader pathfinding, persistence, or a victory condition would require a scope decision rather than being assumed requirements.

## Project references

- [README and implemented interface notes](../README.md)
- [Completed work and remaining tasks](../TODO.html)
- [Software Design Document](sdd-report.pdf)
- [Coding Standards](Coding_Standards.pdf)
- [Software Configuration Management Plan](scm-paydirt.pdf)
