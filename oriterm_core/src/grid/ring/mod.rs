//! Scrollback ring buffer.
//!
//! Rows that scroll off the top of the visible grid are stored here.
//! The buffer grows incrementally up to `max_scrollback`, then overwrites
//! the oldest entry on each push (classic ring buffer).

use std::mem;

use super::row::Row;

/// Default maximum scrollback lines.
pub const DEFAULT_MAX_SCROLLBACK: usize = 10_000;

/// Ring buffer for scrollback history.
///
/// Index 0 is the most recently pushed row (newest), and `len - 1` is
/// the oldest. The buffer grows on demand up to `max_scrollback`; once
/// full, each `push` evicts the oldest row in O(1).
#[derive(Debug, Clone)]
pub struct ScrollbackBuffer {
    /// Storage, grows up to `max_scrollback`.
    inner: Vec<Row>,
    /// Maximum number of rows to retain.
    max_scrollback: usize,
    /// Number of valid rows (always `<= inner.len()`).
    len: usize,
    /// Index of the oldest row when the buffer is full.
    start: usize,
}

impl ScrollbackBuffer {
    /// Create a new scrollback buffer with the given capacity limit.
    pub fn new(max_scrollback: usize) -> Self {
        Self {
            inner: Vec::new(),
            max_scrollback,
            len: 0,
            start: 0,
        }
    }

    /// Add a row to scrollback, returning the evicted row if full.
    ///
    /// During the growth phase (`len < max_scrollback`), returns `None`.
    /// Once full, the oldest row is evicted and returned so the caller
    /// can recycle its allocation. When `max_scrollback == 0`, the
    /// pushed row is returned immediately (no storage).
    pub(super) fn push(&mut self, row: Row) -> Option<Row> {
        if self.max_scrollback == 0 {
            return Some(row);
        }

        if self.inner.len() < self.max_scrollback {
            // Growing phase: just append.
            self.inner.push(row);
            self.len = self.inner.len();
            None
        } else {
            // Full: swap in the new row, return the evicted oldest.
            let evicted = mem::replace(&mut self.inner[self.start], row);
            self.start = (self.start + 1) % self.max_scrollback;
            Some(evicted)
        }
    }

    /// Number of rows currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum number of rows this buffer will retain.
    pub fn max_scrollback(&self) -> usize {
        self.max_scrollback
    }

    /// Retrieve a row by logical index (0 = most recent, `len - 1` = oldest).
    ///
    /// Returns `None` if `index >= len`.
    pub fn get(&self, index: usize) -> Option<&Row> {
        if index >= self.len {
            return None;
        }
        Some(&self.inner[self.physical_index(index)])
    }

    /// Iterate from newest to oldest.
    pub fn iter(&self) -> impl Iterator<Item = &Row> + '_ {
        (0..self.len).map(move |i| &self.inner[self.physical_index(i)])
    }

    /// Clear all stored rows without deallocating.
    pub fn clear(&mut self) {
        self.inner.clear();
        self.len = 0;
        self.start = 0;
    }

    /// Translate a logical index (0 = newest) to a physical Vec index.
    fn physical_index(&self, logical: usize) -> usize {
        debug_assert!(logical < self.len, "logical {logical} >= len {}", self.len);
        let cap = self.inner.len();
        // newest is at (start + len - 1) % cap, so offset backwards by `logical`.
        (self.start + self.len - 1 - logical) % cap
    }
}

#[cfg(test)]
mod tests;
