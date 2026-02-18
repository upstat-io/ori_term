---
section: 29
title: Mux Crate + Layout Engine
status: not-started
tier: 4M
goal: Create the oriterm_mux crate with newtype IDs, immutable SplitTree, FloatingLayer, spatial navigation, and layout computation
sections:
  - id: "29.1"
    title: Crate Bootstrap + Newtype IDs
    status: not-started
  - id: "29.2"
    title: Immutable SplitTree
    status: not-started
  - id: "29.3"
    title: FloatingLayer
    status: not-started
  - id: "29.4"
    title: Layout Computation
    status: not-started
  - id: "29.5"
    title: Spatial Navigation
    status: not-started
  - id: "29.6"
    title: Section Completion
    status: not-started
---

# Section 29: Mux Crate + Layout Engine

**Status:** Not Started
**Goal:** Create the `oriterm_mux` crate — the multiplexing foundation. Defines all identity types, the immutable split tree, floating pane layer, layout computation, and spatial navigation. Pure data structures with no I/O, no GUI, no PTY — fully testable in isolation.

**Crate:** `oriterm_mux` (new crate)
**Dependencies:** None (pure data structures). `serde` for serialization support.
**Prerequisite:** Section 04 (PTY + Event Loop) complete — mux builds on PTY abstractions.

**Inspired by:**
- Ghostty: immutable `SplitTree` — structural sharing, no in-place mutation, undo via history stack
- WezTerm: binary tree splits with `tab_id`/`pane_id` separation, `PaneEntry` for layout results
- Zellij: tiled + floating pane model, floating overlay with position/size
- tmux: the baseline expectation for pane navigation and resize behavior

**Architecture:** `oriterm_mux` sits between `oriterm_core` (terminal library) and `oriterm` (GUI binary). It owns all multiplexing state: which panes exist, how they're laid out, which tab/window they belong to, and how to navigate between them. The GUI binary becomes a thin rendering client.

---

## 29.1 Crate Bootstrap + Newtype IDs

Create the `oriterm_mux` workspace member with newtype identity types. These IDs are the currency of the entire mux system — every other component references panes, tabs, windows, and sessions by these types.

**File:** `oriterm_mux/src/lib.rs`, `oriterm_mux/src/id.rs`

- [ ] Create `oriterm_mux/` directory and `Cargo.toml`:
  - [ ] `[package] name = "oriterm_mux"`, edition 2021
  - [ ] Dependencies: `serde` (with `derive` feature, optional behind `serde` feature flag)
  - [ ] No dependency on `oriterm_core` or `oriterm` — pure standalone crate
- [ ] Add `"oriterm_mux"` to workspace `members` in root `Cargo.toml`
- [ ] Add `oriterm_mux` as dependency of `oriterm` (binary crate)
- [ ] Newtype IDs in `oriterm_mux/src/id.rs`:
  - [ ] `PaneId(u64)` — globally unique pane identifier
  - [ ] `TabId(u64)` — globally unique tab identifier
  - [ ] `WindowId(u64)` — mux-level window identifier (NOT winit `WindowId`)
  - [ ] `SessionId(u64)` — session identifier (for persistence/restore)
  - [ ] All IDs: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`
  - [ ] All IDs: `impl Display` (for logging: `Pane(42)`, `Tab(7)`)
  - [ ] All IDs: optional `#[derive(Serialize, Deserialize)]` behind `serde` feature
- [ ] `IdAllocator` — monotonic counter per ID type:
  - [ ] `IdAllocator::new() -> Self` — starts at 1 (0 reserved for "none")
  - [ ] `IdAllocator::next(&mut self) -> u64` — increment and return
  - [ ] Separate allocators for panes, tabs, windows, sessions
- [ ] `oriterm_mux/src/lib.rs` — re-export public API:
  - [ ] `pub mod id;`
  - [ ] `pub mod layout;` (section 29.2-29.4)
  - [ ] `pub mod nav;` (section 29.5)

**Tests:**
- [ ] IDs are `Copy`, `Hash`, `Eq` — compile-time trait bound check
- [ ] `IdAllocator` produces monotonically increasing unique values
- [ ] `Display` output matches expected format
- [ ] Different ID types are not interchangeable (type safety)

