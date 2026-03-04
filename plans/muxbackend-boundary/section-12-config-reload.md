---
section: "12"
title: Config Reload Cleanup
status: not-started
goal: Config reload has zero terminal().lock() calls — all changes go through MuxBackend
sections:
  - id: "12.1"
    title: Verify config_reload uses MuxBackend
    status: not-started
  - id: "12.2"
    title: Apply theme to all panes
    status: not-started
  - id: "12.3"
    title: Completion Checklist
    status: not-started
---

# Section 12: Config Reload Cleanup

**Status:** 📋 Planned
**Goal:** After Section 05 rewires the core calls, this section verifies completeness and handles the "apply to ALL panes" case (current code only updates the active pane).

**Crate:** `oriterm` (config_reload)
**Depends on:** Section 05 (theme/palette/cursor MuxBackend methods)
**Key files:**
- `oriterm/src/app/config_reload.rs`

---

## 12.1 Verify Config Reload Uses MuxBackend

After Section 05, `config_reload.rs` should have zero `terminal().lock()` calls. Verify this.

- [ ] `grep -rn 'terminal().lock\|\.terminal()\|active_pane()' oriterm/src/app/config_reload.rs` returns zero
- [ ] All color/cursor/behavior changes go through MuxBackend

---

## 12.2 Apply Theme to All Panes

Current code (`apply_color_changes`, `apply_behavior_changes`) only updates the **active pane**. When a user has multiple tabs/panes, only one gets the theme change. Fix this.

**File:** `oriterm/src/app/config_reload.rs`

- [ ] `apply_color_changes`: iterate all pane IDs and call `mux.set_pane_theme(pane_id, theme, palette)` for each
  ```rust
  let pane_ids: Vec<PaneId> = mux.pane_ids(); // or collect from session registry
  for &pane_id in &pane_ids {
      mux.set_pane_theme(pane_id, theme, palette.clone());
  }
  ```
  - Prefer session-registry enumeration (window → tab → pane IDs) instead of `pane_ids()` so this still works after Section 13 removes direct pane accessors from `MuxBackend`.
  - If keeping `pane_ids()` temporarily, ensure daemon mode returns real IDs first (currently returns empty).
- [ ] `apply_behavior_changes`: same — iterate all panes for `mark_all_dirty`
- [ ] `apply_cursor_changes`: same — iterate all panes for `set_cursor_shape`

---

## 12.3 Completion Checklist

- [ ] Zero `terminal().lock()` in config_reload.rs
- [ ] Theme changes apply to ALL panes (not just active)
- [ ] Config reload works in daemon mode
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** Config hot-reload works in daemon mode for all panes.
