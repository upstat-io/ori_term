---
section: 14
title: URL Detection
status: complete
tier: 3
goal: Detect URLs in terminal output for hover underline and Ctrl+click opening
sections:
  - id: "14.1"
    title: URL Detection Engine
    status: complete
  - id: "14.2"
    title: URL Cache
    status: complete
  - id: "14.3"
    title: Hover & Click Handling
    status: complete
  - id: "14.4"
    title: Section Completion
    status: complete
---

# Section 14: URL Detection

**Status:** Complete
**Goal:** Detect URLs in terminal output using regex, provide visual hover feedback (underline + pointer cursor), and open URLs in the system browser on Ctrl+click. Handles soft-wrapped lines, balanced parentheses (Wikipedia-style URLs), and coexists with explicit OSC 8 hyperlinks.

**Crate:** `oriterm` (binary)
**Dependencies:** `regex`, `std::sync::LazyLock`
**Reference:** `_old/src/url_detect.rs`, `_old/src/app/cursor_hover.rs`

**Prerequisite:** Section 01 (Grid), Section 09 (search text extraction — shared `extract_row_text`)

---

## 14.1 URL Detection Engine

Regex-based URL detection across logical lines (sequences of soft-wrapped rows).

**File:** `oriterm/src/url_detect/mod.rs`

**Reference:** `_old/src/url_detect.rs`

- [x] `UrlSegment` type alias — `(usize, usize, usize)` = `(abs_row, start_col, end_col)` inclusive
- [x] `DetectedUrl` struct
  - [x] Fields:
    - `segments: Vec<UrlSegment>` — per-row segments (handles URLs wrapped across rows)
    - `url: String` — the extracted URL string
  - [x] `DetectedUrl::contains(&self, abs_row: usize, col: usize) -> bool`
    - [x] Check if any segment covers the given position
  - [x] Derive: `Debug`, `Clone`
- [x] URL regex pattern (static `LazyLock<Regex>`):
  - [x] `(?:https?|ftp|file)://[^\s<>\[\]'"]+`
  - [x] Covers: http, https, ftp, file schemes
  - [x] Stops at whitespace, angle brackets, square brackets, quotes
- [x] `trim_url_trailing(url: &str) -> &str`
  - [x] Strip trailing punctuation: `.`, `,`, `;`, `:`, `!`, `?`
  - [x] Handle balanced parentheses: only strip trailing `)` if unbalanced
    - [x] Count `(` and `)` in URL
    - [x] If `close > open`: strip one trailing `)`
    - [x] Repeat until stable
  - [x] Preserves Wikipedia-style URLs: `https://en.wikipedia.org/wiki/Rust_(language)`
- [x] `detect_urls_in_logical_line(grid: &Grid, line_start: usize, line_end: usize) -> Vec<DetectedUrl>`
  - [x] Concatenate text from all rows in logical line using `extract_row_text`
  - [x] Build `char_to_pos: Vec<(usize, usize)>` mapping char index to `(abs_row, col)`
  - [x] Run regex on concatenated text
  - [x] For each match:
    - [x] Trim trailing punctuation
    - [x] Skip URLs shorter than scheme prefix (e.g., bare "https://")
    - [x] Convert byte offsets to char offsets
    - [x] Skip if any cell in span has an OSC 8 hyperlink (explicit hyperlinks take precedence)
    - [x] Build per-row segments from `char_to_pos` mapping
    - [x] Emit `DetectedUrl` with segments and URL string
- [x] `logical_line_start(grid: &Grid, abs_row: usize) -> usize` — walk backwards to find first row of logical line
- [x] `logical_line_end(grid: &Grid, abs_row: usize) -> usize` — walk forwards to find last row of logical line

---

## 14.2 URL Cache

Lazy per-logical-line URL detection cache. Avoids redundant regex matching on every mouse move.

**File:** `oriterm/src/url_detect/mod.rs` (continued)

**Reference:** `_old/src/url_detect.rs` (UrlDetectCache)

- [x] `UrlDetectCache` struct
  - [x] Fields:
    - `lines: HashMap<usize, Vec<DetectedUrl>>` — logical line start row -> detected URLs
    - `row_to_line: HashMap<usize, usize>` — any row -> its logical line start (fast lookup)
  - [x] `Default` derive for empty initialization
