//! Unit tests for the Color type.

use super::Color;

// --- Constants ---

#[test]
fn transparent_is_zero() {
    let c = Color::TRANSPARENT;
    assert_eq!(c.r, 0.0);
    assert_eq!(c.g, 0.0);
    assert_eq!(c.b, 0.0);
    assert_eq!(c.a, 0.0);
}

#[test]
fn white_is_opaque_one() {
    let c = Color::WHITE;
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 1.0);
    assert_eq!(c.b, 1.0);
    assert_eq!(c.a, 1.0);
}

#[test]
fn black_is_opaque_zero() {
    let c = Color::BLACK;
    assert_eq!(c.r, 0.0);
    assert_eq!(c.g, 0.0);
    assert_eq!(c.b, 0.0);
    assert_eq!(c.a, 1.0);
}

// --- Default ---

#[test]
fn default_is_transparent() {
    assert_eq!(Color::default(), Color::TRANSPARENT);
}

// --- Constructors ---

#[test]
fn rgba_constructor() {
    let c = Color::rgba(0.1, 0.2, 0.3, 0.4);
    assert_eq!(c.r, 0.1);
    assert_eq!(c.g, 0.2);
    assert_eq!(c.b, 0.3);
    assert_eq!(c.a, 0.4);
}

#[test]
fn rgb_constructor_is_opaque() {
    let c = Color::rgb(0.5, 0.6, 0.7);
    assert_eq!(c.r, 0.5);
    assert_eq!(c.g, 0.6);
    assert_eq!(c.b, 0.7);
    assert_eq!(c.a, 1.0);
}

#[test]
fn hex_constructor() {
    let c = Color::hex(0xFF8000);
    assert_eq!(c.r, 1.0);
    assert!((c.g - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(c.b, 0.0);
    assert_eq!(c.a, 1.0);
}

#[test]
fn hex_constructor_black() {
    let c = Color::hex(0x000000);
    assert_eq!(c.r, 0.0);
    assert_eq!(c.g, 0.0);
    assert_eq!(c.b, 0.0);
    assert_eq!(c.a, 1.0);
}

#[test]
fn hex_constructor_white() {
    let c = Color::hex(0xFFFFFF);
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 1.0);
    assert_eq!(c.b, 1.0);
    assert_eq!(c.a, 1.0);
}

#[test]
fn hex_alpha_constructor() {
    let c = Color::hex_alpha(0xFF000080);
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 0.0);
    assert_eq!(c.b, 0.0);
    assert!((c.a - 128.0 / 255.0).abs() < 1e-6);
}

#[test]
fn hex_alpha_fully_transparent() {
    let c = Color::hex_alpha(0xFFFFFF00);
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 1.0);
    assert_eq!(c.b, 1.0);
    assert_eq!(c.a, 0.0);
}

#[test]
fn from_rgb_u8_boundary_values() {
    let black = Color::from_rgb_u8(0, 0, 0);
    assert_eq!(black.r, 0.0);
    assert_eq!(black.g, 0.0);
    assert_eq!(black.b, 0.0);
    assert_eq!(black.a, 1.0);

    let white = Color::from_rgb_u8(255, 255, 255);
    assert_eq!(white.r, 1.0);
    assert_eq!(white.g, 1.0);
    assert_eq!(white.b, 1.0);
    assert_eq!(white.a, 1.0);
}

#[test]
fn from_rgb_u8_mid_values() {
    let c = Color::from_rgb_u8(128, 64, 192);
    assert!((c.r - 128.0 / 255.0).abs() < 1e-6);
    assert!((c.g - 64.0 / 255.0).abs() < 1e-6);
    assert!((c.b - 192.0 / 255.0).abs() < 1e-6);
}

// --- Methods ---

#[test]
fn with_alpha_preserves_rgb() {
    let c = Color::rgb(0.5, 0.6, 0.7).with_alpha(0.3);
    assert_eq!(c.r, 0.5);
    assert_eq!(c.g, 0.6);
    assert_eq!(c.b, 0.7);
    assert_eq!(c.a, 0.3);
}

#[test]
fn with_alpha_zero_makes_transparent() {
    let c = Color::WHITE.with_alpha(0.0);
    assert_eq!(c.r, 1.0);
    assert_eq!(c.a, 0.0);
}

#[test]
fn to_array_matches_fields() {
    let c = Color::rgba(0.1, 0.2, 0.3, 0.4);
    assert_eq!(c.to_array(), [0.1, 0.2, 0.3, 0.4]);
}

#[test]
fn to_array_transparent() {
    assert_eq!(Color::TRANSPARENT.to_array(), [0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn to_array_white() {
    assert_eq!(Color::WHITE.to_array(), [1.0, 1.0, 1.0, 1.0]);
}

// --- Equality ---

#[test]
fn equality_same_values() {
    let a = Color::rgba(0.1, 0.2, 0.3, 0.4);
    let b = Color::rgba(0.1, 0.2, 0.3, 0.4);
    assert_eq!(a, b);
}

#[test]
fn inequality_different_alpha() {
    let a = Color::rgb(0.5, 0.5, 0.5);
    let b = Color::rgba(0.5, 0.5, 0.5, 0.9);
    assert_ne!(a, b);
}

// --- Copy semantics ---

#[test]
fn copy_semantics() {
    let a = Color::rgb(0.5, 0.6, 0.7);
    let b = a;
    assert_eq!(a, b);
}

// --- Lerp ---

#[test]
fn lerp_black_to_white_at_zero() {
    use crate::animation::Lerp;
    let result = Color::lerp(Color::BLACK, Color::WHITE, 0.0);
    assert_eq!(result, Color::BLACK);
}

#[test]
fn lerp_black_to_white_at_one() {
    use crate::animation::Lerp;
    let result = Color::lerp(Color::BLACK, Color::WHITE, 1.0);
    assert_eq!(result, Color::WHITE);
}

#[test]
fn lerp_black_to_white_midpoint() {
    use crate::animation::Lerp;
    let result = Color::lerp(Color::BLACK, Color::WHITE, 0.5);
    assert!((result.r - 0.5).abs() < 1e-6);
    assert!((result.g - 0.5).abs() < 1e-6);
    assert!((result.b - 0.5).abs() < 1e-6);
    assert_eq!(result.a, 1.0); // Both have a=1.0.
}

#[test]
fn lerp_alpha_interpolation() {
    use crate::animation::Lerp;
    let transparent = Color::BLACK.with_alpha(0.0);
    let opaque = Color::BLACK.with_alpha(1.0);
    let result = Color::lerp(transparent, opaque, 0.5);
    assert!((result.a - 0.5).abs() < 1e-6);
}
