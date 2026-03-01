//! Emoji detection for tab icon extraction.
//!
//! Extracts the leading emoji grapheme cluster from an icon name string
//! (set by OSC 0/1). Only recognizes codepoints in the `Emoji_Presentation`
//! set — alphanumeric or symbol prefixes are not treated as icons.

use oriterm_core::is_emoji_presentation;
use unicode_segmentation::UnicodeSegmentation;

use super::widget::TabIcon;

/// Extract a leading emoji from an icon name for use as a tab icon.
///
/// Returns `Some(TabIcon::Emoji(grapheme))` when the first grapheme cluster
/// starts with an `Emoji_Presentation` codepoint. Returns `None` for plain
/// text, empty strings, or non-emoji leading characters.
pub fn extract_emoji_icon(icon_name: &str) -> Option<TabIcon> {
    let grapheme = icon_name.graphemes(true).next()?;
    let first_cp = grapheme.chars().next()?;
    if is_emoji_presentation(first_cp) {
        Some(TabIcon::Emoji(grapheme.to_owned()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
