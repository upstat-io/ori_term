//! Unit tests for the extract phase.

use oriterm_core::{
    CellFlags, Column, CursorShape, FairMutex, Rgb, Term, TermMode, Theme, VoidListener,
};
use vte::ansi::Processor;

use super::{extract_frame, extract_frame_into};
use crate::font::CellMetrics;
use crate::gpu::frame_input::ViewportSize;

fn make_terminal(rows: usize, cols: usize) -> FairMutex<Term<VoidListener>> {
    FairMutex::new(Term::new(rows, cols, 100, Theme::Dark, VoidListener))
}

fn make_terminal_with_theme(
    rows: usize,
    cols: usize,
    theme: Theme,
) -> FairMutex<Term<VoidListener>> {
    FairMutex::new(Term::new(rows, cols, 100, theme, VoidListener))
}

/// Feed raw bytes through the VTE processor into a locked terminal.
fn feed(term: &mut Term<VoidListener>, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    processor.advance(term, bytes);
}

const CELL: CellMetrics = CellMetrics {
    width: 8.0,
    height: 16.0,
    baseline: 12.0,
    underline_offset: 2.0,
    stroke_size: 1.0,
    strikeout_offset: 4.0,
};

// =======================================================================
// extract_frame — basic passthrough
// =======================================================================

#[test]
fn extract_returns_correct_viewport_and_cell_size() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.viewport, viewport);
    assert_eq!(frame.cell_size, CELL);
}

#[test]
fn extract_captures_all_visible_cells() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    // 24 rows × 80 cols = 1920 cells.
    assert_eq!(frame.content.cells.len(), 24 * 80);
}

#[test]
fn extract_captures_cursor_state() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Default terminal: cursor at (0, 0), visible (SHOW_CURSOR set by default).
    assert!(frame.content.cursor.visible);
    assert_eq!(frame.content.cursor.line, 0);
    assert_eq!(frame.content.cursor.column, Column(0));
}

#[test]
fn extract_captures_palette_colors() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Dark theme has non-black foreground and cursor.
    assert_ne!(frame.palette.foreground, Rgb { r: 0, g: 0, b: 0 });
    assert_ne!(frame.palette.cursor_color, Rgb { r: 0, g: 0, b: 0 });
}

#[test]
fn extract_selection_is_none() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(frame.selection.is_none());
}

#[test]
fn extract_search_matches_are_empty() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(frame.search_matches.is_empty());
}

#[test]
fn extract_does_not_hold_lock_after_return() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let _frame = extract_frame(&terminal, viewport, CELL);

    // If the lock were still held, this would deadlock.
    let _guard = terminal.lock();
}

#[test]
fn extract_captures_damage_info() {
    let terminal = make_terminal(24, 80);

    // Mark all dirty so the snapshot sees it.
    terminal.lock().grid_mut().dirty_mut().mark_all();

    let viewport = ViewportSize::new(640, 384);
    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(frame.content.all_dirty);
}

#[test]
fn extract_fresh_terminal_not_all_dirty() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Fresh terminal starts clean — no lines marked dirty.
    assert!(!frame.content.all_dirty);
}

#[test]
fn extract_captures_terminal_mode() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Default mode includes SHOW_CURSOR and LINE_WRAP.
    assert!(frame.content.mode.contains(TermMode::SHOW_CURSOR));
}

// =======================================================================
// extract_frame_into — basic passthrough
// =======================================================================

#[test]
fn extract_into_reuses_allocation() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    // First extraction allocates.
    let mut frame = extract_frame(&terminal, viewport, CELL);
    let first_capacity = frame.content.cells.capacity();

    // Second extraction reuses the buffer.
    extract_frame_into(&terminal, &mut frame, viewport, CELL);

    // Capacity should not have decreased (Vec reuse).
    assert!(frame.content.cells.capacity() >= first_capacity);
    assert_eq!(frame.content.cells.len(), 24 * 80);
}

#[test]
fn extract_into_updates_viewport() {
    let terminal = make_terminal(24, 80);
    let original = ViewportSize::new(640, 384);
    let updated = ViewportSize::new(1024, 768);

    let mut frame = extract_frame(&terminal, original, CELL);
    assert_eq!(frame.viewport, original);

    extract_frame_into(&terminal, &mut frame, updated, CELL);
    assert_eq!(frame.viewport, updated);
}

