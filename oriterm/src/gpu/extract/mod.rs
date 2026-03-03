//! Extract phase: snapshot terminal state into owned frame data.
//!
//! Two extraction paths:
//!
//! - **Embedded mode**: Locks the terminal via [`FairMutex`], copies visible
//!   cells, cursor, palette, and damage into a [`FrameInput`], then releases
//!   the lock. Total lock hold time: microseconds.
//!
//! - **Daemon mode**: Converts a [`PaneSnapshot`](oriterm_mux::PaneSnapshot)
//!   (wire data from the daemon) into a [`FrameInput`]. No lock needed —
//!   the snapshot is already owned data.

mod from_snapshot;

use std::time::Instant;

use oriterm_core::{EventListener, FairMutex, RenderableContent, Term};

pub(crate) use self::from_snapshot::extract_frame_from_snapshot;

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
    let prompt_marker_rows = extract_prompt_marker_rows(&term, &content);

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
        search: None,
        hovered_cell: None,
        hovered_url_segments: Vec::new(),
        mark_cursor: None,
        fg_dim: 1.0,
        prompt_marker_rows,
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
    extract_prompt_marker_rows_into(&term, &out.content, &mut out.prompt_marker_rows);

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
    out.search = None;
    out.hovered_cell = None;
    out.hovered_url_segments.clear();
    out.mark_cursor = None;
    out.fg_dim = 1.0;
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
        selection_fg: palette.selection_fg(),
        selection_bg: palette.selection_bg(),
    }
}

/// Extract viewport-relative prompt marker rows from the terminal.
///
/// Scans the terminal's prompt markers and returns the viewport line index
/// for each marker whose prompt row falls within the visible range.
fn extract_prompt_marker_rows<T: EventListener>(
    term: &Term<T>,
    content: &RenderableContent,
) -> Vec<usize> {
    let markers = term.prompt_markers();
    if markers.is_empty() {
        return Vec::new();
    }
    let grid = term.grid();
    let sb_len = grid.scrollback().len();
    let offset = content.display_offset;
    let lines = grid.lines();
    // Absolute row of the topmost visible line.
    let viewport_top = sb_len.saturating_sub(offset);
    let viewport_bottom = viewport_top + lines;

    let mut rows = Vec::new();
    for marker in markers {
        if marker.prompt >= viewport_top && marker.prompt < viewport_bottom {
            rows.push(marker.prompt - viewport_top);
        }
    }
    rows
}

/// Extract visible prompt marker rows, reusing an existing `Vec` allocation.
fn extract_prompt_marker_rows_into<T: EventListener>(
    term: &Term<T>,
    content: &RenderableContent,
    out: &mut Vec<usize>,
) {
    out.clear();
    let markers = term.prompt_markers();
    if markers.is_empty() {
        return;
    }
    let grid = term.grid();
    let sb_len = grid.scrollback().len();
    let offset = content.display_offset;
    let lines = grid.lines();
    let viewport_top = sb_len.saturating_sub(offset);
    let viewport_bottom = viewport_top + lines;

    for marker in markers {
        if marker.prompt >= viewport_top && marker.prompt < viewport_bottom {
            out.push(marker.prompt - viewport_top);
        }
    }
}

#[cfg(test)]
mod tests;
