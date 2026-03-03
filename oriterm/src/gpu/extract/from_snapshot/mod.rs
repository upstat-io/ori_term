//! Convert a [`PaneSnapshot`] into a [`FrameInput`] for daemon-mode rendering.
//!
//! In daemon mode the client has no local `Term` — it renders from cached
//! wire snapshots instead. This module bridges the gap: `WireCell` →
//! `RenderableCell`, `WireCursor` → `RenderableCursor`, palette array →
//! `FramePalette`.

use oriterm_core::{
    CellFlags, Column, CursorShape, RenderableCell, RenderableContent, RenderableCursor, Rgb,
    TermMode,
};
use oriterm_mux::{PaneSnapshot, WireCursorShape, WireRgb};

use crate::font::CellMetrics;
use crate::gpu::frame_input::{FrameInput, FramePalette, ViewportSize};

/// Palette index for the foreground color (`vte::ansi::NamedColor::Foreground`).
const PALETTE_FOREGROUND: usize = 256;
/// Palette index for the background color (`vte::ansi::NamedColor::Background`).
const PALETTE_BACKGROUND: usize = 257;
/// Palette index for the cursor color (`vte::ansi::NamedColor::Cursor`).
const PALETTE_CURSOR: usize = 258;

/// Build a [`FrameInput`] from a daemon-mode [`PaneSnapshot`].
///
/// Wire cells already have pre-resolved RGB colors, so this is a
/// straightforward type conversion — no palette lookups needed for
/// per-cell fg/bg.
pub(crate) fn extract_frame_from_snapshot(
    snapshot: &PaneSnapshot,
    viewport: ViewportSize,
    cell_size: CellMetrics,
) -> FrameInput {
    let content = snapshot_to_renderable(snapshot);
    let palette = snapshot_palette(snapshot);

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
        prompt_marker_rows: Vec::new(),
    }
}

/// Convert a [`PaneSnapshot`] into [`RenderableContent`].
///
/// Wire RGB values map directly to [`Rgb`]; no palette resolution needed.
fn snapshot_to_renderable(snapshot: &PaneSnapshot) -> RenderableContent {
    let total_cells: usize = snapshot.cells.iter().map(Vec::len).sum();
    let mut cells = Vec::with_capacity(total_cells);

    for (line, row) in snapshot.cells.iter().enumerate() {
        for (col_idx, wire) in row.iter().enumerate() {
            cells.push(RenderableCell {
                line,
                column: Column(col_idx),
                ch: wire.ch,
                fg: wire_rgb_to_rgb(wire.fg),
                bg: wire_rgb_to_rgb(wire.bg),
                flags: CellFlags::from_bits_truncate(wire.flags),
                underline_color: wire.underline_color.map(wire_rgb_to_rgb),
                has_hyperlink: wire.has_hyperlink,
                zerowidth: wire.zerowidth.clone(),
            });
        }
    }

    let cursor = wire_cursor_to_renderable(snapshot.cursor);

    RenderableContent {
        cells,
        cursor,
        display_offset: snapshot.display_offset as usize,
        stable_row_base: 0,
        mode: TermMode::from_bits_truncate(snapshot.modes),
        all_dirty: true,
        damage: Vec::new(),
    }
}

/// Convert a [`WireCursorShape`] to a [`CursorShape`].
fn wire_shape_to_cursor(shape: WireCursorShape) -> CursorShape {
    match shape {
        WireCursorShape::Block => CursorShape::Block,
        WireCursorShape::Underline => CursorShape::Underline,
        WireCursorShape::Bar => CursorShape::Bar,
        WireCursorShape::HollowBlock => CursorShape::HollowBlock,
        WireCursorShape::Hidden => CursorShape::Hidden,
    }
}

/// Convert a [`WireCursor`] to a [`RenderableCursor`].
fn wire_cursor_to_renderable(wire: oriterm_mux::WireCursor) -> RenderableCursor {
    RenderableCursor {
        line: wire.row as usize,
        column: Column(wire.col as usize),
        shape: wire_shape_to_cursor(wire.shape),
        visible: wire.visible,
    }
}

/// Extract [`FramePalette`] from the snapshot's 270-entry palette array.
fn snapshot_palette(snapshot: &PaneSnapshot) -> FramePalette {
    let get = |idx: usize| -> Rgb {
        snapshot
            .palette
            .get(idx)
            .map_or(Rgb { r: 0, g: 0, b: 0 }, |&[r, g, b]| Rgb { r, g, b })
    };

    FramePalette {
        foreground: get(PALETTE_FOREGROUND),
        background: get(PALETTE_BACKGROUND),
        cursor_color: get(PALETTE_CURSOR),
        opacity: 1.0,
        selection_fg: None,
        selection_bg: None,
    }
}

/// Convert a [`WireRgb`] to an [`Rgb`].
fn wire_rgb_to_rgb(w: WireRgb) -> Rgb {
    Rgb {
        r: w.r,
        g: w.g,
        b: w.b,
    }
}

#[cfg(test)]
mod tests;
