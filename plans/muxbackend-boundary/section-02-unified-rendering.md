---
section: "02"
title: Unified Snapshot Rendering
status: not-started
goal: EmbeddedMux produces real snapshots; rendering uses one code path (snapshot-only)
sections:
  - id: "02.1"
    title: EmbeddedMux snapshot infrastructure
    status: not-started
  - id: "02.2"
    title: Remove default trait impls for snapshot methods
    status: not-started
  - id: "02.3"
    title: Unify single-pane rendering
    status: not-started
  - id: "02.4"
    title: Unify multi-pane rendering
    status: not-started
  - id: "02.5"
    title: Remove dead extract_frame functions
    status: not-started
  - id: "02.6"
    title: Completion Checklist
    status: not-started
---

# Section 02: Unified Snapshot Rendering

**Status:** 📋 Planned
**Goal:** Both `EmbeddedMux` and `MuxClient` produce `PaneSnapshot`s. The rendering pipeline uses a single snapshot-based code path — no `if daemon_mode` branching.

**Crate:** `oriterm_mux` (backend), `oriterm` (rendering)
**Key files:**
- `oriterm_mux/src/backend/embedded/mod.rs` — EmbeddedMux
- `oriterm_mux/src/backend/mod.rs` — MuxBackend trait
- `oriterm/src/app/redraw/mod.rs` — single-pane rendering
- `oriterm/src/app/redraw/multi_pane.rs` — multi-pane rendering
- `oriterm/src/gpu/extract/mod.rs` — `extract_frame`, `extract_frame_into` (non-snapshot variants to remove)
- `oriterm/src/gpu/extract/from_snapshot/mod.rs` — snapshot extraction path to keep

---

## 02.1 EmbeddedMux Snapshot Infrastructure

`EmbeddedMux` currently returns `None` from `pane_snapshot()`. Add real snapshot support.

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Add `snapshot_cache: HashMap<PaneId, PaneSnapshot>` field to `EmbeddedMux`
- [ ] Add `snapshot_dirty: HashSet<PaneId>` field to `EmbeddedMux`
- [ ] In `poll_events()`: when a pane's `grid_dirty` flag is set (or `MuxEvent::PaneOutput` fires), insert the pane_id into `snapshot_dirty`
- [ ] Mark snapshots dirty for **all local state mutations** that affect snapshots, not just PTY output:
  - resize/scroll/search/theme/cursor changes
  - title/icon/cwd updates from mux events
- [ ] Implement `pane_snapshot(&self, pane_id) -> Option<&PaneSnapshot>`: return from `snapshot_cache`
- [ ] Implement `is_pane_snapshot_dirty(&self, pane_id) -> bool`: check `snapshot_dirty.contains(&pane_id)`
- [ ] Implement `refresh_pane_snapshot(&mut self, pane_id) -> Option<&PaneSnapshot>`:
  - Get the `Pane` from the internal panes map
  - Build a `PaneSnapshot` using the same `build_snapshot` function the server uses
  - Insert into `snapshot_cache`
  - Remove from `snapshot_dirty`
  - Return reference
- [ ] Implement `clear_pane_snapshot_dirty(&mut self, pane_id)`: remove from `snapshot_dirty`
- [ ] Ensure `build_snapshot` is accessible from `EmbeddedMux` (may need to make it `pub(crate)` or move to a shared module)
- [ ] Update `MuxClient::poll_events` dirty tracking too: if snapshot-carried metadata changes (`PaneTitleChanged`, etc.), mark the pane dirty so cached snapshots are refreshed

**Performance note:** Building a snapshot locks the terminal briefly (same as `extract_frame` did). The snapshot is built once per dirty frame, same cost as before. The `HashMap` lookup is O(1).

**Dirty invariant:** The snapshot cache must be marked dirty on ALL mutations, not just PTY output. When implementing EmbeddedMux methods in later sections (resize, scroll, theme, search, etc.), each method must insert the pane_id into `snapshot_dirty` after performing the operation. This is the EmbeddedMux equivalent of the daemon's "next snapshot reflects new state" guarantee.

---

## 02.2 Remove Default Trait Impls for Snapshot Methods

Currently the `MuxBackend` trait has default implementations returning `None`/`false`/no-op for snapshot methods. Since both backends now implement them, remove the defaults to force implementation.

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Remove default body from `pane_snapshot()` — make it `fn pane_snapshot(&self, pane_id: PaneId) -> Option<&PaneSnapshot>;`
- [ ] Remove default body from `is_pane_snapshot_dirty()` — make it required
- [ ] Remove default body from `refresh_pane_snapshot()` — make it required
- [ ] Remove default body from `clear_pane_snapshot_dirty()` — make it required
- [ ] Verify both `EmbeddedMux` and `MuxClient` implement all four methods

