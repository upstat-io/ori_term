//! Snapshot building for IPC responses.
//!
//! Converts internal terminal state into wire-friendly types ([`PaneSnapshot`],
//! [`WireCell`], [`WireCursor`]) for transmission to window processes.
//!
//! Colors are pre-resolved server-side via [`Term::renderable_content()`] — the
//! wire cells carry resolved RGB values (bold-as-bright, dim, inverse already
//! applied). This eliminates the need for clients to duplicate color resolution.

use std::collections::HashMap;

use oriterm_core::index::Line;
use oriterm_core::{Column, CursorShape, Grid, RenderableCell, RenderableContent, Rgb, Term};

use crate::mux_event::MuxEventProxy;
use crate::pane::Pane;
use crate::registry::PaneRegistry;
use crate::{
    MuxTabInfo, MuxWindowInfo, PaneId, PaneSnapshot, SessionRegistry, WindowId, WireCell,
    WireCursor, WireCursorShape, WireRgb, WireSearchMatch,
};

/// Build a full snapshot of a pane's visible state.
///
/// Blocks on the terminal lock. The PTY reader uses a fairness-gate
/// lease to control when the renderer gets access — the reader yields
/// between parse cycles, keeping this wait brief even during flood output.
pub fn build_snapshot(pane: &Pane) -> PaneSnapshot {
    let term = pane.terminal().lock();
    build_snapshot_inner(&term, pane)
}

/// Build a snapshot into an existing [`PaneSnapshot`], reusing allocations.
///
/// The caller owns the `out` and `render_buf` buffers. Row `Vec`s, the
/// palette `Vec`, and the `RenderableContent::cells` buffer keep their
/// allocated capacity across frames, avoiding the per-frame allocation
/// that makes [`build_snapshot`] expensive under sustained flood output.
pub fn build_snapshot_into(
    pane: &Pane,
    out: &mut PaneSnapshot,
    render_buf: &mut RenderableContent,
) {
    let lock_start = std::time::Instant::now();
    let term = pane.terminal().lock();
    let lock_elapsed = lock_start.elapsed();
    let build_start = std::time::Instant::now();
    build_snapshot_inner_into(&term, pane, out, render_buf);
    let build_elapsed = build_start.elapsed();
    if lock_elapsed.as_millis() > 2 || build_elapsed.as_millis() > 2 {
        log::warn!(
            "[DIAG] build_snapshot_into: lock={:?} build={:?}",
            lock_elapsed,
            build_elapsed,
        );
    }
}

/// Shared allocation-reusing snapshot logic.
///
/// Same work as [`build_snapshot_inner`] but mutates `out` in place,
/// reusing existing `Vec` capacities.
fn build_snapshot_inner_into(
    term: &Term<MuxEventProxy>,
    pane: &Pane,
    out: &mut PaneSnapshot,
    render_buf: &mut RenderableContent,
) {
    term.renderable_content_into(render_buf);
    let grid = term.grid();
    let cols = grid.cols();

    // Reuse row Vecs: clear and refill existing rows, push new ones if
    // the grid grew, truncate extras if it shrank.
    let offset = render_buf.display_offset;
    let mut row_idx = 0;
    let mut col_count = 0;
    for cell in &render_buf.cells {
        let hyperlink_uri = if cell.has_hyperlink {
            hyperlink_uri_at(grid, cell.line, cell.column, offset)
        } else {
            None
        };
        let wire = renderable_to_wire(cell, hyperlink_uri);

        if col_count == 0 {
            // Start of a new row — reuse or create.
            if row_idx < out.cells.len() {
                out.cells[row_idx].clear();
            } else {
                out.cells.push(Vec::with_capacity(cols));
            }
        }

        if row_idx < out.cells.len() {
            out.cells[row_idx].push(wire);
        }

        col_count += 1;
        if col_count == cols {
            col_count = 0;
            row_idx += 1;
        }
    }
    // Flush partial last row.
    if col_count > 0 {
        row_idx += 1;
    }
    // Truncate extra rows from a previous larger grid.
    out.cells.truncate(row_idx);

    // Cursor.
    out.cursor = WireCursor {
        col: u16::try_from(render_buf.cursor.column.0).unwrap_or(u16::MAX),
        row: u16::try_from(render_buf.cursor.line).unwrap_or(u16::MAX),
        shape: cursor_shape_to_wire(render_buf.cursor.shape),
        visible: render_buf.cursor.visible,
    };

    // Palette: reuse Vec capacity.
    out.palette.clear();
    let palette = term.palette();
    out.palette
        .reserve(270usize.saturating_sub(out.palette.capacity()));
    for i in 0..270 {
        let rgb = palette.color(i);
        out.palette.push([rgb.r, rgb.g, rgb.b]);
    }

    // Title.
    out.title.clear();
    out.title.push_str(pane.effective_title());

    // Icon name.
    out.icon_name = pane.icon_name().map(str::to_owned);

    // CWD.
    out.cwd = pane.cwd().map(str::to_owned);

    // Scalar fields.
    out.modes = render_buf.mode.bits();
    out.scrollback_len = u32::try_from(grid.scrollback().len()).unwrap_or(u32::MAX);
    out.display_offset = u32::try_from(render_buf.display_offset).unwrap_or(u32::MAX);
    out.stable_row_base = render_buf.stable_row_base;
    out.cols = cols as u16;

    // Search state.
    if let Some(search) = pane.search() {
        out.search_active = true;
        out.search_query.clear();
        out.search_query.push_str(search.query());
        out.search_matches.clear();
        for m in search.matches() {
            out.search_matches.push(WireSearchMatch {
                start_row: m.start_row.0,
                start_col: u16::try_from(m.start_col).unwrap_or(u16::MAX),
                end_row: m.end_row.0,
                end_col: u16::try_from(m.end_col).unwrap_or(u16::MAX),
            });
        }
        let total = out.search_matches.len() as u32;
        out.search_total_matches = total;
        out.search_focused = if out.search_matches.is_empty() {
            None
        } else {
            Some(search.focused_index() as u32)
        };
    } else {
        out.search_active = false;
        out.search_query.clear();
        out.search_matches.clear();
        out.search_focused = None;
        out.search_total_matches = 0;
    }
}

