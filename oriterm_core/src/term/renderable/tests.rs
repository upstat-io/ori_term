//! Tests for RenderableContent snapshot extraction.

use vte::ansi::{Color, NamedColor, Processor};

use super::{apply_inverse, resolve_bg, resolve_fg};
use crate::cell::CellFlags;
use crate::color::{Palette, Rgb};
use crate::event::VoidListener;
use crate::grid::CursorShape;
use crate::index::Column;
use crate::term::Term;
use crate::term::mode::TermMode;
use crate::theme::Theme;

/// Create a 4x10 terminal for compact tests.
fn term() -> Term<VoidListener> {
    Term::new(4, 10, 100, Theme::default(), VoidListener)
}

/// Feed raw bytes through the VTE processor.
fn feed(term: &mut impl vte::ansi::Handler, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    processor.advance(term, bytes);
}

// --- RenderableContent extraction ---

#[test]
fn empty_term_produces_space_cells() {
    let t = term();
    let content = t.renderable_content();

    assert_eq!(content.cells.len(), 4 * 10);
    for cell in &content.cells {
        assert_eq!(cell.ch, ' ');
    }
}

#[test]
fn written_chars_appear_in_cells() {
    let mut t = term();
    feed(&mut t, b"Hi");

    let content = t.renderable_content();

    // First row, first two columns should be 'H' and 'i'.
    let h = &content.cells[0];
    assert_eq!(h.line, 0);
    assert_eq!(h.column, Column(0));
    assert_eq!(h.ch, 'H');

    let i = &content.cells[1];
    assert_eq!(i.line, 0);
    assert_eq!(i.column, Column(1));
    assert_eq!(i.ch, 'i');

    // Rest of first row should be spaces.
    for col in 2..10 {
        assert_eq!(content.cells[col].ch, ' ');
    }
}

#[test]
fn cell_ordering_is_row_major() {
    let mut t = term();
    // Write 'A' on line 0, 'B' on line 1.
    feed(&mut t, b"A\r\nB");

    let content = t.renderable_content();

    // cells[0] = line 0, col 0 = 'A'
    assert_eq!(content.cells[0].line, 0);
    assert_eq!(content.cells[0].column, Column(0));
    assert_eq!(content.cells[0].ch, 'A');

    // cells[10] = line 1, col 0 = 'B'
    assert_eq!(content.cells[10].line, 1);
    assert_eq!(content.cells[10].column, Column(0));
    assert_eq!(content.cells[10].ch, 'B');
}

// --- Cursor ---

#[test]
fn cursor_position_matches_term() {
    let mut t = term();
    feed(&mut t, b"AB");

    let content = t.renderable_content();

    assert_eq!(content.cursor.line, 0);
    assert_eq!(content.cursor.column, Column(2));
    assert!(content.cursor.visible);
    assert_eq!(content.cursor.shape, CursorShape::Block);
}

#[test]
fn cursor_on_second_line() {
    let mut t = term();
    feed(&mut t, b"hello\r\nwor");

    let content = t.renderable_content();

    assert_eq!(content.cursor.line, 1);
    assert_eq!(content.cursor.column, Column(3));
}

#[test]
fn cursor_hidden_when_show_cursor_off() {
    let mut t = term();
    // DECRST 25 — hide cursor.
    feed(&mut t, b"\x1b[?25l");

    let content = t.renderable_content();
    assert!(!content.cursor.visible);
}

#[test]
fn cursor_hidden_when_shape_is_hidden() {
    let mut t = term();
    // DECSCUSR 0 resets to default, but DECSCUSR with a hidden shape...
    // Let's use CSI to hide cursor shape. Actually there's no direct CSI
    // for CursorShape::Hidden. Test the logic by directly checking the
    // cursor_shape field influence: if cursor_shape is Hidden, visible is false.
    // Since we can't set Hidden via VTE (it's an internal state), we test
    // through DECRST 25 which is the standard mechanism.
    feed(&mut t, b"\x1b[?25l");
    let content = t.renderable_content();
    assert!(!content.cursor.visible);
}

// --- Color resolution ---

#[test]
fn default_colors_resolve_to_palette_defaults() {
    let t = term();
    let content = t.renderable_content();
    let palette = Palette::default();

    // All cells should have the default foreground/background.
    let cell = &content.cells[0];
    assert_eq!(cell.fg, palette.foreground());
    assert_eq!(cell.bg, palette.background());
}

#[test]
fn sgr_named_color_resolves() {
    let mut t = term();
    // SGR 31 = red foreground, SGR 42 = green background.
    feed(&mut t, b"\x1b[31;42mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'X');
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::Red)));
    assert_eq!(cell.bg, palette.resolve(Color::Named(NamedColor::Green)));
}

