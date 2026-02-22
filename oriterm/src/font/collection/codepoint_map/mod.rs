//! Codepoint-to-font mapping: force specific Unicode ranges to specific fonts.
//!
//! Overrides the normal fallback chain for configured ranges. Common use cases:
//! force Nerd Font symbols to a specific PUA font, CJK to a preferred variant,
//! or emoji to a dedicated color emoji font.

use crate::font::FaceIdx;

/// A single codepoint range mapped to a font face.
struct Entry {
    /// First codepoint in the range (inclusive).
    start: u32,
    /// Last codepoint in the range (inclusive).
    end: u32,
    /// Font face to use for codepoints in this range.
    face_idx: FaceIdx,
}

/// Maps Unicode codepoint ranges to specific font faces.
///
/// Entries are sorted by range start for O(log n) binary search lookup.
/// When the font resolver checks this map and finds a match, it uses the
/// mapped face directly, skipping the normal primary + fallback chain.
///
/// If the mapped face doesn't contain the codepoint (glyph ID = 0),
/// resolution falls through to the normal chain.
pub(crate) struct CodepointMap {
    /// Sorted by `start` ascending. Binary search finds the entry whose
    /// range contains the queried codepoint.
    entries: Vec<Entry>,
}

impl CodepointMap {
    /// Create an empty codepoint map.
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Whether the map has any entries.
    #[allow(dead_code, reason = "diagnostic predicate for logging and future UI")]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add a mapping from a codepoint range to a font face.
    ///
    /// Maintains sorted order by range start. For overlapping ranges, the
    /// entry with the largest `start` that still contains the codepoint wins.
    pub(crate) fn add(&mut self, start: u32, end: u32, face_idx: FaceIdx) {
        debug_assert!(start <= end, "range start must not exceed end");
        let entry = Entry {
            start,
            end,
            face_idx,
        };
        let pos = self.entries.partition_point(|e| e.start <= start);
        self.entries.insert(pos, entry);
    }

    /// Look up a codepoint in the map.
    ///
    /// Returns the mapped [`FaceIdx`] if the codepoint falls within a
    /// configured range, or `None` to fall through to normal resolution.
    ///
    /// For overlapping ranges, the entry with the largest `start` that
    /// contains the codepoint wins. If that entry doesn't contain it,
    /// broader entries are checked.
    pub(crate) fn lookup(&self, codepoint: u32) -> Option<FaceIdx> {
        if self.entries.is_empty() {
            return None;
        }
        // Binary search: find the first entry with start > codepoint.
        let idx = self.entries.partition_point(|e| e.start <= codepoint);
        // Scan backward through all entries with start <= codepoint.
        // For non-overlapping ranges this checks at most 1 entry.
        for i in (0..idx).rev() {
            if codepoint <= self.entries[i].end {
                return Some(self.entries[i].face_idx);
            }
        }
        None
    }
}

/// Parse a hex range string into start and end codepoints.
///
/// Supports two formats:
/// - Range: `"E000-F8FF"` parses to `(0xE000, 0xF8FF)`.
/// - Single: `"E0B0"` parses to `(0xE0B0, 0xE0B0)`.
///
/// Returns `None` if parsing fails or if start exceeds end.
pub(crate) fn parse_hex_range(s: &str) -> Option<(u32, u32)> {
    if let Some((start_s, end_s)) = s.split_once('-') {
        let start = u32::from_str_radix(start_s.trim(), 16).ok()?;
        let end = u32::from_str_radix(end_s.trim(), 16).ok()?;
        if start <= end {
            Some((start, end))
        } else {
            None
        }
    } else {
        let cp = u32::from_str_radix(s.trim(), 16).ok()?;
        Some((cp, cp))
    }
}

#[cfg(test)]
mod tests;
