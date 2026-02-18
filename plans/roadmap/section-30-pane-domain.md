---
section: 30
title: Pane Extraction + Domain System
status: not-started
tier: 4M
goal: Extract Pane from Tab, define the Domain trait for shell spawning, implement LocalDomain, create PaneRegistry and SessionRegistry
sections:
  - id: "30.1"
    title: Pane Struct Extraction
    status: not-started
  - id: "30.2"
    title: Domain Trait + LocalDomain
    status: not-started
  - id: "30.3"
    title: Pane + Session Registries
    status: not-started
  - id: "30.4"
    title: MuxEventProxy
    status: not-started
  - id: "30.5"
    title: Section Completion
    status: not-started
---

# Section 30: Pane Extraction + Domain System

**Status:** Not Started
**Goal:** Extract the per-shell unit (Pane) from the current Tab struct. Define the Domain trait that abstracts where shells are spawned (local, WSL, SSH). Implement LocalDomain for the common case. Build PaneRegistry and SessionRegistry for mux-level state tracking.

**Crate:** `oriterm_mux` (domain trait, registries), `oriterm` (Pane struct, LocalDomain impl)
**Dependencies:** `oriterm_mux` (section 29), `oriterm_core` (Term, Grid), `portable-pty`
**Prerequisite:** Section 29 complete (IDs and layout types available).

**Absorbs:** Section 15.1 (Tab Struct + Lifecycle) — the Pane struct inherits all hard-won patterns: ConPTY-safe shutdown, mode cache, lock-free dirty flags, PtyWriter shared between threads.

**Inspired by:**
- WezTerm: `Domain` trait (`mux/src/domain.rs`), `LocalDomain`, `RemoteDomain`, `WslDomain`
- Alacritty: `Tab` struct with `Arc<FairMutex<Term<EventProxy>>>` + separate `PtyWriter`

---

## 30.1 Pane Struct Extraction

Extract all per-shell-session state from what would have been Tab into a dedicated Pane struct. A Pane is the atomic unit of the mux — one shell process, one grid, one PTY connection.

**File:** `oriterm/src/pane.rs`

**Reference:** `_old/src/tab/mod.rs`, Section 15.1 design (preserved patterns)

- [ ] `Pane` struct fields (mirrors the Section 15.1 Tab design, renamed):
  - [ ] `id: PaneId` — globally unique (from `oriterm_mux::id`)
  - [ ] `terminal: Arc<FairMutex<Term<MuxEventProxy>>>` — thread-shared terminal state
  - [ ] `pty_writer: PtyWriter` — `Arc<Mutex<Box<dyn Write + Send>>>`, shared main + reader thread
  - [ ] `pty_master: Box<dyn portable_pty::MasterPty + Send>` — PTY master handle (owned)
  - [ ] `child: Box<dyn portable_pty::Child + Send + Sync>` — spawned child process
  - [ ] `selection: Option<Selection>` — main-thread-only
  - [ ] `search: Option<SearchState>` — main-thread-only
  - [ ] `grid_dirty: AtomicBool` — lock-free, set by reader thread
  - [ ] `mode_cache: Arc<AtomicU32>` — lock-free cache of `TermMode::bits()`
  - [ ] `wakeup_pending: Arc<AtomicBool>` — coalesces wakeup events
  - [ ] `domain_id: DomainId` — which domain spawned this pane
  - [ ] `title: String` — pane title (from OSC 2 or shell integration)
  - [ ] `cwd: Option<String>` — current working directory (from OSC 7)
