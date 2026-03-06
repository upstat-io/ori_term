---
section: "03"
title: "Flatten Mux Core"
status: complete
goal: "oriterm_mux has zero tab/window/session concepts — only pane lifecycle and I/O"
depends_on: ["02"]
sections:
  - id: "03.1"
    title: "Strip InProcessMux"
    status: complete
  - id: "03.2"
    title: "Simplify MuxNotification"
    status: complete
  - id: "03.3"
    title: "Remove Session Types"
    status: complete
  - id: "03.4"
    title: "Remove ID Types"
    status: complete
  - id: "03.5"
    title: "Flatten PaneRegistry"
    status: complete
  - id: "03.6"
    title: "Purge UI Comments"
    status: complete
  - id: "03.7"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Flatten Mux Core

**Status:** Not Started
**Goal:** `oriterm_mux` contains zero references to tabs, windows, sessions,
layouts, GUI, winit, tab bars, or any presentation concept. It is a flat
pane server.

**Context:** After Section 02, no external code depends on mux session types.
This section deletes them.

**Depends on:** Section 02 (all consumers migrated away).

**RISK: HIGH.** This section modifies the mux's public API contracts
(`ClosePaneResult`, `spawn_pane` signature, `PaneEntry` fields,
`MuxNotification` variants). The test file `in_process/tests.rs` is
5,310 lines. Execute subsections in strict order with build+test
verification between each step.

**03.1 phasing:** 03.1 is the largest sub-step (~500 lines of production
code deleted + ~3,500 lines of test code rewritten). It is split into
three phases (A, B, C) with build gates between each. See the Phase
A/B/C headings below.

**03.2 atomicity:** 03.2 renames `PaneDirty` to `PaneOutput` and `Alert`
to `PaneBell`. These variants are consumed in `server/mod.rs`,
`server/notify/mod.rs`, `backend/client/rpc_methods.rs`, and
`backend/client/transport/reader.rs`. The rename MUST update all emit
AND consume sites in one atomic pass to maintain buildability.

---

## 03.1 Strip InProcessMux

**File(s):** `oriterm_mux/src/in_process/mod.rs`,
`oriterm_mux/src/in_process/event_pump.rs`,
`oriterm_mux/src/in_process/tab_ops.rs`,
`oriterm_mux/src/in_process/floating_ops.rs`

The `InProcessMux` currently orchestrates pane/tab/window CRUD. After
flattening, it only does pane CRUD.

**Execution order is critical.** The naive order (delete source files
first, then fix tests) will NOT build in intermediate states. Execute
phases A -> B -> C strictly, with build+test verification between each.

### Phase A: Remove tab/window/floating tests from `in_process/tests.rs`

`in_process/tests.rs` is 5,310 lines. Estimate: ~60-70% of tests will
be deleted (tab/window/layout ops). The remaining ~30-40% cover pane
lifecycle, event pump basics, and close_pane behavior -- these need
factory function rewrites to use `spawn_pane()` instead of
`inject_test_tab()`.

Do this step BEFORE deleting any source files. The remaining tests must
still compile against the old API during this step.

- [x] Remove ALL tests that call methods being deleted in Phase B/C:
    - `tab_ops.rs` methods: split_pane, toggle_zoom, unzoom, equalize_panes,
      set_divider_ratio, resize_pane, undo_split, redo_split
    - `floating_ops.rs` methods: spawn_floating_pane, move_pane_to_floating,
      move_pane_to_tiled, move_floating_pane, resize_floating_pane,
      set_floating_pane_rect, raise_floating_pane
    - `event_pump.rs` methods: active_tab_id, set_active_pane, session,
      switch_active_tab, cycle_active_tab, reorder_tab, move_tab_to_window,
      move_tab_to_window_at
    - Window/tab CRUD: create_window, close_window, create_tab, close_tab
- [x] Keep tests for pane lifecycle (they still compile against old API):
      spawn_standalone_pane, close_pane, is_last_pane, poll_events,
      drain_notifications
- [x] **Gate: `./build-all.sh && ./test-all.sh` passes** (tests removed,
      source files still exist)

### Phase B: Strip embedded backend of deprecated trait methods

**DEVIATION:** The plan originally called for deleting `tab_ops.rs` and
`floating_ops.rs` entirely, but the server dispatch (`server/dispatch/mod.rs`)
still calls `create_tab`, `close_tab`, `split_pane`, `spawn_floating_pane`,
`move_tab_to_window`, `cycle_active_tab`, `switch_active_tab`, and `session()`.
These can't be deleted until Section 05 rewrites the server protocol.

Instead, Phase B:
- [x] Added default no-op implementations to `MuxBackend` trait for all
      deprecated tab/window/layout/floating methods
