---
section: 34
title: IPC Protocol + Daemon Mode
status: not-started
tier: 7A
goal: Wire protocol for mux server/client communication, MuxServer daemon, OutputCoalescer for push-based rendering, MuxClient for GUI, auto-start daemon
sections:
  - id: "34.1"
    title: Wire Protocol
    status: not-started
  - id: "34.2"
    title: MuxServer Daemon
    status: not-started
  - id: "34.3"
    title: OutputCoalescer
    status: not-started
  - id: "34.4"
    title: MuxClient + Auto-Start
    status: not-started
  - id: "34.5"
    title: Section Completion
    status: not-started
---

# Section 34: IPC Protocol Hardening + Advanced Coalescing

**Status:** Not Started
**Goal:** Harden the IPC protocol established in Section 44 with compression, version negotiation, advanced output coalescing, and forward compatibility. This is polish on top of the working daemon built in Section 44.

**NOTE:** The core daemon, IPC protocol, MuxClient, MuxBackend trait, and cross-process tab migration are built in **Section 44 (Multi-Process Window Architecture)**. This section adds production hardening: zstd compression for large payloads, version negotiation for forward compatibility, tiered output coalescing for optimal rendering latency, and reconnection resilience.

**Crate:** `oriterm_mux` (protocol, server, client), `oriterm` (client integration)
**Dependencies:** Section 44 (multi-process window architecture working)
**Prerequisite:** Section 44 complete.

**Inspired by:**
- WezTerm: mux server with SSH domains, codec protocol, poll-based rendering (140ms — we beat this)
- tmux: server/client architecture, session persistence across terminal restarts
- Zellij: server mode with WASM plugin isolation

