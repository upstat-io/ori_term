//! Tests for clipboard operations helpers.

use super::collapse_lines;

#[test]
fn collapse_single_line_unchanged() {
    assert_eq!(collapse_lines("hello world"), "hello world");
}

#[test]
fn collapse_two_lines() {
    assert_eq!(collapse_lines("hello\nworld"), "hello world");
}

#[test]
fn collapse_multiple_lines() {
    assert_eq!(
        collapse_lines("line one\nline two\nline three"),
        "line one line two line three"
    );
}

#[test]
fn collapse_trailing_newline() {
    // `str::lines()` does not produce a trailing empty element for a trailing '\n'.
    assert_eq!(collapse_lines("hello\n"), "hello");
}

#[test]
fn collapse_empty_string() {
    assert_eq!(collapse_lines(""), "");
}

#[test]
fn collapse_blank_lines_become_spaces() {
    // Two consecutive newlines produce an empty line → extra space.
    assert_eq!(collapse_lines("a\n\nb"), "a  b");
}

#[test]
fn collapse_crlf_lines() {
    // `str::lines()` handles \r\n correctly.
    assert_eq!(collapse_lines("hello\r\nworld"), "hello world");
}

#[test]
fn collapse_preserves_internal_spaces() {
    assert_eq!(
        collapse_lines("hello   world\nfoo  bar"),
        "hello   world foo  bar"
    );
}
