---
section: 22
title: Terminal Modes
status: mostly-complete
tier: 5
goal: Comprehensive DECSET/DECRST mode support, mode interactions, image protocol
sections:
  - id: "22.1"
    title: Mouse Reporting Modes
    status: complete
  - id: "22.2"
    title: Cursor Styles
    status: complete
  - id: "22.3"
    title: Focus Events
    status: complete
  - id: "22.4"
    title: Synchronized Output
    status: complete
  - id: "22.5"
    title: Hyperlinks
    status: complete
  - id: "22.6"
    title: Comprehensive Mode Table
    status: complete
  - id: "22.7"
    title: Image Protocol
    status: not-started
  - id: "22.8"
    title: Section Completion
    status: in-progress
---

# Section 22: Terminal Modes

**Status:** Mostly Complete (22.7 Image Protocol deferred to Section 39)
**Goal:** Complete, correct DECSET/DECRST mode support with proper mode interactions, mouse reporting, cursor styles, hyperlinks, and image protocol. This section is the authoritative reference for every terminal mode ori_term must handle.

**Crate:** `oriterm` (binary) and `oriterm_core` (mode flags, state)
**Dependencies:** `vte` (parser + handler), `unicode-width`, `regex` (URL detection)

**Reference:**
- Ghostty's comprehensive mode handling (`modes.zig`) and feature support
- Alacritty's mouse reporting and cursor style support
- WezTerm's image protocol and hyperlink support
- xterm ctlseqs documentation

---

## 22.1 Mouse Reporting Modes

Report mouse events to applications (vim, tmux, htop, etc.). The terminal must support multiple reporting modes and encoding formats, with correct priority and interaction semantics.

**Files:** `oriterm/src/app/mouse_report/mod.rs`, `oriterm_core/src/term/mode/mod.rs`

- [x] Mouse reporting modes (DECSET):
  - [ ] 9: X10 mouse reporting (button press only, legacy)
  - [x] 1000: Normal tracking (press + release)
  - [x] 1002: Button-event tracking (press + release + drag with button held)
  - [x] 1003: Any-event tracking (all motion, even without button held)
  - [x] Modes are mutually exclusive — enabling one disables the others
- [x] Mouse encoding formats:
  - [x] Default: `ESC[M Cb Cx Cy` (X10-compatible, limited to 223 columns/rows)
  - [x] UTF-8 (DECSET 1005): UTF-8 encoded coordinates (extends range)
  - [x] SGR (DECSET 1006): `ESC[<Cb;Cx;Cy M/m` — preferred, no coordinate limit, distinguishes press (`M`) from release (`m`)
  - [x] URXVT (DECSET 1015): `ESC[Cb;Cx;Cy M` — decimal encoding, no release distinction
  - [x] Encoding modes are mutually exclusive — enabling one disables the others
- [x] Button encoding: left=0, middle=1, right=2, wheel up=64, wheel down=65
- [x] Modifier encoding: Shift adds 4, Alt adds 8, Ctrl adds 16 to button byte
- [x] Shift+click bypasses mouse reporting (allows selection even when app captures mouse)
  - [x] When mouse reporting active, normal clicks go to the application
  - [x] When Shift held, clicks go to selection logic instead
- [x] Motion dedup: only report motion when the cell position changes
  - [x] Track `last_mouse_cell: Option<(usize, usize)>` to avoid redundant reports
- [x] Alternate scroll mode (DECSET 1007):
  - [x] When in alternate screen buffer and this mode is set, scroll events are converted to arrow key sequences (Up/Down) instead of being reported as mouse scroll
  - [x] Enables scrolling in programs like `less`, `man` that don't handle mouse scroll
- [x] `TermMode::ANY_MOUSE` helper constant — union of all mouse reporting mode flags
- [x] `TermMode::ANY_MOUSE_ENCODING` helper constant — union of all encoding mode flags
- [x] **Tests:**
  - [x] Enabling mode 1003 disables 1000 and 1002
  - [x] Enabling mode 1002 disables 1000 and 1003
  - [x] SGR encoding produces correct escape sequence for button press and release
  - [x] Default encoding clamps coordinates to 222
  - [x] Shift+click flag bypasses mouse reporting
  - [x] Motion dedup suppresses duplicate cell positions
  - [x] Alternate scroll converts wheel to arrow keys in alt screen
  - [x] URXVT encoding produces correct `ESC[Cb;Cx;CyM` sequences
  - [x] Encoding priority: SGR > URXVT > UTF-8 > Normal
  - [x] Mouse encoding mutual exclusion (1006 clears 1005/1015, etc.)

---

## 22.2 Cursor Styles

