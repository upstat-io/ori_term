//! Integration tests for the IPC layer.
//!
//! These tests exercise the full lifecycle: bind → connect → accept →
//! read/write → close. They run on the host platform using real IPC
//! (Unix domain sockets on Linux/macOS, named pipes on Windows).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use mio::{Events, Interest, Poll, Token};

use oriterm_ipc::{ClientStream, IpcListener, IpcStream};

/// Monotonic counter for unique test socket paths.
static TEST_ID: AtomicU32 = AtomicU32::new(0);

/// Generate a unique IPC address for a test.
fn test_addr() -> PathBuf {
    let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();

    #[cfg(unix)]
    {
        let dir = std::env::temp_dir().join(format!("oriterm-ipc-test-{pid}"));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("test-{id}.sock"))
    }

    #[cfg(windows)]
    {
        PathBuf::from(format!(r"\\.\pipe\oriterm-ipc-test-{pid}-{id}"))
    }
}

const LISTENER_TOKEN: Token = Token(0);
const STREAM_TOKEN: Token = Token(1);

// ── Bind + Accept ────────────────────────────────────────────────

#[test]
fn bind_and_accept() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    // Client connects from a background thread.
    let addr2 = addr.clone();
    let client = std::thread::spawn(move || ClientStream::connect(&addr2).unwrap());

    // Poll until the listener is readable.
    poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

    let _stream: IpcStream = listener.accept().unwrap();
    let _client: ClientStream = client.join().unwrap();
}

// ── Client → Server data ────────────────────────────────────────

#[test]
fn client_to_server() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    let addr2 = addr.clone();
    let client = std::thread::spawn(move || {
        let mut stream = ClientStream::connect(&addr2).unwrap();
        stream.write_all(b"hello from client").unwrap();
        stream
    });

    poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

    let mut stream = listener.accept().unwrap();
    poll.registry()
        .register(&mut stream, STREAM_TOKEN, Interest::READABLE)
        .unwrap();

    // Read the client's message.
    let data = read_with_poll(&mut poll, &mut events, &mut stream, STREAM_TOKEN);
    assert_eq!(data, b"hello from client");

    let _client = client.join().unwrap();
}

// ── Server → Client data ────────────────────────────────────────

#[test]
fn server_to_client() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    let addr2 = addr.clone();
    let client = std::thread::spawn(move || {
        let mut stream = ClientStream::connect(&addr2).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).unwrap();
        Vec::from(&buf[..n])
    });

    poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

    let mut stream = listener.accept().unwrap();
    poll.registry()
        .register(
            &mut stream,
            STREAM_TOKEN,
            Interest::READABLE | Interest::WRITABLE,
        )
        .unwrap();

    // Write to the accepted stream — this is the exact pattern the daemon
    // uses when sending HelloAck. On Windows, a freshly accepted named
    // pipe may return WouldBlock if the kernel write buffer isn't ready.
    write_with_poll(
        &mut poll,
        &mut events,
        &mut stream,
        STREAM_TOKEN,
        b"hello from server",
    );

    let data = client.join().unwrap();
    assert_eq!(data, b"hello from server");
}

// ── Full bidirectional roundtrip ─────────────────────────────────

#[test]
fn bidirectional_roundtrip() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    let addr2 = addr.clone();
    let client = std::thread::spawn(move || {
        let mut stream = ClientStream::connect(&addr2).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Send request.
        stream.write_all(b"ping").unwrap();

        // Read response.
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).unwrap();
        Vec::from(&buf[..n])
    });

    poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

    let mut stream = listener.accept().unwrap();
    poll.registry()
        .register(
            &mut stream,
            STREAM_TOKEN,
            Interest::READABLE | Interest::WRITABLE,
        )
        .unwrap();

    // Read the client's "ping".
    let data = read_with_poll(&mut poll, &mut events, &mut stream, STREAM_TOKEN);
    assert_eq!(data, b"ping");

    // Send "pong" back.
    write_with_poll(&mut poll, &mut events, &mut stream, STREAM_TOKEN, b"pong");

    let response = client.join().unwrap();
    assert_eq!(response, b"pong");
}

// ── Multiple sequential connections ──────────────────────────────

