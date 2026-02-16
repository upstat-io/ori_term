---
section: 4
title: PTY + Event Loop
status: complete
tier: 1
goal: Spawn a shell via ConPTY, wire the reader thread, and verify end-to-end I/O through Term<EventProxy>
sections:
  - id: "4.1"
    title: Binary Crate Setup
    status: complete
  - id: "4.2"
    title: TabId + TermEvent Types
    status: complete
  - id: "4.3"
    title: PTY Spawning
    status: complete
  - id: "4.4"
    title: Message Channel
    status: complete
  - id: "4.5"
    title: EventProxy (EventListener impl)
    status: complete
  - id: "4.6"
    title: Notifier (Notify impl)
    status: complete
  - id: "4.7"
    title: PTY Reader Thread
    status: complete
  - id: "4.8"
    title: Tab Struct
    status: complete
  - id: "4.9"
    title: End-to-End Verification
    status: complete
  - id: "4.10"
    title: Section Completion
    status: complete
---

# Section 04: PTY + Event Loop

**Status:** 📋 Planned
**Goal:** Spawn a real shell, wire PTY I/O through the reader thread, and process shell output through `Term<EventProxy>`. This is the first time terminal emulation runs against a live shell process.

**Crate:** `oriterm` (binary)
**Dependencies:** `oriterm_core`, `portable-pty`, `winit` (for `EventLoopProxy` type — window not created yet)

---

## 4.1 Binary Crate Setup

Set up the `oriterm/` binary crate in the workspace.

- [x] Create `oriterm/` directory with `Cargo.toml` and `src/main.rs`
  - [x] `Cargo.toml`: name = `oriterm`, edition = 2024, same lint config
  - [x] Dependencies: `oriterm_core = { path = "../oriterm_core" }`, all GUI/platform deps from current root Cargo.toml
  - [x] `[[bin]]` name = `oriterm`, path = `src/main.rs`
- [x] Move existing `src/main.rs` → `oriterm/src/main.rs`
- [x] Move `build.rs` → `oriterm/build.rs`
- [x] Move `assets/` reference in build.rs (update paths)
- [x] Update workspace root `Cargo.toml`:
  - [x] `[workspace]` with `members = ["oriterm_core", "oriterm"]`
  - [x] Remove `[[bin]]` and `[dependencies]` from root (they live in crate-level Cargo.tomls now)
- [x] Verify: `cargo build --target x86_64-pc-windows-gnu` builds both crates
- [x] Verify: `cargo build -p oriterm --target x86_64-pc-windows-gnu` builds the binary

---

## 4.2 TabId + TermEvent Types

Newtype for tab identity and the event type for cross-thread communication.

**File:** `oriterm/src/tab.rs` (initial, will grow)

- [x] `TabId` newtype
  - [x] `pub struct TabId(u64)` (inner field private — construction only via `next()`)
  - [x] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`
  - [x] `TabId::next() -> Self` — atomic counter for unique IDs
    - [x] Use `std::sync::atomic::AtomicU64` static counter
- [x] `TermEvent` enum — winit user event type
  - [x] `Terminal { tab_id: TabId, event: oriterm_core::Event }` — event from terminal library
  - [x] Derive: `Debug`
- [x] **Tests**:
  - [x] `TabId::next()` generates unique IDs
  - [x] `TermEvent` variants can be constructed

---

## 4.3 PTY Spawning

Create a PTY and spawn the default shell.

**File:** `oriterm/src/pty/spawn.rs`

- [x] `spawn_pty(config: &PtyConfig) -> io::Result<PtyHandle>` (richer API than planned `spawn_shell`)
  - [x] Call `portable_pty::native_pty_system()`
  - [x] `pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`
  - [x] `CommandBuilder::new(shell)` with `default_shell()` detection
  - [x] `pair.slave.spawn_command(cmd)` — spawn child process
  - [x] Drop `pair.slave` (reader gets EOF when child exits)
  - [x] Clone reader: `pair.master.try_clone_reader()`
  - [x] Take writer: `pair.master.take_writer()`
  - [x] Return `PtyHandle` containing reader, writer, master, child
- [x] `PtyHandle` struct
  - [x] Fields:
    - `reader: Box<dyn Read + Send>` — PTY output (read by reader thread)
    - `writer: Box<dyn Write + Send>` — PTY input (written by Notifier)
    - `master: Box<dyn portable_pty::MasterPty + Send>` — for resize
    - `child: Box<dyn portable_pty::Child + Send + Sync>` — child process handle
  - [x] `PtyHandle::resize(&self, rows: u16, cols: u16) -> io::Result<()>`
    - [x] `self.master.resize(PtySize { rows, cols, ... })`
- [x] `mod.rs`: `pub mod spawn;` re-export `PtyHandle`, `spawn_pty`
- [x] **Tests**:
  - [x] Spawning a shell succeeds (integration test)
  - [x] Reader and writer are valid (not None)

---

## 4.4 Message Channel

Messages from the main thread to the PTY reader thread.

**File:** `oriterm/src/pty/mod.rs`

- [x] `Msg` enum — commands sent to PTY thread
  - [x] `Input(Vec<u8>)` — bytes to write to PTY
  - [x] `Resize { rows: u16, cols: u16 }` — resize the PTY
  - [x] `Shutdown` — gracefully stop the reader thread
- [x] Use `std::sync::mpsc::channel::<Msg>()` — unbounded channel
  - [x] Sender held by `Notifier` (main thread side)
  - [x] Receiver consumed by reader thread

---

## 4.5 EventProxy (EventListener impl)

Bridges terminal events to the winit event loop.

**File:** `oriterm/src/tab.rs`

- [x] `EventProxy` struct
  - [x] Fields:
    - `proxy: winit::event_loop::EventLoopProxy<TermEvent>` — winit's thread-safe event sender
    - `tab_id: TabId`
  - [x] `impl oriterm_core::EventListener for EventProxy`
    - [x] `fn send_event(&self, event: oriterm_core::Event)`
      - [x] `let _ = self.proxy.send_event(TermEvent::Terminal { tab_id: self.tab_id, event });`
      - [x] Silently ignore send errors (window may have closed)
- [x] `EventProxy` must be `Send + 'static` (required by `EventListener` bound)

