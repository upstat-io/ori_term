---
section: 26
title: Split Panes
status: superseded
tier: 7
goal: Horizontal/vertical splits within a window, binary tree layout, pane navigation and resize
superseded_by: [29, 31, 33]
superseded_reason: "Absorbed into Sections 29 (Mux Crate + Layout Engine), 31 (In-Process Mux + Multi-Pane Rendering), and 33 (Split Navigation + Floating Panes). Split panes are now a foundational mux feature, not a Tier 7 afterthought."
sections:
  - id: "26.1"
    title: Split Data Model
    status: superseded
  - id: "26.2"
    title: Split Creation & Navigation
    status: superseded
  - id: "26.3"
    title: Split Rendering
    status: superseded
  - id: "26.4"
    title: Split Resize
    status: superseded
  - id: "26.5"
    title: Section Completion
    status: superseded
---

# Section 26: Split Panes

> **SUPERSEDED** — This section has been absorbed into the first-class multiplexing architecture.
> - Split data model (SplitTree, PaneNode) → **Section 29** (Mux Crate + Layout Engine)
> - Split rendering + multi-pane frame loop → **Section 31** (In-Process Mux + Multi-Pane Rendering)
> - Split navigation, resize, zoom, floating panes → **Section 33** (Split Navigation + Floating Panes)
>
> The original design added splits as a Tier 7 feature bolted onto existing tabs.
> The multiplexing redesign makes the layout engine foundational (Tier 4M),
> adds floating panes (Zellij-inspired), and builds toward a full server mode
> with session persistence. The immutable SplitTree and domain abstraction
> subsume everything in this section and go far beyond it.

**Status:** Superseded
**Goal:** Allow users to split the terminal window into multiple panes, each running its own shell, with keyboard and mouse navigation between them. This is the largest architectural change remaining.

**Crate:** `oriterm` (app + rendering layer), `oriterm_core` (pane data model)
**Dependencies:** Existing crate dependencies only

**Inspired by:**
- Ghostty: native AppKit/GTK splits, platform-specific UI
- WezTerm: Lua-configurable split layouts, zoom/unzoom
- Kitty: flexible window layouts (tall, fat, grid, splits, stack)
- tmux: the baseline expectation for split behavior

**Architecture impact:** Each window currently has a flat `Vec<TabId>` of tabs. This section replaces the flat structure with a tree layout per tab. A tab becomes a layout container; each leaf in the tree is a pane (shell + grid). This matches Ghostty and WezTerm's approach.

---

## 26.1 Split Data Model

Replace flat tab list with a binary tree layout per tab.

**File:** `oriterm_core/src/pane.rs` (PaneId, Pane, PaneNode), `oriterm/src/tab.rs` (Tab refactor)

**Reference:** `_old/src/tab/mod.rs` (current Tab struct), `_old/src/app/mod.rs` (App.tabs)

**Design:** Each tab entry becomes a `PaneTree` -- a binary tree where leaves are terminal panes and internal nodes are splits.

```rust
/// A single terminal pane: owns a Grid, PTY, and VTE parser.
struct Pane {
    id: PaneId,
    grid_primary: Grid,
    grid_alt: Grid,
    active_is_alt: bool,
    pty_writer: Option<Box<dyn Write + Send>>,
    pty_master: Box<dyn MasterPty>,
    _child: Box<dyn Child>,
    processor: vte::ansi::Processor,
    raw_parser: vte::Parser,
    // ... (all fields currently in Tab)
}

/// Layout tree for a single tab.
enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,       // 0.0-1.0, default 0.5
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

enum SplitDirection { Horizontal, Vertical }

/// A PaneId is globally unique, like TabId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PaneId(u64);
```

**Migration path from current Tab struct:**
1. Extract all terminal-session fields from `Tab` into a new `Pane` struct.
2. `Tab` becomes: `{ title, pane_tree: PaneNode, active_pane: PaneId, ... }`.
3. `App.panes: HashMap<PaneId, Pane>` replaces the terminal fields in `App.tabs`.
4. `App.tabs` keeps metadata (title, color scheme, selection of "tab" in tab bar).
5. When a tab has a single pane (no splits), `PaneNode::Leaf(pane_id)` -- functionally identical to today.