#[test]
fn multiple_sequential_connections() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    for i in 0..3u8 {
        let addr2 = addr.clone();
        let client = std::thread::spawn(move || {
            let mut stream = ClientStream::connect(&addr2).unwrap();
            stream.write_all(&[i]).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0u8; 1];
            let n = stream.read(&mut buf).unwrap();
            assert_eq!(n, 1);
            assert_eq!(buf[0], i + 10);
        });

        poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

        let mut stream = listener.accept().unwrap();
        let token = Token(10 + i as usize);
        poll.registry()
            .register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)
            .unwrap();

        // Read client's byte.
        let data = read_with_poll(&mut poll, &mut events, &mut stream, token);
        assert_eq!(data, vec![i]);

        // Echo back with +10.
        write_with_poll(&mut poll, &mut events, &mut stream, token, &[i + 10]);

        client.join().unwrap();

        // Deregister before next iteration.
        poll.registry().deregister(&mut stream).unwrap();

        // Reregister listener for next connection.
        // On Windows, the listener internally manages pending pipe instances;
        // on Unix, the listener fd is reusable. Either way, reregister
        // ensures we get events for the next connection.
        poll.registry()
            .reregister(&mut listener, LISTENER_TOKEN, Interest::READABLE)
            .unwrap();
    }
}

// ── Write immediately after accept (WouldBlock regression) ───────

/// Verifies that the server can write to a freshly accepted stream
/// without needing to poll for WRITABLE first. This is the pattern
/// used for the Hello/HelloAck handshake in the daemon.
#[test]
fn write_after_accept_does_not_block() {
    let addr = test_addr();
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(16);

    let mut listener = IpcListener::bind_at(&addr).unwrap();
    poll.registry()
        .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
        .unwrap();

    let addr2 = addr.clone();
    let client = std::thread::spawn(move || {
        let mut stream = ClientStream::connect(&addr2).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        // Read all available data.
        let mut buf = vec![0u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        buf.truncate(n);
        buf
    });

    poll_until_readable(&mut poll, &mut events, LISTENER_TOKEN);

    let mut stream = listener.accept().unwrap();
    poll.registry()
        .register(
            &mut stream,
            STREAM_TOKEN,
            Interest::READABLE | Interest::WRITABLE,
        )
        .unwrap();

    // Write a non-trivial payload (simulating a HelloAck PDU).
    let payload = vec![42u8; 256];
    write_with_poll(&mut poll, &mut events, &mut stream, STREAM_TOKEN, &payload);

    let data = client.join().unwrap();
    assert_eq!(data, payload);
}

// ── Helpers ──────────────────────────────────────────────────────

/// Poll until a specific token is readable, with a timeout.
fn poll_until_readable(poll: &mut Poll, events: &mut Events, target: Token) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for token {target:?} to be readable");
        }
        poll.poll(events, Some(remaining.min(Duration::from_millis(100))))
            .unwrap();
        for event in events.iter() {
            if event.token() == target && event.is_readable() {
                return;
            }
        }
    }
}

/// Poll until a specific token is writable, with a timeout.
fn poll_until_writable(poll: &mut Poll, events: &mut Events, target: Token) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for token {target:?} to be writable");
        }
        poll.poll(events, Some(remaining.min(Duration::from_millis(100))))
            .unwrap();
        for event in events.iter() {
            if event.token() == target && event.is_writable() {
                return;
            }
        }
    }
}

/// Read from a non-blocking stream, polling for readability first.
fn read_with_poll(
    poll: &mut Poll,
    events: &mut Events,
    stream: &mut IpcStream,
    token: Token,
) -> Vec<u8> {
    poll_until_readable(poll, events, token);

    let mut buf = vec![0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                return buf;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                poll_until_readable(poll, events, token);
            }
            Err(e) => panic!("read error: {e}"),
        }
    }
}

/// Write to a non-blocking stream, polling for writability on WouldBlock.
fn write_with_poll(
    poll: &mut Poll,
    events: &mut Events,
    stream: &mut IpcStream,
    token: Token,
    data: &[u8],
) {
    let mut written = 0;
    while written < data.len() {
        match stream.write(&data[written..]) {
            Ok(n) => written += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                poll_until_writable(poll, events, token);
            }
            Err(e) => panic!("write error: {e}"),
        }
    }
}
