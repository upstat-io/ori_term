---
section: 31
title: In-Process Mux + Multi-Pane Rendering
status: in-progress
tier: 4M
goal: Wire up InProcessMux, rewire App to use mux layer, render multiple panes per tab with correct viewport offsets and dividers
sections:
  - id: "31.1"
    title: InProcessMux
    status: complete
  - id: "31.2"
    title: App Rewiring
    status: complete
  - id: "31.3"
    title: Multi-Pane Rendering
    status: complete
  - id: "31.4"
    title: PaneRenderCache
    status: not-started
  - id: "31.5"
    title: Section Completion
    status: not-started
---

# Section 31: In-Process Mux + Multi-Pane Rendering

**Status:** In Progress
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

**File:** `oriterm/src/mux/mod.rs` (lives in binary crate, bridges pure-data `oriterm_mux` to I/O)

**Reference:** WezTerm `mux/src/lib.rs` (Mux struct, get/set pattern)

- [x] `InProcessMux` struct:
  - [x] `pane_registry: PaneRegistry`
  - [x] `session: SessionRegistry`
  - [x] `local_domain: LocalDomain` (concrete; extended to domain registry in Section 35)
  - [x] `default_domain` via `local_domain.id()`
  - [x] `pane_alloc: IdAllocator<PaneId>`
  - [x] `tab_alloc: IdAllocator<TabId>`
  - [x] `window_alloc: IdAllocator<WindowId>`
  - [x] `notifications: Vec<MuxNotification>` + `drain_notifications()` double-buffer pattern
  - [x] `event_rx: mpsc::Receiver<MuxEvent>` — incoming events from pane reader threads
- [x] Pane operations:
  - [x] `spawn_pane(&mut self, tab_id, config, theme, winit_proxy) -> Result<(PaneId, Pane)>`
    - [x] Delegate to `LocalDomain::spawn_pane`
    - [x] Register in `PaneRegistry` with `tab_id`
    - [x] Emit `MuxNotification::TabLayoutChanged(tab_id)`
  - [x] `close_pane(&mut self, pane_id) -> ClosePaneResult`
    - [x] Remove from `PaneRegistry`
    - [x] Update `SplitTree` in the owning `MuxTab` (immutable remove)
    - [x] If last pane in tab: close tab
    - [x] Emit notifications
    - [x] **Background thread drop** via Pane's Drop impl
  - [x] `get_pane_entry(&self, pane_id) -> Option<&PaneEntry>`
- [x] Tab operations:
  - [x] `create_tab(&mut self, window_id, config, theme, winit_proxy) -> Result<(TabId, PaneId, Pane)>`
    - [x] Allocate TabId
    - [x] Spawn initial pane via domain
    - [x] Create `MuxTab` with `SplitTree::Leaf(pane_id)`
    - [x] Add tab to `MuxWindow`
    - [x] Emit `MuxNotification::WindowTabsChanged(window_id)`
  - [x] `close_tab(&mut self, tab_id) -> Vec<PaneId>` — close all panes, remove from window
  - [x] `split_pane(&mut self, tab_id, source_pane, direction, config, theme, winit_proxy) -> Result<(PaneId, Pane)>`
    - [x] Spawn new pane
    - [x] Update `MuxTab.tree` via immutable `split_at`
    - [x] Push old tree to `tree_history` (undo stack via `set_tree`)
    - [x] Emit layout change notification
- [x] Window operations:
  - [x] `create_window(&mut self) -> WindowId`
  - [x] `close_window(&mut self, window_id) -> Vec<PaneId>` — close all tabs/panes
- [x] Event pump:
  - [x] `poll_events(&mut self, panes: &mut HashMap<PaneId, Pane>)` — drain `event_rx`, process each `MuxEvent`:
    - [x] `PaneOutput(id)` → clear wakeup, emit `MuxNotification::PaneDirty(id)`
    - [x] `PaneExited(id)` → call `close_pane(id)`
    - [x] `PaneTitleChanged(id, title)` → update `Pane.title`, emit notification
    - [x] `PaneBell(id)` → set bell, emit `MuxNotification::Alert`
    - [x] `PtyWrite` → forward to pane's PTY
    - [x] `ClipboardStore` / `ClipboardLoad` → forward as notifications
  - [ ] Called from `App::about_to_wait()` on every event loop iteration *(wired in 31.2)*

**Tests:**
- [x] `create_tab` → produces valid TabId, window contains tab, tab contains one pane
- [x] `split_pane` → tab now has two panes, tree is `Split`
- [x] `close_pane` → tree collapses, remaining pane is `Leaf`
- [x] `close_pane` on last pane → tab closed → window updated
- [x] Event pump: `PaneExited` triggers `close_pane`
- [x] Notification channel: all mutations emit correct notifications (51 tests passing)

---

## 31.2 App Rewiring

Rewire the `App` struct to use `InProcessMux` as the source of truth for all pane/tab/window state. The App becomes a thin GUI shell that forwards input and renders output.

**File:** `oriterm/src/app/mod.rs`

- [x] Add `InProcessMux` field to `App`:
  - [x] `mux: Option<InProcessMux>` — owns all mux state (`app/mod.rs`)
  - [x] Remove direct `tab: Option<Tab>` field (replaced by mux + pane store)
  - [x] `panes: HashMap<PaneId, Pane>` — the actual Pane structs (owned by App, tracked by mux)
  - [x] `active_window: Option<MuxWindowId>` — maps to the single TermWindow
  - [x] `notification_buf: Vec<MuxNotification>` — double-buffer for pump
  - [x] `active_pane_id() -> Option<PaneId>` — derives active pane from mux session model
  - [x] `active_pane()` / `active_pane_mut()` — convenience accessors
- [x] Rewire `about_to_wait`:
  - [x] `pump_mux_events()` in `app/mux_pump.rs` — called before rendering
  - [x] `mux.poll_events(&mut self.panes)` — process all pending MuxEvents
  - [x] `mux.drain_notifications()` — handle each `MuxNotification`:
    - [x] `PaneDirty(id)` → check selection invalidation, invalidate URL cache, mark dirty
    - [x] `PaneClosed(id)` → remove from `panes`, mark dirty
    - [x] `TabLayoutChanged(id)` → mark dirty (layout recompute in 31.3)
    - [x] `WindowTabsChanged(id)` → sync tab bar titles, mark dirty
    - [x] `Alert(id)` → set bell on pane, ring bell on tab bar
    - [x] `LastWindowClosed` → exit event loop
    - [x] `ClipboardStore/Load` → forward to clipboard system
- [x] Rewire all `self.tab` references to `self.active_pane()` / `self.active_pane_mut()`:
  - [x] `app/mod.rs` — handle_dpi_change, handle_theme_changed, terminal_mode, sync_tab_bar_titles, handle_terminal_event
  - [x] `app/search_ui.rs` — all search operations
  - [x] `app/redraw.rs` — frame extraction
  - [x] `app/keyboard_input/mod.rs` — key dispatch, mark mode
  - [x] `app/mouse_report/mod.rs` — PTY mouse reporting
  - [x] `app/chrome/mod.rs` — chrome hit testing
  - [x] `app/config_reload.rs` — palette/resize
  - [x] `app/clipboard_ops/mod.rs` — copy/paste
  - [x] `app/mouse_selection/mod.rs` — selection lifecycle
  - [x] `app/mouse_input.rs` — press/drag/release (split-borrow pattern)
  - [x] `app/cursor_hover.rs` — URL hover detection
  - [x] `app/mark_mode/mod.rs` — Tab→Pane parameter types
- [x] Remove old Tab infrastructure:
  - [x] Removed `tab: Option<Tab>` field from App
  - [x] Removed `create_initial_tab()` from init
  - [x] Removed `use crate::tab::Tab` imports
  - [x] Added `#[allow(dead_code)]` to old Tab type (full removal deferred)
  - [x] Removed `#[allow(dead_code)]` from actively-used mux/pane types
- [x] Single-pane compatibility:
  - [x] A tab with one pane renders identically — `active_pane()` resolves through mux session model
  - [x] No layout overhead when `SplitTree` is `Leaf` — single pane path is unchanged

