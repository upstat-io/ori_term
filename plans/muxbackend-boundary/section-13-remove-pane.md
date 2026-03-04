---
section: "13"
title: Remove Pane from oriterm
status: complete
goal: The Pane type is never imported in oriterm — type-level enforcement of the boundary
sections:
  - id: "13.1"
    title: Remove active_pane helpers
    status: complete
  - id: "13.2"
    title: Migrate remaining pane_mut callsites
    status: complete
  - id: "13.3"
    title: Remove pane()/pane_mut() from MuxBackend trait
    status: complete
  - id: "13.4"
    title: Remove is_daemon_mode() usage
    status: complete
  - id: "13.5"
    title: Final audit
    status: complete
  - id: "13.6"
    title: Completion Checklist
    status: complete
---

# Section 13: Remove Pane from oriterm

**Status:** Complete
**Goal:** The `Pane` type does not appear anywhere in `oriterm/src/`. The `pane()`, `pane_mut()`, `remove_pane()` methods are removed from `MuxBackend`. `is_daemon_mode()` calls in GUI code are for process lifecycle only.

**Crate:** `oriterm` (cleanup), `oriterm_mux` (trait cleanup)
**Depends on:** ALL previous sections (01–12)

---

## 13.1 Remove `active_pane` Helpers

**File:** `oriterm/src/app/mod.rs`

- [x] Removed `active_pane(&self) -> Option<&Pane>` method
- [x] Removed `active_pane_mut(&mut self) -> Option<&mut Pane>` method
- [x] Removed `use oriterm_mux::pane::Pane` import
- [x] Removed `defer_pane_drop(pane: Pane)` function (logic moved to `EmbeddedMux::cleanup_closed_pane`)

---

## 13.2 Migrate Remaining Pane Callsites

### New MuxBackend Methods Added

- [x] `set_bell(pane_id)` — embedded: `pane.set_bell()`, client: no-op (default impl)
- [x] `clear_bell(pane_id)` — embedded: `pane.clear_bell()`, client: no-op (default impl)
- [x] `cleanup_closed_pane(pane_id)` — embedded: removes pane + background thread drop, client: no-op (default impl)
- [x] `select_command_output(pane_id) -> Option<Selection>` — embedded: `pane.command_output_selection()`, client: None (default impl)
- [x] `select_command_input(pane_id) -> Option<Selection>` — embedded: `pane.command_input_selection()`, client: None (default impl)
- [x] `pane_cwd(pane_id) -> Option<String>` — reads from cached snapshot `cwd` field (default impl)

### PaneSnapshot Enrichment

- [x] Added `icon_name: Option<String>` field to `PaneSnapshot`
- [x] Added `cwd: Option<String>` field to `PaneSnapshot`
- [x] `build_snapshot()` populates both from `pane.icon_name()` and `pane.cwd()`

### Pane Non-mutating Query Methods

- [x] Added `Pane::command_output_selection(&self) -> Option<Selection>` (non-mutating)
- [x] Added `Pane::command_input_selection(&self) -> Option<Selection>` (non-mutating)

### Callsite Migrations

- [x] `mod.rs:write_pane_input` — removed `pane.write_input()` path, uses `mux.send_input()` uniformly
- [x] `mux_pump/mod.rs:PaneClosed` — `remove_pane()` + `defer_pane_drop()` → `mux.cleanup_closed_pane()`
- [x] `mux_pump/mod.rs:Alert` — `pane_mut().set_bell()` → `mux.set_bell()`
- [x] `mux_pump/mod.rs:CommandComplete` — `pane().effective_title()` → `pane_snapshot().title`
- [x] `tab_management/mod.rs:new_tab_in_window` — `active_pane().cwd()` → `mux.pane_cwd()`
- [x] `tab_management/mod.rs:cycle_tab` — `active_pane_mut().clear_bell()` → `mux.clear_bell()`
- [x] `tab_management/mod.rs:switch_to_tab` — `active_pane_mut().clear_bell()` → `mux.clear_bell()`
- [x] `tab_management/mod.rs:build_tab_entries` — `mux.pane().effective_title()/icon_name()` → `pane_snapshot().title/icon_name`
- [x] `tab_management/mod.rs:move_tab_to_new_window_embedded` — `remove_pane()` + `defer_pane_drop()` → `cleanup_closed_pane()`
- [x] `pane_ops.rs:estimate_split_size` — `pane.terminal().lock().grid()` → `pane_snapshot().cells.len()/cols`
- [x] `keyboard_input/mod.rs:execute_scroll` — removed `active_pane().terminal().lock()` fallback, snapshot only
- [x] `keyboard_input/action_dispatch.rs:SelectCommandOutput/Input` — `active_pane_mut().select_command_output/input()` → `mux.select_command_output/input()` + `set_pane_selection()`
- [x] `window_management.rs:close_window` — `remove_pane()` + `defer_pane_drop()` → `cleanup_closed_pane()`
- [x] `window_management.rs:pump_close_notifications` — same pattern

---

## 13.3 Remove `pane()`/`pane_mut()` from MuxBackend Trait

**File:** `oriterm_mux/src/backend/mod.rs`

- [x] Removed `fn pane(&self, pane_id: PaneId) -> Option<&Pane>`
- [x] Removed `fn pane_mut(&mut self, pane_id: PaneId) -> Option<&mut Pane>`
- [x] Removed `fn remove_pane(&mut self, pane_id: PaneId) -> Option<Pane>`
- [x] Removed `use crate::pane::Pane` from trait module
- [x] Kept `fn pane_ids(&self) -> Vec<PaneId>` — used by config reload

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [x] Removed `pane()`, `pane_mut()`, `remove_pane()` implementations

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [x] Removed stub implementations
- [x] Removed `use crate::pane::Pane` import

---

## 13.4 Remove `is_daemon_mode()` Usage

- [x] Audited all `is_daemon_mode()` calls in `oriterm/src/`:
  - `mux_pump/mod.rs:27` — daemon disconnect detection ✓
  - `mux_pump/mod.rs:112` — `WindowTabsChanged` refresh ✓
  - `init/mod.rs:53` — daemon mode init ✓
  - `tab_management/mod.rs:276` — move-tab routing ✓
  - `window_management.rs:126` — claim window ✓
  - `tab_drag/tear_off.rs:30` — tear-off routing ✓
- [x] All uses are for process lifecycle (window creation, daemon communication), not pane access
- [x] `is_daemon_mode()` stays on the trait — legitimate for process lifecycle decisions

---

## 13.5 Final Audit

- [x] `grep -rn 'oriterm_mux::pane' oriterm/src/` — one doc comment reference, zero imports
- [x] `grep -rn '.terminal()' oriterm/src/` — one doc comment reference, zero calls
- [x] `grep -rn '.lock()' oriterm/src/app/` — one doc comment reference, zero terminal locks
- [x] `grep -rn 'Grid' oriterm/src/app/` — one doc comment reference, zero `oriterm_core::Grid` imports
- [x] `grep -rn 'use oriterm_mux::pane::Pane' oriterm/src/` — zero matches

---

## 13.6 Completion Checklist

- [x] `Pane` type not imported in `oriterm/src/` (grep verification)
- [x] `terminal()` not called from `oriterm/src/` (grep verification)
- [x] `pane()` / `pane_mut()` / `remove_pane()` removed from `MuxBackend` trait
- [x] All remaining `is_daemon_mode()` calls in `oriterm/` are for process lifecycle, not pane access
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria:** The `Pane` type cannot be accessed from `oriterm`. If it compiles, the boundary is enforced. ✅
