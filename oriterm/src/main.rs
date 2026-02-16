//! Binary entry point for the oriterm terminal emulator.

mod clipboard;
mod font;
mod gpu;
mod platform;
mod pty;
mod tab;

use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;

use crate::pty::{PtyConfig, PtyEvent, PtyReader, spawn_pty};

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

    let config = PtyConfig::default();
    let mut handle = spawn_pty(&config).expect("failed to spawn PTY");

    if let Some(pid) = handle.process_id() {
        log::debug!("spawned shell (PID {pid})");
    }

    // Verify PTY responds to resize.
    let _ = handle.resize(config.rows, config.cols);

    let reader = handle.take_reader().expect("PTY reader unavailable");
    let mut writer = handle.take_writer().expect("PTY writer unavailable");

    let (tx, rx) = mpsc::channel();
    let pty_reader = PtyReader::spawn(reader, tx).expect("failed to spawn pty-reader thread");

    // Relay stdin to PTY input.
    let _input = thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            match io::Read::read(&mut stdin, &mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        return;
                    }
                }
            }
        }
    });

    // Print PTY output from the reader thread.
    for event in rx {
        match event {
            PtyEvent::Data(data) => {
                let _ = io::stdout().write_all(&data);
                let _ = io::stdout().flush();
            }
            PtyEvent::Closed => break,
        }

        // Detect child exit via signal (complements PTY EOF detection).
        #[cfg(unix)]
        if pty::signal::check() {
            break;
        }
    }

    pty_reader.join();
    let _ = handle.kill();
    let _ = handle.wait();
}
