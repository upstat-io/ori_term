---
section: "13"
title: Remove Pane from oriterm
status: not-started
goal: The Pane type is never imported in oriterm — type-level enforcement of the boundary
sections:
  - id: "13.1"
    title: Remove active_pane helpers
    status: not-started
  - id: "13.2"
    title: Migrate remaining pane_mut callsites
    status: not-started
  - id: "13.3"
    title: Remove pane()/pane_mut() from MuxBackend trait
    status: not-started
  - id: "13.4"
    title: Remove is_daemon_mode() usage
    status: not-started
  - id: "13.5"
    title: Final audit
    status: not-started
  - id: "13.6"
    title: Completion Checklist
    status: not-started
---

# Section 13: Remove Pane from oriterm

**Status:** 📋 Planned
**Goal:** The `Pane` type does not appear anywhere in `oriterm/src/`. The `pane()`, `pane_mut()`, `remove_pane()` methods are removed from `MuxBackend`. `is_daemon_mode()` is removed from all GUI code.

**Crate:** `oriterm` (cleanup), `oriterm_mux` (trait cleanup)
**Depends on:** ALL previous sections (01–12)

---

## 13.1 Remove `active_pane` Helpers

**File:** `oriterm/src/app/mod.rs`

- [ ] Remove `active_pane(&self) -> Option<&Pane>` method
- [ ] Remove `active_pane_mut(&mut self) -> Option<&mut Pane>` method
- [ ] Remove `active_pane_for_window(&self, winit_id) -> Option<&Pane>` method
- [ ] Remove `use oriterm_mux::pane::Pane` import
- [ ] Fix any compilation errors — all callers should have been migrated in Sections 02–12

---

## 13.2 Migrate Remaining Pane Callsites

Comprehensive inventory of pane access sites NOT addressed by Sections 02–12. All must be migrated before `pane()`/`pane_mut()` can be removed from the trait.

### `mux_pump/mod.rs`

- [ ] `PaneTitleChanged` handler (line 50): `pane_mut(id).set_title(title)` → titles already come via `MuxNotification`, consider reading from snapshot `title` field instead of mutating pane
- [ ] `PaneBell` handler (line 121): `pane_mut(id).set_bell()` → replace with client-side bell tracking (tab bar already has `ring_bell()`)
- [ ] `PaneDirty` handler: `pane_mut(id).check_selection_invalidation()` → handled by Section 07.4
- [ ] `PaneClosed` handler: `remove_pane(id)` → replace with `MuxBackend::cleanup_closed_pane(id)`
- [ ] Line 199: `m.pane(pane_id)` → check purpose and migrate

### `tab_management/mod.rs`

- [ ] `new_tab_in_window()` (line 23): `self.active_pane().and_then(|p| p.cwd())` — CWD query for inheriting working directory. Options: add `pane_cwd()` to MuxBackend, or add a `cwd` field to PaneSnapshot
- [ ] `cycle_tab()` (line 152): `self.active_pane_mut()` → `pane.clear_bell()` — replace with client-side bell state or `MuxBackend::clear_bell(pane_id)`
- [ ] `switch_to_tab()` (line 175): same pattern — `pane.clear_bell()`
- [ ] `build_tab_entries()` (lines 544–550): `mux.pane(pid)` → `pane.effective_title()`, `pane.icon_name()` — tab bar title/icon sync. Options: add title/icon to PaneSnapshot, or add `MuxBackend::pane_title(id)` / `pane_icon(id)` queries
- [ ] Pick one metadata source **before** removing `pane()`:
  - Snapshot-driven (`PaneSnapshot { title, icon_name, cwd }`), or
  - Dedicated lightweight metadata queries on `MuxBackend`
- [ ] Daemon path check: `MuxClient::refresh_window_tabs()` currently discards `MuxTabInfo::title`; either consume that title in local session/tab-bar data or rely entirely on snapshot metadata

### `keyboard_input/mod.rs` (not covered by Sections 04, 08)

- [ ] `handle_key_press_kitty()` (line 261): `self.active_pane()` → `pane.terminal().lock()` for reading `TermMode` and grid lines. Should use `mux.pane_mode(pane_id)` + snapshot data. (This is the kitty keyboard encoding path.)
- [ ] `handle_key_press_kitty()` (lines 398–402): context menu `SelectScheme` palette change — **covered in Section 05.5**
- [ ] `handle_overlay_result_action()` (line 430): `self.active_pane_mut()` → `mark_mode::select_all(pane)` — **covered in Section 08.2**

### `mouse_input.rs`

- [ ] `handle_mouse_press()` (line 174): `m.pane_mut(pane_id)` → passed to `mouse_selection::handle_press` — **covered in Section 07.3**
- [ ] `handle_mouse_drag()` (line 200): `m.pane_mut(pane_id)` → passed to `mouse_selection::handle_drag` — **covered in Section 07.3**

### `pane_ops.rs` (not covered by Section 03)

- [ ] `estimate_split_size()` (lines 441–448): `m.pane(source)` → `pane.terminal().lock().grid()` to read rows/cols. Replace with snapshot query: `mux.pane_snapshot(source).map(|s| (s.cells.len() as u16, s.cols))`

