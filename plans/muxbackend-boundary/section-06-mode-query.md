---
section: "06"
title: Pane Mode Query Through MuxBackend
status: complete
goal: Terminal mode queries go through MuxBackend — single code path, no branching
sections:
  - id: "06.1"
    title: Add pane_mode to MuxBackend
    status: complete
  - id: "06.2"
    title: Simplify App::pane_mode helper
    status: complete
  - id: "06.3"
    title: Completion Checklist
    status: complete
---

# Section 06: Pane Mode Query Through MuxBackend

**Status:** 📋 Planned
**Goal:** `App::pane_mode()` delegates to a single `MuxBackend::pane_mode()` call — no embedded/daemon branching.

**Key files:**
- `oriterm_mux/src/backend/mod.rs`
- `oriterm/src/app/mod.rs` — `pane_mode()` helper (lines 410–418)

---

## 06.1 Add `pane_mode` to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Add method:
  ```rust
  /// Terminal mode bits for a pane (raw u32).
  ///
  /// In embedded mode, reads the lock-free atomic cache.
  /// In daemon mode, reads from the cached snapshot.
  fn pane_mode(&self, pane_id: PaneId) -> Option<u32>;
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement: `self.panes.get(&pane_id).map(|p| p.mode())`
  - Uses the lock-free `mode_cache` atomic — no terminal lock needed

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Implement: `self.pane_snapshot(pane_id).map(|s| s.modes)`
  - No RPC needed — reads from local snapshot cache

---

## 06.2 Simplify `App::pane_mode` Helper

**File:** `oriterm/src/app/mod.rs` (lines 410–418)

- [ ] Replace the current branching implementation:
  ```rust
  // Before (branches on pane() vs snapshot):
  fn pane_mode(&self, pane_id: PaneId) -> Option<TermMode> {
      let mux = self.mux.as_ref()?;
      if let Some(pane) = mux.pane(pane_id) {
          Some(pane.terminal().lock().mode())
      } else {
          mux.pane_snapshot(pane_id)
              .map(|s| TermMode::from_bits_truncate(s.modes))
      }
  }

  // After (single delegation):
  fn pane_mode(&self, pane_id: PaneId) -> Option<TermMode> {
      let mux = self.mux.as_ref()?;
      mux.pane_mode(pane_id).map(TermMode::from_bits_truncate)
  }
  ```

---

## 06.3 Completion Checklist

- [ ] `MuxBackend::pane_mode()` implemented on both backends
- [ ] `App::pane_mode()` is a single-line delegation (no branching)
- [ ] Mouse reporting, bracketed paste, focus events, key encoding all work in daemon mode (they already do since we fixed this previously — verify no regression)
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `App::pane_mode` has no `if let Some(pane)` / `else` branching. One code path.