Support different cursor shapes, blinking, and cursor color.

**Files:** `oriterm_core/src/grid/cursor/mod.rs`, `oriterm/src/gpu/renderer.rs`

- [x] Cursor shapes via DECSCUSR (CSI Ps SP q):
  - [x] 0: default (reset to config default, typically blinking block)
  - [x] 1: blinking block
  - [x] 2: steady block
  - [x] 3: blinking underline
  - [x] 4: steady underline
  - [x] 5: blinking bar (I-beam)
  - [x] 6: steady bar
- [x] Store cursor shape in terminal state
- [x] Render cursor according to shape (block, underline, bar, hollow block)
- [x] Blinking: toggle cursor visibility on a timer
- [x] OSC 12: set cursor color
- [x] Save/restore cursor style with DECSC/DECRC
- [x] **Tests:**
  - [x] DECSCUSR 0 resets to default shape
  - [x] DECSCUSR 1-6 sets correct shape and blink flag
  - [x] Save/restore round-trips cursor position, shape, and template

---

## 22.3 Focus Events

Report window focus changes to applications that request them.

**Files:** `oriterm/src/app/mod.rs`, `oriterm_core/src/term/mode/mod.rs`

- [x] DECSET 1004: enable focus event reporting
- [x] When window gains focus: send `ESC[I` to PTY
- [x] When window loses focus: send `ESC[O` to PTY
- [x] Handle winit `WindowEvent::Focused(bool)` in event loop
- [x] Only send focus events when the mode flag is set
- [x] **Tests:**
  - [x] Focus event mode flag toggles correctly with DECSET/DECRST 1004

---

## 22.4 Synchronized Output

Prevent partial frame rendering during rapid output.

- [x] Mode 2026 (SyncUpdate): handled internally by vte 0.15 `Processor`
  - [x] vte buffers handler calls between BSU and ESU, dispatching as one batch
- [x] Explicit documentation in mode handler noting Mode 2026 is handled by vte
- [x] **Tests:**
  - [x] Verify that vte processes BSU/ESU sequences without error

---

## 22.5 Hyperlinks

OSC 8 hyperlink support for clickable URLs, plus implicit URL detection.

**Files:** `oriterm_core/src/cell/mod.rs`, `oriterm/src/url_detect.rs`, `oriterm/src/app/mouse_selection/mod.rs`

- [x] Parse OSC 8 sequences (handled by vte)
- [x] Store hyperlink in `CellExtra`
- [x] Rendering: hyperlinked text with underline
- [x] Mouse hover detection (Ctrl + cursor over link → pointer cursor)
- [x] Ctrl+click: open URL in default browser
- [x] Implicit URL detection (regex-based)
- [x] **Tests:**
  - [x] OSC 8 start/end correctly sets and clears hyperlink on cells
  - [x] URL scheme validation
  - [x] Implicit URL regex matches

---

## 22.6 Comprehensive Mode Table

Complete reference of every DECSET/DECRST private mode.

**Files:** `oriterm_core/src/term/mode/mod.rs`, `oriterm_core/src/term/handler/modes.rs`

### Private Modes (DECSET/DECRST — `CSI ? Pm h` / `CSI ? Pm l`)

| Mode | Name | Status |
|------|------|--------|
| 1 | DECCKM (app cursor keys) | [x] |
| 6 | DECOM (origin mode) | [x] |
| 7 | DECAWM (auto-wrap) | [x] |
| 9 | X10 Mouse | [ ] |
| 12 | ATT610 (cursor blinking) | [x] |
| 25 | DECTCEM (show cursor) | [x] |
| 45 | Reverse Wraparound | [x] |
| 47 | Alt Screen (legacy, no cursor save) | [x] |
| 1000 | Normal Mouse | [x] |
| 1002 | Button Mouse | [x] |
| 1003 | Any Mouse | [x] |
| 1004 | Focus Events | [x] |
| 1005 | UTF-8 Mouse | [x] |
| 1006 | SGR Mouse | [x] |
| 1007 | Alt Scroll | [x] |
| 1015 | URXVT Mouse | [x] |
| 1042 | Urgency Hints | [x] |
| 1047 | Alt Screen (clear on enter) | [x] |
| 1048 | Save Cursor | [x] |
| 1049 | Alt Screen (save/restore cursor) | [x] |
| 2004 | Bracketed Paste | [x] |
| 2026 | Sync Output | [x] |

### Standard Modes (`CSI Pm h` / `CSI Pm l`)

| Mode | Name | Status |
|------|------|--------|
| 4 | IRM (Insert/Replace) | [x] |
| 20 | LNM (Linefeed/New Line) | [x] |

