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
    #[cfg(unix)]
    unix_main();

    #[cfg(not(unix))]
    {
        eprintln!("oriterm-mux daemon is not yet supported on this platform.");
        eprintln!("Use oriterm directly for single-process mode.");
        std::process::exit(1);
    }
}

/// Run modes parsed from CLI arguments.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Run in foreground (default).
    Foreground,
    /// Detach and run in background.
    Daemon,
    /// Stop a running daemon.
    Stop,
}

#[cfg(unix)]
fn unix_main() {
    let mode = parse_args();

    match mode {
        Mode::Foreground => run_foreground(),
        Mode::Daemon => run_daemon(),
        Mode::Stop => run_stop(),
    }
}

/// Parse CLI arguments into a run mode.
#[cfg(unix)]
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

#[cfg(unix)]
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
#[cfg(unix)]
fn run_foreground() {
    init_logger();

    let mut server = match oriterm_mux::server::MuxServer::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to start daemon: {e}");
            std::process::exit(1);
        }
    };

    // Register signal handlers for graceful shutdown.
    let shutdown = server.shutdown_flag();
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGTERM, shutdown.clone()) {
        log::warn!("failed to register SIGTERM handler: {e}");
    }
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGINT, shutdown) {
        log::warn!("failed to register SIGINT handler: {e}");
    }

    if let Err(e) = server.run() {
        log::error!("server error: {e}");
        std::process::exit(1);
    }
}

/// Spawn self in background with `--foreground` and exit.
#[cfg(unix)]
fn run_daemon() {
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("failed to get executable path: {e}");
        std::process::exit(1);
    });

    match Command::new(exe)
        .arg("--foreground")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            eprintln!("oriterm-mux daemon started (pid={})", child.id());
        }
        Err(e) => {
            eprintln!("failed to spawn daemon: {e}");
            std::process::exit(1);
        }
    }
}

/// Stop a running daemon by sending SIGTERM.
#[cfg(unix)]
fn run_stop() {
    use oriterm_mux::server::{pid_file_path, read_pid};

    let path = pid_file_path();
    let pid = match read_pid(&path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no running daemon found ({}): {e}", path.display());
            std::process::exit(1);
        }
    };

    // Send SIGTERM to the daemon process.
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

/// Initialize a minimal logger that writes to stderr.
#[cfg(unix)]
fn init_logger() {
    use std::io::Write;
    use std::sync::Mutex;

    struct StderrLogger(Mutex<()>);

    impl log::Log for StderrLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.target().starts_with("oriterm")
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            if let Ok(_guard) = self.0.lock() {
                let _ = writeln!(std::io::stderr(), "[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {
            let _ = std::io::stderr().flush();
        }
    }

    let logger = Box::new(StderrLogger(Mutex::new(())));
    if log::set_logger(Box::leak(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
