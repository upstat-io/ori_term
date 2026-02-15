//! Line-level dirty tracking for damage-based rendering.
//!
//! Tracks which visible rows have changed since the last drain. The GPU
//! renderer calls `drain()` each frame to discover dirty lines, rebuilds
//! only those lines' instance buffers, and the tracker resets to clean.

use std::ops::Range;

/// Tracks which rows have changed since last read.
///
/// Each visible line has a dirty bit. `mark_all` provides a fast path for
/// operations that invalidate everything (scroll, resize, alternate screen
/// swap). The `drain` iterator yields dirty line indices and resets the
/// tracker to clean in a single pass.
#[derive(Debug, Clone)]
pub struct DirtyTracker {
    /// One bool per visible row.
    dirty: Vec<bool>,
    /// Shortcut: everything changed (resize, scroll, alt screen swap).
    all_dirty: bool,
}

impl DirtyTracker {
    /// Create a new tracker with all lines clean.
    pub fn new(lines: usize) -> Self {
        Self {
            dirty: vec![false; lines],
            all_dirty: false,
        }
    }

    /// Mark a single line dirty.
    pub fn mark(&mut self, line: usize) {
        if let Some(b) = self.dirty.get_mut(line) {
            *b = true;
        }
    }

    /// Mark a contiguous range of lines dirty.
    ///
    /// When the range covers all lines, sets `all_dirty` instead of
    /// individual bits — avoids O(n) bit-setting and lets `collect_damage`
    /// take the fast path. Out-of-bounds indices are clamped silently.
    pub fn mark_range(&mut self, range: Range<usize>) {
        let len = self.dirty.len();
        if range.start == 0 && range.end >= len {
            self.mark_all();
        } else {
            let start = range.start.min(len);
            let end = range.end.min(len);
            for b in &mut self.dirty[start..end] {
                *b = true;
            }
        }
    }

    /// Mark everything dirty.
    pub fn mark_all(&mut self) {
        self.all_dirty = true;
    }

    /// Check whether all lines are marked dirty.
    pub fn is_all_dirty(&self) -> bool {
        self.all_dirty
    }

    /// Check whether a specific line is dirty.
    pub fn is_dirty(&self, line: usize) -> bool {
        self.all_dirty || self.dirty.get(line).copied().unwrap_or(false)
    }

    /// Check whether any line is dirty.
    pub fn is_any_dirty(&self) -> bool {
        self.all_dirty || self.dirty.iter().any(|&b| b)
    }

    /// Yield dirty line indices and reset all to clean.
    ///
    /// The returned iterator borrows the tracker mutably. Each yielded
    /// index is immediately cleared, and any un-iterated dirty lines are
    /// cleared when the iterator is dropped.
    pub fn drain(&mut self) -> DirtyIter<'_> {
        let all = self.all_dirty;
        self.all_dirty = false;
        DirtyIter {
            dirty: &mut self.dirty,
            pos: 0,
            all,
        }
    }

    /// Resize the tracker to a new line count, marking all dirty.
    pub fn resize(&mut self, lines: usize) {
        self.dirty.resize(lines, false);
        self.mark_all();
    }
}

/// Iterator over dirty line indices produced by [`DirtyTracker::drain`].
///
/// Clears each dirty bit as it yields the index. When dropped, clears
/// any remaining dirty bits that were not iterated.
pub struct DirtyIter<'a> {
    dirty: &'a mut [bool],
    pos: usize,
    all: bool,
}

impl Iterator for DirtyIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        while self.pos < self.dirty.len() {
            let line = self.pos;
            self.pos += 1;
            if self.all || self.dirty[line] {
                self.dirty[line] = false;
                return Some(line);
            }
        }
        None
    }
}

impl Drop for DirtyIter<'_> {
    fn drop(&mut self) {
        // Clear any remaining dirty entries that were not iterated.
        self.dirty[self.pos..].fill(false);
    }
}

#[cfg(test)]
mod tests;
