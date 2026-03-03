//! Binary entry point for the oriterm terminal emulator.
//!
//! Builds a winit event loop and runs the [`App`] as the application handler.
//! All initialization (GPU, window, fonts, tab) happens lazily inside
//! [`App::resumed`] when the event loop first becomes active.

// GUI application — no console window on Windows.
#![windows_subsystem = "windows"]

mod app;
mod cli;
mod clipboard;
mod config;
mod event;
mod font;
mod gpu;
mod key_encoding;
mod keybindings;
mod platform;
mod scheme;
mod url_detect;
mod widgets;
mod window;

use clap::Parser;

use crate::config::Config;
use crate::event::TermEvent;

fn main() {
    let args = cli::Cli::parse();

    // CLI subcommands run headlessly — no window, no event loop.
    if let Some(cmd) = args.command {
        cli::attach_console();
        cli::dispatch(cmd);
    }

    init_logger();
    install_panic_hook();

    #[cfg(unix)]
    if let Err(e) = oriterm_mux::pty::signal::init() {
        log::warn!("failed to register SIGCHLD handler: {e}");
    }

    let event_loop = build_event_loop();
    let proxy = event_loop.create_proxy();

    let config = Config::load();

    #[cfg(unix)]
    let mut app = if let Some(ref socket) = args.connect {
        // Explicit --connect: connect to specified daemon socket.
        app::App::new_daemon(proxy, config, socket, args.window)
    } else {
        // Auto-start: try daemon mode, fall back to embedded.
        match oriterm_mux::discovery::ensure_daemon() {
            Ok(socket_path) => app::App::new_daemon(proxy, config, &socket_path, None),
            Err(e) => {
                log::warn!("daemon auto-start failed, using embedded mode: {e}");
                app::App::new(proxy, config)
            }
        }
    };

    #[cfg(not(unix))]
    let mut app = {
        if args.connect.is_some() {
            log::error!("--connect is not supported on this platform");
        }
        app::App::new(proxy, config)
    };

    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("event loop error: {e}");
    }
}

/// Initialize a minimal file logger next to the executable.
///
/// Writes to `oriterm.log` in the same directory as the binary.
/// This avoids needing an external logging crate while still capturing
/// errors from the GUI-subsystem binary (which has no console).
fn init_logger() {
    use std::io::Write;
    use std::sync::Mutex;

    struct FileLogger(Mutex<std::fs::File>);

    impl log::Log for FileLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            // Only log our crate's messages, not wgpu/naga noise.
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
        .and_then(|p| p.parent().map(|d| d.join("oriterm.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("oriterm.log"));

    if let Ok(file) = std::fs::File::create(&path) {
        let logger = Box::new(FileLogger(Mutex::new(file)));
        if log::set_logger(Box::leak(logger)).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}

/// Install a panic hook that writes to the log file before aborting.
///
/// GUI-subsystem binaries on Windows have no console, so panics vanish
/// silently. This hook ensures the backtrace is captured in `oriterm.log`.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}");
        if let Some(bt) = std::backtrace::Backtrace::force_capture()
            .to_string()
            .lines()
            .take(30)
            .collect::<Vec<_>>()
            .first()
        {
            // Log just the first line to confirm backtrace is present;
            // the full backtrace is too noisy for the log. The important
            // info is in the panic message itself.
            log::error!("backtrace (first line): {bt}");
        }
    }));
}

/// Build a winit event loop usable from the main thread.
fn build_event_loop() -> winit::event_loop::EventLoop<TermEvent> {
    #[cfg(windows)]
    {
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("failed to create event loop")
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .expect("failed to create event loop")
    }
    #[cfg(target_os = "macos")]
    {
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("failed to create event loop")
    }
}
