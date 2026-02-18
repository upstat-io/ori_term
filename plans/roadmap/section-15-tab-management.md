---
section: 15
title: Tab Struct & Management
status: superseded
tier: 4
goal: Tab struct with clean lifecycle, mode cache, ConPTY-safe shutdown; tab CRUD operations
superseded_by: [30, 32]
superseded_reason: "Absorbed into Section 30 (Pane Extraction + Domain System) and Section 32 (Tab & Window Management, Mux-Aware). The Tab struct is replaced by Pane + mux-aware Tab shell; tab CRUD moves to Section 32 with mux integration."
sections:
  - id: "15.1"
    title: Tab Struct + Lifecycle
    status: superseded
  - id: "15.2"
    title: Tab Management Operations
    status: superseded
  - id: "15.3"
    title: Section Completion
    status: superseded
---

# Section 15: Tab Struct & Management

> **SUPERSEDED** — This section has been absorbed into the first-class multiplexing architecture.
> - Tab struct + lifecycle → **Section 30** (Pane Extraction + Domain System)
> - Tab management operations → **Section 32** (Tab & Window Management, Mux-Aware)
>
> The original design assumed tabs as the atomic unit. The multiplexing redesign
> makes *panes* the atomic unit (one shell per pane) and tabs become layout
> containers managed through the mux layer. All hard-won patterns from this
> section (ConPTY-safe shutdown, mode cache, lock-free dirty flags, CWD
> inheritance) are preserved in the new sections.

**Status:** Superseded
**Goal:** Tab struct with clean lifecycle, mode cache, ConPTY-safe shutdown. Tab CRUD operations: create, close, duplicate, cycle, reorder, CWD inheritance.

**Crate:** `oriterm` (binary only — no core changes)
**Dependencies:** `portable-pty`, `parking_lot`, `winit`
**Reference:** `_old/src/tab/mod.rs`, `_old/src/tab/types.rs`, `_old/src/app/tab_management.rs`

**Why this was hard in the prototype:** The tab system is where threading, rendering, input dispatch, platform quirks, and state management all collide. The old prototype had a working system but suffered from a god-object `App` struct (40+ fields), unclear ownership boundaries, and platform-specific workarounds scattered across files. The rebuild must preserve all the hard-won behaviors (ConPTY deadlock avoidance, tear-off merge, mode cache, CWD inheritance) while organizing them cleanly.

---

## 15.1 Tab Struct + Lifecycle

The Tab is the per-terminal unit — one Tab per shell process. Each Tab owns a PTY, a shared terminal state behind a FairMutex, and main-thread-only UI state (selection, search). Getting the lifecycle right is critical: ConPTY on Windows blocks indefinitely during cleanup if not handled correctly.

**File:** `oriterm/src/tab.rs`

**Reference:** `_old/src/tab/mod.rs`, `_old/src/tab/types.rs`

- [ ] `Tab` struct fields:
  - [ ] `id: TabId` — unique identifier (newtype over `u64`)
  - [ ] `terminal: Arc<FairMutex<Term<EventProxy>>>` — thread-shared terminal state
  - [ ] `pty_writer: PtyWriter` — `Arc<Mutex<Box<dyn Write + Send>>>`, shared between main thread (keyboard input) and PTY reader thread (VTE responses like DA, DECRPM)
  - [ ] `pty_master: Box<dyn portable_pty::MasterPty + Send>` — PTY master handle (owned, not shared)
  - [ ] `child: Box<dyn portable_pty::Child + Send + Sync>` — spawned child process handle
  - [ ] `selection: Option<Selection>` — main-thread-only, not behind mutex
  - [ ] `search: Option<SearchState>` — main-thread-only, not behind mutex
  - [ ] `has_bell_badge: bool` — bell fired on inactive tab, cleared when tab becomes active
  - [ ] `grid_dirty: AtomicBool` — lock-free dirty flag, set by reader thread, read by renderer
  - [ ] `mode_cache: Arc<AtomicU32>` — lock-free cache of `TermMode::bits()`, updated by reader thread after each VTE parse chunk, read by main thread for mouse reporting / key encoding decisions without acquiring the terminal lock
  - [ ] `wakeup_pending: Arc<AtomicBool>` — coalesces `TermEvent::Wakeup` events so multiple PTY read chunks don't spam the event loop
- [ ] `PtyWriter` type alias: `Arc<parking_lot::Mutex<Box<dyn Write + Send>>>`
  - [ ] Both main thread and reader thread write to the same pipe
  - [ ] Main thread: keyboard input, paste
  - [ ] Reader thread: VTE responses (DA, DECRPM, XTVERSION) — **must be written outside the terminal lock** to avoid ConPTY deadlock
