//! Raw VTE interceptor for sequences the high-level processor drops.
//!
//! The `vte::ansi::Processor` does not route OSC 7, OSC 133, OSC 9/99/777,
//! or XTVERSION (CSI >q) to `Handler` trait methods. This interceptor uses
//! a raw `vte::Parser` with a custom `Perform` impl to catch these sequences
//! before the high-level processor discards them.

use oriterm_core::event::EventListener;
use oriterm_core::{Notification, PromptState};

use oriterm_core::Term;

/// Raw VTE interceptor state.
///
/// Borrows `Term<T>` fields mutably to update them in place. Runs on the
/// same locked terminal, on the same bytes, before the high-level processor.
pub(crate) struct RawInterceptor<'a, T: EventListener> {
    term: &'a mut Term<T>,
}

impl<'a, T: EventListener> RawInterceptor<'a, T> {
    /// Create a new interceptor targeting the given terminal.
    pub(crate) fn new(term: &'a mut Term<T>) -> Self {
        Self { term }
    }
}

impl<T: EventListener> vte::Perform for RawInterceptor<'_, T> {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() || params[0].is_empty() {
            return;
        }
        match params[0] {
            // OSC 7 — Current working directory.
            // Format: OSC 7 ; file://hostname/path ST
            b"7" => self.handle_osc7(params),
            // OSC 133 — Semantic prompt markers.
            b"133" => self.handle_osc133(params),
            // OSC 9 / OSC 99 — iTerm2 / Kitty notifications.
            b"9" | b"99" => self.handle_notification_simple(params),
            // OSC 777 — rxvt-unicode notification.
            b"777" => self.handle_notification_777(params),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        _params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // XTVERSION: CSI > q — report terminal name and version.
        if action == 'q' && intermediates == [b'>'] {
            let version = env!("CARGO_PKG_VERSION");
            let response = format!("\x1bP>|oriterm({version})\x1b\\");
            self.term
                .event_listener()
                .send_event(oriterm_core::Event::PtyWrite(response));
        }
    }
}

impl<T: EventListener> RawInterceptor<'_, T> {
    /// OSC 7: parse `file://hostname/path` URI and store the path.
    fn handle_osc7(&mut self, params: &[&[u8]]) {
        if params.len() < 2 {
            return;
        }
        let uri = std::str::from_utf8(params[1]).unwrap_or_default();
        let raw_path = parse_osc7_path(uri);
        if !raw_path.is_empty() {
            let path = percent_decode(raw_path).into_owned();
            *self.term.cwd_mut() = Some(path.clone());
            self.term.set_has_explicit_title(false);
            self.term.mark_title_dirty();
            self.term
                .event_listener()
                .send_event(oriterm_core::Event::Cwd(path));
        }
    }

    /// OSC 133: update prompt state machine and track command timing.
    fn handle_osc133(&mut self, params: &[&[u8]]) {
        if params.len() < 2 || params[1].is_empty() {
            return;
        }
        match params[1][0] {
            b'A' => {
                *self.term.prompt_state_mut() = PromptState::PromptStart;
                self.term.set_prompt_mark_pending(true);
            }
            b'B' => *self.term.prompt_state_mut() = PromptState::CommandStart,
            b'C' => {
                *self.term.prompt_state_mut() = PromptState::OutputStart;
                self.term.set_command_start(std::time::Instant::now());
            }
            b'D' => {
                *self.term.prompt_state_mut() = PromptState::None;
                if let Some(duration) = self.term.finish_command() {
                    self.term
                        .event_listener()
                        .send_event(oriterm_core::Event::CommandComplete(duration));
                }
            }
            _ => {}
        }
    }

    /// OSC 9/99: simple notification (body only).
    fn handle_notification_simple(&mut self, params: &[&[u8]]) {
        let body = if params.len() >= 2 {
            String::from_utf8_lossy(params[1]).into_owned()
        } else {
            String::new()
        };
        self.term.push_notification(Notification {
            title: String::new(),
            body,
        });
    }

    /// OSC 777: rxvt-unicode notification (`notify;title;body`).
    fn handle_notification_777(&mut self, params: &[&[u8]]) {
        if params.len() < 2 {
            return;
        }
        let action = std::str::from_utf8(params[1]).unwrap_or_default();
        if action != "notify" {
            return;
        }
        let title = params
            .get(2)
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .unwrap_or_default();
        let body = params
            .get(3)
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .unwrap_or_default();
        self.term.push_notification(Notification { title, body });
    }
}

/// Extract the filesystem path from an OSC 7 URI.
///
/// Handles formats like:
/// - `file://hostname/path/to/dir` → `/path/to/dir`
/// - `file:///path/to/dir` → `/path/to/dir` (empty hostname)
/// - `/path/to/dir` → `/path/to/dir` (no prefix)
///
/// Strips query strings (`?...`) and fragments (`#...`) from the path.
pub(super) fn parse_osc7_path(uri: &str) -> &str {
    let Some(after_scheme) = uri.strip_prefix("file://") else {
        return strip_uri_suffix(uri);
    };
    // After `file://`, we have `hostname/path`. Find the first `/`
    // which starts the absolute path (skipping the hostname portion).
    after_scheme
        .as_bytes()
        .iter()
        .position(|&b| b == b'/')
        .map_or(after_scheme, |pos| {
            let path = after_scheme.get(pos..).unwrap_or(after_scheme);
            strip_uri_suffix(path)
        })
}

/// Strip query string (`?...`) and fragment (`#...`) from a URI path.
fn strip_uri_suffix(path: &str) -> &str {
    let path = path.split('?').next().unwrap_or(path);
    path.split('#').next().unwrap_or(path)
}

/// Percent-decode a URI path component (`%20` → ` `, etc.).
///
/// Returns the input unchanged (borrowed) if no `%`-encoded sequences are
/// present. Otherwise allocates and returns an owned decoded string.
pub(super) fn percent_decode(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('%') {
        return std::borrow::Cow::Borrowed(s);
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(decoded) = decode_hex_pair(bytes[i + 1], bytes[i + 2]) {
                out.push(decoded);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .map(std::borrow::Cow::Owned)
        .unwrap_or(std::borrow::Cow::Borrowed(s))
}

/// Decode a pair of hex digits into a single byte.
fn decode_hex_pair(hi: u8, lo: u8) -> Option<u8> {
    let h = match hi {
        b'0'..=b'9' => hi - b'0',
        b'a'..=b'f' => hi - b'a' + 10,
        b'A'..=b'F' => hi - b'A' + 10,
        _ => return None,
    };
    let l = match lo {
        b'0'..=b'9' => lo - b'0',
        b'a'..=b'f' => lo - b'a' + 10,
        b'A'..=b'F' => lo - b'A' + 10,
        _ => return None,
    };
    Some(h << 4 | l)
}