- [x] `UrlDetectCache::url_at(&mut self, grid: &Grid, abs_row: usize, col: usize) -> Option<DetectedUrl>`
  - [x] Ensure logical line is computed (lazy)
  - [x] Search cached URLs for one containing (abs_row, col)
  - [x] Return cloned `DetectedUrl` if found
- [x] `UrlDetectCache::ensure_logical_line(&mut self, grid: &Grid, abs_row: usize) -> usize`
  - [x] If already cached (via `row_to_line`): return cached line start
  - [x] Otherwise: compute logical line bounds, detect URLs, cache results
  - [x] Register all rows in the logical line in `row_to_line`
- [x] `UrlDetectCache::invalidate(&mut self)`
  - [x] Clear both HashMaps
  - [x] Called after: PTY output, scroll, resize, font change (anything that changes grid content or layout)
- [x] Cache is per-tab (stored in Tab or binary-side wrapper)

---

## 14.3 Hover & Click Handling

Visual feedback on URL hover and opening URLs on Ctrl+click.

**File:** `oriterm/src/app/cursor_hover.rs`

**Reference:** `_old/src/app/cursor_hover.rs`, `_old/src/app/hover_url.rs`

- [x] On mouse move (while Ctrl held):
  - [x] Convert pixel position to grid cell (abs_row, col)
  - [x] Query `url_cache.url_at(grid, abs_row, col)`
  - [x] If URL found:
    - [x] Store `hovered_url: Option<DetectedUrl>` in app/tab state
    - [x] Set cursor icon to `CursorIcon::Pointer` (hand cursor)
    - [x] Underline all cells in the URL's segments (solid underline on hover)
    - [x] Request redraw
  - [x] If no URL (or Ctrl not held):
    - [x] Clear `hovered_url`
    - [x] Restore cursor icon to default
    - [x] Remove hover underline
    - [x] Request redraw if state changed
- [x] On Ctrl+click (left button):
  - [x] If `hovered_url` is Some:
    - [x] Validate URL scheme: only `http`, `https`, `ftp`, `file` allowed
    - [x] Open URL in system browser:
      - [x] Windows: `ShellExecuteW` (Win32 API)
      - [x] Linux: `xdg-open`
      - [x] macOS: `open`
    - [x] Consume the click event (don't pass to terminal/selection)
- [x] URL hover rendering integration:
  - [x] During `draw_frame`: check if cell is in `hovered_url` segments
  - [x] If yes: draw solid underline decoration at cell position
  - [x] Color: foreground color (matches text above)
- [x] Interaction with OSC 8 hyperlinks:
  - [x] Implicit URL detection skips cells that already have explicit OSC 8 hyperlinks
  - [x] OSC 8 hyperlinks have their own hover/click behavior (section 20)
- [x] Interaction with mouse reporting:
  - [x] When terminal has mouse reporting enabled: Ctrl+click still opens URL (Ctrl is override)
  - [x] Shift+click bypasses mouse reporting per xterm convention

---

## 14.4 Section Completion

- [x] All 14.1-14.3 items complete
- [x] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [x] Simple URLs detected at correct column ranges (http, https, ftp, file)
- [x] Multiple URLs on same line detected independently
- [x] Wikipedia-style parenthesized URLs preserved: `https://en.wikipedia.org/wiki/Rust_(language)`
- [x] Trailing punctuation stripped: `https://example.com.` detects `https://example.com`
- [x] Wrapped URLs: URL spanning two rows detected with correct per-row segments
- [x] OSC 8 hyperlinks not duplicated by implicit detection
- [x] Ctrl+hover: underline appears, cursor changes to pointer
- [x] Ctrl+click: URL opens in system browser
- [x] Cache invalidated on PTY output/scroll/resize (no stale URLs)
- [x] No URL on plain text: no false positives on words like "https" without "://"
- [x] **Tests** (`oriterm/src/url_detect/tests.rs`):
  - [x] Detect simple URL at correct columns
  - [x] Detect multiple URLs on same line
  - [x] Balanced parentheses preserved
  - [x] No URLs in plain text
  - [x] Wrapped URL spans two rows with correct segments
  - [x] `DetectedUrl::contains` returns correct results for all positions

**Exit Criteria:** Ctrl+hover underlines URLs in terminal output, Ctrl+click opens them in the system browser. Detection handles wrapped lines, parenthesized URLs, and coexists with explicit OSC 8 hyperlinks.
