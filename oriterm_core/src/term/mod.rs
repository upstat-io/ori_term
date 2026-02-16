//! Terminal state machine.
//!
//! `Term<T: EventListener>` owns two grids (primary + alternate), mode flags,
//! color palette, charset state, and processes escape sequences via the
//! `vte::ansi::Handler` trait. Generic over `EventListener` for decoupling
//! from the UI layer.

pub mod charset;
mod handler;
pub mod mode;
pub mod renderable;

pub use charset::CharsetState;
pub use mode::TermMode;
pub use renderable::{DamageLine, RenderableCell, RenderableContent, RenderableCursor, TermDamage};

use std::collections::VecDeque;

use vte::ansi::KeyboardModes;

use crate::color::Palette;
use crate::event::EventListener;
use crate::grid::{CursorShape, Grid};
use crate::index::{Column, Line};

/// Maximum depth for title stack (xterm push/pop title).
///
/// Prevents OOM from malicious PTY input pushing unlimited titles.
/// Matches Alacritty's cap. Enforced in the VTE handler's `push_title`.
const TITLE_STACK_MAX_DEPTH: usize = 4096;

/// Maximum depth for Kitty keyboard enhancement mode stacks.
///
/// Prevents OOM from malicious PTY input. Matches Alacritty's cap.
/// Enforced in the VTE handler's `push_keyboard_mode`.
pub(crate) const KEYBOARD_MODE_STACK_MAX_DEPTH: usize = 4096;

/// The terminal state machine.
///
/// Owns two grids (primary + alternate screen), terminal mode flags, color
/// palette, charset state, title, and keyboard mode stacks. Generic over
/// `T: EventListener` so tests can use `VoidListener` while the real app
/// routes events through winit.
#[derive(Debug)]
pub struct Term<T: EventListener> {
    /// Primary grid (active when not in alt screen).
    grid: Grid,
    /// Alternate grid (active during alt screen; no scrollback).
    alt_grid: Grid,
    /// Terminal mode flags (DECSET/DECRST).
    mode: TermMode,
    /// Color palette (270 entries).
    palette: Palette,
    /// Character set translation state (G0–G3).
    charset: CharsetState,
    /// Window title (set by OSC 0/1/2).
    title: String,
    /// Current working directory (set by OSC 7 shell integration).
    cwd: Option<String>,
    /// Pushed title stack (xterm extension). Capped at [`TITLE_STACK_MAX_DEPTH`].
    title_stack: VecDeque<String>,
    /// Cursor shape for rendering.
    cursor_shape: CursorShape,
    /// Kitty keyboard enhancement mode stack (active screen).
    /// Capped at [`KEYBOARD_MODE_STACK_MAX_DEPTH`].
    keyboard_mode_stack: VecDeque<KeyboardModes>,
    /// Kitty keyboard enhancement mode stack (inactive screen).
    /// Capped at [`KEYBOARD_MODE_STACK_MAX_DEPTH`].
    inactive_keyboard_mode_stack: VecDeque<KeyboardModes>,
    /// Event sink for terminal events.
    event_listener: T,
}

impl<T: EventListener> Term<T> {
    /// Create a new terminal with the given dimensions and scrollback capacity.
    pub fn new(lines: usize, cols: usize, scrollback: usize, listener: T) -> Self {
        Self {
            grid: Grid::with_scrollback(lines, cols, scrollback),
            alt_grid: Grid::with_scrollback(lines, cols, 0),
            mode: TermMode::default(),
            palette: Palette::default(),
            charset: CharsetState::default(),
            title: String::new(),
            cwd: None,
            title_stack: VecDeque::new(),
            cursor_shape: CursorShape::default(),
            keyboard_mode_stack: VecDeque::new(),
            inactive_keyboard_mode_stack: VecDeque::new(),
            event_listener: listener,
        }
    }

