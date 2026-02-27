---
section: 33
title: Split Navigation + Floating Panes
status: in-progress
tier: 4M
goal: Spatial navigation keybinds, divider drag resize, zoom/unzoom, floating pane creation and management, scissored rendering, float-tile toggle, undo/redo split operations
sections:
  - id: "33.1"
    title: Spatial Navigation Keybinds
    status: in-progress
  - id: "33.2"
    title: Divider Drag Resize
    status: in-progress
  - id: "33.3"
    title: Zoom + Unzoom
    status: not-started
  - id: "33.4"
    title: Floating Pane Management
    status: not-started
  - id: "33.5"
    title: Undo + Redo Split Operations
    status: not-started
  - id: "33.6"
    title: Section Completion
    status: not-started
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
| `PrevPane` | `Ctrl+Alt+[` | `goto_split:previous` |
| `NextPane` | `Ctrl+Alt+]` | `goto_split:next` |
| `ClosePane` | `Ctrl+Shift+W` | `close_surface` |

Ghostty uses `Ctrl+Super+[/]` for cycle on Linux — we use `Ctrl+Alt` instead since Super (Windows key) is intercepted by the OS on Windows. All bindings are user-configurable via TOML config.

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
- [x] `split_right_default_binding` — `Alt+Shift+|` → SplitRight
- [x] `split_down_default_binding` — `Alt+Shift+_` → SplitDown
- [x] `focus_pane_arrow_defaults` — all 4 directions
- [x] `cycle_pane_defaults` — `Alt+Shift+{/}` → Prev/NextPane
- [x] `close_pane_default_binding` — `Ctrl+Shift+W` → ClosePane

**Tests (integration — manual):**
- [ ] Split right: two panes side-by-side, both functional
- [ ] Split down: two panes stacked, both functional
- [ ] Arrow focus: navigate between panes in all 4 directions
- [ ] Cycle: sequential traversal wraps around
- [ ] Close non-last pane: remaining pane expands
- [ ] Close last pane: tab closes
- [ ] Window resize: all panes resize proportionally
- [ ] Mouse click on inactive pane: focus switches

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
    - [ ] Immutable tree update: push old tree to undo stack  <!-- blocked-by:33.5 -->
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
- [ ] Hover on divider: cursor changes to resize icon
- [ ] Hover off divider: cursor reverts to default
- [ ] Drag divider: ratio updates proportionally to mouse movement
- [ ] Drag clamp: ratio never below 0.1 or above 0.9
- [ ] Keyboard resize: 5% increments, clamps at bounds
- [ ] Equalize: all ratios reset to 0.5 in nested tree
- [ ] PTY resize: both affected panes receive new dimensions after ratio change

---

## 33.3 Zoom + Unzoom

Toggle zoom on the focused pane — it fills the entire tab area, hiding all other panes. Unzoom restores the full layout.

**File:** `oriterm/src/app/mod.rs`

**Reference:** WezTerm zoom/unzoom, Zellij fullscreen pane

- [ ] Keybind: `Ctrl+Shift+Z` → `Action::ToggleZoom`
- [ ] `MuxTab.zoomed_pane: Option<PaneId>`:
  - [ ] `Some(id)`: render only this pane at full tab dimensions
  - [ ] `None`: render full split tree layout
- [ ] Zoom in:
  - [ ] Set `MuxTab.zoomed_pane = Some(active_pane)`
  - [ ] Resize zoomed pane's PTY to full tab dimensions
  - [ ] Emit `TabLayoutChanged` notification
- [ ] Zoom out:
  - [ ] Set `MuxTab.zoomed_pane = None`
  - [ ] Recompute full layout, resize all panes
  - [ ] Emit `TabLayoutChanged` notification
- [ ] Auto-unzoom triggers:
  - [ ] Any split action (`SplitHorizontal`, `SplitVertical`) unzooms first
  - [ ] Any navigate action (`FocusPaneDirection`, `CyclePane`) unzooms first
  - [ ] Close zoomed pane: unzoom then close
- [ ] Visual indicator:
  - [ ] Tab bar shows `[Z]` badge or zoom icon when a pane is zoomed
  - [ ] Status bar (future) shows "ZOOM" indicator

**Tests:**
- [ ] Toggle zoom: pane fills entire tab area
- [ ] Toggle again: restores full split layout
- [ ] Split while zoomed: unzooms first, then splits
- [ ] Navigate while zoomed: unzooms first, then navigates
- [ ] Close zoomed pane: unzooms, removes pane, layout updates
- [ ] Zoom badge: appears in tab bar when zoomed

---

## 33.4 Floating Pane Management

Create, drag, resize, and manage floating panes that overlay the tiled layout. Floating panes render on top with a drop shadow and can be toggled back to tiled.

**File:** `oriterm/src/app/floating.rs`, `oriterm/src/gpu/renderer.rs`

**Reference:** Zellij `zellij-server/src/panes/floating_panes/`

- [ ] Keybinds:
  - [ ] `Ctrl+Shift+F` → `Action::ToggleFloatingPane` — create or focus floating pane
  - [ ] `Ctrl+Shift+G` → `Action::ToggleFloatTile` — move focused pane between floating and tiled