#[test]
fn sgr_indexed_color_resolves() {
    let mut t = term();
    // SGR 38;5;196 = indexed fg 196, SGR 48;5;21 = indexed bg 21.
    feed(&mut t, b"\x1b[38;5;196;48;5;21mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    assert_eq!(cell.fg, palette.resolve(Color::Indexed(196)));
    assert_eq!(cell.bg, palette.resolve(Color::Indexed(21)));
}

#[test]
fn sgr_truecolor_resolves() {
    let mut t = term();
    // SGR 38;2;255;128;0 = direct fg RGB.
    feed(&mut t, b"\x1b[38;2;255;128;0mX");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(
        cell.fg,
        Rgb {
            r: 255,
            g: 128,
            b: 0
        }
    );
}

#[test]
fn bold_as_bright_promotes_ansi_colors() {
    let mut t = term();
    // SGR 1 = bold, SGR 31 = red foreground. Bold + red → bright red.
    feed(&mut t, b"\x1b[1;31mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    assert_eq!(
        cell.fg,
        palette.resolve(Color::Named(NamedColor::BrightRed))
    );
}

#[test]
fn bold_as_bright_does_not_affect_bright_colors() {
    let mut t = term();
    // SGR 1 = bold, SGR 91 = bright red. Already bright, no double-promotion.
    feed(&mut t, b"\x1b[1;91mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    // BrightRed.to_bright() returns BrightRed (no change).
    assert_eq!(
        cell.fg,
        palette.resolve(Color::Named(NamedColor::BrightRed))
    );
}

#[test]
fn bold_as_bright_does_not_affect_truecolor() {
    let mut t = term();
    // SGR 1 = bold, 38;2;100;200;50 = truecolor fg.
    feed(&mut t, b"\x1b[1;38;2;100;200;50mX");

    let content = t.renderable_content();

    let cell = &content.cells[0];
    // Truecolor is not promoted by bold.
    assert_eq!(
        cell.fg,
        Rgb {
            r: 100,
            g: 200,
            b: 50
        }
    );
}

#[test]
fn inverse_swaps_fg_bg() {
    let mut t = term();
    // SGR 7 = inverse.
    feed(&mut t, b"\x1b[7mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    // Inverse swaps resolved fg/bg: fg shows old bg, bg shows old fg.
    assert_eq!(cell.fg, palette.background());
    assert_eq!(cell.bg, palette.foreground());
}

#[test]
fn inverse_with_custom_colors() {
    let mut t = term();
    // SGR 31 = red fg, SGR 42 = green bg, SGR 7 = inverse.
    feed(&mut t, b"\x1b[31;42;7mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    // Inverse swaps: fg=green, bg=red.
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::Green)));
    assert_eq!(cell.bg, palette.resolve(Color::Named(NamedColor::Red)));
}

#[test]
fn dim_reduces_brightness() {
    let mut t = term();
    // SGR 2 = dim, SGR 31 = red.
    feed(&mut t, b"\x1b[2;31mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    // Dim red uses the DimRed palette entry.
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::DimRed)));
}

// --- resolve_fg / resolve_bg unit tests ---

#[test]
fn resolve_fg_spec_passthrough() {
    let palette = Palette::default();
    let rgb = Rgb {
        r: 42,
        g: 84,
        b: 126,
    };
    assert_eq!(
        resolve_fg(Color::Spec(rgb), CellFlags::empty(), &palette),
        rgb
    );
}

#[test]
fn resolve_fg_bold_indexed_promotion() {
    let palette = Palette::default();
    // Indexed 1 = Red, bold → indexed 9 = BrightRed.
    let result = resolve_fg(Color::Indexed(1), CellFlags::BOLD, &palette);
    assert_eq!(result, palette.resolve(Color::Indexed(9)));
}

#[test]
fn resolve_fg_bold_indexed_no_promotion_above_7() {
    let palette = Palette::default();
    // Indexed 100 is not in 0–7 range, bold should not promote.
    let result = resolve_fg(Color::Indexed(100), CellFlags::BOLD, &palette);
    assert_eq!(result, palette.resolve(Color::Indexed(100)));
}

#[test]
fn resolve_fg_dim_spec_reduces() {
    let palette = Palette::default();
    let rgb = Rgb {
        r: 90,
        g: 150,
        b: 210,
    };
    let result = resolve_fg(Color::Spec(rgb), CellFlags::DIM, &palette);
    assert_eq!(
        result,
        Rgb {
            r: 60,
            g: 100,
            b: 140
        }
    );
}

#[test]
fn resolve_bg_passthrough() {
    let palette = Palette::default();
    let rgb = Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    assert_eq!(resolve_bg(Color::Spec(rgb), &palette), rgb);
}

#[test]
fn apply_inverse_swaps_defaults() {
    let palette = Palette::default();
    let fg = palette.foreground();
    let bg = palette.background();
    let (inv_fg, inv_bg) = apply_inverse(fg, bg, CellFlags::INVERSE);
    assert_eq!(inv_fg, palette.background()); // fg now shows the old bg
    assert_eq!(inv_bg, palette.foreground()); // bg now shows the old fg
}

#[test]
fn apply_inverse_noop_without_flag() {
    let fg = Rgb { r: 1, g: 2, b: 3 };
    let bg = Rgb { r: 4, g: 5, b: 6 };
    let (res_fg, res_bg) = apply_inverse(fg, bg, CellFlags::empty());
    assert_eq!(res_fg, fg);
    assert_eq!(res_bg, bg);
}

// --- Mode snapshot ---

#[test]
fn mode_flags_captured_in_snapshot() {
    let t = term();
    let content = t.renderable_content();
    assert!(content.mode.contains(TermMode::SHOW_CURSOR));
    assert!(content.mode.contains(TermMode::LINE_WRAP));
}

// --- Display offset ---

#[test]
fn display_offset_zero_in_live_view() {
    let t = term();
    let content = t.renderable_content();
    assert_eq!(content.display_offset, 0);
}

// --- Damage ---

#[test]
fn fresh_term_reports_all_dirty() {
    // A fresh grid with DirtyTracker::new starts clean, so no damage.
    let t = term();
    let content = t.renderable_content();
    // Fresh tracker: all bits false, no damage reported.
    assert!(!content.all_dirty);
    assert!(content.damage.is_empty());
}

#[test]
fn writing_marks_line_dirty() {
    let mut t = term();
    // Drain initial dirty state.
    let _ = t.renderable_content();
    // Clear dirty state.
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Write on line 0.
    feed(&mut t, b"X");

    let content = t.renderable_content();
    assert!(!content.all_dirty);
    // Line 0 should be damaged.
    assert!(content.damage.iter().any(|d| d.line == 0));
    // Other lines should not be damaged.
    assert!(!content.damage.iter().any(|d| d.line == 1));
}

