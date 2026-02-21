---
section: 9
title: Selection & Clipboard
status: in-progress
tier: 3
goal: Windows Terminal-style 3-point selection, all selection modes, clipboard with paste filtering
sections:
  - id: "9.1"
    title: Selection Model & Anchoring
    status: in-progress
  - id: "9.2"
    title: Mouse Selection
    status: not-started
  - id: "9.3"
    title: Keyboard Selection (Mark Mode)
    status: not-started
  - id: "9.4"
    title: Word Delimiters & Boundaries
    status: in-progress
  - id: "9.5"
    title: Copy Operations
    status: in-progress
  - id: "9.6"
    title: Paste Operations
    status: not-started
  - id: "9.7"
    title: Selection Rendering
    status: not-started
  - id: "9.8"
    title: Section Completion
    status: not-started
---

# Section 09: Selection & Clipboard

**Status:** Not Started
**Goal:** Implement text selection and clipboard modeled after Windows Terminal, which has the best selection/clipboard UX of any terminal emulator. 3-point selection with char/word/line/block modes, smart copy with formatting, paste filtering, and bracketed paste.

**Crate:** `oriterm_core` (selection model, boundaries, text extraction), `oriterm` (mouse/keyboard integration, clipboard I/O, rendering)
**Dependencies:** `clipboard-win` (Windows clipboard), `oriterm_core` (Grid, Cell, CellFlags)
**Reference:** `_old/src/selection/`, `_old/src/app/mouse_selection.rs`, `_old/src/clipboard.rs`

**Modeled after:** Windows Terminal's selection and clipboard implementation. Key source files: `Selection.cpp`, `Clipboard.cpp`, `ControlInteractivity.cpp`, `textBuffer/TextBuffer.cpp`.

**Prerequisite:** Section 01 complete (Grid, Cell, Row data structures). Section 06 complete (keyboard input dispatch for keybinding wiring).

---

## 9.1 Selection Model & Anchoring

Windows Terminal uses a 3-point selection model: anchor, pivot, and endpoint. The pivot prevents losing the initially selected unit (word or line) during drag.

**Files:** `oriterm_core/src/selection/mod.rs`, `oriterm_core/src/selection/boundaries.rs`, `oriterm_core/src/selection/text.rs`

**Reference:** `_old/src/selection/mod.rs` — carries forward the proven 3-point model with `SelectionPoint`, `Selection`, `SelectionMode`.