#[test]
fn extract_into_clears_search_matches() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let mut frame = extract_frame(&terminal, viewport, CELL);
    // Simulate leftover search matches from a previous frame.
    frame.search_matches.push(());

    extract_frame_into(&terminal, &mut frame, viewport, CELL);
    assert!(frame.search_matches.is_empty());
}

#[test]
fn extract_into_does_not_hold_lock() {
    let terminal = make_terminal(24, 80);
    let viewport = ViewportSize::new(640, 384);

    let mut frame = extract_frame(&terminal, viewport, CELL);
    extract_frame_into(&terminal, &mut frame, viewport, CELL);

    // If the lock were still held, this would deadlock.
    let _guard = terminal.lock();
}

// =======================================================================
// Cell content verification
// =======================================================================

#[test]
fn extract_captures_written_characters() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"Hello");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cells[0].ch, 'H');
    assert_eq!(frame.content.cells[1].ch, 'e');
    assert_eq!(frame.content.cells[2].ch, 'l');
    assert_eq!(frame.content.cells[3].ch, 'l');
    assert_eq!(frame.content.cells[4].ch, 'o');
    // Remaining cells are spaces.
    for i in 5..10 {
        assert_eq!(frame.content.cells[i].ch, ' ');
    }
}

#[test]
fn extract_captures_multiline_content() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"AB\r\nCD");

    let frame = extract_frame(&terminal, viewport, CELL);

    // Row 0: A, B, spaces...
    assert_eq!(frame.content.cells[0].ch, 'A');
    assert_eq!(frame.content.cells[0].line, 0);
    assert_eq!(frame.content.cells[1].ch, 'B');

    // Row 1: C, D, spaces...
    assert_eq!(frame.content.cells[10].ch, 'C');
    assert_eq!(frame.content.cells[10].line, 1);
    assert_eq!(frame.content.cells[11].ch, 'D');
}

#[test]
fn extract_captures_cell_colors() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // SGR 31 = red fg, SGR 42 = green bg.
    feed(&mut terminal.lock(), b"\x1b[31;42mX");

    let frame = extract_frame(&terminal, viewport, CELL);
    let palette = oriterm_core::Palette::for_theme(Theme::Dark);

    let cell = &frame.content.cells[0];
    assert_eq!(cell.ch, 'X');
    assert_eq!(
        cell.fg,
        palette.resolve(vte::ansi::Color::Named(vte::ansi::NamedColor::Red)),
    );
    assert_eq!(
        cell.bg,
        palette.resolve(vte::ansi::Color::Named(vte::ansi::NamedColor::Green)),
    );
}

#[test]
fn extract_captures_cell_flags() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // SGR 1 = bold, SGR 3 = italic.
    feed(&mut terminal.lock(), b"\x1b[1;3mB");

    let frame = extract_frame(&terminal, viewport, CELL);
    let cell = &frame.content.cells[0];

    assert!(cell.flags.contains(CellFlags::BOLD));
    assert!(cell.flags.contains(CellFlags::ITALIC));
    assert_eq!(cell.ch, 'B');
}

#[test]
fn extract_captures_wide_characters() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // U+4E16 = '世' (CJK, width 2).
    feed(&mut terminal.lock(), "世A".as_bytes());

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cells[0].ch, '世');
    assert!(frame.content.cells[0].flags.contains(CellFlags::WIDE_CHAR));

    assert!(
        frame.content.cells[1]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );

    assert_eq!(frame.content.cells[2].ch, 'A');
    assert!(!frame.content.cells[2].flags.contains(CellFlags::WIDE_CHAR));
}

#[test]
fn extract_captures_truecolor() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // SGR 38;2;r;g;b = truecolor fg.
    feed(&mut terminal.lock(), b"\x1b[38;2;100;200;50mT");

    let frame = extract_frame(&terminal, viewport, CELL);
    let cell = &frame.content.cells[0];

    assert_eq!(
        cell.fg,
        Rgb {
            r: 100,
            g: 200,
            b: 50,
        },
    );
}

#[test]
fn extract_captures_inverse_video() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);
    let palette = oriterm_core::Palette::for_theme(Theme::Dark);

    // SGR 7 = inverse.
    feed(&mut terminal.lock(), b"\x1b[7mI");

    let frame = extract_frame(&terminal, viewport, CELL);
    let cell = &frame.content.cells[0];

    // Inverse swaps fg/bg: fg shows old bg, bg shows old fg.
    assert_eq!(cell.fg, palette.background());
    assert_eq!(cell.bg, palette.foreground());
}