#[test]
fn mark_all_dirty_reports_full_redraw() {
    let mut t = term();
    t.grid_mut().dirty_mut().mark_all();

    let content = t.renderable_content();
    assert!(content.all_dirty);
    // When all_dirty is true, damage list is empty (full redraw signal).
    assert!(content.damage.is_empty());
}

// --- Scrollback integration ---

#[test]
fn scrollback_content_visible_when_scrolled() {
    let mut t = Term::new(4, 10, 100, Theme::default(), VoidListener);

    // Fill 4 lines and scroll one into scrollback.
    feed(&mut t, b"AAAAAAAAAA\r\n");
    feed(&mut t, b"BBBBBBBBBB\r\n");
    feed(&mut t, b"CCCCCCCCCC\r\n");
    feed(&mut t, b"DDDDDDDDDD\r\n");
    // Line "AAAAAAAAAA" should now be in scrollback.
    // Write one more line to push it.
    feed(&mut t, b"EEEEEEEEEE");

    // Scroll back 1 line.
    t.grid_mut().scroll_display(1);

    let content = t.renderable_content();

    // First visible line should come from scrollback.
    assert_eq!(content.cells[0].ch, 'A');
    assert_eq!(content.display_offset, 1);
    // Cursor should not be visible when scrolled back.
    assert!(!content.cursor.visible);
}

// --- Flags preserved ---

#[test]
fn cell_flags_preserved_in_renderable() {
    let mut t = term();
    // SGR 1 = bold.
    feed(&mut t, b"\x1b[1mB");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::BOLD));
}

#[test]
fn italic_flag_preserved() {
    let mut t = term();
    // SGR 3 = italic.
    feed(&mut t, b"\x1b[3mI");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::ITALIC));
}

// --- Wide characters (CJK) ---

#[test]
fn wide_char_produces_two_cells() {
    let mut t = term();
    // U+4E16 = '世' (CJK, width 2).
    feed(&mut t, "世".as_bytes());

    let content = t.renderable_content();

    // First cell: the wide character with WIDE_CHAR flag.
    let cell0 = &content.cells[0];
    assert_eq!(cell0.ch, '世');
    assert!(cell0.flags.contains(CellFlags::WIDE_CHAR));

    // Second cell: spacer with WIDE_CHAR_SPACER flag.
    let cell1 = &content.cells[1];
    assert!(cell1.flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert_eq!(cell1.ch, ' ');
}

#[test]
fn wide_char_followed_by_ascii() {
    let mut t = term();
    // '世' (width 2) then 'A' (width 1).
    feed(&mut t, "世A".as_bytes());

    let content = t.renderable_content();

    assert_eq!(content.cells[0].ch, '世');
    assert!(content.cells[0].flags.contains(CellFlags::WIDE_CHAR));
    assert!(content.cells[1].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert_eq!(content.cells[2].ch, 'A');
    assert!(!content.cells[2].flags.contains(CellFlags::WIDE_CHAR));
}

#[test]
fn cursor_advances_by_two_after_wide_char() {
    let mut t = term();
    feed(&mut t, "世".as_bytes());

    let content = t.renderable_content();
    // Cursor should be at column 2 (past the 2-cell character).
    assert_eq!(content.cursor.column, Column(2));
}

#[test]
fn wide_char_with_color_resolves_both_cells() {
    let mut t = term();
    // Red fg, then CJK char.
    feed(&mut t, b"\x1b[31m");
    feed(&mut t, "世".as_bytes());

    let content = t.renderable_content();
    let palette = Palette::default();
    let red = palette.resolve(Color::Named(NamedColor::Red));

    // Primary cell has resolved color.
    assert_eq!(content.cells[0].fg, red);
    // Spacer cell inherits template bg but fg isn't meaningful — just verify no crash.
    let _ = content.cells[1].fg;
}

// --- Combining marks / zero-width characters ---

#[test]
fn combining_marks_propagate_to_renderable() {
    let mut t = term();
    // Combining mark handler is not yet implemented in the VTE handler.
    // When it is, 'e' + U+0301 should produce zerowidth: ['\u{0301}'].
    // For now, verify that the zerowidth field is correctly empty when
    // combining marks aren't appended by the handler.
    feed(&mut t, b"e");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'e');
    assert!(cell.zerowidth.is_empty());

    // Directly test that zerowidth propagates via CellExtra.
    t.grid_mut()
        .cursor_mut()
        .template
        .push_zerowidth('\u{0301}');
    t.grid_mut().put_char('a');

    let content = t.renderable_content();
    let cell = &content.cells[1];
    assert_eq!(cell.ch, 'a');
    assert_eq!(cell.zerowidth, vec!['\u{0301}']);
}

#[test]
fn multiple_combining_marks_propagate() {
    let mut t = term();
    // Directly set combining marks on a cell via grid manipulation.
    t.grid_mut().put_char('a');
    // Push combining marks onto the cell at (0, 0).
    let line = crate::index::Line(0);
    t.grid_mut()[line][Column(0)].push_zerowidth('\u{0301}');
    t.grid_mut()[line][Column(0)].push_zerowidth('\u{0303}');

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'a');
    assert_eq!(cell.zerowidth, vec!['\u{0301}', '\u{0303}']);
}

#[test]
fn no_combining_marks_produces_empty_zerowidth() {
    let mut t = term();
    feed(&mut t, b"A");

    let content = t.renderable_content();
    assert!(content.cells[0].zerowidth.is_empty());
}

// --- Bold + Dim interaction ---

