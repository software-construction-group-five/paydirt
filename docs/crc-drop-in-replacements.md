# Paydirt — SDD replacement passages

Revision basis: develop at 576c752 · November 11, 2026

Integration update: PR12 (develop de0d21d) is now merged with combat phases 1–3.
The original revision passages below remain historical where superseded. See
[Combat and turn contracts](combat-turns.md) and README's Enemy AI section for
current Game ownership, player attacks, scaled stats, rare drops, and AI behavior.
AI now makes one range-and-terrain-sight decision per spent action. Detection
ranges are Slime 3, Ghost 6, Goblin 5; flee thresholds are 0, 25, 40 HP. Attacks
require cardinal adjacency. Lost sight/range yields Idle; healing can end fleeing.
Health updates do not wake healthy idle enemies. Enemy damage and automatic
status-effect scheduling remain unfinished.

Each numbered heading below identifies the original SDD location to replace or extend. Insert the text beneath that heading. Unlisted passages remain unchanged; statements of unfinished functionality describe the current implementation rather than a completed requirement.

## 3.2 — Game CRC card: replace Private Responsibilities and Comments

Private Responsibilities:
- Knows the current game screen and menu selection.
- Maintains the current Level and the game’s RNG.
- Controls screen transitions and records whether the application should quit.

Comments:
Game is the main controller of Paydirt. The application launcher controls the main loop, obtains actions from the Terminal Interface, passes those actions to Game, and requests display of the resulting state. Game does not call Renderer directly. New Game generates a level at depth zero and initializes visibility from the level’s starting position. The current implementation does not create an active Player or MessageLog. Player ownership, gameplay turns, level progression, and game-ending results remain responsibilities to be implemented under Contracts 1, 2, 4, and 5.

## 3.2.2 — Contract ProcessAction: replace Protocol 1

Contract Identifier: G-C02
Contract Name: ProcessAction
Description: Processes recognized actions according to the current screen and updates the game state supported by that screen.

Protocol 1: process_action(action)

Method Signature (Rust):
fn process_action(&mut self, action: &Action)

Responsibilities and Collaborations:
- Receives the recognized action from the application launcher — Terminal Interface (21).
- Updates the main-menu selection and controls screen transitions — Game (3).
- Requests the starting Level when New Game is selected — LevelGenerator (19).
- Initializes visibility at the starting position — Level (8).
- Records a quit request for the application launcher.

Preconditions:
@requires Game is initialized.

Postconditions:
@ensures The action is handled according to the current screen.
@ensures Unsupported actions leave the game state unchanged.
@ensures Quit records an application exit request from any screen.
@ensures Once quit is requested, later actions leave the game state unchanged.

Algorithm description:
Return without changes if quit has already been requested. If the action is Quit, record the exit request. Otherwise, examine the current screen. On MainMenu, update the selection or start a new floor when New Game is confirmed. On Playing, Pause changes the screen to Paused. On Paused, Pause or Confirm resumes play, and Back clears the active level and returns to MainMenu. On GameOver, Confirm or Back returns to MainMenu. Inventory currently has no action handling. Gameplay movement, interactions, survival checks, and turn advancement are not yet implemented.

## 3.2.3 — Contract ChangeScreen: replace protocol description and algorithm

Description: Controls the active screen as part of action processing. The current implementation performs transitions inside process_action rather than through a separate change_screen method.

Responsibilities and Collaborations:
- Knows and updates the current screen.
- Restricts action handling according to the active screen.
- Provides the current screen to the application launcher through screen().

Algorithm description:
Examine the current screen and requested action. Apply a supported transition and preserve or clear the active level as required. The application launcher reads the resulting screen, selects the corresponding keybindings, and passes the screen and display data to Renderer. Game does not notify Renderer directly. Inventory entry and the transition into GameOver remain unimplemented.

## 3.3.1 — Player State Access: replace signatures and associated descriptions

Protocol 1: get_position()
Method Signature (Rust):
fn get_position(&self) -> Position

Description: Returns the player’s current position as a Position containing unsigned x and y coordinates.

Protocol 2: get_stats()
Method Signature (Rust):
fn get_stats(&self) -> &Stats

Description: Returns a read-only reference to the player’s health, attack, defense, and active status effects.

Protocol 4: get_equipped_items()
Method Signature (Rust):
fn get_equipped_items(&self) -> &Equipment

Description: Returns a read-only reference to the player’s equipment. Equipment contains one optional weapon and one optional armor item. Reading equipped items does not transfer ownership or change the player state.

## 3.3.2 — Player State Modification: replace Protocol 2

Protocol 2: update_health()

Method Signature (Rust):
fn update_health(&mut self, amount: i32, effect: Option<StatusEffect>)

Responsibilities and Collaborations:
- Updates the player’s health through Stats.
- Adds an optional status effect through Stats.

Preconditions:
@requires Player is initialized.
@requires Each supplied StatusEffect has nonnegative damage and a positive duration.

Postconditions:
@ensures Positive amounts heal and negative amounts damage.
@ensures Health remains between zero and maximum health.
@ensures None adds no status effect.
@ensures A supplied effect is appended; the oldest effect is removed if three effects are already active.
@ensures Adding an effect does not apply its per-turn damage immediately.

Algorithm description:
Apply the signed health change using saturating arithmetic and clamp the result to the player’s health limits. If an effect is supplied, add it to the active effects, replacing the oldest when the three-effect limit has been reached. Effects with the same name are stored independently.

Protocol: tick_status_effects()

Method Signature (Rust):
fn tick_status_effects(&mut self) -> i32

Description: Applies one turn of active status effects and returns the actual health lost.

Algorithm description:
Add the damage from all active effects using saturating arithmetic. Reduce each effect’s remaining duration by one. Apply the total damage without using defense, then remove expired effects. Return the health lost, limited by the player’s remaining health. The caller determines when a gameplay turn occurs; reading state and drawing a frame do not tick effects.

## 3.3.2 — Player State Modification: replace Protocol 3

Protocol 3: Update Equipped Items

Description: Equipment is represented by one weapon slot and one armor slot rather than an item collection that toggles membership. Equipment accepts weapons and armor in their matching slots and rejects consumables. Player-level operations that transfer items between Inventory and Equipment are not yet implemented.

Supporting Method Signatures (Rust):
fn equip(&mut self, item: Item) -> Result<Option<Item>, Item>
fn unequip_weapon(&mut self) -> Option<Item>
fn unequip_armor(&mut self) -> Option<Item>

Responsibilities and Collaborations:
- Maintains the weapon and armor slots — Equipment.
- Identifies the item category — Item (18).
- Returns ownership of displaced, rejected, or unequipped items to the caller.

Postconditions:
@ensures A valid item occupies its matching slot after equip succeeds.
@ensures Equip returns the displaced item, or None if the slot was empty.
@ensures A rejected consumable is returned and both slots remain unchanged.
@ensures Unequipping clears the requested slot and returns its previous item.

Algorithm description:
Check the item’s category. Replace the corresponding weapon or armor slot and return the previous item. Return a consumable unchanged as an error. Unequip operations take the item out of the requested slot. These Equipment operations do not add returned items to Inventory or apply combat bonuses.

## 3.4 — Level CRC card: replace card

Class Name: Level
Description: Represents and manages the current dungeon environment, including terrain, navigation anchors, enemies, ground items, and exploration state.
Super classes: N/A

Contracts:
Contract 8: Map Access
Description: Provides read-only terrain information and operations for updating and querying player visibility.
- Provides the dungeon map — Game (2), Renderer (20).
- Refreshes current visibility while retaining previously seen terrain — Game (2).
- Provides visible enemies and ground items — Renderer (20).

Contract 9: Position Validation
Description: Determines whether a position is within the dungeon boundaries and traversable according to its terrain.
- Checks map bounds and terrain walkability — Game (2), Enemy (13).
- Keeps terrain validation separate from entity occupancy.

Contract 10: Enemy Management
Description: Maintains enemies using stable identities within the current floor.
- Adds, retrieves, moves, and removes enemies — Game (2), LevelGenerator (19), Enemy (12, 13).
- Rejects enemy placement and movement on blocked terrain or another enemy.