- [ ] Create floating pane:
  - [ ] Spawn new pane via domain (inherits CWD from focused pane)
  - [ ] Add to `MuxTab.floating` layer via immutable `FloatingLayer::add()`
  - [ ] Default size: 60% of tab area, centered
  - [ ] Focus the new floating pane
- [ ] Float → tile toggle:
  - [ ] Remove from `FloatingLayer`, add to `SplitTree` as a split on the focused tiled pane
  - [ ] Pane identity preserved — same PaneId, same shell session
- [ ] Tile → float toggle:
  - [ ] Remove from `SplitTree` (collapse parent split), add to `FloatingLayer`
  - [ ] Position: centered at 60% size
- [ ] Floating pane drag (move):
  - [ ] Click and drag title area of floating pane → move pane
  - [ ] Snap to edges when within 10px of tab boundary
  - [ ] Constrain to tab area (no dragging outside)
- [ ] Floating pane resize:
  - [ ] Drag edges or corners of floating pane → resize
  - [ ] 5px hit zone on borders, corner hit zone 10×10px
  - [ ] Enforce minimum size (20 columns × 5 rows)
  - [ ] Cursor changes: `CursorIcon::NResize`, `SeResize`, etc.
- [ ] Scissored rendering for floating panes:
  - [ ] `render_frame_scissored()`: render floating pane content clipped to its rect
  - [ ] Drop shadow: 4px offset, 50% opacity black, rendered behind floating pane
  - [ ] Border: 1px accent color around floating pane
  - [ ] Background: slightly elevated opacity to distinguish from tiled layer
- [ ] Floating pane z-order:
  - [ ] Click on floating pane → raise to top
  - [ ] Newest floating pane starts at top

**Tests:**
- [ ] Create floating pane: appears centered at 60% size
- [ ] Float → tile: pane moves into split tree, removed from floating layer
- [ ] Tile → float: pane moves out of split tree, added to floating layer
- [ ] Drag floating pane: position updates, snaps to edges
- [ ] Resize floating pane: dimensions update, minimum enforced
- [ ] Scissored rendering: content clipped to pane bounds
- [ ] Z-order: click raises pane, newest on top

---

## 33.5 Undo + Redo Split Operations

Undo/redo for split tree mutations. Every structural change (split, remove, resize, equalize) pushes to the undo stack. Undo restores the previous tree.

**File:** `oriterm_mux/src/layout/history.rs`

- [ ] `SplitHistory` struct on `MuxTab`:  <!-- unblocks:33.2 -->
  - [ ] `undo_stack: Vec<SplitTree>` — previous trees (most recent last), capacity 50
  - [ ] `redo_stack: Vec<SplitTree>` — trees undone (cleared on new mutation)
- [ ] Undo: `Ctrl+Shift+U` → `Action::UndoSplit`
  - [ ] Pop from `undo_stack`, push current tree to `redo_stack`
  - [ ] Restore the popped tree as current
  - [ ] **Pane reconciliation**: if the restored tree references a PaneId that no longer exists (pane was closed), skip that undo entry
  - [ ] Resize all panes to match new layout
- [ ] Redo: `Ctrl+Shift+Y` → `Action::RedoSplit`
  - [ ] Pop from `redo_stack`, push current tree to `undo_stack`
  - [ ] Restore the popped tree as current
  - [ ] Same pane reconciliation logic
- [ ] Mutations that push to undo stack:
  - [ ] `split_at` — before adding split
  - [ ] `remove` — before removing pane
  - [ ] `set_ratio` — before changing ratio
  - [ ] `equalize` — before equalizing
  - [ ] `swap` — before swapping
- [ ] New mutations clear the redo stack
- [ ] Stack size limit: 50 entries. Oldest entry discarded when full.

**Tests:**
- [ ] Split → undo → tree restored to pre-split state
- [ ] Split → undo → redo → tree back to post-split state
- [ ] Multiple undos: walk backward through history
- [ ] New mutation after undo: redo stack cleared
- [ ] Stack overflow: 51st entry drops oldest
- [ ] Undo past closed pane: skips invalid entry

---

## 33.6 Section Completion

- [ ] All 33.1–33.5 items complete
- [ ] Spatial navigation: `Alt+Shift+Arrow` directional, `Alt+Shift+{/}` cycling, mouse click
- [ ] Divider drag resize: mouse + keyboard, clamping, PTY resize
- [ ] Zoom/unzoom: `Ctrl+Shift+Z`, auto-unzoom on split/navigate
- [ ] Floating panes: create, drag, resize, z-order, scissored rendering, drop shadow
- [ ] Float↔tile toggle: preserves pane identity and shell session
- [ ] Undo/redo: full history for split tree mutations
- [ ] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test` — all tests pass
- [ ] **Navigation test**: 4-pane grid, navigate all directions, verify correct focus
- [ ] **Resize test**: drag divider, verify ratio change and PTY resize
- [ ] **Floating test**: create floating pane, drag, resize, toggle to tiled
- [ ] **Undo test**: split 3 times, undo all 3, verify original layout restored

**Exit Criteria:** Full split pane interaction with no external dependencies (tmux, screen). Spatial navigation works for any layout. Floating panes overlay the tiled layout with proper rendering. Undo/redo enables safe experimentation with layouts. Every interaction from the superseded Section 26 is implemented, plus floating panes and undo/redo.
