# Combat and turn contracts

Phase 1 contracts and foundation, November 11, 2026. Scaled stats, item modifiers,
effect specifications, effective-stat queries, enemy effect operations, cardinal
range queries, and AI intents are implemented. Phase 2 adds pure damage
calculation and controlled Level damage/effect updates by enemy ID. Phase 3 adds
player bump attacks, immediate defeat/removal, and rare equipment drops. PR12
(develop de0d21d) adds AI decisions and a spent-action enemy phase; reconciliation
requires terrain sight and cardinal attack range while retaining scaled stats.
Enemy damage, scheduled effects, descent, and game-over history controls remain
later commits. Extend the existing enemy phase rather than scheduling it twice.
This document supplements SDD Contracts 2, 5, 7, 10, 13, 14, 18, and 20.
The original SDD and CRC replacement passages predate these decisions.

## Damage, equipment, and initial balance

Normal attack damage is `max(1, effective_attack - effective_defense)`. Attacks
always hit: no accuracy roll, critical hit, or random damage. Use saturating
arithmetic for stat addition and subtraction. Actual damage is limited to the
defender's remaining HP. Existing health mutation APIs still take a signed change:
negative damages and positive heals. Combat-facing APIs take nonnegative damage
and perform that conversion internally.

Effective player attack is base attack plus the equipped weapon's attack bonus;
effective defense is base defense plus the equipped armor's defense bonus.
Unequipped items contribute nothing. Reading effective stats never modifies base
stats. Enemies initially use their species' base attack and defense.

The following are initial implementation tuning values, selected to give armor
useful increments and enemies several hits of health. They require playtesting;
they are not depth-scaling formulas. Phase 1 applies them to New Game and enemy
construction; health and effect unit fixtures may still use arbitrary stats.

| Actor | Maximum HP | Attack | Defense | Flee below HP |
| --- | ---: | ---: | ---: | ---: |
| Player | 200 | 15 | 5 | N/A |
| Slime | 30 | 12 | 2 | 0 |
| Ghost | 50 | 18 | 3 | 25 |
| Goblin | 100 | 25 | 5 | 40 |

Preserve the existing exclusive flee thresholds and species ordering. AI owns
detection, pursuit, and recovery rules. Increasing enemy HP must also update its
flee threshold instead of retaining thresholds from the old HP scale.

Initial catalog modifiers are Pickaxe `+3 attack` and Leather Armor `+3 defense`.
An unarmored player takes 7/13/20 damage from Slime/Ghost/Goblin; Leather Armor
reduces those values to 4/10/17. Existing catalog indices and availability levels
remain stable. Final additional items and consumable healing values belong to
inventory work.

## Attack requests and AI boundary

Moving into an enemy becomes one player attack, leaves the player in place, and
spends one turn. Attacks require cardinal adjacency (Manhattan distance exactly
one). Diagonal and distant attack requests are rejected. A failed gameplay attack
still spends a turn.

AI returns one decision per enemy: `Wait`, `Move { target: Position }`, or `Attack`.
Attack targets the active player; the scheduling caller already knows EnemyId.
Game validates range, terrain, enemy occupancy, player occupancy, and actor life
before applying a decision. Invalid decisions become a wait, without retries or
an extra turn. AI never changes HP, removes actors, or advances turns.

PR12 implements `AiProfile::for_enemy()` and read-only `ai::decide()` with
detection ranges Slime 3, Ghost 6, Goblin 5 (Chebyshev distance). Game supplies
`can_see_player` from `Map::has_line_of_sight(enemy_position, player_position)`;
this uses current terrain and the eight-adjacent-tile exception without depending
on player exploration memory. Losing sight or range makes the enemy Idle, even
when previously chasing or fleeing. Healing above the threshold permits attack
or pursuit on the next decision. Movement improves Manhattan distance with
deterministic left/right/up/down ties, waits when blocked, and never opens doors.
Diagonal neighbors move cardinally rather than requesting PR12's original
diagonal attack. Attack intents are not yet executed as enemy damage.

Use the active `enemy::AiState` as the single behavior-state type. The disconnected
`enemy/ai.rs` sketch is not an additional source of truth. Integrate the AI branch's
decision interface without adding a second behavior-state enum.

## Weapon effects and enemy effects

