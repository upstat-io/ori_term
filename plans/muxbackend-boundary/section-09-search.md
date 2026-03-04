---
section: "09"
title: Search Through MuxBackend
status: not-started
goal: Search CRUD goes through MuxBackend — search state lives on the server (needs full scrollback)
sections:
  - id: "09.1"
    title: Add search methods to MuxBackend
    status: not-started
  - id: "09.2"
    title: Extend PaneSnapshot with search data
    status: not-started
  - id: "09.3"
    title: Add search PDUs to protocol
    status: not-started
  - id: "09.4"
    title: Server dispatch for search PDUs
    status: not-started
  - id: "09.5"
    title: Refactor search_ui.rs
    status: not-started
  - id: "09.6"
    title: Completion Checklist
    status: not-started
---

# Section 09: Search Through MuxBackend

**Status:** 📋 Planned
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

- [ ] Add methods:
  ```rust
  /// Open search for a pane (initializes empty SearchState).
  fn open_search(&mut self, pane_id: PaneId);

  /// Close search and clear search state.
  fn close_search(&mut self, pane_id: PaneId);

  /// Update the search query. Recomputes matches against the full grid.
  fn search_set_query(&mut self, pane_id: PaneId, query: String);

  /// Navigate to the next search match.
  fn search_next_match(&mut self, pane_id: PaneId);

  /// Navigate to the previous search match.
  fn search_prev_match(&mut self, pane_id: PaneId);

  /// Whether search is currently active for a pane.
  fn is_search_active(&self, pane_id: PaneId) -> bool;
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement by delegating to `Pane` methods:
  ```rust
  fn open_search(&mut self, pane_id: PaneId) {
      if let Some(pane) = self.panes.get_mut(&pane_id) { pane.open_search(); }
  }
  fn close_search(&mut self, pane_id: PaneId) {
      if let Some(pane) = self.panes.get_mut(&pane_id) { pane.close_search(); }
  }
  fn search_set_query(&mut self, pane_id: PaneId, query: String) {
      if let Some(pane) = self.panes.get_mut(&pane_id) {
          let grid_ref = pane.terminal().clone();
          if let Some(search) = pane.search_mut() {
              let term = grid_ref.lock();
              search.set_query(query, term.grid());
          }
      }
  }
  fn search_next_match(&mut self, pane_id: PaneId) {
      if let Some(pane) = self.panes.get_mut(&pane_id) {
          if let Some(search) = pane.search_mut() { search.next_match(); }
      }
  }
  // ... etc
  ```

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] `open_search` / `close_search` / `search_next_match` / `search_prev_match` → fire-and-forget PDUs
- [ ] `search_set_query` → fire-and-forget (search results appear in next snapshot)
- [ ] `is_search_active` → read from cached snapshot field (`snapshot.search_active`)
- [ ] Mark `self.dirty_panes.insert(pane_id)` after each fire-and-forget search command so redraw refreshes snapshots even if no PTY output event is emitted

---

## 09.2 Extend PaneSnapshot with Search Data

**File:** `oriterm_mux/src/protocol/snapshot.rs`

- [ ] Add search match wire type:
  ```rust
  /// A search match position on the wire.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct WireSearchMatch {
      /// Absolute start row of the match.
      pub start_row: u64,
      /// Start column.
      pub start_col: u16,
      /// Absolute end row of the match.
      pub end_row: u64,
      /// End column (inclusive).
      pub end_col: u16,
  }
  ```
- [ ] Add fields to `PaneSnapshot`:
  ```rust
  /// Whether search UI/state is active for this pane.
  pub search_active: bool,
  /// Search matches visible in the viewport.
  pub search_matches: Vec<WireSearchMatch>,
  /// Focused match span in stable coordinates (needed even when off-viewport).
  pub search_focused_match: Option<WireSearchMatch>,
  /// Index of the focused match (None if no matches).
  pub search_focused: Option<u32>,
  /// Current search query (may be empty while search is still active).
  pub search_query: String,
  /// Total match count across the full scrollback.
  pub search_total_matches: u32,
  ```

**File:** `oriterm_mux/src/server/snapshot.rs` (or wherever `build_snapshot` lives)

- [ ] Populate search fields from `Pane::search()`:
  - If search is active: set `search_active = true`, extract matches, query, and counts
  - If not active: set `search_active = false`, empty matches, empty query, `search_total_matches = 0`
  - Include `search_focused_match` so client-side `scroll_to_search_match()` can center an off-screen focused match without sending full match history

---

## 09.3 Add Search PDUs to Protocol

**File:** `oriterm_mux/src/protocol/messages.rs`

- [ ] Add PDU variants:
  ```rust
  OpenSearch { pane_id: PaneId },                    // fire-and-forget
  CloseSearch { pane_id: PaneId },                   // fire-and-forget
  SearchSetQuery { pane_id: PaneId, query: String }, // fire-and-forget
  SearchNextMatch { pane_id: PaneId },               // fire-and-forget
  SearchPrevMatch { pane_id: PaneId },               // fire-and-forget
  ```
- [ ] All fire-and-forget — the search state effect appears in the next snapshot
- [ ] Update `MsgType::from_u16` and `MuxPdu::msg_type()` mappings for the new search PDUs

---

## 09.4 Server Dispatch for Search PDUs

**File:** `oriterm_mux/src/server/dispatch.rs`

- [ ] Handle `OpenSearch`: `pane.open_search()`
- [ ] Handle `CloseSearch`: `pane.close_search()`
- [ ] Handle `SearchSetQuery`: lock terminal, then `if let Some(search) = pane.search_mut() { search.set_query(query, grid); }`
- [ ] Handle `SearchNextMatch`: `if let Some(search) = pane.search_mut() { search.next_match(); }`
- [ ] Handle `SearchPrevMatch`: `if let Some(search) = pane.search_mut() { search.prev_match(); }`
- [ ] After each: mark pane dirty so the next snapshot includes search state
- [ ] Emit/broadcast `NotifyPaneOutput { pane_id }` (or equivalent) after search mutations so subscribed clients refresh snapshots without polling

---

## 09.5 Refactor `search_ui.rs`

**File:** `oriterm/src/app/search_ui.rs`

- [ ] `open_search()` (line 14–21): Replace `pane.open_search()` with `mux.open_search(pane_id)`
- [ ] `close_search()` (line 24–31): Replace `pane.close_search()` with `mux.close_search(pane_id)`
- [ ] `is_search_active()` (line 34–36): Replace `pane.is_search_active()` with `mux.is_search_active(pane_id)`
- [ ] `handle_search_key()` (line 41–100):
  - Enter → `mux.search_next_match(pane_id)` or `mux.search_prev_match(pane_id)` (based on shift)
  - Backspace → pop char from local query string, `mux.search_set_query(pane_id, query)`
  - Character → append char to local query string, `mux.search_set_query(pane_id, query)`
  - Store the query string locally on App (for display in the search bar) since the PDU is fire-and-forget
  - Remove `pane.terminal().clone()` / `.lock().grid()` — no terminal access
- [ ] `scroll_to_search_match()` (line 103–143):
  - Replace with: check snapshot's `search_focused_match` (or derive from focused index when available) → compute scroll delta from snapshot metadata → `mux.scroll_display(pane_id, delta)`
  - All grid reads (`scrollback_len`, `display_offset`, `lines`) come from snapshot
  - Remove `pane.terminal().lock()` entirely
- [ ] Remove `use oriterm_mux::pane::Pane` import
- [ ] Search bar rendering (in `redraw/mod.rs`) already reads from `frame.search` — just need to populate it from the snapshot's search data instead of `pane.search()`

**File:** `oriterm/src/app/redraw/mod.rs` and `redraw/multi_pane.rs`

- [ ] Populate `frame.search` from snapshot search data:
  ```rust
  if snapshot.search_active {
      frame.search = Some(FrameSearch::from_snapshot_search(
          &snapshot.search_matches,
          snapshot.search_focused,
          &snapshot.search_query,
          snapshot.search_total_matches,
          frame.content.stable_row_base,
      ));
  }
  ```
  - May need a new `FrameSearch::from_snapshot_search()` constructor

---

## 09.6 Completion Checklist

- [ ] `MuxBackend` has 6 search methods, implemented on both backends
- [ ] `PaneSnapshot` carries search match data
- [ ] 5 search PDUs added, server handles them
- [ ] `search_ui.rs` has zero `pane.` calls
- [ ] Search works in daemon mode (open, type query, navigate matches, close)
- [ ] Search bar renders match count and highlights
- [ ] Search auto-scrolls to focused match
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** Search works in daemon mode. `grep -rn 'pane\.' oriterm/src/app/search_ui.rs` returns zero matches.
