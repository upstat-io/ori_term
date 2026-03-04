//! Pane — the atomic per-shell unit in the mux model.
//!
//! Each `Pane` owns the full PTY ↔ terminal pipeline: a `Term<MuxEventProxy>`
//! wrapped in `Arc<FairMutex>`, the reader thread, and a `PaneNotifier` that
//! delivers keyboard input to the PTY. Lock-free atomics (`grid_dirty`,
//! `wakeup_pending`, `mode_cache`) allow the renderer and input handler to
//! query pane state without contending on the terminal lock.
//!
//! `Pane` is intentionally independent of `Tab` — the mux layer owns panes
//! directly. `Tab` will be replaced in Section 31/32.

mod mark_cursor;
mod selection;
mod shutdown;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::{DomainId, PaneId};
use oriterm_core::term::cwd_short_path;
use oriterm_core::{FairMutex, SearchState, Selection, StableRowIndex, Term};

pub use mark_cursor::MarkCursor;

use crate::mux_event::MuxEventProxy;
use crate::pty::{Msg, PtyControl, PtyHandle};

/// Sends input to the PTY and commands to the reader thread.
///
/// All writes flow through the `mpsc` channel to the PTY reader thread,
/// which owns the actual PTY writer. This prevents blocking the main
/// thread when the PTY kernel buffer is full (e.g. during flood output).
pub struct PaneNotifier {
    /// Channel sender for input and shutdown commands to the reader thread.
    tx: mpsc::Sender<Msg>,
}

impl PaneNotifier {
    /// Create a new notifier with a command channel to the reader thread.
    pub fn new(tx: mpsc::Sender<Msg>) -> Self {
        Self { tx }
    }

    /// Send raw bytes to the PTY (keyboard input, escape responses).
    ///
    /// Non-blocking — enqueues via the channel. The reader thread drains
    /// the queue and writes to the PTY fd.
    pub fn notify(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Err(e) = self.tx.send(Msg::Input(bytes.to_vec())) {
            log::warn!("PTY channel send failed: {e}");
        }
    }

    /// Request the reader thread to shut down.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}

/// Pre-built parts for constructing a [`Pane`].
///
/// Groups all parameters for `Pane::from_parts` to stay under the clippy
/// argument limit. Produced by `LocalDomain::spawn_pane`.
pub struct PaneParts {
    /// Unique pane identifier.
    pub id: PaneId,
    /// Which domain spawned this pane.
    pub domain_id: DomainId,
    /// Shared terminal state.
    pub terminal: Arc<FairMutex<Term<MuxEventProxy>>>,
    /// Input/shutdown sender.
    pub notifier: PaneNotifier,
    /// PTY control handle for resize.
    pub pty_control: PtyControl,
    /// Reader thread join handle.
    pub reader_thread: JoinHandle<()>,
    /// Writer thread join handle.
    pub writer_thread: JoinHandle<()>,
    /// PTY handle (child lifecycle).
    pub pty: PtyHandle,
    /// Grid dirty flag (lock-free).
    pub grid_dirty: Arc<AtomicBool>,
    /// Wakeup coalescing flag (lock-free).
    pub wakeup_pending: Arc<AtomicBool>,
    /// Mode bits cache (lock-free).
    pub mode_cache: Arc<AtomicU32>,
}

/// Owns all per-shell-session state: terminal, PTY handles, reader thread.
///
/// The atomic `Pane` unit in the mux model — one shell process, one grid,
/// one PTY connection. Created by `LocalDomain::spawn_pane`.
pub struct Pane {
    /// Unique pane identifier (from mux allocator).
    id: PaneId,
    /// Which domain spawned this pane.
    #[allow(dead_code, reason = "read when multi-domain routing is wired to App")]
    domain_id: DomainId,
    /// Shared terminal state (accessed by both render and PTY threads).
    terminal: Arc<FairMutex<Term<MuxEventProxy>>>,
    /// Sends input/shutdown to the PTY.
    notifier: PaneNotifier,
    /// PTY control handle for resize operations.
    pty_control: PtyControl,
    /// PTY reader thread join handle.
    reader_thread: Option<JoinHandle<()>>,
    /// PTY writer thread join handle.
    writer_thread: Option<JoinHandle<()>>,
    /// Spawned PTY (reader/writer/control taken; child remains for lifecycle).
    pty: PtyHandle,
    /// Set by reader thread when new content is available.
    grid_dirty: Arc<AtomicBool>,
    /// Coalesces wakeup events from the reader thread.
    wakeup_pending: Arc<AtomicBool>,
    /// Lock-free cache of `TermMode::bits()` for hot-path queries.
    mode_cache: Arc<AtomicU32>,
    /// Last known window title (from OSC 0/2).
    title: String,
    /// Icon name (from OSC 0/1) for tab icons.
    icon_name: Option<String>,
    /// Current working directory (from OSC 7).
    cwd: Option<String>,
    /// Whether the current title was explicitly set via OSC 0/2.
    /// When `false`, `effective_title()` prefers CWD-based title.
    has_explicit_title: bool,
    /// Duration of the last completed command (from OSC 133 C→D timing).
    last_command_duration: Option<std::time::Duration>,
    /// Bell indicator (set on bell event, cleared on focus).
    has_bell: bool,
    /// Active text selection, if any.
    selection: Option<Selection>,
    /// Mark mode cursor position (keyboard-driven selection).
    mark_cursor: Option<MarkCursor>,
    /// Active search state (query, matches, navigation).
    search: Option<SearchState>,
}

impl Pane {
    /// Construct a pane from pre-built parts.
    ///
    /// Called by `LocalDomain::spawn_pane` after setting up the PTY pipeline.
    pub fn from_parts(parts: PaneParts) -> Self {
        Self {
            id: parts.id,
            domain_id: parts.domain_id,
            terminal: parts.terminal,
            notifier: parts.notifier,
            pty_control: parts.pty_control,
            reader_thread: Some(parts.reader_thread),
            writer_thread: Some(parts.writer_thread),
            pty: parts.pty,
            grid_dirty: parts.grid_dirty,
            wakeup_pending: parts.wakeup_pending,
            mode_cache: parts.mode_cache,
            title: String::new(),
            icon_name: None,
            cwd: None,
            has_explicit_title: false,
            last_command_duration: None,
            has_bell: false,
            selection: None,
            mark_cursor: None,
            search: None,
        }
    }