Contract 11: Ground Item Management
Description: Maintains owned items paired with their floor positions.
- Adds, retrieves, and removes selected ground entries — Game (2), LevelGenerator (19), Item (18).
- Returns the original item on rejection or removal.

Private Responsibilities:
- Maintains the map, depth, player start, stairs, and starting-room tiles.
- Assigns enemy IDs that are never reused within the floor.
- Preserves valid enemy positions and ground-item ownership.
- Maintains current visibility and exploration memory through Map.

Comments:
Level manages the dungeon environment. LevelGenerator constructs and populates the floor, while Game coordinates gameplay. Level does not own the Player; Game checks player occupancy before applying an enemy move. Initial spawning excludes the entire starting room and stairs. These restrictions do not prevent enemies from entering those tiles or items from being dropped there during gameplay.

## 3.4.1 — Contract Map Access: add visibility protocols after get_map()

Protocol 2: update_visibility(player_position)

Method Signature (Rust):
fn update_visibility(&mut self, player_position: Position) -> bool

Description: Recomputes current line of sight and retains exploration memory for the current floor.

Postconditions:
@ensures Visible tiles are also marked seen.
@ensures Previously seen tiles remain seen.
@ensures An invalid origin clears current visibility, preserves memory, and returns false.
@ensures Visibility updates do not advance gameplay turns.

Algorithm description:
Trace sight from the supplied position without a distance limit. Walls, rocks, and closed doors block sight; the first blocking tile is visible. Water, floor, stairs, and open doors transmit sight. Rays cannot pass through blocked diagonal corners. Entities do not block sight. Update visible flags and retain seen flags. Game requests this operation on floor entry and after movement or sight-blocking terrain changes.

Protocol 3: visible_enemies()
Method Signature (Rust):
fn visible_enemies(&self) -> impl Iterator<Item = (EnemyId, &Enemy)>

Protocol 4: visible_ground_items()
Method Signature (Rust):
fn visible_ground_items(&self) -> impl Iterator<Item = (usize, &GroundItem)>

Description: Returns borrowed entities whose current positions are visible. Remembered terrain does not reveal hidden entities. Ground-item results preserve the original storage indices. Full collections remain available independently of visibility.

## 3.4.2 — Contract Position Validation: replace algorithm and postconditions

Postconditions:
@ensures The result is true only for an in-bounds tile with walkable terrain.
@ensures Walls, rocks, water, and closed doors return false.
@ensures Floor, stairs, and open doors return true.
@ensures Entity occupancy does not affect the result.
@ensures Level state remains unchanged.

Algorithm description:
Check the map bounds and the tile’s terrain. Return whether the terrain allows movement. Game checks enemies separately through enemy_at() and checks the active Player’s position before requesting an enemy move.

## 3.4.3 — Contract Enemy Management: replace protocols

Contract Identifier: 10
Contract Name: Enemy Management
Description: Manages enemies by stable, floor-local identity and validates their placement and movement.

Protocol 1: insert_enemy(enemy)
Method Signature (Rust):
fn insert_enemy(&mut self, enemy: Enemy) -> Result<EnemyId, Enemy>

Algorithm description:
Check that the enemy’s position is walkable and contains no enemy. Assign a fresh EnemyId and store the enemy. If placement is invalid or no IDs remain, return the original enemy without changing storage. IDs are never reused within one Level and must be discarded when the floor is replaced.

Protocol 2: enemies()
Method Signature (Rust):
fn enemies(&self) -> impl Iterator<Item = (EnemyId, &Enemy)>

Algorithm description:
Return borrowed enemies in insertion-ID order without changing the collection.

Protocol 3: enemy(id) and enemy_at(position)
Method Signatures (Rust):
fn enemy(&self, id: EnemyId) -> Option<&Enemy>
fn enemy_at(&self, position: Position) -> Option<EnemyId>

Algorithm description:
Return the matching enemy or the ID occupying the requested tile. Return None if no match exists.