#[test]
fn extract_preserves_empty_cells_defaults() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);
    let palette = oriterm_core::Palette::for_theme(Theme::Dark);

    let frame = extract_frame(&terminal, viewport, CELL);

    for cell in &frame.content.cells {
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, palette.foreground());
        assert_eq!(cell.bg, palette.background());
        assert!(cell.flags.is_empty());
        assert!(cell.zerowidth.is_empty());
        assert_eq!(cell.underline_color, None);
    }
}

// =======================================================================
// Granular damage tracking
// =======================================================================

/// Helper: clear all dirty marks so subsequent mutations produce precise damage.
fn clear_dirty(terminal: &FairMutex<Term<VoidListener>>) {
    terminal
        .lock()
        .grid_mut()
        .dirty_mut()
        .drain()
        .for_each(drop);
}

#[test]
fn extract_single_line_write_marks_only_that_line() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    // Write on line 0 only.
    feed(&mut terminal.lock(), b"X");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(!frame.content.all_dirty);
    assert!(
        frame.content.damage.iter().any(|d| d.line == 0),
        "line 0 should be dirty after write",
    );
    assert!(
        !frame.content.damage.iter().any(|d| d.line == 1),
        "line 1 should be clean",
    );
    assert!(
        !frame.content.damage.iter().any(|d| d.line == 2),
        "line 2 should be clean",
    );
    assert!(
        !frame.content.damage.iter().any(|d| d.line == 3),
        "line 3 should be clean",
    );
}

#[test]
fn extract_writes_on_two_lines_mark_both() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    // Write on line 0 and line 2.
    feed(&mut terminal.lock(), b"A\x1b[3;1HB");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(!frame.content.all_dirty);
    assert!(frame.content.damage.iter().any(|d| d.line == 0));
    assert!(frame.content.damage.iter().any(|d| d.line == 2));
    assert!(!frame.content.damage.iter().any(|d| d.line == 3));
}

#[test]
fn extract_cursor_movement_marks_old_and_new_lines() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    // Move cursor from line 0 to line 2 via CUP.
    feed(&mut terminal.lock(), b"\x1b[3;1H");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(!frame.content.all_dirty);
    // Both old line (0) and new line (2) should be dirty.
    assert!(
        frame.content.damage.iter().any(|d| d.line == 0),
        "old cursor line should be dirty",
    );
    assert!(
        frame.content.damage.iter().any(|d| d.line == 2),
        "new cursor line should be dirty",
    );
}

#[test]
fn extract_multiple_writes_same_line_single_damage_entry() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"ABCDEF");

    let frame = extract_frame(&terminal, viewport, CELL);

    let line0_count = frame.content.damage.iter().filter(|d| d.line == 0).count();
    assert_eq!(line0_count, 1, "line 0 should appear exactly once");
}

#[test]
fn extract_damage_spans_full_line_width() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"X");

    let frame = extract_frame(&terminal, viewport, CELL);

    let line0 = frame
        .content
        .damage
        .iter()
        .find(|d| d.line == 0)
        .expect("line 0 should be dirty");
    assert_eq!(line0.left, Column(0));
    assert_eq!(line0.right, Column(9));
}

#[test]
fn extract_mark_range_produces_partial_damage() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    // Mark lines 1..3 dirty.
    terminal.lock().grid_mut().dirty_mut().mark_range(1..3);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(!frame.content.all_dirty);
    assert!(!frame.content.damage.iter().any(|d| d.line == 0));
    assert!(frame.content.damage.iter().any(|d| d.line == 1));
    assert!(frame.content.damage.iter().any(|d| d.line == 2));
    assert!(!frame.content.damage.iter().any(|d| d.line == 3));
}

#[test]
fn extract_mark_all_sets_all_dirty_flag() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    terminal.lock().grid_mut().dirty_mut().mark_all();

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(frame.content.all_dirty);
    // When all_dirty is true, per-line damage list is empty.
    assert!(frame.content.damage.is_empty());
}

#[test]
fn extract_scroll_triggers_all_dirty() {
    let terminal = make_terminal(4, 10);
    clear_dirty(&terminal);
    let viewport = ViewportSize::new(80, 64);

    // Scroll enough to trigger mark_all (past bottom of screen).
    feed(&mut terminal.lock(), b"\r\n\r\n\r\n\r\n\r\n");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(frame.content.all_dirty);
}