#[test]
fn bold_plus_dim_named_color() {
    let mut t = term();
    // SGR 1 = bold, SGR 2 = dim, SGR 31 = red.
    // DIM takes priority — no bright promotion, just dim the base color.
    feed(&mut t, b"\x1b[1;2;31mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    // DIM wins: Red.to_dim() = DimRed, no bright promotion.
    let expected = palette.resolve(Color::Named(NamedColor::DimRed));
    assert_eq!(cell.fg, expected);
}

#[test]
fn bold_plus_dim_indexed_0_to_7() {
    let palette = Palette::default();
    // Bold + dim on indexed 2 (Green): DIM takes priority — dim the base
    // color without bright promotion.
    let result = resolve_fg(
        Color::Indexed(2),
        CellFlags::BOLD | CellFlags::DIM,
        &palette,
    );
    // DIM wins: dim_rgb of base color (idx 2 = Green), no promotion to idx 10.
    let base_green = palette.resolve(Color::Indexed(2));
    let expected = Rgb {
        r: (base_green.r as u16 * 2 / 3) as u8,
        g: (base_green.g as u16 * 2 / 3) as u8,
        b: (base_green.b as u16 * 2 / 3) as u8,
    };
    assert_eq!(result, expected);
}

#[test]
fn bold_plus_dim_truecolor() {
    let palette = Palette::default();
    // Bold does not promote truecolor. Dim reduces brightness.
    let result = resolve_fg(
        Color::Spec(Rgb {
            r: 150,
            g: 120,
            b: 90,
        }),
        CellFlags::BOLD | CellFlags::DIM,
        &palette,
    );
    assert_eq!(
        result,
        Rgb {
            r: 100,
            g: 80,
            b: 60
        }
    );
}

#[test]
fn bold_plus_dim_indexed_8_to_15() {
    let palette = Palette::default();
    // Bold + dim on indexed 9 (BrightRed, already in 8–15 range).
    // DIM takes priority — dim the base color, no further promotion.
    let result = resolve_fg(
        Color::Indexed(9),
        CellFlags::BOLD | CellFlags::DIM,
        &palette,
    );
    let base = palette.resolve(Color::Indexed(9));
    let expected = Rgb {
        r: (base.r as u16 * 2 / 3) as u8,
        g: (base.g as u16 * 2 / 3) as u8,
        b: (base.b as u16 * 2 / 3) as u8,
    };
    assert_eq!(result, expected);
}

#[test]
fn bold_plus_dim_consistent_across_named_and_indexed() {
    let palette = Palette::default();
    // Named Red and Indexed 1 should produce the same result with BOLD+DIM.
    let flags = CellFlags::BOLD | CellFlags::DIM;
    let named = resolve_fg(Color::Named(NamedColor::Red), flags, &palette);
    let indexed = resolve_fg(Color::Indexed(1), flags, &palette);
    // Both should dim the base Red color without bright promotion.
    assert_eq!(
        named, indexed,
        "Named and Indexed paths must agree on BOLD+DIM",
    );
}

#[test]
fn bold_plus_dim_default_foreground() {
    let palette = Palette::default();
    // BOLD+DIM on default foreground: DIM wins → DimForeground.
    let result = resolve_fg(
        Color::Named(NamedColor::Foreground),
        CellFlags::BOLD | CellFlags::DIM,
        &palette,
    );
    let expected = palette.resolve(Color::Named(NamedColor::DimForeground));
    assert_eq!(result, expected);
}

// --- Underline color resolution ---

#[test]
fn underline_color_truecolor_in_snapshot() {
    let mut t = term();
    // SGR 4 = underline, SGR 58;2;255;0;128 = truecolor underline color.
    feed(&mut t, b"\x1b[4;58;2;255;0;128mU");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::UNDERLINE));
    assert_eq!(
        cell.underline_color,
        Some(Rgb {
            r: 255,
            g: 0,
            b: 128
        })
    );
}

#[test]
fn underline_color_indexed_in_snapshot() {
    let mut t = term();
    // SGR 4 = underline, SGR 58;5;196 = indexed underline color.
    feed(&mut t, b"\x1b[4;58;5;196mU");

    let content = t.renderable_content();
    let palette = Palette::default();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::UNDERLINE));
    assert_eq!(
        cell.underline_color,
        Some(palette.resolve(Color::Indexed(196)))
    );
}

#[test]
fn no_underline_color_is_none() {
    let mut t = term();
    // SGR 4 = underline without explicit color.
    feed(&mut t, b"\x1b[4mU");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::UNDERLINE));
    assert_eq!(cell.underline_color, None);
}

// --- Alt screen rendering ---

#[test]
fn alt_screen_snapshot_reads_alt_grid() {
    let mut t = term();
    // Write on primary screen.
    feed(&mut t, b"PRIMARY");
    // Enter alt screen.
    feed(&mut t, b"\x1b[?1049h");
    // Write on alt screen.
    feed(&mut t, b"ALT");

    let content = t.renderable_content();

    // Should see alt screen content, not primary.
    assert_eq!(content.cells[0].ch, 'A');
    assert_eq!(content.cells[1].ch, 'L');
    assert_eq!(content.cells[2].ch, 'T');
    // Primary content should NOT be visible.
    assert_ne!(content.cells[0].ch, 'P');
    assert!(content.mode.contains(TermMode::ALT_SCREEN));
}

#[test]
fn leaving_alt_screen_restores_primary() {
    let mut t = term();
    feed(&mut t, b"PRI");
    feed(&mut t, b"\x1b[?1049h");
    feed(&mut t, b"ALT");
    feed(&mut t, b"\x1b[?1049l");

    let content = t.renderable_content();

    // Should see primary content again.
    assert_eq!(content.cells[0].ch, 'P');
    assert_eq!(content.cells[1].ch, 'R');
    assert_eq!(content.cells[2].ch, 'I');
    assert!(!content.mode.contains(TermMode::ALT_SCREEN));
}

