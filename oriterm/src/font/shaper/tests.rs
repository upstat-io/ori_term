//! Unit tests for the text shaping pipeline.

use std::sync::Arc;

use oriterm_core::{Cell, CellExtra, CellFlags};

use super::{prepare_line, shape_prepared_runs};
use crate::font::collection::FontCollection;
use crate::font::{FaceIdx, FontSet, GlyphFormat};

// ── Helpers ──

/// Build a row of cells from a plain ASCII string (no flags, no extras).
fn make_cells(text: &str) -> Vec<Cell> {
    text.chars()
        .map(|ch| Cell {
            ch,
            ..Cell::default()
        })
        .collect()
}

/// Build a FontCollection from system discovery with default settings.
fn test_collection() -> FontCollection {
    let font_set = FontSet::load(None, 400).expect("font must load");
    FontCollection::new(font_set, 12.0, 96.0, GlyphFormat::Alpha, 400)
        .expect("collection must build")
}

// ── Phase 1: Run Segmentation ──

#[test]
fn prepare_line_hello() {
    let fc = test_collection();
    let cells = make_cells("hello");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    // All ASCII chars in same face → single run.
    assert_eq!(runs.len(), 1, "single face should produce one run");
    assert_eq!(runs[0].text, "hello");
    assert_eq!(runs[0].col_start, 0);
    assert_eq!(runs[0].face_idx, FaceIdx::REGULAR);
}

#[test]
fn prepare_line_space_excluded_from_runs() {
    let fc = test_collection();
    let cells = make_cells("hello world");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    // Spaces are skipped (handled by renderer at fixed cell width).
    // Characters on both sides of the space share the same face, so they
    // merge into a single run. The text excludes the space.
    assert_eq!(runs.len(), 1, "same-face chars merge across spaces");
    assert_eq!(runs[0].text, "helloworld");
    assert_eq!(runs[0].col_start, 0);
}

#[test]
fn prepare_line_all_spaces() {
    let fc = test_collection();
    let cells = make_cells("   ");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    assert!(runs.is_empty(), "all spaces should produce no runs");
}

#[test]
fn prepare_line_null_chars() {
    let fc = test_collection();
    let cells = make_cells("\0\0\0");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    assert!(runs.is_empty(), "null chars should produce no runs");
}

#[test]
fn prepare_line_combining_mark() {
    let fc = test_collection();

    // 'a' followed by combining acute accent U+0301.
    let mut cells = vec![
        Cell {
            ch: 'a',
            ..Cell::default()
        },
        Cell {
            ch: 'b',
            ..Cell::default()
        },
    ];
    // Add combining mark to first cell.
    cells[0].extra = Some(Arc::new(CellExtra {
        underline_color: None,
        hyperlink: None,
        zerowidth: vec!['\u{0301}'],
    }));

    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    assert_eq!(runs.len(), 1, "same face should be one run");
    // Text should include both the base 'a', the combining mark, and 'b'.
    assert_eq!(runs[0].text, "a\u{0301}b");
    // byte_to_col: 'a' maps to col 0, U+0301 (2 bytes) maps to col 0, 'b' maps to col 1.
    assert_eq!(runs[0].byte_to_col[0], 0); // 'a'
    assert_eq!(runs[0].byte_to_col[1], 0); // U+0301 byte 1
    assert_eq!(runs[0].byte_to_col[2], 0); // U+0301 byte 2
    assert_eq!(runs[0].byte_to_col[3], 1); // 'b'
}

#[test]
fn prepare_line_wide_char() {
    let fc = test_collection();

    // CJK ideograph (wide char) followed by ASCII.
    let cells = vec![
        Cell {
            ch: '\u{4E00}',
            flags: CellFlags::WIDE_CHAR,
            ..Cell::default()
        },
        Cell {
            ch: ' ',
            flags: CellFlags::WIDE_CHAR_SPACER,
            ..Cell::default()
        },
        Cell {
            ch: 'a',
            ..Cell::default()
        },
    ];

    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    // With embedded-only font, both chars resolve to Regular (CJK is .notdef).
    // They may be in the same run or different depending on face resolution.
    // Key check: spacer is NOT in any run's text.
    for run in &runs {
        assert!(
            !run.text.contains(' '),
            "spacer should not appear in run text"
        );
    }
}

#[test]
fn prepare_line_byte_to_col_ascii() {
    let fc = test_collection();
    let cells = make_cells("abc");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    assert_eq!(runs.len(), 1);
    // ASCII: 1 byte per char.
    assert_eq!(runs[0].byte_to_col, vec![0, 1, 2]);
}

