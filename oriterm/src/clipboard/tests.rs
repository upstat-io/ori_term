use oriterm_core::event::ClipboardType;

use super::{Clipboard, ClipboardProvider};

/// Mock clipboard provider that stores text in memory.
struct MockProvider {
    text: Option<String>,
}

impl MockProvider {
    fn new() -> Self {
        Self { text: None }
    }

    fn with_text(text: &str) -> Self {
        Self {
            text: Some(text.to_owned()),
        }
    }
}

impl ClipboardProvider for MockProvider {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) -> bool {
        self.text = Some(text.to_owned());
        true
    }
}

#[test]
fn mock_round_trip() {
    let mut mock = MockProvider::new();
    assert!(mock.get_text().is_none());
    assert!(mock.set_text("hello"));
    assert_eq!(mock.get_text().as_deref(), Some("hello"));
}

#[test]
fn clipboard_store_and_load() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    cb.store(ClipboardType::Clipboard, "test text");
    assert_eq!(cb.load(ClipboardType::Clipboard), "test text");
}

#[test]
fn selection_fallback_to_clipboard() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::with_text("fallback")),
        selection: None,
    };

    // Loading Selection with no selection provider falls back to clipboard.
    assert_eq!(cb.load(ClipboardType::Selection), "fallback");
}

#[test]
fn selection_store_ignored_without_provider() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    // Storing to Selection with no selection provider is silently ignored.
    cb.store(ClipboardType::Selection, "ignored");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");
}

#[test]
fn dual_providers() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: Some(Box::new(MockProvider::new())),
    };

    cb.store(ClipboardType::Clipboard, "clipboard text");
    cb.store(ClipboardType::Selection, "selection text");

    assert_eq!(cb.load(ClipboardType::Clipboard), "clipboard text");
    assert_eq!(cb.load(ClipboardType::Selection), "selection text");
}

#[test]
fn nop_clipboard() {
    let mut cb = Clipboard::new_nop();

    // NopProvider discards stores and returns empty on load.
    cb.store(ClipboardType::Clipboard, "nop");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");
}

#[test]
fn overwrite_clipboard() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    cb.store(ClipboardType::Clipboard, "first");
    cb.store(ClipboardType::Clipboard, "second");
    assert_eq!(cb.load(ClipboardType::Clipboard), "second");
}

#[test]
fn empty_string_round_trip() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    cb.store(ClipboardType::Clipboard, "");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");
}

#[test]
fn unicode_round_trip() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    let text = "Hello \u{1F600} \u{4E16}\u{754C} caf\u{00E9}";
    cb.store(ClipboardType::Clipboard, text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn multiline_lf_round_trip() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    let text = "line one\nline two\nline three\n";
    cb.store(ClipboardType::Clipboard, text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn multiline_crlf_preserved() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    // Clipboard layer must not normalize line endings — that's the caller's job.
    let text = "line one\r\nline two\r\n";
    cb.store(ClipboardType::Clipboard, text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn control_characters_preserved() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    // Tab, escape, bell — clipboard should pass through verbatim.
    let text = "col1\tcol2\t\x1b[31mred\x07";
    cb.store(ClipboardType::Clipboard, text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn null_bytes_preserved() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    let text = "before\0after";
    cb.store(ClipboardType::Clipboard, text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn large_content_round_trip() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: None,
    };

    // 100KB of repeated text — validates no truncation in the pipeline.
    let text: String = "abcdefghij".repeat(10_000);
    cb.store(ClipboardType::Clipboard, &text);
    assert_eq!(cb.load(ClipboardType::Clipboard), text);
}

#[test]
fn failing_provider_store() {
    /// Provider that always fails to store.
    struct FailStore;
    impl ClipboardProvider for FailStore {
        fn get_text(&mut self) -> Option<String> {
            None
        }
        fn set_text(&mut self, _text: &str) -> bool {
            false
        }
    }

    let mut cb = Clipboard {
        clipboard: Box::new(FailStore),
        selection: None,
    };

    // Store failure is logged but doesn't panic.
    cb.store(ClipboardType::Clipboard, "data");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");
}

#[test]
fn selection_and_clipboard_independent() {
    let mut cb = Clipboard {
        clipboard: Box::new(MockProvider::new()),
        selection: Some(Box::new(MockProvider::new())),
    };

    // Store only to selection — clipboard stays empty.
    cb.store(ClipboardType::Selection, "sel only");
    assert_eq!(cb.load(ClipboardType::Selection), "sel only");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");

    // Store only to clipboard — selection stays untouched.
    cb.store(ClipboardType::Clipboard, "clip only");
    assert_eq!(cb.load(ClipboardType::Clipboard), "clip only");
    assert_eq!(cb.load(ClipboardType::Selection), "sel only");
}

// -- HTML clipboard (set_html) --

/// Mock provider that tracks both text and HTML separately.
struct HtmlMockProvider {
    text: Option<String>,
    html: Option<String>,
}

impl HtmlMockProvider {
    fn new() -> Self {
        Self {
            text: None,
            html: None,
        }
    }
}

impl ClipboardProvider for HtmlMockProvider {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) -> bool {
        self.text = Some(text.to_owned());
        true
    }

    fn set_html(&mut self, html: &str, alt_text: &str) -> bool {
        self.html = Some(html.to_owned());
        self.text = Some(alt_text.to_owned());
        true
    }
}

#[test]
fn store_html_sets_both_html_and_alt_text() {
    let provider = HtmlMockProvider::new();
    let mut cb = Clipboard {
        clipboard: Box::new(provider),
        selection: None,
    };

    cb.store_html("<b>hello</b>", "hello");
    // Alt text is accessible via normal load.
    assert_eq!(cb.load(ClipboardType::Clipboard), "hello");
}

#[test]
fn set_html_default_trait_falls_back_to_set_text() {
    // MockProvider doesn't override set_html, so the default trait
    // implementation should fall back to set_text with alt_text.
    let mut provider = MockProvider::new();
    let result = provider.set_html("<b>hi</b>", "hi");
    assert!(result, "default set_html should succeed via set_text");
    assert_eq!(provider.get_text().as_deref(), Some("hi"));
}

#[test]
fn store_html_with_failing_provider() {
    /// Provider that fails set_html.
    struct FailHtml;
    impl ClipboardProvider for FailHtml {
        fn get_text(&mut self) -> Option<String> {
            None
        }
        fn set_text(&mut self, _text: &str) -> bool {
            false
        }
    }

    let mut cb = Clipboard {
        clipboard: Box::new(FailHtml),
        selection: None,
    };

    // Should not panic, just log warning.
    cb.store_html("<b>x</b>", "x");
    assert_eq!(cb.load(ClipboardType::Clipboard), "");
}