- [x] `Side` enum — `Left`, `Right`
  - [x] Sub-cell precision for selection boundaries (which half of the cell was clicked)
  - [x] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [x] `SelectionPoint` struct
  - [x] Fields:
    - `row: StableRowIndex` — row identity that survives scrollback eviction
    - `col: usize` — column index
    - `side: Side` — which half of the cell
  - [x] `effective_start_col(&self) -> usize` — when `side == Right`, selection starts at `col + 1`
  - [x] `effective_end_col(&self) -> usize` — when `side == Left && col > 0`, selection ends at `col - 1`
  - [x] `impl Ord` — compare by row, then col, then side (Left < Right)
  - [x] `impl PartialOrd` — delegate to `Ord`
  - [x] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [x] `SelectionMode` enum
  - [x] `Char` — character-by-character (single click + drag)
  - [x] `Word` — word selection (double-click, subsequent drag expands by words)
  - [x] `Line` — full logical line selection (triple-click, follows WRAPLINE)
  - [x] `Block` — rectangular block selection (Alt+click+drag)
  - [x] Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`
- [x] `Selection` struct
  - [x] Fields:
    - `mode: SelectionMode`
    - `anchor: SelectionPoint` — initial click position (fixed)
    - `pivot: SelectionPoint` — other end of initial unit (word end, line end); prevents losing selected word during drag
    - `end: SelectionPoint` — current drag endpoint (moves with mouse)
  - [x] `Selection::new_char(row: StableRowIndex, col: usize, side: Side) -> Self` — anchor = pivot = end
  - [x] `Selection::new_word(anchor: SelectionPoint, pivot: SelectionPoint) -> Self` — anchor/pivot set to word boundaries
  - [x] `Selection::new_line(anchor: SelectionPoint, pivot: SelectionPoint) -> Self` — anchor/pivot set to line boundaries
  - [x] `ordered(&self) -> (SelectionPoint, SelectionPoint)` — normalize: sort anchor, pivot, end and return (min, max)
  - [x] `contains(&self, stable_row: StableRowIndex, col: usize) -> bool` — test if cell is within selection
    - [x] Block mode: rectangular bounds (min_col..max_col within row range)
    - [x] Other modes: use effective_start_col/effective_end_col at boundary rows, full rows in between
  - [x] `is_empty(&self) -> bool` — true if Char mode and anchor == end (zero area)
- [x] Selection across scrollback: points use `StableRowIndex` (absolute row positions that survive scrollback eviction)
- [ ] Selection invalidation: clear on output that affects selected region
- [ ] Multi-click detection:
  - [ ] Track last click position and timestamp
  - [ ] Use 500ms window for multi-click detection
  - [ ] Click counter cycles: 1 -> 2 -> 3 -> 1 (single -> double -> triple -> reset)
  - [ ] Clicks must be in same cell position to count as multi-click
- [x] Re-export `Selection`, `SelectionPoint`, `SelectionMode`, `Side` from `oriterm_core/src/lib.rs`
- [x] **Tests** (`oriterm_core/src/selection/mod.rs` `#[cfg(test)]`):
  - [x] `new_char` creates selection with anchor == pivot == end
  - [x] `new_word` creates selection with distinct anchor and pivot
  - [x] `ordered()` returns min/max regardless of anchor/end order
  - [x] `contains()` returns true for cells inside selection, false outside
  - [x] `contains()` respects Side precision at boundary cells
  - [x] Block mode `contains()` uses rectangular bounds
  - [x] `is_empty()` returns true for zero-area Char selection
  - [x] SelectionPoint ordering: row takes priority, then col, then side

---

## 9.2 Mouse Selection

Windows Terminal-style mouse selection with drag threshold, multi-click modes, and auto-scroll.

**File:** `oriterm/src/app/mouse_selection.rs`

**Reference:** `_old/src/app/mouse_selection.rs` — carries forward click counting, word/line selection creation, drag endpoint updates.

- [ ] **Click count detection** (`detect_click_count`):
  - [ ] Track `last_click_time: Option<Instant>`, `last_click_pos: Option<(usize, usize)>`, `click_count: u8`
  - [ ] Same position + same window + within 500ms: increment count (1 -> 2 -> 3 -> 1)
  - [ ] Different position or expired window: reset to 1
- [ ] **Drag threshold**: Selection only starts after cursor moves >= 1/4 cell width from initial click position
  - [ ] Track touchdown position separately from selection anchor
  - [ ] Only initiate selection once threshold exceeded (prevents accidental selection)
- [ ] **Single click + drag** — Character selection:
  - [ ] Convert pixel position to cell coordinates (account for display_offset, tab bar offset)
  - [ ] Determine Side (Left/Right) from pixel sub-cell position
  - [ ] Clear any existing selection
  - [ ] Set anchor at click position with `Selection::new_char()`
  - [ ] Drag extends endpoint via `update_selection_end()`
- [ ] **Double-click** — Word selection:
  - [ ] Compute word boundaries around click position (see 9.4)
  - [ ] Create selection with `Selection::new_word(start_boundary, end_boundary)`
  - [ ] Pivot set to expanded word boundaries
  - [ ] Subsequent drag expands by words: compare drag position to anchor, use nearest word boundary
- [ ] **Triple-click** — Line selection:
  - [ ] Select entire logical line (follows wrapped lines via WRAPLINE flag)
  - [ ] Walk backwards through `logical_line_start()` to find first row of logical line
  - [ ] Walk forwards through `logical_line_end()` to find last row
  - [ ] Start at (first_row, col 0, Side::Left), end at (last_row, last_col, Side::Right)
  - [ ] Create selection with `Selection::new_line()`
- [ ] **Alt+click+drag** — Toggle block/character mode:
  - [ ] If current mode is Char or Line: switch to `SelectionMode::Block`
  - [ ] If current mode is Block: switch to `SelectionMode::Char`