    /// Reference to the active grid.
    pub fn grid(&self) -> &Grid {
        if self.mode.contains(TermMode::ALT_SCREEN) { &self.alt_grid } else { &self.grid }
    }

    /// Mutable reference to the active grid.
    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.mode.contains(TermMode::ALT_SCREEN) { &mut self.alt_grid } else { &mut self.grid }
    }

    /// Current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        self.mode
    }

    /// Reference to the color palette.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Current window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Current working directory (set by OSC 7).
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Reference to the charset state.
    pub fn charset(&self) -> &CharsetState {
        &self.charset
    }

    /// The title stack (xterm push/pop title).
    #[cfg(test)]
    pub(crate) fn title_stack(&self) -> &VecDeque<String> {
        &self.title_stack
    }

    /// Current keyboard mode stack (Kitty keyboard protocol).
    #[cfg(test)]
    pub(crate) fn keyboard_mode_stack(&self) -> &VecDeque<KeyboardModes> {
        &self.keyboard_mode_stack
    }

    /// Extract a complete rendering snapshot.
    ///
    /// Convenience wrapper that allocates a fresh [`RenderableContent`] and
    /// fills it. For hot-path rendering, prefer [`renderable_content_into`]
    /// with a reused buffer to avoid per-frame allocation.
    ///
    /// This is a pure read — dirty state is **not** cleared. Callers must
    /// drain dirty state separately via `grid_mut().dirty_mut().drain()`
    /// after consuming the snapshot.
    ///
    /// [`renderable_content_into`]: Self::renderable_content_into
    pub fn renderable_content(&self) -> RenderableContent {
        let grid = self.grid();
        let mut out = RenderableContent {
            cells: Vec::with_capacity(grid.lines() * grid.cols()),
            cursor: RenderableCursor {
                line: 0,
                column: Column(0),
                shape: CursorShape::default(),
                visible: false,
            },
            display_offset: 0,
            mode: TermMode::empty(),
            all_dirty: false,
            damage: Vec::new(),
        };
        self.renderable_content_into(&mut out);
        out
    }

    /// Fill an existing [`RenderableContent`] with the current terminal state.
    ///
    /// Clears `out` and refills it, reusing the underlying `Vec` allocations.
    /// The renderer should keep a single `RenderableContent` and pass it each
    /// frame to avoid the ~`lines * cols * 56` byte allocation that
    /// [`renderable_content`] performs.
    ///
    /// This is a pure read — dirty state is **not** cleared. Callers must
    /// drain dirty state separately via `grid_mut().dirty_mut().drain()`
    /// after consuming the snapshot.
    ///
    /// [`renderable_content`]: Self::renderable_content
    pub fn renderable_content_into(&self, out: &mut RenderableContent) {
        out.cells.clear();
        out.damage.clear();

        let grid = self.grid();
        let raw_offset = grid.display_offset();
        debug_assert!(
            raw_offset <= grid.scrollback().len(),
            "display_offset ({raw_offset}) must be <= scrollback.len() ({})",
            grid.scrollback().len(),
        );
        let offset = raw_offset.min(grid.scrollback().len());
        let lines = grid.lines();
        let cols = grid.cols();
        let palette = &self.palette;

        for vis_line in 0..lines {
            // Top `offset` lines come from scrollback; the rest from the grid.
            let row = if vis_line < offset {
                let sb_idx = offset - 1 - vis_line;
                match grid.scrollback().get(sb_idx) {
                    Some(row) => row,
                    None => continue,
                }
            } else {
                let grid_line = vis_line - offset;
                &grid[Line(grid_line as i32)]
            };

            for col_idx in 0..cols {
                let col = Column(col_idx);
                let cell = &row[col];

                let fg = renderable::resolve_fg(cell.fg, cell.flags, palette);
                let bg = renderable::resolve_bg(cell.bg, palette);
                let (fg, bg) = renderable::apply_inverse(fg, bg, cell.flags);

                let (underline_color, zerowidth) = match cell.extra.as_ref() {
                    Some(e) => (
                        e.underline_color.map(|c| palette.resolve(c)),
                        e.zerowidth.clone(),
                    ),
                    None => (None, Vec::new()),
                };

                out.cells.push(RenderableCell {
                    line: vis_line,
                    column: col,
                    ch: cell.ch,
                    fg,
                    bg,
                    flags: cell.flags,
                    underline_color,
                    zerowidth,
                });
            }
        }

        // Cursor is visible when SHOW_CURSOR is set and we're at the live view.
        let cursor_visible = self.mode.contains(TermMode::SHOW_CURSOR)
            && offset == 0
            && self.cursor_shape != CursorShape::Hidden;

        out.cursor = RenderableCursor {
            line: grid.cursor().line(),
            column: grid.cursor().col(),
            shape: self.cursor_shape,
            visible: cursor_visible,
        };

        let (all_dirty, damage) = self.collect_damage(grid, lines, cols);
        out.display_offset = offset;
        out.mode = self.mode;
        out.all_dirty = all_dirty;
        out.damage = damage;
    }

    /// Collect damage information from the grid's dirty tracker.
    fn collect_damage(&self, grid: &Grid, lines: usize, cols: usize) -> (bool, Vec<DamageLine>) {
        let dirty = grid.dirty();

        // Fast path: tracker explicitly flagged all-dirty (resize, alt swap).
        // Avoids building a Vec that would be immediately discarded.
        if dirty.is_all_dirty() {
            return (true, Vec::new());
        }

        // Fast path: nothing dirty — skip the per-line scan entirely.
        if !dirty.is_any_dirty() {
            return (false, Vec::new());
        }

        // Slow path: check individual bits (handles mark_range covering all lines).
        let mut all_dirty = true;
        let mut damage = Vec::with_capacity(lines);
        for line in 0..lines {
            if dirty.is_dirty(line) {
                damage.push(DamageLine {
                    line,
                    left: Column(0),
                    right: Column(cols.saturating_sub(1)),
                });
            } else {
                all_dirty = false;
            }
        }

        if all_dirty && !damage.is_empty() {
            (true, Vec::new())
        } else {
            (false, damage)
        }
    }

    /// Drain damage from the active grid.
    ///
    /// Returns a [`TermDamage`] iterator that yields dirty lines and clears
    /// marks as it goes. Check [`TermDamage::is_all_dirty`] first — when true,
    /// repaint everything and drop the iterator (which clears remaining marks).
    pub fn damage(&mut self) -> TermDamage<'_> {
        let grid = self.grid_mut();
        let cols = grid.cols();
        let all_dirty = grid.dirty().is_all_dirty();
        TermDamage::new(grid.dirty_mut().drain(), cols, all_dirty)
    }

    /// Clear all damage marks without reading them.
    ///
    /// Called when the renderer wants to discard pending damage (e.g. after
    /// a full repaint that doesn't need per-line tracking).
    pub fn reset_damage(&mut self) {
        self.grid_mut().dirty_mut().drain().for_each(drop);
    }

    /// Switch between primary and alternate screen.
    ///
    /// Saves/restores cursor, toggles `TermMode::ALT_SCREEN`, swaps keyboard
    /// mode stacks, and marks all lines dirty.
    pub fn swap_alt(&mut self) {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            // Switching back to primary: save alt cursor, restore primary cursor.
            self.alt_grid.save_cursor();
            self.grid.restore_cursor();
        } else {
            // Switching to alt: save primary cursor, restore alt cursor.
            self.grid.save_cursor();
            self.alt_grid.restore_cursor();
        }

        self.mode.toggle(TermMode::ALT_SCREEN);
        std::mem::swap(&mut self.keyboard_mode_stack, &mut self.inactive_keyboard_mode_stack);
        self.grid_mut().dirty_mut().mark_all();
    }
}

#[cfg(test)]
mod tests;
