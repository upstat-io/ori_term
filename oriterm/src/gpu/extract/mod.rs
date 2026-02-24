//! Extract phase: snapshot terminal state into owned frame data.
//!
//! Locks the terminal via [`FairMutex`], copies visible cells, cursor,
//! palette, and damage information into a [`FrameInput`], then releases
//! the lock immediately. The returned data is fully owned — no references
//! back to the terminal. Total lock hold time: microseconds.

use std::time::Instant;

use oriterm_core::{EventListener, FairMutex, Term};

use super::frame_input::{FrameInput, FramePalette, ViewportSize};
use crate::font::CellMetrics;

/// Snapshot terminal state into a [`FrameInput`] for the Prepare phase.
///
/// Acquires a fair lock on the terminal, extracts all rendering data, and
/// releases the lock before returning. The returned `FrameInput` is fully
/// owned — the terminal lock is never touched again during this frame.
///
/// # Lock discipline
///
/// The lock is held only during the snapshot copy (microseconds). After
/// this function returns, the caller **must not** re-acquire the terminal
/// lock for the remainder of the frame.
pub(crate) fn extract_frame<T: EventListener>(
    terminal: &FairMutex<Term<T>>,
    viewport: ViewportSize,
    cell_size: CellMetrics,
) -> FrameInput {
    let start = Instant::now();

    // Acquire fair lock — guarantees render thread access.
    let term = terminal.lock();
    let lock_acquired = Instant::now();

    // Snapshot all rendering data under lock.
    let content = term.renderable_content();
    let palette = extract_palette(&term);

    // Release lock immediately — no terminal access after this point.
    drop(term);
    let lock_released = Instant::now();

    log::trace!(
        "extract_frame: lock wait={:?}, held={:?}, total={:?}",
        lock_acquired - start,
        lock_released - lock_acquired,
        lock_released - start,
    );

    FrameInput {
        content,
        viewport,
        cell_size,
        palette,
        selection: None,
        search_matches: Vec::new(),
        hovered_cell: None,
        mark_cursor: None,
    }
}

/// Snapshot terminal state, reusing allocations from a previous frame.
///
/// Like [`extract_frame`] but refills `out` in place, reusing the `Vec`
/// allocations inside `out.content`. Avoids per-frame allocation for the
/// `cells` and `damage` vectors (typically `lines × cols × 56` bytes).
pub(crate) fn extract_frame_into<T: EventListener>(
    terminal: &FairMutex<Term<T>>,
    out: &mut FrameInput,
    viewport: ViewportSize,
    cell_size: CellMetrics,
) {
    let start = Instant::now();

    let term = terminal.lock();
    let lock_acquired = Instant::now();

    term.renderable_content_into(&mut out.content);
    let palette = extract_palette(&term);

    drop(term);
    let lock_released = Instant::now();

    log::trace!(
        "extract_frame_into: lock wait={:?}, held={:?}, total={:?}",
        lock_acquired - start,
        lock_released - lock_acquired,
        lock_released - start,
    );

    out.viewport = viewport;
    out.cell_size = cell_size;
    out.palette = palette;
    out.selection = None;
    out.search_matches.clear();
    out.hovered_cell = None;
    out.mark_cursor = None;
}

/// Extract semantic palette colors from the terminal.
///
/// Opacity is set to 1.0 here (terminal state doesn't know about window
/// opacity). The app layer overrides it from config after extraction.
fn extract_palette<T: EventListener>(term: &Term<T>) -> FramePalette {
    let palette = term.palette();
    FramePalette {
        background: palette.background(),
        foreground: palette.foreground(),
        cursor_color: palette.cursor_color(),
        opacity: 1.0,
    }
}

#[cfg(test)]
mod tests;
