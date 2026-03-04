# MuxBackend as Sole API Boundary — Overview

## Mandate

Make `MuxBackend` the **single abstraction boundary** between the GUI (`oriterm`) and terminal state (`oriterm_mux`/`oriterm_core`). The GUI must be a "dummy UI" — it receives snapshots to paint and sends commands through `MuxBackend`. It never touches `Pane`, `Terminal`, `Grid`, or `TermMode` directly.

Today the GUI has **70+ direct accesses** to `Pane` internals via `pane()`, `pane_mut()`, `terminal().lock().grid()`. In daemon mode, `mux.pane()` returns `None` (panes live in the daemon), causing selection, search, mark mode, scroll, URL detection, resize, and config reload to silently fail. The embedded and daemon code paths diverge at every pane interaction point.

## Design Principles

1. **One code path** — Embedded and daemon mode use identical GUI logic. No `if daemon_mode` branching.
2. **Snapshot-driven rendering** — The GUI always renders from `PaneSnapshot`, never from `Arc<FairMutex<Terminal>>`.
3. **Client-side visual state** — Selection, mark cursor operate on snapshot data locally (no server round-trip).
4. **Server-side persistent state** — Scroll position, search, theme/palette, resize go through `MuxBackend` (they modify the actual terminal).
5. **Type-level enforcement** — `Pane` type is never imported in `oriterm`. If it compiles, it's correct.

## Migration Safety

- Protocol additions must be versioned/capability-gated (`Hello` handshake feature bits or explicit protocol version) so mismatched `MuxClient`/`MuxServer` binaries fail fast with a clear error.
- During rollout, add new PDUs/fields in a backward-compatible order: server accepts both old/new where needed, then client switches usage, then old paths are removed.

## Architecture: Before and After

### Before (Current)

```
GUI (oriterm)
  │
  ├─ mux.pane(id) ──► &Pane ──► .terminal().lock().grid() ──► read cells, scrollback, etc.
  ├─ pane.set_selection(sel)     direct mutation
  ├─ pane.scroll_display(delta)  direct mutation
  ├─ pane.resize_grid(r, c)     direct mutation
  ├─ pane.search_mut()           direct mutation
  └─ pane.write_input(bytes)     direct PTY write
```

### After (Target)

```
GUI (oriterm)
  │
  ├─ mux.pane_snapshot(id)   ──► &PaneSnapshot  (read-only rendering data)
  ├─ mux.pane_mode(id)       ──► u32            (terminal mode bits)
  ├─ mux.send_input(id, data)                    (fire-and-forget)
  ├─ mux.resize_pane_grid(id, r, c)              (fire-and-forget)
  ├─ mux.scroll_display(id, delta)                (fire-and-forget)
  ├─ mux.search_set_query(id, q)                  (fire-and-forget)
  ├─ mux.extract_text(id, sel)                    (RPC)
  └─ self.pane_selections[id] = sel               (client-side state)
```

## State Ownership

| State | Owner | Rationale |
|-------|-------|-----------|
| Selection | GUI (App) | Visual overlay on snapshot cells. No server state. |
| Mark cursor | GUI (App) | Visual cursor navigating snapshot. No server state. |
| Search | Server (Pane) | Needs full scrollback (millions of lines), not just viewport snapshot. |
| Scroll position | Server (Terminal) | `display_offset` determines what the next snapshot contains. |
| Theme / palette | Server (Terminal) | Affects cell color resolution in snapshot building. |
| Cursor shape | Server (Terminal) | Affects snapshot's `WireCursor::shape`. |
| Resize | Server (Terminal + PTY) | Grid reflow + SIGWINCH to shell. |

## Phases

| Phase | Title | Sections | Goal |
|-------|-------|----------|------|
| 1 | Snapshot Unification | 01–02 | EmbeddedMux produces snapshots; rendering uses one code path |
| 2 | Server-Side Operations | 03–06 | Resize, scroll, theme, mode queries go through MuxBackend |
| 3 | Client-Side Selection | 07–08 | Selection + mark mode operate on snapshot data, owned by App |
| 4 | Server-Side Search + Clipboard | 09–10 | Search CRUD via MuxBackend; clipboard text extraction via MuxBackend |
| 5 | URL Detection + Config Reload | 11–12 | Hover detection on snapshot; config changes through MuxBackend |
| 6 | Final Cleanup + E2E Tests | 13–14 | Remove Pane from oriterm; MuxServer integration tests |

## Dependency Graph

```
Phase 1: Snapshot Unification (01, 02)
    │
    ├──► Phase 2: Server-Side Operations (03, 04, 05, 06)
    │       │
    │       ├──► Phase 3: Client-Side Selection (07, 08)   [needs 04 for scroll]
    │       │       │
    │       │       └──► Phase 4: Server-Side Search (09, 10)  [needs 07 for SnapshotGrid]
    │       │
    │       └──► Phase 5: URL + Config (11, 12)
    │
    └──────────► Phase 6: Cleanup + Tests (13, 14)  [depends on ALL above]
```

