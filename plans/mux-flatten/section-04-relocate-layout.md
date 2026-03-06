---
section: "04"
title: "Relocate Layout Modules"
status: complete
goal: "SplitTree, FloatingLayer, Rect, layout compute, and nav all live in oriterm, not oriterm_mux"
depends_on: ["01"]
late_depends_on: ["03", "05"]
sections:
  - id: "04.1"
    title: "Copy SplitTree to oriterm"
    status: complete
  - id: "04.2"
    title: "Copy FloatingLayer to oriterm"
    status: complete
  - id: "04.3"
    title: "Copy Rect and Layout Compute to oriterm"
    status: complete
  - id: "04.4"
    title: "Copy Nav to oriterm"
    status: complete
  - id: "04.5"
    title: "Delete Mux Layout Module (LATE — after sections 03+05)"
    status: complete
  - id: "04.6"
    title: "Completion Checklist"
    status: complete
---

# Section 04: Relocate Layout Modules

**Status:** In Progress (04.1-04.4 copy phase complete; 04.5 deletion blocked on 03+05)
**Goal:** All layout and navigation code lives in `oriterm` (the GUI binary).
`oriterm_mux` has no `layout/` or `nav/` modules.

**Context:** The mux currently owns `SplitTree`, `FloatingLayer`, `Rect`,
layout computation, and directional navigation. These are all presentation
concepts — they describe how panes are spatially arranged for rendering.
A non-GUI client (SSH attach) would not use any of this.

**Depends on:** Section 01 (GUI session types exist as the landing zone).

**IMPORTANT: Two-phase execution.** Steps 04.1-04.4 COPY layout code into
`oriterm/src/session/`. These are additive and can run in parallel with
section 02. Step 04.5 DELETES `layout/` and `nav/` from `oriterm_mux` and
depends on sections 03 AND 05 completing first (all internal mux consumers
must be stripped before the modules can be deleted).

**Co-implementation with Section 02:** The GUI session types created in
Section 01 need these layout modules. Section 02 (migrating oriterm)
and Section 04.1-04.4 (copying layout) should land together so the GUI's
`Tab` struct can own a `SplitTree`.

After 04.1-04.4, `oriterm/src/session/` will contain 9 submodules: `id`,
`tab`, `window`, `registry`, `split_tree`, `floating`, `rect`, `compute`,
`nav`. This is acceptable -- they form a coherent domain (GUI session
model) and each submodule is individually small. `session/mod.rs` stays
under 500 lines (~35 lines of mod declarations and re-exports).

---

## 04.1 Copy SplitTree to oriterm

**File(s):**
- Source: `oriterm_mux/src/layout/split_tree/mod.rs` (177 lines),
  `oriterm_mux/src/layout/split_tree/mutations.rs` (381 lines),
  `oriterm_mux/src/layout/split_tree/tests.rs` (769 lines)
- Destination: `oriterm/src/session/split_tree/`

- [x] Copy `split_tree/` directory to `oriterm/src/session/split_tree/`
      (do NOT delete from mux yet -- that happens in step 04.5).
      The directory contains `mod.rs` (177 lines), `mutations.rs`
      (381 lines), and `tests.rs` (769 lines). Copy all three.
- [x] Update imports: `crate::id::PaneId` becomes
      `oriterm_mux::PaneId` (SplitTree only needs PaneId)
- [x] Update `SplitDirection` imports if it moved
- [x] Ensure sibling `tests.rs` pattern is maintained (test-organization.md):
      `mod.rs` ends with `#[cfg(test)] mod tests;`, `tests.rs` has no wrapper module
- [x] Re-export from `oriterm/src/session/mod.rs`:
      `pub use split_tree::{SplitDirection, SplitTree};`
      (deferred — re-exports will be added when consumers exist, to avoid unused import warnings)
- [x] `cargo test --target x86_64-pc-windows-gnu` passes for split_tree tests in new location

---

## 04.2 Copy FloatingLayer to oriterm

**File(s):**
- Source: `oriterm_mux/src/layout/floating/mod.rs` (305 lines),
  `oriterm_mux/src/layout/floating/tests.rs` (430 lines)
- Destination: `oriterm/src/session/floating/`

- [x] Copy `floating/` to `oriterm/src/session/floating/`
      (do NOT delete from mux yet — that happens in step 04.5)