Protocol 4: move_enemy(id, position)
Method Signature (Rust):
fn move_enemy(&mut self, id: EnemyId, position: Position) -> bool

Algorithm description:
Reject an unknown ID, blocked terrain, or a tile occupied by another enemy. Otherwise, update the enemy’s position and return true. Remaining on its current tile succeeds. Rejection leaves all state unchanged. Game checks the Player’s position before calling this operation.

Protocol 5: remove_enemy(id)
Method Signature (Rust):
fn remove_enemy(&mut self, id: EnemyId) -> Option<Enemy>

Algorithm description:
Remove and return the matching enemy. Return None for an unknown ID. Removing an enemy does not change the IDs of other enemies.

## 3.4.4 — Contract Ground Item Management: replace protocols

Contract Identifier: 11
Contract Name: Ground Item Management
Description: Manages owned ground items and their positions, preserving individual copies and transferring ownership on insertion and removal.

Protocol 1: insert_ground_item(item, position)
Method Signature (Rust):
fn insert_ground_item(&mut self, item: Item, position: Position) -> Result<(), Item>

Algorithm description:
Check that the position has walkable terrain. Store the supplied Item and Position as a GroundItem and return success. Return the original item on rejection. Runtime items may share a tile with other items or an enemy, including tiles in the starting room and stairs.

Protocol 2: ground_items() and ground_items_at(position)
Method Signatures (Rust):
fn ground_items(&self) -> &[GroundItem]
fn ground_items_at(&self, position: Position) -> impl Iterator<Item = (usize, &GroundItem)>

Algorithm description:
Return borrowed ground entries in storage order, or entries at the requested position with their current storage indices. Reading entries does not change Level state.

Protocol 3: remove_ground_item(index)
Method Signature (Rust):
fn remove_ground_item(&mut self, index: usize) -> Option<GroundItem>

Algorithm description:
Remove and return the selected ground entry, including the original Item and Position. Return None for an invalid index without changing storage. Later indices shift after removal. Callers rebuild item selections after a removal; a storage index identifies an entry rather than an item catalog type.

## 3.5 — Enemy CRC card: replace card

Class Name: Enemy
Description: Represents an enemy and maintains the information needed for behavior and combat interactions.
Super classes: N/A

Contracts:
Contract 12: Get Current Enemy State
Description: Provides read-only access to the enemy’s species, position, statistics, and behavior state.
- Knows enemy species, position, health, attack, and defense.
- Knows enemy state: Idle, Attacking, or Fleeing.

Contract 13: Enemy Update
Description: Receives a validated position and updates behavior using the current health threshold.
- Updates the enemy position — Level (9, 10).
- Updates behavior after a signed health change — Stats.

Contract 14: Combat Information
Description: Provides the enemy’s fixed base attack value.
- Knows the attack value stored in Stats.

Private Responsibilities:
- Maintains species, position, Stats, behavior state, and flee-health threshold.

Comments:
Enemy does not choose or validate a movement target. Level validates terrain and enemy occupancy, and Game checks player occupancy. Enemies start Idle. The current health-based state update does not use player distance or line of sight. Movement and attack decisions remain unimplemented.

## 3.5.1 — Get Current Enemy State: replace Protocol 1

Protocol 1: Current State Access

Method Signatures (Rust):
fn kind(&self) -> &EnemyType
fn position(&self) -> &Position
fn stats(&self) -> &Stats
fn ai(&self) -> &AiState

Responsibilities and Collaborations:
- Provides the enemy’s species, position, combat statistics, and behavior state.

Preconditions:
@requires Enemy is initialized.

Postconditions:
@ensures Returned references describe the current enemy state.
@ensures Enemy state remains unchanged.

Algorithm description:
Return read-only references to the requested fields. Level performs lookup by EnemyId before the caller reads these values.

## 3.5.2 — Enemy Update: replace Protocol 2 and add health-change protocol

Protocol 2: update_status()

Method Signature (Rust):
fn update_status(&mut self)

Description: Updates the behavior state using the enemy’s current health and species threshold.