### Application Keypad (DECKPAM/DECKPNM)

- [x] `ESC =` (DECKPAM): Application keypad mode
- [x] `ESC >` (DECKPNM): Normal keypad mode
- [x] Stored as `TermMode::APP_KEYPAD`

### DECALN — Screen Alignment Test (`ESC # 8`)

- [x] `ESC # 8` (DECALN): fill entire screen with 'E' characters

### Mode Interactions

- [x] Mouse modes (1000, 1002, 1003) are mutually exclusive
- [x] Mouse encoding modes (1005, 1006, 1015) are mutually exclusive
- [x] Alt screen swap (1049) saves/restores cursor and keyboard mode stack
- [x] DECTCEM (25) is independent of alt screen
- [x] Origin mode (6) interacts with scroll regions
- [x] Mode 47 swaps without cursor save/restore
- [x] Mode 1047 clears alt screen on enter, no cursor save/restore
- [x] Mode 1048 saves/restores cursor standalone

### Save/Restore Modes (XTSAVE/XTRESTORE)

- [x] `CSI ? Pm s` (XTSAVE): save current state of mode `Pm`
- [x] `CSI ? Pm r` (XTRESTORE): restore previously saved state of mode `Pm`
- [x] Stored in `HashMap<u16, bool>` (single save per mode, no stack)
- [x] Cleared on RIS (full reset)

### Reverse Wraparound (Mode 45)

- [x] BS at column 0 wraps to last column of previous line if soft-wrapped
- [x] No-op if previous line was not soft-wrapped (hard newline)
- [x] Disabled by default (opt-in via DECSET 45)

### Implementation Checklist

- [x] Define all modes as constants/flags in `TermMode`
- [x] `set_private_mode` handles all modes in the table above
- [x] `unset_private_mode` handles all modes in the table above
- [x] Mode interactions enforced (mutual exclusion, alt screen save/restore)
- [x] XTSAVE/XTRESTORE implemented for all applicable modes
- [x] Unknown modes logged at debug level and ignored (no panic)
- [x] **Tests:**
  - [x] Setting each mode flag and verifying it is set
  - [x] Mutual exclusion: setting mode 1003 clears 1000 and 1002
  - [x] Encoding mutual exclusion: setting mode 1006 clears 1005 and 1015
  - [x] Alt screen enter/exit saves and restores mode state (mode 1049)
  - [x] Mode 47 swaps without cursor save
  - [x] Mode 1047 clears alt on enter
  - [x] Mode 1048 saves/restores cursor
  - [x] XTSAVE/XTRESTORE round-trip for mode 25
  - [x] XTRESTORE without save is no-op
  - [x] Multiple modes saved/restored independently
  - [x] RIS clears saved private modes
  - [x] Reverse wraparound wraps at col 0 of soft-wrapped line
  - [x] Reverse wraparound is no-op at col 0 of hard-newline line
  - [x] Unknown mode number does not panic

---

## 22.7 Image Protocol

**Moved to Section 39.** Image protocol support (Kitty Graphics, Sixel, iTerm2) is now a dedicated section with full design detail. See `section-39-image-protocols.md`.

- [ ] Section 39 complete (Kitty Graphics + Sixel + iTerm2 image protocols)

---

## 22.8 Section Completion

- [x] All 22.1-22.6 items complete
- [ ] 22.7 Image Protocol (deferred to Section 39)
- [x] Mouse reporting works with all encoding formats (SGR, URXVT, UTF-8, Normal)
- [x] Mouse mode mutual exclusion enforced
- [x] Mouse encoding mutual exclusion enforced
- [x] SGR mouse encoding supported (no coordinate limits)
- [x] Shift+click bypasses mouse reporting for selection
- [x] Cursor shape changes work (block, underline, bar) with blinking
- [x] Focus events sent when window gains/loses focus
- [x] Synchronized output prevents flicker (vte handles internally)
- [x] OSC 8 hyperlinks render and are clickable (Ctrl+click)
- [x] Implicit URL detection works on plain-text URLs
- [x] All modes in the comprehensive mode table are implemented (except X10 mode 9)
- [x] Mode interactions (mutual exclusion, alt screen save/restore) are correct
- [x] XTSAVE/XTRESTORE work for applicable modes
- [x] Legacy alt screen modes (47, 1047, 1048) implemented
- [x] Reverse wraparound (mode 45) implemented
- [x] `cargo test` — all mode tests pass
- [x] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings

**Exit Criteria:** Every mode in the comprehensive mode table is implemented and tested. tmux, vim, htop, and other TUI applications have fully working mode support including mouse, cursor styles, and focus events.
