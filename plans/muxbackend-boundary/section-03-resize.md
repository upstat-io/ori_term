---
section: "03"
title: Resize Through MuxBackend
status: not-started
goal: Pane resize goes through MuxBackend — fixes daemon mode stuck at 80×24
sections:
  - id: "03.1"
    title: Add resize_pane_grid to MuxBackend
    status: not-started
  - id: "03.2"
    title: Rewire GUI resize callsites
    status: not-started
  - id: "03.3"
    title: Completion Checklist
    status: not-started
---

# Section 03: Resize Through MuxBackend

**Status:** 📋 Planned
**Goal:** All pane resize operations go through `MuxBackend::resize_pane_grid()`. Daemon panes are no longer stuck at 80×24.

**Crate:** `oriterm_mux` (trait + impls), `oriterm` (callsites)
**Key files:**
- `oriterm_mux/src/backend/mod.rs` — trait
- `oriterm_mux/src/backend/embedded/mod.rs` — EmbeddedMux impl
- `oriterm_mux/src/backend/client/rpc_methods.rs` — MuxClient impl
- `oriterm/src/app/chrome/mod.rs` — `sync_grid_layout()` (line 412–415)
- `oriterm/src/app/pane_ops.rs` — `resize_all_panes()`, `resize_single_pane()`

**Note:** The `Resize` PDU (0x0107) already exists in the protocol and the server already handles it in `dispatch.rs`. MuxClient just needs to send it.
For multi-client correctness, resize handling should also trigger a pane-dirty notification broadcast so other subscribed clients refresh snapshots.

---

## 03.1 Add `resize_pane_grid` to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Add method to trait:
  ```rust
  /// Resize a pane's terminal grid and PTY.
  ///
  /// In embedded mode, calls `Pane::resize_grid` + `Pane::resize_pty`.
  /// In daemon mode, sends a fire-and-forget `Resize` PDU.
  fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16);
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement:
  ```rust
  fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16) {
      if let Some(pane) = self.panes.get(&pane_id) {
          pane.resize_grid(rows, cols);
          pane.resize_pty(rows, cols);
      }
      self.snapshot_dirty.insert(pane_id);
  }
  ```

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Implement using fire-and-forget (same pattern as `send_input`):
  ```rust
  fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16) {
      if let Some(transport) = &mut self.transport {
          transport.fire_and_forget(MuxPdu::Resize { pane_id, cols, rows });
      }
      self.dirty_panes.insert(pane_id);
  }
  ```
  - Verify `MuxPdu::Resize` variant exists and matches this signature
  - The server dispatch already handles Resize by calling `pane.resize_grid()` + `pane.resize_pty()`

---

## 03.2 Rewire GUI Resize Callsites

Replace all direct `pane.resize_grid()` / `pane.resize_pty()` calls with `mux.resize_pane_grid()`.

**File:** `oriterm/src/app/chrome/mod.rs` — `sync_grid_layout()` (lines 410–416)

- [ ] Replace:
  ```rust
  // Before:
  if let Some(pane) = self.active_pane_for_window(winit_id) {
      pane.resize_grid(rows as u16, cols as u16);
      pane.resize_pty(rows as u16, cols as u16);
  }

  // After:
  if let Some(pane_id) = self.active_pane_id_for_window(winit_id) {
      if let Some(mux) = self.mux.as_mut() {
          mux.resize_pane_grid(pane_id, rows as u16, cols as u16);
      }
  }
  ```
  - May need an `active_pane_id_for_window()` helper (returns `PaneId` not `&Pane`)
  - If this helper doesn't exist, add it using `session().get_window(win_id)?.active_tab()` → `session().get_tab(tab_id)?.active_pane()`

**File:** `oriterm/src/app/pane_ops.rs` — `resize_all_panes()` (lines 186–191)

- [ ] Replace:
  ```rust
  // Before:
  if let Some(pane) = mux.pane(layout.pane_id) {
      pane.resize_grid(layout.rows, layout.cols);
      pane.resize_pty(layout.rows, layout.cols);
  }

  // After:
  mux.resize_pane_grid(layout.pane_id, layout.rows, layout.cols);
  ```
  - Note: need `mux.as_mut()` since `resize_pane_grid` takes `&mut self`
  - The `compute_pane_layouts()` call borrows `&self`, so split the borrow: compute layouts first, then iterate and resize

**File:** `oriterm/src/app/pane_ops.rs` — `resize_single_pane()` (lines 211–214)

- [ ] Replace:
  ```rust
  // Before:
  if let Some(pane) = self.active_pane() {
      pane.resize_grid(rows, cols);
      pane.resize_pty(rows, cols);
  }

  // After:
  if let Some(pane_id) = self.active_pane_id() {
      if let Some(mux) = self.mux.as_mut() {
          mux.resize_pane_grid(pane_id, rows, cols);
      }
  }
  ```

---

## 03.3 Completion Checklist

- [ ] `MuxBackend::resize_pane_grid()` is implemented on both backends
- [ ] Zero calls to `pane.resize_grid()` or `pane.resize_pty()` from `oriterm/` (grep check)
- [ ] Window resize in daemon mode reaches the pane (terminal no longer 80×24)
- [ ] Window resize in embedded mode works identically
- [ ] Multi-pane split resize works
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'resize_grid\|resize_pty' oriterm/src/` returns zero matches. Daemon pane resizes when the window resizes.
