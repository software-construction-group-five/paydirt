# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

The phase 1 [combat and turn contracts](docs/combat-turns.md) define damage,
turn order, equipment drops, descent, and game-over message history. Scaled stats,
equipment modifiers, reusable weapon effects, enemy effect operations, and AI
intent types are implemented. Phase 2 adds pure damage calculation and controlled
enemy damage/effect updates through Level, reporting actual HP loss and death.
Phase 3 connects player bump attacks, surviving-target weapon effects, defeat
messages, immediate enemy removal, and rare equipment drops. PR12 adds enemy
decisions and one enemy AI phase per spent action. Enemy damage, scheduled effect
ticks, and game-over history controls remain unfinished.

## Theme configuration

Characters and foreground colors are defined in `config/theme.toml`. The TUI
provides `Theme::load(path)` to read it. Backgrounds always use the terminal
default. Rocks use dark gray and water uses blue.

`[tiles.unseen]` controls unexplored cells, defaulting to a very dark teal-gray
`.` (`#1b3033`). Set `symbol = ""` to leave them blank. Older themes without this
section retain blank unseen cells. This appearance never reveals hidden terrain
or entities.

Entity settings use `[enemies.goblin]`, `[enemies.ghost]`, `[enemies.slime]`,
`[items.weapon]`, `[items.armor]`, and `[items.consumable]`, each with `symbol` and
`color`. Missing entity entries use defaults, so existing themes remain valid.
Default glyphs are `g` (red Goblin), `G` (white Ghost), `s` (green Slime),
`)` (yellow weapon), `[` (cyan armor), and `!` (magenta consumable).

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
Populations are stored in Level and rendered when their tiles are in current sight.

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

Items are chosen uniformly from eligible entries exposed by `item::catalog()`.
The catalog currently contains Pickaxe, Leather Armor, and Health Potion.
Catalog level one is available at dungeon depth zero; availability uses saturating
`depth + 1`. Identical depth and RNG seed reproduce terrain and population.

These spawn restrictions do not apply during gameplay: enemies may enter the
starting room and stairs. Dropped items may occupy any walkable tile and share
tiles with enemies or other items. Rejected insertions return ownership unchanged.
Ground-item indices identify current entries, not catalog items; removal shifts
later indices, so callers must rebuild selections after a removal.

## Line of sight and fog of war

Each `Tile` owns `seen` (persistent exploration memory) and `visible` (current
line of sight), both initially false. `seen` replaces the unused `explored` field.
The launcher-facing Game controller initializes sight at the player start on New
Game. Call `Level::update_visibility(player_position)` after movement, floor entry,
or sight-blocking terrain changes. Updating sight spends no turn and never clears
previously seen tiles. Invalid origins clear current visibility but preserve memory.

Sight has no distance limit. Walls, rocks, and closed doors block sight; water,
floor, stairs, and open doors transmit it. The first opaque tile remains visible.
All eight adjacent tiles are visible regardless of blockage, revealing room corners.
Beyond that neighborhood, straight rays cannot pass diagonally through blocked corners. Entities do not
block sight. `Map::has_line_of_sight(origin, target)` queries geometry without
changing memory; `Map::is_visible(position)` reads the last visibility update.

`Level::visible_enemies()` and `visible_ground_items()` filter entities using their
current tile's visibility. Remembered terrain does not reveal entities outside
current sight. Ground-item filtering preserves storage indices for selection.
Full collections remain available for simulation independently of visibility.

The renderer fills unseen cells with the unseen glyph, retains seen terrain with its configured
appearance, and draws entities only on currently visible tiles. It applies these
rules on both gameplay and paused screens and does not update exploration itself.
Items render below enemies, and the player renders last when supplied. Multiple
items on one tile render in storage order, with the last item on top.

## Enemy AI (PR12 integration)

After each spent action, Game snapshots enemy IDs in insertion order and applies
one decision per living enemy. Moves, waits, bump attacks, door opening, and
blocked attempts all trigger this phase. Pause, menus, and redraws do not.

| Species | Detection range (Chebyshev tiles) | Flee below HP |
| --- | ---: | ---: |
| Slime | 3 | 0 |
| Ghost | 6 | 6 |
| Goblin | 5 | 9 |

Detection requires both range and terrain line of sight from the enemy to the
player. AI uses `Map::has_line_of_sight()`, including the eight-adjacent-tile
exception, rather than the player's cached visibility or exploration memory.
Walls, rocks, and closed doors block distant detection; water and open doors
transmit sight. Losing range or sight returns an enemy to Idle, without pursuit
memory. Opening a door can expose the player before that action's enemy phase.

