---
section: 18
title: Multi-Window & Window Lifecycle
status: superseded
tier: 4
goal: Multiple windows with shared GPU, window creation/destruction, DPI, Aero Snap
superseded_by: [32]
superseded_reason: "Absorbed into Section 32 (Tab & Window Management, Mux-Aware). Multi-window support is now built on top of the mux layer, with windows as thin GUI shells that subscribe to mux notifications."
sections:
  - id: "18.1"
    title: Multi-Window Support
    status: superseded
  - id: "18.2"
    title: Window Lifecycle
    status: superseded
  - id: "18.3"
    title: Section Completion
    status: superseded
---

# Section 18: Multi-Window & Window Lifecycle

> **SUPERSEDED** — This section has been absorbed into the first-class multiplexing architecture.
> - Multi-window support + window lifecycle → **Section 32** (Tab & Window Management, Mux-Aware)
>
> The original design had windows owning tabs directly. The multiplexing
> redesign makes windows thin GUI clients that subscribe to mux notifications
> for pane output. All hard-won patterns (no-flash startup, DPI handling,
> Aero Snap, ConPTY-safe cleanup, exit-before-drop) are preserved in Section 32.

**Status:** Superseded
**Goal:** Multiple windows with shared GPU device, font collections, and config. Window creation/destruction with no-flash startup, DPI handling, ConPTY-safe cleanup, and Aero Snap support.

**Crate:** `oriterm` (binary only — no core changes)
**Dependencies:** `wgpu`, `winit`, `window-vibrancy` (optional)
**Reference:** `_old/src/app/window_management.rs`

**Prerequisite:** Section 13 complete (tab management). Section 04 complete (GPU rendering foundation).

---

## 18.1 Multi-Window Support

Multiple windows, each with their own tab list. Tabs can move between windows via tear-off and merge. All windows share the same GPU device, font collections, and config.

**File:** `oriterm/src/app/window_management.rs`

**Reference:** `_old/src/app/window_management.rs`

- [ ] Data structure:
  - [ ] `App::windows: HashMap<WindowId, TermWindow>` — all open windows
  - [ ] `App::tabs: HashMap<TabId, Tab>` — global tab storage (flat, not nested in windows)
  - [ ] Each `TermWindow` has `tabs: Vec<TabId>` (display order) and `active_tab: usize` (index into vec)
  - [ ] A tab exists in exactly one window at a time
  - [ ] `window_containing_tab(&self, tab_id: TabId) -> Option<WindowId>` — linear scan of all windows
- [ ] `TermWindow` struct:
  - [ ] `window: Arc<Window>` — winit window handle
  - [ ] `surface: wgpu::Surface<'static>` — GPU surface for this window
  - [ ] `surface_config: wgpu::SurfaceConfiguration` — surface format, size
  - [ ] `tabs: Vec<TabId>` — tab order in this window
  - [ ] `active_tab: usize` — index into `tabs` (NOT a TabId)
  - [ ] `is_maximized: bool` — tracked for maximize/restore button icon
- [ ] `TermWindow` methods:
  - [ ] `active_tab_id(&self) -> Option<TabId>` — `self.tabs.get(self.active_tab).copied()`
  - [ ] `tab_index(&self, id: TabId) -> Option<usize>` — `self.tabs.iter().position(|t| *t == id)`
  - [ ] `add_tab(&mut self, id: TabId)` — push to vec, set `active_tab = len - 1`
  - [ ] `remove_tab(&mut self, id: TabId) -> bool` — remove from vec, adjust `active_tab` if needed, return true if vec is now empty
  - [ ] `resize_surface(&mut self, device: &wgpu::Device, width: u32, height: u32)` — reconfigure surface
- [ ] `active_tab_id(&self, window_id: WindowId) -> Option<TabId>` — convenience on App
- [ ] Cross-window tab movement:
  - [ ] Tab identity (TabId) is preserved — same tab object, different window
  - [ ] Remove from source `tw.tabs`, add to target `tw.tabs`
  - [ ] May need grid resize if target window has different dimensions
- [ ] Focus tracking:
  - [ ] `WindowEvent::Focused(true)` — send focus-in to active tab's terminal (if `FOCUS_IN_OUT` mode active)
  - [ ] `WindowEvent::Focused(false)` — send focus-out

---

## 18.2 Window Lifecycle

Window creation is expensive (GPU surface, compositor effects, DPI handling). Destruction has ordering constraints on Windows due to ConPTY. Getting startup right (no gray flash, correct DPI) requires careful sequencing.

**File:** `oriterm/src/app/window_management.rs`

**Reference:** `_old/src/app/window_management.rs`