    // -- Identity --

    /// Pane identity.
    #[allow(dead_code, reason = "used when pane CRUD is fully wired to App")]
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// Which domain spawned this pane.
    #[allow(dead_code, reason = "used when multi-domain routing is wired to App")]
    pub fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    // -- Lock-free accessors --

    /// Whether the pane's grid has new content to render.
    pub fn grid_dirty(&self) -> bool {
        self.grid_dirty.load(Ordering::Acquire)
    }

    /// Clear the grid dirty flag after rendering.
    pub fn clear_grid_dirty(&self) {
        self.grid_dirty.store(false, Ordering::Release);
    }

    /// Clear the wakeup pending flag after processing.
    pub fn clear_wakeup(&self) {
        self.wakeup_pending.store(false, Ordering::Release);
    }

    /// Current terminal mode bits (lock-free).
    ///
    /// Updated by the reader thread after each VTE chunk; read by the main
    /// thread for mouse reporting and cursor style without locking the terminal.
    pub fn mode(&self) -> u32 {
        self.mode_cache.load(Ordering::Acquire)
    }

    // -- Terminal access --

    /// Shared terminal state for rendering.
    pub fn terminal(&self) -> &Arc<FairMutex<Term<MuxEventProxy>>> {
        &self.terminal
    }

    // -- Title / CWD / Bell --

    /// Set the pane title (from OSC 0/2 via `MuxEvent::PaneTitleChanged`).
    pub fn set_title(&mut self, title: String) {
        self.has_explicit_title = !title.is_empty();
        self.title = title;
    }

    /// Icon name (from OSC 0/1) for tab icon detection.
    pub fn icon_name(&self) -> Option<&str> {
        self.icon_name.as_deref()
    }

    /// Set the icon name.
    pub fn set_icon_name(&mut self, name: String) {
        if name.is_empty() {
            self.icon_name = None;
        } else {
            self.icon_name = Some(name);
        }
    }