**Key improvement over WezTerm:** Push-based rendering with 1ms coalesce (vs WezTerm's 140ms poll interval). The server pushes dirty pane notifications to clients; clients don't poll. This gives near-local responsiveness even over the IPC boundary.

---

## 34.1 Wire Protocol

Binary protocol for communication between the mux daemon and GUI clients. Designed for low latency, low overhead, and forward compatibility.

**File:** `oriterm_mux/src/protocol.rs`, `oriterm_mux/src/protocol/codec.rs`

- [ ] Frame format (15-byte header):
  ```
  ┌──────────┬──────────┬──────────┬──────────────────────┐
  │ magic(2) │ ver(1)   │ type(2)  │ payload_len(4)       │
  ├──────────┴──────────┴──────────┴──────────────────────┤
  │ flags(2)            │ seq(4)                           │
  ├─────────────────────┴──────────────────────────────────┤
  │ payload (bincode-encoded, optionally zstd-compressed)  │
  └────────────────────────────────────────────────────────┘
  ```
  - [ ] Magic: `0x4F54` ("OT" for OriTerm)
  - [ ] Version: protocol version (starts at 1)
  - [ ] Type: message type ID (u16)
  - [ ] Payload length: u32 (max 16MB per frame)
  - [ ] Flags: `COMPRESSED = 0x01` (payload is zstd-compressed), `RESPONSE = 0x02`
  - [ ] Sequence: u32 request ID for request/response correlation
- [ ] Serialization: `bincode` for payload encoding (fast, compact)
- [ ] Compression: `zstd` (level 1) for payloads > 4KB — fast compression, ~3× ratio for terminal output
- [ ] Version negotiation:
  - [ ] Client sends `Hello { version: u8, features: u64 }` on connect
  - [ ] Server responds `HelloAck { version: u8, features: u64 }` — negotiated feature set
  - [ ] Incompatible versions: server returns `VersionMismatch` and closes
- [ ] Message types (request → response pairs):
  - [ ] `SpawnPane(SpawnConfig) → PaneSpawned(PaneId)` — create a new pane
  - [ ] `ClosePaneReq(PaneId) → PaneClosed(PaneId)` — close a pane
  - [ ] `SplitPaneReq { tab_id, pane_id, direction, config } → PaneSplit { new_pane_id, tree }` — split
  - [ ] `CreateTab(WindowId, SpawnConfig) → TabCreated(TabId)` — new tab
  - [ ] `CloseTab(TabId) → TabClosed(TabId)` — close tab
  - [ ] `Input(PaneId, Vec<u8>)` → (no response, fire-and-forget) — keyboard input
  - [ ] `Resize(PaneId, u16, u16)` → (no response) — PTY resize
  - [ ] `GetPaneContent(PaneId) → PaneContent { cells, cursor, palette }` — full pane snapshot
  - [ ] `Subscribe(PaneId)` → stream of `PaneOutput { pane_id, dirty_rows }` — push notifications
  - [ ] `Unsubscribe(PaneId)` → (ack)
  - [ ] `ListPanes → PaneList(Vec<PaneEntry>)`
  - [ ] `ListTabs → TabList(Vec<MuxTab>)`
  - [ ] `GetLayout(TabId) → LayoutSnapshot { tree, floating, focused }`
- [ ] Transport: Unix domain socket (Linux/macOS), named pipe (Windows)
  - [ ] Socket path: `$XDG_RUNTIME_DIR/oriterm-mux.<pid>.sock` (Linux)
  - [ ] Named pipe: `\\.\pipe\oriterm-mux.<pid>` (Windows)

**Tests:**
- [ ] Frame encode/decode roundtrip: all message types
- [ ] Version negotiation: compatible versions → success
- [ ] Version mismatch: server rejects incompatible client
- [ ] Compression: payloads > 4KB compressed with zstd, < 4KB uncompressed
- [ ] Sequence correlation: request seq matches response seq
- [ ] Max payload: 16MB limit enforced

---

## 34.2 MuxServer Daemon

The `oriterm-mux` daemon process. Keeps all terminal sessions alive. Accepts connections from GUI clients. Routes pane output to subscribed clients.

**File:** `oriterm_mux/src/server.rs`, `oriterm_mux/src/server/connection.rs`

- [ ] `MuxServer` struct:
  - [ ] `mux: InProcessMux` — the actual mux state
  - [ ] `listener: UnixListener` (or named pipe listener on Windows)
  - [ ] `connections: HashMap<ClientId, ClientConnection>`
  - [ ] `subscriptions: HashMap<PaneId, Vec<ClientId>>` — which clients want output for which panes
- [ ] `ClientConnection`:
  - [ ] `id: ClientId`
  - [ ] `stream: UnixStream` (or named pipe handle)
  - [ ] `codec: ProtocolCodec` — encode/decode frames
  - [ ] `subscribed_panes: HashSet<PaneId>`
- [ ] Server event loop (single-threaded async with `mio` or `tokio`):
  - [ ] Accept new connections
  - [ ] Read incoming messages from clients
  - [ ] Dispatch to `InProcessMux` methods
  - [ ] Poll `mux.event_rx` for pane events
  - [ ] Push notifications to subscribed clients
- [ ] Connection lifecycle:
  - [ ] Client connects → version negotiation → authenticated (no auth in v1, localhost only)
  - [ ] Client disconnects → unsubscribe all panes, but panes stay alive
  - [ ] All clients disconnect → daemon keeps running (sessions persist)
- [ ] Daemon lifecycle:
  - [ ] Start: `oriterm-mux --daemon` — background process, writes PID file
  - [ ] Stop: `oriterm-mux --stop` — sends shutdown signal
  - [ ] All panes exit naturally → daemon exits (configurable: `--persist` keeps running)
  - [ ] PID file: `$XDG_RUNTIME_DIR/oriterm-mux.pid`
- [ ] Shadow grid (for reconnection):
  - [ ] Server maintains last-known `RenderableContent` for each pane
  - [ ] On client `Subscribe(pane_id)`: send full `PaneContent` first (cold start), then push incremental updates
  - [ ] Enables instant display on reconnect — no waiting for shell to redraw

**Tests:**
- [ ] Server starts, accepts connection, version negotiates
- [ ] Client sends `SpawnPane` → server creates pane, returns PaneId
- [ ] Client subscribes to pane → receives `PaneOutput` notifications
- [ ] Client disconnects → panes stay alive
- [ ] New client connects → can `ListPanes` and see existing panes
- [ ] New client subscribes → receives full `PaneContent` snapshot
- [ ] All panes exit → server exits (unless `--persist`)

---

## 34.3 OutputCoalescer

The push-based rendering engine. Coalesces rapid pane output (e.g., `cat large_file.txt`) into batched notifications with configurable latency targets. This is what makes mux-mode rendering fast.

**File:** `oriterm_mux/src/server/coalescer.rs`

**Reference:** WezTerm's 140ms poll (we beat this with push + coalesce)

- [ ] `OutputCoalescer`:
  - [ ] Per-pane coalesce timer: 1ms default (configurable)
  - [ ] When pane produces output: start coalesce timer for that pane
  - [ ] When timer fires: push `PaneOutput` notification to subscribed clients
  - [ ] If more output arrives during coalesce window: extend the batch, do NOT reset timer
  - [ ] Result: at most one notification per pane per millisecond
- [ ] Tiered coalescing:
  - [ ] **Focused pane**: 1ms coalesce — near-instant rendering
  - [ ] **Visible unfocused pane**: 16ms coalesce (~60 FPS) — smooth but efficient
  - [ ] **Hidden pane** (scrolled offscreen, in background tab): 100ms coalesce — low overhead
- [ ] `PaneOutput` notification content:
  - [ ] `pane_id: PaneId`
  - [ ] `dirty_rows: Option<Vec<u16>>` — which rows changed (for incremental rendering), `None` = full redraw
  - [ ] `cursor_changed: bool` — cursor position or shape changed
  - [ ] `title_changed: Option<String>` — new title if changed during this batch
- [ ] Backpressure:
  - [ ] If client is slow to consume: drop intermediate `PaneOutput` notifications, keep only the latest
  - [ ] Client always gets the most recent state, never stale data
  - [ ] `PaneOutput` is a "latest value" channel, not a queue

**Tests:**
- [ ] Single output: notification fires after 1ms
- [ ] Burst output: multiple outputs within 1ms window → single notification
- [ ] Tiered coalescing: focused pane at 1ms, visible at 16ms, hidden at 100ms
- [ ] Backpressure: slow client gets latest state, not stale intermediate states
- [ ] Focus change: pane promoted from hidden → focused gets tighter coalesce

---

## 34.4 MuxClient + Auto-Start

The GUI's connection to the mux daemon. `MuxClient` implements the same API as `InProcessMux` so the App doesn't care whether it's local or daemon mode.

**File:** `oriterm_mux/src/client.rs`, `oriterm/src/app/mod.rs`

- [ ] `MuxClient` struct:
  - [ ] `stream: UnixStream` (or named pipe)
  - [ ] `codec: ProtocolCodec`
  - [ ] `pending: HashMap<u32, oneshot::Sender<Response>>` — pending request/response
  - [ ] `next_seq: u32` — sequence number allocator
- [ ] `MuxClient` API (mirrors `InProcessMux`):
  - [ ] `spawn_pane(&mut self, tab_id, config) -> Result<PaneId>`
  - [ ] `close_pane(&mut self, pane_id) -> Result<()>`
  - [ ] `split_pane(&mut self, tab_id, pane_id, dir, config) -> Result<PaneId>`
  - [ ] `create_tab(&mut self, window_id, config) -> Result<TabId>`
  - [ ] `close_tab(&mut self, tab_id) -> Result<()>`
  - [ ] `send_input(&mut self, pane_id, bytes: &[u8])` — fire-and-forget
  - [ ] `resize_pane(&mut self, pane_id, cols, rows)` — fire-and-forget
  - [ ] `subscribe(&mut self, pane_id) -> Result<Receiver<PaneOutput>>`
  - [ ] `get_layout(&mut self, tab_id) -> Result<LayoutSnapshot>`
- [ ] `MuxBackend` trait — unified API over `InProcessMux` and `MuxClient`:
  - [ ] Both implement the same trait
  - [ ] App uses `Box<dyn MuxBackend>` — transparent switching between modes
- [ ] Auto-start daemon:
  - [ ] On app launch: try to connect to existing daemon socket
  - [ ] If no daemon running: fork `oriterm-mux --daemon`, wait for socket, connect
  - [ ] If connection fails after 3 retries: fall back to `InProcessMux` (graceful degradation)
  - [ ] Config option: `mux_mode = "auto" | "local" | "server"` — default "auto"
- [ ] Reconnection:
  - [ ] If daemon connection drops: attempt reconnect every 500ms (3 attempts)
  - [ ] On reconnect: re-subscribe to all previously subscribed panes
  - [ ] Shadow grid enables instant display — no blank screen during reconnect

**Tests:**
- [ ] `MuxClient` API matches `InProcessMux` — compile-time trait check
- [ ] Round-trip: spawn pane via client → server creates pane → client gets PaneId
- [ ] Subscribe: client receives push notifications for pane output
- [ ] Auto-start: client starts daemon if not running, connects
- [ ] Fallback: if daemon unavailable, falls back to InProcessMux
- [ ] Reconnection: simulated disconnect → reconnect → re-subscribe

---

## 34.5 Section Completion

- [ ] All 34.1–34.4 items complete
- [ ] Wire protocol: 15-byte header, bincode + zstd, version negotiation
- [ ] MuxServer: accepts connections, routes messages, pushes output
- [ ] OutputCoalescer: tiered 1ms/16ms/100ms coalescing, backpressure handling
- [ ] MuxClient: same API as InProcessMux, transparent backend switching
- [ ] Auto-start daemon: seamless to user, fallback to in-process
- [ ] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test` — all tests pass
- [ ] **Latency test**: keystroke → screen update < 5ms through daemon
- [ ] **Throughput test**: `cat large_file.txt` renders smoothly, no dropped frames
- [ ] **Reconnection test**: kill GUI, relaunch → sessions restored instantly
- [ ] **Multi-client test**: two GUI windows connected to same daemon

**Exit Criteria:** Full server/client architecture. The daemon keeps sessions alive across GUI restarts. Push-based rendering with 1ms coalesce beats WezTerm's 140ms poll. Transparent backend switching lets the App work identically in local and daemon modes. Auto-start and graceful fallback make the daemon invisible to users who don't need it.