- [ ] `SpawnConfig` struct — creation parameters:
  - [ ] `id: TabId`, `cols: usize`, `rows: usize`
  - [ ] `proxy: EventLoopProxy<TermEvent>` — for reader thread → main thread wakeup
  - [ ] `shell: Option<String>` — override default shell
  - [ ] `max_scrollback: usize` — scrollback buffer capacity
  - [ ] `cursor_shape: CursorShape` — initial cursor shape from config
  - [ ] `integration_dir: Option<PathBuf>` — path to shell-integration scripts
  - [ ] `cwd: Option<String>` — working directory (inherited from parent tab)
- [ ] `Tab::spawn(config: SpawnConfig) -> Option<Tab>`
  - [ ] Create PTY with portable-pty at given dimensions
  - [ ] Spawn shell command (respecting `config.shell` or default)
  - [ ] Set environment: `TERM_PROGRAM=oriterm`, `TERM=xterm-256color`
  - [ ] If `cwd` provided: set working directory for child
  - [ ] Clone reader from PTY master
  - [ ] Take writer from PTY master, wrap in `Arc<Mutex<>>`
  - [ ] Create `Term<EventProxy>` with grid dimensions and scrollback
  - [ ] Wrap in `Arc<FairMutex<>>`
  - [ ] Spawn reader thread (see section 03 for PTY reader details)
  - [ ] Apply shell integration environment variables (see section 17)
  - [ ] Return Tab
- [ ] `Tab::shutdown(&mut self)`
  - [ ] Kill child process first — this unblocks the reader thread's blocking `read()` call
  - [ ] Reader thread will see EOF or error, send `PtyExited`, and exit
  - [ ] **Do NOT call this from the event loop directly on Windows** — `ClosePseudoConsole` blocks until the reader thread exits, which can take seconds for full-screen apps (vim, htop)
- [ ] Lock-free accessors (no terminal lock required):
  - [ ] `Tab::grid_dirty(&self) -> bool` — `self.grid_dirty.load(Relaxed)`
  - [ ] `Tab::set_grid_dirty(&self, dirty: bool)` — `self.grid_dirty.store(dirty, Relaxed)`
  - [ ] `Tab::clear_wakeup(&self)` — `self.wakeup_pending.store(false, Relaxed)` — called when Wakeup event is consumed, allows reader thread to send another
  - [ ] `Tab::mode(&self) -> TermMode` — `TermMode::from_bits_truncate(self.mode_cache.load(Relaxed))` — hot path for mouse reporting checks on every mouse move
- [ ] Locking accessors:
  - [ ] `Tab::grid(&self) -> MappedMutexGuard<'_, Grid>` — locks terminal, maps to grid reference. **Callers must not call other locking methods while this guard is alive.**
  - [ ] `Tab::send_pty(&self, bytes: &[u8])` — acquire pty_writer lock, write bytes, flush
  - [ ] `Tab::cwd(&self) -> Option<String>` — lock terminal briefly to read CWD
  - [ ] `Tab::resize(&self, cols: usize, rows: usize, pixel_w: u16, pixel_h: u16)` — lock terminal, resize grid, send resize to PTY master
  - [ ] `Tab::scroll_to_bottom(&mut self)` — lock terminal, set display_offset = 0
  - [ ] `Tab::clear_selection(&mut self)` — set `self.selection = None` (no lock needed, main-thread-only)
- [ ] **Mode cache protocol** (critical for responsiveness):
  - [ ] Reader thread updates: `self.mode_cache.store(term.mode.bits(), Relaxed)` after each VTE parse chunk, inside the terminal lock, just before dropping it
  - [ ] Main thread reads: `Tab::mode()` returns cached value without locking
  - [ ] Used for: mouse reporting check on every `CursorMoved`, key encoding mode selection, bracketed paste detection
  - [ ] Safe because `TermMode::bits()` is a pure bitfield — no pointer validity concerns from stale reads

---

## 15.2 Tab Management Operations

Create, close, duplicate, cycle, and move tabs between windows. Tabs live in a global `HashMap<TabId, Tab>` while windows hold `Vec<TabId>` for order. A tab exists in exactly one window at a time.

**File:** `oriterm/src/app/tab_management.rs`

**Reference:** `_old/src/app/tab_management.rs`

- [ ] Tab ID allocation:
  - [ ] `App::next_tab_id: u64` — monotonically increasing counter
  - [ ] `alloc_tab_id(&mut self) -> TabId` — `TabId(self.next_tab_id)`, increment
- [ ] `new_tab_in_window(&mut self, window_id: WindowId) -> Option<TabId>`
  - [ ] Inherit CWD from currently active tab in the window (if any)
  - [ ] Call `spawn_tab(window_id, cwd)`
  - [ ] Clear `tab_width_lock` if it belongs to this window (tab count changed, widths will recalculate)
  - [ ] Return TabId
