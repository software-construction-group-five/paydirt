# Paydirt

A terminal roguelike about Paydirt Pete's daring escape from the mine

Combat uses dice-based player and enemy attacks, weapon status effects, immediate
enemy removal, rare equipment drops, and a shared turn loop. Death stops the turn
and shows final depth, cause, and scrollable full message history. Inventory
controls support pickup, drop, equipment swaps, and healing potions. Stairs support progression between generated floors.

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
Esc pauses, Esc or Enter resumes, and m returns to the main menu.
The pause panel begins with `PAUSED`, followed by a left-aligned session overview:
current depth, elapsed turns, enemies slain (including effect kills), and items acquired
(successful pickups, including reacquired drops; starting gear is excluded).
These counts persist across floors and reset for each new session. The launcher
loads both configuration files relative to the project root.

## Keybindings

Edit `config/keybinds.toml` to change controls. Each action accepts a list
of keys. Bindings are scoped by screen, with a global section for quit.

Press `?` from any screen to open the controls menu. It uses the same centered
panel, colors, and backdrop as the pause overlay, sized to its text. The menu
shows the current screen's actual configured bindings, including remaps. Press
`?`, Esc, or Enter to close it and restore that screen. Combat and effects stay
frozen; inventory selection and game-over history position are preserved.
Controls are shown here instead of inline footers.

## Inventory and game-over controls

| Screen | Default key | Action |
| --- | --- | --- |
| Any screen | `?` | Open or close the controls menu without spending a turn |
| Help | Esc / Enter | Return to the previous screen |
| Playing | `>` | Descend while standing on stairs |
| Playing | `i` | Open inventory without spending a turn |
| Playing / Inventory | `g` | Pick up the first ground item at your tile |
| Playing / Inventory | `.` | Wait one turn without moving |
| Inventory | Up/Down or W/S | Select a carried entry or equipped slot |
| Inventory | `e` / `r` | Equip a carried item / unequip the selected slot |
| Inventory | `u` / `d` | Use a carried consumable / drop the selected item |
| Inventory | Esc / `i` | Return to play without spending a turn |
| GameOver | Up/Down | Scroll one history line older/newer |
| GameOver | PageUp/PageDown | Scroll ten history lines older/newer |
| GameOver | Enter | Return to the main menu |

Inventory shows carried copies first, then occupied weapon and armor slots.
Duplicate copies have separate selections. Transfers preserve original item data,
including damage dice and weapon effects. A swap returns displaced equipment to
the pack, even when its category was full; removing the selected carried copy
frees that space. Unequip checks capacity before changing anything. Carried limits
remain three weapons, one armor, and five consumables; equipped items are separate.
Dropping an equipped item directly restores its unequipped stats.

Health Potions restore up to **12 HP**, capped at maximum health, and consume one
selected copy on success. At full health the potion is kept. Pickup, drop, equip,
unequip, and use attempts all spend a turn, including full categories, empty
selections, wrong item types, or full-health rejections. Enemies and effects can
therefore damage or kill you while the inventory is open. The inventory displays
current health, equipment, and recent results; opening, closing, and selecting
rows spend no turns. Controls and help use the same configurable bindings.

GameOver starts at the newest messages and can scroll the full retained history,
including multiline entries older than the recent 50-message window. Scrolling
spends no turns and consumes no RNG. On short terminals, history gets priority
over the summary. Returning to menu or starting a new session clears selections,
scroll position, history, and summary.

## Turns, lamp fuel, and refueling

Player `Stats` records elapsed spent turns and lamp fuel. New sessions start at
zero turns with **200 turns of fuel**. Every spent action increments the counter
and burns one fuel, including blocked moves, failed inventory attempts, lethal
actions, and accepted or rejected descent. Menus, selection, help, pause, and
history scrolling spend none. Empty lamps stay at zero without stopping play.
The status panel shows elapsed turns and remaining fuel in play and inventory.

The lamp's Chebyshev sight radius decreases linearly in two segments:

| Fuel remaining | Sight radius |
| ---: | ---: |
| 200 | 10 |
| 50 | 5 |
| 0 | 1 |

For fuel `f >= 50`, radius is `5 + (f - 50) / 30`; below 50 it is
`1 + 4 * f / 50`. Tile centers must fall within this continuous radius, so the
visible tile boundary changes in whole-tile steps. Sight refreshes after every
spent action using the remaining fuel, including while stationary. Exploration
memory persists; enemy detection keeps its own range and terrain line of sight.

