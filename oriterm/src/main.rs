//! Binary entry point for the oriterm terminal emulator.

mod pty;

use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;

use crate::pty::{PtyConfig, PtyEvent, PtyReader, spawn_pty};

fn main() {
    #[cfg(unix)]
    if let Err(e) = pty::signal::init() {
        log::warn!("failed to register SIGCHLD handler: {e}");
    }

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
    let pty_reader = PtyReader::spawn(reader, tx);

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