/// Shared snapshot logic — converts terminal state to wire format.
///
/// Uses [`Term::renderable_content()`] to produce pre-resolved colors.
/// The wire cells carry resolved RGB — clients never need to reference the
/// palette for per-cell fg/bg.
fn build_snapshot_inner(term: &Term<MuxEventProxy>, pane: &Pane) -> PaneSnapshot {
    // renderable_content() resolves all per-cell colors (bold-as-bright,
    // dim, inverse) and computes cursor visibility.
    let content = term.renderable_content();
    let grid = term.grid();
    let lines = grid.lines();
    let cols = grid.cols();

    // Convert flat cell vec to wire rows.
    let offset = content.display_offset;
    let mut cells = Vec::with_capacity(lines);
    let mut row_buf = Vec::with_capacity(cols);
    for cell in &content.cells {
        let hyperlink_uri = if cell.has_hyperlink {
            hyperlink_uri_at(grid, cell.line, cell.column, offset)
        } else {
            None
        };
        row_buf.push(renderable_to_wire(cell, hyperlink_uri));
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

    // Search state.
    let (search_active, search_query, search_matches, search_focused, search_total_matches) =
        if let Some(search) = pane.search() {
            let matches: Vec<WireSearchMatch> = search
                .matches()
                .iter()
                .map(|m| WireSearchMatch {
                    start_row: m.start_row.0,
                    start_col: u16::try_from(m.start_col).unwrap_or(u16::MAX),
                    end_row: m.end_row.0,
                    end_col: u16::try_from(m.end_col).unwrap_or(u16::MAX),
                })
                .collect();
            let total = matches.len() as u32;
            let focused = if matches.is_empty() {
                None
            } else {
                Some(search.focused_index() as u32)
            };
            (true, search.query().to_string(), matches, focused, total)
        } else {
            (false, String::new(), Vec::new(), None, 0)
        };

    PaneSnapshot {
        cells,
        cursor: wire_cursor,
        palette: palette_rgb,
        title: pane.effective_title().to_string(),
        icon_name: pane.icon_name().map(str::to_owned),
        cwd: pane.cwd().map(str::to_owned),
        modes: content.mode.bits(),
        scrollback_len: u32::try_from(grid.scrollback().len()).unwrap_or(u32::MAX),
        display_offset: u32::try_from(content.display_offset).unwrap_or(u32::MAX),
        stable_row_base: content.stable_row_base,
        cols: cols as u16,
        search_active,
        search_query,
        search_matches,
        search_focused,
        search_total_matches,
    }
}

/// Convert a pre-resolved [`RenderableCell`] to a [`WireCell`].
fn renderable_to_wire(cell: &RenderableCell, hyperlink_uri: Option<String>) -> WireCell {
    WireCell {
        ch: cell.ch,
        fg: rgb_to_wire(cell.fg),
        bg: rgb_to_wire(cell.bg),
        flags: cell.flags.bits(),
        underline_color: cell.underline_color.map(rgb_to_wire),
        hyperlink_uri,
        zerowidth: cell.zerowidth.clone(),
    }
}

/// Look up the hyperlink URI for a viewport cell from the grid.
///
/// Only called when `RenderableCell::has_hyperlink` is true.
fn hyperlink_uri_at(
    grid: &Grid,
    vis_line: usize,
    col: Column,
    display_offset: usize,
) -> Option<String> {
    let row = if vis_line < display_offset {
        let sb_idx = grid.scrollback().len() - display_offset + vis_line;
        grid.scrollback().get(sb_idx)?
    } else {
        let grid_line = vis_line - display_offset;
        &grid[Line(grid_line as i32)]
    };
    row[col].hyperlink().map(|h| h.uri.clone())
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
