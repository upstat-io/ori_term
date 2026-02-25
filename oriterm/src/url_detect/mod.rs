//! Implicit URL detection in grid text using regex patterns.
//!
//! Detects URLs (http, https, ftp, file) in terminal output by running a regex
//! over logical lines (sequences of soft-wrapped rows). Results are cached per
//! logical line and invalidated on grid content changes. Explicit OSC 8
//! hyperlinks take precedence — cells with hyperlinks are skipped.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use oriterm_core::{CellFlags, Column, Grid, extract_row_text};

/// A single row-segment of a detected URL: `(abs_row, start_col, end_col)` inclusive.
pub type UrlSegment = (usize, usize, usize);

/// A URL detected across one or more grid rows (handles soft-wrapped lines).
#[derive(Debug, Clone)]
pub struct DetectedUrl {
    /// Per-row segments, each with inclusive column bounds.
    pub segments: Vec<UrlSegment>,
    /// The extracted URL string.
    pub url: String,
}

impl DetectedUrl {
    /// Check whether this URL covers (`abs_row`, `col`).
    pub fn contains(&self, abs_row: usize, col: usize) -> bool {
        self.segments
            .iter()
            .any(|&(r, sc, ec)| r == abs_row && col >= sc && col <= ec)
    }
}

/// Cache of detected URLs keyed by the first absolute row of the logical line.
///
/// Lazily computes URLs for logical lines (sequences of wrapped rows) and caches
/// them to avoid redundant regex matching on hover/click.
#[derive(Default)]
pub struct UrlDetectCache {
    /// Logical line start row -> detected URLs for that logical line.
    lines: HashMap<usize, Vec<DetectedUrl>>,
    /// Row index -> logical line start (for fast lookup of any row).
    row_to_line: HashMap<usize, usize>,
}

impl UrlDetectCache {
    /// Finds a URL at the specified grid position, computing and caching
    /// the logical line if needed.
    ///
    /// Returns the detected URL if one covers this position.
    pub fn url_at(&mut self, grid: &Grid, abs_row: usize, col: usize) -> Option<DetectedUrl> {
        let line_start = self.ensure_logical_line(grid, abs_row);
        let urls = self.lines.get(&line_start)?;
        urls.iter().find(|u| u.contains(abs_row, col)).cloned()
    }

    /// Ensures the logical line containing the row is computed and cached.
    ///
    /// Returns the absolute row index of the logical line start.
    fn ensure_logical_line(&mut self, grid: &Grid, abs_row: usize) -> usize {
        if let Some(&ls) = self.row_to_line.get(&abs_row) {
            return ls;
        }
        let line_start = url_logical_line_start(grid, abs_row);
        let line_end = url_logical_line_end(grid, abs_row);

        let urls = detect_urls_in_logical_line(grid, line_start, line_end);

        for r in line_start..=line_end {
            self.row_to_line.insert(r, line_start);
        }
        self.lines.insert(line_start, urls);
        line_start
    }

    /// Invalidates the entire cache.
    ///
    /// Call after PTY output, scroll, resize, or font change — anything that
    /// changes grid content or layout.
    pub fn invalidate(&mut self) {
        self.lines.clear();
        self.row_to_line.clear();
    }
}

/// URL regex pattern: matches http, https, ftp, file schemes.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:https?|ftp|file)://[^\s<>\[\]'"]+"#).expect("URL regex is valid")
});

/// Trims trailing punctuation from a URL, preserving balanced parentheses.
///
/// Strips `.`, `,`, `;`, `:`, `!`, `?` from the end. Only strips trailing `)`
/// when parentheses are unbalanced, preserving Wikipedia-style URLs like
/// `https://en.wikipedia.org/wiki/Rust_(language)`.
fn trim_url_trailing(url: &str) -> &str {
    let mut s = url;
    loop {
        let prev = s;
        s = s.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        // Trim trailing ')' only if unbalanced.
        if let Some(stripped) = s.strip_suffix(')') {
            let open = s.chars().filter(|&c| c == '(').count();
            let close = s.chars().filter(|&c| c == ')').count();
            if close > open {
                s = stripped;
            }
        }
        if s == prev {
            break;
        }
    }
    s
}