- [x] Update imports: `PaneId` from `oriterm_mux`, `Rect` from local
- [x] Keep pixel-space operations (hit_test, snap_to_edge, centered) —
      these are GUI-owned presentation logic, appropriate in this location
- [x] Ensure sibling `tests.rs` pattern is maintained (test-organization.md)
- [x] Re-export from session: `pub use floating::{FloatingLayer, FloatingPane};`
      (deferred — re-exports will be added when consumers exist, to avoid unused import warnings)
- [x] `./build-all.sh && ./test-all.sh` passes for floating tests in new location

---

## 04.3 Copy Rect and Layout Compute to oriterm

**File(s):**
- Source: `oriterm_mux/src/layout/rect.rs` (29 lines),
  `oriterm_mux/src/layout/compute/mod.rs` (349 lines),
  `oriterm_mux/src/layout/compute/tests.rs` (968 lines)
- Destination: `oriterm/src/session/compute/` (or `oriterm/src/session/layout/`)

Per test-organization.md, a file with tests must be a directory module.
Convert `rect.rs` to a directory module when copying.
- [x] Copy `rect.rs` to `oriterm/src/session/rect/mod.rs` (not `rect.rs`)
      and add `#[cfg(test)] mod tests;` at the bottom
      (do NOT delete from mux yet — that happens in step 04.5)
- [x] Create `oriterm/src/session/rect/tests.rs` with unit tests for
      `Rect::contains_point()` (boundary/interior/exterior) and
      `Rect::center()`
- [x] Copy `compute/` to `oriterm/src/session/compute/`
      (do NOT delete from mux yet — that happens in step 04.5)
- [x] Update imports: `PaneId` from `oriterm_mux`, layout types from
      local session module
- [x] `PaneLayout`, `DividerLayout`, `LayoutDescriptor` -- all GUI types now
- [x] Ensure sibling `tests.rs` pattern is maintained (test-organization.md)
- [x] Re-export key types from session module
      (deferred — re-exports will be added when consumers exist, to avoid unused import warnings)

---

## 04.4 Copy Nav to oriterm

**File(s):**
- Source: `oriterm_mux/src/nav/mod.rs` (235 lines),
  `oriterm_mux/src/nav/tests.rs` (727 lines)
- Destination: `oriterm/src/session/nav/`

- [x] Copy `nav/` to `oriterm/src/session/nav/`
      (do NOT delete from mux yet — that happens in step 04.5)
- [x] Update imports: `PaneLayout` from local session, `PaneId` from
      `oriterm_mux`, `Direction` stays with nav
- [x] Ensure sibling `tests.rs` pattern is maintained (test-organization.md)
- [x] Re-export: `pub use nav::Direction;`
      (deferred — re-exports will be added when consumers exist, to avoid unused import warnings)
- [x] `./build-all.sh && ./test-all.sh` passes for nav tests in new location

---

## 04.5 Delete Mux Layout Module

**WARNING: This step depends on sections 03 AND 05 being complete.** It runs
in Phase 3b of the implementation sequence, not during Phase 1 with 04.1-04.4.

**File(s):** `oriterm_mux/src/layout/` (entire directory)

- [x] Pre-condition check: all internal consumers removed (03 + 05 complete)
- [x] Delete `oriterm_mux/src/layout/` entirely
- [x] Delete `oriterm_mux/src/nav/` entirely
- [x] Remove `pub mod layout;` and `pub mod nav;` from `lib.rs`
- [x] Remove layout/nav re-exports from `lib.rs`
- [x] Verify: `grep -rn "layout::\|nav::" oriterm_mux/src/` returns zero results

---

## 04.6 Completion Checklist

### Phase 1 gate (after 04.1-04.4):
- [x] `oriterm/src/session/` contains: `split_tree/`, `floating/`,
      `rect/`, `compute/`, `nav/`
- [x] All layout tests pass in their new location
- [x] Mux layout modules still exist (not deleted yet)
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

### Phase 3b gate (after 04.5, which requires 03+05 complete):
- [x] `oriterm_mux/src/layout/` does not exist
- [x] `oriterm_mux/src/nav/` does not exist
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria (Phase 1):** Layout and navigation code compiles and
passes tests in `oriterm/src/session/`. Mux copies still exist.

**Exit Criteria (Phase 3b):** Mux has no layout or nav modules. All
builds and tests green.