    /// Resolved display title with 3-source priority:
    /// 1. Explicit title from OSC 0/2.
    /// 2. Short path from CWD (last component).
    /// 3. Fallback to raw title (may be empty).
    pub fn effective_title(&self) -> &str {
        if self.has_explicit_title {
            return &self.title;
        }
        if let Some(ref cwd) = self.cwd {
            return cwd_short_path(cwd);
        }
        &self.title
    }

    /// Current working directory (from OSC 7).
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Set the current working directory (clears explicit title flag).
    pub fn set_cwd(&mut self, cwd: String) {
        self.has_explicit_title = false;
        self.cwd = Some(cwd);
    }

    /// Duration of the last completed command.
    #[allow(
        dead_code,
        reason = "read when command notification UI is wired to App"
    )]
    pub fn last_command_duration(&self) -> Option<std::time::Duration> {
        self.last_command_duration
    }

    /// Store the duration of a completed command.
    pub fn set_last_command_duration(&mut self, duration: std::time::Duration) {
        self.last_command_duration = Some(duration);
    }

    /// Whether the bell has fired since the pane was last focused.
    #[allow(dead_code, reason = "used when bell indicator is wired to App")]
    pub fn has_bell(&self) -> bool {
        self.has_bell
    }

    /// Clear the bell indicator (call when the pane gains focus).
    #[allow(dead_code, reason = "used when bell indicator is wired to App")]
    pub fn clear_bell(&mut self) {
        self.has_bell = false;
    }

    /// Set the bell indicator.
    pub fn set_bell(&mut self) {
        self.has_bell = true;
    }

    // -- Mark cursor --

    /// Whether mark mode is active.
    pub fn is_mark_mode(&self) -> bool {
        self.mark_cursor.is_some()
    }

    /// Current mark cursor position.
    pub fn mark_cursor(&self) -> Option<MarkCursor> {
        self.mark_cursor
    }

    /// Enter mark mode at the terminal cursor position.
    pub fn enter_mark_mode(&mut self) {
        if self.mark_cursor.is_some() {
            return;
        }
        self.scroll_to_bottom();
        let mc = {
            let term = self.terminal.lock();
            let g = term.grid();
            let cursor = g.cursor();
            let abs_row = g.scrollback().len() + cursor.line();
            let stable = StableRowIndex::from_absolute(g, abs_row);
            MarkCursor {
                row: stable,
                col: cursor.col().0,
            }
        };
        self.mark_cursor = Some(mc);
    }

    /// Exit mark mode.
    pub fn exit_mark_mode(&mut self) {
        self.mark_cursor = None;
    }

    /// Update the mark cursor position.
    pub fn set_mark_cursor(&mut self, cursor: MarkCursor) {
        self.mark_cursor = Some(cursor);
    }

    // -- I/O operations --

    /// Send raw bytes to the PTY.
    pub fn write_input(&self, bytes: &[u8]) {
        self.notifier.notify(bytes);
    }

    /// Scroll to the live terminal position.
    pub fn scroll_to_bottom(&self) {
        let mut term = self.terminal.lock();
        if term.grid().display_offset() > 0 {
            term.grid_mut().scroll_display(isize::MIN);
        }
    }

    /// Scroll the viewport by `delta` lines.
    pub fn scroll_display(&self, delta: isize) {
        self.terminal.lock().grid_mut().scroll_display(delta);
    }

    /// Resize the terminal grids (with reflow). Does NOT resize the PTY.
    pub fn resize_grid(&self, rows: u16, cols: u16) {
        self.terminal.lock().resize(rows as usize, cols as usize);
    }

    /// Resize the OS PTY handle, sending SIGWINCH to the shell.
    pub fn resize_pty(&self, rows: u16, cols: u16) {
        if let Err(e) = self.pty_control.resize(rows, cols) {
            log::warn!("PTY resize failed: {e}");
        }
    }

    // -- Prompt navigation --

    /// Scroll to the nearest prompt above the current viewport.
    ///
    /// Returns `true` if the viewport was scrolled.
    pub fn scroll_to_previous_prompt(&self) -> bool {
        self.terminal.lock().scroll_to_previous_prompt()
    }

    /// Scroll to the nearest prompt below the current viewport.
    ///
    /// Returns `true` if the viewport was scrolled.
    pub fn scroll_to_next_prompt(&self) -> bool {
        self.terminal.lock().scroll_to_next_prompt()
    }
}

#[cfg(test)]
mod tests;