---

## 4.6 Notifier (Notify impl)

Sends input bytes and commands to the PTY reader thread.

**File:** `oriterm/src/tab.rs`

- [x] `Notifier` struct
  - [x] Fields:
    - `tx: std::sync::mpsc::Sender<Msg>` — channel sender
  - [x] `Notifier::notify(&self, bytes: &[u8])` — send bytes (skips empty)
    - [x] `let _ = self.tx.send(Msg::Input(bytes.to_vec()));`
  - [x] `Notifier::resize(&self, rows: u16, cols: u16)`
    - [x] `let _ = self.tx.send(Msg::Resize { rows, cols });`
  - [x] `Notifier::shutdown(&self)`
    - [x] `let _ = self.tx.send(Msg::Shutdown);`

---

## 4.7 PTY Reader Thread

The dedicated thread that reads PTY output, parses VTE, and updates terminal state.

**File:** `oriterm/src/pty/event_loop.rs`

- [x] `PtyEventLoop` struct
  - [x] Fields:
    - `terminal: Arc<oriterm_core::FairMutex<oriterm_core::Term<T>>>` — shared terminal state (generic over `EventListener`)
    - `reader: Box<dyn Read + Send>` — PTY read handle
    - `writer: Box<dyn Write + Send>` — PTY write handle
    - `rx: std::sync::mpsc::Receiver<Msg>` — command receiver
    - `pty_master: Box<dyn portable_pty::MasterPty + Send>` — for resize
    - `processor: vte::ansi::Processor` — VTE parser state machine
  - [x] `PtyEventLoop::new(...)` — constructor, takes all handles
  - [x] `PtyEventLoop::spawn(self) -> JoinHandle<()>` — start the reader thread
    - [x] `std::thread::Builder::new().name("pty-reader".into()).spawn(move || self.run())`
  - [x] `fn run(mut self)` — main loop: drain commands → blocking read → parse in bounded chunks → EOF/error exits
  - [x] `fn parse_pty_output(&mut self, data: &[u8])` — lock-bounded VTE parsing in 64KB chunks
  - [x] `fn process_commands(&mut self) -> bool` — drain rx:
    - [x] `Msg::Input(bytes)` → `self.writer.write_all(&bytes)`
    - [x] `Msg::Resize { rows, cols }` → `self.resize_pty(rows, cols)` (PTY master only; Term resize is Section 12)
    - [x] `Msg::Shutdown` → return false (breaks loop)
  - [x] `fn resize_pty(&self, rows, cols)` — resize PTY master via `portable_pty::PtySize`
  - [x] Read buffer: `vec![0u8; 65536]` (64KB, heap-allocated to avoid clippy::large_stack_arrays)
  - [x] Max locked parse: `MAX_LOCKED_PARSE = 0x1_0000` (64KB) per lock acquisition, then release and re-lock
    - [x] Prevents holding lock for too long on large output bursts
