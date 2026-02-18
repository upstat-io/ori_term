---
section: 31
title: In-Process Mux + Multi-Pane Rendering
status: not-started
tier: 4M
goal: Wire up InProcessMux, rewire App to use mux layer, render multiple panes per tab with correct viewport offsets and dividers
sections:
  - id: "31.1"
    title: InProcessMux
    status: not-started
  - id: "31.2"
    title: App Rewiring
    status: not-started
  - id: "31.3"
    title: Multi-Pane Rendering
    status: not-started
  - id: "31.4"
    title: PaneRenderCache
    status: not-started
  - id: "31.5"
    title: Section Completion
    status: not-started
---

# Section 31: In-Process Mux + Multi-Pane Rendering

**Status:** Not Started
**Goal:** Create the `InProcessMux` that runs all mux logic in the same process (no daemon). Rewire `App` to route all pane/tab/window operations through the mux. Implement multi-pane rendering with per-pane viewport offsets, dividers, and focus borders.

**Crate:** `oriterm_mux` (InProcessMux), `oriterm` (App rewiring, rendering)
**Dependencies:** Section 29 (layout engine), Section 30 (Pane, Domain, registries), Section 05 (GPU rendering)
**Prerequisite:** Sections 29 and 30 complete.

**Inspired by:**
- WezTerm: in-process `Mux` singleton with notification channels
- Ghostty: per-surface rendering with viewport offsets
- Alacritty: `prepare_frame_into` with offset parameters (already exists in our codebase)

**Key constraint:** After this section, the single-pane path must still work identically — a tab with one pane renders exactly as before. Multi-pane is additive.

---

## 31.1 InProcessMux

The in-process mux is the synchronous fast path — all mux operations happen on the main thread via direct method calls. No IPC, no serialization, no daemon. This is the default mode; the daemon (Section 34) layers on top later.

**File:** `oriterm_mux/src/mux.rs`

**Reference:** WezTerm `mux/src/lib.rs` (Mux struct, get/set pattern)

- [ ] `InProcessMux` struct:
  - [ ] `pane_registry: PaneRegistry`
  - [ ] `session: SessionRegistry`
  - [ ] `domains: Vec<Box<dyn Domain>>`
  - [ ] `default_domain: DomainId`
  - [ ] `pane_allocator: IdAllocator` — for PaneId
  - [ ] `tab_allocator: IdAllocator` — for TabId
  - [ ] `window_allocator: IdAllocator` — for WindowId
  - [ ] `notification_tx: mpsc::Sender<MuxNotification>`
  - [ ] `notification_rx: mpsc::Receiver<MuxNotification>` — consumed by GUI
  - [ ] `event_rx: mpsc::Receiver<MuxEvent>` — incoming events from pane reader threads
- [ ] Pane operations:
  - [ ] `spawn_pane(&mut self, tab_id: TabId, config: SpawnConfig) -> Result<PaneId>`
    - [ ] Delegate to domain's `spawn_pane`
    - [ ] Register in `PaneRegistry` with `tab_id`
    - [ ] Emit `MuxNotification::TabLayoutChanged(tab_id)`
  - [ ] `close_pane(&mut self, pane_id: PaneId)`
    - [ ] Remove from `PaneRegistry`
    - [ ] Update `SplitTree` in the owning `MuxTab` (immutable remove)
    - [ ] If last pane in tab: close tab
    - [ ] Emit notifications
    - [ ] **Background thread drop** for ConPTY safety
  - [ ] `get_pane_entry(&self, pane_id: PaneId) -> Option<&PaneEntry>`
- [ ] Tab operations:
  - [ ] `create_tab(&mut self, window_id: WindowId, config: SpawnConfig) -> Result<TabId>`
    - [ ] Allocate TabId
    - [ ] Spawn initial pane via domain
    - [ ] Create `MuxTab` with `SplitTree::Leaf(pane_id)`
    - [ ] Add tab to `MuxWindow`
    - [ ] Emit `MuxNotification::WindowTabsChanged(window_id)`
  - [ ] `close_tab(&mut self, tab_id: TabId)` — close all panes, remove from window
  - [ ] `split_pane(&mut self, tab_id: TabId, pane_id: PaneId, dir: SplitDirection, config: SpawnConfig) -> Result<PaneId>`
    - [ ] Spawn new pane (inherits CWD from source pane)
    - [ ] Update `MuxTab.tree` via immutable `split_at`
    - [ ] Push old tree to `tree_history` (undo stack)
    - [ ] Emit layout change notification
- [ ] Window operations:
  - [ ] `create_window(&mut self) -> WindowId`
  - [ ] `close_window(&mut self, window_id: WindowId)` — close all tabs/panes
- [ ] Event pump:
  - [ ] `poll_events(&mut self)` — drain `event_rx`, process each `MuxEvent`:
    - [ ] `PaneOutput(id)` → mark pane dirty, emit `MuxNotification::PaneDirty(id)`
    - [ ] `PaneExited(id)` → call `close_pane(id)`
    - [ ] `PaneTitleChanged(id, title)` → update `PaneEntry.title`, emit notification
    - [ ] `PaneBell(id)` → emit `MuxNotification::Alert`
  - [ ] Called from `App::about_to_wait()` on every event loop iteration

**Tests:**
- [ ] `create_tab` → produces valid TabId, window contains tab, tab contains one pane
- [ ] `split_pane` → tab now has two panes, tree is `Split`
- [ ] `close_pane` → tree collapses, remaining pane is `Leaf`
- [ ] `close_pane` on last pane → tab closed → window updated
- [ ] Event pump: `PaneExited` triggers `close_pane`
- [ ] Notification channel: all mutations emit correct notifications

---

## 31.2 App Rewiring

Rewire the `App` struct to use `InProcessMux` as the source of truth for all pane/tab/window state. The App becomes a thin GUI shell that forwards input and renders output.

**File:** `oriterm/src/app/mod.rs`

- [ ] Add `InProcessMux` field to `App`:
  - [ ] `mux: InProcessMux` — owns all mux state
  - [ ] Remove direct `HashMap<TabId, Tab>` (now managed by mux + pane store)
  - [ ] `panes: HashMap<PaneId, Pane>` — the actual Pane structs (owned by App, tracked by mux)
- [ ] Rewire `about_to_wait`:
  - [ ] Call `mux.poll_events()` — process all pending MuxEvents
  - [ ] Drain `mux.notification_rx` — handle each `MuxNotification`:
    - [ ] `PaneDirty(id)` → mark window containing pane for redraw
    - [ ] `PaneClosed(id)` → drop Pane on background thread, remove from `panes`
    - [ ] `TabLayoutChanged(id)` → recompute layout, resize affected panes
    - [ ] `WindowTabsChanged(id)` → update tab bar
- [ ] Rewire input dispatch:
  - [ ] Keyboard input → `panes[active_pane].send_pty(bytes)`
  - [ ] Mouse click on pane → `mux.session.get_tab_mut(tab_id).active_pane = clicked_pane_id`
  - [ ] Split keybind → `mux.split_pane(tab_id, active_pane_id, direction, config)`
- [ ] Rewire tab operations:
  - [ ] New tab → `mux.create_tab(window_id, config)`
  - [ ] Close tab → `mux.close_tab(tab_id)`
  - [ ] Cycle tab → update `MuxWindow.active_tab` via mux
- [ ] Single-pane compatibility:
  - [ ] A tab with one pane must render identically to the current single-pane path
  - [ ] No layout overhead when `SplitTree` is `Leaf` — fast path

**Tests:**
- [ ] App creates mux on startup, spawns initial tab with one pane
- [ ] Input dispatch: keyboard bytes reach the active pane's PTY
- [ ] Tab creation via mux: new tab appears, tab bar updates
- [ ] Single-pane rendering: identical output before and after rewiring
- [ ] Multi-pane: split creates visible second pane

---

## 31.3 Multi-Pane Rendering

Render multiple panes per tab, each with its own viewport offset. The key change: `prepare_pane_into()` takes an origin offset so instances are positioned correctly within the overall frame.

**File:** `oriterm/src/gpu/prepare/mod.rs` (prepare_pane_into), `oriterm/src/gpu/renderer.rs` (frame loop)

**Reference:** Existing `prepare_frame_into` (already takes `FrameInput` with viewport)

- [ ] `prepare_pane_into(input: &FrameInput, atlas: &dyn AtlasLookup, origin: (f32, f32), out: &mut PreparedFrame)`
  - [ ] Same as `prepare_frame_into` but adds `origin.0` and `origin.1` to all x/y coordinates
  - [ ] Each pane's instances are offset to their pixel rect within the tab area
  - [ ] Cursor instances also offset