**Lamp Fuel** is a consumable: pick it up with `g`, select it in inventory, and
press `u` to refill to 200. It shares the five-consumable carried limit with
potions. A full lamp rejects use and keeps the fuel; the attempt still spends a
turn. Successful use burns its action's fuel after refilling, leaving **199**.
Refueling preserves health and does not reset elapsed turns.

Every new game generates **exactly one extra fuel item**. On descent, generate
one only if fuel is **below 150 before spending the descent turn**; with 150 or
more, generate none. Fuel is separate from ordinary random loot and enemy drops.
It reserves a distinct eligible tile before ordinary populations are placed; if
no eligible tile exists, it falls back to the walkable player start. Accepted
descent preserves remaining fuel and the turn counter, and lethal transition
effects prevent generation as before.

## Level generation

`LevelGenerator::generate_level(depth: u32, rng: &mut impl Rng) -> Level` returns
an owned dungeon floor with one extra fuel item. Game supplies the RNG and decides
when to request a new floor. For descent it calls
`generate_level_with_fuel(depth, spawn_fuel, rng)` with the threshold decision.

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
`3 + min(depth / 3, 5)` ordinary items, plus the conditional fuel supply. Candidates are shuffled once and consumed without
replacement. On cramped floors, reserve up to half the available tiles for items,
then place as many enemies and items as fit. Placement terminates with no retries.

Items come from eligible entries exposed by `item::catalog()`. Floor loot has a
5% helmet chance, independent of the number of unlocked helmets. Otherwise it
chooses uniformly among Pickaxe, Shovel, and Health Potion. Only the highest
unlocked helmet tier is selected; weaker helmets stop spawning when an upgrade
unlocks. The nine-item catalog contains those three items, five helmets, and Lamp Fuel.
Fuel never enters this random selection.

| Helmet | Armor bonus | First dungeon depth | Catalog level |
| --- | ---: | ---: | ---: |
| Rusty Helmet | +1 | 0 | 1 |
| Lead Helmet | +2 | 3 | 4 |
| Iron Helmet | +3 | 6 | 7 |
| Steel Helmet | +4 | 9 | 10 |
| Pete's Helmet | +5 | 12 | 13 |

Helmets use the existing armor slot and have fixed stats. Rusty Helmet preserves
the former Leather Armor's +1 defense and catalog index. The maximum helmet
bonus is +5; the player's base defense of 1 makes maximum total defense 6.

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
Game. Game calls `Level::update_visibility_with_radius(player_position, radius)` after
every spent action and on floor entry. `update_visibility(position)` remains a
full-radius helper for independent callers. Updating sight spends no turn and never clears
previously seen tiles. Invalid origins clear current visibility but preserve memory.

Player sight reaches up to 10 tiles in each direction (Chebyshev distance),
limited by the current lamp fuel as described above. Previously seen terrain remains remembered beyond
that range. Walls, rocks, and closed doors block sight; water,
floor, stairs, and open doors transmit it. The first opaque tile remains visible.
All eight adjacent tiles are visible regardless of blockage, revealing room corners.
Beyond that neighborhood, straight rays cannot pass diagonally through blocked corners. Entities do not
block sight. Opaque boundary tiles are also revealed when the transparent tile
immediately toward the viewer is visible, so diagonal room walls are seen without
revealing terrain behind them. `Map::has_line_of_sight(origin, target)` queries geometry without
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

Enemy attack intents roll the species' damage dice once, subtract effective player
defense (minimum one), and report actual HP lost. Invalid, dead, or non-cardinal
targets do not consume a roll. Lethal damage stops the remaining enemy actions.

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
of one damage. New games equip a rolled Pickaxe with base damage 2d4; supported weapon dice are d4, d6,
and d12. Unarmed damage is 1d4. Player HP is 40; Slime/Ghost/Goblin HP is 8/14/24,
with damage rolls of 1d4/1d6/1d12 and defense of 0/1/2. Player base defense is 1,
and Rusty Helmet adds 1 (later helmets add up to 5). All starting flat attack modifiers are zero. Damage
rolls use Game’s RNG; reading weapon data and rendering never roll dice.
Messages report actual damage, weapon effects, defeat, and drops using species
names without enemy IDs. Equipped item names appear in the status panel. Each defeated
enemy has a 5% chance to drop equipment. Successful drops select helmets 5% of
the time and weapons 95% of the time, giving a 0.25% helmet chance per kill.
Each category uses its own highest unlocked catalog tier (level no greater than
saturating depth + 1), so deeper helmets never displace weapons from the drop pool. Weapon drops roll their bonuses and rare effects at the current depth. Entering a defeated enemy's tile takes another move.
Movement attempts, door opening, and waiting report a spent turn, including blocked
attempts by a living player. Dead-player gameplay actions are ignored without
spending turns. Normal combat deaths immediately enter GameOver. Entity display
and visibility queries do not spend turns.