- [ ] `PtyWriter` type alias: `Arc<parking_lot::Mutex<Box<dyn Write + Send>>>`
- [ ] `Pane::shutdown(&mut self)` — kill child process first (unblocks reader thread's blocking `read()`)
  - [ ] **ConPTY safety:** must not call from event loop directly on Windows — `ClosePseudoConsole` blocks
  - [ ] Kill child → reader sees EOF → sends `PtyExited` → exits
- [ ] Lock-free accessors (same as Section 15.1 design):
  - [ ] `grid_dirty(&self) -> bool`
  - [ ] `set_grid_dirty(&self, dirty: bool)`
  - [ ] `clear_wakeup(&self)`
  - [ ] `mode(&self) -> TermMode` — hot path for mouse reporting, no lock
- [ ] Locking accessors:
  - [ ] `send_pty(&self, bytes: &[u8])` — acquire pty_writer, write, flush
  - [ ] `resize(&self, cols: u16, rows: u16, pixel_w: u16, pixel_h: u16)` — lock terminal, resize grid, send to PTY master
  - [ ] `scroll_to_bottom(&mut self)` — lock terminal, set display_offset = 0
  - [ ] `clear_selection(&mut self)` — main-thread-only, no lock
- [ ] Mode cache protocol (unchanged from Section 15.1):
  - [ ] Reader thread: `mode_cache.store(term.mode.bits(), Relaxed)` after each VTE chunk
  - [ ] Main thread: `Pane::mode()` reads without lock

**Tests:**
- [ ] Pane creation: all fields initialized correctly
- [ ] Lock-free dirty flag: set on one thread, read on another
- [ ] Mode cache: write from "reader" thread, read from "main" thread
- [ ] Shutdown: child killed, reader can detect EOF

---

## 30.2 Domain Trait + LocalDomain

The Domain trait abstracts shell spawning. Each domain knows how to create a shell process in a particular environment. This is the extension point for WSL and SSH support.

**File:** `oriterm_mux/src/domain.rs` (trait), `oriterm/src/domain/local.rs` (LocalDomain impl)

**Reference:** WezTerm `mux/src/domain.rs` (Domain trait, DomainId, DomainState)

- [ ] `DomainId(u64)` newtype in `oriterm_mux/src/id.rs` — add to existing ID family
- [ ] `DomainState` enum: `Attached`, `Detached`
- [ ] `SpawnConfig` struct in `oriterm_mux/src/domain.rs`:
  - [ ] `cols: u16`, `rows: u16` — initial grid dimensions
  - [ ] `shell: Option<String>` — override default shell
  - [ ] `cwd: Option<String>` — working directory
  - [ ] `env: Vec<(String, String)>` — additional environment variables
  - [ ] `max_scrollback: usize`
  - [ ] `cursor_shape: CursorShape` — initial cursor shape from config
- [ ] `Domain` trait:
  ```rust
  pub trait Domain: Send + Sync {
      fn id(&self) -> DomainId;
      fn name(&self) -> &str;
      fn state(&self) -> DomainState;
      fn spawn_pane(&self, config: SpawnConfig) -> Result<PaneId>;
      fn can_spawn(&self) -> bool;
  }
  ```
- [ ] `LocalDomain` — spawns shells on the local machine via `portable-pty`:
  - [ ] `LocalDomain::new(id: DomainId) -> Self`
  - [ ] `spawn_pane`: create PTY, spawn shell, build `Pane`, register in `PaneRegistry`, spawn reader thread
  - [ ] Sets `TERM_PROGRAM=oriterm`, `TERM=xterm-256color`
  - [ ] Respects `SpawnConfig.shell` or detects default (`$SHELL`, `cmd.exe`)
  - [ ] Applies CWD if provided
- [ ] `WslDomain` — stub for now (Tier 7A full implementation):
  - [ ] `WslDomain::new(id: DomainId, distro: String) -> Self`
  - [ ] `spawn_pane`: spawns `wsl.exe -d <distro> -- <shell>` via `portable-pty`
  - [ ] `can_spawn`: checks if WSL is available (`wsl.exe --list` succeeds)

**Tests:**
- [ ] `LocalDomain`: `can_spawn()` returns true
- [ ] `LocalDomain`: `spawn_pane()` creates a pane with valid PaneId
- [ ] `WslDomain` stub: `can_spawn()` returns false if WSL unavailable
- [ ] `SpawnConfig` default values are sensible

---

## 30.3 Pane + Session Registries

Central registries that track all panes and sessions. The mux layer owns these — the GUI queries them by ID, never by direct reference.

**File:** `oriterm_mux/src/registry.rs`

- [ ] `PaneRegistry`:
  - [ ] `HashMap<PaneId, PaneEntry>` — metadata per pane (not the Pane struct itself, which lives in `oriterm`)
  - [ ] `PaneEntry`:
    - [ ] `pane_id: PaneId`
    - [ ] `tab_id: TabId` — which tab this pane belongs to
    - [ ] `domain_id: DomainId` — which domain spawned it
    - [ ] `title: String` — display title
    - [ ] `is_alive: bool` — false after PTY exit
  - [ ] `register(entry: PaneEntry)` — add pane to registry
  - [ ] `unregister(pane_id: PaneId)` — remove pane
  - [ ] `get(pane_id: PaneId) -> Option<&PaneEntry>`
  - [ ] `panes_in_tab(tab_id: TabId) -> Vec<PaneId>` — all panes belonging to a tab
  - [ ] `alive_count() -> usize` — number of living panes
- [ ] `SessionRegistry`:
  - [ ] `MuxTab` struct (mux-level tab, NOT the GUI tab bar concept):
    - [ ] `id: TabId`
    - [ ] `title: String`
    - [ ] `tree: SplitTree` — the immutable layout tree
    - [ ] `floating: FloatingLayer` — floating panes overlay
    - [ ] `active_pane: PaneId` — currently focused pane
    - [ ] `zoomed_pane: Option<PaneId>` — zoomed pane (fills entire tab area)
    - [ ] `tree_history: Vec<SplitTree>` — undo stack (limited to 50 entries)
  - [ ] `MuxWindow` struct:
    - [ ] `id: WindowId`
    - [ ] `tabs: Vec<TabId>` — tab order
    - [ ] `active_tab: usize` — index into `tabs`
  - [ ] `SessionRegistry`:
    - [ ] `tabs: HashMap<TabId, MuxTab>`
    - [ ] `windows: HashMap<WindowId, MuxWindow>`
    - [ ] Tab CRUD: `create_tab`, `close_tab`, `get_tab`, `get_tab_mut`
    - [ ] Window CRUD: `create_window`, `close_window`, `get_window`, `get_window_mut`
    - [ ] `tab_for_pane(pane_id: PaneId) -> Option<TabId>` — find which tab contains a pane
    - [ ] `window_for_tab(tab_id: TabId) -> Option<WindowId>` — find which window contains a tab

**Tests:**
- [ ] PaneRegistry: register/unregister/get lifecycle
- [ ] PaneRegistry: `panes_in_tab` returns correct subset
- [ ] SessionRegistry: create tab with initial pane, verify tree is `Leaf`
- [ ] SessionRegistry: split pane, verify tree updates to `Split`
- [ ] SessionRegistry: `tab_for_pane` and `window_for_tab` resolve correctly
- [ ] Undo stack: tree_history grows on mutations, pops on undo

---

## 30.4 MuxEventProxy

The bridge between the PTY reader thread and the mux layer. Replaces the direct `EventLoopProxy<TermEvent>` with a mux-aware proxy that routes events through the mux before reaching the GUI.

**File:** `oriterm/src/mux_event.rs`

**Reference:** Alacritty `event.rs` (EventListener trait), WezTerm `mux/src/lib.rs` (MuxNotification)

- [ ] `MuxEvent` enum — events from panes to the mux:
  - [ ] `PaneOutput(PaneId)` — pane has new terminal output (dirty)
  - [ ] `PaneExited(PaneId)` — PTY process exited
  - [ ] `PaneTitleChanged(PaneId, String)` — OSC 2 title update
  - [ ] `PaneCwdChanged(PaneId, String)` — OSC 7 CWD update
  - [ ] `PaneBell(PaneId)` — bell fired
- [ ] `MuxEventProxy` — implements `oriterm_core::EventListener`:
  - [ ] Wraps `mpsc::Sender<MuxEvent>` (or `crossbeam::channel::Sender`)
  - [ ] `send_event(Event)` → maps `Event` variants to `MuxEvent` variants
  - [ ] Coalesces `Wakeup` events via `AtomicBool` (same as Section 15.1 pattern)
  - [ ] Cheap to clone (sender is `Clone`)
- [ ] `MuxNotification` enum — events from mux to GUI:
  - [ ] `PaneDirty(PaneId)` — pane needs redraw
  - [ ] `PaneClosed(PaneId)` — pane was closed
  - [ ] `TabLayoutChanged(TabId)` — split tree changed
  - [ ] `WindowTabsChanged(WindowId)` — tab list changed
  - [ ] `Alert(PaneId, AlertKind)` — bell, urgent, etc.
- [ ] GUI subscribes to `MuxNotification` via an `mpsc::Receiver` on the main thread
  - [ ] `EventLoopProxy::send_event(TermEvent::MuxNotification)` to wake winit

**Tests:**
- [ ] `MuxEventProxy` implements `EventListener` correctly
- [ ] Wakeup coalescing: multiple sends produce one notification
- [ ] All `Event` variants map to correct `MuxEvent` variants
- [ ] `MuxNotification` roundtrips: send from mux, receive on GUI

---

## 30.5 Section Completion

- [ ] All 30.1–30.4 items complete
- [ ] Pane struct: clean extraction from Tab, all lock-free patterns preserved
- [ ] Domain trait: defined in `oriterm_mux`, `LocalDomain` implemented in `oriterm`
- [ ] `WslDomain` stub compiles (full impl in Section 35)
- [ ] PaneRegistry and SessionRegistry: central state management with correct lookups
- [ ] MuxEventProxy: bridges PTY reader → mux → GUI with coalescing
- [ ] `cargo build --target x86_64-pc-windows-gnu` — full workspace compiles
- [ ] `cargo clippy -p oriterm_mux --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test -p oriterm_mux` — all tests pass
- [ ] No `unsafe` code

**Exit Criteria:** Pane is extracted as the atomic per-shell unit. The Domain trait abstracts shell spawning for local, WSL, and (future) SSH. Registries provide central lookup for all mux state. The event proxy bridges PTY threads to the mux layer with proper coalescing. All patterns from the superseded Section 15 are preserved.
