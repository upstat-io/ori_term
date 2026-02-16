//! Binary entry point for the oriterm terminal emulator.
//!
//! Currently a verification harness: validates font discovery, GPU adapters,
//! clipboard, and the full PTY → VTE → Term pipeline via [`Tab`].

mod clipboard;
mod font;
mod gpu;
mod platform;
mod pty;
mod tab;

use std::thread;
use std::time::{Duration, Instant};

use oriterm_core::{Column, Line};

use crate::tab::{Tab, TabId, TermEvent};

fn main() {
    #[cfg(unix)]
    if let Err(e) = pty::signal::init() {
        log::warn!("failed to register SIGCHLD handler: {e}");
    }

    // Discover fonts at startup (validates the discovery pipeline).
    let fonts = font::discovery::discover_fonts(None, 400);
    log::info!(
        "font discovery: primary={:?} (origin={:?}, face_indices={:?}, \
         variants=[{}, {}, {}, {}]), embedded={}B, {} fallback(s)",
        fonts.primary.family_name,
        fonts.primary.origin,
        fonts.primary.face_indices,
        fonts.primary.has_variant[0],
        fonts.primary.has_variant[1],
        fonts.primary.has_variant[2],
        fonts.primary.has_variant[3],
        font::discovery::EMBEDDED_FONT_DATA.len(),
        fonts.fallbacks.len(),
    );
    for (i, path) in fonts.primary.paths.iter().enumerate() {
        if let Some(p) = path {
            log::info!("  slot[{i}]: {}", p.display());
        }
    }
    for fb in &fonts.fallbacks {
        log::info!(
            "  fallback: {} (face={}, origin={:?})",
            fb.path.display(),
            fb.face_index,
            fb.origin,
        );
    }
    // Exercise user fallback resolution (no-op with None result).
    let _ = font::discovery::resolve_user_fallback("__nonexistent__");

    // Validate GPU availability (enumerate adapters without needing a window).
    let adapter_count = gpu::validate_gpu();
    log::info!("GPU validation: {adapter_count} adapter(s) found");

    // Validate clipboard pipeline (falls back to no-op if no display server).
    let mut cb = clipboard::Clipboard::new();
    cb.store(
        oriterm_core::event::ClipboardType::Clipboard,
        "oriterm clipboard test",
    );
    let clip = cb.load(oriterm_core::event::ClipboardType::Clipboard);
    log::info!("clipboard: loaded {} bytes", clip.len());

    // --- End-to-end Tab verification (Section 4.9) ---

    // Create a winit EventLoop for the EventProxy (no window needed yet).
    let event_loop = build_event_loop();
    let proxy = event_loop.create_proxy();
    // Leak the event loop so the proxy remains valid. The event loop isn't run
    // (no window to dispatch to) — it only exists for EventLoopProxy.
    std::mem::forget(event_loop);

    // Spawn a Tab (PTY + reader thread + Term + VTE pipeline).
    let mut tab = Tab::new(TabId::next(), 24, 80, 1000, proxy).expect("failed to create tab");
    log::info!("tab {:?}: spawned 24x80", tab.id());

    // Send an echo command to the shell.
    tab.write_input(b"echo hello\r\n");

    // Poll until "hello" appears in the terminal grid or timeout.
    let found = poll_grid_for(&tab, "hello", Duration::from_secs(5));

    if found {
        log::info!("PASS: end-to-end PTY -> VTE -> Term pipeline verified");
    } else {
        log::error!("FAIL: 'hello' not found in terminal grid after 5s");
    }

    // Verify resize: change dimensions from 80x24 to 120x40.
    tab.resize(40, 120);
    log::info!("tab {:?}: resized to 40x120", tab.id());

    // Exercise title and bell state (normally set by shell via events).
    tab.set_title("verification".into());
    log::info!("tab {:?}: title={:?}", tab.id(), tab.title());

    tab.set_bell();
    log::info!("tab {:?}: bell={}", tab.id(), tab.has_bell());
    tab.clear_bell();

    // Detect child exit via signal (complements PTY EOF detection in reader).
    #[cfg(unix)]
    if pty::signal::check() {
        log::info!("SIGCHLD detected");
    }

    // Clean shutdown: drop sends Shutdown, kills child, joins reader thread.
    drop(tab);

    log::info!("verification complete");
}

/// Poll the terminal grid until `needle` appears in any visible row.
///
/// Returns `true` if found before `timeout`, `false` otherwise.
fn poll_grid_for(tab: &Tab, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));

        let term = tab.terminal().lock();
        let grid = term.grid();

        for line_idx in 0..grid.lines() {
            let row = &grid[Line(line_idx as i32)];
            let text: String = (0..grid.cols()).map(|c| row[Column(c)].ch).collect();
            if text.contains(needle) {
                return true;
            }
        }
    }

    false
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
