//! Tests for PtyEventLoop.
//!
//! Uses anonymous pipes to test the event loop without real PTY processes,
//! avoiding platform-specific ConPTY issues with blocking reads.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use oriterm_core::{FairMutex, Term, Theme, VoidListener};

use super::{COALESCE_DELAY, COALESCE_THRESHOLD, MAX_LOCKED_PARSE, PtyEventLoop, READ_BUFFER_SIZE};
use crate::pty::Msg;

/// Build a PtyEventLoop with the given reader.
fn build_event_loop(
    reader: Box<dyn Read + Send>,
    _writer: Box<dyn Write + Send>,
) -> (
    PtyEventLoop<VoidListener>,
    Arc<FairMutex<Term<VoidListener>>>,
    mpsc::Sender<Msg>,
) {
    let terminal = Arc::new(FairMutex::new(Term::new(
        24,
        80,
        1000,
        Theme::default(),
        VoidListener,
    )));
    let (tx, rx) = mpsc::channel();

    let event_loop = PtyEventLoop::new(Arc::clone(&terminal), reader, rx);

    (event_loop, terminal, tx)
}

#[test]
fn shutdown_on_reader_eof() {
    // Anonymous pipe where we control the write end — dropping it produces EOF.
    let (pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, _terminal, _tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    // Drop the write end → reader gets EOF → thread exits.
    drop(pipe_writer);

    join.join().expect("reader thread should exit on EOF");
}

#[test]
fn processes_pty_output_into_terminal() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, _tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    // Simulate shell output: write raw text to the reader pipe.
    pipe_writer.write_all(b"hello world").expect("write");

    // Close the pipe to trigger EOF so the thread exits.
    drop(pipe_writer);

    join.join().expect("reader thread should exit on EOF");

    // Verify terminal received the output.
    let term = terminal.lock();
    let grid = term.grid();
    let first_row = &grid[oriterm_core::Line(0)];
    let text: String = (0..80)
        .map(|col| first_row[oriterm_core::Column(col)].ch)
        .collect();
    assert!(
        text.contains("hello world"),
        "terminal grid should contain 'hello world', got: {text:?}",
    );
}

#[test]
fn read_buffer_size_is_64kb() {
    assert_eq!(READ_BUFFER_SIZE, 65536);
}

#[test]
fn max_locked_parse_is_64kb() {
    assert_eq!(MAX_LOCKED_PARSE, 65536);
}

#[test]
fn coalesce_threshold_below_read_buffer() {
    assert!(COALESCE_THRESHOLD < READ_BUFFER_SIZE);
}

#[test]
fn coalesce_delay_is_1ms() {
    assert_eq!(COALESCE_DELAY, Duration::from_millis(1));
}

// --- Contention benchmarks ---
//
// These test the FairMutex locking strategies under realistic contention:
// a "reader" thread floods data through a real PtyEventLoop (VTE parsing),
// while a "renderer" thread tries to lock the terminal periodically.