---

## 29.2 Immutable SplitTree

The layout tree is the core data structure. Following Ghostty's approach: the tree is **immutable** — every mutation returns a new tree, enabling structural sharing, undo/redo, and safe concurrent reads. Internal nodes are splits; leaves are pane references.

**File:** `oriterm_mux/src/layout/split_tree.rs`

**Reference:** Ghostty `src/terminal/SplitTree.zig`, WezTerm `mux/src/tab.rs` (Tree struct)

- [ ] `SplitTree` enum:
  ```rust
  /// Immutable binary layout tree.
  ///
  /// Every mutation method returns a new tree (COW via `Arc`).
  /// History of previous trees enables undo/redo.
  #[derive(Debug, Clone, PartialEq)]
  pub enum SplitTree {
      Leaf(PaneId),
      Split {
          direction: SplitDirection,
          ratio: f32,
          first: Arc<SplitTree>,
          second: Arc<SplitTree>,
      },
  }
  ```
- [ ] `SplitDirection` enum: `Horizontal` (top/bottom), `Vertical` (left/right)
- [ ] Immutable mutation methods (all return new `SplitTree`):
  - [ ] `split_at(pane: PaneId, dir: SplitDirection, new_pane: PaneId, ratio: f32) -> SplitTree` — find `Leaf(pane)`, replace with `Split { first: Leaf(pane), second: Leaf(new_pane) }`
  - [ ] `remove(pane: PaneId) -> Option<SplitTree>` — remove pane, collapse parent split to sibling. Returns `None` if removing the last pane.
  - [ ] `set_ratio(pane: PaneId, direction: SplitDirection, new_ratio: f32) -> SplitTree` — find the nearest ancestor split matching direction, update ratio (clamped 0.1..=0.9)
  - [ ] `equalize() -> SplitTree` — recursively set all ratios to 0.5
  - [ ] `swap(a: PaneId, b: PaneId) -> SplitTree` — swap two pane positions in the tree
- [ ] Query methods:
  - [ ] `contains(pane: PaneId) -> bool`
  - [ ] `pane_count() -> usize` — number of leaves
  - [ ] `panes() -> Vec<PaneId>` — all pane IDs in tree order (depth-first, first-child-first)
  - [ ] `depth() -> usize` — maximum nesting depth
  - [ ] `parent_split(pane: PaneId) -> Option<(SplitDirection, f32)>` — direction and ratio of the split containing this pane
  - [ ] `sibling(pane: PaneId) -> Option<PaneId>` — the other pane in the same split (only if sibling is a leaf)
- [ ] Ratio clamping: minimum 0.1, maximum 0.9 — enforces minimum pane size
- [ ] `Arc` sharing: unchanged subtrees share memory between old and new trees

**Tests:**
- [ ] Single pane: `Leaf(p1)` — `pane_count() == 1`, `contains(p1) == true`
- [ ] Split at leaf: produces correct `Split` node with original and new pane
- [ ] Nested split: split a pane inside an existing split — 3 panes total
- [ ] Remove middle pane: tree collapses correctly, remaining panes preserved
- [ ] Remove last pane: returns `None`
- [ ] `equalize()` sets all ratios to 0.5 recursively
- [ ] Ratio clamping: values below 0.1 clamped to 0.1, above 0.9 to 0.9
- [ ] `swap()` exchanges two pane positions
- [ ] `panes()` returns depth-first order
- [ ] Structural sharing: after `split_at`, unchanged subtrees share `Arc` pointers

---

## 29.3 FloatingLayer

Floating panes overlay the tiled layout. Inspired by Zellij's floating pane system — panes have absolute position and size within the window, rendered on top of the tiled layer with a drop shadow.

**File:** `oriterm_mux/src/layout/floating.rs`

**Reference:** Zellij `zellij-server/src/panes/floating_panes/` (FloatingPaneGrid, FloatingPanes)

- [ ] `FloatingPane` struct:
  ```rust
  pub struct FloatingPane {
      pub pane_id: PaneId,
      pub x: f32,       // Logical pixels from left edge of tab area.
      pub y: f32,       // Logical pixels from top edge of tab area.
      pub width: f32,   // Logical width in pixels.
      pub height: f32,  // Logical height in pixels.
      pub z_order: u32, // Higher = closer to viewer.
  }
  ```