// =======================================================================
// Cursor shape and variant tests
// =======================================================================

#[test]
fn extract_cursor_shape_default_is_block() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cursor.shape, CursorShape::Block);
}

#[test]
fn extract_cursor_shape_bar() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // DECSCUSR 6 = steady bar.
    feed(&mut terminal.lock(), b"\x1b[6 q");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cursor.shape, CursorShape::Bar);
    assert!(frame.content.cursor.visible);
}

#[test]
fn extract_cursor_shape_underline() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // DECSCUSR 4 = steady underline.
    feed(&mut terminal.lock(), b"\x1b[4 q");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cursor.shape, CursorShape::Underline);
}

#[test]
fn extract_cursor_hidden_via_dectcem() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    // DECRST 25 — hide cursor.
    feed(&mut terminal.lock(), b"\x1b[?25l");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert!(!frame.content.cursor.visible);
}

#[test]
fn extract_cursor_position_after_writes() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"ABC\r\nDE");

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cursor.line, 1);
    assert_eq!(frame.content.cursor.column, Column(2));
}

#[test]
fn extract_cursor_shape_reset_to_default() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    feed(&mut terminal.lock(), b"\x1b[6 q"); // Set bar.
    feed(&mut terminal.lock(), b"\x1b[0 q"); // Reset to default.

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cursor.shape, CursorShape::Block);
}

// =======================================================================
// Palette edge cases
// =======================================================================

#[test]
fn extract_dark_theme_palette() {
    let terminal = make_terminal_with_theme(4, 10, Theme::Dark);
    let viewport = ViewportSize::new(80, 64);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Dark theme: black background, light foreground, white cursor.
    assert_eq!(frame.palette.background, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        frame.palette.foreground,
        Rgb {
            r: 0xd3,
            g: 0xd7,
            b: 0xcf,
        },
    );
    assert_eq!(
        frame.palette.cursor_color,
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        },
    );
}

#[test]
fn extract_light_theme_palette() {
    let terminal = make_terminal_with_theme(4, 10, Theme::Light);
    let viewport = ViewportSize::new(80, 64);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Light theme: white background, dark foreground, black cursor.
    assert_eq!(
        frame.palette.background,
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        },
    );
    assert_eq!(
        frame.palette.foreground,
        Rgb {
            r: 0x2e,
            g: 0x34,
            b: 0x36,
        },
    );
    assert_eq!(frame.palette.cursor_color, Rgb { r: 0, g: 0, b: 0 });
}

#[test]
fn extract_palette_matches_terminal_state() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Verify palette matches what the terminal reports.
    let term = terminal.lock();
    let pal = term.palette();
    assert_eq!(frame.palette.background, pal.background());
    assert_eq!(frame.palette.foreground, pal.foreground());
    assert_eq!(frame.palette.cursor_color, pal.cursor_color());
}

#[test]
fn extract_theme_switch_updates_palette() {
    let terminal = make_terminal_with_theme(4, 10, Theme::Dark);
    let viewport = ViewportSize::new(80, 64);

    let dark_frame = extract_frame(&terminal, viewport, CELL);

    // Switch to light theme.
    terminal.lock().set_theme(Theme::Light);

    let light_frame = extract_frame(&terminal, viewport, CELL);

    // Background should differ between themes.
    assert_ne!(
        dark_frame.palette.background,
        light_frame.palette.background
    );
    assert_ne!(
        dark_frame.palette.cursor_color,
        light_frame.palette.cursor_color,
    );
}

// =======================================================================
// Scrollback history exclusion
// =======================================================================

#[test]
fn extract_returns_only_visible_rows() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    {
        let mut t = terminal.lock();
        // Fill 5 lines (scrollback holds line "A", visible shows B/C/D/E).
        feed(&mut t, b"AAAAAAAAAA\r\n");
        feed(&mut t, b"BBBBBBBBBB\r\n");
        feed(&mut t, b"CCCCCCCCCC\r\n");
        feed(&mut t, b"DDDDDDDDDD\r\n");
        feed(&mut t, b"EEEEEEEEEE");
    }

    let frame = extract_frame(&terminal, viewport, CELL);

    // Still 4 rows × 10 cols = 40 cells.
    assert_eq!(frame.content.cells.len(), 4 * 10);
    // Scrollback 'A' should NOT be visible.
    assert_eq!(frame.content.cells[0].ch, 'B');
    assert_eq!(frame.content.display_offset, 0);
}