---

## 02.3 Unify Single-Pane Rendering

Remove the `if daemon_mode { snapshot } else { terminal.lock() }` branch in `handle_redraw`.

**File:** `oriterm/src/app/redraw/mod.rs` (lines 85–119)

- [ ] Remove `let daemon_mode = mux.is_daemon_mode();`
- [ ] Remove the `if daemon_mode { ... } else { ... }` block
- [ ] Replace with unified snapshot path:
  ```rust
  if mux.pane_snapshot(pane_id).is_none() || mux.is_pane_snapshot_dirty(pane_id) {
      mux.refresh_pane_snapshot(pane_id);
  }
  let Some(snapshot) = mux.pane_snapshot(pane_id) else {
      log::warn!("redraw: no snapshot for pane {pane_id:?}");
      return;
  };
  match &mut ctx.frame {
      Some(existing) => extract_frame_from_snapshot_into(snapshot, existing, viewport, cell),
      slot @ None => *slot = Some(extract_frame_from_snapshot(snapshot, viewport, cell)),
  }
  mux.clear_pane_snapshot_dirty(pane_id);
  ```
- [ ] Remove the `!daemon_mode` guard on pane annotations (lines 137–153)
  - Selection/search/mark annotations will be updated in Phases 3–4 to use client-side state
  - For now, keep the annotation block but source data from client-side state (empty for now — marks the intermediate state)
- [ ] Remove `use crate::gpu::{extract_frame, extract_frame_into}` imports (now dead)

---

## 02.4 Unify Multi-Pane Rendering

Same changes as 02.3 but in the multi-pane path.

**File:** `oriterm/src/app/redraw/multi_pane.rs` (lines 148–257)

- [ ] Remove `let daemon_mode = mux.as_ref().is_some_and(|m| m.is_daemon_mode());`
- [ ] Remove the dirty check branching (lines 155–168): use `mux.is_pane_snapshot_dirty(pane_id)` uniformly
  - For the `grid_dirty` check in embedded mode: `is_pane_snapshot_dirty` now covers this (02.1 marks panes dirty when `grid_dirty` fires)
- [ ] Ensure cache-miss bootstrap in multi-pane too: if `pane_snapshot(pane_id).is_none()`, refresh even when `is_pane_snapshot_dirty` is false
- [ ] Remove the `if daemon_mode { ... } else { ... }` extraction block (lines 178–220): use snapshot path only
- [ ] Remove the `if !daemon_mode` guard on pane annotations (lines 237–257): will be sourced from client-side state (empty for now)
- [ ] Remove the `if !daemon_mode` guard on search bar restoration (lines 321–329)
- [ ] Remove `pane.clear_grid_dirty()` call (line 211) — dirty tracking now lives in snapshot infrastructure
- [ ] Remove dead imports: `extract_frame`, `extract_frame_into`, `oriterm_mux::Pane::grid_dirty`

---

## 02.5 Remove Dead `extract_frame` Functions

With both rendering paths using `extract_frame_from_snapshot*`, the non-snapshot variants are dead code.

**Files:** `oriterm/src/gpu/extract/mod.rs` and `oriterm/src/gpu/mod.rs`

- [ ] Remove `extract_frame(terminal: &FairMutex<Term<...>>, ...)` function
- [ ] Remove `extract_frame_into(terminal: &FairMutex<Term<...>>, ...)` function
- [ ] Remove their re-exports from `oriterm/src/gpu/mod.rs`
- [ ] Clean up any now-unused imports in the gpu module

---

## 02.6 Completion Checklist

- [ ] `EmbeddedMux::pane_snapshot()` returns `Some` for dirty panes after refresh
- [ ] First render works when no snapshot is cached yet (cache miss triggers refresh)
- [ ] Rendering uses one code path (no `daemon_mode` checks)
- [ ] `extract_frame` / `extract_frame_into` (non-snapshot) are removed
- [ ] No `is_daemon_mode()` calls in `redraw/mod.rs` or `redraw/multi_pane.rs`
- [ ] Embedded mode rendering is visually identical (test manually or screenshot comparison)
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** The GUI rendering pipeline has zero awareness of embedded vs daemon mode. It always uses snapshots. `EmbeddedMux` and `MuxClient` are interchangeable for rendering.