- [ ] **Shift+click** — Extend existing selection:
  - [ ] If selection exists: update endpoint to clicked position
  - [ ] If click is beyond anchor: include clicked cell
  - [ ] If click is before anchor: start from clicked position
  - [ ] Respect double-wide character boundaries
- [ ] **Ctrl+click** — Open hyperlink URL:
  - [ ] Check OSC 8 hyperlink on clicked cell (takes priority)
  - [ ] Fall through to implicit URL detection
  - [ ] If URL found: open in default browser, consume click
- [ ] **Auto-scroll during drag** (mouse above/below viewport):
  - [ ] When dragging above grid top: scroll viewport up into history (1 line per event)
  - [ ] When dragging below grid bottom: scroll viewport down toward live (if display_offset > 0)
  - [ ] Continue extending selection into scrollback during auto-scroll
- [ ] **Double-wide character handling**:
  - [ ] Selection never splits a double-wide character
  - [ ] If click lands on WIDE_CHAR_SPACER: redirect to base cell (col - 1)
  - [ ] Automatically adjust selection endpoint to cell boundary
- [ ] **Tests** (`oriterm/src/app/mouse_selection.rs` `#[cfg(test)]`):
  - [ ] Click count detection: rapid clicks cycle 1 -> 2 -> 3 -> 1
  - [ ] Click at different position resets to 1
  - [ ] Expired click window resets to 1
  - [ ] Double-click creates Word selection with correct boundaries
  - [ ] Triple-click creates Line selection spanning wrapped lines
  - [ ] Alt+click toggles block mode
  - [ ] Shift+click extends existing selection

---

## 9.3 Keyboard Selection (Mark Mode)

Keyboard-driven selection for accessibility and power users, modeled after Windows Terminal's mark mode.

**File:** `oriterm/src/app/input_keyboard.rs` (mark mode branch in dispatch)

- [ ] **Enter mark mode**: Ctrl+Shift+M
  - [ ] Set `mark_mode: bool` on active tab
  - [ ] Show visual cursor at current terminal cursor position
  - [ ] Arrow keys move selection cursor (not terminal cursor, not sent to PTY)
- [ ] **Shift+Arrow keys** — Extend selection by one cell:
  - [ ] Shift+Left/Right: extend by one column
  - [ ] Shift+Up/Down: extend by one row
- [ ] **Ctrl+Shift+Arrow keys** — Extend selection by word:
  - [ ] Ctrl+Shift+Left: extend to previous word boundary
  - [ ] Ctrl+Shift+Right: extend to next word boundary
- [ ] **Shift+Page Up/Down** — Extend by one screen:
  - [ ] Selection extends by `grid.lines` rows
- [ ] **Shift+Home/End** — Extend to line boundaries:
  - [ ] Shift+Home: extend to start of current line (column 0)
  - [ ] Shift+End: extend to end of current line (last non-empty column)
- [ ] **Ctrl+Shift+Home/End** — Extend to buffer boundaries:
  - [ ] Ctrl+Shift+Home: extend to top of scrollback
  - [ ] Ctrl+Shift+End: extend to bottom of buffer
- [ ] **Ctrl+A** — Select all:
  - [ ] If cursor is in shell input line (with shell integration): select input line
  - [ ] Otherwise: select entire buffer (visible + scrollback)
- [ ] **Escape** — Cancel selection:
  - [ ] Clear selection
  - [ ] Exit mark mode
- [ ] **Enter** — Copy and exit:
  - [ ] Copy current selection to clipboard
  - [ ] Exit mark mode
- [ ] **Tests**:
  - [ ] Enter mark mode sets flag, exit clears it
  - [ ] Shift+Right extends selection by one column
  - [ ] Ctrl+A selects entire buffer
  - [ ] Escape clears selection and exits mark mode

---

## 9.4 Word Delimiters & Boundaries

Configurable word boundary detection for double-click selection and Ctrl+arrow word movement.

**File:** `oriterm_core/src/selection/boundaries.rs`

**Reference:** `_old/src/selection/boundaries.rs` — carries forward the char_class + scan approach.

