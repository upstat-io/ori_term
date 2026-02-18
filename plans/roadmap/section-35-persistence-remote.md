---
section: 35
title: Session Persistence + Remote Domains
status: not-started
tier: 7A
goal: Session save/load with crash recovery, scrollback archiving, full SshDomain and WslDomain implementations for remote shell spawning
sections:
  - id: "35.1"
    title: Session Save + Load
    status: not-started
  - id: "35.2"
    title: Crash Recovery
    status: not-started
  - id: "35.3"
    title: Scrollback Archive
    status: not-started
  - id: "35.4"
    title: SshDomain
    status: not-started
  - id: "35.5"
    title: WslDomain Full Implementation
    status: not-started
  - id: "35.6"
    title: Section Completion
    status: not-started
---

# Section 35: Session Persistence + Remote Domains

**Status:** Not Started
**Goal:** Persist terminal sessions across daemon restarts and crashes. Archive scrollback to disk for unlimited history. Implement SSH and WSL domains for spawning shells on remote machines and WSL distributions.

**Crate:** `oriterm_mux` (persistence, domains)
**Dependencies:** Section 34 (daemon mode working)
**Prerequisite:** Section 34 complete.

**Inspired by:**
- tmux: session persistence is the killer feature — survives SSH disconnects
- WezTerm: SSH domain with remote mux, but NO session persistence (we add this)
- Zellij: session management with layout restore
- Ghostty: no persistence or remote domains (we go beyond)

**Key differentiator:** WezTerm has no session persistence — close the GUI, lose your sessions. tmux has persistence but no native GUI. ori_term combines both: native GPU-rendered terminal with sessions that survive restarts, crashes, and reboots.

---

## 35.1 Session Save + Load

