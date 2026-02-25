//! Unit tests for URL detection engine.

use oriterm_core::{Column, Grid, Line};

use super::{DetectedUrl, detect_urls_in_logical_line, trim_url_trailing};

/// Build a grid with the given row strings (visible rows, no scrollback).
fn grid_with_rows(rows: &[&str]) -> Grid {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(80);
    let lines = rows.len();
    let mut grid = Grid::with_scrollback(lines, cols, 0);
    for (line, text) in rows.iter().enumerate() {
        for (col, ch) in text.chars().enumerate() {
            grid[Line(line as i32)][Column(col)].ch = ch;
        }
    }
    grid
}

#[test]
fn detect_simple_url() {
    let grid = grid_with_rows(&["Visit https://example.com for info"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://example.com");
    assert_eq!(urls[0].segments.len(), 1);
    assert_eq!(urls[0].segments[0], (0, 6, 24));
}

#[test]
fn detect_multiple_urls() {
    let grid = grid_with_rows(&["see https://a.com and http://b.com/x ok"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].url, "https://a.com");
    assert_eq!(urls[1].url, "http://b.com/x");
}

#[test]
fn detect_url_with_balanced_parens() {
    let grid = grid_with_rows(&["see https://en.wikipedia.org/wiki/Rust_(language) ok"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://en.wikipedia.org/wiki/Rust_(language)");
}

#[test]
fn no_urls_in_plain_text() {
    let grid = grid_with_rows(&["just plain text here"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert!(urls.is_empty());
}

#[test]
fn detect_wrapped_url() {
    // 20-col grid: URL wraps to second row.
    use oriterm_core::cell::CellFlags;
    let mut grid = Grid::with_scrollback(2, 20, 0);
    let text = "go https://example.com/long/path ok";
    // Write first 20 chars to row 0, remaining to row 1.
    for (i, ch) in text.chars().take(20).enumerate() {
        grid[Line(0)][Column(i)].ch = ch;
    }
    // Mark row 0 as wrapped (last cell has WRAP flag).
    grid[Line(0)][Column(19)].flags.insert(CellFlags::WRAP);
    for (i, ch) in text.chars().skip(20).enumerate() {
        grid[Line(1)][Column(i)].ch = ch;
    }

    let urls = detect_urls_in_logical_line(&grid, 0, 1);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://example.com/long/path");
    assert_eq!(urls[0].segments.len(), 2);
    // First segment: starts at col 3 on row 0, goes to col 19.
    assert_eq!(urls[0].segments[0].0, 0);
    assert_eq!(urls[0].segments[0].1, 3);
    assert_eq!(urls[0].segments[0].2, 19);
    // Second segment: continues on row 1.
    assert_eq!(urls[0].segments[1].0, 1);
}

#[test]
fn url_contains() {
    let url = DetectedUrl {
        segments: vec![(5, 3, 19), (6, 0, 10)],
        url: "https://example.com/long/path".to_string(),
    };
    assert!(url.contains(5, 3));
    assert!(url.contains(5, 19));
    assert!(url.contains(6, 0));
    assert!(url.contains(6, 10));
    assert!(!url.contains(5, 2));
    assert!(!url.contains(5, 20));
    assert!(!url.contains(6, 11));
    assert!(!url.contains(7, 0));
}

#[test]
fn trim_trailing_punctuation() {
    assert_eq!(
        trim_url_trailing("https://example.com."),
        "https://example.com"
    );
    assert_eq!(
        trim_url_trailing("https://example.com,"),
        "https://example.com"
    );
    assert_eq!(
        trim_url_trailing("https://example.com;"),
        "https://example.com"
    );
    assert_eq!(
        trim_url_trailing("https://example.com:"),
        "https://example.com"
    );
    assert_eq!(
        trim_url_trailing("https://example.com!"),
        "https://example.com"
    );
    assert_eq!(
        trim_url_trailing("https://example.com?"),
        "https://example.com"
    );
}

#[test]
fn trim_preserves_balanced_parens() {
    assert_eq!(
        trim_url_trailing("https://en.wikipedia.org/wiki/Rust_(language)"),
        "https://en.wikipedia.org/wiki/Rust_(language)"
    );
}

#[test]
fn trim_strips_unbalanced_parens() {
    assert_eq!(
        trim_url_trailing("https://example.com)"),
        "https://example.com"
    );
}

#[test]
fn no_false_positive_bare_scheme() {
    let grid = grid_with_rows(&["the word https is not a url"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert!(urls.is_empty());
}

#[test]
fn ftp_and_file_schemes() {
    let grid = grid_with_rows(&["ftp://files.example.com/pub file://localhost/tmp"]);
    let urls = detect_urls_in_logical_line(&grid, 0, 0);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].url, "ftp://files.example.com/pub");
    assert_eq!(urls[1].url, "file://localhost/tmp");
}
