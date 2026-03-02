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
mod shell_state;

pub use charset::CharsetState;
pub use mode::TermMode;
pub use renderable::{DamageLine, RenderableCell, RenderableContent, RenderableCursor, TermDamage};

use std::collections::VecDeque;

use vte::ansi::KeyboardModes;

use crate::color::Palette;
use crate::event::EventListener;
use crate::grid::{CursorShape, Grid};
use crate::index::{Column, Line};
use crate::theme::Theme;

/// Shell integration prompt lifecycle state.
///
/// Tracks transitions from OSC 133 sub-parameters:
/// `None` → `PromptStart` (A) → `CommandStart` (B) → `OutputStart` (C) → `None` (D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptState {
    /// No prompt activity or command completed (after D marker).
    #[default]
    None,
    /// Prompt is being displayed (after A marker).
    PromptStart,
    /// User is typing a command (after B marker).
    CommandStart,
    /// Command output is being produced (after C marker).
    OutputStart,
}

/// A single prompt lifecycle's boundary rows (absolute row indices).
///
/// Associates the OSC 133 sub-marker rows for one prompt: where the prompt
/// started (A), where the command line started (B), and where command output
/// started (C). Used for semantic zone navigation and selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptMarker {
    /// Absolute row where OSC 133;A (prompt start) was received.
    pub prompt: usize,
    /// Absolute row where OSC 133;B (command start) was received.
    pub command: Option<usize>,
    /// Absolute row where OSC 133;C (output start) was received.
    pub output: Option<usize>,
}

/// Desktop notification from the shell (OSC 9/99/777).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Notification title (may be empty for OSC 9/99).
    pub title: String,
    /// Notification body text.
    pub body: String,
}

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
#[allow(
    clippy::struct_excessive_bools,
    reason = "terminal state naturally has independent boolean flags \
              (selection_dirty, prompt_mark_pending, command_start_mark_pending, \
              output_start_mark_pending, has_explicit_title, title_dirty)"
)]
pub struct Term<T: EventListener> {
    /// Primary grid (active when not in alt screen).
    grid: Grid,
    /// Alternate grid (active during alt screen; no scrollback).
    alt_grid: Grid,
    /// Terminal mode flags (DECSET/DECRST).
    mode: TermMode,
    /// Color palette (270 entries).
    palette: Palette,
    /// Active color theme (dark/light).
    theme: Theme,
    /// Character set translation state (G0–G3).
    charset: CharsetState,
    /// Window title (set by OSC 0/2).
    title: String,
    /// Icon name (set by OSC 0/1).
    icon_name: String,
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
    /// Set by content-modifying VTE handler operations (character printing,
    /// erase, insert/delete, scroll). Checked by the owning layer to decide
    /// whether to clear an active selection.
    selection_dirty: bool,
    /// Shell integration prompt lifecycle state (OSC 133).
    prompt_state: PromptState,
    /// Set when OSC 133;A arrives — actual grid row marking is deferred
    /// until after both VTE parsers finish processing.
    prompt_mark_pending: bool,
    /// Prompt lifecycle markers (OSC 133 A/B/C positions).
    /// Used for jump-to-prompt navigation and semantic zone selection.
    /// Pruned when scrollback eviction removes old rows.
    prompt_markers: Vec<PromptMarker>,
    /// Set when OSC 133;B arrives — deferred until after both VTE parsers finish.
    command_start_mark_pending: bool,
    /// Set when OSC 133;C arrives — deferred until after both VTE parsers finish.
    output_start_mark_pending: bool,
    /// Pending desktop notifications collected from OSC 9/99/777.
    pending_notifications: Vec<Notification>,
    /// When OSC 133;C (output start) was received — marks command execution start.
    command_start: Option<std::time::Instant>,
    /// Duration of the last completed command (OSC 133;D − OSC 133;C).
    last_command_duration: Option<std::time::Duration>,
    /// Whether the current title was explicitly set via OSC 0/2.
    /// When `false`, the tab bar should prefer CWD-based title.
    has_explicit_title: bool,
    /// Title dirty flag — set when CWD or explicit title changes.
    title_dirty: bool,
}

impl<T: EventListener> Term<T> {
    /// Create a new terminal with the given dimensions and scrollback capacity.
    pub fn new(lines: usize, cols: usize, scrollback: usize, theme: Theme, listener: T) -> Self {
        Self {
            grid: Grid::with_scrollback(lines, cols, scrollback),
            alt_grid: Grid::with_scrollback(lines, cols, 0),
            mode: TermMode::default(),
            palette: Palette::for_theme(theme),
            theme,
            charset: CharsetState::default(),
            title: String::new(),
            icon_name: String::new(),
            cwd: None,
            title_stack: VecDeque::new(),
            cursor_shape: CursorShape::default(),
            keyboard_mode_stack: VecDeque::new(),
            inactive_keyboard_mode_stack: VecDeque::new(),
            event_listener: listener,
            selection_dirty: false,
            prompt_state: PromptState::None,
            prompt_mark_pending: false,
            prompt_markers: Vec::new(),
            command_start_mark_pending: false,
            output_start_mark_pending: false,
            pending_notifications: Vec::new(),
            command_start: None,
            last_command_duration: None,
            has_explicit_title: false,
            title_dirty: false,
        }
    }