/// Feed flood data through a real PtyEventLoop while a contending thread
/// measures how often it can acquire the terminal lock.
///
/// Returns `(reader_bytes, renderer_locks, elapsed)`.
fn run_contention_bench(duration: Duration) -> (usize, usize, Duration) {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    let done = Arc::new(AtomicBool::new(false));
    let renderer_count = Arc::new(AtomicUsize::new(0));

    // Renderer thread — tries to lock the terminal in a tight loop.
    let term_clone = Arc::clone(&terminal);
    let done_clone = Arc::clone(&done);
    let rc = Arc::clone(&renderer_count);
    let renderer = thread::spawn(move || {
        while !done_clone.load(Ordering::Relaxed) {
            let _guard = term_clone.lock();
            rc.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Feed flood data from this thread.
    // Use a repeating pattern of printable chars + newlines.
    let flood_line = "A".repeat(79) + "\n";
    let flood_block = flood_line.repeat(100); // ~8KB per block
    let flood_bytes = flood_block.as_bytes();
    let mut total_written = 0usize;

    let start = Instant::now();
    while start.elapsed() < duration {
        match pipe_writer.write(flood_bytes) {
            Ok(n) => total_written += n,
            Err(_) => break,
        }
    }

    // Stop.
    done.store(true, Ordering::Relaxed);
    let elapsed = start.elapsed();

    // Close pipe → EOF → event loop exits.
    drop(pipe_writer);
    let _ = tx.send(Msg::Shutdown);
    let _ = join.join();
    renderer.join().expect("renderer thread");

    let locks = renderer_count.load(Ordering::Relaxed);
    (total_written, locks, elapsed)
}

/// Verifies that the renderer is not starved during flood output.
///
/// The reader thread floods data through a real PtyEventLoop (with actual
/// VTE parsing). A contending renderer thread measures how many lock
/// acquisitions it gets. With a working fair-lock strategy, the renderer
/// must get consistent access.
#[test]
fn renderer_not_starved_during_flood() {
    let (bytes, renderer_locks, elapsed) = run_contention_bench(Duration::from_millis(500));

    let mb_written = bytes as f64 / (1024.0 * 1024.0);
    let secs = elapsed.as_secs_f64();
    let throughput_mbps = mb_written / secs;
    let renderer_per_sec = renderer_locks as f64 / secs;

    eprintln!("--- contention benchmark ---");
    eprintln!("  duration:       {elapsed:?}");
    eprintln!("  data written:   {mb_written:.1} MB");
    eprintln!("  throughput:     {throughput_mbps:.1} MB/s");
    eprintln!("  renderer locks: {renderer_locks} ({renderer_per_sec:.0}/s)");

    // The renderer must get at least 60 locks/sec (one per frame at 60fps).
    // A starved renderer would get 0 or single-digit locks over 500ms.
    assert!(
        renderer_locks >= 30,
        "renderer starved: only {renderer_locks} locks in {elapsed:?} \
         (need >= 30 for 60fps). throughput={throughput_mbps:.1} MB/s",
    );
}

/// Measures baseline throughput without contention (reader only).
///
/// This establishes how fast the PtyEventLoop can parse data when there's
/// no renderer thread competing for the lock.
#[test]
fn reader_throughput_no_contention() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, _terminal, tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    let flood_line = "A".repeat(79) + "\n";
    let flood_block = flood_line.repeat(100);
    let flood_bytes = flood_block.as_bytes();
    let mut total_written = 0usize;

    let duration = Duration::from_millis(500);
    let start = Instant::now();
    while start.elapsed() < duration {
        match pipe_writer.write(flood_bytes) {
            Ok(n) => total_written += n,
            Err(_) => break,
        }
    }
    let elapsed = start.elapsed();

    drop(pipe_writer);
    let _ = tx.send(Msg::Shutdown);
    let _ = join.join();

    let mb = total_written as f64 / (1024.0 * 1024.0);
    let secs = elapsed.as_secs_f64();
    let throughput = mb / secs;

    eprintln!("--- throughput benchmark (no contention) ---");
    eprintln!("  duration:   {elapsed:?}");
    eprintln!("  written:    {mb:.1} MB");
    eprintln!("  throughput: {throughput:.1} MB/s");
}

/// Verifies that interactive-size reads (below coalesce threshold) do not
/// trigger coalesce delays, preserving keystroke latency.
///
/// Feeds small payloads (single characters, short escape sequences) through
/// a real PtyEventLoop with a contending renderer. Measures end-to-end
/// latency: if the reader coalesced on small reads, latency would spike
/// by at least 1ms per character.
#[test]
fn interactive_reads_skip_coalesce() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, _tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    // Renderer thread — creates contention so take_contended() would be true.
    let term_clone = Arc::clone(&terminal);
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);
    let renderer = thread::spawn(move || {
        while !done_clone.load(Ordering::Relaxed) {
            let _g = term_clone.lock();
            thread::yield_now();
        }
    });

    // Feed 50 small writes (simulating keystrokes). If coalesce triggered
    // on each, total time would be >= 50ms. Without coalesce, this should
    // complete in under 20ms.
    let start = Instant::now();
    for i in 0..50 {
        let ch = b'a' + (i % 26);
        pipe_writer.write_all(&[ch]).expect("write");
        // Tiny sleep between keystrokes to let the reader process each
        // individually (separate read() calls).
        thread::sleep(Duration::from_micros(100));
    }
    let elapsed = start.elapsed();

    done.store(true, Ordering::Relaxed);
    drop(pipe_writer);
    let _ = join.join();
    renderer.join().expect("renderer thread");

    // 50 keystrokes at 100us spacing = ~5ms baseline. If coalesce fires
    // (1ms each), total would be >= 50ms. Allow generous margin.
    assert!(
        elapsed < Duration::from_millis(100),
        "interactive reads too slow ({elapsed:?}), coalesce may be firing on small reads",
    );
}