- [ ] `spawn_tab(&mut self, window_id: WindowId, cwd: Option<&str>) -> Option<TabId>`
  - [ ] Compute grid dimensions from window size via `grid_dims_for_size()`
  - [ ] Allocate TabId
  - [ ] Build `SpawnConfig` with shell, scrollback, cursor shape, integration dir, CWD
  - [ ] Call `Tab::spawn(config)` — blocks on PTY creation, spawns reader thread
  - [ ] Apply current color scheme to new tab: `tab.apply_color_config(scheme, &config.colors, bold_is_bright)`
  - [ ] Insert into `self.tabs: HashMap<TabId, Tab>`
  - [ ] Add to window's tab list: `tw.add_tab(tab_id)` — new tab becomes active
  - [ ] Mark `tab_bar_dirty = true`, request redraw
- [ ] `close_tab(&mut self, tab_id: TabId, event_loop: &ActiveEventLoop)`
  - [ ] Remove from window's tab list: `tw.remove_tab(tab_id)` — returns true if window now empty
  - [ ] If window now empty and it's the **last** terminal window: call `exit_app()` **immediately** — do NOT drop tabs first (ConPTY cleanup blocks on Windows)
  - [ ] If window now empty but other windows exist: close the empty window
  - [ ] Otherwise: adjust `active_tab` index, request redraw
  - [ ] Remove tab from global map: `self.tabs.remove(&tab_id)`
  - [ ] Call `tab.shutdown()` — kill child process
  - [ ] **Spawn background thread to drop the tab**: `std::thread::spawn(move || drop(tab))` — on Windows, `ClosePseudoConsole` blocks until reader thread exits. Full-screen apps (vim, htop) may take seconds. Must not freeze the event loop.
  - [ ] Mark `tab_bar_dirty = true`
- [ ] `duplicate_tab_at(&mut self, tab_index: usize)`
  - [ ] Find the tab at `tab_index` in any window
  - [ ] Clone CWD from source tab
  - [ ] Call `spawn_tab(window_id, cwd)` — new tab inherits directory but starts a fresh shell
- [ ] `cycle_tab(&mut self, window_id: WindowId, delta: isize)`
  - [ ] `tw.active_tab = (tw.active_tab as isize + delta).rem_euclid(n as isize) as usize`
  - [ ] Wrapping arithmetic — Ctrl+Tab wraps from last to first, Ctrl+Shift+Tab wraps first to last
  - [ ] Clear bell badge on newly active tab
  - [ ] Mark `tab_bar_dirty`, request redraw
- [ ] `switch_to_tab(&mut self, tab_id: TabId)`
  - [ ] Find the window containing this tab
  - [ ] Set `tw.active_tab` to the tab's index
  - [ ] Clear bell badge
  - [ ] Mark dirty, redraw
- [ ] `move_tab(&mut self, from: usize, to: usize, window_id: WindowId)`
  - [ ] Reorder `tw.tabs` vec: remove from `from`, insert at `to`
  - [ ] Adjust `tw.active_tab` to track the same tab
  - [ ] Mark `tab_bar_dirty`
- [ ] `move_tab_to_new_window(&mut self, tab_index: usize, event_loop: &ActiveEventLoop)`
  - [ ] Refuse if it's the last tab in the window (would close the app)
  - [ ] Remove tab from source window's tab list
  - [ ] Create new window (see section 16)
  - [ ] Add tab to new window's tab list
  - [ ] Mark dirty, redraw both windows
- [ ] Auto-close on PTY exit:
  - [ ] `TermEvent::PtyExited(tab_id)` received → call `close_tab(tab_id, event_loop)`
- [ ] **Tests**:
  - [ ] Create 3 tabs, verify IDs are unique and monotonically increasing
  - [ ] Close middle tab, verify remaining tabs order is preserved and active_tab adjusts
  - [ ] Next/prev cycling wraps around: tab 2 of 3 → next → tab 0
  - [ ] CWD inheritance: new tab inherits active tab's CWD
  - [ ] Closing last tab in last window triggers `exit_app()`
  - [ ] Tab drop happens on background thread (verify with a mock that blocks)

---

## 15.3 Section Completion

- [ ] All 15.1–15.2 items complete
- [ ] Tab struct: clean ownership, lock-free mode cache, background thread cleanup
- [ ] Tab management: create, close, duplicate, cycle, reorder, CWD inheritance
- [ ] ConPTY deadlock avoidance: kill child before drop, background thread for cleanup
- [ ] Mode cache protocol: reader thread writes, main thread reads without locking
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] **Tests**: tab lifecycle, ID allocation, close ordering, cycling wrap-around
- [ ] **Stress test**: rapidly close many tabs — no freeze, no orphaned PTY processes

**Exit Criteria:** Tab struct has clean ownership with lock-free hot-path accessors. Tab management handles all CRUD operations with correct ConPTY-safe shutdown ordering. CWD inheritance works. Background thread cleanup prevents event loop freezes.