- [x] **Default word delimiters**: ``[]{}()=\,;"'-`` plus space (always a delimiter)
- [x] **Character classification** (`fn char_class(ch: char) -> u8`):
  - [x] Class 0: Word characters (alphanumeric + `_`)
  - [x] Class 1: Whitespace (space, `\0`, tab)
  - [x] Class 2: Punctuation/other (all other characters)
  - [x] Two non-zero classes allow asymmetric word navigation behavior
- [x] `is_word_delimiter(ch: char) -> bool` — returns true if class != 0
- [x] `delimiter_class(ch: char) -> u8` — returns classification
- [x] `word_boundaries(grid: &Grid, abs_row: usize, col: usize) -> (usize, usize)`
  - [x] Returns (start_col, end_col) inclusive
  - [x] If clicked on WIDE_CHAR_SPACER: redirect to base cell (col - 1)
  - [x] Classify the clicked character
  - [x] Scan left: move while `char_class(cell.c) == click_class`, skipping WIDE_CHAR_SPACER
  - [x] Scan right: move while `char_class(cell.c) == click_class`, including WIDE_CHAR_SPACER that follows a wide char
  - [x] Returns (start, end) of contiguous same-class region
- [x] `logical_line_start(grid: &Grid, abs_row: usize) -> usize`
  - [x] Walk backwards through rows connected by WRAPLINE flag
  - [x] Returns absolute row index of first row in logical line
- [x] `logical_line_end(grid: &Grid, abs_row: usize) -> usize`
  - [x] Walk forwards through rows connected by WRAPLINE flag
  - [x] Returns absolute row index of last row in logical line
- [ ] Configurable delimiters via settings (future: wired through config in Section 13) <!-- blocked-by:13 -->
- [x] **Tests** (`oriterm_core/src/selection/boundaries.rs` `#[cfg(test)]`):
  - [x] `char_class('a')` returns 0 (word)
  - [x] `char_class(' ')` returns 1 (whitespace)
  - [x] `char_class(';')` returns 2 (punctuation)
  - [x] `word_boundaries` on "hello world" at col 2 returns (0, 4)
  - [x] `word_boundaries` on "hello world" at col 5 returns (5, 5) (space is its own unit)
  - [x] `word_boundaries` on wide char spacer redirects to base cell
  - [x] `logical_line_start` walks back through WRAPLINE rows
  - [x] `logical_line_end` walks forward through WRAPLINE rows

---

## 9.5 Copy Operations <!-- unblocks:8.3 -->

Windows Terminal copies multiple clipboard formats simultaneously. Smart copy behavior adapts to context.

**File:** `oriterm/src/clipboard.rs` (clipboard I/O), `oriterm_core/src/selection/text.rs` (text extraction)

**Reference:** `_old/src/selection/text.rs` — carries forward text extraction with wrap handling, spacer skipping, grapheme cluster support.

- [ ] **Copy triggers**:
  - [ ] Ctrl+Shift+C — copy selection
  - [ ] Ctrl+C — smart: copy if selection exists, send SIGINT (`\x03`) if not
  - [ ] Ctrl+Insert — copy selection
  - [ ] Enter — copy selection (in mark mode, then exit mark mode)
  - [ ] CopyOnSelect setting: auto-copy on mouse release after selection (does NOT clear selection)
  - [ ] Right-click: copy if selection exists (when context menu disabled)
- [x] **Text extraction** (`extract_text(grid: &Grid, selection: &Selection) -> String`):
  - [x] Convert StableRowIndex to absolute row for iteration
  - [x] Walk selected cells, concatenate characters
  - [x] Skip WIDE_CHAR_SPACER cells (include the wide char cell, not its spacer)
  - [x] Skip LEADING_WIDE_CHAR_SPACER cells
  - [x] Replace `\0` (null) with space
  - [x] Append zero-width characters (combining marks) from `cell.zerowidth()`
  - [x] Handle wrapped lines: rows connected by WRAPLINE flag join without newline
  - [x] Unwrapped lines: trim trailing spaces, add newline between rows
  - [x] Block selection: add newlines between rows, trim trailing spaces per row, use min_col..max_col bounds
  - [x] Handle grapheme clusters: base char + all zerowidth chars from CellExtra