- [x] Removed embedded backend (`EmbeddedMux`) overrides for deprecated
      methods (now uses trait defaults)
- [x] Cleaned up embedded backend tests: removed all tests exercising
      deprecated methods, kept pane lifecycle and basic query tests
- [x] Updated contract tests to use `spawn_pane()` instead of `create_tab()`
- [x] Kept `tab_ops.rs`, `floating_ops.rs`, and server-consumed methods
      in `event_pump.rs` — deferred to Section 05
- [x] **Gate: `./build-all.sh && ./clippy-all.sh && ./test-all.sh` passes**

### Phase C: Strip InProcessMux struct and simplify API

**UNBLOCKED:** Section 05 complete. Server dispatch no longer calls tab/window
methods on `InProcessMux`. Partial progress below.

**Completed:**
- [x] Delete `tab_ops.rs` entirely (create_tab, close_tab, split_pane,
      toggle_zoom, unzoom, equalize_panes, set_divider_ratio, resize_pane,
      undo_split, redo_split)
- [x] Delete `floating_ops.rs` entirely (spawn_floating_pane,
      move_pane_to_floating, move_pane_to_tiled, move_floating_pane,
      resize_floating_pane, set_floating_pane_rect, raise_floating_pane)
- [x] Remove `switch_active_tab()`, `cycle_active_tab()`,
      `move_tab_to_window()` from `event_pump.rs`
- [x] Remove tab-scoped `spawn_pane(tab_id, ...)` from `mod.rs`
- [x] Remove `tab_alloc: IdAllocator<TabId>` field

**Completed:**
- [x] Strip `InProcessMux` struct of: `session: SessionRegistry`,
      `window_alloc: IdAllocator<WindowId>`, `create_window()`,
      `close_window()`, `handle_window_after_tab_removal()`,
      `is_last_pane()` (moved to oriterm session)
- [x] Simplify `close_pane()` — flat pane-only: unregister + emit `PaneClosed`
- [x] Delete `close_window()` entirely
- [x] Simplify `ClosePaneResult` — only `PaneRemoved` and `NotFound`
- [x] Remove `tab: Option<TabId>` field from `PaneEntry`
- [x] Update `InProcessMux::new()` — no session, no window allocator
- [x] Rewrite `in_process/tests.rs` — flat pane-only helpers,
      removed all tab/window/floating test infrastructure
- [x] Replace `inject_test_tab()`/`inject_split()` with `inject_test_pane()`
- [x] Update `embedded/tests.rs` to use `inject_test_pane()`
- [x] Update contract test `TestContext` — removed `window_id`/`tab_id` fields
- [x] Remove `session()` accessor from `event_pump.rs`
- [x] **Gate: `./build-all.sh && ./clippy-all.sh && ./test-all.sh` passes**

---

## 03.2 Simplify MuxNotification

**File(s):** `oriterm_mux/src/mux_event/mod.rs`

**Atomicity:** Treat the variant removal, renames, and consume-site
updates below as a SINGLE atomic step. Renaming enum variants without
updating all match arms will break the build.

**PARTIAL:** Renames completed. Variant removal blocked on Section 05
(server still emits `TabLayoutChanged`, `WindowClosed`, `LastWindowClosed`,
etc.).

- [x] Remove these tab/window variants from `MuxNotification`:
  - `TabLayoutChanged(TabId)` — removed (no tabs in mux)
  - `FloatingPaneChanged(TabId)` — removed (no tabs in mux)
  - `WindowTabsChanged(WindowId)` — removed (no windows in mux)
  - `WindowClosed(WindowId)` — removed (no windows in mux)
  - `LastWindowClosed` — removed (client decides when to exit)
- [x] Rename `PaneDirty(PaneId)` to `PaneOutput(PaneId)` (matches `MuxEvent::PaneOutput`)
- [x] Rename `Alert(PaneId)` to `PaneBell(PaneId)` (matches `MuxEvent::PaneBell`)
- [ ] Remaining variants:
  - `PaneOutput(PaneId)`
  - `PaneClosed(PaneId)`
  - `PaneTitleChanged(PaneId)`
  - `PaneBell(PaneId)`
  - `CommandComplete { pane_id, duration }`
  - `ClipboardStore { pane_id, clipboard_type, text }`
  - `ClipboardLoad { pane_id, clipboard_type, formatter }`
- [x] Update `Debug` impl to match new variants
- [x] Remove `TabId` and `WindowId` from `use crate::{PaneId, TabId, WindowId};`
      import (only `PaneId` remains)
- [x] Update emit sites in `event_pump.rs` (line 30 and line 65):
  - `MuxNotification::PaneDirty(id)` -> `MuxNotification::PaneOutput(id)`
  - `MuxNotification::Alert(id)` -> `MuxNotification::PaneBell(id)`
