---
section: "09"
title: Search Through MuxBackend
status: complete
goal: Search CRUD goes through MuxBackend — search state lives on the server (needs full scrollback)
sections:
  - id: "09.1"
    title: Add search methods to MuxBackend
    status: complete
  - id: "09.2"
    title: Extend PaneSnapshot with search data
    status: complete
  - id: "09.3"
    title: Add search PDUs to protocol
    status: complete
  - id: "09.4"
    title: Server dispatch for search PDUs
    status: complete
  - id: "09.5"
    title: Refactor search_ui.rs
    status: complete
  - id: "09.6"
    title: Completion Checklist
    status: complete
---

# Section 09: Search Through MuxBackend

**Status:** Complete
**Goal:** Search state lives on the server (it needs the full scrollback buffer for matching, not just the viewport snapshot). The GUI sends search commands through `MuxBackend`. Search match data is included in `PaneSnapshot` for rendering.

**Rationale:** Unlike selection (which operates on visible cells), search must scan the **entire scrollback** (potentially millions of lines). The client only has a viewport-sized snapshot. Therefore search must be server-side.

**Crate:** `oriterm_mux` (trait + impls + protocol + snapshot), `oriterm` (search_ui)
**Key files:**
- `oriterm_mux/src/backend/mod.rs`
- `oriterm_mux/src/protocol/snapshot.rs` — add search match data
- `oriterm/src/app/search_ui.rs` — refactor

---

## 09.1 Add Search Methods to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [x] Add methods:
  ```rust
  fn open_search(&mut self, pane_id: PaneId);
  fn close_search(&mut self, pane_id: PaneId);
  fn search_set_query(&mut self, pane_id: PaneId, query: String);
  fn search_next_match(&mut self, pane_id: PaneId);
  fn search_prev_match(&mut self, pane_id: PaneId);
  fn is_search_active(&self, pane_id: PaneId) -> bool;
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [x] Implement by delegating to `Pane` methods + marking `snapshot_dirty`

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [x] `open_search` / `close_search` / `search_next_match` / `search_prev_match` → fire-and-forget PDUs
- [x] `search_set_query` → fire-and-forget (search results appear in next snapshot)
- [x] `is_search_active` → read from cached snapshot field (`snapshot.search_active`)
- [x] Mark `self.dirty_panes.insert(pane_id)` after each fire-and-forget search command

---

## 09.2 Extend PaneSnapshot with Search Data

**File:** `oriterm_mux/src/protocol/snapshot.rs`

- [x] Add `WireSearchMatch` wire type with `start_row`, `start_col`, `end_row`, `end_col`
- [x] Add fields to `PaneSnapshot`: `search_active`, `search_query`, `search_matches`, `search_focused`, `search_total_matches`

**File:** `oriterm_mux/src/server/snapshot.rs`

- [x] Populate search fields from `Pane::search()` in `build_snapshot`

---

## 09.3 Add Search PDUs to Protocol

**File:** `oriterm_mux/src/protocol/messages.rs`

- [x] Add 5 fire-and-forget PDU variants: `OpenSearch`, `CloseSearch`, `SearchSetQuery`, `SearchNextMatch`, `SearchPrevMatch`
- [x] Add `MsgType` variants (0x011B–0x011F) with `from_u16` and `msg_type()` mappings
- [x] All marked fire-and-forget in `is_fire_and_forget()`

---

## 09.4 Server Dispatch for Search PDUs

**File:** `oriterm_mux/src/server/dispatch.rs`

- [x] Handle all 5 search PDUs (all fire-and-forget, return `None`)

---

## 09.5 Refactor `search_ui.rs`

**File:** `oriterm/src/app/search_ui.rs`

- [x] `open_search()`: uses `mux.open_search(pane_id)`
- [x] `close_search()`: uses `mux.close_search(pane_id)`
- [x] `is_search_active()`: uses `mux.is_search_active(pane_id)`
- [x] `handle_search_key()`: uses `mux.search_next_match/prev_match/set_query`, reads current query from snapshot
- [x] `scroll_to_search_match()`: reads focused match + viewport metadata from snapshot, calls `mux.scroll_display()`
- [x] Removed `use oriterm_mux::pane::Pane` import — zero direct Pane access

**File:** `oriterm/src/app/redraw/mod.rs` and `redraw/multi_pane.rs`

- [x] Populate `frame.search` from snapshot via `FrameSearch::from_snapshot()`
- [x] Removed `pane.search()` calls entirely
- [x] Removed unused `focused_base` variable from multi_pane.rs

**File:** `oriterm/src/gpu/frame_input/mod.rs`

- [x] Added `FrameSearch::from_snapshot()` constructor (converts `WireSearchMatch` → `SearchMatch`)
- [x] Removed dead `FrameSearch::new()` (no remaining callers after refactor)
- [x] Removed unused `SearchState` import

---

## 09.6 Completion Checklist

- [x] `MuxBackend` has 6 search methods, implemented on both backends
- [x] `PaneSnapshot` carries search match data
- [x] 5 search PDUs added, server handles them
- [x] `search_ui.rs` has zero `pane.` calls
- [x] Search bar renders match count and highlights (via `FrameSearch::from_snapshot`)
- [x] Search auto-scrolls to focused match (via snapshot metadata)
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes
