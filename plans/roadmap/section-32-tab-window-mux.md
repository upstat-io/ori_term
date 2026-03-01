---
section: 32
title: Tab & Window Management (Mux-Aware)
status: complete
tier: 4M
goal: Multi-tab with mux integration, multi-window with shared GPU, tab CRUD, window lifecycle, cross-window tab movement, ConPTY-safe shutdown
sections:
  - id: "32.1"
    title: Mux-Aware Tab Management
    status: complete
  - id: "32.2"
    title: Multi-Window + Shared GPU
    status: complete
  - id: "32.3"
    title: Window Lifecycle
    status: complete
  - id: "32.4"
    title: Cross-Window Operations
    status: complete
  - id: "32.5"
    title: Section Completion
    status: complete
---

# Section 32: Tab & Window Management (Mux-Aware)

**Status:** In Progress
**Goal:** Full tab and window management built on the mux layer. Multiple tabs per window, multiple windows with shared GPU device. Tab CRUD, window lifecycle with no-flash startup, DPI handling, ConPTY-safe cleanup. Cross-window tab movement preserving pane state.

**Crate:** `oriterm` (App, TermWindow), `oriterm_mux` (MuxTab, MuxWindow)
**Dependencies:** Section 31 (InProcessMux wired into App)
**Prerequisite:** Section 31 complete.

**Absorbs:** Section 15.2 (Tab Management Operations) and Section 18 (Multi-Window & Window Lifecycle). All hard-won patterns preserved: ConPTY-safe shutdown ordering, exit-before-drop, no-flash window creation, DPI handling, Aero Snap, background thread cleanup, CWD inheritance.

---

## 32.1 Mux-Aware Tab Management

Tab CRUD operations that go through the mux layer. The mux owns tab state (MuxTab with SplitTree); the GUI owns rendering state (tab bar layout, animation offsets).

**File:** `oriterm/src/app/tab_management.rs`

**Reference:** `_old/src/app/tab_management.rs`, Section 15.2 design (preserved patterns)

- [x] New tab:
  - [x] `App::new_tab_in_window(&mut self, window_id: WindowId)`
  - [x] Inherit CWD from active pane in current tab (via `pane.cwd()`)
  - [x] Build `SpawnConfig` with shell, scrollback, cursor shape from config
  - [x] Call `mux.create_tab(mux_window_id, config)` — creates MuxTab with one Leaf pane
  - [x] Map mux `TabId` → GUI tab bar entry
  - [x] Clear `tab_width_lock` (tab count changed)
  - [x] Mark `tab_bar_dirty`, request redraw
- [x] Close tab:
  - [x] `App::close_tab(&mut self, tab_id: TabId)`
  - [x] Call `mux.close_tab(tab_id)` — closes all panes in tab, updates MuxWindow
  - [x] If window now empty and last terminal window: call `shutdown()` **immediately** (ConPTY)
  - [x] If window now empty but other windows exist: close the empty window
  - [x] Background thread drops for all Pane structs
  - [x] Mark `tab_bar_dirty`
- [x] Duplicate tab:
  - [x] `App::duplicate_active_tab(&mut self)`
  - [x] Clone CWD from source tab's active pane
  - [x] Create new tab via mux (fresh shell, inherited directory)
- [x] Cycle tabs:
  - [x] `App::cycle_tab(&mut self, delta: isize)`
  - [x] Update `MuxWindow.active_tab` via mux: wrapping arithmetic
  - [x] Clear bell badge on newly active tab
  - [x] Mark dirty, request redraw
- [x] Switch to specific tab:
  - [x] `App::switch_to_tab(&mut self, tab_id: TabId)` — find window, set active
- [x] Reorder tabs:
  - [x] `App::move_tab(&mut self, from: usize, to: usize)` (wired to drag in Section 17)
  - [x] Update `MuxWindow.tabs` vec order via mux
  - [x] Adjust `active_tab` index to track the same tab
- [x] Auto-close on PTY exit:
  - [x] `MuxEvent::PaneExited` → `close_pane` → tab auto-removed if last pane → `WindowTabsChanged`/`LastWindowClosed`

