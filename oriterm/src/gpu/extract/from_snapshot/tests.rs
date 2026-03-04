//! Tests for snapshot-to-FrameInput conversion.

use oriterm_core::{CellFlags, Column, CursorShape, Rgb, TermMode};
use oriterm_mux::{PaneSnapshot, WireCell, WireCursor, WireCursorShape, WireRgb};

use crate::font::CellMetrics;
use crate::gpu::frame_input::ViewportSize;

use super::{
    PALETTE_BACKGROUND, PALETTE_CURSOR, PALETTE_FOREGROUND, extract_frame_from_snapshot,
    extract_frame_from_snapshot_into, snapshot_palette, snapshot_to_renderable,
    snapshot_to_renderable_into,
};

/// Build a minimal test snapshot with 2×2 cells.
fn test_snapshot() -> PaneSnapshot {
    let white = WireRgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let black = WireRgb { r: 0, g: 0, b: 0 };

    PaneSnapshot {
        cells: vec![
            vec![
                WireCell {
                    ch: 'A',
                    fg: white,
                    bg: black,
                    flags: 0,
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                },
                WireCell {
                    ch: 'B',
                    fg: white,
                    bg: black,
                    flags: CellFlags::BOLD.bits(),
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                },
            ],
            vec![
                WireCell {
                    ch: ' ',
                    fg: white,
                    bg: black,
                    flags: 0,
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                },
                WireCell {
                    ch: 'C',
                    fg: WireRgb { r: 255, g: 0, b: 0 },
                    bg: black,
                    flags: CellFlags::UNDERLINE.bits(),
                    underline_color: Some(WireRgb {
                        r: 0,
                        g: 128,
                        b: 255,
                    }),
                    hyperlink_uri: Some("https://test.example".to_string()),
                    zerowidth: vec!['\u{0301}'],
                },
            ],
        ],
        cursor: WireCursor {
            col: 1,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: (0..270).map(|i| [(i % 256) as u8, 0, 0]).collect(),
        title: "test".into(),
        icon_name: None,
        cwd: None,
        modes: TermMode::SHOW_CURSOR.bits(),
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 2,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    }
}

fn test_cell_metrics() -> CellMetrics {
    CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 5.0)
}

#[test]
fn renderable_cell_positions() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert_eq!(content.cells.len(), 4);
    assert_eq!(content.cells[0].line, 0);
    assert_eq!(content.cells[0].column, Column(0));
    assert_eq!(content.cells[0].ch, 'A');
    assert_eq!(content.cells[1].line, 0);
    assert_eq!(content.cells[1].column, Column(1));
    assert_eq!(content.cells[1].ch, 'B');
    assert_eq!(content.cells[2].line, 1);
    assert_eq!(content.cells[2].column, Column(0));
    assert_eq!(content.cells[3].line, 1);
    assert_eq!(content.cells[3].column, Column(1));
    assert_eq!(content.cells[3].ch, 'C');
}

#[test]
fn renderable_colors_pre_resolved() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert_eq!(
        content.cells[0].fg,
        Rgb {
            r: 211,
            g: 215,
            b: 207
        }
    );
    assert_eq!(content.cells[0].bg, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(content.cells[3].fg, Rgb { r: 255, g: 0, b: 0 });
}

#[test]
fn renderable_flags_preserved() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert!(content.cells[1].flags.contains(CellFlags::BOLD));
    assert!(content.cells[3].flags.contains(CellFlags::UNDERLINE));
    assert!(!content.cells[0].flags.contains(CellFlags::BOLD));
}

#[test]
fn renderable_underline_color_and_hyperlink() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert_eq!(content.cells[0].underline_color, None);
    assert!(!content.cells[0].has_hyperlink);

    assert_eq!(
        content.cells[3].underline_color,
        Some(Rgb {
            r: 0,
            g: 128,
            b: 255
        })
    );
    assert!(content.cells[3].has_hyperlink);
}

