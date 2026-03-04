---
section: "08"
title: Client-Side Mark Mode
status: complete
goal: Mark cursor state lives on App, motion computations use SnapshotGrid
sections:
  - id: "08.1"
    title: Move mark cursor state to App
    status: complete
  - id: "08.2"
    title: Rewire mark mode entry points
    status: complete
  - id: "08.3"
    title: Refactor handle_mark_mode_key
    status: complete
  - id: "08.4"
    title: Wire mark cursor into rendering
    status: complete
  - id: "08.5"
    title: Completion Checklist
    status: complete
---

# Section 08: Client-Side Mark Mode

**Status:** ✅ Complete
**Goal:** Mark cursor state is owned by `App`, motion computations use `SnapshotGrid`. `Pane::enter_mark_mode`, `exit_mark_mode`, `set_mark_cursor` are never called from the GUI.

**Crate:** `oriterm` (refactored mark_mode)
**Depends on:** Section 07 (SnapshotGrid), Section 04 (scroll through MuxBackend)
**Key files:**
- `oriterm/src/app/mod.rs` — App struct (add mark cursor map)
- `oriterm/src/app/mark_mode/mod.rs` — refactor
- `oriterm/src/app/redraw/mod.rs` — read mark cursor from App state

---

## 08.1 Move Mark Cursor State to App

**File:** `oriterm/src/app/mod.rs`

- [x] Add field: `mark_cursors: HashMap<PaneId, MarkCursor>`
- [x] Add helper methods:
  ```rust
  fn pane_mark_cursor(&self, pane_id: PaneId) -> Option<MarkCursor> {
      self.mark_cursors.get(&pane_id).copied()
  }
  fn is_mark_mode(&self, pane_id: PaneId) -> bool {
      self.mark_cursors.contains_key(&pane_id)
  }
  fn enter_mark_mode(&mut self, pane_id: PaneId) {
      // Read cursor position from snapshot
      let mux = self.mux.as_mut().expect("mux");
      mux.scroll_to_bottom(pane_id);
      if mux.is_pane_snapshot_dirty(pane_id) || mux.pane_snapshot(pane_id).is_none() {
          mux.refresh_pane_snapshot(pane_id);
      }
      if let Some(snapshot) = mux.pane_snapshot(pane_id) {
          let mc = MarkCursor {
              row: StableRowIndex(snapshot.stable_row_base + snapshot.cursor.row as u64),
              col: snapshot.cursor.col as usize,
          };
          self.mark_cursors.insert(pane_id, mc);
      }
  }
  fn exit_mark_mode(&mut self, pane_id: PaneId) {
      self.mark_cursors.remove(&pane_id);
  }
  fn set_mark_cursor(&mut self, pane_id: PaneId, cursor: MarkCursor) {
      self.mark_cursors.insert(pane_id, cursor);
  }
  ```
- [x] Clean up `mark_cursors` when a pane is closed

---

## 08.2 Rewire Mark Mode Entry Points

Mark mode is entered and dispatched from `keyboard_input/`, not just `mark_mode/`. All these callsites need migration.

**File:** `oriterm/src/app/keyboard_input/action_dispatch.rs`