**Tests:**
- [x] Create 3 tabs: IDs are unique, window contains all 3
- [x] Close middle tab: remaining tabs order preserved, active_tab adjusts
- [x] Cycle wrap: tab 2 of 3 → next → tab 0
- [x] CWD inheritance: new tab starts in active pane's directory (via CWD in SpawnConfig)
- [x] Closing last tab in last window triggers `shutdown()`
- [x] Pane drop on background thread (via `std::thread::spawn(move || drop(pane))`)

---

## 32.2 Multi-Window + Shared GPU

Multiple windows, each a thin GUI shell. All windows share the same GPU device, font collection, and config. The mux tracks window state; the GUI maps `winit::window::WindowId` to `oriterm_mux::WindowId`.

**File:** `oriterm/src/app/window_management.rs`, `oriterm/src/window.rs`

**Reference:** `_old/src/app/window_management.rs`, Section 18.1 design (preserved patterns)

- [x] `TermWindow` struct (GUI-level window):
  - [x] `winit_window: Arc<Window>` — winit window handle
  - [x] `surface: wgpu::Surface<'static>` — GPU surface
  - [x] `surface_config: wgpu::SurfaceConfiguration`
  - [x] `mux_window_id: WindowId` — link to mux MuxWindow
  - [x] `is_maximized: bool`
  - [x] `scale_factor: f64` — current DPI scale
- [x] Window ID mapping:
  - [x] `App::windows: HashMap<winit::window::WindowId, TermWindow>` — maps winit ID → TermWindow (which contains mux_window_id)
  - [x] `App::focused_window_id: Option<winit::window::WindowId>` — tracks focused OS window
- [x] `TermWindow` methods:
  - [x] `resize_surface(&mut self, width: u32, height: u32, gpu: &GpuState)` — reconfigure surface
- [x] Shared resources across windows:
  - [x] `GpuState` (device, queue, adapter) — created once, shared
  - [x] `FontCollection` — created once, shared (rebuilt on DPI change)
  - [x] `GlyphAtlas` — created once, shared across windows
  - [x] Config — single source of truth
- [x] Focus tracking:
  - [x] `WindowEvent::Focused(true)` → send focus-in to active pane's terminal (if `FOCUS_IN_OUT` mode)
  - [x] `WindowEvent::Focused(false)` → send focus-out

**Tests:**
- [x] Create two windows: both share same GPU device (verified by architecture: `create_window` reuses `self.gpu`)
- [x] Focus tracking: mode gating and multi-window session tests verify focus event dispatch
- [x] Window ID mapping: multi-window session tests verify mux ID → pane resolution per window

---

## 32.3 Window Lifecycle

Window creation, resize, DPI changes, and destruction. All operations coordinated with the mux.

**File:** `oriterm/src/app/window_management.rs`

**Reference:** Section 18.2 design (all patterns preserved)

- [x] `create_window(&mut self, event_loop: &ActiveEventLoop) -> Option<WindowId>`
  - [x] Calculate window size from font metrics + grid dimensions + `TAB_BAR_HEIGHT`
  - [x] Request transparency if opacity < 1.0
  - [x] Enable `WS_EX_NOREDIRECTIONBITMAP` on Windows
  - [x] Create winit window
  - [x] Capture initial DPI scale factor
  - [x] **First window only**: initialize `GpuState` and `GpuRenderer` (via `try_init`)
  - [x] Create wgpu `Surface` for this window
  - [x] **Render clear frame BEFORE showing** (prevent gray/white flash)
  - [x] Apply compositor effects (Mica/acrylic on Windows, vibrancy on macOS)
  - [x] Enable Aero Snap on Windows (WndProc subclass for `WM_NCHITTEST`)
  - [x] Register mux window: `mux.create_window()` → `WindowId`
  - [x] Map winit `WindowId` ↔ mux `WindowId`
  - [x] Show window
- [x] `handle_resize(&mut self, winit_id: WindowId, size: PhysicalSize<u32>)`
  - [x] Map to mux WindowId, get TermWindow
  - [x] Clear `tab_width_lock`
  - [x] Resize wgpu surface
  - [x] If DPI changed: reload fonts, rebuild atlas
  - [x] Compute new grid dimensions
  - [x] Resize panes in active tab of this window
  - [x] Mark dirty, request redraw
