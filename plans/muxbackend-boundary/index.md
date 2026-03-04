# MuxBackend Boundary Refactor — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Enrich PaneSnapshot
**File:** `section-01-enrich-snapshot.md` | **Status:** Complete

```
PaneSnapshot, snapshot, WireCell, WireCursor, WireRgb, WireCellFlags
stable_row_base, cols, scrollback_len, display_offset
snapshot.rs, build_snapshot, server/snapshot
wire format, serialization, serde, bincode
```

---

### Section 02: Unified Snapshot Rendering
**File:** `section-02-unified-rendering.md` | **Status:** Complete

```
EmbeddedMux, pane_snapshot, refresh_pane_snapshot, snapshot cache
extract_frame, extract_frame_into, extract_frame_from_snapshot
redraw, handle_redraw, multi_pane, rendering pipeline
daemon_mode, is_daemon_mode, branching, unified path
grid_dirty, clear_grid_dirty, is_pane_snapshot_dirty
```

---

### Section 03: Resize Through MuxBackend
**File:** `section-03-resize.md` | **Status:** Complete

```
resize, resize_pane_grid, resize_pty, resize_grid
sync_grid_layout, resize_all_panes, resize_single_pane
pane_ops, chrome/mod.rs, Resize PDU
80x24, daemon pane size, SIGWINCH
```

---

### Section 04: Scroll Through MuxBackend
**File:** `section-04-scroll.md` | **Status:** Complete

```
scroll, scroll_display, scroll_to_bottom, display_offset
scroll_to_previous_prompt, scroll_to_next_prompt, prompt navigation
ScrollDisplay PDU, ScrollToBottom PDU, ScrollToPrompt PDU
action_dispatch, auto-scroll, mouse drag scroll
```

---

### Section 05: Theme + Palette + Cursor Shape
**File:** `section-05-theme-palette.md` | **Status:** Complete

```
theme, palette, set_theme, palette_mut, set_cursor_shape
config_reload, apply_color_changes, apply_cursor_changes
bold_is_bright, mark_all_dirty, grid_mut, dirty_mut
SetTheme PDU, SetCursorShape PDU, MarkAllDirty PDU
apply_palette, handle_theme_changed, handle_dpi_changed
split_pane palette, new_tab palette, init palette
SelectScheme context menu, window_management reconnect
```

---

### Section 06: Pane Mode Query
**File:** `section-06-mode-query.md` | **Status:** Complete

```
pane_mode, TermMode, mode bits, mode_cache
terminal_mode, from_bits_truncate, modes u32
mouse reporting, bracketed paste, focus events
keyboard input, kitty protocol
```

---

### Section 07: Client-Side Selection (SnapshotGrid)
**File:** `section-07-selection.md` | **Status:** Complete

```
selection, Selection, SelectionPoint, SelectionMode
mouse_selection, handle_press, handle_drag, handle_release
SnapshotGrid, snapshot_grid, word_boundaries, redirect_spacer
pane_selections, client-side state, set_selection, clear_selection
StableRowIndex, stable_row_base, viewport math
```

---

### Section 08: Client-Side Mark Mode
**File:** `section-08-mark-mode.md` | **Status:** Complete

```
mark mode, MarkCursor, mark_cursor, enter_mark_mode, exit_mark_mode
handle_mark_mode_key, apply_motion, motion.rs
select_all, extend_or_create_selection, ensure_visible
mark_cursors, client-side state
EnterMarkMode action, SelectCommandOutput, SelectCommandInput
context menu SelectAll, is_mark_mode dispatch
```

---

### Section 09: Search Through MuxBackend
**File:** `section-09-search.md` | **Status:** Complete

```
search, SearchState, search_ui, open_search, close_search
search_set_query, search_next_match, search_prev_match
focused_match, scroll_to_search_match, search bar
WireSearchMatch, search_matches, search_focused, search_query
OpenSearch PDU, CloseSearch PDU, SearchSetQuery PDU
```

---

### Section 10: Clipboard Through MuxBackend
**File:** `section-10-clipboard.md` | **Status:** Complete

```
clipboard, extract_text, extract_html, clipboard_ops
copy, paste, smart_copy, selection text
ExtractText PDU, ExtractHtml PDU
```

---

### Section 11: URL Detection on Snapshot
**File:** `section-11-url-detection.md` | **Status:** Complete

```
URL, hover, cursor_hover, detect_hover_url, update_url_hover
fill_hovered_url_viewport_segments, UrlSegment
hyperlink, OSC 8, has_hyperlink, hyperlink_uri
url_detect, UrlDetectCache, implicit URL, regex
```

---

### Section 12: Config Reload Cleanup
**File:** `section-12-config-reload.md` | **Status:** Complete

```
config_reload, apply_config_reload, apply_color_changes
apply_cursor_changes, apply_behavior_changes
bold_is_bright, mark_all_dirty
set_pane_theme, set_cursor_shape
```

---

### Section 13: Remove Pane from oriterm
**File:** `section-13-remove-pane.md` | **Status:** Complete

```
Pane, pane(), pane_mut(), remove_pane(), pane_ids()
active_pane, active_pane_mut, active_pane_for_window
oriterm_mux::pane, import removal, type-level enforcement
MuxBackend trait cleanup, method removal
tab_management cwd, clear_bell, effective_title, icon_name
estimate_split_size, write_pane_input display_offset check
pane_cwd, cleanup_closed_pane, select_command_output/input
```

---

### Section 14: E2E MuxServer Integration Tests
**File:** `section-14-e2e-tests.md` | **Status:** Complete

```
integration test, e2e, MuxServer, MuxClient
test harness, spawn server, connect client
resize, scroll, search, extract_text, snapshot
daemon mode testing, IPC testing
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Enrich PaneSnapshot | `section-01-enrich-snapshot.md` |
| 02 | Unified Snapshot Rendering | `section-02-unified-rendering.md` |
| 03 | Resize Through MuxBackend | `section-03-resize.md` |
| 04 | Scroll Through MuxBackend | `section-04-scroll.md` |
| 05 | Theme + Palette + Cursor Shape | `section-05-theme-palette.md` |
| 06 | Pane Mode Query | `section-06-mode-query.md` |
| 07 | Client-Side Selection | `section-07-selection.md` |
| 08 | Client-Side Mark Mode | `section-08-mark-mode.md` |
| 09 | Search Through MuxBackend | `section-09-search.md` |
| 10 | Clipboard Through MuxBackend | `section-10-clipboard.md` |
| 11 | URL Detection on Snapshot | `section-11-url-detection.md` |
| 12 | Config Reload Cleanup | `section-12-config-reload.md` |
| 13 | Remove Pane from oriterm | `section-13-remove-pane.md` |
| 14 | E2E MuxServer Integration Tests | `section-14-e2e-tests.md` |
