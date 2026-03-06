---
section: "02"
title: "Migrate oriterm to Own Session Types"
status: complete
goal: "All oriterm code uses local session types instead of mux-owned tab/window/layout types"
depends_on: ["01"]
sections:  # Listed in execution order (02.5 before 02.2 because sync is prerequisite for queries)
  - id: "02.1"
    title: "Swap ID Types"
    status: complete
  - id: "02.5"
    title: "Synchronize Session State from Mux Events"
    status: complete
  - id: "02.2"
    title: "Swap Session Queries"
    status: complete
  - id: "02.3"
    title: "Swap Notification Handling"
    status: complete
  - id: "02.4"
    title: "Update MuxBackend Consumers"
    status: complete
  - id: "02.6"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Migrate oriterm to Own Session Types

**Status:** Complete
**Goal:** Every import of `TabId`, `WindowId`, `MuxTab`, `MuxWindow`,
`SessionRegistry` in `oriterm` comes from `crate::session`, not from
`oriterm_mux`. The mux types are no longer consumed by the GUI.

**Context:** The GUI currently imports these types from `oriterm_mux` and
queries them via `mux.session()`. After this section, the GUI maintains its
own session state and only talks to the mux for pane operations.

**Depends on:** Section 01 (GUI session types must exist first).

**RISK: MEDIUM.** The remaining semantic migration (02.2, 02.4) is harder
than the mechanical swaps (02.3). The sync mechanism (02.5) is already
done, so the GUI now dual-writes session state. **Execution order:
02.1 (done) -> 02.5 (done) -> 02.2 -> 02.3 -> 02.4 -> 02.6**.

---

## 02.1 Swap ID Types

**File(s):** All files in `oriterm/src/` that import `TabId` or `WindowId`
from `oriterm_mux`

Known consumers (from audit):
- `oriterm/src/window/mod.rs` — `WindowId as MuxWindowId`
- `oriterm/src/app/mod.rs` — `WindowId as MuxWindowId`
- `oriterm/src/app/window_management.rs` — `WindowId as MuxWindowId`
- `oriterm/src/app/tab_management/mod.rs` — `TabId, WindowId as MuxWindowId`
- `oriterm/src/app/pane_ops/helpers.rs` — `PaneId, TabId`
- `oriterm/src/app/mux_pump/mod.rs` — `PaneId, WindowId as MuxWindowId`
- `oriterm/src/app/constructors.rs` — `oriterm_mux::WindowId::from_raw` (inline qualified path)
- `oriterm/src/app/tab_drag/mod.rs`, `tear_off.rs`, `merge.rs` — `TabId`
- `oriterm/src/app/tab_management/move_ops.rs` — `TabId, WindowId as MuxWindowId`
- `oriterm/src/app/tests.rs` — `TabId, WindowId, MuxTab, MuxWindow`
- `oriterm/src/app/tab_management/tests.rs` — `TabId, WindowId, MuxWindow`

- [x] Replace all `use oriterm_mux::{TabId, WindowId}` with
      `use crate::session::{TabId, WindowId}` across oriterm
- [x] Remove the `as MuxWindowId` alias — now `WindowId as SessionWindowId`
      (session type, bridged to mux via `.to_mux()`)
- [x] Rename `TermWindow.mux_window_id` field and `mux_window_id()` accessor
      to `session_window_id` / `session_window_id()` — uses `SessionWindowId`
      alias where `winit::window::WindowId` is also in scope
- [x] Verify: `cargo build --target x86_64-pc-windows-gnu` succeeds

---

## 02.2 Swap Session Queries

**File(s):** All `oriterm/src/app/` files that call `mux.session()` (see
caller list below), plus GPU consumer files that import layout types

The GUI currently calls `mux.session().get_window(id)` and
`mux.session().get_tab(id)` to read `MuxWindow`/`MuxTab`, where `mux`
is obtained from the `MuxBackend` trait object. These need to read from
the GUI's own `SessionRegistry` instead.