#[test]
fn renderable_zerowidth() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert!(content.cells[0].zerowidth.is_empty());
    assert_eq!(content.cells[3].zerowidth, vec!['\u{0301}']);
}

#[test]
fn renderable_cursor() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert_eq!(content.cursor.line, 0);
    assert_eq!(content.cursor.column, Column(1));
    assert_eq!(content.cursor.shape, CursorShape::Block);
    assert!(content.cursor.visible);
}

#[test]
fn renderable_mode_flags() {
    let snap = test_snapshot();
    let content = snapshot_to_renderable(&snap);

    assert!(content.mode.contains(TermMode::SHOW_CURSOR));
    assert!(content.all_dirty);
    assert!(content.damage.is_empty());
}

#[test]
fn palette_extracts_semantic_colors() {
    let snap = test_snapshot();
    let palette = snapshot_palette(&snap);

    // Palette entries at indices 256, 257, 258 are [idx % 256, 0, 0].
    assert_eq!(
        palette.foreground,
        Rgb {
            r: (PALETTE_FOREGROUND % 256) as u8,
            g: 0,
            b: 0
        }
    );
    assert_eq!(
        palette.background,
        Rgb {
            r: (PALETTE_BACKGROUND % 256) as u8,
            g: 0,
            b: 0
        }
    );
    assert_eq!(
        palette.cursor_color,
        Rgb {
            r: (PALETTE_CURSOR % 256) as u8,
            g: 0,
            b: 0
        }
    );
    assert_eq!(palette.opacity, 1.0);
    assert_eq!(palette.selection_fg, None);
    assert_eq!(palette.selection_bg, None);
}

#[test]
fn extract_frame_produces_valid_frame_input() {
    let snap = test_snapshot();
    let viewport = ViewportSize::new(160, 320);
    let cell = test_cell_metrics();

    let frame = extract_frame_from_snapshot(&snap, viewport, cell);

    assert_eq!(frame.viewport, viewport);
    assert_eq!(frame.cell_size, cell);
    assert_eq!(frame.content.cells.len(), 4);
    assert!(frame.selection.is_none());
    assert!(frame.search.is_none());
    assert!(frame.hovered_cell.is_none());
    assert!(frame.hovered_url_segments.is_empty());
    assert!(frame.mark_cursor.is_none());
    assert_eq!(frame.fg_dim, 1.0);
    assert!(frame.prompt_marker_rows.is_empty());
}

