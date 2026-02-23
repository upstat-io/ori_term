//! Tests for paste text processing.

use std::path::Path;

use super::{
    count_newlines, filter_paste, format_dropped_paths, normalize_line_endings, prepare_paste,
    strip_escape_chars,
};

// --- filter_paste ---

#[test]
fn filter_empty_string() {
    assert_eq!(filter_paste(""), "");
}

#[test]
fn filter_only_tabs_produces_empty() {
    // All-whitespace (tabs only) filtered to empty — downstream must handle empty result.
    assert_eq!(filter_paste("\t\t\t"), "");
}

#[test]
fn filter_strips_tabs() {
    assert_eq!(filter_paste("hello\tworld"), "helloworld");
    assert_eq!(filter_paste("\t\t"), "");
}

#[test]
fn filter_converts_smart_double_quotes() {
    // U+201C left double quotation mark, U+201D right double quotation mark.
    assert_eq!(filter_paste("\u{201C}hello\u{201D}"), "\"hello\"");
}

#[test]
fn filter_converts_smart_single_quotes() {
    // U+2018 left single quotation mark, U+2019 right single quotation mark.
    assert_eq!(filter_paste("\u{2018}it\u{2019}s"), "'it's");
}

#[test]
fn filter_converts_em_dash_to_double_hyphen() {
    assert_eq!(filter_paste("hello\u{2014}world"), "hello--world");
}

#[test]
fn filter_converts_en_dash_to_hyphen() {
    assert_eq!(filter_paste("2020\u{2013}2025"), "2020-2025");
}

#[test]
fn filter_converts_non_breaking_spaces() {
    assert_eq!(filter_paste("hello\u{00A0}world"), "hello world");
    assert_eq!(filter_paste("hello\u{202F}world"), "hello world");
}

#[test]
fn filter_preserves_normal_text() {
    let text = "Hello, world! 123 @#$";
    assert_eq!(filter_paste(text), text);
}

#[test]
fn filter_combined_characters() {
    let input = "\u{201C}Don\u{2019}t\u{201D}\t\u{2014}\u{00A0}test";
    assert_eq!(filter_paste(input), "\"Don't\"-- test");
}

// --- normalize_line_endings ---

#[test]
fn normalize_empty_string() {
    assert_eq!(normalize_line_endings(""), "");
}

#[test]
fn normalize_consecutive_lf_preserved() {
    // Double newline must become double CR, not collapsed.
    assert_eq!(normalize_line_endings("a\n\nb"), "a\r\rb");
}

#[test]
fn normalize_only_newlines() {
    assert_eq!(normalize_line_endings("\n\n\n"), "\r\r\r");
}

#[test]
fn normalize_trailing_newline() {
    assert_eq!(normalize_line_endings("hello\n"), "hello\r");
}

#[test]
fn normalize_leading_newline() {
    assert_eq!(normalize_line_endings("\nhello"), "\rhello");
}

#[test]
fn normalize_crlf_to_cr() {
    assert_eq!(normalize_line_endings("hello\r\nworld"), "hello\rworld");
}

#[test]
fn normalize_standalone_lf_to_cr() {
    assert_eq!(normalize_line_endings("hello\nworld"), "hello\rworld");
}

#[test]
fn normalize_standalone_cr_unchanged() {
    assert_eq!(normalize_line_endings("hello\rworld"), "hello\rworld");
}

#[test]
fn normalize_mixed_line_endings() {
    // CRLF, then LF, then CR.
    assert_eq!(normalize_line_endings("a\r\nb\nc\rd"), "a\rb\rc\rd");
}

#[test]
fn normalize_preserves_unicode() {
    assert_eq!(normalize_line_endings("日本語\r\nテスト"), "日本語\rテスト");
}

#[test]
fn normalize_consecutive_crlf() {
    assert_eq!(normalize_line_endings("a\r\n\r\nb"), "a\r\rb");
}

