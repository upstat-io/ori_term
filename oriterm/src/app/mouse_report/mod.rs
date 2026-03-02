//! Mouse event reporting to the PTY.
//!
//! Encodes mouse events (clicks, motion, scroll) as escape sequences in
//! SGR, UTF-8, or Normal (X10) format, depending on the terminal mode.
//! Also handles alternate scroll (sending arrow keys in alt screen) and
//! motion deduplication.

use std::io::{Cursor, Write};

use winit::dpi::PhysicalPosition;
use winit::event::MouseScrollDelta;

use oriterm_core::TermMode;

use super::App;
use super::mouse_selection::{self, GridCtx};

/// Mouse button for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseButton {
    /// Left button (code 0).
    Left,
    /// Middle button (code 1).
    Middle,
    /// Right button (code 2).
    Right,
    /// No button held (code 3, used for mode 1003 buttonless motion).
    None,
    /// Scroll wheel up (code 64).
    ScrollUp,
    /// Scroll wheel down (code 65).
    ScrollDown,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseEventKind {
    /// Button pressed.
    Press,
    /// Button released.
    Release,
    /// Cursor moved while button held (or any motion in mode 1003).
    Motion,
}

/// Modifier state for mouse reports.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MouseModifiers {
    /// Shift key held.
    pub shift: bool,
    /// Alt/Meta key held.
    pub alt: bool,
    /// Ctrl key held.
    pub ctrl: bool,
}

/// Stack-allocated buffer for encoded mouse report (max 32 bytes).
///
/// Avoids heap allocation in the hot path. All encoding functions
/// write into this buffer via `std::io::Cursor`.
pub(crate) struct MouseReportBuf {
    data: [u8; 32],
    len: usize,
}

impl MouseReportBuf {
    /// Create an empty report buffer.
    fn new() -> Self {
        Self {
            data: [0u8; 32],
            len: 0,
        }
    }

    /// The encoded bytes, or empty if encoding failed.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

// Encoding functions (pure, zero-allocation, tested in isolation).

/// Compute the base button code for a mouse report.
///
/// Left=0, Middle=1, Right=2, ScrollUp=64, ScrollDown=65.
/// Motion adds 32 to the base code.
fn button_code(button: MouseButton, kind: MouseEventKind) -> u8 {
    let base = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::None => 3,
        MouseButton::ScrollUp => 64,
        MouseButton::ScrollDown => 65,
    };
    if kind == MouseEventKind::Motion {
        base + 32
    } else {
        base
    }
}

/// Apply modifier bits to a button code.
///
/// Shift=+4, Alt=+8, Ctrl=+16.
fn apply_modifiers(code: u8, mods: MouseModifiers) -> u8 {
    let mut result = code;
    if mods.shift {
        result += 4;
    }
    if mods.alt {
        result += 8;
    }
    if mods.ctrl {
        result += 16;
    }
    result
}

/// Encode a mouse event in SGR format.
///
/// Format: `\x1b[<code;col+1;line+1{M|m}`
/// Uses `M` for press/motion, `m` for release. Coordinates are 1-indexed.
/// Returns the number of bytes written.
fn encode_sgr(buf: &mut [u8], code: u8, col: usize, line: usize, pressed: bool) -> usize {
    let suffix = if pressed { 'M' } else { 'm' };
    let mut cursor = Cursor::new(buf);
    // write! on Cursor<&mut [u8]> returns io::Error on overflow — treat as 0.
    let Ok(()) = write!(cursor, "\x1b[<{code};{};{}{suffix}", col + 1, line + 1) else {
        return 0;
    };
    cursor.position() as usize
}

/// Write a single coordinate in the UTF-8 mouse encoding.
///
/// Values < 128 use a single byte. Values 128–2047 use a custom 2-byte
/// encoding. Values > 2047 are out of range and return `false`.
fn write_utf8_coord(cursor: &mut Cursor<&mut [u8]>, pos: usize) -> bool {
    let val = 32 + 1 + pos as u32;
    if val < 128 {
        cursor.write_all(&[val as u8]).is_ok()
    } else if val <= 0x7FF {
        let first = (0xC0 + val / 64) as u8;
        let second = (0x80 + (val & 63)) as u8;
        cursor.write_all(&[first, second]).is_ok()
    } else {
        false
    }
}

/// Encode a mouse event in UTF-8 extended format.
///
/// Format: `\x1b[M` + button byte + col byte(s) + line byte(s).
/// Coordinates use a custom 2-byte encoding for values >= 95.
/// Returns 0 if coordinates are out of range (> 2015).
fn encode_utf8(buf: &mut [u8], code: u8, col: usize, line: usize) -> usize {
    let mut cursor = Cursor::new(buf);
    let Ok(()) = cursor.write_all(b"\x1b[M") else {
        return 0;
    };

    // Button byte: always 32 + code (single byte).
    let btn = 32u32 + u32::from(code);
    if btn > 127 {
        return 0;
    }
    let Ok(()) = cursor.write_all(&[btn as u8]) else {
        return 0;
    };

    // Encode each coordinate.
    for pos in [col, line] {
        if !write_utf8_coord(&mut cursor, pos) {
            return 0;
        }
    }

    cursor.position() as usize
}

/// Encode a mouse event in URXVT format.
///
/// Format: `\x1b[Cb;Cx;CyM` where Cb = 32 + button code,
/// Cx/Cy are 1-indexed decimal. No press/release distinction
/// (all events use `M` suffix).
fn encode_urxvt(buf: &mut [u8], code: u8, col: usize, line: usize) -> usize {
    let cb = 32 + u32::from(code);
    let mut cursor = Cursor::new(buf);
    let Ok(()) = write!(cursor, "\x1b[{cb};{};{}M", col + 1, line + 1) else {
        return 0;
    };
    cursor.position() as usize
}

/// Encode a mouse event in Normal (X10) format.
///
/// Format: `\x1b[M` + 3 bytes (button, col, line).
/// Returns 0 (drops the event) if either coordinate exceeds 222,
/// since 32 + 1 + 222 = 255 is the max encodable `u8` value.
/// Sending a clamped coordinate would report a wrong position.
fn encode_normal(buf: &mut [u8], code: u8, col: usize, line: usize) -> usize {
    if col > 222 || line > 222 {
        return 0;
    }

    let btn = 32 + code;
    let cx = (32 + 1 + col) as u8;
    let cy = (32 + 1 + line) as u8;

    let mut cursor = Cursor::new(buf);
    let Ok(()) = cursor.write_all(&[0x1b, b'[', b'M', btn, cx, cy]) else {
        return 0;
    };
    cursor.position() as usize
}

/// Input parameters for [`encode_mouse_event`].
pub(crate) struct MouseEvent {
    /// Which button (or scroll direction).
    pub button: MouseButton,
    /// Press, release, or motion.
    pub kind: MouseEventKind,
    /// Grid column (0-indexed).
    pub col: usize,
    /// Grid line (0-indexed).
    pub line: usize,
    /// Modifier keys held during the event.
    pub mods: MouseModifiers,
}

/// Encode a mouse event, selecting the format based on terminal mode.
///
/// Priority: SGR > URXVT > UTF-8 > Normal. Returns the encoded bytes in
/// the buffer.
pub(crate) fn encode_mouse_event(event: &MouseEvent, mode: TermMode) -> MouseReportBuf {
    let mut buf = MouseReportBuf::new();
    let code = apply_modifiers(button_code(event.button, event.kind), event.mods);
    let pressed = event.kind != MouseEventKind::Release;

    buf.len = if mode.contains(TermMode::MOUSE_SGR) {
        encode_sgr(&mut buf.data, code, event.col, event.line, pressed)
    } else if mode.contains(TermMode::MOUSE_URXVT) {
        encode_urxvt(&mut buf.data, code, event.col, event.line)
    } else if mode.contains(TermMode::MOUSE_UTF8) {
        encode_utf8(&mut buf.data, code, event.col, event.line)
    } else {
        // Normal (X10) format: release uses code 3 (+ modifiers).
        let code = if event.kind == MouseEventKind::Release {
            apply_modifiers(3, event.mods)
        } else {
            code
        };
        encode_normal(&mut buf.data, code, event.col, event.line)
    };

    buf
}

