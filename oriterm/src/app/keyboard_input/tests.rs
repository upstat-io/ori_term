//! Unit tests for IME handling and preedit overlay.

use winit::event::Ime;

use oriterm_core::{
    CellFlags, Column, CursorShape, RenderableCell, RenderableContent, RenderableCursor, Rgb,
    TermMode,
};

use super::super::redraw::overlay_preedit_cells;
use super::{ImeEffect, ImeState};

const FG: Rgb = Rgb {
    r: 211,
    g: 215,
    b: 207,
};
const BG: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// Build a renderable content with a grid of spaces and cursor at `(line, col)`.
fn content_with_cursor(
    cols: usize,
    rows: usize,
    cursor_line: usize,
    cursor_col: usize,
) -> RenderableContent {
    let mut cells = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            cells.push(RenderableCell {
                line: row,
                column: Column(col),
                ch: ' ',
                fg: FG,
                bg: BG,
                flags: CellFlags::empty(),
                underline_color: None,
                has_hyperlink: false,
                zerowidth: Vec::new(),
            });
        }
    }
    RenderableContent {
        cells,
        cursor: RenderableCursor {
            line: cursor_line,
            column: Column(cursor_col),
            shape: CursorShape::Block,
            visible: true,
        },
        display_offset: 0,
        stable_row_base: 0,
        mode: TermMode::SHOW_CURSOR,
        all_dirty: true,
        damage: Vec::new(),
    }
}

// --- overlay_preedit_cells ---

#[test]
fn preedit_replaces_cell_at_cursor() {
    let mut content = content_with_cursor(10, 1, 0, 3);
    overlay_preedit_cells("A", &mut content, 10);

    assert_eq!(content.cells[3].ch, 'A');
    assert!(content.cells[3].flags.contains(CellFlags::UNDERLINE));
}

#[test]
fn preedit_hides_cursor() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    assert!(content.cursor.visible);

    overlay_preedit_cells("x", &mut content, 10);
    assert!(!content.cursor.visible);
}

