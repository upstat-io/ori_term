//! Terminal state machine.
//!
//! `Term<T: EventListener>` owns two grids (primary + alternate), mode flags,
//! color palette, charset state, and processes escape sequences via the
//! `vte::ansi::Handler` trait. Generic over `EventListener` for decoupling
//! from the UI layer.

pub mod charset;
mod handler;
pub mod mode;

pub use charset::CharsetState;
pub use mode::TermMode;

use crate::color::Palette;
use crate::event::EventListener;
use crate::grid::{CursorShape, Grid};

/// Maximum depth for title stack (xterm push/pop title).
///
/// Prevents OOM from malicious PTY input pushing unlimited titles.
/// Matches Alacritty's cap. Enforced in the VTE handler's `push_title`.
#[expect(dead_code, reason = "cap enforced when VTE handler push_title is implemented")]
const TITLE_STACK_MAX_DEPTH: usize = 4096;

/// Maximum depth for Kitty keyboard enhancement mode stacks.
///
/// Prevents OOM from malicious PTY input. Matches Alacritty's cap.
/// Enforced in the VTE handler's `set_keyboard_mode`.
#[expect(dead_code, reason = "cap enforced when VTE handler set_keyboard_mode is implemented")]
const KEYBOARD_MODE_STACK_MAX_DEPTH: usize = 4096;

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
    /// Window title (set by OSC 0/2).
    title: String,
    /// Pushed title stack (xterm extension). Capped at [`TITLE_STACK_MAX_DEPTH`].
    title_stack: Vec<String>,
    /// Cursor shape for rendering.
    cursor_shape: CursorShape,
    /// Kitty keyboard enhancement mode stack (active screen).
    /// Capped at [`KEYBOARD_MODE_STACK_MAX_DEPTH`].
    keyboard_mode_stack: Vec<u8>,
    /// Kitty keyboard enhancement mode stack (inactive screen).
    /// Capped at [`KEYBOARD_MODE_STACK_MAX_DEPTH`].
    inactive_keyboard_mode_stack: Vec<u8>,
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
            title_stack: Vec::new(),
            cursor_shape: CursorShape::default(),
            keyboard_mode_stack: Vec::new(),
            inactive_keyboard_mode_stack: Vec::new(),
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

    /// Current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Reference to the charset state.
    pub fn charset(&self) -> &CharsetState {
        &self.charset
    }

    /// The title stack (xterm push/pop title).
    pub fn title_stack(&self) -> &[String] {
        &self.title_stack
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