## Weapon rolls and rare effects

Pickaxes deal base **2d4** damage; shovels deal **2d4+2**. Both unlock at
catalog level one. Every newly created weapon (starting equipment, floor loot,
and enemy drops) rolls once at its current dungeon depth. With
`tier = min(depth / 4, 1000)`, its flat damage bonus is its base bonus plus
`tier + random(0..=2 + tier)`. Attack dice still roll independently on every hit.
Inventory transfers and floor transitions retain the original rolled values.

Each weapon has a **1%** chance to gain one effect, chosen equally from:

| Effect | Damage-text color | On-hit behavior |
| --- | --- | --- |
| Bleed | Red | Deals fixed damage for three turn boundaries, bypassing defense. |
| Weakness | Purple | Reduces the enemy's outgoing attack roll for three turn boundaries, before defense and minimum-one damage. |
| Lightning | Electric blue | Immediately damages other enemies within Chebyshev distance two of the target, requiring terrain line of sight and bypassing defense. |

Effect power is one quarter of the weapon's average base damage (including the
shovel's base +2), rounded up: currently **2** for both weapons. It excludes the
random depth bonus and the attack's dice result. Lightning also triggers when the
primary hit kills, does not chain, and uses the usual defeat/drop handling for
splash kills. Bleed and weakness apply only to surviving targets, share the
three-effect capacity, stack independently, and tick on the hit turn. Lightning
is immediate and does not occupy a status slot.

Inventory and equipped status labels show the full expression, for example
`Shovel (2d4+4 Weakness)`, with only the parenthesized damage text colored.

## Combat turn order

`Game::process_action()` invokes one shared `advance_turn()` for each spent action:

1. Resolve the player action and remove defeated enemies immediately.
2. Process surviving enemies in stable ID order, applying one movement or attack
   decision each. Revalidate cardinal attack range and apply dice damage once.
3. Tick player effects once. If lethal, skip every remaining enemy effect tick.
4. Tick surviving enemy effects once in ID order. Remove effect deaths and use
   the same once-only equipment-drop path as direct kills.

New weapon effects tick in the turn they are applied. An enemy killed by a direct
hit never retaliates; one killed by a later effect can act before that tick.
Effects bypass defense, stack independently up to three, and expire after their
last tick. Logs show actual aggregate effect HP loss and individual expirations.
The status panel shows active effect names and remaining turns.

Blocked moves, waits, bump attacks, and opening doors all run this boundary once.
Menus, pause, unsupported keys, and redraws do not advance enemies or effects.
Pickup/drop/equip/unequip/use handlers return their spent-turn result to this
same boundary, including failed attempts, rather than starting another loop.

After any lethal damage update, Game stores final depth and cause, appends the
death message, enters GameOver, and rejects further gameplay actions. The full
message history remains available for scrolling on GameOver, retained in MessageLog
until returning to menu or restarting. Summary, history, effects, and equipment
reset for a new session. Press `>` while standing on stairs to descend; movement onto stairs alone does
not descend. Accepted descent ticks player effects on the old floor before
replacement; lethal effects retain the old floor and final depth. Survivors enter
the next floor at its start with health, effects, inventory, and equipment retained
and sight initialized. Neither floor runs enemy actions or enemy effect ticks
during this transition. Failed descent, including depth overflow, spends an ordinary
turn. Descent is available only during play.

Run `cargo fmt --check`, `cargo test --offline`, and
`cargo clippy --offline --all-targets -- -D warnings` before review. In a terminal,
check New Game, pause/resume, return to menu, restart, resize, and quit. On New Game,
only terrain in sight is revealed; a floor may initially show no entities because
the entire starting room is protected and closed doors block sight.
