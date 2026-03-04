---
section: "11"
title: URL Detection on Snapshot Data
status: not-started
goal: URL hover detection uses PaneSnapshot cells — no terminal lock
sections:
  - id: "11.1"
    title: Add hyperlink_uri to WireCell
    status: not-started
  - id: "11.2"
    title: Refactor cursor_hover.rs
    status: not-started
  - id: "11.3"
    title: Refactor fill_hovered_url_viewport_segments
    status: not-started
  - id: "11.4"
    title: Completion Checklist
    status: not-started
---

# Section 11: URL Detection on Snapshot Data

**Status:** 📋 Planned
**Goal:** URL hover detection (Ctrl+hover underline, Ctrl+click open) operates on `PaneSnapshot` data, not by locking the terminal.

**Crate:** `oriterm_mux` (snapshot enrichment), `oriterm` (cursor_hover)
**Key files:**
- `oriterm_mux/src/protocol/snapshot.rs` — WireCell
- `oriterm/src/app/cursor_hover.rs`

---

## 11.1 Add `hyperlink_uri` to WireCell

Currently `WireCell` has `has_hyperlink: bool` but not the URI. URL detection needs the URI for OSC 8 hyperlinks.

**File:** `oriterm_mux/src/protocol/snapshot.rs`

- [ ] Replace `has_hyperlink: bool` with `hyperlink_uri: Option<String>` on `WireCell`
  - Or keep both: `has_hyperlink` for fast checks, `hyperlink_uri` for the URI value
  - Prefer `hyperlink_uri: Option<String>` only — `has_hyperlink` is redundant with `is_some()`
- [ ] Update serialization — `None` serializes compactly
- [ ] Evaluate payload size impact: if URI strings are duplicated per-cell, consider deduping with a per-snapshot hyperlink table + cell hyperlink IDs

**File:** `oriterm_mux/src/server/snapshot.rs` (or wherever WireCell is populated)

- [ ] Populate `hyperlink_uri` from terminal grid cells while building the snapshot
  - `RenderableCell` currently exposes `has_hyperlink` but not the URI text, so `renderable_content()` alone is insufficient
  - Choose one implementation:
    - Extend `RenderableCell`/renderable extraction to carry `hyperlink_uri: Option<String>`, or
    - While building `PaneSnapshot`, walk the visible grid rows/cells in parallel and copy hyperlink URI from source cells

**File:** `oriterm/src/gpu/extract/from_snapshot/mod.rs`

- [ ] Update any code that reads `has_hyperlink` to use `hyperlink_uri.is_some()`

---

## 11.2 Refactor `cursor_hover.rs` — `detect_hover_url`

**File:** `oriterm/src/app/cursor_hover.rs` (lines 29–112)

Current code:
1. Gets `&Pane` via `mux.pane(pane_id)`
2. Locks terminal: `pane.terminal().lock()`
3. Reads `grid.scrollback().len()`, `grid.display_offset()` for absolute row computation
4. Gets `grid.absolute_row(abs_row)` → reads cell for hyperlink
5. Calls `UrlDetectCache::url_at(grid, abs_row, col)` for implicit URL detection

Refactored approach:
1. Get `&PaneSnapshot` via `mux.pane_snapshot(pane_id)`
2. Use snapshot metadata for viewport math:
   - `abs_row = snapshot.stable_row_base as usize + viewport_line`
3. Get cell from snapshot: `snapshot.cells[viewport_line][col]`
4. Check `hyperlink_uri` for OSC 8 URLs
5. For implicit URL detection: `url_detect_cache.url_at_snapshot(snapshot, viewport_line, col)`
   - `UrlDetectCache` needs a snapshot path that works on snapshot cell data instead of `Grid`
   - Or: inline the URL regex matching on snapshot row text
   - Preserve logical-line behavior (soft-wrap continuation), not just single-row regex, to avoid regressions on wrapped URLs

- [ ] Replace `mux.pane(pane_id)` with `mux.pane_snapshot(pane_id)`
- [ ] Replace `pane.terminal().lock()` / `grid` with snapshot field reads
- [ ] Replace `grid.absolute_row(abs_row)` cell lookup with `snapshot.cells[line][col]`
- [ ] Replace `row[Column(col)].hyperlink()` with `snapshot.cells[line][col].hyperlink_uri`
- [ ] Update or add a snapshot-compatible path in `UrlDetectCache`:
  - May need `url_at_from_snapshot_chars(row_chars, abs_row, col)` that takes character data
  - Extract chars from snapshot row: `snapshot.cells[line].iter().map(|c| c.ch).collect()`
- [ ] Define viewport-boundary behavior for implicit URLs that continue outside the snapshot:
  - Prefer best-effort detection limited to visible logical lines, and keep OSC 8 hyperlinks exact
  - Add tests for wrapped URLs split across multiple rows
- **Viewport boundary limitation:** Implicit URL detection (regex-based, not OSC 8) operates on snapshot viewport cells only. URLs spanning a viewport boundary (top/bottom row) may be truncated. This matches the current implementation's behavior (`UrlDetectCache` works on visible rows). This is an acceptable limitation — do not add extra continuity metadata to the snapshot for this edge case.

---

## 11.3 Refactor `fill_hovered_url_viewport_segments`

**File:** `oriterm/src/app/cursor_hover.rs` (lines 179–213)

- [ ] Replace `self.active_pane()` + `pane.terminal().lock().grid()` with snapshot:
  ```rust
  let mux = self.mux.as_ref()?;
  let snapshot = mux.pane_snapshot(pane_id)?;
  let sb_len = snapshot.scrollback_len as usize;
  let display_offset = snapshot.display_offset as usize;
  let lines = snapshot.cells.len();
  ```
- [ ] Rest of the logic (viewport conversion) stays the same

---

## 11.4 Completion Checklist

- [ ] `WireCell` has `hyperlink_uri: Option<String>` (replaces `has_hyperlink`)
- [ ] `detect_hover_url` uses snapshot data
- [ ] `fill_hovered_url_viewport_segments` uses snapshot data
- [ ] URL hover works in daemon mode (Ctrl+hover shows pointer, Ctrl+click opens)
- [ ] OSC 8 hyperlinks work in daemon mode
- [ ] Zero `pane.terminal().lock()` in `cursor_hover.rs`
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'terminal().lock\|active_pane()' oriterm/src/app/cursor_hover.rs` returns zero matches.
