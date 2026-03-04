---
section: "11"
title: URL Detection on Snapshot Data
status: complete
goal: URL hover detection uses PaneSnapshot cells — no terminal lock
sections:
  - id: "11.1"
    title: Add hyperlink_uri to WireCell
    status: complete
  - id: "11.2"
    title: Refactor cursor_hover.rs
    status: complete
  - id: "11.3"
    title: Refactor fill_hovered_url_viewport_segments
    status: complete
  - id: "11.4"
    title: Completion Checklist
    status: complete
---

# Section 11: URL Detection on Snapshot Data

**Status:** Complete
**Goal:** URL hover detection (Ctrl+hover underline, Ctrl+click open) operates on `PaneSnapshot` data, not by locking the terminal.

**Crate:** `oriterm_mux` (snapshot enrichment), `oriterm` (cursor_hover, url_detect)
**Key files:**
- `oriterm_mux/src/protocol/snapshot.rs` — WireCell
- `oriterm_mux/src/server/snapshot.rs` — hyperlink URI extraction
- `oriterm/src/app/cursor_hover.rs`
- `oriterm/src/url_detect/mod.rs`

---

## 11.1 Add `hyperlink_uri` to WireCell

- [x] Replaced `has_hyperlink: bool` with `hyperlink_uri: Option<String>` on `WireCell`
- [x] Updated `server/snapshot.rs`: added `hyperlink_uri_at()` helper that looks up the hyperlink URI from grid cells during snapshot building (only when `RenderableCell::has_hyperlink` is true)
- [x] Updated `gpu/extract/from_snapshot/mod.rs`: maps `wire.hyperlink_uri.is_some()` → `RenderableCell::has_hyperlink`
- [x] `RenderableCell` unchanged — keeps `has_hyperlink: bool` for the GPU rendering path (only needs boolean for underline drawing)
- [x] Updated all test files constructing `WireCell`

---

## 11.2 Refactor `cursor_hover.rs` — `detect_hover_url`

- [x] Replaced `mux.pane(pane_id)` with `mux.pane_snapshot(pane_id)`
- [x] Replaced `pane.terminal().lock()` / grid access with snapshot cell lookup
- [x] OSC 8 hyperlinks read from `wire_cell.hyperlink_uri` instead of `row[Column(col)].hyperlink()`
- [x] Implicit URL detection via `url_cache.url_at_snapshot(snapshot, line, col)` operating on snapshot cells
- [x] Added snapshot-compatible URL detection to `UrlDetectCache`:
  - `url_at_snapshot()` method
  - `ensure_snapshot_logical_line()` method
  - `snapshot_row_continues()` — WrapOrFilled heuristic on `WireCellFlags`
  - `snapshot_logical_line_start/end()` — viewport-bounded logical line walking
  - `extract_snapshot_row_text()` — mirrors `extract_row_text` for `WireCell` data
  - `detect_urls_in_snapshot_lines()` — regex detection on snapshot text
- [x] Viewport boundary limitation: implicit URLs truncated at viewport edges (acceptable)
- [x] Grid-based functions moved behind `#[cfg(test)]` — used only by existing unit tests

---

## 11.3 Refactor `fill_hovered_url_viewport_segments`

- [x] Replaced `self.active_pane()` + `pane.terminal().lock().grid()` with snapshot:
  ```rust
  let snapshot = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id))?;
  let base = snapshot.stable_row_base as usize;
  let lines = snapshot.cells.len();
  ```
- [x] Absolute-to-viewport conversion uses `stable_row_base` instead of `scrollback().len() - display_offset()`

---

## 11.4 Completion Checklist

- [x] `WireCell` has `hyperlink_uri: Option<String>` (replaces `has_hyperlink`)
- [x] `detect_hover_url` uses snapshot data
- [x] `fill_hovered_url_viewport_segments` uses snapshot data
- [x] URL hover works in daemon mode (Ctrl+hover shows pointer, Ctrl+click opens)
- [x] OSC 8 hyperlinks work in daemon mode
- [x] Zero `pane.terminal().lock()` in `cursor_hover.rs`
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'terminal().lock\|active_pane()' oriterm/src/app/cursor_hover.rs` returns zero matches (only doc comment).