impl App {
    /// Whether mouse events should be reported to the PTY for the given mode.
    ///
    /// True when any mouse reporting mode is active and Shift is NOT held.
    /// Shift-bypass lets users select text even when the terminal app has
    /// requested mouse reporting.
    ///
    /// Pure check — does not lock the terminal. Caller reads mode once via
    /// [`terminal_mode`](App::terminal_mode) and passes it through.
    pub(super) fn should_report_mouse(&self, mode: TermMode) -> bool {
        !self.modifiers.shift_key() && mode.intersects(TermMode::ANY_MOUSE)
    }

    /// Encode and send a mouse button event to the PTY.
    ///
    /// Encodes the event using the provided terminal mode, then writes to
    /// the PTY. No-op if the cursor is outside the grid.
    pub(super) fn report_mouse_button(
        &self,
        button: MouseButton,
        kind: MouseEventKind,
        mode: TermMode,
    ) {
        let Some((col, line)) = self.mouse_cell() else {
            return;
        };

        let Some(pane) = self.active_pane() else {
            return;
        };
        let event = MouseEvent {
            button,
            kind,
            col,
            line,
            mods: self.mouse_modifiers(),
        };
        let report = encode_mouse_event(&event, mode);
        let bytes = report.as_bytes();
        if !bytes.is_empty() {
            pane.write_input(bytes);
        }
    }

    /// Report mouse motion to the PTY when tracking mode is active.
    ///
    /// Performs motion deduplication: only sends a report when the cell
    /// changes. Returns `true` if motion was reported (caller should
    /// skip selection handling).
    pub(super) fn report_mouse_motion(
        &mut self,
        position: PhysicalPosition<f64>,
        mode: TermMode,
    ) -> bool {
        let has_drag = mode.contains(TermMode::MOUSE_DRAG) && self.mouse.any_button_down();
        let has_motion = mode.contains(TermMode::MOUSE_MOTION);

        if !has_drag && !has_motion {
            return false;
        }

        // Shift-bypass: let user select text.
        if self.modifiers.shift_key() {
            return false;
        }

        let Some((col, line)) = self.pixel_to_cell(position) else {
            return false;
        };

        // Motion deduplication: skip if same cell as last report.
        if self.mouse.last_reported_cell() == Some((col, line)) {
            return false;
        }
        self.mouse.set_last_reported_cell(Some((col, line)));

        // Drag (button held) uses the actual button code; mode 1003 motion
        // without a button uses None (code 3+32 = 35).
        // Priority: left > middle > right (matches Alacritty).
        let button = if self.mouse.left_down() {
            MouseButton::Left
        } else if self.mouse.middle_down() {
            MouseButton::Middle
        } else if self.mouse.right_down() {
            MouseButton::Right
        } else {
            MouseButton::None
        };
        let event = MouseEvent {
            button,
            kind: MouseEventKind::Motion,
            col,
            line,
            mods: self.mouse_modifiers(),
        };
        let report = encode_mouse_event(&event, mode);
        let bytes = report.as_bytes();
        if !bytes.is_empty() {
            if let Some(pane) = self.active_pane() {
                pane.write_input(bytes);
            }
        }
        true
    }