#[test]
fn preedit_wide_char_sets_flags() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    // U+4E2D '中' is a CJK character (display width 2).
    overlay_preedit_cells("中", &mut content, 10);

    assert_eq!(content.cells[0].ch, '中');
    assert!(content.cells[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[0].flags.contains(CellFlags::UNDERLINE));
    assert!(content.cells[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
}

#[test]
fn preedit_multiple_chars() {
    let mut content = content_with_cursor(10, 1, 0, 2);
    overlay_preedit_cells("AB", &mut content, 10);

    assert_eq!(content.cells[2].ch, 'A');
    assert_eq!(content.cells[3].ch, 'B');
    assert!(content.cells[2].flags.contains(CellFlags::UNDERLINE));
    assert!(content.cells[3].flags.contains(CellFlags::UNDERLINE));
    // Other cells unchanged.
    assert_eq!(content.cells[0].ch, ' ');
    assert_eq!(content.cells[4].ch, ' ');
}

#[test]
fn preedit_clips_at_grid_edge() {
    let mut content = content_with_cursor(4, 1, 0, 3);
    // Cursor at col 3, grid is 4 cols — only 1 cell available.
    overlay_preedit_cells("XY", &mut content, 4);

    assert_eq!(content.cells[3].ch, 'X');
    // 'Y' is clipped (col 4 doesn't exist).
}

#[test]
fn preedit_wide_char_clips_at_edge() {
    let mut content = content_with_cursor(4, 1, 0, 3);
    // Wide char at col 3 needs 2 cells but only 1 available — still placed
    // (the spacer at col 4 is out of bounds, which is handled gracefully).
    overlay_preedit_cells("中", &mut content, 4);

    assert_eq!(content.cells[3].ch, '中');
    assert!(content.cells[3].flags.contains(CellFlags::WIDE_CHAR));
}

#[test]
fn preedit_empty_string_no_change() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    // Empty preedit shouldn't change anything (but cursor is still hidden
    // because overlay_preedit_cells is only called when preedit is non-empty
    // in the actual app code).
    let original_ch = content.cells[0].ch;
    overlay_preedit_cells("", &mut content, 10);

    assert_eq!(content.cells[0].ch, original_ch);
}

#[test]
fn preedit_on_second_row() {
    let mut content = content_with_cursor(10, 3, 1, 5);
    overlay_preedit_cells("Z", &mut content, 10);

    // Row 1, col 5 = index 1*10 + 5 = 15.
    assert_eq!(content.cells[15].ch, 'Z');
    assert!(content.cells[15].flags.contains(CellFlags::UNDERLINE));
}

#[test]
fn preedit_cjk_composition_sequence() {
    let mut content = content_with_cursor(20, 1, 0, 0);
    // Typical CJK composition: two wide characters.
    overlay_preedit_cells("中文", &mut content, 20);

    // First char at cols 0-1.
    assert_eq!(content.cells[0].ch, '中');
    assert!(content.cells[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    // Second char at cols 2-3.
    assert_eq!(content.cells[2].ch, '文');
    assert!(content.cells[2].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[3].flags.contains(CellFlags::WIDE_CHAR_SPACER));
}

// --- ImeState: state machine transitions ---

#[test]
fn ime_enabled_sets_active() {
    let mut ime = ImeState::new();
    assert!(!ime.active);

    let effect = ime.handle_event(Ime::Enabled);
    assert!(ime.active);
    assert_eq!(effect, ImeEffect::Redraw);
}

#[test]
fn ime_preedit_stores_text_and_cursor() {
    let mut ime = ImeState::new();
    ime.active = true;

    let effect = ime.handle_event(Ime::Preedit("abc".into(), Some((1, 2))));
    assert_eq!(ime.preedit, "abc");
    assert_eq!(ime.preedit_cursor, Some(1));
    assert_eq!(effect, ImeEffect::UpdateCursorArea);
}

#[test]
fn ime_commit_clears_state_and_returns_text() {
    let mut ime = ImeState::new();
    ime.active = true;
    ime.preedit = "composing".into();
    ime.preedit_cursor = Some(3);

    let effect = ime.handle_event(Ime::Commit("final".into()));
    assert!(!ime.active);
    assert!(ime.preedit.is_empty());
    assert!(ime.preedit_cursor.is_none());
    assert_eq!(effect, ImeEffect::Commit("final".into()));
}

#[test]
fn ime_disabled_clears_all_state() {
    let mut ime = ImeState::new();
    ime.active = true;
    ime.preedit = "partial".into();
    ime.preedit_cursor = Some(2);

    let effect = ime.handle_event(Ime::Disabled);
    assert!(!ime.active);
    assert!(ime.preedit.is_empty());
    assert!(ime.preedit_cursor.is_none());
    assert_eq!(effect, ImeEffect::Redraw);
}

#[test]
fn ime_full_lifecycle() {
    let mut ime = ImeState::new();

    // Enabled.
    ime.handle_event(Ime::Enabled);
    assert!(ime.active);

    // Preedit starts.
    ime.handle_event(Ime::Preedit("k".into(), Some((1, 1))));
    assert_eq!(ime.preedit, "k");
    assert!(ime.should_suppress_key());

    // Preedit updates.
    ime.handle_event(Ime::Preedit("ka".into(), Some((2, 2))));
    assert_eq!(ime.preedit, "ka");

    // Commit.
    let effect = ime.handle_event(Ime::Commit("か".into()));
    assert_eq!(effect, ImeEffect::Commit("か".into()));
    assert!(!ime.active);
    assert!(!ime.should_suppress_key());
}

#[test]
fn ime_enabled_then_disabled_without_preedit() {
    let mut ime = ImeState::new();

    ime.handle_event(Ime::Enabled);
    assert!(ime.active);
    // No preedit — should NOT suppress keys (active but preedit empty).
    assert!(!ime.should_suppress_key());

    ime.handle_event(Ime::Disabled);
    assert!(!ime.active);
}

// --- ImeState: key suppression ---

#[test]
fn suppress_key_when_active_with_preedit() {
    let mut ime = ImeState::new();
    ime.active = true;
    ime.preedit = "中".into();
    assert!(ime.should_suppress_key());
}

#[test]
fn no_suppress_when_inactive() {
    let ime = ImeState::new();
    assert!(!ime.should_suppress_key());
}

#[test]
fn no_suppress_when_active_but_preedit_empty() {
    let mut ime = ImeState::new();
    ime.active = true;
    // Active with empty preedit (between commit and disable).
    assert!(!ime.should_suppress_key());
}

#[test]
fn no_suppress_after_commit() {
    let mut ime = ImeState::new();
    ime.active = true;
    ime.preedit = "test".into();
    assert!(ime.should_suppress_key());

    ime.handle_event(Ime::Commit("test".into()));
    // After commit: active=false, preedit=empty.
    assert!(!ime.should_suppress_key());
}

// --- overlay_preedit_cells: combining marks ---

#[test]
fn preedit_combining_mark_does_not_occupy_cell() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    // 'a' followed by U+0301 (combining acute accent) — only 'a' has width.
    overlay_preedit_cells("a\u{0301}", &mut content, 10);

    // The 'a' occupies col 0.
    assert_eq!(content.cells[0].ch, 'a');
    assert!(content.cells[0].flags.contains(CellFlags::UNDERLINE));
    // Col 1 is unchanged (combining mark has width 0, skipped).
    assert_eq!(content.cells[1].ch, ' ');
}

// --- overlay_preedit_cells: cursor offset with wide chars ---

#[test]
fn preedit_wide_chars_advance_column_by_two() {
    let mut content = content_with_cursor(20, 1, 0, 0);
    // Mix of narrow and wide: 'A' (w=1) + '中' (w=2) + 'B' (w=1).
    overlay_preedit_cells("A中B", &mut content, 20);

    // 'A' at col 0.
    assert_eq!(content.cells[0].ch, 'A');
    // '中' at col 1 (wide: occupies cols 1-2).
    assert_eq!(content.cells[1].ch, '中');
    assert!(content.cells[1].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[2].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    // 'B' at col 3 (after wide char's 2-cell span).
    assert_eq!(content.cells[3].ch, 'B');
}

// --- overlay_preedit_cells: very long preedit ---

#[test]
fn preedit_long_string_truncates_at_grid_width() {
    let mut content = content_with_cursor(5, 1, 0, 0);
    let long_preedit = "ABCDEFGHIJ"; // 10 chars, grid is 5 wide.
    overlay_preedit_cells(long_preedit, &mut content, 5);

    assert_eq!(content.cells[0].ch, 'A');
    assert_eq!(content.cells[1].ch, 'B');
    assert_eq!(content.cells[2].ch, 'C');
    assert_eq!(content.cells[3].ch, 'D');
    assert_eq!(content.cells[4].ch, 'E');
    // Only 5 chars fit, rest clipped. No panic.
}

#[test]
fn preedit_long_wide_string_truncates_gracefully() {
    let mut content = content_with_cursor(6, 1, 0, 0);
    // 4 CJK chars = 8 display width, grid is 6 wide.
    overlay_preedit_cells("中文日本", &mut content, 6);

    // '中' at cols 0-1.
    assert_eq!(content.cells[0].ch, '中');
    // '文' at cols 2-3.
    assert_eq!(content.cells[2].ch, '文');
    // '日' at cols 4-5.
    assert_eq!(content.cells[4].ch, '日');
    // '本' would need cols 6-7 but col 6 >= grid width → clipped.
}

// --- overlay_preedit_cells: emoji ---

#[test]
fn preedit_emoji_basic() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    // U+1F600 GRINNING FACE — typically width 2.
    overlay_preedit_cells("😀", &mut content, 10);

    assert_eq!(content.cells[0].ch, '😀');
    // unicode-width reports 2 for most emoji.
    assert!(content.cells[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[0].flags.contains(CellFlags::UNDERLINE));
    assert!(content.cells[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
}

// --- overlay_preedit_cells: rapid updates ---

#[test]
fn preedit_successive_overlays_replace_cleanly() {
    let mut content = content_with_cursor(10, 1, 0, 0);

    // First preedit: "AB" at cols 0-1.
    overlay_preedit_cells("AB", &mut content, 10);
    assert_eq!(content.cells[0].ch, 'A');
    assert_eq!(content.cells[1].ch, 'B');

    // Reset cursor visibility for next overlay (in real code, content is
    // re-extracted each frame, so cursor.visible starts true again).
    content.cursor.visible = true;

    // Second preedit: "X" at col 0. Col 1 retains 'B' from first overlay
    // (in real code, content is fresh each frame — this tests the function
    // itself doesn't leave stale state beyond its written range).
    overlay_preedit_cells("X", &mut content, 10);
    assert_eq!(content.cells[0].ch, 'X');
    assert!(content.cells[0].flags.contains(CellFlags::UNDERLINE));
}

// --- overlay_preedit_cells: empty/zero grid ---

#[test]
fn preedit_zero_cols_no_panic() {
    let mut content = content_with_cursor(10, 1, 0, 0);
    // cols=0 should early-return without modifying anything.
    overlay_preedit_cells("A", &mut content, 0);
    assert_eq!(content.cells[0].ch, ' ');
    assert!(content.cursor.visible);
}

#[test]
fn preedit_empty_cells_no_panic() {
    let mut content = RenderableContent {
        cells: Vec::new(),
        cursor: RenderableCursor {
            line: 0,
            column: Column(0),
            shape: CursorShape::Block,
            visible: true,
        },
        display_offset: 0,
        stable_row_base: 0,
        mode: TermMode::SHOW_CURSOR,
        all_dirty: true,
        damage: Vec::new(),
    };
    // Empty cells vec should early-return without panic.
    overlay_preedit_cells("A", &mut content, 10);
    assert!(content.cells.is_empty());
}