Algorithm description:
If the enemy is Idle or Attacking, set the state to Fleeing when health is strictly below its threshold; otherwise set it to Attacking. An enemy already Fleeing remains Fleeing. Goblins flee below 10 HP, ghosts below 5 HP, and slimes have a threshold of zero. This operation does not inspect the Player’s position or select a movement or attack action.

Protocol 3: take_damage(amount)

Method Signature (Rust):
fn take_damage(&mut self, amount: i32)

Description: Applies a signed health change and then updates the behavior state.

Algorithm description:
Apply the amount through Stats. Negative amounts reduce health and positive amounts restore health, within zero and maximum health. Call update_status() afterward. This method does not remove a dead enemy from Level.

## 3.5.3 — Contract Get Combat Information: replace protocol

Contract Identifier: 14
Contract Name: Get Combat Information
Description: Provides the enemy’s fixed base attack value without changing its state.

Protocol 1: get_roll()
Method Signature (Rust):
fn get_roll(&self) -> i32

Preconditions:
@requires Enemy is initialized.

Postconditions:
@ensures The result equals the attack value stored in Stats.
@ensures Enemy state remains unchanged.

Algorithm description:
Return the stored base attack value. No random roll, defense calculation, or equipment adjustment is performed.

## 3.6 — Inventory CRC card: replace contract descriptions and Comments

Contract 15: Add Item to Inventory
Description: Takes a valid catalog index and constructs one independent copy of the corresponding item if its category has room. Returns false when that category is full.

Contract 16: Remove Item from Inventory
Description: Removes and returns the first carried copy with the requested catalog index. Returns None when no copy is carried.

Contract 17: Return Inventory Content
Description: Returns a string listing carried items by category. Limits are five consumables, three weapons, and one carried armor item.

Comments:
Inventory stores carried items separately from Equipment. Equipping, using, and transferring a ground item into Inventory are not implemented by these operations. Duplicate catalog items occupy separate spaces. add_item() constructs a catalog copy; it does not accept ownership of an existing ground item.

## 3.6.1 — Contract Add Item: replace protocol

Contract Identifier: G-C15
Contract Name: Add Item
Description: Adds one catalog item when its category has available capacity.

Protocol 1: add_item(index)
Method Signature (Rust):
fn add_item(&mut self, index: u32) -> bool

Preconditions:
@requires The index identifies an existing catalog item.

Postconditions:
@ensures The relevant category limit is not exceeded.
@ensures Success adds one independent item copy and returns true.
@ensures A full category leaves Inventory unchanged and returns false.

Algorithm description:
Retrieve the catalog item. Count carried items in the same category. If the category is full, return false. Otherwise, append the new item and return true. The limits are five consumables, three weapons, and one armor item. This method does not print an error message.

## 3.6.2 — Contract Remove Item: replace protocol

Contract Identifier: G-C16
Contract Name: Remove Item
Description: Removes and returns one carried copy with the requested catalog index.

Protocol 1: remove_item(index)
Method Signature (Rust):
fn remove_item(&mut self, index: u32) -> Option<Item>

Preconditions:
@requires Inventory is initialized.

Postconditions:
@ensures Success returns the original removed item.
@ensures Other copies and unrelated items remain in Inventory.
@ensures A missing index returns None and leaves Inventory unchanged.

Algorithm description:
Find the first carried item whose catalog index matches the request. Remove and return that item. Return None if no matching item is carried.

## 3.6.3 — Return Inventory Content: replace signature and algorithm

Method Signature (Rust):
fn return_inventory(&self) -> String

Algorithm description:
If Inventory is empty, return “Inventory is empty.” Otherwise, list item names under Weapons, Armor, and Consumables in that order. Use “none” for an empty category. Return the combined string without changing Inventory. The armor listed here is carried armor; equipped armor is stored separately in Equipment.

## 3.7.1 — Contract Get Specific Item: replace signature and add catalog protocol

Protocol 1: get_item(index)
Method Signature (Rust):
fn get_item(index: u32) -> Item

Preconditions:
@requires The index identifies an existing catalog entry.

Postconditions:
@ensures The result is an independently owned Item with the requested catalog index, name, category, and level.