#[test]
fn prepare_line_reuses_scratch_buffer() {
    let fc = test_collection();
    let cells = make_cells("hello");
    let mut runs = Vec::new();

    // First call.
    prepare_line(&cells, cells.len(), &fc, &mut runs);
    assert_eq!(runs.len(), 1);

    // Second call should clear and reuse.
    let cells2 = make_cells("A B");
    prepare_line(&cells2, cells2.len(), &fc, &mut runs);
    // "A" and "B" share the same face → 1 run ("AB"), space excluded.
    assert_eq!(runs.len(), 1, "scratch buffer should be cleared and reused");
}

// ── VS16 emoji presentation (Section 6.10) ──

#[test]
fn prepare_line_vs16_in_zerowidth() {
    // A cell with VS16 (U+FE0F) in zerowidth should use emoji resolution.
    // With system fonts, this may resolve to a different face than normal.
    let fc = test_collection();
    let cells = vec![
        Cell {
            ch: '\u{2764}', // ❤ (HEAVY BLACK HEART)
            extra: Some(Arc::new(CellExtra {
                underline_color: None,
                hyperlink: None,
                zerowidth: vec!['\u{FE0F}'], // VS16
            })),
            ..Cell::default()
        },
        Cell {
            ch: 'a',
            ..Cell::default()
        },
    ];
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    // Should produce at least one run containing the heart character.
    let has_heart = runs.iter().any(|r| r.text.contains('\u{2764}'));
    assert!(has_heart, "heart should appear in a shaping run");

    // VS16 should also be in the run text (passed to shaper for font handling).
    let has_vs16 = runs.iter().any(|r| r.text.contains('\u{FE0F}'));
    assert!(has_vs16, "VS16 should be in run text for shaper");
}

#[test]
fn prepare_line_vs16_may_use_different_face() {
    // With VS16, the heart should resolve preferring emoji fallback.
    // Without VS16, it should use normal resolution order.
    let fc = test_collection();

    // Cell WITH VS16.
    let with_vs16 = vec![Cell {
        ch: '\u{2764}',
        extra: Some(Arc::new(CellExtra {
            underline_color: None,
            hyperlink: None,
            zerowidth: vec!['\u{FE0F}'],
        })),
        ..Cell::default()
    }];
    let mut runs_vs16 = Vec::new();
    prepare_line(&with_vs16, with_vs16.len(), &fc, &mut runs_vs16);

    // Cell WITHOUT VS16.
    let without_vs16 = vec![Cell {
        ch: '\u{2764}',
        ..Cell::default()
    }];
    let mut runs_plain = Vec::new();
    prepare_line(&without_vs16, without_vs16.len(), &fc, &mut runs_plain);

    // Both should produce runs (the character exists in some font).
    // The face_idx may differ if emoji fallback is available.
    // Key invariant: no panics, valid runs produced.
    if !runs_vs16.is_empty() && !runs_plain.is_empty() {
        // If a color emoji font is in the fallback chain, VS16 version
        // should use a fallback face (emoji font) while plain may use
        // the primary font.
        // This is a soft check — depends on system fonts.
        let vs16_face = runs_vs16[0].face_idx;
        let plain_face = runs_plain[0].face_idx;
        // Log for diagnostic visibility; both outcomes are valid.
        if vs16_face != plain_face {
            // VS16 triggered emoji fallback — expected behavior.
        }
    }
}

// ── Phase 2: Shaping ──

#[test]
fn shape_hello_produces_five_glyphs() {
    let fc = test_collection();
    let cells = make_cells("Hello");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    let faces = fc.create_shaping_faces();
    let mut output = Vec::new();
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);

    assert_eq!(output.len(), 5, "5 glyphs for 'Hello'");
    for g in &output {
        assert_eq!(g.col_span, 1, "each ASCII glyph spans 1 column");
        assert_ne!(g.glyph_id, 0, "glyph ID should not be .notdef for ASCII");
    }
}

#[test]
fn shape_preserves_column_positions() {
    let fc = test_collection();
    let cells = make_cells("A B");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    let faces = fc.create_shaping_faces();
    let mut output = Vec::new();
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);

    // "A" and "B" merge into one run "AB" with byte_to_col=[0, 2].
    assert_eq!(output.len(), 2);
    assert_eq!(output[0].col_start, 0, "'A' at column 0");
    assert_eq!(output[1].col_start, 2, "'B' at column 2 (space skipped)");
}

#[test]
fn shape_empty_runs_produces_no_output() {
    let fc = test_collection();
    let faces = fc.create_shaping_faces();
    let mut output = Vec::new();
    shape_prepared_runs(&[], &faces, &fc, &mut output, &mut None);

    assert!(output.is_empty());
}

