---
section: "05"
title: Theme + Palette + Cursor Shape Through MuxBackend
status: not-started
goal: Config reload applies theme/palette/cursor changes through MuxBackend
sections:
  - id: "05.1"
    title: Add theme/palette/cursor methods to MuxBackend
    status: not-started
  - id: "05.2"
    title: Add PDUs for theme changes
    status: not-started
  - id: "05.3"
    title: Server dispatch for theme PDUs
    status: not-started
  - id: "05.4"
    title: Rewire config_reload.rs
    status: not-started
  - id: "05.5"
    title: Rewire apply_palette callsites
    status: not-started
  - id: "05.6"
    title: Completion Checklist
    status: not-started
---

# Section 05: Theme + Palette + Cursor Shape Through MuxBackend

**Status:** 📋 Planned
**Goal:** Config hot-reload applies theme, palette, and cursor shape changes through `MuxBackend`, not by locking the terminal directly.

**Crate:** `oriterm_mux` (trait + impls + protocol), `oriterm` (config_reload)
**Key files:**
- `oriterm/src/app/config_reload.rs` — `apply_color_changes()`, `apply_cursor_changes()`, `apply_behavior_changes()`
- `oriterm_mux/src/backend/mod.rs`

---

## 05.1 Add Theme/Palette/Cursor Methods to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Add methods:
  ```rust
  /// Apply a theme and palette to a pane's terminal.
  fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: Palette);

  /// Change the cursor shape for a pane.
  fn set_cursor_shape(&mut self, pane_id: PaneId, shape: CursorShape);

  /// Mark all lines in a pane as dirty (forces full re-render).
  fn mark_all_dirty(&mut self, pane_id: PaneId);
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement by locking the terminal:
  ```rust
  fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: Palette) {
      if let Some(pane) = self.panes.get(&pane_id) {
          let mut term = pane.terminal().lock();
          term.set_theme(theme);
          *term.palette_mut() = palette;
          term.grid_mut().dirty_mut().mark_all();
      }
      self.snapshot_dirty.insert(pane_id);
  }
  fn set_cursor_shape(&mut self, pane_id: PaneId, shape: CursorShape) {
      if let Some(pane) = self.panes.get(&pane_id) {
          pane.terminal().lock().set_cursor_shape(shape);
      }
      self.snapshot_dirty.insert(pane_id);
  }
  fn mark_all_dirty(&mut self, pane_id: PaneId) {
      if let Some(pane) = self.panes.get(&pane_id) {
          pane.terminal().lock().grid_mut().dirty_mut().mark_all();
      }
      self.snapshot_dirty.insert(pane_id);
  }
  ```

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Implement as fire-and-forget PDUs (theme changes are applied server-side, reflected in next snapshot)
- [ ] Mark `self.dirty_panes.insert(pane_id)` after enqueueing each fire-and-forget command so redraw fetches the updated snapshot even without PTY output

---

## 05.2 Add PDUs for Theme Changes

**File:** `oriterm_mux/src/protocol/messages.rs`

- [ ] Add PDU variants:
  ```rust
  SetTheme { pane_id: PaneId, theme: String, palette_rgb: Vec<[u8; 3]> }, // fire-and-forget
  SetCursorShape { pane_id: PaneId, shape: u8 },                        // fire-and-forget
  MarkAllDirty { pane_id: PaneId },                                      // fire-and-forget
  ```
- [ ] Reuse existing protocol helpers (`theme_to_wire` / `wire_to_theme`) instead of introducing a new numeric mapping
- [ ] `shape: u8` maps to `WireCursorShape` discriminant
- [ ] `palette_rgb` is the 270 RGB triplets (same format as PaneSnapshot::palette)
- [ ] Update `MsgType::from_u16` and `MuxPdu::msg_type()` mappings for the new PDUs

---

## 05.3 Server Dispatch for Theme PDUs

**File:** `oriterm_mux/src/server/dispatch.rs`

- [ ] Handle `SetTheme`: lock terminal, set theme, replace palette, mark all dirty
- [ ] Handle `SetCursorShape`: lock terminal, set cursor shape
- [ ] Handle `MarkAllDirty`: lock terminal, mark all grid lines dirty
- [ ] After mutations, emit/broadcast `NotifyPaneOutput { pane_id }` (or equivalent) so subscribed clients refresh stale snapshots

---

## 05.4 Rewire `config_reload.rs`

**File:** `oriterm/src/app/config_reload.rs`

- [ ] `apply_color_changes()` (lines 161–185): Replace `pane.terminal().lock()` → `set_theme` → `palette_mut` → `mark_all_dirty` with:
  ```rust
  if let Some(pane_id) = self.active_pane_id() {
      let palette = build_palette_from_config(&new.colors, theme);
      if let Some(mux) = self.mux.as_mut() {
          mux.set_pane_theme(pane_id, theme, palette);
      }
  }
  ```
  - Note: should iterate ALL panes, not just active. Check if current code does this.

- [ ] `apply_cursor_changes()` (lines 188–193): Replace `pane.terminal().lock().set_cursor_shape(shape)` with:
  ```rust
  if let Some(pane_id) = self.active_pane_id() {
      if let Some(mux) = self.mux.as_mut() {
          mux.set_cursor_shape(pane_id, shape);
      }
  }
  ```

- [ ] `apply_behavior_changes()` (lines 233–239): Replace `pane.terminal().lock().grid_mut().dirty_mut().mark_all()` with:
  ```rust
  if let Some(pane_id) = self.active_pane_id() {
      if let Some(mux) = self.mux.as_mut() {
          mux.mark_all_dirty(pane_id);
      }
  }
  ```

---

## 05.5 Rewire `apply_palette` Callsites

The free function `apply_palette(config, pane, theme)` in `mod.rs:500–504` directly locks the terminal to set the palette. It's called from 5+ places when creating new panes. All must use `mux.set_pane_theme()` instead.

**File:** `oriterm/src/app/mod.rs`

- [ ] Remove `apply_palette()` free function (lines 500–504) — it takes `&Pane` and locks the terminal directly
- [ ] Replace `handle_theme_changed()` (lines 340–345): `pane.terminal().lock()` → `mux.set_pane_theme(...)`
  - Apply to **all panes** in all windows, not only the focused pane
- [ ] Replace `handle_dpi_changed()` mark_all_dirty (line 318–319): `pane.terminal().lock().grid_mut().dirty_mut().mark_all()` → `mux.mark_all_dirty(pane_id)`

**File:** `oriterm/src/app/pane_ops.rs`

- [ ] `split_pane()` (line 92): Replace `mux.pane(new_pane_id)` + `apply_palette(config, pane, theme)` with `mux.set_pane_theme(new_pane_id, theme, palette)`
- [ ] `toggle_floating_pane()` (line 330): Same pattern — replace with `mux.set_pane_theme(new_pane_id, theme, palette)`

**File:** `oriterm/src/app/tab_management/mod.rs`

- [ ] `new_tab_in_window()` (line 43): Replace `mux.pane(pane_id)` + `apply_palette` with `mux.set_pane_theme(pane_id, theme, palette)`

**File:** `oriterm/src/app/init/mod.rs`

- [ ] Initial pane setup (line 293): Replace `mux.pane(pane_id)` + `apply_palette` with `mux.set_pane_theme(pane_id, theme, palette)`

**File:** `oriterm/src/app/window_management.rs`

- [ ] Reconnect palette application (line 56): Replace `mux.pane(pane_id)` + palette application with `mux.set_pane_theme(pane_id, theme, palette)`

**File:** `oriterm/src/app/keyboard_input/mod.rs`

- [ ] Context menu `SelectScheme` (lines 398–402): Replace `pane.terminal().lock()` → `palette_mut` → `mark_all` with `mux.set_pane_theme(...)`
  - Apply selected scheme to **all panes**, since it updates app-level color config

---

## 05.6 Completion Checklist

- [ ] `MuxBackend` has `set_pane_theme`, `set_cursor_shape`, `mark_all_dirty`
- [ ] PDUs added, server handles them
- [ ] Zero `terminal().lock()` calls in `config_reload.rs`
- [ ] `apply_palette()` free function removed from `mod.rs`
- [ ] All palette application on new panes uses `mux.set_pane_theme()`
- [ ] `handle_theme_changed` and `handle_dpi_changed` use MuxBackend
- [ ] Context menu scheme change uses MuxBackend
- [ ] Config hot-reload works in daemon mode (theme/cursor changes reflected)
- [ ] Config hot-reload works in embedded mode identically
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'terminal().lock()' oriterm/src/app/config_reload.rs` returns zero matches. `grep -rn 'apply_palette' oriterm/src/app/` returns zero matches.