**Implementation notes:**
- Split-borrow pattern: `active_pane_id()` returns `Option<PaneId>` (Copy), then `self.panes.get_mut(&id)` avoids borrowing all of `self`
- `extract_frame_into<T: EventListener>` is generic, so `Term<MuxEventProxy>` works seamlessly
- Old `Tab` type retained with `#[allow(dead_code)]` — full removal when EventProxy is retired
- `app/mod.rs` at 634 lines (over 500 limit) — pre-existing; will drop when old EventProxy handler is removed

**Tests:**
- [x] All 3696 existing tests pass (1507 oriterm + 1191 oriterm_core + 172 oriterm_mux + 826 oriterm_ui)
- [x] mark_mode tests updated to use `make_pane` (LocalDomain + MuxEventProxy) instead of `make_tab`
- [ ] Tab creation via mux: new tab appears, tab bar updates *(deferred to 31.3)*
- [ ] Multi-pane: split creates visible second pane *(deferred to 31.3)*

---

## 31.3 Multi-Pane Rendering

Render multiple panes per tab, each with its own viewport offset. The key change: `prepare_pane_into()` takes an origin offset so instances are positioned correctly within the overall frame.

**File:** `oriterm/src/gpu/prepare/mod.rs` (prepare_pane_into), `oriterm/src/gpu/renderer.rs` (frame loop)

**Reference:** Existing `prepare_frame_into` (already takes `FrameInput` with viewport)

- [x] `fill_frame_shaped` made `pub(crate)` for multi-pane direct calls
- [x] `fg_dim: f32` field added to `FrameInput` for inactive pane dimming
  - [x] Threaded through `GlyphEmitter` and all glyph push calls (shaped + unshaped paths)
  - [x] Default 1.0 in extract and test_grid constructors
- [x] `GpuRenderer::prepare_pane()` — shapes, caches, and fills one pane (appends to PreparedFrame)
  - [x] Each pane's instances offset by origin `(pixel_rect.x, pixel_rect.y)`
  - [x] Cursor instances also offset
- [x] Multi-pane frame loop in `App::handle_redraw_multi_pane()`:
  - [x] `compute_pane_layouts()` → `compute_all(tree, floating, focused, desc)` → `(Vec<PaneLayout>, Vec<DividerLayout>)`
  - [x] `begin_multi_pane_frame()` → clear PreparedFrame, set viewport
  - [x] For each `PaneLayout`: extract_frame_into → prepare_pane at layout origin
  - [x] After all panes: append dividers + focus border
  - [x] Single pane optimization: `compute_pane_layouts()` returns `None`, existing fast path unchanged
- [x] Divider rendering:
  - [x] `append_dividers()` pushes background rect instances for each `DividerLayout`
  - [x] Divider color: `Rgb(80, 80, 80)` (subtle contrast)
- [x] Focus border:
  - [x] `append_focus_border()` — 2px border (4 cursor-layer rects) around focused pane
  - [x] Color: cornflower blue `Rgb(100, 149, 237)`
  - [x] Only shown when `layouts.len() > 1`
- [x] Inactive pane dimming (config-controlled):
  - [x] `PaneConfig` struct: `dim_inactive`, `inactive_opacity`, `divider_px`, `min_cells`
  - [x] `fg_dim` set to `inactive_opacity` for unfocused panes when `dim_inactive` enabled
- [x] `app/redraw.rs` → `app/redraw/mod.rs` directory module
  - [x] `app/redraw/multi_pane.rs` — `compute_pane_layouts()` + `handle_redraw_multi_pane()`
  - [x] Branching in `handle_redraw()`: multi-pane path dispatches via early return

**Tests:**
- [x] `fg_dim_default_alpha_is_one` — default 1.0 produces alpha 1.0
- [x] `fg_dim_reduces_glyph_alpha` — fg_dim=0.7 produces alpha ~0.7
- [x] `fill_frame_shaped_accumulates_without_clearing` — two fills accumulate instances
- [x] `two_panes_at_correct_offsets` — pane B bg at x=400
- [x] `cursor_only_in_focused_pane` — cursor_blink_visible=false suppresses cursor
- [x] `pane_config_defaults` / `pane_config_roundtrip` / `pane_config_partial_toml` — config serialization
- [x] `pane_config_effective_opacity_clamps` / `pane_config_effective_opacity_nan_defaults` — opacity validation

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