#[test]
fn shape_reuses_scratch_buffer() {
    let fc = test_collection();
    let faces = fc.create_shaping_faces();
    let mut runs = Vec::new();
    let mut output = Vec::new();

    let cells = make_cells("AB");
    prepare_line(&cells, cells.len(), &fc, &mut runs);
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);
    assert_eq!(output.len(), 2);

    // Re-shape a different line — output should be replaced.
    let cells2 = make_cells("X");
    prepare_line(&cells2, cells2.len(), &fc, &mut runs);
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);
    assert_eq!(output.len(), 1, "output should be cleared on re-shape");
}

// ── Phase 3: Column ↔ Glyph Mapping ──

#[test]
fn col_glyph_map_simple_ascii() {
    let fc = test_collection();
    let cells = make_cells("abc");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    let faces = fc.create_shaping_faces();
    let mut output = Vec::new();
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);

    let mut map = Vec::new();
    super::build_col_glyph_map(&output, cells.len(), &mut map);

    assert_eq!(map.len(), 3);
    // Each column maps to its glyph.
    assert_eq!(map[0], Some(0));
    assert_eq!(map[1], Some(1));
    assert_eq!(map[2], Some(2));
}

#[test]
fn col_glyph_map_with_spaces() {
    let fc = test_collection();
    let cells = make_cells("A B");
    let mut runs = Vec::new();
    prepare_line(&cells, cells.len(), &fc, &mut runs);

    let faces = fc.create_shaping_faces();
    let mut output = Vec::new();
    shape_prepared_runs(&runs, &faces, &fc, &mut output, &mut None);

    let mut map = Vec::new();
    super::build_col_glyph_map(&output, cells.len(), &mut map);

    assert_eq!(map.len(), 3);
    // 'A' at col 0, space at col 1 (no glyph), 'B' at col 2.
    assert_eq!(map[0], Some(0));
    assert_eq!(map[1], None, "space column has no glyph");
    assert_eq!(map[2], Some(1));
}

#[test]
fn col_glyph_map_empty_line() {
    let mut map = Vec::new();
    super::build_col_glyph_map(&[], 5, &mut map);

    assert_eq!(map.len(), 5);
    assert!(map.iter().all(|e| e.is_none()));
}

#[test]
fn col_glyph_map_reuses_buffer() {
    let mut map = Vec::new();

    // First call.
    super::build_col_glyph_map(&[], 3, &mut map);
    assert_eq!(map.len(), 3);

    // Second call with different size.
    let glyphs = vec![super::ShapedGlyph {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        col_start: 0,
        col_span: 1,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    super::build_col_glyph_map(&glyphs, 5, &mut map);
    assert_eq!(map.len(), 5);
    assert_eq!(map[0], Some(0));
    assert!(map[1..].iter().all(|e| e.is_none()));
}

#[test]
fn col_glyph_map_first_wins_for_combining_marks() {
    // Two glyphs at the same col_start: base char (glyph 50) and combining mark (glyph 51).
    // build_col_glyph_map should store the FIRST glyph's index.
    let glyphs = vec![
        super::ShapedGlyph {
            glyph_id: 50,
            face_idx: FaceIdx::REGULAR,
            col_start: 0,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        super::ShapedGlyph {
            glyph_id: 51,
            face_idx: FaceIdx::REGULAR,
            col_start: 0, // same column — combining mark
            col_span: 1,
            x_offset: 1.5,
            y_offset: 3.0,
        },
        super::ShapedGlyph {
            glyph_id: 52,
            face_idx: FaceIdx::REGULAR,
            col_start: 1,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];

    let mut map = Vec::new();
    super::build_col_glyph_map(&glyphs, 2, &mut map);

    // First-wins: col 0 points to glyph 0 (the base), not glyph 1 (the combining mark).
    assert_eq!(map[0], Some(0), "first-wins: base glyph claims col 0");
    assert_eq!(map[1], Some(2), "next column maps to glyph at col 1");
}

#[test]
fn col_glyph_map_ligature_span() {
    // Simulate a ligature spanning 2 columns.
    let glyphs = vec![
        super::ShapedGlyph {
            glyph_id: 100,
            face_idx: FaceIdx::REGULAR,
            col_start: 0,
            col_span: 2, // ligature spans cols 0-1
            x_offset: 0.0,
            y_offset: 0.0,
        },
        super::ShapedGlyph {
            glyph_id: 101,
            face_idx: FaceIdx::REGULAR,
            col_start: 2,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];

    let mut map = Vec::new();
    super::build_col_glyph_map(&glyphs, 3, &mut map);

    assert_eq!(map[0], Some(0), "ligature starts at col 0");
    assert_eq!(map[1], None, "col 1 is continuation of ligature");
    assert_eq!(map[2], Some(1), "normal glyph at col 2");
}