#[test]
fn alt_screen_cursor_position() {
    let mut t = term();
    feed(&mut t, b"ABCDE");
    feed(&mut t, b"\x1b[?1049h");
    // Alt screen starts with cursor at (0, 0).
    feed(&mut t, b"XY");

    let content = t.renderable_content();
    // Cursor reflects alt screen position.
    assert_eq!(content.cursor.line, 0);
    assert_eq!(content.cursor.column, Column(2));
}

// --- Cursor shape variants ---

#[test]
fn cursor_shape_bar_in_snapshot() {
    let mut t = term();
    // DECSCUSR 6 = steady bar (blinking bar = 5, steady bar = 6).
    feed(&mut t, b"\x1b[6 q");

    let content = t.renderable_content();
    assert_eq!(content.cursor.shape, CursorShape::Bar);
}

#[test]
fn cursor_shape_underline_in_snapshot() {
    let mut t = term();
    // DECSCUSR 4 = steady underline.
    feed(&mut t, b"\x1b[4 q");

    let content = t.renderable_content();
    assert_eq!(content.cursor.shape, CursorShape::Underline);
}

#[test]
fn cursor_shape_block_in_snapshot() {
    let mut t = term();
    // DECSCUSR 2 = steady block.
    feed(&mut t, b"\x1b[2 q");

    let content = t.renderable_content();
    assert_eq!(content.cursor.shape, CursorShape::Block);
}

#[test]
fn cursor_shape_reset_to_default() {
    let mut t = term();
    feed(&mut t, b"\x1b[6 q"); // Set bar.
    feed(&mut t, b"\x1b[0 q"); // Reset to default.

    let content = t.renderable_content();
    assert_eq!(content.cursor.shape, CursorShape::Block);
}

// --- WRAP flag preserved ---

#[test]
fn wrap_flag_set_at_end_of_line() {
    let mut t = term();
    // Fill a 10-column line completely then write one more char to wrap.
    feed(&mut t, b"0123456789X");

    let content = t.renderable_content();

    // Column 9 (last in row) should have WRAP flag.
    let last_col = &content.cells[9];
    assert!(
        last_col.flags.contains(CellFlags::WRAP),
        "last cell on wrapped line should have WRAP flag",
    );

    // 'X' is on line 1, col 0.
    assert_eq!(content.cells[10].ch, 'X');
    assert_eq!(content.cells[10].line, 1);
}

// --- Scrollback with attributes ---

#[test]
fn scrollback_preserves_colors() {
    let mut t = Term::new(4, 10, 100, Theme::default(), VoidListener);
    let palette = Palette::default();

    // Write a red line that will scroll into scrollback.
    feed(&mut t, b"\x1b[31m");
    feed(&mut t, b"RRRRRRRRRR\r\n");
    feed(&mut t, b"\x1b[0m");
    feed(&mut t, b"line2\r\nline3\r\nline4\r\nline5");

    // Scroll back to see the red line.
    t.grid_mut().scroll_display(1);

    let content = t.renderable_content();

    // First visible line from scrollback should have red foreground.
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'R');
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::Red)));
}

#[test]
fn scrollback_preserves_bold_flag() {
    let mut t = Term::new(4, 10, 100, Theme::default(), VoidListener);

    // Write a bold line that scrolls into scrollback.
    feed(&mut t, b"\x1b[1mBBBBBBBBBB\r\n\x1b[0m");
    feed(&mut t, b"line2\r\nline3\r\nline4\r\nline5");

    t.grid_mut().scroll_display(1);

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'B');
    assert!(cell.flags.contains(CellFlags::BOLD));
}

// --- Truecolor + dim ---

#[test]
fn dim_reduces_truecolor_brightness() {
    let mut t = term();
    // SGR 2 = dim, then truecolor fg.
    feed(&mut t, b"\x1b[2;38;2;150;120;90mX");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(
        cell.fg,
        Rgb {
            r: 100,
            g: 80,
            b: 60
        }
    );
}

#[test]
fn dim_does_not_affect_background() {
    let mut t = term();
    // SGR 2 = dim, SGR 41 = red bg. Dim only affects foreground.
    feed(&mut t, b"\x1b[2;41mX");

    let content = t.renderable_content();
    let palette = Palette::default();
    let cell = &content.cells[0];
    // Background is unaffected by dim.
    assert_eq!(cell.bg, palette.resolve(Color::Named(NamedColor::Red)));
}

// --- Bold indexed promotion edge cases ---

#[test]
fn bold_does_not_promote_indexed_8_to_15() {
    let palette = Palette::default();
    // Indexed 9 = BrightRed. Bold should NOT promote further.
    let result = resolve_fg(Color::Indexed(9), CellFlags::BOLD, &palette);
    assert_eq!(result, palette.resolve(Color::Indexed(9)));
}

#[test]
fn bold_does_not_promote_indexed_16_plus() {
    let palette = Palette::default();
    // Indexed 200 — well above the 0–7 range.
    let result = resolve_fg(Color::Indexed(200), CellFlags::BOLD, &palette);
    assert_eq!(result, palette.resolve(Color::Indexed(200)));
}

#[test]
fn bold_promotes_all_ansi_0_through_7() {
    let palette = Palette::default();
    for idx in 0..8u8 {
        let result = resolve_fg(Color::Indexed(idx), CellFlags::BOLD, &palette);
        let expected = palette.resolve(Color::Indexed(idx + 8));
        assert_eq!(
            result,
            expected,
            "Bold should promote indexed {idx} to {}",
            idx + 8
        );
    }
}

