---
section: "01"
title: "Target API & GUI Session Layer"
status: complete
goal: "Define the flat mux API and create GUI-side session types to replace mux-owned tab/window/layout concepts"
depends_on: []
sections:
  - id: "01.1"
    title: "Define Flat Mux API"
    status: complete
  - id: "01.2"
    title: "Create GUI Session Types"
    status: complete
  - id: "01.3"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Target API & GUI Session Layer

**Status:** Complete
**Goal:** The flat mux API is defined and documented. GUI-side session types
exist in `oriterm` and can model the current tab/window/layout hierarchy
without depending on any mux types beyond `PaneId` and `DomainId`.

**Context:** The mux currently owns the entire session model: `MuxWindow`
contains `Vec<TabId>`, `MuxTab` contains `SplitTree` with `active_pane` and
zoom state. The GUI reads these immutably via `mux.session()`. We need to
invert this — the GUI owns its session model and the mux is just a pane
server.

**Reference implementations:**
- **tmux** `server.c`, `session.c`: Server owns sessions/windows/panes, but
  the client protocol is pane-centric (send keys to pane, get output from
  pane). Session model is server-side because tmux IS a multiplexer.
- **Alacritty**: No mux layer at all. Single window, single terminal. The
  simplest version of "GUI owns everything."

---

## 01.1 Define Flat Mux API

**File(s):** Documentation only (no code changes yet)

The flat mux exposes only pane-level operations. Everything else is the
client's problem.

### What stays in `oriterm_mux`

**Types:**
- `PaneId` — pane identity
- `DomainId` — spawn domain identity (local, WSL, SSH)
- `ClientId` — IPC client identity (daemon mode)
- `IdAllocator<PaneId>`, `IdAllocator<DomainId>`, `IdAllocator<ClientId>`

**Pane lifecycle:**
- `spawn_pane(config, wakeup) -> io::Result<(PaneId, Pane)>`
- `close_pane(pane_id)` — kills PTY, unregisters
- `resize_pane(pane_id, cols, rows)` -- resizes PTY
  (currently named `resize_pane_grid` in `MuxBackend` trait; rename
  after the tab-scoped `resize_pane` is removed in section 05.4)
- `write_to_pane(pane_id, data)` — sends bytes to PTY stdin
- `get_pane(pane_id) -> Option<&Pane>` / `get_pane_mut`

**Pane metadata:**
- `PaneEntry { pane: PaneId, domain: DomainId }` — no `tab` field
- `PaneRegistry` — flat `HashMap<PaneId, PaneEntry>`

**Events (PTY -> mux):**
- `MuxEvent::PaneOutput(PaneId)` — new terminal data
- `MuxEvent::PaneExited { pane_id, exit_code }`
- `MuxEvent::PaneTitleChanged { pane_id, title }`
- `MuxEvent::PaneIconChanged { pane_id, icon_name }`
- `MuxEvent::PaneCwdChanged { pane_id, cwd }`
- `MuxEvent::CommandComplete { pane_id, duration }`
- `MuxEvent::PaneBell(PaneId)`
- `MuxEvent::PtyWrite { pane_id, data }` — DA responses
- `MuxEvent::ClipboardStore { pane_id, clipboard_type, text }`
- `MuxEvent::ClipboardLoad { pane_id, clipboard_type, formatter }`

**Notifications (mux -> client):**
- `MuxNotification::PaneOutput(PaneId)` — content changed
- `MuxNotification::PaneClosed(PaneId)` — pane removed
- `MuxNotification::PaneTitleChanged(PaneId)`
- `MuxNotification::PaneBell(PaneId)`
- `MuxNotification::CommandComplete { pane_id, duration }`
- `MuxNotification::ClipboardStore { pane_id, clipboard_type, text }`
- `MuxNotification::ClipboardLoad { pane_id, clipboard_type, formatter }`

### What moves OUT of `oriterm_mux`