#[test]
fn extract_scrolled_back_shows_history() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    {
        let mut t = terminal.lock();
        feed(&mut t, b"AAAAAAAAAA\r\n");
        feed(&mut t, b"BBBBBBBBBB\r\n");
        feed(&mut t, b"CCCCCCCCCC\r\n");
        feed(&mut t, b"DDDDDDDDDD\r\n");
        feed(&mut t, b"EEEEEEEEEE");
    }

    // Scroll back 1 line to reveal 'A' row.
    terminal.lock().grid_mut().scroll_display(1);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cells.len(), 4 * 10);
    assert_eq!(frame.content.cells[0].ch, 'A');
    assert_eq!(frame.content.display_offset, 1);
}

#[test]
fn extract_cursor_hidden_when_scrolled_back() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    {
        let mut t = terminal.lock();
        feed(&mut t, b"AAAAAAAAAA\r\n");
        feed(&mut t, b"BBBBBBBBBB\r\n");
        feed(&mut t, b"CCCCCCCCCC\r\n");
        feed(&mut t, b"DDDDDDDDDD\r\n");
        feed(&mut t, b"EEEEEEEEEE");
    }

    terminal.lock().grid_mut().scroll_display(1);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Cursor should not be visible when scrolled back.
    assert!(!frame.content.cursor.visible);
}

#[test]
fn extract_scrollback_preserves_colors() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);
    let palette = oriterm_core::Palette::for_theme(Theme::Dark);

    {
        let mut t = terminal.lock();
        // Red line that will scroll into scrollback.
        feed(&mut t, b"\x1b[31mRRRRRRRRRR\r\n\x1b[0m");
        feed(&mut t, b"line2\r\nline3\r\nline4\r\nline5");
    }

    terminal.lock().grid_mut().scroll_display(1);

    let frame = extract_frame(&terminal, viewport, CELL);

    let cell = &frame.content.cells[0];
    assert_eq!(cell.ch, 'R');
    assert_eq!(
        cell.fg,
        palette.resolve(vte::ansi::Color::Named(vte::ansi::NamedColor::Red)),
    );
}

#[test]
fn extract_scrollback_preserves_flags() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    {
        let mut t = terminal.lock();
        feed(&mut t, b"\x1b[1mBBBBBBBBBB\r\n\x1b[0m");
        feed(&mut t, b"line2\r\nline3\r\nline4\r\nline5");
    }

    terminal.lock().grid_mut().scroll_display(1);

    let frame = extract_frame(&terminal, viewport, CELL);

    let cell = &frame.content.cells[0];
    assert_eq!(cell.ch, 'B');
    assert!(cell.flags.contains(CellFlags::BOLD));
}

// =======================================================================
// Viewport boundary conditions
// =======================================================================

#[test]
fn extract_with_zero_viewport_clamps_to_one() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(0, 0);

    let frame = extract_frame(&terminal, viewport, CELL);

    // ViewportSize clamps to (1, 1).
    assert_eq!(frame.viewport.width, 1);
    assert_eq!(frame.viewport.height, 1);
    // Cell count depends on grid, not viewport.
    assert_eq!(frame.content.cells.len(), 4 * 10);
}

#[test]
fn extract_with_large_viewport() {
    let terminal = make_terminal(4, 10);
    // Viewport far larger than the grid's cell needs.
    let viewport = ViewportSize::new(10000, 10000);

    let frame = extract_frame(&terminal, viewport, CELL);

    // Still only 4×10 cells from the terminal grid.
    assert_eq!(frame.content.cells.len(), 4 * 10);
    assert_eq!(frame.viewport.width, 10000);
}

#[test]
fn extract_small_terminal() {
    let terminal = make_terminal(1, 1);
    let viewport = ViewportSize::new(8, 16);

    let frame = extract_frame(&terminal, viewport, CELL);

    assert_eq!(frame.content.cells.len(), 1);
    assert_eq!(frame.content.cells[0].ch, ' ');
    assert!(frame.content.cursor.visible);
}

// =======================================================================
// Allocation strategy for extract_frame_into
// =======================================================================