Persist the complete mux state (tabs, panes, splits, window positions) to disk. On daemon restart, restore the layout. Shell processes must be re-spawned (OS doesn't preserve PTYs across process boundaries), but the layout and metadata are preserved.

**File:** `oriterm_mux/src/persistence/session.rs`

- [ ] `SessionSnapshot` — serializable mux state:
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct SessionSnapshot {
      pub version: u32,
      pub timestamp: u64,
      pub windows: Vec<WindowSnapshot>,
  }

  #[derive(Serialize, Deserialize)]
  pub struct WindowSnapshot {
      pub id: WindowId,
      pub position: Option<(i32, i32)>,
      pub size: Option<(u32, u32)>,
      pub tabs: Vec<TabSnapshot>,
      pub active_tab: usize,
  }

  #[derive(Serialize, Deserialize)]
  pub struct TabSnapshot {
      pub id: TabId,
      pub title: String,
      pub tree: SplitTreeSnapshot,
      pub floating: Vec<FloatingPaneSnapshot>,
      pub active_pane: PaneId,
  }

  #[derive(Serialize, Deserialize)]
  pub struct PaneSnapshot {
      pub id: PaneId,
      pub domain_id: DomainId,
      pub cwd: Option<String>,
      pub title: String,
      pub shell: Option<String>,
      pub env: Vec<(String, String)>,
  }
  ```
- [ ] Save:
  - [ ] `save_session(mux: &InProcessMux, path: &Path) -> Result<()>`
  - [ ] Serialize `SessionSnapshot` to JSON (human-readable for debugging)
  - [ ] Write atomically: write to `.tmp`, then rename (prevents corruption on crash)
  - [ ] Auto-save interval: every 30 seconds (configurable)
  - [ ] Save on clean daemon shutdown
- [ ] Load:
  - [ ] `load_session(path: &Path) -> Result<SessionSnapshot>`
  - [ ] Deserialize JSON
  - [ ] Validate: check all referenced IDs are consistent
- [ ] Restore:
  - [ ] `restore_session(mux: &mut InProcessMux, snapshot: SessionSnapshot) -> Result<()>`
  - [ ] Recreate windows and tabs with same layout structure
  - [ ] For each pane: spawn a fresh shell in the saved CWD via the saved domain
  - [ ] Restore split tree and floating layer
  - [ ] Restore window positions and sizes
  - [ ] **Note:** shell history, running processes, and terminal content are NOT restored (those are in the scrollback archive, Section 35.3)
- [ ] Session file location:
  - [ ] `$XDG_STATE_HOME/oriterm/sessions/<session-id>.json` (Linux)
  - [ ] `%LOCALAPPDATA%\oriterm\sessions\<session-id>.json` (Windows)

**Tests:**
- [ ] Save → load roundtrip: snapshot serializes and deserializes identically
- [ ] Atomic write: crash during save doesn't corrupt existing file
- [ ] Restore: correct number of windows, tabs, panes created
- [ ] Restore: split tree layout matches snapshot
- [ ] Restore: CWD passed to new shell processes
- [ ] Auto-save: fires every 30 seconds

---

## 35.2 Crash Recovery

Detect unclean daemon shutdown and offer to restore the last saved session. Distinguish between intentional exit (user closed all windows) and crash (daemon killed/OOM/panic).

**File:** `oriterm_mux/src/persistence/recovery.rs`

- [ ] Crash detection:
  - [ ] On daemon start: check for stale PID file (daemon not running but PID file exists)
  - [ ] On daemon start: check for session file with `is_clean_shutdown: false`
  - [ ] Clean shutdown sets `is_clean_shutdown: true` in the session file before exiting
- [ ] Recovery flow:
  1. Daemon starts, detects stale session
  2. Reads session snapshot
  3. Prompts user (via the first connecting GUI client): "Restore previous session?"
  4. If yes: call `restore_session()` — windows, tabs, layout restored
  5. If no: delete stale session file, start fresh
- [ ] Auto-recovery mode (configurable):
  - [ ] `restore_on_crash = "ask" | "always" | "never"` — default "ask"
  - [ ] "always": silently restore without prompting
- [ ] Clean shutdown protocol:
  - [ ] `exit_app()` → set `is_clean_shutdown: true` → save session → stop daemon
  - [ ] All clients disconnect: daemon saves session, marks clean, waits for `--persist` or exits

**Tests:**
- [ ] Clean shutdown: session file has `is_clean_shutdown: true`
- [ ] Crash: session file has `is_clean_shutdown: false`
- [ ] Recovery: stale session detected, restored correctly
- [ ] Skip recovery: stale session deleted on "no"
- [ ] Auto-recovery: "always" mode restores without prompt

---

## 35.3 Scrollback Archive

Archive scrollback buffer to disk when panes are closed or when scrollback exceeds a threshold. Enables unlimited scrollback without unbounded memory.

**File:** `oriterm_mux/src/persistence/scrollback.rs`

- [ ] `ScrollbackArchive`:
  - [ ] Per-pane scrollback archive file
  - [ ] Format: bincode + zstd compressed rows
  - [ ] Write: append-only — new rows added to end of file
  - [ ] Read: seek to offset, decompress, return rows
- [ ] Archive triggers:
  - [ ] Scrollback exceeds `max_scrollback_memory` (e.g., 50MB per pane): oldest rows archived to disk
  - [ ] Pane closed: entire scrollback archived (for "undo close pane" feature)
  - [ ] Session save: scrollback state checkpointed
- [ ] Memory-mapped reading (optional):
  - [ ] Use `mmap` for reading archived scrollback — OS manages page cache
  - [ ] Scrolling through archived content: transparent read from disk
- [ ] Archive location:
  - [ ] `$XDG_STATE_HOME/oriterm/scrollback/<pane-id>.scrollback` (Linux)
  - [ ] `%LOCALAPPDATA%\oriterm\scrollback\<pane-id>.scrollback` (Windows)
- [ ] Cleanup:
  - [ ] Archives for closed panes: kept for 7 days (configurable), then deleted
  - [ ] Total archive size limit: 1GB (configurable), oldest archives pruned

**Tests:**
- [ ] Archive: write rows to file, read back identically
- [ ] Compression: archived size significantly smaller than raw
- [ ] Overflow archive: exceeding `max_scrollback_memory` triggers archive
- [ ] Close pane: scrollback persisted to disk
- [ ] Cleanup: old archives deleted after retention period

---

## 35.4 SshDomain

Spawn terminal panes on remote machines over SSH. The mux daemon manages the SSH connections; the GUI just renders the output.

**File:** `oriterm_mux/src/domain/ssh.rs`

**Reference:** WezTerm `wezterm-mux-server-impl/src/domain/ssh.rs`

- [ ] `SshDomain`:
  - [ ] Implements `Domain` trait
  - [ ] `SshDomain::new(id: DomainId, config: SshConfig) -> Self`
  - [ ] `SshConfig`: `host`, `port`, `user`, `identity_file`, `proxy_command`
- [ ] SSH connection management:
  - [ ] Use `openssh` or `thrussh` crate for SSH protocol
  - [ ] One SSH connection per domain, multiplexed channels per pane
  - [ ] Connection keepalive: `ServerAlive` interval
  - [ ] Reconnect on disconnect: attempt reconnect every 5 seconds, notify user
- [ ] `spawn_pane(config: SpawnConfig) -> Result<PaneId>`:
  - [ ] Open new SSH channel on existing connection
  - [ ] Request PTY allocation on the channel
  - [ ] Start shell (respecting `config.shell` or remote default)
  - [ ] Set `TERM=xterm-256color`, `TERM_PROGRAM=oriterm`
  - [ ] CWD: `cd <cwd> && exec $SHELL` (if CWD provided)
  - [ ] Create `Pane` with SSH channel reader/writer instead of local PTY
- [ ] SSH agent forwarding (configurable)
- [ ] X11 forwarding (configurable, off by default)
- [ ] Config:
  ```toml
  [[ssh_domains]]
  name = "dev-server"
  host = "dev.example.com"
  user = "eric"
  identity_file = "~/.ssh/id_ed25519"
  ```

**Tests:**
- [ ] `SshDomain` implements `Domain` trait
- [ ] Config parsing: host, port, user, identity
- [ ] Connection: SSH handshake succeeds (integration test with local SSH server)
- [ ] Spawn pane: shell runs on remote, output reaches local mux
- [ ] Reconnect: simulated disconnect → reconnect → panes resume

---

## 35.5 WslDomain Full Implementation

Full WSL domain implementation — spawn shells in any installed WSL distribution. Upgrades the stub from Section 30.2.

**File:** `oriterm/src/domain/wsl.rs`

**Reference:** WezTerm `wezterm-gui/src/domain/wsl.rs`, Section 30.2 stub

- [ ] `WslDomain` full implementation:
  - [ ] `WslDomain::new(id: DomainId, distro: String) -> Self`
  - [ ] Auto-detect installed distributions: `wsl.exe --list --quiet`
  - [ ] `can_spawn()`: verify distro is installed and running
  - [ ] `spawn_pane(config)`:
    - [ ] Spawn `wsl.exe -d <distro> --cd <cwd> -- <shell>` via `portable-pty`
    - [ ] Map Windows paths to WSL paths for CWD: `\\wsl$\<distro>\home\...` ↔ `/home/...`
    - [ ] Set `TERM=xterm-256color`, `WSLENV=TERM_PROGRAM`
  - [ ] Environment bridging: `WSLENV` for passing env vars across boundary
- [ ] Auto-discovery:
  - [ ] On startup: enumerate WSL distributions, create a `WslDomain` for each
  - [ ] Default domain: the default WSL distribution (`wsl.exe --list --verbose` → look for `*`)
- [ ] Config:
  ```toml
  [[wsl_domains]]
  name = "Ubuntu"
  distro = "Ubuntu-24.04"
  default_shell = "/bin/zsh"
  ```
- [ ] Path mapping utilities:
  - [ ] `win_to_wsl(win_path: &str, distro: &str) -> String` — `C:\Users\...` → `/mnt/c/Users/...`
  - [ ] `wsl_to_win(wsl_path: &str, distro: &str) -> String` — `/home/...` → `\\wsl$\<distro>\home\...`

**Tests:**
- [ ] Auto-detect: lists installed WSL distributions
- [ ] Path mapping: Windows ↔ WSL path conversion roundtrip
- [ ] `can_spawn()`: returns true for installed distro, false for missing
- [ ] Spawn pane: shell runs in WSL, output reaches local mux (integration test)
- [ ] CWD mapping: Windows CWD correctly mapped to WSL path

---

## 35.6 Section Completion

- [ ] All 35.1–35.5 items complete
- [ ] Session persistence: save/load/restore with atomic writes
- [ ] Crash recovery: detects unclean shutdown, offers restore
- [ ] Scrollback archive: unlimited scrollback via disk archiving
- [ ] SshDomain: spawn panes on remote machines over SSH
- [ ] WslDomain: spawn panes in any installed WSL distribution
- [ ] `cargo build --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings
- [ ] `cargo test` — all tests pass
- [ ] **Persistence test**: save session, kill daemon, restart → layout restored
- [ ] **Crash test**: kill -9 daemon → restart → crash recovery prompts
- [ ] **Scrollback test**: 100K lines of output → scrollback archived → searchable
- [ ] **SSH test**: connect to remote, spawn pane, type commands (integration)
- [ ] **WSL test**: spawn pane in Ubuntu, verify CWD mapping (integration)

**Exit Criteria:** Terminal sessions survive daemon restarts and crashes. Scrollback is unlimited via disk archiving. SSH and WSL domains allow spawning shells on remote machines and WSL distributions. ori_term now has persistence that WezTerm lacks and native GUI that tmux lacks — the best of both worlds.
