//! OSC (Operating System Command) handler implementations.
//!
//! Handles title management (OSC 0/1/2), color operations (OSC 4/10-12/104/110-112),
//! clipboard (OSC 52), and hyperlinks (OSC 8). Methods are called by the
//! `vte::ansi::Handler` trait impl on `Term<T>`.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as Base64;
use log::debug;
use vte::ansi::{Hyperlink as VteHyperlink, NamedColor};

use crate::cell::Hyperlink;
use crate::color::Rgb;
use crate::event::{ClipboardType, Event, EventListener};

use super::super::{Term, TITLE_STACK_MAX_DEPTH};

impl<T: EventListener> Term<T> {
    /// OSC 0/2: set window title.
    pub(super) fn osc_set_title(&mut self, title: Option<String>) {
        let event = if let Some(t) = title {
            debug!("Setting title to '{t}'");
            self.title.clone_from(&t);
            Event::Title(t)
        } else {
            debug!("Resetting title");
            self.title.clear();
            Event::ResetTitle
        };
        self.event_listener.send_event(event);
    }

    /// Push current title onto the title stack (xterm extension).
    pub(super) fn osc_push_title(&mut self) {
        debug!("Pushing title '{}'", self.title);
        if self.title_stack.len() >= TITLE_STACK_MAX_DEPTH {
            self.title_stack.remove(0);
        }
        self.title_stack.push(self.title.clone());
    }

    /// Pop title from the stack and set it (xterm extension).
    pub(super) fn osc_pop_title(&mut self) {
        if let Some(title) = self.title_stack.pop() {
            debug!("Popped title '{title}'");
            self.osc_set_title(Some(title));
        }
    }

    /// OSC 4/10/11/12: set a palette color by index.
    ///
    /// Marks all lines dirty when a non-cursor color changes, since any
    /// cell could reference the modified palette entry.
    pub(super) fn osc_set_color(&mut self, index: usize, color: Rgb) {
        debug!("Setting color[{index}] = {color:?}");
        self.palette.set_indexed(index, color);

        // Cursor color change doesn't require full redraw.
        if index != NamedColor::Cursor as usize {
            self.grid_mut().dirty_mut().mark_all();
        }
    }

    /// OSC 104/110/111/112: reset a palette color to its default.
    pub(super) fn osc_reset_color(&mut self, index: usize) {
        debug!("Resetting color[{index}]");
        self.palette.reset_indexed(index);

        if index != NamedColor::Cursor as usize {
            self.grid_mut().dirty_mut().mark_all();
        }
    }

    /// OSC 4/10/11/12 query: respond with the current color value.
    ///
    /// Sends a `ColorRequest` event with a closure that formats the
    /// response escape sequence using the same terminator as the query.
    pub(super) fn osc_dynamic_color_sequence(
        &self,
        prefix: String,
        index: usize,
        terminator: &str,
    ) {
        debug!("Color query for index {index} (prefix={prefix})");
        let terminator = terminator.to_owned();
        self.event_listener.send_event(Event::ColorRequest(
            index,
            Arc::new(move |color| {
                format!(
                    "\x1b]{};rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}{}",
                    prefix, color.r, color.r, color.g, color.g, color.b, color.b, terminator,
                )
            }),
        ));
    }

    /// OSC 52: store clipboard content (base64 encoded).
    pub(super) fn osc_clipboard_store(&self, clipboard: u8, base64: &[u8]) {
        let clipboard_type = match clipboard {
            b'c' => ClipboardType::Clipboard,
            b'p' | b's' => ClipboardType::Selection,
            _ => return,
        };

        let bytes = match Base64.decode(base64) {
            Ok(b) => b,
            Err(e) => {
                debug!("OSC 52: invalid base64: {e}");
                return;
            }
        };

        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                debug!("OSC 52: invalid UTF-8: {e}");
                return;
            }
        };

        self.event_listener
            .send_event(Event::ClipboardStore(clipboard_type, text));
    }

    /// OSC 52: request clipboard content.
    ///
    /// Sends a `ClipboardLoad` event with a closure that formats the
    /// base64-encoded response.
    pub(super) fn osc_clipboard_load(&self, clipboard: u8, terminator: &str) {
        let clipboard_type = match clipboard {
            b'c' => ClipboardType::Clipboard,
            b'p' | b's' => ClipboardType::Selection,
            _ => return,
        };

        let terminator = terminator.to_owned();
        self.event_listener.send_event(Event::ClipboardLoad(
            clipboard_type,
            Arc::new(move |text| {
                let encoded = Base64.encode(text);
                format!("\x1b]52;{};{}{}", clipboard as char, encoded, terminator)
            }),
        ));
    }

    /// OSC 8: set or clear hyperlink on cursor template.
    pub(super) fn osc_set_hyperlink(&mut self, hyperlink: Option<VteHyperlink>) {
        debug!("Setting hyperlink: {hyperlink:?}");
        let link = hyperlink.map(|h| Hyperlink { id: h.id, uri: h.uri });
        self.grid_mut().cursor_mut().template.set_hyperlink(link);
    }
}