Algorithm description:
Look up the entry in the shared catalog and construct the corresponding Item. An unknown index causes a panic. Catalog lookup does not require an existing Item instance.

Protocol 2: catalog()
Method Signature (Rust):
fn catalog() -> impl Iterator<Item = Item>

Description: Returns independent Items for every catalog entry in index order. LevelGenerator uses this catalog to select items available at the requested depth.

## 3.8 — Contract Generate Level: replace contract

Contract Number: 19
Contract Name: Generate Level
Description: Builds and returns a procedurally generated floor at the depth supplied by Game. Game supplies the RNG and decides when a new level is needed. LevelGenerator keeps no state between calls. The returned Level contains its terrain, navigation anchors, enemies, and ground items.

Method Signature (Rust):
pub fn generate_level(depth: u32, rng: &mut impl Rng) -> Level

Preconditions:
@requires A mutable RNG is supplied by the caller.

Postconditions:
@ensures The returned Level has the requested depth.
@ensures The starting position and stairs are connected when intervening doors are treated as openable.
@ensures Initial entities occupy distinct ordinary floor tiles outside the entire starting room.
@ensures Initial entities do not occupy stairs or blocked terrain.
@ensures All random choices use the supplied RNG.

Algorithm description:
Build rectangular and depth-dependent circular rooms, connect them with corridors, and place closed doors and obstacles without sealing routes. Retain the actual shape of the first room as the spawning exclusion area. Select a distant floor tile for the stairs. Shuffle eligible spawn positions and consume them without replacement. Target 4 + min(depth, 16) enemies and 3 + min(depth / 3, 5) items, limiting placement to available tiles and reserving item space on cramped floors.

Select enemy species by depth. Slimes are available at depth zero with weight 1, ghosts at depth two with weight depth − 1, and goblins at depth four with weight (depth − 3)². Species below their minimum depth are excluded. Species use their existing base statistics. Select items uniformly from catalog entries whose level is no greater than saturating depth + 1. Return the populated Level. Visibility flags remain unset until Game requests a visibility update.

## 3.9 — Renderer CRC card: replace card

Class Name: Renderer
Description: Handles terminal game representation. Reads borrowed display data and draws the active screen using ASCII graphics.
Super classes: N/A

Contracts:
Contract 20: Render Game State
Description: Draws the supplied screen and available game information without modifying gameplay state.
- Draws the active screen — Game (3), through the application launcher.
- Draws previously seen terrain — Level (8).
- Builds themed entries for currently visible enemies and ground items — Level (10, 11), Enemy (12), Item (18).
- Draws the supplied player position, health, effects, equipment, and messages — Player (6), Game (1), through display views.
- Draws supplied inventory entries and game-over results — Inventory (17), Game (5), through display views.

Contract 21: Interpret Input
Description: The Terminal Interface provides this contract through Keybinds. Keybinds reads terminal input and maps key presses to actions for the active screen. The application launcher passes recognized actions to Game (2).

Private Responsibilities:
- Knows the Theme used for terrain, entities, text, and screen decoration.
- Draws terrain, items, enemies, and the player in that order.
- Clips the map to the viewport and centers it on the player or supplied camera focus.
- Enforces current visibility when drawing entities.

Comments:
Renderer reads display data and never changes gameplay state or exploration memory. Theme and Keybinds load their respective configuration files. The application launcher validates configuration before terminal setup, manages the terminal lifecycle, forwards actions, and requests redraws. Renderer does not advance turns or decide screen transitions. Player statistics, inventory rows, messages, and results are drawn when supplied; their gameplay integration remains unfinished.

## 3.9.1 — RenderGameState: replace Protocol 1

Protocol 1: render(frame, screen, view)

