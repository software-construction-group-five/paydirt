# Basic Inventory Implementation Plan

**Goal:** Start Inventory contracts 15–17 at the same scope as the team's other structs.

**Architecture:** Reuse Robert's Item implementation and store carried items in a simple vector. Inventory handles storage and limits; Game and Player coordinate gameplay later.

**Tech Stack:** Existing Rust crate; no new dependencies.

**Spec:** `/Users/ramo._./Downloads/sdd-report.docx`, Inventory contracts 15–17.

## Constraints

- Maximum 5 consumables, 3 weapons, and 1 armor item.
- Use catalog indices (`u32`), matching the existing Item API.
- Duplicate catalog items are allowed; removal removes one copy.
- Keep equipment, pickup, and TUI wiring outside this first change.

## Implementation

- [x] Update from develop and reuse `src/game/item.rs` from Robert's branch.
- [x] Write Inventory tests for capacity boundaries, mixed categories, removing one copy, missing items, and contents output; observe failure before implementation.
- [x] Implement `new()`, `add_item(index: u32) -> bool`, `remove_item(index: u32) -> Option<Item>`, and `return_inventory() -> String` in `src/game/inventory.rs`.
- [x] Update the Player test fixture to construct `Inventory::new()`.
- [x] Run the complete test suite and check formatting of changed files.

## Review Focus

Full-category rejection must preserve contents and not block other categories. Removal must free capacity, return the stored item, and preserve other copies. Empty contents must be readable. Invalid add indices follow the existing catalog's documented panic behavior.

## Verification

`cargo test --locked --offline`: 38 passed, 0 failed. New tests initially failed because Inventory had no constructor or methods. `rustfmt --check --edition 2024 src/game/inventory.rs src/game/item.rs` and `git diff --check` pass.