| Current Location | Type/Module | Destination |
|-----------------|-------------|-------------|
| `id/mod.rs` | `TabId`, `WindowId`, `SessionId` | `oriterm/src/session/` (delete `SessionId` if unused) |
| `session/mod.rs` | `MuxTab`, `MuxWindow` | `oriterm/src/session/` |
| `registry/mod.rs` | `SessionRegistry` | `oriterm/src/session/` |
| `layout/split_tree/` | `SplitTree` | `oriterm/src/session/` |
| `layout/floating/` | `FloatingLayer`, `FloatingPane` | `oriterm/src/session/` |
| `layout/rect.rs` | `Rect` | `oriterm/src/session/` |
| `layout/compute/` | `compute_layout`, `PaneLayout` | `oriterm/src/session/` |
| `nav/` | `navigate`, `Direction` | `oriterm/src/session/` |
| `in_process/event_pump.rs` | tab/window ops | `oriterm/src/app/` |
| `in_process/tab_ops.rs` | split/zoom/equalize | `oriterm/src/app/` |
| `in_process/floating_ops.rs` | floating pane CRUD | `oriterm/src/app/` |

### What gets DELETED (no replacement needed)

- `MuxNotification::TabLayoutChanged(TabId)` — GUI tracks its own layout changes
- `MuxNotification::FloatingPaneChanged(TabId)` — GUI tracks its own floating state
- `MuxNotification::WindowTabsChanged(WindowId)` — GUI tracks its own tab lists
- `MuxNotification::WindowClosed(WindowId)` — GUI manages its own windows
- `MuxNotification::LastWindowClosed` — GUI decides when to exit
- `MuxNotification::PaneDirty(PaneId)` — renamed to `PaneOutput(PaneId)` (aligns with `MuxEvent::PaneOutput`)
- `MuxNotification::Alert(PaneId)` — renamed to `PaneBell(PaneId)` (aligns with `MuxEvent::PaneBell`)
- All protocol messages for tab/window operations (see section 05 for full list)
- Server `window_to_client: HashMap<WindowId, ClientId>` mapping
- `MuxBackend` methods for tab/window/layout queries and mutations

- [x] Document the flat API in a design comment (not code yet)
- [x] Review against the SSH-attach litmus test: could a non-GUI client
      use this API to interact with panes without faking any session state?

---

## 01.2 Create GUI Session Types

**File(s):** `oriterm/src/session/` (new module)

Create the GUI's own session model. These types replace `MuxTab`, `MuxWindow`,
`SessionRegistry`, and the relocated layout modules.

- [x] Create `oriterm/src/session/mod.rs` with module declarations
  ```rust
  //! GUI session model: windows, tabs, and pane layouts.
  //!
  //! This module owns all presentation state — how panes are grouped into
  //! tabs, how tabs are grouped into windows, how panes are arranged
  //! within a tab. The mux layer knows nothing about this; it just
  //! provides panes.

  mod id;
  mod tab;
  mod window;
  mod registry;

  // Layout submodules (populated by section 04):
  // mod split_tree;
  // mod floating;
  // mod rect;
  // mod compute;
  // mod nav;

  pub use id::{TabId, WindowId};
  pub use tab::Tab;
  pub use window::Window;
  pub use registry::SessionRegistry;
  ```

- [x] Create `oriterm/src/session/id/mod.rs` — `TabId(u64)`, `WindowId(u64)`,
      `IdAllocator<TabId>`, `IdAllocator<WindowId>`
  - These are GUI-local IDs, not mux IDs
  - Keep the same newtype pattern as mux's `PaneId`
  - Use newtype wrappers (impl-hygiene.md: "Newtypes for IDs")
- [x] Create `oriterm/src/session/id/tests.rs` — unit tests for `TabId`, `WindowId`,
      `IdAllocator` (from_raw, raw, allocation sequence)