Method Signature (Rust):
fn render(&self, frame: &mut Frame, screen: &Screen, view: &RenderView<'_>)

Responsibilities and Collaborations:
- Draws the supplied active screen — Game (3), through the application launcher.
- Reads borrowed terrain and optional display information.
- Uses the configured Theme.

Preconditions:
@requires The application launcher supplies a terminal frame and the active screen.

Postconditions:
@ensures The frame contains the presentation of the supplied screen and display data.
@ensures Gameplay state and exploration memory remain unchanged.

Algorithm description:
Reset the frame’s display style. Select the draw function for MainMenu, Playing, Inventory, Paused, or GameOver. For Paused, draw the current gameplay view followed by the pause overlay. Use available display data without constructing missing gameplay state. The application launcher submits the frame to the terminal.

## 3.9.1 — RenderGameState: replace Protocol 2

Protocol 2: draw_level(frame, area, level, theme)

Method Signature (Rust):
fn draw_level(frame: &mut Frame, area: Rect, level: &LevelView<'_>, theme: &Theme)

Description: Draws the visible viewport from borrowed terrain and entity display entries.

Preconditions:
@requires The caller supplies a frame, drawing area, level view, and theme.

Postconditions:
@ensures Unseen terrain remains blank.
@ensures Previously seen terrain is drawn with its configured appearance.
@ensures Entities are drawn only when their current tile is visible.
@ensures Gameplay state and exploration memory remain unchanged.

Algorithm description:
Center the viewport on an in-bounds player position, otherwise the supplied in-bounds camera focus, otherwise the map origin. Clip the viewport to the drawing area and map bounds. Draw seen terrain, then visible items, enemies, and the player. Multiple items on one tile are drawn in storage order, with the last item on top. Visibility is checked during drawing even when entries have already been filtered by the entity builders.

## 3.9.1 — RenderGameState: replace Protocols 3–5

Protocol 3: draw_status(frame, area, status, theme)
Method Signature (Rust):
fn draw_status(frame: &mut Frame, area: Rect, status: &StatusView<'_>, theme: &Theme)

Description: Draws supplied health, effects, and equipment without changing player state. Message rendering is provided separately by draw_messages(). Missing information is represented by placeholders.

Supporting Method Signature (Rust):
fn draw_messages(frame: &mut Frame, area: Rect, messages: &[&str], theme: &Theme)

Algorithm description:
Format the supplied status information. Draw the newest message lines that fit the available panel in chronological order. These functions do not retrieve or modify Player or MessageLog directly.

Protocol 4: Inventory Screen
Method Signature (Rust), in tui::screens::inventory:
fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme)

Algorithm description:
Draw the supplied inventory entries with their names, categories, optional selection keys, and equipped markers. Clamp the displayed selection to the available rows and scroll to the selected entry. Show an empty-pack message when no entries are supplied. Rendering does not change item ownership.

Protocol 5: GameOver Screen
Method Signature (Rust), in tui::screens::game_over:
fn draw(frame: &mut Frame, view: &RenderView<'_>, theme: &Theme)

Algorithm description:
Draw the supplied outcome, final depth, and control help. Use placeholders when results are absent. Rendering does not construct a game summary or end the session.

## 3.9.2 — Contract InterpretInput: replace protocol

Contract Identifier: R-C02
Contract Name: InterpretInput
Description: Reads terminal input and translates recognized key presses into borrowed Actions using global and screen-specific bindings. Keybinds provides this contract within the Terminal Interface.

Protocol 1: read_input(screen)
Method Signature (Rust):
fn read_input(&self, screen: &Screen) -> io::Result<Option<&Action>>

Responsibilities and Collaborations:
- Reads the next terminal event.
- Matches key presses against global and active-screen bindings.
- Returns the recognized action to the application launcher — Game (2).

Preconditions:
@requires The terminal is set up and keybind settings are loaded.

Postconditions:
@ensures A recognized key press returns Ok(Some(action)).
@ensures Unmapped keys, released keys, and other terminal events return Ok(None).
@ensures Terminal input failures return an error.
@ensures Game state remains unchanged.

Algorithm description:
Read one terminal event. For a key event, match the key and modifiers against global bindings and the bindings for the supplied screen. Return a reference to the configured Action when the event is a recognized key press. Otherwise, return no action. The application launcher passes the returned action to Game::process_action(). Configuration loading rejects unknown actions, invalid key descriptions, and conflicting bindings. Control help is built from the same bindings used for input.