Detected healthy enemies approach; injured enemies flee. Healing above the
exclusive threshold ends fleeing on the next decision. Enemies choose a legal
cardinal step that improves Manhattan distance, trying left, right, up, then down
for ties. If no improving step is legal, they wait. This is local movement, without
pathfinding or door opening. Game revalidates terrain, occupancy, and the player's
tile before moving. Attacks require cardinal adjacency; diagonal neighbors
approach instead of requesting PR12's original diagonal attack.

Enemy attack intents currently wait without applying damage. Automatic status
effect ticks, player death handling, and full combat turn integration remain work
for the next combat phases.

## TUI integration

The application loads `Theme` and `Keybinds` before setting up the terminal. Resolve
both config paths relative to the project root rather than the working directory.
Inside the application loop:

- The game controller supplies the active `Screen` and a borrowed `RenderView`.
- `Renderer::render(frame, screen, view)` draws without changing game state.
- `Keybinds::action(screen, key_event)` translates input into an `Action`.
- The game controller processes that action and provides updated display data.

The TUI does not construct levels, move players, validate movement, advance turns,
or decide screen transitions. The launcher calls `Renderer::enemy_entities()` and
`item_entities()` to build visible, themed entries and borrows those vectors in
`LevelView`. The renderer also checks tile visibility to prevent hidden entities
from leaking through custom display data. The launcher supplies the active player
position for drawing and camera focus.
`LevelView::focus` remains available as a fallback for views without a player.

## Level entity integration notes

These interfaces implement SDD contracts 8–11, 19, and 20. The SDD PDF describes
older signatures; this branch uses the following concrete ownership rules:

| Area | Implemented contract |
| --- | --- |
| Enemy identity | `EnemyId` is stable within one Level and never reused there; discard old IDs on floor replacement. |
| Enemy access | Borrow through `enemy()`, `enemy_at()`, and `enemies()`; move through validated `move_enemy()`, remove through `remove_enemy()`. |
| Ground items | Insert the existing Item; removal returns its owned GroundItem. Rebuild entry selections after removal shifts indices. |
| Generation | Caller supplies depth and RNG; returned floors already contain terrain, enemies, and items. |
| Visibility | `seen` replaces the unused `explored` tile field. Refresh current sight before rendering; entities use their tile's current visibility. |
| Presentation | Renderer consumes borrowed views and resolves themes without changing gameplay state. |

Game owns the Player and MessageLog, checks player/enemy collisions, and refreshes
sight after movement and door interactions. The launcher supplies the active player
position, health, and messages to the renderer. Bumping a closed door opens it
without moving; entering it takes another action. Bumping an enemy attacks it
without moving, rolling weapon damage and subtracting defense with a minimum
of one damage. New games equip a 2d4 Pickaxe; supported weapon dice are d4, d6,
and d12. Unarmed damage is 1d4. Player HP is 40; Slime/Ghost/Goblin HP is 8/14/24,
with damage rolls of 1d4/1d6/1d12 and defense of 0/1/2. Player base defense is 1,
and Leather Armor adds 1. All starting flat attack modifiers are zero. Damage
rolls use Game’s RNG; reading weapon data and rendering never roll dice.
Messages report actual damage, weapon effects, defeat, and drops using species
names without enemy IDs. Equipped item names appear in the status panel. Each defeated
enemy has a 5% chance to drop a weapon or armor from the highest unlocked catalog
tier (level no greater than saturating depth + 1). The current catalog only has
level-one equipment. Entering a defeated enemy's tile takes another move.
Movement attempts, door opening, and waiting report a spent turn, including blocked
attempts by a living player. Dead-player gameplay actions are ignored without
spending turns; pause/menu/quit remain available. Enemies do not retaliate yet,
and status effects do not tick automatically;
the full combat turn loop remains unfinished. PR12 does advance enemy movement
and behavior once per spent action. Entity display and visibility queries do not
spend turns.

Run `cargo fmt --check`, `cargo test --offline`, and
`cargo clippy --offline --all-targets -- -D warnings` before review. In a terminal,
check New Game, pause/resume, return to menu, restart, resize, and quit. On New Game,
only terrain in sight is revealed; a floor may initially show no entities because
the entire starting room is protected and closed doors block sight.
