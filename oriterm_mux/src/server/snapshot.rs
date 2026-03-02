//! Snapshot building for IPC responses.
//!
//! Converts internal terminal state (`Cell`, `Color`, `Cursor`, `Grid`) into
//! wire-friendly types ([`PaneSnapshot`], [`WireCell`], [`WireColor`],
//! [`WireCursor`]) for transmission to window processes.

use std::collections::HashMap;

use oriterm_core::{Column, Line};
use vte::ansi::Color;

use crate::pane::Pane;
use crate::registry::PaneRegistry;
use crate::{
    MuxTabInfo, MuxWindowInfo, PaneId, PaneSnapshot, SessionRegistry, WindowId, WireCell,
    WireColor, WireCursor, WireCursorShape, WireRgb,
};

/// Build a full snapshot of a pane's visible state.
pub fn build_snapshot(pane: &Pane) -> PaneSnapshot {
    let term = pane.terminal().lock();
    let grid = term.grid();

    let lines = grid.lines();
    let cols = grid.cols();

    // Convert visible grid to wire cells.
    let mut cells = Vec::with_capacity(lines);
    for row_idx in 0..lines {
        let row = &grid[Line(row_idx as i32)];
        let mut wire_row = Vec::with_capacity(cols);
        for col_idx in 0..cols {
            wire_row.push(cell_to_wire(&row[Column(col_idx)]));
        }
        cells.push(wire_row);
    }

    // Cursor.
    let cursor = grid.cursor();
    let cursor_shape = term.cursor_shape();
    let mode = term.mode();
    let cursor_visible = mode.contains(oriterm_core::TermMode::SHOW_CURSOR)
        && grid.display_offset() == 0
        && cursor_shape != oriterm_core::CursorShape::Hidden;

    let wire_cursor = WireCursor {
        col: cursor.col().0 as u16,
        row: cursor.line() as u16,
        shape: cursor_shape_to_wire(cursor_shape),
        visible: cursor_visible,
    };

    // Palette: extract 270 RGB triplets.
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
        modes: mode.bits(),
        scrollback_len: grid.scrollback().len() as u32,
        display_offset: grid.display_offset() as u32,
    }
}

/// Convert a core `Cell` to a `WireCell`.
fn cell_to_wire(cell: &oriterm_core::Cell) -> WireCell {
    let zerowidth = cell
        .extra
        .as_ref()
        .map(|e| e.zerowidth.clone())
        .unwrap_or_default();

    WireCell {
        ch: cell.ch,
        fg: color_to_wire(cell.fg),
        bg: color_to_wire(cell.bg),
        flags: cell.flags.bits(),
        zerowidth,
    }
}

/// Convert a `vte::ansi::Color` to a `WireColor`.
fn color_to_wire(color: Color) -> WireColor {
    match color {
        Color::Named(nc) => WireColor::Named(nc as u8),
        Color::Indexed(i) => WireColor::Indexed(i),
        Color::Spec(rgb) => WireColor::Rgb(WireRgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }),
    }
}

/// Map `CursorShape` enum to [`WireCursorShape`].
fn cursor_shape_to_wire(shape: oriterm_core::CursorShape) -> WireCursorShape {
    match shape {
        oriterm_core::CursorShape::Block => WireCursorShape::Block,
        oriterm_core::CursorShape::Underline => WireCursorShape::Underline,
        oriterm_core::CursorShape::Bar => WireCursorShape::Bar,
        oriterm_core::CursorShape::HollowBlock => WireCursorShape::HollowBlock,
        oriterm_core::CursorShape::Hidden => WireCursorShape::Hidden,
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
        let active_tab_id = win.active_tab().unwrap_or_else(|| {
            // Shouldn't happen — windows always have at least one tab.
            crate::TabId::from_raw(0)
        });
        windows.push(MuxWindowInfo {
            window_id,
            tab_count: win.tabs().len() as u32,
            active_tab_id,
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