- [ ] Define `PaneId`, `Pane`, `PaneNode`, `SplitDirection` types
- [ ] Refactor `Tab` to separate session state (`Pane`) from tab metadata
- [ ] Add `PaneNode` to `Tab` (defaults to `Leaf` wrapping one pane)
- [ ] `active_pane: PaneId` on `Tab` tracks which pane has focus
- [ ] `App.panes: HashMap<PaneId, Pane>` global pane registry
- [ ] Ratio is 0.0-1.0 (default 0.5 for even split)
- [ ] Tree can be arbitrarily nested (split a split)
- [ ] `PaneNode::compute_rects(total_rect) -> Vec<(PaneId, Rect)>`: recursively subdivide the available pixel area

**Key design decisions:**
- Panes are **not** tabs in the tab bar -- the tab bar still shows tabs, each tab may contain 1+ panes.
- Tab tear-off still works: tearing off moves the whole tab (with all its panes).
- PTY events use `PaneId` instead of `TabId` for routing `PtyOutput`.

**Tests:**
- [ ] Single-pane tab produces one rect covering full area
- [ ] Horizontal split produces two rects stacked vertically
- [ ] Vertical split produces two rects side by side
- [ ] Nested splits subdivide correctly (3+ levels deep)
- [ ] `compute_rects` subtracts divider width from available space
- [ ] Ratio 0.0 and 1.0 are clamped to minimum pane size

---

## 26.2 Split Creation & Navigation

Keybindings and focus management for panes.

**File:** `oriterm/src/app.rs` (keybindings, focus), `oriterm/src/keybindings.rs` (action enum)

**Reference:** `_old/src/app/mod.rs` (tab management), `_old/src/keybindings/defaults.rs`

- [ ] Create splits:
  - [ ] `Ctrl+Shift+D` -- split horizontal (new pane below)
  - [ ] `Ctrl+Shift+E` -- split vertical (new pane right)
  - [ ] New pane spawns a new `Pane` with a new PTY
  - [ ] New pane inherits CWD from focused pane (if OSC 7 reported a CWD)
  - [ ] Insert split: replace `Leaf(active)` with `Split { first: Leaf(active), second: Leaf(new) }`
- [ ] Navigate between panes:
  - [ ] `Alt+Arrow` -- move focus to pane in direction
    - [ ] Find pane whose rect center is closest in the given direction
    - [ ] Only consider panes in the same tab
  - [ ] `Alt+[` / `Alt+]` -- cycle focus between panes (in tree order)
  - [ ] Click on a pane to focus it (grid area hit-test per pane rect)
  - [ ] Visual indicator: focused pane has a colored border or accent on its edge
- [ ] Close pane:
  - [ ] `Ctrl+W` closes the focused pane (not the whole tab)
  - [ ] When a split has one child removed, collapse: replace `Split { first, _ }` with `first`
  - [ ] When last pane closes, close the tab
  - [ ] Closing a pane kills its PTY and removes it from `App.panes`
- [ ] Zoom/unzoom:
  - [ ] `Ctrl+Shift+Z` -- toggle zoom on focused pane (fills entire tab area)
  - [ ] Store `zoomed_pane: Option<PaneId>` on Tab
  - [ ] When zoomed, render only that pane at full tab dimensions
  - [ ] Tab bar shows "[Z]" or zoom icon when a pane is zoomed
  - [ ] Any split/navigate action unzooms first

**Tests:**
- [ ] Split horizontal creates two panes in correct layout
- [ ] Split vertical creates two panes in correct layout
- [ ] Alt+Arrow navigates to correct directional neighbor
- [ ] Alt+[/] cycles through panes in tree order
- [ ] Close pane collapses split node to remaining child
- [ ] Close last pane closes the tab
- [ ] Zoom renders single pane at full size
- [ ] Split action unzooms before splitting

---

## 26.3 Split Rendering

Draw split borders and render each pane independently.

**File:** `oriterm/src/gpu/renderer.rs` (multi-pane rendering), `oriterm/src/gpu/render_grid.rs` (per-pane grid)