#[test]
fn palette_handles_short_array() {
    let mut snap = test_snapshot();
    snap.palette.clear();

    let palette = snapshot_palette(&snap);

    // Missing entries default to black.
    assert_eq!(palette.foreground, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(palette.background, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(palette.cursor_color, Rgb { r: 0, g: 0, b: 0 });
}

// -- Cursor shape variant tests --

#[test]
fn cursor_shape_all_variants() {
    let variants = [
        (WireCursorShape::Block, CursorShape::Block),
        (WireCursorShape::Underline, CursorShape::Underline),
        (WireCursorShape::Bar, CursorShape::Bar),
        (WireCursorShape::HollowBlock, CursorShape::HollowBlock),
        (WireCursorShape::Hidden, CursorShape::Hidden),
    ];

    for (wire_shape, expected_shape) in variants {
        let mut snap = test_snapshot();
        snap.cursor.shape = wire_shape;
        let content = snapshot_to_renderable(&snap);
        assert_eq!(
            content.cursor.shape, expected_shape,
            "wire shape {wire_shape:?} should map to {expected_shape:?}"
        );
    }
}

#[test]
fn cursor_hidden_invisible() {
    let mut snap = test_snapshot();
    snap.cursor.visible = false;
    snap.cursor.shape = WireCursorShape::Hidden;

    let content = snapshot_to_renderable(&snap);

    assert!(!content.cursor.visible);
    assert_eq!(content.cursor.shape, CursorShape::Hidden);
}

// -- Empty snapshot --

#[test]
fn empty_snapshot_no_cells() {
    let snap = PaneSnapshot {
        cells: vec![],
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0, 0, 0]; 270],
        title: String::new(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 0,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    let content = snapshot_to_renderable(&snap);
    assert!(content.cells.is_empty());
    assert_eq!(content.cursor.line, 0);
    assert_eq!(content.cursor.column, Column(0));
}

#[test]
fn empty_snapshot_frame_input() {
    let snap = PaneSnapshot {
        cells: vec![],
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0, 0, 0]; 270],
        title: String::new(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 0,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    let viewport = ViewportSize::new(160, 320);
    let cell = test_cell_metrics();
    let frame = extract_frame_from_snapshot(&snap, viewport, cell);

    assert!(frame.content.cells.is_empty());
    assert_eq!(frame.viewport, viewport);
}

// -- Non-zero display_offset --

#[test]
fn display_offset_carried_through() {
    let mut snap = test_snapshot();
    snap.display_offset = 42;

    let content = snapshot_to_renderable(&snap);
    assert_eq!(content.display_offset, 42);
}

#[test]
fn display_offset_large_value() {
    let mut snap = test_snapshot();
    snap.display_offset = 100_000;

    let content = snapshot_to_renderable(&snap);
    assert_eq!(content.display_offset, 100_000);
}

// -- Wide char (CJK) flag preservation --

#[test]
fn wide_char_flag_preserved() {
    let snap = PaneSnapshot {
        cells: vec![vec![WireCell {
            ch: '漢',
            fg: WireRgb {
                r: 211,
                g: 215,
                b: 207,
            },
            bg: WireRgb { r: 0, g: 0, b: 0 },
            flags: CellFlags::WIDE_CHAR.bits(),
            underline_color: None,
            hyperlink_uri: None,
            zerowidth: vec![],
        }]],
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0, 0, 0]; 270],
        title: String::new(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 1,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    let content = snapshot_to_renderable(&snap);
    assert!(content.cells[0].flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(content.cells[0].ch, '漢');
}

// -- _into variants: allocation-reusing equivalence tests --

#[test]
fn renderable_into_matches_fresh() {
    let snap = test_snapshot();

    let fresh = snapshot_to_renderable(&snap);
    let mut reused = snapshot_to_renderable(&snap);
    // Mutate to prove `_into` overwrites everything.
    reused.display_offset = 999;
    reused.stable_row_base = 42;
    reused.all_dirty = false;
    reused.mode = TermMode::empty();

    snapshot_to_renderable_into(&snap, &mut reused);

    assert_eq!(fresh.cells.len(), reused.cells.len());
    for (a, b) in fresh.cells.iter().zip(reused.cells.iter()) {
        assert_eq!(a.line, b.line);
        assert_eq!(a.column, b.column);
        assert_eq!(a.ch, b.ch);
        assert_eq!(a.fg, b.fg);
        assert_eq!(a.bg, b.bg);
        assert_eq!(a.flags, b.flags);
        assert_eq!(a.underline_color, b.underline_color);
        assert_eq!(a.has_hyperlink, b.has_hyperlink);
        assert_eq!(a.zerowidth, b.zerowidth);
    }
    assert_eq!(fresh.cursor.line, reused.cursor.line);
    assert_eq!(fresh.cursor.column, reused.cursor.column);
    assert_eq!(fresh.cursor.shape, reused.cursor.shape);
    assert_eq!(fresh.cursor.visible, reused.cursor.visible);
    assert_eq!(fresh.display_offset, reused.display_offset);
    assert_eq!(fresh.stable_row_base, reused.stable_row_base);
    assert_eq!(fresh.mode, reused.mode);
    assert_eq!(fresh.all_dirty, reused.all_dirty);
    assert_eq!(fresh.damage.len(), reused.damage.len());
}

#[test]
fn extract_into_matches_fresh() {
    let snap = test_snapshot();
    let viewport = ViewportSize::new(160, 320);
    let cell = test_cell_metrics();

    let fresh = extract_frame_from_snapshot(&snap, viewport, cell);

    // Seed with a different snapshot to prove _into overwrites correctly.
    let mut reused = extract_frame_from_snapshot(&snap, ViewportSize::new(1, 1), cell);
    reused.fg_dim = 0.5;
    reused.hovered_url_segments.push((0, 0, 10));
    reused.prompt_marker_rows.push(99);

    extract_frame_from_snapshot_into(&snap, &mut reused, viewport, cell);

    assert_eq!(fresh.viewport, reused.viewport);
    assert_eq!(fresh.cell_size, reused.cell_size);
    assert_eq!(fresh.content.cells.len(), reused.content.cells.len());
    assert!(reused.selection.is_none());
    assert!(reused.search.is_none());
    assert!(reused.hovered_cell.is_none());
    assert!(reused.hovered_url_segments.is_empty());
    assert!(reused.mark_cursor.is_none());
    assert_eq!(reused.fg_dim, 1.0);
    assert!(reused.prompt_marker_rows.is_empty());
    assert_eq!(fresh.palette.foreground, reused.palette.foreground);
    assert_eq!(fresh.palette.background, reused.palette.background);
    assert_eq!(fresh.palette.cursor_color, reused.palette.cursor_color);
}

#[test]
fn extract_into_preserves_capacity() {
    let snap = test_snapshot();
    let viewport = ViewportSize::new(160, 320);
    let cell = test_cell_metrics();

    // First extraction allocates.
    let mut frame = extract_frame_from_snapshot(&snap, viewport, cell);
    let cells_cap = frame.content.cells.capacity();
    assert!(cells_cap >= 4, "should have allocated for 4 cells");

    // Second extraction into the same frame reuses allocations.
    extract_frame_from_snapshot_into(&snap, &mut frame, viewport, cell);
    assert!(
        frame.content.cells.capacity() >= cells_cap,
        "capacity should not shrink"
    );
    assert_eq!(frame.content.cells.len(), 4);
}

// -- Large snapshot through extract_frame_from_snapshot --

#[test]
fn large_snapshot_through_extract() {
    let cols = 200;
    let rows = 50;
    let white = WireRgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let black = WireRgb { r: 0, g: 0, b: 0 };

    let cells: Vec<Vec<WireCell>> = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| WireCell {
                    ch: char::from(b'A' + ((r * cols + c) % 26) as u8),
                    fg: white,
                    bg: black,
                    flags: 0,
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                })
                .collect()
        })
        .collect();

    let snap = PaneSnapshot {
        cells,
        cursor: WireCursor {
            col: 100,
            row: 25,
            shape: WireCursorShape::Underline,
            visible: true,
        },
        palette: (0..270).map(|i| [(i % 256) as u8, 0, 0]).collect(),
        title: "large".into(),
        icon_name: None,
        cwd: None,
        modes: TermMode::SHOW_CURSOR.bits(),
        scrollback_len: 10_000,
        display_offset: 50,
        stable_row_base: 9_950,
        cols: cols as u16,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    let viewport = ViewportSize::new(1600, 800);
    let cell = test_cell_metrics();
    let frame = extract_frame_from_snapshot(&snap, viewport, cell);

    assert_eq!(frame.content.cells.len(), rows * cols);
    assert_eq!(frame.content.cursor.line, 25);
    assert_eq!(frame.content.cursor.column, Column(100));
    assert_eq!(frame.content.cursor.shape, CursorShape::Underline);
    assert_eq!(frame.content.display_offset, 50);

    // Spot-check first and last cells.
    assert_eq!(frame.content.cells[0].ch, 'A');
    assert_eq!(frame.content.cells[0].line, 0);
    assert_eq!(frame.content.cells[0].column, Column(0));
    let last = &frame.content.cells[rows * cols - 1];
    assert_eq!(last.line, rows - 1);
    assert_eq!(last.column, Column(cols - 1));
}
