//! PTY spawning, shell detection, and environment setup.

use std::io;
use std::path::PathBuf;

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Exit status from a child process.
///
/// Wraps the underlying PTY library's exit status so callers don't depend
/// on `portable_pty` types directly.
#[derive(Debug, Clone)]
#[allow(dead_code, reason = "fields read by methods; methods used when UI reports exit status")]
pub struct ExitStatus {
    /// Process exit code.
    code: u32,
    /// Signal name if the process was terminated by a signal.
    signal: Option<String>,
}

#[allow(dead_code, reason = "used when UI reports exit status")]
impl ExitStatus {
    /// Returns `true` if the process exited successfully (code 0, no signal).
    pub fn success(&self) -> bool {
        self.signal.is_none() && self.code == 0
    }

    /// Returns the process exit code.
    pub fn exit_code(&self) -> u32 {
        self.code
    }

    /// Returns the signal name if the process was killed by a signal.
    pub fn signal(&self) -> Option<&str> {
        self.signal.as_deref()
    }
}

impl From<portable_pty::ExitStatus> for ExitStatus {
    fn from(status: portable_pty::ExitStatus) -> Self {
        Self {
            code: status.exit_code(),
            signal: status.signal().map(String::from),
        }
    }
}

/// Owned PTY control handle for resize operations.
///
/// Wraps the underlying PTY library's control handle so callers don't depend
/// on `portable_pty` types directly.
pub struct PtyControl(Box<dyn MasterPty + Send>);

impl PtyControl {
    /// Construct from a raw `MasterPty` trait object (test use only).
    #[cfg(test)]
    pub(crate) fn from_raw(inner: Box<dyn MasterPty + Send>) -> Self {
        Self(inner)
    }

    /// Resize the PTY to the given dimensions.
    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        self.0
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

/// Configuration for spawning a PTY.
pub struct PtyConfig {
    /// Terminal dimensions in rows.
    pub rows: u16,
    /// Terminal dimensions in columns.
    pub cols: u16,
    /// Shell program override. If `None`, uses the platform default.
    pub shell: Option<String>,
    /// Working directory for the child process.
    pub working_dir: Option<PathBuf>,
    /// Additional environment variables to set in the child.
    pub env: Vec<(String, String)>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            shell: None,
            working_dir: None,
            env: Vec::new(),
        }
    }
}

/// Handles to a spawned PTY and its child process.
///
/// The reader and writer are taken separately via [`take_reader`] and
/// [`take_writer`] for use by the reader thread and input handler.
/// Resize, kill, and wait operations remain available on the handle.
///
/// [`take_reader`]: PtyHandle::take_reader
/// [`take_writer`]: PtyHandle::take_writer
pub struct PtyHandle {
    reader: Option<Box<dyn io::Read + Send>>,
    writer: Option<Box<dyn io::Write + Send>>,
    control: Option<PtyControl>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtyHandle {
    /// Take the PTY output reader (child to parent).
    ///
    /// Returns `None` if already taken. The reader is handed to the
    /// [`PtyEventLoop`](super::event_loop::PtyEventLoop) background thread.
    pub fn take_reader(&mut self) -> Option<Box<dyn io::Read + Send>> {
        self.reader.take()
    }

    /// Take the PTY input writer (parent to child).
    ///
    /// Returns `None` if already taken. The writer is typically owned by the
    /// input handler or notifier that forwards keyboard input.
    pub fn take_writer(&mut self) -> Option<Box<dyn io::Write + Send>> {
        self.writer.take()
    }

    /// Take the PTY control handle (for resize operations).
    ///
    /// Returns `None` if already taken. The control handle is typically
    /// handed to the [`PtyEventLoop`](super::event_loop::PtyEventLoop).
    pub fn take_control(&mut self) -> Option<PtyControl> {
        self.control.take()
    }

    /// Resize the PTY to new dimensions.
    ///
    /// Returns an error if the control handle has been taken.
    #[allow(dead_code, reason = "used for direct resize before Tab takes control")]
    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        let ctl = self
            .control
            .as_ref()
            .ok_or_else(|| io::Error::other("PTY control handle already taken"))?;
        ctl.resize(rows, cols)
    }

    /// Get the child process ID, if available.
    #[allow(dead_code, reason = "exposed via Tab in Section 5 window lifecycle")]
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    /// Wait for the child process to exit (blocking).
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().map(ExitStatus::from)
    }

    /// Non-blocking check for child exit.
    ///
    /// Returns `Ok(Some(status))` if the child has exited, `Ok(None)` if
    /// still running, or `Err` on failure.
    #[allow(dead_code, reason = "used when Tab reports child exit to UI")]
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait().map(|opt| opt.map(ExitStatus::from))
    }
}

/// Spawn a PTY with the configured shell and environment.
///
/// Creates a platform-native PTY pair, spawns the shell as a child process,
/// and returns a handle with reader, writer, and child management methods.
pub fn spawn_pty(config: &PtyConfig) -> io::Result<PtyHandle> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| io::Error::other(e.to_string()))?;

    let cmd = build_command(config);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| io::Error::other(e.to_string()))?;

    // Drop the slave side so the reader detects EOF when child exits.
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| io::Error::other(e.to_string()))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| io::Error::other(e.to_string()))?;

    Ok(PtyHandle {
        reader: Some(reader),
        writer: Some(writer),
        control: Some(PtyControl(pair.master)),
        child,
    })
}

/// Build a `CommandBuilder` with shell detection and environment variables.
pub(crate) fn build_command(config: &PtyConfig) -> CommandBuilder {
    let shell = config
        .shell
        .as_deref()
        .unwrap_or_else(|| default_shell());

    let mut cmd = CommandBuilder::new(shell);

    if let Some(ref dir) = config.working_dir {
        cmd.cwd(dir);
    }

    // Terminal identification variables.
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "oriterm");

    // User-provided overrides.
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    cmd
}

/// Returns the default shell for the current platform.
///
/// On Windows, returns `cmd.exe`. On Unix, reads the `SHELL` environment
/// variable and falls back to `/bin/sh`.
#[cfg(windows)]
pub(crate) fn default_shell() -> &'static str {
    "cmd.exe"
}

/// Returns the default shell for the current platform.
#[cfg(not(windows))]
pub(crate) fn default_shell() -> &'static str {
    // Leak a static reference from the environment variable.
    // Called once at startup, so the small allocation is acceptable.
    static SHELL: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    SHELL.get_or_init(|| match std::env::var("SHELL") {
        Ok(shell) if !shell.is_empty() => Box::leak(shell.into_boxed_str()),
        _ => "/bin/sh",
    })
}