- [ ] `FloatingLayer` struct:
  - [ ] `panes: Vec<FloatingPane>` — ordered by z_order (ascending)
  - [ ] Immutable mutation methods (return new `FloatingLayer`):
    - [ ] `add(pane: FloatingPane) -> FloatingLayer`
    - [ ] `remove(pane_id: PaneId) -> FloatingLayer`
    - [ ] `move_pane(pane_id: PaneId, x: f32, y: f32) -> FloatingLayer`
    - [ ] `resize_pane(pane_id: PaneId, width: f32, height: f32) -> FloatingLayer`
    - [ ] `raise(pane_id: PaneId) -> FloatingLayer` — bring to front (highest z_order)
    - [ ] `lower(pane_id: PaneId) -> FloatingLayer` — send to back
  - [ ] Query methods:
    - [ ] `hit_test(x: f32, y: f32) -> Option<PaneId>` — topmost floating pane at point (reverse z_order)
    - [ ] `pane_rect(pane_id: PaneId) -> Option<Rect>` — pixel rect for a floating pane
    - [ ] `contains(pane_id: PaneId) -> bool`
    - [ ] `panes() -> &[FloatingPane]`
    - [ ] `is_empty() -> bool`
- [ ] Default floating pane size: 60% of tab width, 60% of tab height, centered
- [ ] Minimum floating pane size: 20 columns × 5 rows (computed from cell size at resolve time)
- [ ] Snap-to-edge when dragged within 10px of tab boundary

**Tests:**
- [ ] Add floating pane: appears in layer, `contains` returns true
- [ ] Remove floating pane: `contains` returns false, other panes unaffected
- [ ] `hit_test`: returns topmost pane at overlap point
- [ ] `hit_test`: returns `None` outside all floating panes
- [ ] `raise`: pane moves to highest z_order
- [ ] `move_pane`: updates position, clamps to tab bounds
- [ ] `resize_pane`: updates dimensions, enforces minimum size

---

## 29.4 Layout Computation

Convert the abstract `SplitTree` + `FloatingLayer` into concrete pixel rectangles for rendering and PTY resize. This is the bridge between the mux data model and the GPU renderer.

**File:** `oriterm_mux/src/layout/compute.rs`

- [ ] `LayoutDescriptor` — input to layout computation:
  ```rust
  pub struct LayoutDescriptor {
      /// Total available pixel area for the tab content (excludes tab bar).
      pub available: Rect,
      /// Cell dimensions for converting pixels to columns/rows.
      pub cell_width: f32,
      pub cell_height: f32,
      /// Divider thickness in logical pixels.
      pub divider_px: f32,
      /// Minimum pane size in cells (width, height).
      pub min_pane_cells: (u16, u16),
  }
  ```
- [ ] `PaneLayout` — output per pane:
  ```rust
  pub struct PaneLayout {
      pub pane_id: PaneId,
      pub pixel_rect: Rect,
      pub cols: u16,
      pub rows: u16,
      pub is_focused: bool,
      pub is_floating: bool,
  }
  ```
- [ ] `compute_layout(tree: &SplitTree, floating: &FloatingLayer, focused: PaneId, desc: &LayoutDescriptor) -> Vec<PaneLayout>`
  - [ ] Recursively subdivide `desc.available` according to `SplitTree` splits and ratios
  - [ ] Subtract `desc.divider_px` between split children
  - [ ] Snap pane boundaries to cell grid (no partial cells)
  - [ ] Convert pixel dimensions to `cols` / `rows` using cell size
  - [ ] Append floating pane layouts (overlaid on top of tiled layouts)
  - [ ] Set `is_focused` on the pane matching `focused`
  - [ ] Enforce `min_pane_cells`: if a split produces a pane smaller than minimum, clamp ratio
- [ ] `DividerLayout` — output for divider rendering:
  ```rust
  pub struct DividerLayout {
      pub rect: Rect,
      pub direction: SplitDirection,
      /// The two pane IDs on either side (for drag resize targeting).
      pub pane_before: PaneId,
      pub pane_after: PaneId,
  }
  ```