Known callers of `mux.session()` (from audit):
- `oriterm/src/app/mod.rs` — `active_pane_context()` + `build_tab_entries()` (5 calls)
- `oriterm/src/app/pane_ops/mod.rs` — 3 calls
- `oriterm/src/app/pane_ops/helpers.rs` — 3 calls (`active_pane_context`)
- `oriterm/src/app/redraw/multi_pane.rs` — window + tab lookup
- `oriterm/src/app/tab_management/mod.rs` — `tab_count`, `window_for_tab`, get_window (8 calls)
- `oriterm/src/app/tab_drag/mod.rs` — window tab list
- `oriterm/src/app/floating_drag.rs` — tab lookup (3 calls)
- `oriterm/src/app/session_sync.rs` — mux tab/window sync (4 calls)

- [x] Add `session: SessionRegistry` field to `App` struct (done in 02.5)
- [x] Replace all `mux.session().get_window(id)` with
      `self.session.get_window(id)` throughout
- [x] Replace all `mux.session().get_tab(id)` with
      `self.session.get_tab(id)` throughout
- [x] Tab bar building reads from `self.session` instead of mux
- [x] `active_pane_context()` in `pane_ops/helpers.rs` resolves
      window -> tab -> active_pane from local session
- [x] Update test helpers to build local session state instead of
      injecting into mux
- [ ] **GPU consumer files** that import layout types from `oriterm_mux`:
  - `oriterm/src/gpu/pane_cache/mod.rs` — `oriterm_mux::layout::PaneLayout`
  - `oriterm/src/gpu/pane_cache/tests.rs` — `PaneLayout`, `Rect`
  - `oriterm/src/gpu/window_renderer/multi_pane.rs` — `DividerLayout`, `Rect`
  - `oriterm/src/app/redraw/multi_pane.rs` — `DividerLayout`, `LayoutDescriptor`, `PaneLayout`, `Rect`, `compute_all`
  - `oriterm/src/app/divider_drag.rs` — `SplitDirection`, `DividerLayout`, `Rect`
  - `oriterm/src/app/floating_drag.rs` — `Rect`, `snap_to_edge`
  - `oriterm/src/app/window_context.rs` — `DividerLayout`
  - `oriterm/src/app/pane_ops/helpers.rs` — `Rect`, `SplitDirection`
  - `oriterm/src/app/pane_ops/mod.rs` — `SplitDirection`, `Direction`
  These switch to `crate::session::` imports after section 04 lands.
  **Coordinate with section 04** so imports point to the right place.

---

## 02.3 Swap Notification Handling

**File(s):** `oriterm/src/app/mux_pump/mod.rs`

The mux currently emits `MuxNotification` variants that reference tabs and
windows. After flattening, the mux only emits pane-level notifications.
The GUI needs to translate pane events into its own session updates.

**Note:** These checklist items use the post-rename variant names (`PaneOutput`,
`PaneBell`) from section 03.2. At execution time, section 02.3 runs before
section 03.2. Handle these in the pre-rename form (`PaneDirty`, `Alert`)
during implementation, then update names when section 03.2 lands.

- [x] Map `MuxNotification::PaneDirty(pid)` — already correct (pane-level)
- [x] Map `MuxNotification::PaneClosed(pid)` to: find which tab contains
      this pane (local session lookup), remove from split tree, handle
      last-pane-in-tab / last-tab-in-window / last-window cases locally
- [x] Map `MuxNotification::PaneTitleChanged(pid)` — already correct
- [x] Map `MuxNotification::Alert(pid)` — already correct
- [x] Stop consuming: `TabLayoutChanged`, `FloatingPaneChanged`,
      `WindowTabsChanged` — gated on daemon mode only (embedded mode
      mutations are local-first). `WindowClosed` and `LastWindowClosed`
      kept as safety nets for daemon-originated closes.
