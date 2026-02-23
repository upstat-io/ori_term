//! Tests for HTML extraction from grid selections.

use vte::ansi::{Color, NamedColor};

use crate::cell::CellFlags;
use crate::color::Palette;
use crate::grid::{Grid, StableRowIndex};
use crate::index::{Column, Side};

use super::super::{Selection, SelectionMode, SelectionPoint};
use super::extract_html;

/// Helper: create a grid and write text at row 0.
fn grid_with_text(text: &str) -> Grid {
    let cols = text.len().max(10);
    let mut grid = Grid::new(5, cols);
    grid.move_to(0, Column(0));
    for c in text.chars() {
        grid.put_char(c);
    }
    grid
}

/// Helper: create a char selection spanning row 0 from col `start` to col `end`.
fn char_selection(grid: &Grid, start: usize, end: usize) -> Selection {
    let base = StableRowIndex::from_visible(grid, 0);
    let mut sel = Selection::new_char(base, start, Side::Left);
    sel.end = SelectionPoint {
        row: base,
        col: end,
        side: Side::Right,
    };
    sel
}

// -- Basic extraction --

#[test]
fn plain_text_produces_pre_wrapper() {
    let grid = grid_with_text("hello");
    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 4);
    let html = extract_html(&grid, &sel, &palette, "JetBrains Mono", 12.0);

    assert!(html.starts_with("<pre style=\""));
    assert!(html.ends_with("</pre>"));
    assert!(html.contains("font-family:'JetBrains Mono',monospace"));
    assert!(html.contains("font-size:12.0pt"));
    assert!(html.contains("hello"));
}

#[test]
fn plain_text_no_spans() {
    let grid = grid_with_text("abc");
    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // No spans for default-styled text.
    assert!(!html.contains("<span"));
    assert!(!html.contains("</span>"));
}

// -- Color styling --

#[test]
fn colored_text_gets_span_with_color() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.fg = Color::Indexed(1); // red
    grid.put_char('A');
    grid.put_char('B');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 1);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("<span style=\""));
    assert!(html.contains("color:#"));
    assert!(html.contains("AB"));
}

// -- Bold styling --

#[test]
fn bold_text_gets_font_weight() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::BOLD;
    grid.put_char('X');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("font-weight:bold"));
}

// -- Italic styling --

#[test]
fn italic_text_gets_font_style() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::ITALIC;
    grid.put_char('I');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("font-style:italic"));
}

// -- Underline variants --

#[test]
fn underline_text_gets_text_decoration() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::UNDERLINE;
    grid.put_char('U');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("text-decoration:underline"));
}

#[test]
fn curly_underline_maps_to_wavy() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::CURLY_UNDERLINE;
    grid.put_char('W');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("text-decoration:underline wavy"));
}

// -- Strikethrough --

#[test]
fn strikethrough_text_gets_line_through() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::STRIKETHROUGH;
    grid.put_char('S');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("text-decoration:line-through"));
}

// -- Combined underline + strikethrough --

#[test]
fn underline_and_strikethrough_combined() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::UNDERLINE | CellFlags::STRIKETHROUGH;
    grid.put_char('C');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("text-decoration:underline line-through"));
}

// -- Dim styling --

#[test]
fn dim_text_gets_opacity() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::DIM;
    grid.put_char('D');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("opacity:0.5"));
}

// -- HTML escaping --

#[test]
fn html_special_chars_escaped() {
    let grid = grid_with_text("<a>&x");
    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 4);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("&lt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("&gt;"));
}

// -- Style coalescing --

#[test]
fn adjacent_cells_same_style_coalesced() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::BOLD;
    grid.put_char('A');
    grid.put_char('B');
    grid.put_char('C');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // Should have exactly one opening span and one closing span.
    assert_eq!(html.matches("<span").count(), 1);
    assert_eq!(html.matches("</span>").count(), 1);
    assert!(html.contains("ABC"));
}

// -- Inverse (INVERSE flag) --

