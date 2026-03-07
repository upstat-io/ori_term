//! Sixel color utilities: HLS-to-RGB conversion and the default VT340 palette.

/// Default VT340 palette (16 colors).
pub(super) const VT340_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],       // 0: black
    [51, 51, 204],   // 1: blue
    [204, 33, 33],   // 2: red
    [51, 204, 51],   // 3: green
    [204, 51, 204],  // 4: magenta
    [51, 204, 204],  // 5: cyan
    [204, 204, 51],  // 6: yellow
    [135, 135, 135], // 7: grey 50%
    [51, 51, 51],    // 8: grey 25%
    [68, 68, 255],   // 9: bright blue
    [255, 68, 68],   // 10: bright red
    [68, 255, 68],   // 11: bright green
    [255, 68, 255],  // 12: bright magenta
    [68, 255, 255],  // 13: bright cyan
    [255, 255, 68],  // 14: bright yellow
    [255, 255, 255], // 15: white
];

/// Convert HLS (Hue, Lightness, Saturation) to RGB.
///
/// Sixel uses HLS with a 120° hue rotation from standard HSL:
/// - Sixel: Blue=0°, Red=120°, Green=240°
/// - Standard: Red=0°, Green=120°, Blue=240°
///
/// H in 0-360, L in 0-100, S in 0-100.
pub(super) fn hls_to_rgb(hue: u32, lightness: u32, saturation: u32) -> [u8; 3] {
    let lf = lightness as f64 / 100.0;
    let sf = saturation as f64 / 100.0;

    if sf == 0.0 {
        // Achromatic.
        let val = (lf * 255.0) as u8;
        return [val, val, val];
    }

    // Rotate sixel hue to standard HSL: subtract 120°.
    let hf = hue as f64 - 120.0;
    let hf = if hf < 0.0 { hf + 360.0 } else { hf };
    let hf = hf / 360.0;

    let upper = if lf < 0.5 {
        lf * (1.0 + sf)
    } else {
        lf + sf - lf * sf
    };
    let lower = 2.0 * lf - upper;

    let red = hue_to_channel(lower, upper, hf + 1.0 / 3.0);
    let green = hue_to_channel(lower, upper, hf);
    let blue = hue_to_channel(lower, upper, hf - 1.0 / 3.0);

    [
        (red * 255.0) as u8,
        (green * 255.0) as u8,
        (blue * 255.0) as u8,
    ]
}

/// Convert a hue value to an RGB channel (standard HSL algorithm).
fn hue_to_channel(lower: f64, upper: f64, mut frac: f64) -> f64 {
    if frac < 0.0 {
        frac += 1.0;
    }
    if frac > 1.0 {
        frac -= 1.0;
    }
    if frac < 1.0 / 6.0 {
        return lower + (upper - lower) * 6.0 * frac;
    }
    if frac < 1.0 / 2.0 {
        return upper;
    }
    if frac < 2.0 / 3.0 {
        return lower + (upper - lower) * (2.0 / 3.0 - frac) * 6.0;
    }
    lower
}