- [x] The GUI generates its own "session changed" events internally
      when it mutates tabs/windows (local-first in 02.4)

---

## 02.4 Update MuxBackend Consumers

**File(s):** `oriterm/src/app/` (various)

The `MuxBackend` trait currently has methods for tab/window operations.
After flattening, it only has pane operations. The GUI's tab/window
operations become local mutations.

- [x] Tab creation: `mux.spawn_pane()` + local `Tab::new()` +
      `session.add_tab()` + `window.add_tab()` (init, new_tab, window_management)
- [x] Tab closing: per-pane `mux.close_pane(pid)` + local session removal
      (close_tab, close_focused_pane, handle_pane_closed)
- [x] Window creation: `mux.create_window()` (mux-side for pane routing) +
      local `Window::new()` + `session.add_window()` — mux.create_window
      stays until section 03 strips mux window concept
- [x] Tab moves: local `win.remove_tab()` + `win.add_tab/insert_tab_at()`
      (move_tab_to_window, tear_off, merge). Daemon-mode move_tab_to_window
      still uses mux RPC intentionally.
- [x] Splits: `mux.spawn_pane()` + local `tab.tree().split_at()` +
      `tab.replace_layout()` (split_pane)
- [x] All layout mutations local: zoom, float/tile, divider drag, floating
      drag, resize_toward, equalize, undo/redo, reorder, cycle, switch
- [x] Verify: all tab/window/layout operations are local; only pane
      spawn/close/resize/write go through the mux

---

## 02.5 Synchronize Session State from Mux Events

The current `create_tab` flow is `GUI -> mux.create_tab(window_id, config, theme)` which
returns `(TabId, PaneId)`. After flattening, the mux has no tab concept, so the flow becomes:

1. GUI calls `mux.spawn_pane(config, theme)` to get `PaneId`
2. GUI creates a local `Tab` via `Tab::new(tab_id, pane_id)`
3. GUI registers the tab via `self.session.add_tab(tab)` and
   `self.session.get_window_mut(wid).add_tab(tab_id)`
4. GUI's session now owns the full state

- [x] Implement the new spawn flow in `App`:
  - `MuxBackend::spawn_pane()` added (standalone pane, no tab context)
  - Dual-write: `session_add_tab` syncs local session after `mux.create_tab()`
  - `session_add_window` (inline) syncs after `mux.create_window()`
- [x] Implement the new close flow:
  - `session_remove_tab` syncs local session after `mux.close_tab()`
  - `session_remove_window` syncs after `mux.close_window()`
- [x] Implement the new split flow:
  - `session_sync_tab_layout` syncs local split tree after layout mutations
  - Covers: split, zoom, float/tile toggle, undo/redo, active pane change
- [x] Implement ID allocation for `TabId`/`WindowId` locally in `SessionRegistry`
  - `IdAllocator<T>` implemented (monotonic, type-safe)
  - Not yet used — IDs still from mux during transition (switched in 02.4)

---

## 02.6 Completion Checklist

- [ ] Zero imports of `TabId`, `WindowId`, `MuxTab`, `MuxWindow`,
      `SessionRegistry` from `oriterm_mux` in `oriterm/src/`
      (remaining: bridge types in `session/id/`, daemon sync in
      `session_sync.rs`/`mux_pump/`, notification types in `mux_pump/` —
      these are intentional until section 03 strips mux tab/window)
- [x] `App` owns a local `SessionRegistry`
- [x] All session queries read from local state
- [x] All session mutations are local (embedded mode)
- [x] Only pane operations go through `MuxBackend` (plus daemon-mode
      `create_window`/`move_tab_to_window` — stripped in section 03)
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria:** `grep -r "oriterm_mux.*TabId\|oriterm_mux.*WindowId\|oriterm_mux.*MuxTab\|oriterm_mux.*MuxWindow\|oriterm_mux.*SessionRegistry" oriterm/src/`
returns zero results. All builds and tests green.