## Key Types

### PaneSnapshot (enriched)

```rust
pub struct PaneSnapshot {
    pub cells: Vec<Vec<WireCell>>,
    pub cursor: WireCursor,
    pub palette: Vec<[u8; 3]>,
    pub title: String,
    pub modes: u32,
    pub scrollback_len: u32,
    pub display_offset: u32,
    // --- New fields ---
    pub stable_row_base: u64,          // stable row identity for viewport line 0 (eviction-aware)
    pub cols: u16,                     // grid columns (avoids cells[0].len() fragility)
    pub search_active: bool,           // search UI visibility/state (Section 09)
    pub search_matches: Vec<WireSearchMatch>,  // match positions (Section 09)
    pub search_focused: Option<u32>,   // focused match index (Section 09)
    pub search_query: String,          // current query (Section 09)
    pub search_total_matches: u32,     // full-grid match count (Section 09)
}
```

### SnapshotGrid (Section 07)

Thin adapter wrapping `&PaneSnapshot` that provides grid queries for selection/mark mode:

```rust
pub struct SnapshotGrid<'a> {
    snapshot: &'a PaneSnapshot,
}

impl SnapshotGrid<'_> {
    fn cols(&self) -> usize;
    fn lines(&self) -> usize;
    fn scrollback_len(&self) -> usize;
    fn display_offset(&self) -> usize;
    fn stable_row_base(&self) -> u64;
    fn cell_char(&self, viewport_row: usize, col: usize) -> char;
    fn word_boundaries(&self, viewport_row: usize, col: usize, delimiters: &str) -> (usize, usize);
}
```

### New MuxBackend Methods (cumulative across sections)

```rust
// Section 03: Resize
fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16);

// Section 04: Scroll
fn scroll_display(&mut self, pane_id: PaneId, delta: isize);
fn scroll_to_bottom(&mut self, pane_id: PaneId);
fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool;
fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool;

// Section 05: Theme / palette / cursor
fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: Palette);
fn set_cursor_shape(&mut self, pane_id: PaneId, shape: CursorShape);
fn mark_all_dirty(&mut self, pane_id: PaneId);

// Section 06: Mode
fn pane_mode(&self, pane_id: PaneId) -> Option<u32>;

// Section 09: Search
fn open_search(&mut self, pane_id: PaneId);
fn close_search(&mut self, pane_id: PaneId);
fn search_set_query(&mut self, pane_id: PaneId, query: String);
fn search_next_match(&mut self, pane_id: PaneId);
fn search_prev_match(&mut self, pane_id: PaneId);
fn is_search_active(&self, pane_id: PaneId) -> bool;

// Section 10: Clipboard
fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String>;
fn extract_html(&mut self, pane_id: PaneId, selection: &Selection, ...) -> Option<(String, String)>;
```

## New PDU Variants (Section 03–06, 09–10)

| PDU | Direction | Type |
|-----|-----------|------|
| `ScrollDisplay { pane_id, delta }` | Request | Fire-and-forget |
| `ScrollToBottom { pane_id }` | Request | Fire-and-forget |
| `ScrollToPrompt { pane_id, direction }` | Request | RPC → `ScrollToPromptAck { scrolled: bool }` |
| `SetTheme { pane_id, palette_rgb }` | Request | Fire-and-forget |
| `SetCursorShape { pane_id, shape }` | Request | Fire-and-forget |
| `MarkAllDirty { pane_id }` | Request | Fire-and-forget |
| `OpenSearch { pane_id }` | Request | Fire-and-forget |
| `CloseSearch { pane_id }` | Request | Fire-and-forget |
| `SearchSetQuery { pane_id, query }` | Request | Fire-and-forget |
| `SearchNextMatch { pane_id }` | Request | Fire-and-forget |
| `SearchPrevMatch { pane_id }` | Request | Fire-and-forget |
| `ExtractText { pane_id, selection }` | Request | RPC → `ExtractTextResp { text }` |
| `ExtractHtml { pane_id, selection, ... }` | Request | RPC → `ExtractHtmlResp { html, text }` |

## What Stays Unchanged

- GPU rendering pipeline (`gpu/renderer.rs`, shaders, atlas) — stays in frontend.
- Font rasterization, glyph caching — stays in frontend.
- Window chrome, tab bar, overlays — stays in frontend.
- `TerminalGridWidget` layout — stays in frontend.
- `MuxBackend` session queries (`session()`, `active_tab_id()`, etc.) — already abstract.
- Tab/window/pane CRUD methods — already implemented on both backends.
- `EmbeddedMux` for tests and `--embedded` flag — stays as escape hatch.

