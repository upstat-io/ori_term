//! Snapshot building for IPC responses.
//!
//! Converts internal terminal state into wire-friendly types ([`PaneSnapshot`],
//! [`WireCell`], [`WireCursor`]) for transmission to window processes.
//!
//! Colors are pre-resolved server-side via [`Term::renderable_content()`] — the
//! wire cells carry resolved RGB values (bold-as-bright, dim, inverse already
//! applied). This eliminates the need for clients to duplicate color resolution.

use std::collections::HashMap;

use oriterm_core::{CursorShape, RenderableCell, Rgb};

use crate::pane::Pane;
use crate::registry::PaneRegistry;
use crate::{
    MuxTabInfo, MuxWindowInfo, PaneId, PaneSnapshot, SessionRegistry, WindowId, WireCell,
    WireCursor, WireCursorShape, WireRgb,
};

/// Build a full snapshot of a pane's visible state.
///
/// Uses [`Term::renderable_content()`] to produce pre-resolved colors.
/// The wire cells carry resolved RGB — clients never need to reference the
/// palette for per-cell fg/bg.
pub fn build_snapshot(pane: &Pane) -> PaneSnapshot {
    let term = pane.terminal().lock();

    // renderable_content() resolves all per-cell colors (bold-as-bright,
    // dim, inverse) and computes cursor visibility.
    let content = term.renderable_content();
    let grid = term.grid();
    let lines = grid.lines();
    let cols = grid.cols();

    // Convert flat cell vec to wire rows.
    let mut cells = Vec::with_capacity(lines);
    let mut row_buf = Vec::with_capacity(cols);
    for cell in &content.cells {
        row_buf.push(renderable_to_wire(cell));
        if row_buf.len() == cols {
            cells.push(std::mem::replace(&mut row_buf, Vec::with_capacity(cols)));
        }
    }
    // Flush any partial last row.
    if !row_buf.is_empty() {
        cells.push(row_buf);
    }

    // Cursor (visibility already resolved by renderable_content).
    let wire_cursor = WireCursor {
        col: u16::try_from(content.cursor.column.0).unwrap_or(u16::MAX),
        row: u16::try_from(content.cursor.line).unwrap_or(u16::MAX),
        shape: cursor_shape_to_wire(content.cursor.shape),
        visible: content.cursor.visible,
    };

    // Palette: extract 270 RGB triplets (needed for FramePalette semantic
    // colors: background, foreground, cursor color, selection overrides).
    let palette = term.palette();
    let palette_rgb: Vec<[u8; 3]> = (0..270)
        .map(|i| {
            let rgb = palette.color(i);
            [rgb.r, rgb.g, rgb.b]
        })
        .collect();

    PaneSnapshot {
        cells,
        cursor: wire_cursor,
        palette: palette_rgb,
        title: pane.effective_title().to_string(),
        modes: content.mode.bits(),
        scrollback_len: u32::try_from(grid.scrollback().len()).unwrap_or(u32::MAX),
        display_offset: u32::try_from(content.display_offset).unwrap_or(u32::MAX),
    }
}

/// Convert a pre-resolved [`RenderableCell`] to a [`WireCell`].
fn renderable_to_wire(cell: &RenderableCell) -> WireCell {
    WireCell {
        ch: cell.ch,
        fg: rgb_to_wire(cell.fg),
        bg: rgb_to_wire(cell.bg),
        flags: cell.flags.bits(),
        underline_color: cell.underline_color.map(rgb_to_wire),
        has_hyperlink: cell.has_hyperlink,
        zerowidth: cell.zerowidth.clone(),
    }
}

/// Convert an [`Rgb`] to a [`WireRgb`].
fn rgb_to_wire(rgb: Rgb) -> WireRgb {
    WireRgb {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}

/// Map [`CursorShape`] enum to [`WireCursorShape`].
fn cursor_shape_to_wire(shape: CursorShape) -> WireCursorShape {
    match shape {
        CursorShape::Block => WireCursorShape::Block,
        CursorShape::Underline => WireCursorShape::Underline,
        CursorShape::Bar => WireCursorShape::Bar,
        CursorShape::HollowBlock => WireCursorShape::HollowBlock,
        CursorShape::Hidden => WireCursorShape::Hidden,
    }
}

/// Build the list of all mux windows for a `ListWindows` response.
pub fn build_window_list(session: &SessionRegistry) -> Vec<MuxWindowInfo> {
    let mut windows = Vec::new();
    // Iterate all windows by checking known IDs.
    // SessionRegistry exposes get_window — we iterate via the session's
    // internal window map. Since SessionRegistry doesn't expose an iterator,
    // we use the window_ids accessor.
    for (&window_id, win) in session.windows() {
        windows.push(MuxWindowInfo {
            window_id,
            tab_count: win.tabs().len() as u32,
            active_tab_id: win.active_tab(),
        });
    }
    windows
}

/// Build the list of tabs in a window for a `ListTabs` response.
pub fn build_tab_list(
    session: &SessionRegistry,
    pane_registry: &PaneRegistry,
    panes: &HashMap<PaneId, Pane>,
    window_id: WindowId,
) -> Vec<MuxTabInfo> {
    let Some(win) = session.get_window(window_id) else {
        return Vec::new();
    };

    win.tabs()
        .iter()
        .filter_map(|&tab_id| {
            let tab = session.get_tab(tab_id)?;
            let active_pane_id = tab.active_pane();
            let pane_count = pane_registry.panes_in_tab(tab_id).len() as u32;
            let title = panes
                .get(&active_pane_id)
                .map(|p| p.effective_title().to_string())
                .unwrap_or_default();
            Some(MuxTabInfo {
                tab_id,
                active_pane_id,
                pane_count,
                title,
            })
        })
        .collect()
}
