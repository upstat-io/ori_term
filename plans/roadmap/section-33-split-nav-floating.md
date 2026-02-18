---
section: 33
title: Split Navigation + Floating Panes
status: not-started
tier: 4M
goal: Spatial navigation keybinds, divider drag resize, zoom/unzoom, floating pane creation and management, scissored rendering, float-tile toggle, undo/redo split operations
sections:
  - id: "33.1"
    title: Spatial Navigation Keybinds
    status: not-started
  - id: "33.2"
    title: Divider Drag Resize
    status: not-started
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

**Status:** Not Started
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

Keyboard shortcuts for navigating between panes using directional movement and sequential cycling. These keybinds work in both tiled and floating modes.

**File:** `oriterm/src/app/input_keyboard.rs`, `oriterm/src/keybindings.rs`

- [ ] Default keybindings (configurable):
  - [ ] `Alt+Arrow` (Up/Down/Left/Right) — move focus to pane in direction
  - [ ] `Alt+[` / `Alt+]` — cycle focus backward/forward through panes
  - [ ] `Ctrl+Shift+D` — split horizontal (new pane below current)
  - [ ] `Ctrl+Shift+E` — split vertical (new pane right of current)
  - [ ] `Ctrl+W` — close focused pane (not the entire tab)
- [ ] `Action` enum additions:
  - [ ] `SplitHorizontal` — split active pane horizontally
  - [ ] `SplitVertical` — split active pane vertically
  - [ ] `FocusPaneDirection(Direction)` — navigate to pane in direction
  - [ ] `CyclePaneForward` / `CyclePaneBackward` — sequential cycle
  - [ ] `ClosePane` — close the focused pane
- [ ] Keybind execution:
  - [ ] `SplitHorizontal` → `mux.split_pane(tab_id, pane_id, Horizontal, config)`
    - [ ] New pane inherits CWD from focused pane
  - [ ] `FocusPaneDirection(dir)` → `navigate(layouts, focused, dir)` → update `MuxTab.active_pane`
  - [ ] `ClosePane` → `mux.close_pane(pane_id)`
    - [ ] If last pane: close tab (and possibly window/app per Section 32 logic)
- [ ] Mouse click to focus:
  - [ ] On `MouseButton::Left` in grid area: hit-test against `PaneLayout` rects
  - [ ] If clicked pane differs from focused pane: update `MuxTab.active_pane`
  - [ ] Floating panes take priority in hit-test (higher z_order)

**Tests:**
- [ ] `Alt+Right` from left pane focuses right pane
- [ ] `Alt+Down` from top pane focuses bottom pane
- [ ] Navigation at boundary: `Alt+Left` from leftmost pane → no change
- [ ] `Alt+]` cycles through panes in order, wraps at end
- [ ] `Ctrl+Shift+D` creates horizontal split, new pane receives focus
- [ ] `Ctrl+W` closes focused pane, focus moves to sibling
- [ ] `Ctrl+W` on last pane closes the tab
- [ ] Mouse click on inactive pane: focus switches

---

## 33.2 Divider Drag Resize

Drag split dividers with the mouse to resize panes. Keyboard resize with modifier+arrow.

**File:** `oriterm/src/app/input_mouse.rs`, `oriterm/src/drag.rs`

- [ ] Divider hit detection:
  - [ ] 5px hit zone centered on the 2px divider (detect during `CursorMoved`)
  - [ ] Change cursor icon: `CursorIcon::ColResize` for vertical splits, `CursorIcon::RowResize` for horizontal
  - [ ] Store `hovering_divider: Option<DividerLayout>` on App
- [ ] Divider drag state:
  - [ ] On `MouseButton::Left` press while hovering divider: enter drag mode
  - [ ] Store initial ratio and mouse position
  - [ ] On `CursorMoved` during drag: compute new ratio from delta
    - [ ] `new_ratio = initial_ratio + (delta_px / total_px)`
    - [ ] Clamp to `0.1..=0.9`
  - [ ] On `MouseButton::Left` release: commit ratio via `mux.set_ratio()`
    - [ ] Immutable tree update: push old tree to undo stack
  - [ ] Resize affected panes' PTYs after ratio change
- [ ] Keyboard resize:
  - [ ] `Alt+Shift+Arrow` — resize focused pane in direction
  - [ ] Find nearest ancestor split matching the arrow direction
  - [ ] Adjust ratio by ±5% per keypress
  - [ ] Clamp and resize PTYs
- [ ] Equalize: `Ctrl+Shift+=` — reset all ratios to 0.5 (recursive)
  - [ ] `mux.equalize(tab_id)` → immutable `SplitTree::equalize()`

**Tests:**
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

- [ ] `SplitHistory` struct on `MuxTab`:
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
- [ ] Spatial navigation: `Alt+Arrow` directional, `Alt+[/]` cycling, mouse click
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