- [x] `Action::EnterMarkMode` (line 79): Replace `self.active_pane_mut()` → `pane.enter_mark_mode()` with `self.enter_mark_mode(pane_id)` (uses App's client-side mark cursor from 08.1)
- [x] `Action::SelectCommandOutput` (line 174): Replace `self.active_pane_mut()` → `pane.select_command_output()` with a MuxBackend method (see Section 13 for remaining migration)
- [x] `Action::SelectCommandInput` (line 183): Same pattern — `pane.select_command_input()` needs MuxBackend or snapshot-based equivalent

**File:** `oriterm/src/app/keyboard_input/mod.rs`

- [x] Mark mode dispatch block (lines 168–189): Replace `m.pane_mut(pane_id)` → `pane.is_mark_mode()` + `handle_mark_mode_key(pane, ...)` with:
  - `self.is_mark_mode(pane_id)` check (uses App's mark_cursors map)
  - `handle_mark_mode_key` takes `&mut self` (App) + snapshot + key event instead of `&mut Pane`
- [x] Context menu `SelectAll` (line 430): Replace `self.active_pane_mut()` → `mark_mode::select_all(pane)` with `self.select_all(pane_id)` using SnapshotGrid

---

## 08.3 Refactor `handle_mark_mode_key`

**File:** `oriterm/src/app/mark_mode/mod.rs`

- [x] Change `handle_mark_mode_key` signature: remove `pane: &mut Pane`, take `SnapshotGrid` + mark cursor + selection state
  - Or: make it a method on `App` so it can access all state
- [x] `select_all(pane)` (line 281–308): refactor to use `SnapshotGrid`:
  - `pane.terminal().lock().grid()` → `SnapshotGrid::from(snapshot)`
  - `StableRowIndex::from_absolute(g, 0)` → `grid.viewport_to_stable_row(0)` (or compute from snapshot metadata)
  - `g.scrollback().len() + g.lines() - 1` → `grid.scrollback_len() + grid.lines() - 1`
  - Update App's selection state instead of `pane.set_selection()`
- [x] `apply_motion` (line 132–190): refactor to use `SnapshotGrid`:
  - `pane.terminal().lock().grid()` → `SnapshotGrid`
  - `GridBounds` computed from snapshot: `total_rows = scrollback_len + lines`, `cols`, `visible_lines = lines`
  - `extract_word_context` → use `SnapshotGrid::word_boundaries`
  - `pane.clear_selection()` / `pane.set_mark_cursor()` → update App state
- [x] `ensure_visible` (line 311–338): refactor:
  - `pane.terminal().lock().grid()` → snapshot metadata
  - `pane.scroll_display(d)` → `mux.scroll_display(pane_id, d)`
- [x] `extend_or_create_selection` (line 239–278): update to use App's selection state instead of `pane.selection()` / `pane.set_selection()`
- [x] Remove `use oriterm_mux::pane::{MarkCursor, Pane}` import
  - Re-export `MarkCursor` from `oriterm_mux` root (`pub use pane::MarkCursor`) so `oriterm` can import `oriterm_mux::MarkCursor` without importing the `pane` module

**File:** `oriterm/src/app/mark_mode/motion.rs`

- [x] No changes needed — `motion.rs` is already pure (takes `AbsCursor`, `GridBounds`, `WordContext`). It doesn't access `Pane` at all.

---

## 08.4 Wire Mark Cursor into Rendering

**File:** `oriterm/src/app/redraw/mod.rs` (around line 140–148)

- [x] Replace `pane.mark_cursor().and_then(...)` with:
  ```rust
  frame.mark_cursor = self.pane_mark_cursor(pane_id).and_then(|mc| {
      let (line, col) = mc.to_viewport(frame.content.stable_row_base, frame.rows())?;
      Some(MarkCursorOverride {
          line,
          column: Column(col),
          shape: CursorShape::HollowBlock,
      })
  });
  ```

**File:** `oriterm/src/app/redraw/multi_pane.rs`

- [x] Same change for the multi-pane annotation block

---

## 08.5 Completion Checklist

- [x] Mark cursor state on `App::mark_cursors` (not on `Pane`)
- [x] `handle_mark_mode_key` uses `SnapshotGrid` for motion
- [x] `ensure_visible` scrolls through `mux.scroll_display()`
- [x] Mark cursor renders correctly in both modes
- [x] Ctrl+Shift+M enters/exits mark mode in daemon mode
- [x] Arrow key motion, word motion, page up/down all work
- [x] Shift+arrow extends selection in mark mode
- [x] Zero `pane.enter_mark_mode()`, `pane.exit_mark_mode()`, `pane.set_mark_cursor()`, `pane.mark_cursor()` calls from `oriterm/`
- [x] Zero `use oriterm_mux::pane::Pane` in `mark_mode/`
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria:** Mark mode works in daemon mode. Mark cursor state owned by App.