- [ ] `create_window(&mut self, event_loop: &ActiveEventLoop, saved_pos: Option<&WindowState>, visible: bool) -> Option<WindowId>`
  - [ ] Calculate window size from font metrics + grid dimensions + `TAB_BAR_HEIGHT`
  - [ ] Request transparency if opacity < 1.0 (`WindowAttributes::with_transparent(true)`)
  - [ ] Enable `WS_EX_NOREDIRECTIONBITMAP` on Windows for proper alpha compositing
  - [ ] Create winit window (may fail if display server unavailable)
  - [ ] Capture initial DPI scale factor from `window.scale_factor()`
  - [ ] If DPI differs from app-level scale: reload font collections at scaled size
  - [ ] **On FIRST window only**: initialize `GpuState` and `GpuRenderer` (expensive, ~10ms, includes device/adapter/queue creation)
  - [ ] Create wgpu `Surface` for this window
  - [ ] **Render a clear frame BEFORE showing** — prevents gray/white flash:
    1. Build a black frame (or themed background)
    2. Submit to GPU
    3. `device.poll(wgpu::Maintain::Wait)` — synchronous, ensures frame is ready
  - [ ] Apply compositor effects:
    - [ ] Windows: accent border color, Mica/acrylic if configured (window-vibrancy crate)
    - [ ] macOS: vibrancy/blur
  - [ ] Enable Aero Snap on Windows (custom WndProc subclass for `WM_NCHITTEST`)
  - [ ] Restore saved window position if `saved_pos` provided (before showing, to avoid jump)
  - [ ] Show window (if `visible == true`)
  - [ ] Return `WindowId`
- [ ] **Fullscreen toggle**:
  - [ ] `TermWindow::toggle_fullscreen(&self)` — query `window.fullscreen()`, toggle between `Some(Fullscreen::Borderless(None))` and `None`
  - [ ] Wired to `Action::ToggleFullscreen` keybinding (Alt+Enter on Windows/Linux, Ctrl+Cmd+F on macOS)
  - [ ] No separate `is_fullscreen` state — query winit's `window.fullscreen()` directly as source of truth
  - [ ] **Ref:** Alacritty `display/window.rs:392-428`, winit `Window::set_fullscreen`, `Window::fullscreen`
- [ ] `handle_resize(&mut self, window_id: WindowId, width: u32, height: u32)`
  - [ ] Settings window: just resize surface, return early
  - [ ] Query actual DPI on Windows (WndProc subclass may have updated it via `WM_DPICHANGED`)
  - [ ] Clear `tab_width_lock` (window size changed, tab widths must recalculate)
  - [ ] Resize wgpu surface
  - [ ] If DPI changed since last resize: reload fonts at scaled size, rebuild atlas
  - [ ] Compute new grid dimensions: `grid_dims_for_size(width, height)`
  - [ ] **Resize ALL tabs in the window** (not just active) — inactive tabs need their grids reflow'd:
    - [ ] For each `tab_id` in `tw.tabs`: `tab.clear_selection()`, `tab.resize(cols, rows, pixel_w, pixel_h)`
  - [ ] Mark `tab_bar_dirty`, request redraw
- [ ] `close_window(&mut self, window_id: WindowId, event_loop: &ActiveEventLoop)`
  - [ ] If settings window: just close it, don't exit
  - [ ] Check if other terminal windows remain
  - [ ] If **last** terminal window: call `exit_app()` **before** dropping tabs (ConPTY blocks on drop)
  - [ ] For each tab in window: shutdown and drop on background thread
  - [ ] Remove window from `self.windows`
- [ ] `exit_app(&mut self)`
  - [ ] Save window position to disk (for restore on next launch)
  - [ ] Save GPU pipeline cache to disk (faster shader compilation next time)
  - [ ] Shutdown all tabs
  - [ ] Release mouse capture (prevents stale events going to app behind)
  - [ ] `process::exit(0)` — don't join threads, OS process cleanup handles them
  - [ ] **Must not return** — callers rely on this not returning to avoid use-after-free on tab state
- [ ] DPI change handling:
  - [ ] `handle_scale_factor_changed(&mut self, window_id: WindowId, new_scale: f64)`
  - [ ] Reload font collections at `config.font.size * new_scale`
  - [ ] Reload UI font collection at scaled size x `UI_FONT_SCALE`
  - [ ] Rebuild glyph atlas
  - [ ] Resize all tabs in all windows (grid dimensions change when cell size changes)

---

## 18.3 Section Completion

- [ ] All 18.1–18.2 items complete
- [ ] Multi-window: shared GPU, flat tab storage, cross-window tab movement
- [ ] Window lifecycle: no-flash startup, DPI-aware resize, ConPTY-safe cleanup, exit-before-drop
- [ ] TermWindow struct with clean surface management and tab list
- [ ] Focus tracking sends focus-in/focus-out to terminal when FOCUS_IN_OUT mode is active
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] **No-flash test**: window opens with themed background, no gray/white flash on any DPI
- [ ] **DPI test**: drag window between monitors with different DPI — fonts reload, grid reflows, no artifacts
- [ ] **Multi-window test**: tear-off tab creates new window, close last tab in window closes it, close last window exits app cleanly
- [ ] **Exit ordering test**: closing the last window calls `exit_app()` before dropping tabs — no ConPTY deadlock

**Exit Criteria:** Multiple windows with shared GPU device work correctly. Windows create without flash, handle DPI changes, and destroy with correct ConPTY-safe ordering. Cross-window tab movement preserves tab identity and resizes grids as needed.