// --- strip_escape_chars ---

#[test]
fn strip_multiple_esc_sequences() {
    assert_eq!(
        strip_escape_chars("\x1b[31mred\x1b[0m normal \x1b[1mbold\x1b[0m"),
        "[31mred[0m normal [1mbold[0m"
    );
}

#[test]
fn strip_only_esc_chars() {
    assert_eq!(strip_escape_chars("\x1b\x1b\x1b"), "");
}

#[test]
fn strip_removes_esc() {
    assert_eq!(strip_escape_chars("hello\x1b[31mworld"), "hello[31mworld");
}

#[test]
fn strip_preserves_text_without_esc() {
    let text = "hello world 123";
    assert_eq!(strip_escape_chars(text), text);
}

#[test]
fn strip_removes_bracketed_paste_end_marker() {
    // The ESC in \x1b[201~ is stripped, preventing paste escape.
    assert_eq!(
        strip_escape_chars("injected\x1b[201~text"),
        "injected[201~text"
    );
}

// --- count_newlines ---

#[test]
fn count_empty_string() {
    assert_eq!(count_newlines(""), 0);
}

#[test]
fn count_only_newlines() {
    assert_eq!(count_newlines("\n\n\n"), 3);
}

#[test]
fn count_trailing_newline() {
    assert_eq!(count_newlines("hello\n"), 1);
}

#[test]
fn count_no_newlines() {
    assert_eq!(count_newlines("hello world"), 0);
}

#[test]
fn count_single_lf() {
    assert_eq!(count_newlines("hello\nworld"), 1);
}

#[test]
fn count_crlf_as_one() {
    assert_eq!(count_newlines("hello\r\nworld"), 1);
}

#[test]
fn count_mixed_newlines() {
    assert_eq!(count_newlines("a\r\nb\nc\rd"), 3);
}

// --- prepare_paste ---

#[test]
fn prepare_empty_string_plain() {
    assert_eq!(prepare_paste("", false, false), b"");
}

#[test]
fn prepare_empty_string_bracketed() {
    // Empty paste with bracketed mode should emit empty brackets.
    assert_eq!(prepare_paste("", true, false), b"\x1b[200~\x1b[201~");
}

#[test]
fn prepare_bracketed_neutralizes_end_marker() {
    // Text containing the bracketed paste end sequence `\x1b[201~` must be
    // de-fanged so the application can't be tricked into ending paste early.
    let result = prepare_paste("injected\x1b[201~tail", true, false);
    assert_eq!(result, b"\x1b[200~injected[201~tail\x1b[201~");
}

#[test]
fn prepare_nul_bytes_passed_through() {
    // NUL bytes in paste text pass through (terminal/PTY handles them).
    let result = prepare_paste("a\0b", false, false);
    assert_eq!(result, b"a\0b");
}

#[test]
fn prepare_plain_no_filter() {
    let result = prepare_paste("hello\nworld", false, false);
    assert_eq!(result, b"hello\rworld");
}

#[test]
fn prepare_with_filter() {
    let result = prepare_paste("hello\tworld", false, true);
    assert_eq!(result, b"helloworld");
}

#[test]
fn prepare_bracketed() {
    let result = prepare_paste("hello", true, false);
    assert_eq!(result, b"\x1b[200~hello\x1b[201~");
}

#[test]
fn prepare_bracketed_strips_esc() {
    let result = prepare_paste("hello\x1bworld", true, false);
    assert_eq!(result, b"\x1b[200~helloworld\x1b[201~");
}

#[test]
fn prepare_bracketed_with_filter_and_crlf() {
    let result = prepare_paste("\u{201C}hi\u{201D}\r\n", true, true);
    // Filter: smart quotes → straight. Normalize: CRLF → CR. Strip ESC: no ESC to strip.
    assert_eq!(result, b"\x1b[200~\"hi\"\r\x1b[201~");
}

