---
section: "07"
title: Client-Side Selection (SnapshotGrid)
status: not-started
goal: Selection state lives on App, operates on PaneSnapshot data — no Pane access
sections:
  - id: "07.1"
    title: Create SnapshotGrid adapter
    status: not-started
  - id: "07.2"
    title: Move selection state to App
    status: not-started
  - id: "07.3"
    title: Refactor mouse_selection to use SnapshotGrid
    status: not-started
  - id: "07.4"
    title: Selection invalidation from snapshot changes
    status: not-started
  - id: "07.5"
    title: Wire selection into rendering
    status: not-started
  - id: "07.6"
    title: Completion Checklist
    status: not-started
---

# Section 07: Client-Side Selection (SnapshotGrid)

**Status:** 📋 Planned
**Goal:** Selection state is owned by `App` (per-pane), operates on `PaneSnapshot` data via a `SnapshotGrid` adapter. No `Pane` access for selection.

**Crate:** `oriterm` (new module + refactored mouse_selection)
**Key files:**
- `oriterm/src/app/snapshot_grid/mod.rs` — NEW: SnapshotGrid adapter
- `oriterm/src/app/mod.rs` — App struct (add selection map)
- `oriterm/src/app/mouse_selection/mod.rs` — refactor to use SnapshotGrid
- `oriterm/src/app/mouse_selection/helpers.rs` — refactor
- `oriterm/src/app/redraw/mod.rs` — read selection from App state

---

## 07.1 Create SnapshotGrid Adapter

A thin wrapper around `&PaneSnapshot` that provides the grid query interface needed by selection, mark mode, and word boundary detection.

**File:** `oriterm/src/app/snapshot_grid/mod.rs` (NEW)

- [ ] Create the module directory and file
- [ ] Define `SnapshotGrid<'a>` struct wrapping `&'a PaneSnapshot`
- [ ] Implement accessor methods:
  ```rust
  pub fn cols(&self) -> usize { self.snapshot.cols as usize }
  pub fn lines(&self) -> usize { self.snapshot.cells.len() }
  pub fn scrollback_len(&self) -> usize { self.snapshot.scrollback_len as usize }
  pub fn display_offset(&self) -> usize { self.snapshot.display_offset as usize }
  pub fn stable_row_base(&self) -> u64 { self.snapshot.stable_row_base }
  ```
- [ ] Implement `cell_char(viewport_row, col) -> char`:
  ```rust
  pub fn cell_char(&self, row: usize, col: usize) -> char {
      self.snapshot.cells.get(row)
          .and_then(|r| r.get(col))
          .map_or(' ', |c| c.ch)
  }
  ```
- [ ] Implement `word_boundaries(viewport_row, col, delimiters) -> (usize, usize)`:
  - Extract the row's characters from snapshot cells
  - Apply the same word boundary logic as `oriterm_core::selection::word_boundaries`
  - Key difference: operates on `Vec<WireCell>` instead of `&Row`
  - May need to factor out the pure word-boundary logic into a function that takes `&[char]` + col + delimiters
- [ ] Implement `viewport_to_stable_row(viewport_line) -> StableRowIndex`:
  ```rust
  pub fn viewport_to_stable_row(&self, line: usize) -> StableRowIndex {
      StableRowIndex(self.stable_row_base().saturating_add(line as u64))
  }
  ```
- [ ] Implement `stable_row_to_viewport(stable) -> Option<usize>`:
  - `let delta = stable.0.checked_sub(self.stable_row_base())?;`
  - Return `Some(delta as usize)` only if `< self.lines()`, else `None`
- [ ] Implement `redirect_spacer(viewport_row, col) -> usize`:
  - Wide character spacer redirection (if a cell is a spacer, redirect to the primary cell)
  - Check if WireCell flags indicate spacer status (via `WireCellFlags`)
- [ ] Add `#[cfg(test)] mod tests;` with unit tests for word boundaries on constructed snapshots