**Reference:** `_old/src/gpu/renderer.rs` (single-grid rendering), `_old/src/gpu/render_grid.rs`

- [ ] Layout computation:
  - [ ] `PaneNode::compute_rects(available: Rect) -> Vec<(PaneId, Rect)>`
  - [ ] Subtract divider width (2px) when splitting
  - [ ] Each pane's pixel rect converted to cols/rows for grid and PTY resize
- [ ] Render each pane:
  - [ ] `build_grid_instances()` already takes grid dimensions and offset
  - [ ] Call it once per pane, with each pane's offset and size
  - [ ] Each pane has its own: cursor, scroll position, selection
- [ ] Split divider rendering:
  - [ ] 2px line between panes (palette surface color)
  - [ ] Active pane border: highlight the focused pane's edge with accent color
  - [ ] Inactive panes optionally dimmed (lower opacity -- multiply fg alpha by 0.7)
- [ ] Render order:
  1. All pane backgrounds (one pass)
  2. Split dividers
  3. All pane foreground glyphs (one pass)
  4. Cursor for active pane
  5. Selection highlights per pane
- [ ] PTY resize: when layout changes, resize each pane's PTY independently
  - [ ] `pane.pty_master.resize(pane_cols, pane_rows)`

**Tests:**
- [ ] Two-pane layout produces correct pixel rects with divider gap
- [ ] Each pane renders its own grid content independently
- [ ] Active pane border renders with accent color
- [ ] Inactive pane dimming applies correct alpha multiplier
- [ ] PTY resize called with per-pane dimensions on layout change

---

## 26.4 Split Resize

Drag to resize splits with mouse and keyboard.

**File:** `oriterm/src/app.rs` (mouse handling, keybindings), `oriterm/src/drag.rs` (drag state)

**Reference:** `_old/src/app/input_mouse.rs` (mouse handling), `_old/src/drag.rs` (drag state machine)

- [ ] Mouse drag on split divider:
  - [ ] Detect hover on divider: 5px hit zone centered on the 2px divider
  - [ ] Change cursor to resize icon (`CursorIcon::ColResize` / `RowResize`)
  - [ ] On drag: update `ratio` in the `Split` node
  - [ ] Clamp ratio so minimum pane size is 4 columns / 2 rows
  - [ ] Resize all affected pane PTYs after ratio change
- [ ] Keyboard resize:
  - [ ] `Alt+Shift+Arrow` -- resize focused pane in direction
  - [ ] Find the nearest split ancestor in the direction
  - [ ] Adjust ratio by +/-5% of parent dimension
- [ ] Equalize: `Ctrl+Shift+=` -- reset all split ratios to 0.5 (recursive)
- [ ] Window resize: pane pixel rects recalculated proportionally
  - [ ] Each pane gets `resize()` called with new cols/rows
  - [ ] Text reflow applies independently per pane

**Tests:**
- [ ] Divider hit-test detects 5px zone correctly
- [ ] Drag updates ratio proportionally to mouse movement
- [ ] Ratio clamped to enforce minimum pane size
- [ ] Keyboard resize adjusts ratio by 5% increments
- [ ] Equalize resets all nested ratios to 0.5
- [ ] Window resize preserves split ratios and reflows each pane

---

## 26.5 Section Completion

- [ ] All 26.1-26.4 items complete
- [ ] Horizontal and vertical splits work
- [ ] Nested splits (split a split) work
- [ ] Keyboard navigation between panes (Alt+Arrow, Alt+[/])
- [ ] Mouse click to focus pane
- [ ] Drag to resize split divider
- [ ] Close pane collapses the split tree correctly
- [ ] Each pane has independent scroll, selection, cursor
- [ ] PTY resize sent to each pane independently
- [ ] Zoom/unzoom a single pane
- [ ] No rendering artifacts at split boundaries
- [ ] Tab tear-off works with multi-pane tabs
- [ ] Performance: multiple panes don't cause frame drops

**Exit Criteria:** User can create, navigate, resize, and close split panes with no tmux needed for basic multi-pane workflows.