- [ ] Multi-pane frame loop in `GpuRenderer::draw_frame()`:
  - [ ] Compute layout: `compute_layout(tree, floating, focused, desc)` → `Vec<PaneLayout>`
  - [ ] For each `PaneLayout`:
    1. Lock pane's terminal, extract `FrameInput` snapshot
    2. Call `prepare_pane_into(input, atlas, (layout.pixel_rect.x, layout.pixel_rect.y), frame)`
    3. Release lock
  - [ ] After all panes: render dividers as filled rectangles (palette surface color)
  - [ ] Render focus border: 2px accent color around the focused pane's rect
  - [ ] Single pane optimization: skip layout computation, use existing `prepare_frame_into` directly
- [ ] Divider rendering:
  - [ ] `compute_dividers()` produces `Vec<DividerLayout>`
  - [ ] Each divider: push a background rect instance with divider color
  - [ ] Divider color: subtle contrast from background (e.g., `palette.surface` or 20% lighter)
- [ ] Focus border:
  - [ ] 2px border around the focused pane's pixel rect
  - [ ] Color: `palette.accent` or configurable
  - [ ] Only shown when tab has more than one pane
- [ ] Inactive pane dimming (optional, config-controlled):
  - [ ] Multiply foreground alpha by 0.7 for unfocused panes
  - [ ] Applied during `prepare_pane_into` based on `is_focused` flag

**Tests:**
- [ ] Single pane: output identical to non-mux path (regression test)
- [ ] Two panes: instances for each pane at correct offsets
- [ ] Dividers: correct position and dimensions between panes
- [ ] Focus border: surrounds only the focused pane
- [ ] Inactive dimming: unfocused pane glyphs have reduced alpha
- [ ] Cursor: appears only in focused pane at correct offset position

---

## 31.4 PaneRenderCache

Per-pane `PreparedFrame` caching to avoid re-preparing unchanged panes on every frame. Only dirty panes get re-prepared; clean panes reuse their cached instances.

**File:** `oriterm/src/gpu/pane_cache.rs`

- [ ] `PaneRenderCache`:
  - [ ] `HashMap<PaneId, CachedPaneFrame>`
  - [ ] `CachedPaneFrame`:
    - [ ] `prepared: PreparedFrame` — cached GPU-ready instances for this pane
    - [ ] `layout: PaneLayout` — layout at time of preparation (for invalidation on resize)
    - [ ] `generation: u64` — incremented on each prepare, for staleness detection
  - [ ] `get_or_prepare(pane_id: PaneId, layout: &PaneLayout, dirty: bool, prepare_fn: F) -> &PreparedFrame`
    - [ ] If `dirty == false` and layout unchanged: return cached `PreparedFrame`
    - [ ] Otherwise: call `prepare_fn`, store result, increment generation
  - [ ] `invalidate(pane_id: PaneId)` — force re-prepare on next frame
  - [ ] `remove(pane_id: PaneId)` — pane closed, free memory
  - [ ] `invalidate_all()` — atlas rebuild, font change, etc.
- [ ] Integration with frame loop:
  - [ ] Check `pane.grid_dirty()` for each pane
  - [ ] Only lock terminal and call `prepare_pane_into` if dirty or layout changed
  - [ ] Merge all cached `PreparedFrame`s into the final frame buffer
- [ ] Memory: one `PreparedFrame` per pane (~307KB for 80×24). For 10 panes: ~3MB. Acceptable.

**Tests:**
- [ ] Clean pane: `get_or_prepare` returns cached frame, `prepare_fn` NOT called
- [ ] Dirty pane: `get_or_prepare` calls `prepare_fn`, updates cache
- [ ] Layout change: triggers re-prepare even if not dirty
- [ ] `invalidate_all`: forces all panes to re-prepare
- [ ] `remove`: frees memory for closed pane

---

## 31.5 Section Completion

- [ ] All 31.1–31.4 items complete
- [ ] `InProcessMux` handles all pane/tab/window CRUD with correct notification flow
- [ ] App rewired: all state management goes through mux, no direct pane access for mutations
- [ ] Multi-pane rendering: each pane at correct offset, dividers between, focus border on active
- [ ] `PaneRenderCache`: only dirty panes re-prepared, clean panes cached
- [ ] Single-pane fast path: zero overhead compared to pre-mux rendering
- [ ] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test` — all existing tests pass (no regression)
- [ ] **Visual test**: split pane shows two independent terminal grids
- [ ] **Performance test**: frame time with 4 panes < 2× single-pane frame time

**Exit Criteria:** The mux layer is fully wired into the App. Multiple panes render correctly with proper offsets, dividers, and focus borders. Cached rendering prevents unnecessary GPU work. The single-pane case has zero overhead. All existing functionality works unchanged.