/// Detects URLs across a logical line spanning `line_start..=line_end` (absolute rows).
///
/// Concatenates text from all rows, runs the regex, then maps byte spans
/// back to per-row segments. Skips cells with OSC 8 hyperlinks.
#[expect(
    clippy::string_slice,
    reason = "char-to-byte offset mapping is validated"
)]
fn detect_urls_in_logical_line(
    grid: &Grid,
    line_start: usize,
    line_end: usize,
) -> Vec<DetectedUrl> {
    let mut text = String::new();
    let mut char_to_pos: Vec<(usize, usize)> = Vec::new();

    for abs_row in line_start..=line_end {
        let Some(row) = grid.absolute_row(abs_row) else {
            continue;
        };
        let (row_text, col_map) = extract_row_text(row);
        for (ci, _ch) in row_text.chars().enumerate() {
            let col = col_map.get(ci).copied().unwrap_or(0);
            char_to_pos.push((abs_row, col));
        }
        text.push_str(&row_text);
    }

    let mut urls = Vec::new();

    for m in URL_RE.find_iter(&text) {
        let trimmed = trim_url_trailing(m.as_str());
        if trimmed.len() <= "https://".len() {
            continue;
        }

        // Convert byte offsets to char offsets.
        let char_start = text[..m.start()].chars().count();
        let trimmed_char_len = trimmed.chars().count();
        let char_end = char_start + trimmed_char_len - 1; // inclusive

        if char_end >= char_to_pos.len() {
            continue;
        }

        let segments = build_segments(&char_to_pos, char_start, char_end);
        urls.push(DetectedUrl {
            segments,
            url: trimmed.to_string(),
        });
    }

    urls
}

/// Build per-row segments from a character span.
fn build_segments(
    char_to_pos: &[(usize, usize)],
    char_start: usize,
    char_end: usize,
) -> Vec<UrlSegment> {
    let mut segments: Vec<UrlSegment> = Vec::new();
    let mut current_row = char_to_pos[char_start].0;
    let mut seg_start_col = char_to_pos[char_start].1;
    let mut seg_end_col = seg_start_col;

    for &(ar, col) in &char_to_pos[char_start..=char_end] {
        if ar != current_row {
            segments.push((current_row, seg_start_col, seg_end_col));
            current_row = ar;
            seg_start_col = col;
        }
        seg_end_col = col;
    }
    segments.push((current_row, seg_start_col, seg_end_col));

    segments
}

/// Check whether a row continues onto the next row for URL detection.
///
/// Uses the `WrapOrFilled` heuristic from the old prototype: a row continues
/// if the WRAP flag is set OR the last cell is non-empty. This catches
/// application-driven wrapping where programs break long output (including
/// URLs) at the terminal width without the terminal auto-wrapping.
fn row_continues_for_url(grid: &Grid, abs_row: usize) -> bool {
    let Some(row) = grid.absolute_row(abs_row) else {
        return false;
    };
    let cols = row.cols();
    if cols == 0 {
        return false;
    }
    let last = &row[Column(cols - 1)];
    last.flags.contains(CellFlags::WRAP) || (last.ch != '\0' && last.ch != ' ')
}

/// Walk backwards to find the start of a logical line for URL detection.
///
/// Uses `WrapOrFilled` heuristic — see [`row_continues_for_url`].
fn url_logical_line_start(grid: &Grid, abs_row: usize) -> usize {
    let mut current = abs_row;
    while current > 0 {
        if row_continues_for_url(grid, current - 1) {
            current -= 1;
        } else {
            break;
        }
    }
    current
}

/// Walk forwards to find the end of a logical line for URL detection.
///
/// Uses `WrapOrFilled` heuristic — see [`row_continues_for_url`].
fn url_logical_line_end(grid: &Grid, abs_row: usize) -> usize {
    let mut current = abs_row;
    loop {
        if row_continues_for_url(grid, current) {
            current += 1;
        } else {
            break;
        }
    }
    current
}

#[cfg(test)]
mod tests;
