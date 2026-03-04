//! Binary entry point for the oriterm mux daemon.
//!
//! The daemon owns all PTY sessions via [`InProcessMux`] and accepts IPC
//! connections from stateless window processes. This separation ensures
//! terminal sessions survive window crashes.
//!
//! # Usage
//!
//! ```text
//! oriterm-mux [OPTIONS]
//!
//! Options:
//!     --daemon       Detach and run in background
//!     --foreground   Run in foreground (default)
//!     --stop         Stop a running daemon
//!     -h, --help     Print this message
//! ```

fn main() {
    let mode = parse_args();

    match mode {
        Mode::Foreground => run_foreground(),
        Mode::Daemon => run_daemon(),
        Mode::Stop => run_stop(),
    }
}

/// Run modes parsed from CLI arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Run in foreground (default).
    Foreground,
    /// Detach and run in background.
    Daemon,
    /// Stop a running daemon.
    Stop,
}

/// Parse CLI arguments into a run mode.
fn parse_args() -> Mode {
    let mut mode = Mode::Foreground;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--daemon" => mode = Mode::Daemon,
            "--foreground" => mode = Mode::Foreground,
            "--stop" => mode = Mode::Stop,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("unknown argument: {arg}");
                print_usage();
                std::process::exit(1);
            }
        }
    }

    mode
}

fn print_usage() {
    eprintln!("Usage: oriterm-mux [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("    --daemon       Detach and run in background");
    eprintln!("    --foreground   Run in foreground (default)");
    eprintln!("    --stop         Stop a running daemon");
    eprintln!("    -h, --help     Print this message");
}

/// Run the server in the foreground.
fn run_foreground() {
    init_logger();
    install_panic_hook();

    let mut server = match oriterm_mux::server::MuxServer::new() {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to start daemon: {e}");
            std::process::exit(1);
        }
    };

    register_shutdown_handler(&server);

    if let Err(e) = server.run() {
        log::error!("server error: {e}");
        std::process::exit(1);
    }
}

/// Register platform-appropriate signal/event handlers for graceful shutdown.
#[cfg(unix)]
fn register_shutdown_handler(server: &oriterm_mux::server::MuxServer) {
    let shutdown = server.shutdown_flag();
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGTERM, shutdown.clone()) {
        log::warn!("failed to register SIGTERM handler: {e}");
    }
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGINT, shutdown) {
        log::warn!("failed to register SIGINT handler: {e}");
    }
}

/// Register a console control handler on Windows for graceful shutdown.
#[cfg(windows)]
fn register_shutdown_handler(server: &oriterm_mux::server::MuxServer) {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
    };

    // Store the shutdown flag in a global so the handler callback can access it.
    static SHUTDOWN_FLAG: OnceLock<std::sync::Arc<AtomicBool>> = OnceLock::new();

    #[allow(unsafe_code, reason = "SetConsoleCtrlHandler requires unsafe FFI")]
    unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
        match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
                if let Some(flag) = SHUTDOWN_FLAG.get() {
                    flag.store(true, Ordering::Release);
                }
                1 // Handled.
            }
            _ => 0, // Not handled.
        }
    }

    let _ = SHUTDOWN_FLAG.set(server.shutdown_flag());

    // SAFETY: `SetConsoleCtrlHandler` is a well-documented Win32 API.
    // The handler function has the correct calling convention and signature.
    #[allow(unsafe_code, reason = "SetConsoleCtrlHandler requires unsafe FFI")]
    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }
}

/// Spawn self in background with `--foreground` and exit.
fn run_daemon() {
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("failed to get executable path: {e}");
        std::process::exit(1);
    });

    let mut cmd = Command::new(exe);
    cmd.arg("--foreground")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // On Windows, detach the child so it survives the parent.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x200) | DETACHED_PROCESS (0x8)
        cmd.creation_flags(0x200 | 0x8);
    }

    match cmd.spawn() {
        Ok(child) => {
            eprintln!("oriterm-mux daemon started (pid={})", child.id());
        }
        Err(e) => {
            eprintln!("failed to spawn daemon: {e}");
            std::process::exit(1);
        }
    }
}