#[test]
fn inverse_swaps_fg_and_bg() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.fg = Color::Named(NamedColor::Foreground);
    grid.cursor_mut().template.bg = Color::Named(NamedColor::Background);
    grid.cursor_mut().template.flags = CellFlags::INVERSE;
    grid.put_char('R');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // After inverse, the default fg becomes the bg and vice versa. Both are
    // "non-default" from the perspective of the swapped comparison, so we
    // expect color and background-color CSS properties.
    assert!(html.contains("color:#"));
    assert!(html.contains("background-color:#"));
}

// -- Empty selection --

#[test]
fn out_of_range_selection_returns_empty() {
    let grid = Grid::new(5, 20);
    let sel = Selection::new_char(StableRowIndex(999_999), 0, Side::Left);
    let palette = Palette::default();
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.is_empty());
}

// -- Multi-line --

#[test]
fn multi_line_selection_has_newlines() {
    let mut grid = Grid::new(5, 10);
    grid.move_to(0, Column(0));
    for c in "hello".chars() {
        grid.put_char(c);
    }
    grid.move_to(1, Column(0));
    for c in "world".chars() {
        grid.put_char(c);
    }

    let palette = Palette::default();
    let base0 = StableRowIndex::from_visible(&grid, 0);
    let base1 = StableRowIndex::from_visible(&grid, 1);
    let mut sel = Selection::new_char(base0, 0, Side::Left);
    sel.end = SelectionPoint {
        row: base1,
        col: 4,
        side: Side::Right,
    };

    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // Body should contain a newline between the two lines.
    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];
    assert!(body.contains('\n'));
    assert!(body.contains("hello"));
    assert!(body.contains("world"));
}

// -- Background color --

#[test]
fn background_color_produces_css() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.bg = Color::Indexed(4); // blue
    grid.put_char('B');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("background-color:#"));
}

// -- Combined styles --

#[test]
fn bold_italic_colored_produces_all_css_properties() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::BOLD | CellFlags::ITALIC;
    grid.cursor_mut().template.fg = Color::Indexed(1); // red
    grid.put_char('M');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("font-weight:bold"));
    assert!(html.contains("font-style:italic"));
    assert!(html.contains("color:#"));
}

#[test]
fn bold_dim_produces_both_properties() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::BOLD | CellFlags::DIM;
    grid.put_char('D');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    assert!(html.contains("font-weight:bold"));
    assert!(html.contains("opacity:0.5"));
}

// -- Style change mid-row (coalescing boundary) --

#[test]
fn style_change_mid_row_produces_multiple_spans() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));

    // 'A' is bold.
    grid.cursor_mut().template.flags = CellFlags::BOLD;
    grid.put_char('A');

    // 'B' is default (no flags).
    grid.cursor_mut().template.flags = CellFlags::empty();
    grid.cursor_mut().template.fg = Color::Named(NamedColor::Foreground);
    grid.put_char('B');

    // 'C' is italic.
    grid.cursor_mut().template.flags = CellFlags::ITALIC;
    grid.put_char('C');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // Should have 2 spans: one for bold 'A', one for italic 'C'.
    // 'B' is default-styled (no span).
    assert_eq!(html.matches("<span").count(), 2, "bold A + italic C");
    assert_eq!(html.matches("</span>").count(), 2);
    assert!(html.contains("font-weight:bold"));
    assert!(html.contains("font-style:italic"));
    assert!(html.contains('A'));
    assert!(html.contains('B'));
    assert!(html.contains('C'));
}

// -- Wide characters in HTML extraction --

#[test]
fn wide_char_skips_spacer_in_html() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢'); // wide: cols 0-1
    grid.put_char('X'); // col 2

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // Should contain '漢' once and 'X' once, no duplicate from spacer.
    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];
    assert_eq!(body.matches('漢').count(), 1);
    assert!(body.contains('X'));
}

#[test]
fn emoji_wide_char_in_html() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.put_char('💀'); // wide emoji
    grid.put_char('A');

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];
    assert_eq!(body.matches('💀').count(), 1);
    assert!(body.contains('A'));
}

// -- HIDDEN cells --