**Design note:** The `word_boundaries` function in `oriterm_core::selection` currently takes `&Grid`. We may need to:
  1. Factor the pure logic into a helper that takes a char iterator, OR
  2. Duplicate the logic in `SnapshotGrid` (acceptable since it's ~20 lines), OR
  3. Add a trait that both `Grid` and `SnapshotGrid` implement

Option 2 is simplest and avoids touching `oriterm_core`. The logic is: scan left from col until delimiter/start, scan right until delimiter/end.

---

## 07.2 Move Selection State to App

**File:** `oriterm/src/app/mod.rs`

- [ ] Add field: `pane_selections: HashMap<PaneId, Selection>`
- [ ] Add helper methods:
  ```rust
  fn pane_selection(&self, pane_id: PaneId) -> Option<&Selection> {
      self.pane_selections.get(&pane_id)
  }
  fn set_pane_selection(&mut self, pane_id: PaneId, sel: Selection) {
      self.pane_selections.insert(pane_id, sel);
  }
  fn clear_pane_selection(&mut self, pane_id: PaneId) {
      self.pane_selections.remove(&pane_id);
  }
  fn update_pane_selection_end(&mut self, pane_id: PaneId, end: SelectionPoint) {
      if let Some(sel) = self.pane_selections.get_mut(&pane_id) {
          sel.end = end;
      }
  }
  ```
- [ ] Clean up `pane_selections` when a pane is closed (in `handle_mux_notification(PaneClosed(id))`)

---

## 07.3 Refactor `mouse_selection` to Use SnapshotGrid

**File:** `oriterm/src/app/mouse_selection/mod.rs`

- [ ] Change `handle_press` signature: remove `pane: &mut Pane` parameter, add `snapshot: &PaneSnapshot` (or `grid: &SnapshotGrid`)
  - Terminal lock accesses (lines 242–266) replaced with `SnapshotGrid` queries:
    - `g.cols()` → `grid.cols()`
    - `g.scrollback().len().saturating_sub(g.display_offset()) + l` → `grid.stable_row_base() as usize + l`
    - `StableRowIndex::from_absolute(g, abs)` → `grid.viewport_to_stable_row(l)`
    - `word_boundaries(g, abs, c, delimiters)` → `grid.word_boundaries(l, c, delimiters)`
    - `redirect_spacer(g, abs, c)` → `grid.redirect_spacer(l, c)`
  - `pane.selection().map(|s| s.mode)` → `app.pane_selection(pane_id).map(|s| s.mode)`
  - `pane.update_selection_end(point)` → `app.update_pane_selection_end(pane_id, point)`
  - `pane.set_selection(selection)` → `app.set_pane_selection(pane_id, selection)`
- [ ] Update the call site in App that calls `handle_press` — pass snapshot + update App state with the result
- [ ] Remove `use oriterm_mux::pane::Pane` import

**File:** `oriterm/src/app/mouse_selection/helpers.rs`

- [ ] `update_drag_endpoint`: replace `pane: &mut Pane` with snapshot/selection references
  - Terminal lock (lines 31–32) → `SnapshotGrid` queries
  - `pane.selection()` → passed-in `&Selection`
  - Return the updated `SelectionPoint` instead of mutating pane
- [ ] `handle_auto_scroll`: replace `pane.scroll_display(±1)` with a returned delta or callback
  - The caller (App) applies the scroll via `mux.scroll_display(pane_id, delta)`
  - Terminal lock (lines 132–134) → `SnapshotGrid` queries for display_offset

---

## 07.4 Selection Invalidation from Snapshot Changes

When terminal output arrives, the selection may become stale (scrollback grew, content shifted).

**File:** `oriterm/src/app/mux_pump/mod.rs`

- [ ] In `PaneDirty` notification handling: after refreshing the snapshot, check if `scrollback_len` changed compared to the previous snapshot
  - If scrollback grew (new output), invalidate the selection: `self.clear_pane_selection(pane_id)`
  - This matches the existing `Pane::check_selection_invalidation()` behavior but operates on snapshot metadata
- [ ] Also invalidate on structural snapshot changes that can stale selection coordinates:
  - `cols` changed (resize/reflow)
  - viewport line count changed (`cells.len()`)
  - `stable_row_base` discontinuity beyond normal scroll movement
- [ ] May need to store previous snapshot metadata per pane, or compare before/after refresh

---

## 07.5 Wire Selection into Rendering and State Reads

**File:** `oriterm/src/app/redraw/mod.rs` (around line 137–152)

- [ ] Replace `pane.selection().map(...)` with:
  ```rust
  frame.selection = self.pane_selection(pane_id)
      .map(|sel| FrameSelection::new(sel, frame.content.stable_row_base));
  ```
  - Note: `FrameSelection::new` takes `&Selection` and `stable_row_base` — this should work directly

**File:** `oriterm/src/app/redraw/multi_pane.rs`

- [ ] Same change for the multi-pane annotation block

**File:** `oriterm/src/app/keyboard_input/action_dispatch.rs`

- [ ] `Action::SmartCopy` (line 36): Replace `self.active_pane().is_some_and(|p| p.selection().is_some())` with `self.pane_selection(pane_id).is_some()` (selection now lives on App, not Pane)

**File:** `oriterm/src/app/mouse_input.rs`

- [ ] `open_grid_context_menu()` (line 128): Replace `self.active_pane().is_some_and(|p| p.selection().is_some())` with `self.active_pane_id().and_then(|id| self.pane_selection(id)).is_some()`

---

## 07.6 Completion Checklist

- [ ] `SnapshotGrid` adapter created with full test coverage
- [ ] Selection state on `App::pane_selections` (not on `Pane`)
- [ ] `handle_press`, `handle_drag`, `handle_release` use `SnapshotGrid`
- [ ] Auto-scroll during drag goes through `mux.scroll_display()`
- [ ] Selection renders correctly in both modes
- [ ] Selection cleared on new terminal output
- [ ] Zero `pane.selection()`, `pane.set_selection()`, `pane.clear_selection()` calls from `oriterm/`
- [ ] Zero `use oriterm_mux::pane::Pane` in `mouse_selection/`
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** Mouse text selection works in daemon mode. Selection state owned by App. `SnapshotGrid` is the only grid interface the GUI uses.