Item combat data contains a nonnegative attack bonus, nonnegative defense bonus,
and an optional on-hit effect specification. The specification describes a name,
nonnegative damage per turn, and a positive duration. It creates a fresh
`StatusEffect` per hit; an Item never holds a ticking effect instance.

An equipped weapon with an on-hit effect applies it on every valid, nonlethal
hit, after direct damage. This initial framework has no random proc chance.
The placeholder Pickaxe has no effect; future catalog weapons opt into the same
data-driven mechanism without name-based combat branches. Armor and consumables
cannot apply weapon on-hit effects.

Both Player and Enemy receive and tick effects using Stats. Preserve the existing
rules: effects bypass defense, duplicate names stack independently, at most three
effects are active, and adding a fourth replaces the oldest. New effects tick in
the effect phase of the turn in which they were added. Durations count effect
phases, including that first tick. Directly lethal hits do not attach effects.

## One shared turn boundary

For an ordinary spent gameplay action:

1. Apply the player action, including direct damage and on-hit effects. Remove
   defeated enemies immediately and resolve their drops. If the player dies,
   end the session immediately.
2. Snapshot surviving floor-local enemy IDs in ascending insertion-ID order.
   Each living, still-present enemy gets one AI decision. Apply it and check
   player death immediately after damage. Stop the turn on player death.
3. Tick player effects once. Log actual HP lost and expirations. On death, end
   the session immediately and skip all remaining processing.
4. Tick surviving enemies' effects once in ascending ID order. Update surviving
   enemies' behavior after HP changes; immediately remove and resolve drops for
   effect deaths. Removed IDs never tick or act again.

An enemy poisoned by the player's attack can act before the effect's first tick;
an enemy killed by direct damage cannot retaliate. Stats currently applies all of
one actor's effect damage as one aggregate update; death is checked after that
update. No later actor is processed after player death.

Move, attack, wait, door opening, pickup, drop, equip, unequip, use, and descend
attempts spend one turn, even on failure. Failed changes leave state unchanged
before normal turn processing. Screen navigation, opening/closing inventory,
selection changes, pause/resume, redraw, and quit spend no turn. Item handlers
report their result into this boundary rather than running a separate enemy loop.

## Descent exception

A living player must press `>` while standing on stairs. Stepping onto stairs
does not descend. For a valid descent, tick player effects once on the old floor
before generating or replacing anything. If the player dies, show GameOver at the
old depth and retain its log; do not generate a floor. Otherwise replace the floor,
preserve the existing player's stats/effects/items, move them to the new start,
and initialize visibility. Neither floor gets enemy actions or enemy effect ticks
on this transition turn. Discard all old enemy IDs and pending decisions.

Invalid descent, including checked depth overflow, spends an ordinary turn on the
current floor. GameOver rejects gameplay actions without spending turns.

## Enemy removal and equipment drops

All enemy deaths, including effect deaths, use one Game-owned defeat path. Capture
the enemy's position and identity, remove it through Level, and resolve one drop
roll only when removal succeeds. No experience, currency, or other rewards yet.

Initial tuning: each defeated enemy has a 5% chance to drop exactly one weapon or
armor item. Use Game's supplied RNG for all rolls and selection. Eligible catalog
entries have level no greater than saturating `depth + 1`; among these, select
uniformly from the highest available level tier across both categories. This
keeps drops on par with the floor as the catalog grows. If no equipment is
eligible, produce no drop. Never substitute a consumable or invent a catalog item.
The current level-one-only catalog cannot yet provide stronger high-depth loot.

Insert the owned item at the defeated enemy's tile through Level. Ground items
may share tiles. Record the drop only after successful insertion. Runtime enemy
placement already guarantees walkable death tiles; insertion rejection must
return ownership rather than silently destroy an item. Effect and direct deaths
must not roll twice for the same EnemyId.

## Messages, death, and game-over access

Log attacker, defender, and actual HP lost for each attack; log effect application,
effect damage, expiration, enemy defeat, equipment drops, and player death in
resolution order. Include enemy species and ID when needed to distinguish actors.
Append the lethal damage and death messages before building the final summary.

On player death, stop the turn, store final depth and cause of death, enter
GameOver, and reject gameplay actions. Preserve MessageLog until returning to the
menu or starting a new session. Its existing `history()` retains the full session;
`messages()` exposes only the recent window.