- [ ] **Clipboard formats** (placed on clipboard simultaneously):
  - [ ] `CF_UNICODETEXT` — plain text (always)
  - [ ] `HTML Format` — HTML with inline styles (if CopyFormatting enabled)
    - [ ] Per-cell foreground/background colors as inline CSS
    - [ ] Font name and size
    - [ ] Bold rendering for BOLD cells
    - [ ] Underline colors
  - [ ] `Rich Text Format` — RTF with same styling (if CopyFormatting enabled)
- [ ] **Copy modifiers**:
  - [ ] Shift held during copy: collapse multi-line selection to single line (join with spaces)
  - [ ] Alt held during copy: force HTML/RTF formatting regardless of CopyFormatting setting
- [ ] Selection NOT cleared after copy (user must press Escape or click elsewhere)
- [ ] **OSC 52 clipboard integration**:
  - [ ] Application can set clipboard via `ESC]52;c;{base64_data}ST`
  - [ ] Application can request clipboard (if permitted by config)
- [x] **Tests** (`oriterm_core/src/selection/tests.rs`):
  - [x] Extract text from single row: correct characters
  - [x] Extract text skips WIDE_CHAR_SPACER
  - [x] Extract text includes zero-width chars (combining marks)
  - [x] Wrapped lines joined without newline
  - [x] Unwrapped lines separated by newline
  - [x] Trailing spaces trimmed per row
  - [x] Block selection extracts rectangular region
  - [x] Null chars replaced with spaces

---

## 9.6 Paste Operations

Windows Terminal-style paste with character filtering, line ending normalization, and bracketed paste support.

**File:** `oriterm/src/clipboard.rs` (continued)

**Reference:** `_old/src/clipboard.rs`

- [ ] **Paste triggers**:
  - [ ] Ctrl+Shift+V — paste from clipboard
  - [ ] Ctrl+V — paste (when no VT conflict)
  - [ ] Shift+Insert — paste
  - [ ] Right-click — paste (when no selection and context menu disabled)
- [ ] **Character filtering on paste** (configurable `FilterOnPaste` setting):
  | Character | Behavior |
  |-----------|----------|
  | Tab (`\t`) | Strip (prevents tab expansion issues) |
  | Non-breaking space (U+00A0, U+202F) | Convert to regular space |
  | Smart quotes (U+201C, U+201D) | Convert to straight double quotes (`"`) |
  | Smart single quotes (U+2018, U+2019) | Convert to straight single quotes (`'`) |
  | Em-dash (U+2014) | Convert to double hyphen (`--`) |
  | En-dash (U+2013) | Convert to hyphen (`-`) |
- [ ] **Line ending handling**:
  - [ ] Convert Windows CRLF (`\r\n`) to CR (`\r`) for terminal
  - [ ] Filter duplicate `\n` if preceded by `\r` (collapse CRLF to CR)
  - [ ] Strip ESC characters when bracketed paste mode enabled
- [ ] **Bracketed paste** (XTERM DECSET 2004):
  - [ ] Check TermMode::BRACKETED_PASTE flag on active tab
  - [ ] When enabled: wrap paste in `\x1b[200~` ... `\x1b[201~`
  - [ ] Allows applications to differentiate pasted text from typed text
  - [ ] Strip ESC (`\x1b`) characters from pasted content within brackets
- [ ] **Multi-line paste warning** (configurable):
  - [ ] Detect newlines in pasted content
  - [ ] Optionally warn user before sending multi-line paste to shell
  - [ ] Configurable: always warn, never warn, warn if > N lines
- [ ] **File drag-and-drop paste**:
  - [ ] Handle `WindowEvent::DroppedFile` events
  - [ ] Extract file path(s)
  - [ ] Auto-quote paths containing spaces: `"C:\path with spaces\file.txt"`
  - [ ] Write path(s) to PTY as if typed
  - [ ] Multiple files: space-separated