## Cross-Cutting Concerns

### `stable_row_base` Semantics (Section 01)

`stable_row_base` must align with `StableRowIndex` semantics: it represents the total number of evicted + visible rows before the viewport base — specifically `total_rows_ever_produced - display_offset - visible_lines`. The naive `scrollback_len - display_offset` formula is only correct when `scrollback_len` equals the actual number of scrollback rows (not capped by max scrollback). Verify the computation matches `StableRowIndex::from_absolute()` exactly, accounting for scrollback eviction.

### IPC Protocol Compatibility

Adding new PDU variants and PaneSnapshot fields will break mixed-version client/daemon pairs. Strategy:

1. **Protocol version handshake**: The `Hello` PDU already carries a `pid`. Extend it (or add a `Capabilities` exchange) with a protocol version number. Increment it when adding new PDUs/fields.
2. **Forward compatibility**: Unknown PDU variants should be logged and ignored (not crash). PaneSnapshot deserialization must tolerate missing fields (use `#[serde(default)]` on new fields).
3. **Rollout**: Client and daemon ship from the same build, so version skew is a self-update problem. The version check triggers a "daemon restart required" notification rather than silent failure.

### Embedded Snapshot Cache Dirtying

The EmbeddedMux snapshot cache (Section 02.1) must mark panes dirty on ALL mutations, not just PTY output. Specifically:
- `resize_pane_grid()` — grid dimensions changed
- `scroll_display()` / `scroll_to_bottom()` / `scroll_to_*_prompt()` — viewport shifted
- `set_pane_theme()` — palette/colors changed
- `set_cursor_shape()` — cursor appearance changed
- `mark_all_dirty()` — explicit dirty
- `open_search()` / `close_search()` / `search_set_query()` / `search_next_match()` / `search_prev_match()` — search state changed

Each EmbeddedMux method implementation must insert into `snapshot_dirty` after performing the operation.

### URL Detection Viewport Limitation (Section 11)

Implicit URL detection (regex-based, not OSC 8) operates on snapshot viewport cells only. URLs spanning a viewport boundary (first row or last row) may be truncated. This is an acceptable limitation — the same issue exists with the current implementation since `UrlDetectCache` works on visible rows. Document this explicitly: "Implicit URLs are detected within the viewport only; a URL starting above the viewport and ending within it will match only the visible portion."

### RPC Error Handling

New RPC calls (`ScrollToPrompt`, `ExtractText`, `ExtractHtml`) need defined timeout and error behavior:
- **Timeout**: Reuse the existing RPC timeout (from `ClientTransport::rpc`). If a response doesn't arrive within the timeout, return `None`/`false`.
- **`ScrollToPrompt`**: On timeout/error, return `false` (no prompt found). The GUI treats this identically to "no prompt markers in scrollback."
- **`ExtractText` / `ExtractHtml`**: On timeout/error, return `None`. The copy operation silently fails (no clipboard change). Log at `warn` level.
- **Fire-and-forget PDUs** (scroll, theme, resize, search): No timeout — failures are reflected in the next snapshot (stale state). If the transport is disconnected, the GUI detects it via the notification channel and handles reconnection.

### Rollout / Migration Sequence

Each section should produce a compilable intermediate state. Strategy:
1. **Add before remove**: New MuxBackend methods and PDUs are added with default implementations (returning no-op/None) first. Both backends implement them. This compiles without touching any callsite.
2. **Migrate callsites**: Switch oriterm code from `pane.*()` to `mux.*()` one file at a time. Each file change compiles independently.
3. **Remove old path**: Only after all callsites are migrated, remove the default implementations and make the trait methods required (Section 02.2 pattern). Then remove `pane()`/`pane_mut()` from the trait (Section 13.3).
4. **Staged commits**: Each subsection (e.g., 04.1, 04.2, 04.3, 04.4) is a separate commit. Revert granularity matches the plan granularity.

## Verification

After each section:
1. `./build-all.sh` — cross-compiles for Windows (exercises `#[cfg(windows)]` paths)
2. `./clippy-all.sh` — catches dead code, unused imports, missing `#[cfg]`
3. `./test-all.sh` — runs all tests

After Phase 6:
4. `grep -rn 'oriterm_mux::pane::Pane' oriterm/src/` returns zero matches
5. `grep -rn '\.terminal()' oriterm/src/` returns zero matches
6. `is_daemon_mode()` is not used for pane/grid access logic in `oriterm/src/` (lifecycle-only uses are acceptable)
7. E2E test: spin up MuxServer → connect MuxClient → exercise full API
8. Manual: `oriterm-mux --foreground` + `oriterm` on Windows — keyboard input, resize, scroll, selection all work