- [ ] `compute_dividers(tree: &SplitTree, desc: &LayoutDescriptor) -> Vec<DividerLayout>`
  - [ ] One divider per internal `Split` node
  - [ ] Divider rect: full span of the split in the perpendicular direction, `divider_px` thick
- [ ] `Rect` type (if not already in `oriterm_ui`):
  - [ ] `x: f32, y: f32, width: f32, height: f32`
  - [ ] `contains(px: f32, py: f32) -> bool`
  - [ ] `intersects(other: &Rect) -> bool`

**Tests:**
- [ ] Single pane: layout fills entire available rect
- [ ] Horizontal split 50/50: two rects stacked vertically, divider between
- [ ] Vertical split 70/30: two rects side by side with correct proportions
- [ ] Nested splits: 3-pane L-shape layout produces correct rects
- [ ] Cell grid snapping: pixel rects align to cell boundaries
- [ ] Divider computation: correct position and neighbors for each divider
- [ ] Minimum pane size enforcement: ratio clamped when split would produce tiny pane
- [ ] Floating panes: appear in layout with correct pixel rects, `is_floating == true`
- [ ] Layout is deterministic: same inputs always produce same outputs

---

## 29.5 Spatial Navigation

Navigate between panes using directional movement (up/down/left/right) and sequential cycling. This must work identically for tiled and floating panes.

**File:** `oriterm_mux/src/nav.rs`

**Reference:** Ghostty `src/input/navigate.zig`, Zellij `zellij-server/src/panes/tiled_panes/mod.rs` (directional_move)

- [ ] `navigate(layouts: &[PaneLayout], from: PaneId, direction: Direction) -> Option<PaneId>`
  - [ ] `Direction` enum: `Up`, `Down`, `Left`, `Right`
  - [ ] Algorithm: from the center of `from`'s rect, cast a ray in `direction`. Find the nearest pane whose rect intersects the ray (or is closest to the ray in the perpendicular axis).
  - [ ] Floating panes participate in navigation (if visible)
  - [ ] Returns `None` if no pane exists in that direction
- [ ] `cycle(layouts: &[PaneLayout], from: PaneId, forward: bool) -> Option<PaneId>`
  - [ ] Cycle through panes in layout order (tiled depth-first, then floating by z_order)
  - [ ] Wraps around: last pane → first pane (forward), first → last (backward)
- [ ] `nearest_pane(layouts: &[PaneLayout], x: f32, y: f32) -> Option<PaneId>`
  - [ ] Find the pane whose rect contains the point, preferring floating panes (higher z_order)
  - [ ] Used for mouse click → focus

**Tests:**
- [ ] 2x2 grid: navigate right from top-left → top-right
- [ ] 2x2 grid: navigate down from top-left → bottom-left
- [ ] Navigation wraps: navigate right from rightmost pane → `None`
- [ ] Cycle forward: visits panes in order, wraps to first
- [ ] Cycle backward: reverse order, wraps to last
- [ ] Floating pane: `nearest_pane` prefers floating over tiled at overlap point
- [ ] Navigate from tiled to floating pane in correct direction

---

## 29.6 Section Completion

- [ ] All 29.1–29.5 items complete
- [ ] `oriterm_mux` crate compiles with `cargo build -p oriterm_mux`
- [ ] `cargo clippy -p oriterm_mux` — no warnings
- [ ] `cargo test -p oriterm_mux` — all tests pass
- [ ] Newtype IDs: `PaneId`, `TabId`, `WindowId`, `SessionId` with Display, Hash, Eq
- [ ] `SplitTree`: immutable, structural sharing, all mutation methods return new trees
- [ ] `FloatingLayer`: immutable, z-ordered, hit-testing
- [ ] `compute_layout`: pixel rects snapped to cell grid, dividers, minimum pane enforcement
- [ ] Spatial navigation: directional + cycling, works for tiled and floating
- [ ] Zero dependencies on `oriterm_core` or `oriterm` — pure standalone crate
- [ ] No `unsafe` code

**Exit Criteria:** `oriterm_mux` is a standalone crate with a complete layout engine. SplitTree and FloatingLayer are immutable data structures with full test coverage. Layout computation converts abstract trees into concrete pixel rects. Spatial navigation works for any pane arrangement. The crate compiles and tests pass independently.
