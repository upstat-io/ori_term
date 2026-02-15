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
pub use renderable::{DamageLine, RenderableCell, RenderableContent, RenderableCursor};

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
    /// Iterates visible rows (accounting for `display_offset` into scrollback),
    /// resolves all cell colors via the palette, and captures cursor + damage
    /// info. Designed to be called under lock — copies data so the renderer
    /// can work without holding the lock.
    pub fn renderable_content(&self) -> RenderableContent {
        let grid = self.grid();
        let offset = grid.display_offset();
        let lines = grid.lines();
        let cols = grid.cols();
        let palette = &self.palette;

        let mut cells = Vec::with_capacity(lines * cols);

        debug_assert!(
            offset <= grid.scrollback().len(),
            "display_offset ({offset}) must be <= scrollback.len() ({})",
            grid.scrollback().len(),
        );

        for vis_line in 0..lines {
            // Top `offset` lines come from scrollback; the rest from the grid.
            let row = if vis_line < offset {
                let sb_idx = offset - 1 - vis_line;
                grid.scrollback()
                    .get(sb_idx)
                    .expect("display_offset must be <= scrollback.len()")
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

                let underline_color = cell
                    .extra
                    .as_ref()
                    .and_then(|e| e.underline_color)
                    .map(|c| palette.resolve(c));

                let zerowidth = cell
                    .extra
                    .as_ref()
                    .map(|e| e.zerowidth.clone())
                    .unwrap_or_default();

                cells.push(RenderableCell {
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

        let cursor = RenderableCursor {
            line: grid.cursor().line(),
            column: grid.cursor().col(),
            shape: self.cursor_shape,
            visible: cursor_visible,
        };

        let (all_dirty, damage) = self.collect_damage(grid, lines, cols);

        RenderableContent {
            cells,
            cursor,
            display_offset: offset,
            mode: self.mode,
            all_dirty,
            damage,
        }
    }

    /// Collect damage information from the grid's dirty tracker.
    fn collect_damage(&self, grid: &Grid, lines: usize, cols: usize) -> (bool, Vec<DamageLine>) {
        let dirty = grid.dirty();

        // Fast path: tracker explicitly flagged all-dirty (resize, alt swap).
        // Avoids building a Vec that would be immediately discarded.
        if dirty.is_all_dirty() {
            return (true, Vec::new());
        }

        // Slow path: check individual bits (handles mark_range covering all lines).
        let mut all_dirty = true;
        let mut damage = Vec::new();
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