- [x] **Thread safety**:
  - [x] PTY reader thread holds `FairMutex` lock only during `processor.advance()` (microseconds to low ms)
  - [x] Uses `lease()` → `lock_unfair()` pattern from Alacritty
  - [x] Releases lock between read batches
- [x] `PtyHandle::take_master()` — added to `spawn.rs` so master can be handed to PtyEventLoop
- [x] **Tests** (mock-based with `std::io::pipe()` + `MockMaster`, no real PTY):
  - [x] `shutdown_on_reader_eof` — drop pipe write end → EOF → thread exits
  - [x] `processes_pty_output_into_terminal` — write bytes to pipe → VTE parses into grid
  - [x] `processes_channel_input` — `Msg::Input` forwarded to PTY writer
  - [x] `read_buffer_size_is_64kb` — constant check
  - [x] `max_locked_parse_is_64kb` — constant check

---

## 4.8 Tab Struct

Owns all per-tab state: terminal, PTY handles, reader thread.

**File:** `oriterm/src/tab.rs`

- [x] `Tab` struct
  - [x] Fields:
    - `id: TabId`
    - `terminal: Arc<oriterm_core::FairMutex<oriterm_core::Term<EventProxy>>>`
    - `notifier: Notifier` — send input/resize/shutdown to PTY thread
    - `reader_thread: Option<JoinHandle<()>>` — reader thread handle
    - `pty: PtyHandle` — child process lifecycle (reader/writer/control taken)
    - `title: String` — last known title (updated from Event::Title)
    - `has_bell: bool` — bell badge (cleared on focus)
  - [x] `Tab::new(id: TabId, rows: u16, cols: u16, scrollback: usize, proxy: EventLoopProxy<TermEvent>) -> io::Result<Self>`
    - [x] Spawn PTY via `pty::spawn_pty(&PtyConfig)`
    - [x] Create `EventProxy` with tab_id and proxy
    - [x] Create `Term::new(rows, cols, scrollback, event_proxy)`
    - [x] Wrap in `Arc<FairMutex<...>>`
    - [x] Create `(tx, rx)` channel
    - [x] Create `Notifier` with tx
    - [x] Create `PtyEventLoop` with terminal clone, reader, writer, rx, control
    - [x] Spawn reader thread: `event_loop.spawn()`
    - [x] Return Tab
  - [x] `Tab::write_input(&self, bytes: &[u8])` — send input to PTY via Notifier
  - [x] `Tab::resize(&self, rows: u16, cols: u16)` — resize PTY + terminal
  - [x] `Tab::terminal(&self) -> &Arc<FairMutex<Term<EventProxy>>>` — for renderer to lock + snapshot
  - [x] `impl Drop for Tab`
    - [x] Send `Msg::Shutdown` to reader thread
    - [x] Kill child process to unblock pending PTY read
    - [x] Join reader thread (with timeout via `is_finished()` poll loop)
    - [x] Reap child process

---

## 4.9 End-to-End Verification

At this point there's no window, but we can verify the full PTY → VTE → Term pipeline.

- [x] Temporary `main.rs` for verification:
  - [x] Create a winit `EventLoop` (needed for `EventLoopProxy`, even without a window)
  - [x] Create a Tab
  - [x] Send `"echo hello\r\n"` via `tab.write_input()`
  - [x] Wait briefly (100ms)
  - [x] Lock terminal, read grid, verify "hello" appears in grid cells
  - [x] Print verification result to log/stderr
  - [x] Exit
- [x] Verify thread lifecycle:
  - [x] Tab creation spawns reader thread
  - [x] Tab drop sends Shutdown and joins thread
  - [x] No thread leaks, no panics on drop
- [x] Verify FairMutex under load:
  - [x] Send rapid input while reader thread is processing
  - [x] Neither thread starves (both make progress)
- [x] Verify resize:
  - [x] Create tab at 80x24
  - [x] Resize to 120x40
  - [x] PTY dimensions updated, terminal grid resized

---

## 4.10 Section Completion

- [x] All 4.1–4.9 items complete
- [x] `cargo build -p oriterm --target x86_64-pc-windows-gnu` succeeds
- [x] Tab spawns shell, reader thread processes output into Term
- [x] Input sent via Notifier arrives at shell
- [x] Shutdown is clean (no thread leaks, no panics)
- [x] FairMutex prevents starvation under concurrent access
- [x] Resize works end-to-end (PTY + terminal grid)
- [x] No window yet — next section adds GUI

**Exit Criteria:** Live shell output is parsed through VTE into `Term<EventProxy>`. Input flows main thread → Notifier → channel → PTY. Reader thread is clean (proper lifecycle, lock discipline, no starvation). Ready for a window to render the terminal state.