### `init/mod.rs`

- [ ] Initial palette application (line 293): `mux.pane(pane_id)` + palette — **covered in Section 05.5**

### `window_management.rs`

- [ ] Reconnect palette application (line 56): `mux.pane(pane_id)` — **covered in Section 05.5**

### `mod.rs` (not covered by Sections 05, 06)

- [ ] `write_pane_input()` (line 426): `mux.pane(pane_id)` to check `display_offset() > 0` before scroll_to_bottom — replace with snapshot check: `mux.pane_snapshot(pane_id).is_some_and(|s| s.display_offset > 0)`

### Tests (`app/` module tests)

- [ ] `mark_mode/tests.rs` currently locks `pane.terminal()` directly. Refactor tests to construct snapshot-driven state (`SnapshotGrid`, `pane_selections`, `mark_cursors`) instead of using `Pane`.
- [ ] `app/tests.rs` and related helpers should avoid `Pane`-typed assumptions once trait methods are removed; use session/snapshot fixtures via `MuxBackend` methods.

### New MuxBackend Methods

- [ ] `fn pane_cwd(&self, pane_id: PaneId) -> Option<String>` — for CWD inheritance in new tab creation. Embedded: reads from `pane.cwd()`. Daemon: could add to snapshot or as a dedicated RPC.
- [ ] `fn clear_bell(&mut self, pane_id: PaneId)` — clear bell state for tab switching. Embedded: `pane.clear_bell()`. Daemon: fire-and-forget PDU (or make bell purely client-side).
- [ ] `fn cleanup_closed_pane(&mut self, pane_id: PaneId)` — removes the pane from storage. Embedded: takes pane out + background Drop thread. Daemon: no-op.
- [ ] `fn select_command_output(&mut self, pane_id: PaneId)` / `fn select_command_input(&mut self, pane_id: PaneId)` — for shell-integration selection. These need server-side scrollback access to find prompt boundaries, so they're server-side operations returning a Selection.

---

## 13.3 Remove `pane()`/`pane_mut()` from MuxBackend Trait

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Remove `fn pane(&self, pane_id: PaneId) -> Option<&Pane>`
- [ ] Remove `fn pane_mut(&mut self, pane_id: PaneId) -> Option<&mut Pane>`
- [ ] Remove `fn remove_pane(&mut self, pane_id: PaneId) -> Option<Pane>`
- [ ] Consider keeping `fn pane_ids(&self) -> Vec<PaneId>` — useful for enumerating panes in config reload. Or replace with session registry queries.

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Remove the corresponding implementations (these delegate to `InProcessMux`)
- [ ] `InProcessMux` keeps its internal `pane()` / `pane_mut()` — these are used by the server dispatch, not the GUI

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Remove the stub implementations that return `None`

---

## 13.4 Remove `is_daemon_mode()` Usage

After unified rendering and MuxBackend methods, `is_daemon_mode()` should be unnecessary in the GUI.

- [ ] `grep -rn 'is_daemon_mode' oriterm/src/` — list all remaining uses
- [ ] For each: determine if the branch can be removed or unified
  - Rendering: already unified (Section 02)
  - Input: already unified (Section 06)
  - Tab management: may still use `is_daemon_mode()` for window spawning logic — evaluate case by case
- [ ] Remove usage where possible. Some legitimate uses may remain (e.g., `is_daemon_mode()` for window spawning via `--connect` vs local creation) — these are OK as they're about process lifecycle, not pane access.
- [ ] Remove `is_daemon_mode()` from `MuxBackend` trait if all uses are eliminated. Otherwise leave it.

---

## 13.5 Final Audit

- [ ] `grep -rn 'oriterm_mux::pane' oriterm/src/` — must return zero matches
- [ ] `grep -rn '\.terminal()' oriterm/src/` — must return zero matches
- [ ] `grep -rn '\.lock()' oriterm/src/app/` — should return zero matches related to terminal/grid (font/config mutexes are OK)
- [ ] `grep -rn 'Grid' oriterm/src/app/` — check for `oriterm_core::grid::Grid` imports (should be zero)
- [ ] `grep -rn 'use oriterm_core::' oriterm/src/app/` — audit remaining oriterm_core imports:
  - OK: `TermMode`, `CursorShape`, `Selection`, `SelectionPoint`, `StableRowIndex`, `Column`, `Rgb`, `Theme`
  - Not OK: `Grid`, `Term`, `FairMutex`, `Cell`, `Row`

---

## 13.6 Completion Checklist

- [ ] `Pane` type not imported in `oriterm/src/` (grep verification)
- [ ] `terminal()` not called from `oriterm/src/` (grep verification)
- [ ] `pane()` / `pane_mut()` / `remove_pane()` removed from `MuxBackend` trait
- [ ] All remaining `is_daemon_mode()` calls in `oriterm/` are for process lifecycle, not pane access
- [ ] Embedded mode works identically (regression test)
- [ ] Daemon mode works identically (manual test)
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** The `Pane` type cannot be accessed from `oriterm`. If it compiles, the boundary is enforced.