- [x] `close_window(&mut self, winit_id: WindowId, event_loop: &ActiveEventLoop)`
  - [x] Map to mux WindowId
  - [x] If **last** terminal window: call `exit_app()` **before** dropping panes (ConPTY)
  - [x] Close all tabs via mux: `mux.close_window(window_id)`
  - [x] Drop all Pane structs on background threads
  - [x] Remove WindowContext and update focus
- [x] `exit_app(&self) -> !`
  - [x] Save GPU pipeline cache to disk (async)
  - [x] `process::exit(0)` — **must not return**
- [x] Fullscreen toggle:
  - [x] Query `window.fullscreen()`, toggle between `Some(Borderless(None))` and `None`
  - [x] Wired to `Action::ToggleFullscreen` keybinding
- [x] DPI change:
  - [x] `handle_dpi_change(&mut self, winit_id: WindowId, scale_factor: f64)`
  - [x] Reload fonts at `config.font.size * new_scale`
  - [x] Rebuild glyph atlas
  - [x] Mark all grid lines dirty for re-extraction

**Tests:**
- [x] No-flash: window opens with themed background, no gray/white flash (visual/manual)
- [x] DPI change: fonts reload, grids reflow, no artifacts (visual/manual)
- [x] Multi-window: `NewWindow` keybinding creates new window, close removes it
- [x] Exit ordering: last window → `exit_app()` before dropping panes (ConPTY-safe)
- [x] Resize: per-window resize via parameterized `handle_resize(winit_id, size)`

---

## 32.4 Cross-Window Operations

Move tabs between windows. Tab identity (TabId) preserved — same panes, same layout tree, different window.

**File:** `oriterm/src/app/window_management.rs`

- [x] `move_tab_to_window(&mut self, tab_id: TabId, target_window: WindowId)`
  - [x] Remove tab from source `MuxWindow.tabs`
  - [x] Add to target `MuxWindow.tabs`
  - [x] If source window now empty: close it (unless it's the last)
  - [x] Resize all panes in moved tab to target window dimensions
  - [x] Mark both windows dirty
- [x] `move_tab_to_new_window(&mut self, tab_id: TabId, event_loop: &ActiveEventLoop)`
  - [x] Refuse if it's the last tab in the last window
  - [x] Create new window via `create_window()`
  - [x] Move tab to new window
- [x] Tab tear-off integration (built on Section 17 drag infrastructure):
  - [x] Drag tab beyond `TEAR_OFF_THRESHOLD` → `move_tab_to_new_window`
  - [x] Drag tab to another window → `move_tab_to_window`
  - [x] Multi-pane tabs move as a unit (entire SplitTree preserved)

**Tests:**
- [x] Move tab from window A to window B: tab appears in B, removed from A
- [x] Move tab: panes resized to target window dimensions
- [x] Move last tab: source window closes (not the app, if other windows exist)
- [x] Tear-off: creates new window with the dragged tab
- [x] Multi-pane tab: split layout preserved after cross-window move

---

## 32.5 Section Completion

- [x] All 32.1–32.4 items complete
- [x] Tab management: create, close, duplicate, cycle, reorder — all through mux
- [x] Multi-window: shared GPU, font collection, config. Correct lifecycle.
- [x] No-flash window startup, DPI handling, Aero Snap
- [x] ConPTY-safe shutdown: exit_app before drop, background thread cleanup
- [x] Cross-window tab movement preserves pane state and layout tree
- [x] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [x] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [x] `cargo test` — all tests pass
- [x] **Tab lifecycle test**: create 5 tabs, close 3, cycle remaining, verify state
- [x] **Multi-window test**: 2 windows, move tab between, close one window
- [x] **Stress test**: rapidly create/close tabs — no freeze, no orphaned PTYs

**Exit Criteria:** Complete tab and window management through the mux layer. All patterns from superseded Sections 15 and 18 are implemented: ConPTY safety, no-flash startup, DPI handling, CWD inheritance, background thread drops, exit-before-drop ordering. Cross-window tab movement works with multi-pane tabs.