- [x] Update `mux_event/tests.rs`:
  - Update tests for renamed variants (`PaneDirty` -> `PaneOutput`, `Alert` -> `PaneBell`)
**Rename coordination:** `PaneDirty` is referenced in `server/mod.rs`,
`server/notify/mod.rs`, `backend/client/rpc_methods.rs`,
`backend/client/transport/reader.rs`, `backend/client/mod.rs` (doc),
and multiple test files. Rename all emit AND consume sites atomically
in 03.2 to maintain buildability. Do not defer consume-site renames
to section 05.
- [x] Update all consume sites of `PaneDirty` -> `PaneOutput`
- [x] Update all consume sites of `Alert` -> `PaneBell`
- [x] Update module doc: "Pane lifecycle notifications" (done in 03.6 purge)

---

## 03.3 Remove Session Types

**File(s):** `oriterm_mux/src/session/mod.rs`,
`oriterm_mux/src/session/tests.rs`

- [x] Delete `oriterm_mux/src/session/` entirely (includes `mod.rs` and `tests.rs`)
- [x] Remove `pub mod session;` from `lib.rs`
- [x] Remove `pub use session::{MuxTab, MuxWindow};` from `lib.rs`

---

## 03.4 Remove ID Types

**File(s):** `oriterm_mux/src/id/mod.rs`

- [x] Remove `TabId`, `WindowId`, `SessionId` from `id/mod.rs`
- [x] Remove their `MuxId` impls, `sealed::Sealed` impls, `Display` impls,
      `from_raw`/`raw` convenience impls
- [x] Remove `IdAllocator<TabId>`, `IdAllocator<WindowId>`,
      `IdAllocator<SessionId>` (generic impl stays, just fewer instantiations)
- [x] Update `lib.rs` re-export to `pub use id::{ClientId, DomainId, IdAllocator, MuxId, PaneId};`
- [x] Remove `sealed::Sealed` impls for `TabId`, `WindowId`, `SessionId`
- [x] Update `id/tests.rs`: removed tests for `TabId`, `WindowId`, `SessionId`
- [x] Keep: `PaneId`, `DomainId`, `ClientId`, `MuxId`, `IdAllocator`

---

## 03.5 Flatten PaneRegistry

**File(s):** `oriterm_mux/src/registry/mod.rs`

- [x] Remove `SessionRegistry` struct and its `impl` block from `registry/mod.rs`
- [x] Remove `PaneEntry.tab: Option<TabId>` field
- [x] `PaneEntry` is now: `{ pane: PaneId, domain: DomainId }`
- [x] Remove `panes_in_tab()` method from `PaneRegistry`
- [x] Update `lib.rs` re-export to `pub use registry::{PaneEntry, PaneRegistry};`
- [x] Remove session and ID imports from `registry/mod.rs`
- [x] Rewrite `registry/tests.rs`: pane-only tests, no tab/window/session

---

## 03.6 Purge UI Comments

**File(s):** All files in `oriterm_mux/src/`

- [x] Remove all references to: GUI, winit, tab bar, frontend, re-sync, mux-to-GUI
- [x] Rewrite `lib.rs` module doc (pane server, not multiplexer)
- [x] Rewrite `mux_event/mod.rs` — "event loop iteration" not "winit wakeup"
- [x] Rewrite `backend/mod.rs` — "client app" not "GUI app"
- [x] Update `server/connection.rs`, `server/clients.rs` — "client process" not "GUI process"
- [x] Update `layout/compute/mod.rs` — "pane layout" not "tab content (excludes tab bar)"
- [x] `grep -rn "GUI\|winit\|tab.bar\|frontend" oriterm_mux/src/` returns zero results

---

## 03.7 Completion Checklist

- [x] `grep -rn "TabId\|WindowId\|SessionId\|MuxTab\|MuxWindow\|SessionRegistry" oriterm_mux/src/`
      returns zero results
- [x] `grep -rn "GUI\|winit\|tab.bar\|frontend" oriterm_mux/src/`
      returns zero results
- [x] `InProcessMux` has only pane methods: `spawn_standalone_pane`, `close_pane`,
      `get_pane_entry`, `poll_events`, `drain_notifications`,
      `discard_notifications`, `pane_registry`, `event_tx`, `default_domain`
- [x] `MuxNotification` has only pane variants
- [x] `PaneEntry` has no `tab` field — `{ pane: PaneId, domain: DomainId }`
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria:** `oriterm_mux` is a flat pane server. Zero references to
tabs, windows, sessions, GUI, or any presentation concept. All builds and
tests green.