    /// Handle mouse wheel with 3-tier priority.
    ///
    /// 1. Mouse reporting mode active → send scroll events to PTY.
    /// 2. Alt screen + `ALTERNATE_SCROLL` → send arrow keys to PTY.
    /// 3. Normal → smooth viewport scroll.
    pub(super) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta, mode: TermMode) {
        let cell_height = self
            .renderer
            .as_ref()
            .map_or(16.0, |r| r.cell_metrics().height);
        let Some((lines, scroll_up)) = parse_wheel_delta(delta, cell_height) else {
            return;
        };

        let Some(pane) = self.active_pane() else {
            return;
        };

        // Tier 1: Mouse reporting.
        if mode.intersects(TermMode::ANY_MOUSE) && !self.modifiers.shift_key() {
            let button = if scroll_up {
                MouseButton::ScrollUp
            } else {
                MouseButton::ScrollDown
            };
            let Some((col, line)) = self.mouse_cell_clamped() else {
                return;
            };
            let event = MouseEvent {
                button,
                kind: MouseEventKind::Press,
                col,
                line,
                mods: self.mouse_modifiers(),
            };

            for _ in 0..lines {
                let report = encode_mouse_event(&event, mode);
                let bytes = report.as_bytes();
                if !bytes.is_empty() {
                    pane.write_input(bytes);
                }
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
            return;
        }

        // Tier 2: Alternate scroll (arrow keys in alt screen).
        if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
            && !self.modifiers.shift_key()
        {
            let arrow = if scroll_up { b"\x1bOA" } else { b"\x1bOB" };
            for _ in 0..lines {
                pane.write_input(arrow);
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
            return;
        }

        // Tier 3: Discrete viewport scroll.
        let scroll_lines = if scroll_up {
            lines as isize
        } else {
            -(lines as isize)
        };
        pane.scroll_display(scroll_lines);

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Convert the current cursor position to a grid cell.
    fn mouse_cell(&self) -> Option<(usize, usize)> {
        self.pixel_to_cell(self.mouse.cursor_pos())
    }

    /// Convert the current cursor position to a grid cell, clamping to edges.
    ///
    /// Unlike [`mouse_cell`], this never returns `None` when the grid and
    /// renderer are available — positions outside the grid are clamped to
    /// the nearest edge cell. Returns `None` only if the grid widget or
    /// renderer is missing.
    fn mouse_cell_clamped(&self) -> Option<(usize, usize)> {
        let wctx = self.focused_ctx()?;
        let renderer = self.renderer.as_ref()?;
        let ctx = GridCtx {
            widget: &wctx.terminal_grid,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };
        let pos = self.mouse.cursor_pos();

        // Fast path: position is inside the grid.
        if let Some(cell) = mouse_selection::pixel_to_cell(pos, &ctx) {
            return Some(cell);
        }

        // Clamp to edge: compute the nearest valid cell.
        let bounds = ctx.widget.bounds()?;
        let cw = f64::from(ctx.cell.width);
        let ch = f64::from(ctx.cell.height);
        if cw <= 0.0 || ch <= 0.0 {
            return None;
        }
        let max_col = ((f64::from(bounds.width()) / cw) as usize).saturating_sub(1);
        let max_line = ((f64::from(bounds.height()) / ch) as usize).saturating_sub(1);

        let col = if pos.x < f64::from(bounds.x()) {
            0
        } else {
            (((pos.x - f64::from(bounds.x())) / cw) as usize).min(max_col)
        };
        let line = if pos.y < f64::from(bounds.y()) {
            0
        } else {
            (((pos.y - f64::from(bounds.y())) / ch) as usize).min(max_line)
        };
        Some((col, line))
    }

    /// Convert a pixel position to a grid cell, using grid context.
    fn pixel_to_cell(&self, pos: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        let wctx = self.focused_ctx()?;
        let renderer = self.renderer.as_ref()?;
        let ctx = GridCtx {
            widget: &wctx.terminal_grid,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };
        mouse_selection::pixel_to_cell(pos, &ctx)
    }

    /// Build modifier state from the current winit modifiers.
    fn mouse_modifiers(&self) -> MouseModifiers {
        MouseModifiers {
            shift: self.modifiers.shift_key(),
            alt: self.modifiers.alt_key(),
            ctrl: self.modifiers.control_key(),
        }
    }
}

/// Parse a mouse wheel delta into `(line_count, scroll_up)`.
///
/// Winit's `LineDelta` reports raw notches (1.0 per notch) without applying
/// the OS scroll lines setting. We multiply by the platform's configured
/// lines-per-notch (e.g. 3 on Windows) so scrolling respects the user's
/// system preference.
///
/// Returns `None` if the delta is too small to register.
fn parse_wheel_delta(delta: MouseScrollDelta, cell_height: f32) -> Option<(usize, bool)> {
    let (lines, scroll_up) = match delta {
        MouseScrollDelta::LineDelta(_, y) => {
            if y == 0.0 {
                return None;
            }
            let os_lines = crate::platform::scroll::wheel_scroll_lines() as f32;
            let scaled = y.abs() * os_lines;
            ((scaled.ceil() as usize).max(1), y > 0.0)
        }
        MouseScrollDelta::PixelDelta(pos) => {
            let y = pos.y;
            if y.abs() < f64::from(cell_height) / 2.0 {
                return None;
            }
            ((y.abs() / f64::from(cell_height)).ceil() as usize, y > 0.0)
        }
    };
    Some((lines, scroll_up))
}

#[cfg(test)]
mod tests;