/// Stop a running daemon via IPC shutdown, falling back to SIGTERM on Unix.
fn run_stop() {
    use oriterm_mux::server::{pid_file_path, socket_path};

    // Try IPC shutdown first (works on all platforms).
    if try_ipc_shutdown(&socket_path()) {
        return;
    }

    // Unix: fall back to SIGTERM via PID file.
    #[cfg(unix)]
    {
        use oriterm_mux::server::read_pid;

        let path = pid_file_path();
        let pid = match read_pid(&path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("no running daemon found ({}): {e}", path.display());
                std::process::exit(1);
            }
        };

        // SAFETY: `kill` is a standard POSIX function. Sending SIGTERM to a
        // process we own is safe. If the PID is stale, kill returns ESRCH.
        #[allow(unsafe_code, reason = "kill(2) requires unsafe")]
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };

        if result == 0 {
            eprintln!("sent SIGTERM to oriterm-mux daemon (pid={pid})");
        } else {
            let err = std::io::Error::last_os_error();
            eprintln!("failed to stop daemon (pid={pid}): {err}");
            std::process::exit(1);
        }
    }

    // Windows: no SIGTERM equivalent — IPC shutdown was the only path.
    #[cfg(windows)]
    {
        eprintln!(
            "daemon not reachable via IPC (no PID file at {})",
            pid_file_path().display()
        );
        std::process::exit(1);
    }
}

/// Attempt graceful shutdown via IPC: Hello handshake then Shutdown PDU.
///
/// Returns `true` if the daemon acknowledged the shutdown.
fn try_ipc_shutdown(sock: &std::path::Path) -> bool {
    use std::time::Duration;

    use oriterm_ipc::ClientStream;
    use oriterm_mux::{MuxPdu, ProtocolCodec};

    let mut stream = match ClientStream::connect(sock) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let timeout = Some(Duration::from_secs(5));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);

    // Hello handshake.
    if ProtocolCodec::encode_frame(
        &mut stream,
        1,
        &MuxPdu::Hello {
            pid: std::process::id(),
        },
    )
    .is_err()
    {
        return false;
    }

    match ProtocolCodec::new().decode_frame(&mut stream) {
        Ok(f) if matches!(f.pdu, MuxPdu::HelloAck { .. }) => {}
        _ => return false,
    }

    // Send Shutdown request.
    if ProtocolCodec::encode_frame(&mut stream, 2, &MuxPdu::Shutdown).is_err() {
        return false;
    }

    match ProtocolCodec::new().decode_frame(&mut stream) {
        Ok(f) if matches!(f.pdu, MuxPdu::ShutdownAck) => {
            // RAII: dropping stream closes the connection cleanly.
            eprintln!("daemon shutdown acknowledged via IPC");
            true
        }
        _ => false,
    }
}

/// Install a panic hook that writes to the log file before aborting.
///
/// The daemon runs detached with no console, so panics vanish silently.
/// This hook ensures the panic info is captured in `oriterm-mux.log`.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}");
    }));
}

/// Initialize a minimal file logger next to the executable.
///
/// Writes to `oriterm-mux.log` in the same directory as the binary.
/// The daemon is spawned with stderr redirected to null, so file-based
/// logging is the only way to capture diagnostic output.
fn init_logger() {
    use std::io::Write;
    use std::sync::Mutex;

    struct FileLogger(Mutex<std::fs::File>);

    impl log::Log for FileLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.target().starts_with("oriterm")
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            if let Ok(mut f) = self.0.lock() {
                let _ = writeln!(f, "[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {
            if let Ok(f) = self.0.lock() {
                let _ = Write::flush(&mut &*f);
            }
        }
    }

    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("oriterm-mux.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("oriterm-mux.log"));

    if let Ok(file) = std::fs::File::create(&path) {
        let logger = Box::new(FileLogger(Mutex::new(file)));
        if log::set_logger(Box::leak(logger)).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}