- [ ] **Tests** (`oriterm/src/clipboard.rs` `#[cfg(test)]`):
  - [ ] FilterOnPaste strips tabs
  - [ ] FilterOnPaste converts smart quotes to straight quotes
  - [ ] FilterOnPaste converts em-dash to double hyphen
  - [ ] CRLF converted to CR
  - [ ] Bracketed paste wraps content in ESC[200~ / ESC[201~
  - [ ] ESC chars stripped within bracketed paste
  - [ ] File path with spaces gets quoted

---

## 9.7 Selection Rendering

Visual highlighting of selected text during GPU rendering.

**File:** `oriterm/src/gpu/render_grid.rs` (selection overlay during cell rendering)

**Reference:** `_old/src/gpu/render_grid.rs` (selection check in cell loop)

- [ ] **Selection colors**: configurable selection foreground and background
  - [ ] Default: inverted colors (swap fg/bg of selected cells)
  - [ ] Alternative: user-configured selection_fg / selection_bg from palette
  - [ ] Colors stored in palette semantic slots (see Section 01, 1.3: CellExtra)
- [ ] **Render approach** (during cell rendering loop): <!-- unblocks:5.13 --><!-- unblocks:6.5 --><!-- unblocks:6.16 -->
  - [ ] For each visible cell: check `selection.contains(stable_row, col)`
  - [ ] If selected: override fg/bg with selection colors
  - [ ] Convert viewport row to StableRowIndex for comparison
  - [ ] Selection check must be efficient (called per-cell per-frame)
- [ ] **Double-wide character handling**:
  - [ ] If WIDE_CHAR cell is selected: highlight both the wide char cell and its spacer
  - [ ] If only the spacer col is in selection bounds: still highlight both cells
  - [ ] Never render half of a double-wide character as selected
- [ ] **Selection across wrapped lines**:
  - [ ] Highlight continues seamlessly across wrap boundaries
  - [ ] No gap between wrapped rows in the selection highlight
- [ ] **Block selection rendering**:
  - [ ] Only highlight cells within rectangular bounds (min_col..max_col, min_row..max_row)
  - [ ] Rows between start and end use same column bounds
- [ ] **Include selection range in RenderableContent**:
  - [ ] Pass current selection (if any) to the render function
  - [ ] Borrow selection immutably during frame building
- [ ] **Selection damage tracking** (incremental redraw on selection change):
  - [ ] Mark only affected lines dirty when selection is created, extended, or cleared
  - [ ] See Section 23.1 for full design  <!-- blocked-by:23 -->
- [ ] **Tests** (visual/integration):
  - [ ] Selection highlight inverts colors for selected cells
  - [ ] Wide character selected as complete unit
  - [ ] Block selection renders rectangular highlight
  - [ ] Selection across wrapped lines has no visual gap

---

## 9.8 Section Completion

- [ ] All 9.1-9.7 items complete
- [ ] `cargo test -p oriterm_core --target x86_64-pc-windows-gnu` — selection model tests pass
- [ ] `cargo test -p oriterm --target x86_64-pc-windows-gnu` — clipboard + mouse selection tests pass
- [ ] `cargo clippy --workspace --target x86_64-pc-windows-gnu` — no warnings
- [ ] Single click + drag selects text character-by-character
- [ ] Drag threshold prevents accidental selection on slight mouse movement
- [ ] Double-click selects words (configurable delimiters)
- [ ] Triple-click selects full logical lines (follows wraps)
- [ ] Alt+drag does block/rectangular selection
- [ ] Shift+click extends existing selection
- [ ] Keyboard selection with Shift+arrows, Ctrl+Shift+arrows
- [ ] Ctrl+A selects all
- [ ] Ctrl+Shift+C copies selection
- [ ] Ctrl+C smart behavior (copy if selection, SIGINT if not)
- [ ] CopyOnSelect option (auto-copy on mouse release)
- [ ] Ctrl+Shift+V pastes from clipboard
- [ ] Bracketed paste mode wraps pasted text in ESC[200~ / ESC[201~
- [ ] FilterOnPaste strips/converts special characters
- [ ] File drag-and-drop auto-quotes paths with spaces
- [ ] Selection visually highlighted with configurable colors
- [ ] Wide characters selected as complete units
- [ ] Soft-wrapped lines joined correctly in copied text
- [ ] Selection across scrollback works (StableRowIndex survives eviction)
- [ ] OSC 52 clipboard integration works

**Exit Criteria:** Selection and clipboard works identically to Windows Terminal. Users coming from Windows Terminal should feel completely at home with the selection, copy, and paste behavior.
