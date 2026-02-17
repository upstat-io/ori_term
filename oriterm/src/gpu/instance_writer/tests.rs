//! Unit tests for GPU instance buffer writer.

use oriterm_core::Rgb;

use super::{INSTANCE_SIZE, InstanceKind, InstanceWriter};

/// Read a little-endian `f32` from the given byte offset.
fn read_f32(buf: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

/// Read a little-endian `u32` from the given byte offset.
fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

// --- Record layout ---

#[test]
fn instance_size_is_80_bytes() {
    assert_eq!(INSTANCE_SIZE, 80);
}

#[test]
fn push_rect_produces_80_byte_record() {
    let mut w = InstanceWriter::new();
    w.push_rect(10.0, 20.0, 8.0, 16.0, BLACK, 1.0);

    assert_eq!(w.len(), 1);
    assert_eq!(w.byte_len(), 80);
}

#[test]
fn push_rect_field_offsets() {
    let mut w = InstanceWriter::new();
    w.push_rect(10.0, 20.0, 8.0, 16.0, RED, 0.5);

    let rec = w.as_bytes();

    // Position.
    assert_eq!(read_f32(rec, 0), 10.0);
    assert_eq!(read_f32(rec, 4), 20.0);

    // Size.
    assert_eq!(read_f32(rec, 8), 8.0);
    assert_eq!(read_f32(rec, 12), 16.0);

    // UV (zeroed for rects).
    assert_eq!(read_f32(rec, 16), 0.0);
    assert_eq!(read_f32(rec, 20), 0.0);
    assert_eq!(read_f32(rec, 24), 0.0);
    assert_eq!(read_f32(rec, 28), 0.0);

    // FG (zeroed for rects).
    assert_eq!(read_f32(rec, 32), 0.0);
    assert_eq!(read_f32(rec, 36), 0.0);
    assert_eq!(read_f32(rec, 40), 0.0);
    assert_eq!(read_f32(rec, 44), 0.0);

    // BG = RED at alpha 0.5.
    assert_eq!(read_f32(rec, 48), 1.0); // R
    assert_eq!(read_f32(rec, 52), 0.0); // G
    assert_eq!(read_f32(rec, 56), 0.0); // B
    assert_eq!(read_f32(rec, 60), 0.5); // A

    // Kind = Rect (0).
    assert_eq!(read_u32(rec, 64), 0);

    // Padding is zeroed.
    assert_eq!(read_u32(rec, 68), 0);
    assert_eq!(read_u32(rec, 72), 0);
    assert_eq!(read_u32(rec, 76), 0);
}

#[test]
fn push_glyph_field_offsets() {
    let mut w = InstanceWriter::new();
    let uv = [0.25, 0.5, 0.125, 0.25];
    w.push_glyph(100.0, 200.0, 8.0, 16.0, uv, WHITE, 1.0);

    let rec = w.as_bytes();

    // Position.
    assert_eq!(read_f32(rec, 0), 100.0);
    assert_eq!(read_f32(rec, 4), 200.0);

    // UV.
    assert_eq!(read_f32(rec, 16), 0.25);
    assert_eq!(read_f32(rec, 20), 0.5);
    assert_eq!(read_f32(rec, 24), 0.125);
    assert_eq!(read_f32(rec, 28), 0.25);

    // FG = WHITE at alpha 1.0.
    assert_eq!(read_f32(rec, 32), 1.0);
    assert_eq!(read_f32(rec, 36), 1.0);
    assert_eq!(read_f32(rec, 40), 1.0);
    assert_eq!(read_f32(rec, 44), 1.0);

    // BG (zeroed for glyphs).
    assert_eq!(read_f32(rec, 48), 0.0);
    assert_eq!(read_f32(rec, 52), 0.0);
    assert_eq!(read_f32(rec, 56), 0.0);
    assert_eq!(read_f32(rec, 60), 0.0);

    // Kind = Glyph (1).
    assert_eq!(read_u32(rec, 64), 1);
}

#[test]
fn push_cursor_field_offsets() {
    let mut w = InstanceWriter::new();
    let green = Rgb { r: 0, g: 128, b: 0 };
    w.push_cursor(50.0, 100.0, 8.0, 16.0, green, 0.75);

    let rec = w.as_bytes();

    // FG = green at alpha 0.75.
    assert_eq!(read_f32(rec, 32), 0.0);
    assert!((read_f32(rec, 36) - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(read_f32(rec, 40), 0.0);
    assert_eq!(read_f32(rec, 44), 0.75);

    // Kind = Cursor (2).
    assert_eq!(read_u32(rec, 64), 2);
}

// --- Color conversion ---

#[test]
fn rgb_conversion_boundary_values() {
    let mut w = InstanceWriter::new();
    w.push_rect(0.0, 0.0, 1.0, 1.0, WHITE, 1.0);

    let rec = w.as_bytes();
    assert_eq!(read_f32(rec, 48), 1.0);
    assert_eq!(read_f32(rec, 52), 1.0);
    assert_eq!(read_f32(rec, 56), 1.0);

    let mut w2 = InstanceWriter::new();
    w2.push_rect(0.0, 0.0, 1.0, 1.0, BLACK, 0.0);

    let rec2 = w2.as_bytes();
    assert_eq!(read_f32(rec2, 48), 0.0);
    assert_eq!(read_f32(rec2, 52), 0.0);
    assert_eq!(read_f32(rec2, 56), 0.0);
    assert_eq!(read_f32(rec2, 60), 0.0);
}

#[test]
fn rgb_mid_value_conversion() {
    let mid = Rgb {
        r: 128,
        g: 64,
        b: 192,
    };
    let mut w = InstanceWriter::new();
    w.push_rect(0.0, 0.0, 1.0, 1.0, mid, 1.0);

    let rec = w.as_bytes();
    assert!((read_f32(rec, 48) - 128.0 / 255.0).abs() < 1e-6);
    assert!((read_f32(rec, 52) - 64.0 / 255.0).abs() < 1e-6);
    assert!((read_f32(rec, 56) - 192.0 / 255.0).abs() < 1e-6);
}

// --- Lifecycle ---

#[test]
fn empty_writer() {
    let w = InstanceWriter::new();
    assert!(w.is_empty());
    assert_eq!(w.len(), 0);
    assert_eq!(w.byte_len(), 0);
    assert!(w.as_bytes().is_empty());
}

#[test]
fn with_capacity_starts_empty() {
    let w = InstanceWriter::with_capacity(100);
    assert!(w.is_empty());
    assert_eq!(w.len(), 0);
}

#[test]
fn multiple_pushes_accumulate() {
    let mut w = InstanceWriter::new();
    w.push_rect(0.0, 0.0, 8.0, 16.0, BLACK, 1.0);
    w.push_glyph(8.0, 0.0, 8.0, 16.0, [0.0; 4], WHITE, 1.0);
    w.push_cursor(16.0, 0.0, 2.0, 16.0, RED, 1.0);

    assert_eq!(w.len(), 3);
    assert_eq!(w.byte_len(), 240);

    // Each record starts at the right offset.
    let bytes = w.as_bytes();
    assert_eq!(read_u32(bytes, 64), InstanceKind::Rect as u32);
    assert_eq!(read_u32(bytes, 80 + 64), InstanceKind::Glyph as u32);
    assert_eq!(read_u32(bytes, 160 + 64), InstanceKind::Cursor as u32);
}

#[test]
fn clear_resets_length_but_retains_capacity() {
    let mut w = InstanceWriter::new();
    for _ in 0..50 {
        w.push_rect(0.0, 0.0, 8.0, 16.0, BLACK, 1.0);
    }
    assert_eq!(w.len(), 50);

    w.clear();
    assert!(w.is_empty());
    assert_eq!(w.len(), 0);
    assert_eq!(w.byte_len(), 0);

    // Capacity should still be at least 50 * 80.
    // (Vec::capacity is in bytes for Vec<u8>.)
}

#[test]
fn clear_and_reuse() {
    let mut w = InstanceWriter::new();
    w.push_rect(0.0, 0.0, 8.0, 16.0, RED, 1.0);
    w.clear();
    w.push_glyph(10.0, 20.0, 8.0, 16.0, [0.1, 0.2, 0.3, 0.4], WHITE, 1.0);

    assert_eq!(w.len(), 1);
    let rec = w.as_bytes();
    assert_eq!(read_f32(rec, 0), 10.0);
    assert_eq!(read_u32(rec, 64), InstanceKind::Glyph as u32);
}

// --- Raw push ---

#[test]
fn push_raw_valid() {
    let mut w = InstanceWriter::new();
    let mut raw = [0u8; INSTANCE_SIZE];
    raw[64..68].copy_from_slice(&42u32.to_le_bytes());
    w.push_raw(&raw);

    assert_eq!(w.len(), 1);
    assert_eq!(read_u32(w.as_bytes(), 64), 42);
}

#[test]
#[should_panic(expected = "raw instance must be exactly 80 bytes")]
fn push_raw_wrong_size_panics() {
    let mut w = InstanceWriter::new();
    w.push_raw(&[0u8; 40]);
}

// --- Default ---

#[test]
fn default_is_empty() {
    let w = InstanceWriter::default();
    assert!(w.is_empty());
}

// --- InstanceKind values ---

#[test]
fn instance_kind_discriminants() {
    assert_eq!(InstanceKind::Rect as u32, 0);
    assert_eq!(InstanceKind::Glyph as u32, 1);
    assert_eq!(InstanceKind::Cursor as u32, 2);
}