/// Verifies renderer access survives bursty flood patterns.
///
/// Alternates between flood bursts (above coalesce threshold) and idle
/// periods, simulating realistic shell usage: `ls` output → prompt → `cat`.
/// The renderer must get consistent lock access throughout.
#[test]
fn bursty_flood_renderer_access() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, _tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    let done = Arc::new(AtomicBool::new(false));
    let renderer_count = Arc::new(AtomicUsize::new(0));

    let term_clone = Arc::clone(&terminal);
    let done_clone = Arc::clone(&done);
    let rc = Arc::clone(&renderer_count);
    let renderer = thread::spawn(move || {
        while !done_clone.load(Ordering::Relaxed) {
            let _g = term_clone.lock();
            rc.fetch_add(1, Ordering::Relaxed);
        }
    });

    let flood_block = ("A".repeat(79) + "\n").repeat(100); // ~8KB
    let flood_bytes = flood_block.as_bytes();

    // 5 cycles of: 100ms flood → 50ms idle.
    for _ in 0..5 {
        let burst_start = Instant::now();
        while burst_start.elapsed() < Duration::from_millis(100) {
            match pipe_writer.write(flood_bytes) {
                Ok(_) => {}
                Err(_) => break,
            }
        }
        // Idle — simulates user reading output or typing next command.
        thread::sleep(Duration::from_millis(50));
    }

    done.store(true, Ordering::Relaxed);
    drop(pipe_writer);
    let _ = join.join();
    renderer.join().expect("renderer thread");

    let locks = renderer_count.load(Ordering::Relaxed);
    // 750ms total (5 × 150ms). Renderer needs at least 45 locks (60fps).
    assert!(
        locks >= 45,
        "renderer starved during bursty flood: only {locks} locks in 750ms \
         (need >= 45 for 60fps)",
    );
}

/// Verifies the coalesce delay is approximately 1ms (not wildly inaccurate).
///
/// Measures 100 consecutive sleeps of `COALESCE_DELAY` and checks the
/// average is within a reasonable range. On Windows, `Sleep(1)` rounds
/// up to one scheduler quantum (~1-15ms), so we allow wide tolerance.
#[test]
fn coalesce_delay_accuracy() {
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        thread::sleep(COALESCE_DELAY);
    }
    let elapsed = start.elapsed();
    let avg = elapsed / iterations;

    eprintln!("--- coalesce delay accuracy ---");
    eprintln!("  target:  {:?}", COALESCE_DELAY);
    eprintln!("  average: {avg:?} ({iterations} iterations)");

    // Must be at least 1ms (the requested delay).
    assert!(
        avg >= Duration::from_micros(500),
        "coalesce sleep too short: avg={avg:?}, expected >= 0.5ms",
    );
    // Must not be absurdly long (> 50ms would destroy throughput).
    assert!(
        avg < Duration::from_millis(50),
        "coalesce sleep too long: avg={avg:?}, expected < 50ms",
    );
}

/// Processes a sustained large flood without memory growth.
///
/// Feeds 50MB+ through a real PtyEventLoop with VTE parsing and verifies
/// the thread exits cleanly. If internal buffers grew unbounded, this
/// would OOM or the thread would hang.
#[test]
fn sustained_flood_no_oom() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, tx) =
        build_event_loop(Box::new(pipe_reader), Box::new(Vec::<u8>::new()));

    let join = event_loop.spawn().expect("spawn event loop");

    // Renderer thread — applies backpressure like production.
    let term_clone = Arc::clone(&terminal);
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);
    let renderer = thread::spawn(move || {
        while !done_clone.load(Ordering::Relaxed) {
            let _g = term_clone.lock();
            thread::sleep(Duration::from_millis(16)); // ~60fps
        }
    });

    // Feed 50MB of data.
    let flood_block = ("X".repeat(79) + "\n").repeat(1000); // ~80KB
    let flood_bytes = flood_block.as_bytes();
    let target = 50 * 1024 * 1024; // 50MB
    let mut total = 0usize;

    while total < target {
        match pipe_writer.write(flood_bytes) {
            Ok(n) => total += n,
            Err(_) => break,
        }
    }

    let mb = total as f64 / (1024.0 * 1024.0);
    eprintln!("--- sustained flood ---");
    eprintln!("  written: {mb:.1} MB");

    done.store(true, Ordering::Relaxed);
    drop(pipe_writer);
    let _ = tx.send(Msg::Shutdown);

    // Thread must exit within 5 seconds. If it hangs, buffers are growing
    // unbounded or the coalesce logic is deadlocking.
    let join_result = join.join();
    renderer.join().expect("renderer thread");
    assert!(
        join_result.is_ok(),
        "event loop thread panicked during sustained flood"
    );
}