#[test]
fn extract_into_viewport_shrink_preserves_capacity() {
    let terminal = make_terminal(24, 80);
    let large = ViewportSize::new(1920, 1080);
    let small = ViewportSize::new(640, 384);

    let mut frame = extract_frame(&terminal, large, CELL);
    let large_capacity = frame.content.cells.capacity();

    extract_frame_into(&terminal, &mut frame, small, CELL);

    // Capacity should not decrease — Vec reuse.
    assert!(frame.content.cells.capacity() >= large_capacity);
    // But length matches the grid (unchanged terminal size).
    assert_eq!(frame.content.cells.len(), 24 * 80);
}

#[test]
fn extract_into_viewport_grow_extends_if_needed() {
    let terminal = make_terminal(24, 80);
    let small = ViewportSize::new(640, 384);
    let large = ViewportSize::new(1920, 1080);

    let mut frame = extract_frame(&terminal, small, CELL);
    let small_capacity = frame.content.cells.capacity();

    extract_frame_into(&terminal, &mut frame, large, CELL);

    // Length stays at 24×80 (grid size didn't change).
    assert_eq!(frame.content.cells.len(), 24 * 80);
    // Capacity is at least what it was before.
    assert!(frame.content.cells.capacity() >= small_capacity);
}

#[test]
fn extract_into_repeated_extractions_stable() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let mut frame = extract_frame(&terminal, viewport, CELL);

    // Run 10 extractions — capacity should stabilize.
    for _ in 0..10 {
        extract_frame_into(&terminal, &mut frame, viewport, CELL);
    }

    assert_eq!(frame.content.cells.len(), 4 * 10);
    // No exponential growth — capacity should be reasonable.
    assert!(
        frame.content.cells.capacity() < 4 * 10 * 4,
        "capacity should not grow excessively: got {}",
        frame.content.cells.capacity(),
    );
}

#[test]
fn extract_into_updates_palette() {
    let terminal = make_terminal_with_theme(4, 10, Theme::Dark);
    let viewport = ViewportSize::new(80, 64);

    let mut frame = extract_frame(&terminal, viewport, CELL);
    let dark_bg = frame.palette.background;

    terminal.lock().set_theme(Theme::Light);

    extract_frame_into(&terminal, &mut frame, viewport, CELL);

    assert_ne!(frame.palette.background, dark_bg);
    assert_eq!(
        frame.palette.background,
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        },
    );
}

#[test]
fn extract_into_updates_cell_content() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let mut frame = extract_frame(&terminal, viewport, CELL);
    assert_eq!(frame.content.cells[0].ch, ' ');

    feed(&mut terminal.lock(), b"Z");

    extract_frame_into(&terminal, &mut frame, viewport, CELL);
    assert_eq!(frame.content.cells[0].ch, 'Z');
}

#[test]
fn extract_into_clears_selection() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let mut frame = extract_frame(&terminal, viewport, CELL);
    frame.selection = Some(());

    extract_frame_into(&terminal, &mut frame, viewport, CELL);
    assert!(frame.selection.is_none());
}

#[test]
fn extract_into_damage_vec_reuses_capacity() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let mut frame = extract_frame(&terminal, viewport, CELL);

    // Force some damage.
    terminal.lock().grid_mut().dirty_mut().mark(0);
    extract_frame_into(&terminal, &mut frame, viewport, CELL);
    let cap = frame.content.damage.capacity();

    // Second extraction should reuse the Vec.
    terminal.lock().grid_mut().dirty_mut().mark(1);
    extract_frame_into(&terminal, &mut frame, viewport, CELL);

    // Capacity doesn't shrink.
    assert!(frame.content.damage.capacity() >= cap);
}

// =======================================================================
// Concurrent lock access (verifies FairMutex discipline)
// =======================================================================

#[test]
fn extract_frame_then_mutate_then_extract_again() {
    let terminal = make_terminal(4, 10);
    let viewport = ViewportSize::new(80, 64);

    let frame1 = extract_frame(&terminal, viewport, CELL);
    assert_eq!(frame1.content.cells[0].ch, ' ');

    // Mutate terminal between extractions.
    feed(&mut terminal.lock(), b"Q");

    let frame2 = extract_frame(&terminal, viewport, CELL);
    assert_eq!(frame2.content.cells[0].ch, 'Q');

    // frame1 is still valid (owned data, no references back).
    assert_eq!(frame1.content.cells[0].ch, ' ');
}
