---
section: "12"
title: Config Reload Cleanup
status: complete
goal: Config reload has zero terminal().lock() calls — all changes go through MuxBackend
sections:
  - id: "12.1"
    title: Verify config_reload uses MuxBackend
    status: complete
  - id: "12.2"
    title: Apply theme to all panes
    status: complete
  - id: "12.3"
    title: Completion Checklist
    status: complete
---

# Section 12: Config Reload Cleanup

**Status:** Complete
**Goal:** After Section 05 rewires the core calls, this section verifies completeness and handles the "apply to ALL panes" case.

**Crate:** `oriterm` (config_reload)
**Depends on:** Section 05 (theme/palette/cursor MuxBackend methods)
**Key files:**
- `oriterm/src/app/config_reload.rs`

---

## 12.1 Verify Config Reload Uses MuxBackend

- [x] `grep -rn 'terminal().lock\|\.terminal()\|active_pane()' oriterm/src/app/config_reload.rs` returns zero
- [x] All color/cursor/behavior changes go through MuxBackend
- [x] Was already clean after Section 05 — no changes needed

---

## 12.2 Apply Theme to All Panes

- [x] `apply_color_changes`: already iterates all pane IDs via `mux.pane_ids()` + `mux.set_pane_theme()`
- [x] `apply_cursor_changes`: changed from active-pane-only to iterating all panes via `mux.pane_ids()` + `mux.set_cursor_shape()`
- [x] `apply_behavior_changes`: changed from active-pane-only to iterating all panes via `mux.pane_ids()` + `mux.mark_all_dirty()`

---

## 12.3 Completion Checklist

- [x] Zero `terminal().lock()` in config_reload.rs
- [x] Theme changes apply to ALL panes (not just active)
- [x] Config reload works in daemon mode
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes
