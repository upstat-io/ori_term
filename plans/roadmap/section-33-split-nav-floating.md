---
section: 33
title: Split Navigation + Floating Panes
status: complete
tier: 4M
goal: Spatial navigation keybinds, divider drag resize, zoom/unzoom, floating pane creation and management, scissored rendering, float-tile toggle, undo/redo split operations
sections:
  - id: "33.1"
    title: Spatial Navigation Keybinds
    status: complete
  - id: "33.2"
    title: Divider Drag Resize
    status: complete
  - id: "33.3"
    title: Zoom + Unzoom
    status: complete
  - id: "33.4"
    title: Floating Pane Management
    status: complete
  - id: "33.5"
    title: Undo + Redo Split Operations
    status: complete
  - id: "33.6"
    title: Section Completion
    status: complete
---

# Section 33: Split Navigation + Floating Panes

**Status:** In Progress
**Goal:** Full split pane interaction: keyboard and mouse navigation, divider resize, zoom/unzoom, floating pane creation/drag/resize with scissored rendering, float↔tile toggle, and undo/redo for split operations.

**Crate:** `oriterm` (input handling, rendering), `oriterm_mux` (tree mutations, undo stack)
**Dependencies:** Section 31 (multi-pane rendering working)
**Prerequisite:** Section 31 complete.

**Absorbs:** Section 26.2 (Split Creation & Navigation), 26.3 (Split Rendering, partially — divider interaction), 26.4 (Split Resize).

**Inspired by:**
- Ghostty: directional navigation, immutable tree undo
- Zellij: floating pane mode, float↔tile toggle, floating pane drag/resize
- WezTerm: zoom/unzoom pane, pane selection mode
- tmux: the baseline for every split navigation interaction

---

## 33.1 Spatial Navigation Keybinds

Keyboard shortcuts for split creation, directional navigation, sequential cycling, and pane close. Mouse click-to-focus is also included here.

**Files:** `oriterm/src/keybindings/mod.rs`, `oriterm/src/keybindings/defaults.rs`, `oriterm/src/keybindings/parse.rs`, `oriterm/src/app/pane_ops.rs`, `oriterm/src/app/keyboard_input/mod.rs`, `oriterm/src/mux/mod.rs`, `oriterm/src/app/mux_pump.rs`, `oriterm/src/app/chrome/mod.rs`

**Default keybindings** (Ghostty-style):

| Action | Key | Ghostty equivalent |
|--------|-----|--------------------|
| `SplitRight` | `Ctrl+Shift+O` | `new_split:right` |
| `SplitDown` | `Ctrl+Shift+E` | `new_split:down` |
| `FocusPaneUp` | `Ctrl+Alt+Up` | `goto_split:top` |
| `FocusPaneDown` | `Ctrl+Alt+Down` | `goto_split:bottom` |
| `FocusPaneLeft` | `Ctrl+Alt+Left` | `goto_split:left` |
| `FocusPaneRight` | `Ctrl+Alt+Right` | `goto_split:right` |
| `PrevPane` | `Ctrl+Shift+K` | `goto_split:previous` |
| `NextPane` | `Ctrl+Shift+J` | `goto_split:next` |
| `ClosePane` | `Ctrl+Shift+W` | `close_surface` |

Ghostty uses `Ctrl+Super+[/]` for cycle on Linux — we use `Ctrl+Shift+J/K` (vi-style) since `Ctrl+Alt+[/]` is broken on Windows (`Ctrl+Alt` = `AltGr`, brackets report as `Unidentified`). All bindings are user-configurable via TOML config.

- [x] `Action` enum variants (9 total):
  - [x] `SplitRight`, `SplitDown` — split active pane
  - [x] `FocusPaneUp/Down/Left/Right` — directional navigation
  - [x] `NextPane`, `PrevPane` — sequential cycling
  - [x] `ClosePane` — close the focused pane
- [x] `as_str()` roundtrip: all 9 actions parse/serialize correctly
- [x] `parse_action()` arms for all 9 actions
- [x] Default keybindings in `defaults.rs`
- [x] `InProcessMux::set_active_pane(tab_id, pane_id)` helper
- [x] `InProcessMux::active_tab_id(window_id)` helper
- [x] `app/pane_ops.rs` — new module:
  - [x] `execute_pane_action()` — dispatch hub for pane actions
  - [x] `split_pane(direction)` — calls `mux.split_pane()`, applies palette, inserts into `self.panes`
  - [x] `focus_pane_direction(dir)` — `navigate()` + `set_active_pane()`
  - [x] `cycle_pane(forward)` — `cycle()` + `set_active_pane()`
  - [x] `close_focused_pane()` — `mux.close_pane()`, notification handles cleanup
  - [x] `resize_all_panes()` — recompute layouts, resize grid+PTY for each pane
- [x] `execute_action()` wired in `keyboard_input/mod.rs`
- [x] Multi-pane resize propagation:
  - [x] `TabLayoutChanged` notification calls `resize_all_panes()`
  - [x] `sync_grid_layout()` calls `resize_all_panes()` after window resize
- [x] `#[allow(dead_code)]` removed from `InProcessMux::split_pane()`
- [x] Mouse click to focus:
  - [x] On `MouseButton::Left` in grid area: hit-test via `nearest_pane(layouts, x, y)`
  - [x] If clicked pane differs from focused pane: call `set_focused_pane()`
  - [x] Forward the click event to the target pane after focus switch
  - [x] Floating panes take priority in hit-test (higher z_order)

**Tests (keybindings):**
- [x] `action_as_str_roundtrip` includes all 9 new actions
- [x] `split_right_default_binding` — `Ctrl+Shift+O` → SplitRight
- [x] `split_down_default_binding` — `Ctrl+Shift+E` → SplitDown
- [x] `focus_pane_arrow_defaults` — `Ctrl+Alt+Arrow` all 4 directions
- [x] `cycle_pane_has_no_default_binding` — PrevPane/NextPane have no defaults (arrow nav wraps instead)
- [x] `close_pane_default_binding` — `Ctrl+Shift+W` → ClosePane

**Tests (integration — manual):**
- [x] Split right: two panes side-by-side, both functional
- [x] Split down: two panes stacked, both functional
- [x] Arrow focus: navigate between panes in all 4 directions
- [x] Cycle: sequential traversal wraps around (arrow keys wrap)
- [x] Close non-last pane: remaining pane expands
- [x] Close last pane: app exits cleanly (same code path as window close)
- [x] Window resize: all panes resize proportionally
- [x] Mouse click on inactive pane: focus switches

---

## 33.2 Divider Drag Resize

Drag split dividers with the mouse to resize panes. Keyboard resize with modifier+arrow.

**File:** `oriterm/src/app/divider_drag.rs`, `oriterm/src/app/mouse_input.rs`, `oriterm/src/app/pane_ops.rs`

- [x] Divider hit detection:
  - [x] 5px hit zone centered on the 2px divider (detect during `CursorMoved`)
  - [x] Change cursor icon: `CursorIcon::ColResize` for vertical splits, `CursorIcon::RowResize` for horizontal
  - [x] Store `hovering_divider: Option<DividerLayout>` on App
- [x] Divider drag state:
  - [x] On `MouseButton::Left` press while hovering divider: enter drag mode
  - [x] Store initial ratio and mouse position
  - [x] On `CursorMoved` during drag: compute new ratio from delta
    - [x] `new_ratio = initial_ratio + (delta_px / total_px)`
    - [x] Clamp to `0.1..=0.9`
  - [x] On `MouseButton::Left` release: commit ratio via `mux.set_divider_ratio()`
    - [x] Immutable tree update: push old tree to undo stack (via `set_tree()`)
  - [x] Resize affected panes' PTYs after ratio change
- [x] Keyboard resize:
  - [x] `Ctrl+Alt+Shift+Arrow` — resize focused pane in direction
  - [x] Find nearest ancestor split matching the arrow direction
  - [x] Adjust ratio by ±5% per keypress
  - [x] Clamp and resize PTYs
- [x] Equalize: `Ctrl+Shift+=` — reset all ratios to 0.5 (recursive)
  - [x] `mux.equalize_panes(tab_id)` → immutable `SplitTree::equalize()`

**Tests (unit — SplitTree):**
- [x] `set_divider_ratio`: simple, nested inner, nested outer, clamp, nonexistent
- [x] `resize_toward`: right/left/up/down, nested deepest, wrong side noop, clamp, mixed directions

**Tests (integration — manual):**
- [x] Hover on divider: cursor changes to resize icon
- [x] Hover off divider: cursor reverts to default
- [x] Drag divider: ratio updates proportionally to mouse movement
- [x] Drag clamp: ratio never below 0.1 or above 0.9
- [x] Keyboard resize: 5% increments, clamps at bounds
- [x] Equalize: all ratios reset to 0.5 in nested tree
- [x] PTY resize: both affected panes receive new dimensions after ratio change

---

## 33.3 Zoom + Unzoom

Toggle zoom on the focused pane — it fills the entire tab area, hiding all other panes. Unzoom restores the full layout.

**File:** `oriterm/src/app/mod.rs`

**Reference:** WezTerm zoom/unzoom, Zellij fullscreen pane

- [x] Keybind: `Ctrl+Shift+Z` → `Action::ToggleZoom`
- [x] `MuxTab.zoomed_pane: Option<PaneId>`:
  - [x] `Some(id)`: render only this pane at full tab dimensions
  - [x] `None`: render full split tree layout
- [x] Zoom in:
  - [x] Set `MuxTab.zoomed_pane = Some(active_pane)`
  - [x] Resize zoomed pane's PTY to full tab dimensions
  - [x] Emit `TabLayoutChanged` notification
- [x] Zoom out:
  - [x] Set `MuxTab.zoomed_pane = None`
  - [x] Recompute full layout, resize all panes
  - [x] Emit `TabLayoutChanged` notification
- [x] Auto-unzoom triggers:
  - [x] Any split action (`SplitHorizontal`, `SplitVertical`) unzooms first
  - [x] Any navigate action (`FocusPaneDirection`, `CyclePane`) unzooms first
  - [x] Close zoomed pane: unzoom then close
- [x] Visual indicator:
  - [x] Tab bar shows `[Z]` badge when a pane is zoomed
  - [ ] Status bar (future) shows "ZOOM" indicator

**Tests:**
- [x] Toggle zoom: `toggle_zoom_sets_zoomed_pane`, `toggle_zoom_twice_unzooms`
- [x] Unzoom: `unzoom_clears_zoom_and_emits_notification`, `unzoom_noop_when_not_zoomed`
- [x] Close zoomed pane: `close_zoomed_pane_clears_zoom`
- [x] Keybinding: `toggle_zoom_default_binding`, `action_as_str_roundtrip` includes `ToggleZoom`
- [x] MuxTab state: `zoomed_pane_default_none`, `set_zoomed_pane_roundtrip`, `zoomed_pane_cleared_on_none`
- [x] Integration (manual): toggle zoom, auto-unzoom on split/navigate, zoom badge in tab bar

---

## 33.4 Floating Pane Management

Create, drag, resize, and manage floating panes that overlay the tiled layout. Floating panes render on top with a drop shadow and can be toggled back to tiled.

**File:** `oriterm/src/app/floating.rs`, `oriterm/src/gpu/renderer.rs`

**Reference:** Zellij `zellij-server/src/panes/floating_panes/`

- [x] Keybinds:
  - [x] `Ctrl+Shift+P` → `Action::ToggleFloatingPane` — create or focus floating pane (P for Pane; F conflicts with find)
  - [x] `Ctrl+Shift+G` → `Action::ToggleFloatTile` — move focused pane between floating and tiled
- [x] Create floating pane:
  - [x] Spawn new pane via domain (inherits CWD from focused pane)
  - [x] Add to `MuxTab.floating` layer via immutable `FloatingLayer::add()`
  - [x] Default size: 60% of tab area, centered
  - [x] Focus the new floating pane
- [x] Float → tile toggle:
  - [x] Remove from `FloatingLayer`, add to `SplitTree` as a split on the focused tiled pane
  - [x] Pane identity preserved — same PaneId, same shell session
- [x] Tile → float toggle:
  - [x] Remove from `SplitTree` (collapse parent split), add to `FloatingLayer`
  - [x] Position: centered at 60% size
- [x] Floating pane drag (move):
  - [x] Click and drag title area of floating pane → move pane
  - [x] Snap to edges when within 10px of tab boundary
  - [x] Constrain to tab area (no dragging outside)
- [x] Floating pane resize:
  - [x] Drag edges or corners of floating pane → resize
  - [x] 5px hit zone on borders, corner hit zone 10×10px
  - [x] Enforce minimum size (20 columns × 5 rows)
  - [x] Cursor changes: `CursorIcon::NsResize`, `EwResize`, `NwseResize`, `NeswResize`
- [x] Scissored rendering for floating panes:
  - [x] Pane content rendered at viewport-clipped pixel offset (no overrun)
  - [x] Drop shadow: 2px offset, 4px expand, 0.3 opacity black, rendered behind floating pane
  - [x] Border: 1px accent color around floating pane
  - [x] Background: dim_inactive + decoration visually distinguishes from tiled layer
- [x] Floating pane z-order:
  - [x] Click on floating pane → raise to top
  - [x] Newest floating pane starts at top

**Tests:**
- [x] Create floating pane: appears centered at 60% size (`centered_pane_is_60_percent_of_available`, `centered_pane_is_centered_in_available`, `centered_pane_respects_available_offset`)
- [x] Float → tile: pane moves into split tree, removed from floating layer (`move_pane_to_tiled_removes_from_floating`)
- [x] Tile → float: pane moves out of split tree, added to floating layer (`move_pane_to_floating_removes_from_tree`, `move_last_tiled_pane_to_floating_rejected`)
- [x] Drag floating pane: position updates, snaps to edges (`snap_to_left_edge`, `snap_to_right_edge`, `snap_to_corner`, etc.)
- [x] Resize floating pane: dimensions update, minimum enforced (`resize_pane_updates_dimensions`)
- [x] Scissored rendering: content clipped to pane bounds (viewport extraction ensures clipping)
- [x] Z-order: click raises pane, newest on top (`raise_floating_pane_updates_z_order`, `raise_moves_pane_to_front`)

---

## 33.5 Undo + Redo Split Operations

Undo/redo for split tree mutations. Every structural change (split, remove, resize, equalize) pushes to the undo stack via `set_tree()`. Undo restores the previous tree, redo re-applies undone mutations. Both stacks skip stale entries referencing closed panes.

**Files:** `oriterm_mux/src/session/mod.rs`, `oriterm/src/keybindings/{mod,parse,defaults}.rs`, `oriterm/src/app/{keyboard_input/mod,pane_ops}.rs`, `oriterm/src/mux/mod.rs`

- [x] Redo stack on `MuxTab`:
  - [x] `redo: VecDeque<SplitTree>` field, initialized empty
  - [x] `set_tree()` clears redo stack on every new mutation
  - [x] Both stacks capped at `MAX_UNDO_ENTRIES` (32)
- [x] Undo: `Ctrl+Shift+U` → `Action::UndoSplit`
  - [x] Pop from undo, push current tree to redo
  - [x] Restore the popped tree as current
  - [x] **Pane reconciliation**: skip entries referencing closed PaneIds
  - [x] Layout recomputation via `TabLayoutChanged` notification
- [x] Redo: `Ctrl+Shift+Y` → `Action::RedoSplit`
  - [x] Pop from redo, push current tree to undo
  - [x] Restore the popped tree as current
  - [x] Same pane reconciliation logic
- [x] Mutations that push to undo stack (via `set_tree()`):
  - [x] `split_at`, `remove`, `set_ratio`, `equalize`, `resize_toward`
- [x] New mutations clear the redo stack
- [x] Stack size limit: 32 entries (matches existing `MAX_UNDO_ENTRIES`)

**Tests:**
- [x] Split → undo → tree restored to pre-split state (`undo_split_restores_previous_tree`)
- [x] Split → undo → redo → tree back to post-split state (`redo_restores_undone_tree`)
- [x] Multiple undos: walk backward through history (`multiple_undo_then_redo_walks_forward`)
- [x] New mutation after undo: redo stack cleared (`new_mutation_after_undo_clears_redo`, `set_tree_clears_redo_stack`)
- [x] Stack overflow: 32nd+ entry drops oldest (`redo_stack_capped_at_32`)
- [x] Undo past closed pane: skips invalid entry (`undo_skips_stale_pane_entry`, `redo_skips_stale_pane_entry`)
- [x] Keybinding tests: `undo_split_default_binding`, `redo_split_default_binding`, `undo_redo_actions_roundtrip_through_parse`
- [x] InProcessMux tests: `undo_split_restores_previous_tree`, `redo_split_restores_undone_tree`, `split_undo_redo_undo_cycle`, `undo_past_closed_pane_skips_entry`

---

## 33.6 Section Completion

- [x] All 33.1–33.5 items complete
- [x] Spatial navigation: `Alt+Shift+Arrow` directional, `Alt+Shift+{/}` cycling, mouse click
- [x] Divider drag resize: mouse + keyboard, clamping, PTY resize
- [x] Zoom/unzoom: `Ctrl+Shift+Z`, auto-unzoom on split/navigate
- [x] Floating panes: create, drag, resize, z-order, scissored rendering, drop shadow
- [x] Float↔tile toggle: preserves pane identity and shell session
- [x] Undo/redo: full history for split tree mutations
- [x] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [x] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [x] `cargo test` — all tests pass
- [x] **Navigation test**: 4-pane grid, navigate all directions, verify correct focus
- [x] **Resize test**: drag divider, verify ratio change and PTY resize
- [x] **Floating test**: create floating pane, drag, resize, toggle to tiled
- [x] **Undo test**: split 3 times, undo all 3, verify original layout restored

**Exit Criteria:** Full split pane interaction with no external dependencies (tmux, screen). Spatial navigation works for any layout. Floating panes overlay the tiled layout with proper rendering. Undo/redo enables safe experimentation with layouts. Every interaction from the superseded Section 26 is implemented, plus floating panes and undo/redo.