    /// Event listener for terminal events.
    pub fn event_listener(&self) -> &T {
        &self.event_listener
    }

    /// Whether grid content was modified since the last check.
    ///
    /// Set by content-modifying VTE handler operations (character printing,
    /// erase, insert/delete, scroll). The owning layer should check this
    /// after terminal output and clear any active selection when true.
    pub fn is_selection_dirty(&self) -> bool {
        self.selection_dirty
    }

    /// Reset the selection-dirty flag after handling invalidation.
    pub fn clear_selection_dirty(&mut self) {
        self.selection_dirty = false;
    }

    /// Reference to the active grid.
    pub fn grid(&self) -> &Grid {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            &self.alt_grid
        } else {
            &self.grid
        }
    }

    /// Mutable reference to the active grid.
    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            &mut self.alt_grid
        } else {
            &mut self.grid
        }
    }

    /// Current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        self.mode
    }

    /// Reference to the color palette.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Mutable reference to the color palette (for config overrides).
    pub fn palette_mut(&mut self) -> &mut Palette {
        &mut self.palette
    }

    /// Current color theme.
    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Switch the active color theme.
    ///
    /// Rebuilds the palette for the new theme and marks all lines dirty so
    /// the renderer repaints with the new colors. No-op if the theme is
    /// unchanged.
    pub fn set_theme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.palette = Palette::for_theme(theme);
        self.grid_mut().dirty_mut().mark_all();
    }

    /// Current window title (raw OSC 0/2 value).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Current icon name (set by OSC 0/1).
    pub fn icon_name(&self) -> &str {
        &self.icon_name
    }

    /// Current working directory (set by OSC 7).
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    // Shell integration methods (prompt state, CWD, title resolution,
    // notifications, prompt navigation) are in `shell_state.rs`.

    /// Current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Override the cursor shape (config-driven, not VTE-driven).
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
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
            stable_row_base: 0,
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

                let (underline_color, has_hyperlink, zerowidth) = match cell.extra.as_ref() {
                    Some(e) => (
                        e.underline_color.map(|c| palette.resolve(c)),
                        e.hyperlink.is_some(),
                        e.zerowidth.clone(),
                    ),
                    None => (None, false, Vec::new()),
                };

                out.cells.push(RenderableCell {
                    line: vis_line,
                    column: col,
                    ch: cell.ch,
                    fg,
                    bg,
                    flags: cell.flags,
                    underline_color,
                    has_hyperlink,
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

        out.all_dirty = renderable::collect_damage(grid, lines, cols, &mut out.damage);
        out.display_offset = offset;
        let base_abs = grid.scrollback().len().saturating_sub(offset);
        out.stable_row_base = grid.total_evicted() as u64 + base_abs as u64;
        out.mode = self.mode;
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

    /// Resize the terminal to new dimensions.
    ///
    /// Resizes both primary and alternate grids. The primary grid uses text
    /// reflow (soft-wrapped lines re-wrap to fit the new width). The alternate
    /// grid does not reflow (full-screen apps manage their own layout).
    ///
    /// Marks all lines dirty so the renderer repaints. Also marks selection
    /// as dirty since content positions change.
    pub fn resize(&mut self, new_lines: usize, new_cols: usize) {
        if new_lines == 0 || new_cols == 0 {
            return;
        }

        // Primary grid: reflow enabled.
        self.grid.resize(new_lines, new_cols, true);

        // Alternate grid: no reflow (apps like vim handle their own layout).
        self.alt_grid.resize(new_lines, new_cols, false);

        // Mark selection dirty since cell positions changed.
        // Note: both grids are already fully marked dirty by
        // `Grid::finalize_resize` → `dirty.resize()` → `mark_all()`.
        self.selection_dirty = true;
    }

    /// Switch between primary and alternate screen.
    ///
    /// Saves/restores cursor, toggles `TermMode::ALT_SCREEN`, swaps keyboard
    /// mode stacks, and marks all lines dirty. Also marks selection as dirty
    /// since screen content changes completely.
    pub fn swap_alt(&mut self) {
        self.selection_dirty = true;
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
        std::mem::swap(
            &mut self.keyboard_mode_stack,
            &mut self.inactive_keyboard_mode_stack,
        );
        self.grid_mut().dirty_mut().mark_all();
    }
}

/// Extract the last path component from a CWD path for tab display.
///
/// - `/home/user/projects` → `projects`
/// - `/` → `/`
/// - `~` passthrough (shouldn't occur from OSC 7, but handle gracefully).
pub fn cwd_short_path(cwd: &str) -> &str {
    if cwd == "/" {
        return cwd;
    }
    // Strip trailing slash then take last component.
    let trimmed = cwd.strip_suffix('/').unwrap_or(cwd);
    let component = trimmed.rsplit('/').next().unwrap_or(cwd);
    // Paths like `///` reduce to an empty component after stripping — return `/`.
    if component.is_empty() { "/" } else { component }
}

#[cfg(test)]
mod tests;