#[test]
fn hidden_cells_skipped_in_html() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.put_char('A');
    grid.put_char('B');
    grid.put_char('C');

    // Mark 'B' as HIDDEN.
    grid[crate::index::Line(0)][Column(1)].flags |= CellFlags::HIDDEN;

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];
    assert!(body.contains('A'));
    assert!(!body.contains('B'), "HIDDEN cell should be skipped");
    assert!(body.contains('C'));
}

// -- Block mode HTML extraction --

#[test]
fn block_mode_html_extracts_rectangular_region() {
    let mut grid = Grid::new(5, 10);
    grid.move_to(0, Column(0));
    for c in "ABCDEFGHIJ".chars() {
        grid.put_char(c);
    }
    grid.move_to(1, Column(0));
    for c in "KLMNOPQRST".chars() {
        grid.put_char(c);
    }

    let palette = Palette::default();
    let base0 = StableRowIndex::from_visible(&grid, 0);
    let base1 = StableRowIndex::from_visible(&grid, 1);

    // Block select cols 2-5 across rows 0-1.
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: base0,
            col: 2,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: base0,
            col: 2,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: base1,
            col: 5,
            side: Side::Right,
        },
    };

    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);
    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];

    // Row 0: cols 2-5 = "CDEF", Row 1: cols 2-5 = "MNOP".
    assert!(body.contains("CDEF"));
    assert!(body.contains("MNOP"));
    assert!(body.contains('\n'), "block mode rows separated by newline");
}

// -- Trailing whitespace trim --

#[test]
fn trailing_whitespace_trimmed() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    for c in "hello".chars() {
        grid.put_char(c);
    }
    // Rest of row is spaces (default cells).

    let palette = Palette::default();
    // Select the entire row including trailing spaces.
    let sel = char_selection(&grid, 0, 19);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // The body should be trimmed — no trailing spaces before </pre>.
    assert!(html.ends_with("</pre>"));
    let pre_end = html.rfind("</pre>").unwrap();
    let before = html.as_bytes()[pre_end - 1];
    assert_ne!(before, b' ', "trailing spaces should be trimmed");
}

// -- Deterministic output --

#[test]
fn identical_selections_produce_identical_html() {
    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.cursor_mut().template.flags = CellFlags::BOLD;
    grid.cursor_mut().template.fg = Color::Indexed(2); // green
    for c in "test".chars() {
        grid.put_char(c);
    }

    let palette = Palette::default();
    let sel1 = char_selection(&grid, 0, 3);
    let sel2 = char_selection(&grid, 0, 3);
    let html1 = extract_html(&grid, &sel1, &palette, "Mono", 10.0);
    let html2 = extract_html(&grid, &sel2, &palette, "Mono", 10.0);

    assert_eq!(
        html1, html2,
        "identical selections must produce identical HTML"
    );
}

// -- Font family with special characters --

#[test]
fn font_family_with_spaces_in_pre_style() {
    let grid = grid_with_text("abc");
    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 2);
    let html = extract_html(&grid, &sel, &palette, "JetBrains Mono NL", 11.5);

    assert!(html.contains("font-family:'JetBrains Mono NL',monospace"));
    assert!(html.contains("font-size:11.5pt"));
}

// -- Zero-width characters (combining marks) --

#[test]
fn zerowidth_chars_included_in_html() {
    use std::sync::Arc;

    let mut grid = Grid::new(5, 20);
    grid.move_to(0, Column(0));
    grid.put_char('e');

    // Manually add a combining acute accent to the cell.
    let cell = &mut grid[crate::index::Line(0)][Column(0)];
    let extra = cell
        .extra
        .get_or_insert_with(|| Arc::new(crate::cell::CellExtra::new()));
    Arc::make_mut(extra).zerowidth.push('\u{0301}'); // COMBINING ACUTE ACCENT

    let palette = Palette::default();
    let sel = char_selection(&grid, 0, 0);
    let html = extract_html(&grid, &sel, &palette, "Mono", 10.0);

    // The HTML should contain 'e' followed by the combining mark.
    let body_start = html.find('>').unwrap() + 1;
    let body = &html[body_start..html.rfind("</pre>").unwrap()];
    assert!(body.contains('e'));
    assert!(
        body.contains('\u{0301}'),
        "combining mark should be in HTML"
    );
}