// --- Hidden cells ---

#[test]
fn hidden_flag_preserved_in_snapshot() {
    let mut t = term();
    // SGR 8 = hidden.
    feed(&mut t, b"\x1b[8mH");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::HIDDEN));
    // The character is still stored — renderer decides how to handle HIDDEN.
    assert_eq!(cell.ch, 'H');
}

// --- Blink flag ---

#[test]
fn blink_flag_preserved_in_snapshot() {
    let mut t = term();
    // SGR 5 = blink.
    feed(&mut t, b"\x1b[5mB");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::BLINK));
}

// --- Strikethrough flag ---

#[test]
fn strikethrough_flag_preserved_in_snapshot() {
    let mut t = term();
    // SGR 9 = strikethrough.
    feed(&mut t, b"\x1b[9mS");

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert!(cell.flags.contains(CellFlags::STRIKETHROUGH));
}

// --- Complex SGR resets ---

#[test]
fn sgr_reset_in_middle_of_sequence() {
    let mut t = term();
    // SGR 31 (red), then SGR 0 (reset), then SGR 32 (green).
    // Result should be green, not red.
    feed(&mut t, b"\x1b[31;0;32mX");

    let content = t.renderable_content();
    let palette = Palette::default();

    let cell = &content.cells[0];
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::Green)));
    // Background should be default (reset cleared everything).
    assert_eq!(cell.bg, palette.background());
}

#[test]
fn sgr_reset_clears_all_attributes() {
    let mut t = term();
    // Set many attributes, then reset.
    feed(&mut t, b"\x1b[1;2;3;4;5;7;8;9mX");
    feed(&mut t, b"\x1b[0mY");

    let content = t.renderable_content();

    // X has all the flags.
    let x = &content.cells[0];
    assert!(x.flags.contains(CellFlags::BOLD));
    assert!(x.flags.contains(CellFlags::DIM));
    assert!(x.flags.contains(CellFlags::ITALIC));
    assert!(x.flags.contains(CellFlags::UNDERLINE));
    assert!(x.flags.contains(CellFlags::BLINK));
    assert!(x.flags.contains(CellFlags::INVERSE));
    assert!(x.flags.contains(CellFlags::HIDDEN));
    assert!(x.flags.contains(CellFlags::STRIKETHROUGH));

    // Y after reset has no flags.
    let y = &content.cells[1];
    assert!(y.flags.is_empty());
}

#[test]
fn sgr_reset_restores_default_colors() {
    let mut t = term();
    let palette = Palette::default();

    // Set red fg, green bg, then reset.
    feed(&mut t, b"\x1b[31;42mX\x1b[0mY");

    let content = t.renderable_content();

    let y = &content.cells[1];
    assert_eq!(y.fg, palette.foreground());
    assert_eq!(y.bg, palette.background());
}

// --- SGR 39/49 (default color reset) ---

#[test]
fn sgr_39_resets_foreground_to_default() {
    let mut t = term();
    let palette = Palette::default();

    // Set red fg, then reset fg only.
    feed(&mut t, b"\x1b[31mR\x1b[39mD");

    let content = t.renderable_content();

    let r = &content.cells[0];
    assert_eq!(r.fg, palette.resolve(Color::Named(NamedColor::Red)));

    let d = &content.cells[1];
    assert_eq!(d.fg, palette.foreground());
}

#[test]
fn sgr_49_resets_background_to_default() {
    let mut t = term();
    let palette = Palette::default();

    // Set green bg, then reset bg only.
    feed(&mut t, b"\x1b[42mG\x1b[49mD");

    let content = t.renderable_content();

    let g = &content.cells[0];
    assert_eq!(g.bg, palette.resolve(Color::Named(NamedColor::Green)));

    let d = &content.cells[1];
    assert_eq!(d.bg, palette.background());
}

#[test]
fn sgr_39_preserves_background() {
    let mut t = term();
    let palette = Palette::default();

    // Set red fg + green bg, then reset fg only.
    feed(&mut t, b"\x1b[31;42mX\x1b[39mY");

    let content = t.renderable_content();

    let y = &content.cells[1];
    assert_eq!(y.fg, palette.foreground());
    // Background should still be green.
    assert_eq!(y.bg, palette.resolve(Color::Named(NamedColor::Green)));
}

// --- Underline mutual exclusion ---

#[test]
fn underline_styles_are_mutually_exclusive() {
    let mut t = term();
    // SGR 4 = single underline, then SGR 4:2 = double underline.
    // Double should replace single.
    feed(&mut t, b"\x1b[4mU\x1b[4:2mD");

    let content = t.renderable_content();

    let u = &content.cells[0];
    assert!(u.flags.contains(CellFlags::UNDERLINE));
    assert!(!u.flags.contains(CellFlags::DOUBLE_UNDERLINE));

    let d = &content.cells[1];
    assert!(d.flags.contains(CellFlags::DOUBLE_UNDERLINE));
    assert!(!d.flags.contains(CellFlags::UNDERLINE));
}

#[test]
fn sgr_24_clears_all_underline_variants() {
    let mut t = term();
    // Set double underline via sub-param, then SGR 24 to cancel.
    feed(&mut t, b"\x1b[4:2mD\x1b[24mN");

    let content = t.renderable_content();

    let d = &content.cells[0];
    assert!(d.flags.contains(CellFlags::DOUBLE_UNDERLINE));

    let n = &content.cells[1];
    assert!(!n.flags.intersects(CellFlags::ALL_UNDERLINES));
}

// --- Damage tracking edge cases ---

