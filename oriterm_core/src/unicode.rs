//! Unicode utilities shared across crates.
//!
//! Centralizes emoji detection so range tables stay in sync between the
//! font shaper (face selection) and UI layer (tab icon extraction).

/// Whether a codepoint has `Emoji_Presentation` — renders as emoji by default.
///
/// Covers the most common pictographic emoji ranges. Variation selectors
/// (U+FE0F) and ZWJ (U+200D) are intentionally excluded — they participate
/// in emoji *sequences* but are not emoji-presentation codepoints themselves.
/// Callers that need to match those should layer them on top.
pub fn is_emoji_presentation(cp: char) -> bool {
    matches!(cp,
        // Miscellaneous Technical (watch, hourglass, play buttons, etc.).
        '\u{2300}'..='\u{23FF}'
        // Miscellaneous Symbols (sun, cloud, stars, zodiac, etc.).
        | '\u{2600}'..='\u{27BF}'
        // Supplemental arrows / misc symbols (stars, circles).
        | '\u{2B50}'..='\u{2B55}'
        // CJK symbols and Mahjong tiles.
        | '\u{3030}' | '\u{303D}'
        // Enclosed CJK letters.
        | '\u{3297}' | '\u{3299}'
        // Enclosed alphanumeric supplement.
        | '\u{1F100}'..='\u{1F1FF}'
        // Supplementary symbols and pictographs (most emoji live here).
        | '\u{1F200}'..='\u{1FFFF}'
    )
}