- [x] Create `oriterm/src/session/tab/mod.rs` — replaces `MuxTab`
  ```rust
  /// A GUI tab: a layout container for panes.
  ///
  /// Owns the split tree, floating layer, active pane tracking,
  /// zoom state, and undo/redo for layout mutations.
  pub struct Tab {
      id: TabId,
      tree: SplitTree,
      floating: FloatingLayer,
      active_pane: PaneId,
      undo: VecDeque<SplitTree>,
      redo: VecDeque<SplitTree>,
      zoomed_pane: Option<PaneId>,
  }
  ```
- [x] Create `oriterm/src/session/tab/tests.rs` — unit tests for `Tab`

- [x] Create `oriterm/src/session/window/mod.rs` — replaces `MuxWindow`
  ```rust
  /// A GUI window: an ordered collection of tabs.
  pub struct Window {
      id: WindowId,
      tabs: Vec<TabId>,
      active_tab_idx: usize,
  }
  ```
- [x] Create `oriterm/src/session/window/tests.rs` — unit tests for `Window`

- [x] Create `oriterm/src/session/registry/mod.rs` — replaces `SessionRegistry`
  ```rust
  /// GUI-side registry of tabs and windows.
  pub struct SessionRegistry {
      tabs: HashMap<TabId, Tab>,
      windows: HashMap<WindowId, Window>,
      tab_alloc: IdAllocator<TabId>,
      window_alloc: IdAllocator<WindowId>,
  }
  ```
  Note: Embed `IdAllocator` fields here so `SessionRegistry` owns ID allocation
  (the mux no longer allocates `TabId`/`WindowId`).
- [x] Create `oriterm/src/session/registry/tests.rs` — unit tests for `SessionRegistry`

- [x] Wire `mod session;` into `oriterm/src/main.rs`
- [x] Ensure `Tab` provides at minimum these methods (matching current `MuxTab` API):
  - `new(id, initial_pane_id)`, `id()`, `active_pane()`, `set_active_pane()`
  - `tree()`, `set_tree()`, `replace_layout()` (push to undo)
  - `floating()`, `floating_mut()`, `set_floating()`
  - `zoomed_pane()`, `set_zoomed_pane()`
  - `undo_tree()`, `redo_tree()`
  - `all_panes() -> Vec<PaneId>` (tiled + floating)
- [x] Ensure `Window` provides at minimum these methods (matching current `MuxWindow` API):
  - `new(id)`, `id()`, `tabs()`, `active_tab_idx()`, `set_active_tab_idx()`
  - `active_tab() -> Option<TabId>`, `add_tab()`, `remove_tab()`
  - `insert_tab_at()`, `reorder_tab()`, `replace_tabs()`
- [x] Ensure `SessionRegistry` provides at minimum these methods:
  - `new()`, `add_tab()`, `remove_tab()`, `get_tab()`, `get_tab_mut()`
  - `add_window()`, `remove_window()`, `get_window()`, `get_window_mut()`
  - `window_for_tab()`, `tab_count()`, `window_count()`, `windows()`
  - `is_last_pane(pane_id) -> bool`
  - Also: `alloc_tab_id()`, `alloc_window_id()` (embedded allocators)
- [x] Verify: all session methods that take an ID return `Option`, never panic
      on missing IDs (impl-hygiene.md: "No panics on user input")
- [x] Verify: `cargo build --target x86_64-pc-windows-gnu` succeeds
      (new types exist, nothing uses them yet)

---

## 01.3 Completion Checklist

- [x] Flat mux API documented with clear "stays" / "moves" / "deletes" lists
- [x] GUI session types compile in `oriterm`
- [x] No behavioral changes — existing code still uses mux types
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes (2 pre-existing flaky contract tests excluded)

**Exit Criteria:** `oriterm/src/session/` module exists with `Tab`, `Window`,
`TabId`, `WindowId`, `SessionRegistry` types that compile. The mux is
untouched. All builds and tests green.