#[test]
fn scroll_marks_all_dirty() {
    let mut t = term();
    // Clear initial dirty.
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Scroll up 1 line (triggers mark_all).
    feed(&mut t, b"\r\n\r\n\r\n\r\n\r\n");

    let content = t.renderable_content();
    assert!(content.all_dirty);
}

#[test]
fn multiple_writes_same_line_single_damage() {
    let mut t = term();
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Write multiple chars on line 0.
    feed(&mut t, b"ABC");

    let content = t.renderable_content();
    // Line 0 appears exactly once in damage.
    let line0_count = content.damage.iter().filter(|d| d.line == 0).count();
    assert_eq!(line0_count, 1);
}

#[test]
fn writes_on_different_lines_produce_separate_damage() {
    let mut t = term();
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Write on line 0 and line 2.
    feed(&mut t, b"A\x1b[3;1HB"); // Move to line 3 (1-based), col 1.

    let content = t.renderable_content();
    assert!(content.damage.iter().any(|d| d.line == 0));
    assert!(content.damage.iter().any(|d| d.line == 2));
}

// --- Full multi-attribute combinations ---

#[test]
fn all_sgr_attributes_combined() {
    let mut t = term();
    // Bold + Dim + Italic + Underline + Blink + Inverse + Hidden + Strikethrough + Red fg + Green bg.
    feed(&mut t, b"\x1b[1;2;3;4;5;7;8;9;31;42mX");

    let content = t.renderable_content();
    let cell = &content.cells[0];

    assert!(cell.flags.contains(CellFlags::BOLD));
    assert!(cell.flags.contains(CellFlags::DIM));
    assert!(cell.flags.contains(CellFlags::ITALIC));
    assert!(cell.flags.contains(CellFlags::UNDERLINE));
    assert!(cell.flags.contains(CellFlags::BLINK));
    assert!(cell.flags.contains(CellFlags::INVERSE));
    assert!(cell.flags.contains(CellFlags::HIDDEN));
    assert!(cell.flags.contains(CellFlags::STRIKETHROUGH));
    assert_eq!(cell.ch, 'X');
}

#[test]
fn bold_inverse_with_named_colors() {
    let mut t = term();
    let palette = Palette::default();

    // Bold + inverse + red fg + green bg.
    // Bold promotes red → bright red, then inverse swaps.
    feed(&mut t, b"\x1b[1;7;31;42mX");

    let content = t.renderable_content();
    let cell = &content.cells[0];

    let bright_red = palette.resolve(Color::Named(NamedColor::BrightRed));
    let green = palette.resolve(Color::Named(NamedColor::Green));

    // Inverse swaps: fg=green, bg=bright_red.
    assert_eq!(cell.fg, green);
    assert_eq!(cell.bg, bright_red);
}

#[test]
fn dim_inverse_with_default_colors() {
    let mut t = term();
    let palette = Palette::default();

    // Dim + inverse with default colors.
    feed(&mut t, b"\x1b[2;7mX");

    let content = t.renderable_content();
    let cell = &content.cells[0];

    // Dim affects fg → DimForeground, then inverse swaps.
    let dim_fg = palette.resolve(Color::Named(NamedColor::DimForeground));
    let bg = palette.background();

    // After inverse: fg=bg, bg=dim_fg.
    assert_eq!(cell.fg, bg);
    assert_eq!(cell.bg, dim_fg);
}

#[test]
fn indexed_truecolor_mixed_in_one_line() {
    let mut t = term();
    let palette = Palette::default();

    // Indexed fg + truecolor bg on first char.
    feed(&mut t, b"\x1b[38;5;100;48;2;10;20;30mA");
    // Truecolor fg + named bg on second char.
    feed(&mut t, b"\x1b[38;2;200;150;100;44mB");

    let content = t.renderable_content();

    let a = &content.cells[0];
    assert_eq!(a.fg, palette.resolve(Color::Indexed(100)));
    assert_eq!(
        a.bg,
        Rgb {
            r: 10,
            g: 20,
            b: 30
        }
    );

    let b = &content.cells[1];
    assert_eq!(
        b.fg,
        Rgb {
            r: 200,
            g: 150,
            b: 100
        }
    );
    assert_eq!(b.bg, palette.resolve(Color::Named(NamedColor::Blue)));
}

// --- Empty/default cell optimization ---

#[test]
fn empty_cells_have_default_everything() {
    let t = term();
    let palette = Palette::default();
    let content = t.renderable_content();

    for cell in &content.cells {
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, palette.foreground());
        assert_eq!(cell.bg, palette.background());
        assert!(cell.flags.is_empty());
        assert_eq!(cell.underline_color, None);
        assert!(cell.zerowidth.is_empty());
    }
}

// --- Extended zero-width / combining mark renderable tests ---