// --- format_dropped_paths ---

#[test]
fn format_single_path_no_spaces() {
    let paths = [Path::new("/home/user/file.txt")];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(format_dropped_paths(&refs), "/home/user/file.txt");
}

#[test]
fn format_single_path_with_spaces() {
    let paths = [Path::new("/home/user/my file.txt")];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(format_dropped_paths(&refs), "\"/home/user/my file.txt\"");
}

#[test]
fn format_multiple_paths() {
    let paths = [
        Path::new("/home/user/a.txt"),
        Path::new("/home/user/my file.txt"),
        Path::new("/tmp/b.txt"),
    ];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(
        format_dropped_paths(&refs),
        "/home/user/a.txt \"/home/user/my file.txt\" /tmp/b.txt"
    );
}

#[test]
fn format_path_with_quotes() {
    // Paths containing double quotes are not double-escaped — they pass through.
    // Shell interpretation is the user's responsibility.
    let paths = [Path::new("/home/user/file\"name.txt")];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(format_dropped_paths(&refs), "/home/user/file\"name.txt");
}

#[test]
fn format_empty_paths() {
    let refs: Vec<&Path> = vec![];
    assert_eq!(format_dropped_paths(&refs), "");
}

#[test]
fn format_path_with_backslashes() {
    // Windows-style paths with backslashes.
    let paths = [Path::new("C:\\Users\\test\\file.txt")];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(format_dropped_paths(&refs), "C:\\Users\\test\\file.txt");
}

#[test]
fn format_path_backslash_with_spaces() {
    // Windows-style path with spaces gets quoted.
    let paths = [Path::new("C:\\Program Files\\app.exe")];
    let refs: Vec<&Path> = paths.iter().copied().collect();
    assert_eq!(
        format_dropped_paths(&refs),
        "\"C:\\Program Files\\app.exe\""
    );
}

// --- paste: OSC/CSI injection defense ---

#[test]
fn prepare_bracketed_strips_osc_title_injection() {
    // OSC sequence (title set) inside bracketed paste — ESC is stripped.
    let result = prepare_paste("before\x1b]0;evil\x07after", true, false);
    assert_eq!(result, b"\x1b[200~before]0;evil\x07after\x1b[201~");
}

#[test]
fn prepare_bracketed_strips_csi_sgr_injection() {
    // CSI SGR sequence inside bracketed paste — ESC is stripped.
    let result = prepare_paste("test\x1b[31mred", true, false);
    assert_eq!(result, b"\x1b[200~test[31mred\x1b[201~");
}

#[test]
fn prepare_bracketed_strips_multiple_esc_sequences() {
    // Multiple ESC chars scattered through paste text.
    let result = prepare_paste("\x1bA\x1bB\x1bC", true, false);
    assert_eq!(result, b"\x1b[200~ABC\x1b[201~");
}

// --- normalize_line_endings: long chains ---

#[test]
fn normalize_many_consecutive_crlf() {
    // 5 consecutive CRLF pairs → 5 consecutive CR.
    let input = "a\r\n\r\n\r\n\r\n\r\nb";
    assert_eq!(normalize_line_endings(input), "a\r\r\r\r\rb");
}

#[test]
fn normalize_alternating_cr_lf_crlf() {
    // Mixed: bare CR, bare LF, CRLF — all become CR.
    assert_eq!(normalize_line_endings("a\rb\nc\r\nd"), "a\rb\rc\rd");
}

// --- plain mode passes ESC through ---

#[test]
fn prepare_plain_preserves_esc_sequences() {
    // In plain (non-bracketed) mode, ESC chars pass through to the PTY.
    // Only bracketed mode strips ESC. This is intentional: plain-mode pastes
    // may legitimately contain escape sequences.
    let result = prepare_paste("before\x1b[201~after", false, false);
    assert_eq!(
        result, b"before\x1b[201~after",
        "plain mode should not strip ESC"
    );
}