GameOver must show recent messages and allow scrolling through the full session
history, including the lethal event. Add screen-specific history up/down actions
with Up/Down and PageUp/PageDown bindings, clamp the scroll offset, and start at
the newest messages. These controls spend no turn. The launcher supplies borrowed
history and the selected scroll offset; rendering never modifies Game or the log.
Returning to menu clears history, summary, and scroll state. Small terminals must
still allow access to the final messages through scrolling.

## Implemented phase 1 interfaces

- `stats::EffectSpec::new()` and `instantiate()` validate reusable data and create
  independent effect instances.
- `item::CombatModifiers::weapon()` / `armor()`, `Item::with_combat()`, and
  `Item::combat()` provide category-checked item data. Catalog lookup and iteration
  preserve that data; constructors for custom items default to no modifiers.
- `Player::effective_attack()`, `effective_defense()`, and `weapon_effect()` read
  equipped data without changing base stats or ticking effects.
- `Enemy::add_status_effect()` and `tick_status_effects()` use existing Stats
  behavior; dead enemies ignore new effects and do not tick.
- `enemy::ai::Intent` exposes Wait, Move with target, and Attack, using the existing
  `enemy::AiState` as the only behavior-state enum.
- `Position::is_cardinal_neighbor()` provides overflow-safe attack-range geometry.

## Implemented phase 2 interfaces

- `combat::attack_damage(attack: i32, defense: i32) -> u32` implements the minimum-one
  deterministic formula, rejects negative stats, and expects equipment bonuses
  already included.
- `Level::damage_enemy(id, damage: u32, effect: Option<StatusEffect>)` applies
  nonnegative damage, then attaches the optional effect only to a survivor.
  Damage is already calculated: defense is not subtracted again. Zero damage
  does not change behavior state but can attach a supplied effect.
- `Level::tick_enemy_effects(id)` explicitly ticks one enemy, without scheduling
  other actors. Both operations return `Option<DamageOutcome>`: None for missing
  IDs, otherwise actual HP lost, current death status, and effect application.
  An already-dead enemy returns zero loss and no effect application unchanged.
- Neither operation removes enemies, rolls drops, validates attack range, emits
  messages, or spends turns. Game must handle a dead outcome immediately through
  its defeat path; `is_dead` describes final state, not a new death event.

## Implemented phase 3 integration

- Game routes a cardinal bump into an enemy to `attack_enemy()`, calculates damage
  from effective player attack and enemy defense, and supplies a fresh weapon
  effect to Level. It logs actual HP loss and effect application. The player stays
  put, including on a lethal hit, and the action reports one spent turn.
- `Game::defeat_enemy()` rejects living/missing IDs, removes a dead enemy once,
  logs defeat, then rolls equipment loot with Game's RNG. Effect deaths can use
  the same helper when the turn loop is connected.
- `combat::equipment_drop()` is crate-private: 5% chance, highest unlocked tier,
  uniform selection among equipment in that tier. It supports saturating depth,
  empty catalogs, and no eligible equipment. Game inserts the owned drop on the
  death tile, which can already hold other items, and then logs it.
- These operations do not run enemy actions or tick effects automatically.

## PR12 integration

`Game::process_action()` now invokes `advance_enemy_turns()` once after a spent
action, including a bump attack or blocked move. Menus and pause skip it. Game
snapshots stable enemy IDs, skips removed/dead actors and dead players, computes
range/sight decisions, applies state through `Level::set_enemy_ai_state()`, and
revalidates movement against current occupancy. Enemy attack damage and automatic
effect ticking remain unimplemented; phase 4 must extend this boundary.

## Interfaces to implement in later phases

These describe responsibilities, not already-exported Rust APIs:

- One Game turn coordinator that reuses the enemy-defeat/drop path for effect
  deaths. A successful descent outcome explicitly skips ordinary turn processing.
- Stored game-over summary and history-scroll state, with borrowed history in
  display data. The renderer has no gameplay or turn responsibilities.

## Validation required by subsequent commits

Cover damage boundaries and armor improvements; cardinal bump attacks; fresh
per-hit effects and independent duplicate durations; once-only enemy/effect
phases; immediate direct and effect deaths; seeded drop reproducibility and depth
eligibility; no double drops; inventory failures; descent effect death at the old
depth; and game-over history scrolling without turns. Verify restart clears all
session state. Run formatting, offline tests, and Clippy before review.