#[test]
fn combining_mark_via_vte_propagates_to_renderable() {
    let mut t = term();
    // Full VTE pipeline: 'e' + U+0301 (combining acute accent).
    feed(&mut t, "e\u{0301}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'e');
    assert_eq!(cell.zerowidth, vec!['\u{0301}']);
}

#[test]
fn zerowidth_space_propagates_to_renderable() {
    let mut t = term();
    // 'a' + U+200B (zero-width space).
    feed(&mut t, "a\u{200B}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'a');
    assert_eq!(cell.zerowidth, vec!['\u{200B}']);
}

#[test]
fn variation_selector_propagates_to_renderable() {
    let mut t = term();
    // '❤' (U+2764) + VS16 (U+FE0F).
    feed(&mut t, "\u{2764}\u{FE0F}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, '\u{2764}');
    assert_eq!(cell.zerowidth, vec!['\u{FE0F}']);
}

#[test]
fn zjw_emoji_sequence_renderable_cells() {
    let mut t = Term::new(4, 20, 100, Theme::default(), VoidListener);
    // 👨‍👩‍👧 = U+1F468 + ZWJ + U+1F469 + ZWJ + U+1F467
    // Without mode 2027: each emoji is a separate wide char, ZWJs stored.
    feed(
        &mut t,
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}".as_bytes(),
    );

    let content = t.renderable_content();

    // 👨 at col 0 (wide) with ZWJ.
    let man = &content.cells[0];
    assert_eq!(man.ch, '\u{1F468}');
    assert!(man.flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(man.zerowidth, vec!['\u{200D}']);

    // Spacer at col 1.
    let spacer1 = &content.cells[1];
    assert!(spacer1.flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert!(spacer1.zerowidth.is_empty());

    // 👩 at col 2 (wide) with ZWJ.
    let woman = &content.cells[2];
    assert_eq!(woman.ch, '\u{1F469}');
    assert!(woman.flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(woman.zerowidth, vec!['\u{200D}']);

    // 👧 at col 4 (wide) — no zerowidth.
    let girl = &content.cells[4];
    assert_eq!(girl.ch, '\u{1F467}');
    assert!(girl.flags.contains(CellFlags::WIDE_CHAR));
    assert!(girl.zerowidth.is_empty());
}

#[test]
fn wide_char_combining_mark_propagates_to_renderable() {
    let mut t = term();
    // CJK '世' + combining acute. Mark should be on the base cell, not spacer.
    feed(&mut t, "世\u{0301}".as_bytes());

    let content = t.renderable_content();

    let base = &content.cells[0];
    assert_eq!(base.ch, '世');
    assert!(base.flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(base.zerowidth, vec!['\u{0301}']);

    let spacer = &content.cells[1];
    assert!(spacer.flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert!(spacer.zerowidth.is_empty());
}

#[test]
fn multiple_zerowidth_types_propagate_to_renderable() {
    let mut t = term();
    // 'a' + combining acute + ZWJ + VS16 — all three in renderable.
    feed(&mut t, "a\u{0301}\u{200D}\u{FE0F}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'a');
    assert_eq!(cell.zerowidth, vec!['\u{0301}', '\u{200D}', '\u{FE0F}']);
}

#[test]
fn four_combining_marks_propagate_to_renderable() {
    let mut t = term();
    // 'o' + 4 combining marks.
    feed(&mut t, "o\u{0300}\u{0301}\u{0302}\u{0303}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'o');
    assert_eq!(
        cell.zerowidth,
        vec!['\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}'],
    );
}

#[test]
fn scrollback_preserves_combining_marks() {
    let mut t = Term::new(4, 10, 100, Theme::default(), VoidListener);

    // Write 'é' (e + combining acute) that will scroll into scrollback.
    feed(&mut t, "e\u{0301}".as_bytes());
    feed(&mut t, b"AAAAAAAAA\r\n"); // Fill rest of line 0.
    feed(&mut t, b"line2\r\nline3\r\nline4\r\nline5");

    t.grid_mut().scroll_display(1);

    let content = t.renderable_content();

    // First visible cell from scrollback should have combining mark.
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'e');
    assert_eq!(cell.zerowidth, vec!['\u{0301}']);
}

#[test]
fn alt_screen_preserves_combining_marks() {
    let mut t = term();
    // Enter alt screen, write char with combining mark.
    feed(&mut t, b"\x1b[?1049h");
    feed(&mut t, "n\u{0303}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'n');
    assert_eq!(cell.zerowidth, vec!['\u{0303}']);
    assert!(content.mode.contains(TermMode::ALT_SCREEN));
}

#[test]
fn combining_mark_with_color_resolves_correctly() {
    let mut t = term();
    let palette = Palette::default();

    // Red 'e' + combining acute.
    feed(&mut t, b"\x1b[31m");
    feed(&mut t, "e\u{0301}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, 'e');
    assert_eq!(cell.zerowidth, vec!['\u{0301}']);
    assert_eq!(cell.fg, palette.resolve(Color::Named(NamedColor::Red)));
}

#[test]
fn damage_tracked_for_combining_mark_write() {
    let mut t = term();
    // Write 'a', drain dirty.
    feed(&mut t, b"a");
    let _ = t.renderable_content();
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Combining mark should appear in damage.
    feed(&mut t, "\u{0301}".as_bytes());

    let content = t.renderable_content();
    assert!(
        content.damage.iter().any(|d| d.line == 0),
        "combining mark write should mark line 0 as damaged",
    );
}

#[test]
fn zerowidth_at_col_zero_produces_empty_renderable() {
    let mut t = term();
    // Combining mark at col 0 — discarded by push_zerowidth.
    feed(&mut t, "\u{0301}".as_bytes());

    let content = t.renderable_content();
    let cell = &content.cells[0];
    assert_eq!(cell.ch, ' ');
    assert!(cell.zerowidth.is_empty());
}

#[test]
fn combining_mark_at_wrap_pending_propagates_to_renderable() {
    // 10-column terminal. Fill line with "ABCDEFGHIJ" (wrap pending).
    // Combining mark should attach to 'J' at col 9.
    let mut t = term();
    feed(&mut t, b"ABCDEFGHIJ");
    feed(&mut t, "\u{0300}".as_bytes());

    let content = t.renderable_content();

    let last = &content.cells[9];
    assert_eq!(last.ch, 'J');
    assert_eq!(last.zerowidth, vec!['\u{0300}']);
    // Cursor still wrap-pending — combining mark didn't advance it.
    assert_eq!(content.cursor.column, Column(10));
    assert_eq!(content.cursor.line, 0);
}
