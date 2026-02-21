---
section: 8
title: Keyboard Input
status: in-progress
tier: 3
goal: Legacy + Kitty keyboard encoding, keyboard dispatch, IME support
sections:
  - id: "8.1"
    title: Legacy Key Encoding
    status: complete
  - id: "8.2"
    title: Kitty Keyboard Protocol
    status: complete
  - id: "8.3"
    title: Keyboard Input Dispatch
    status: in-progress
  - id: "8.4"
    title: Section Completion
    status: in-progress
---

# Section 08: Keyboard Input

**Status:** Not Started
**Goal:** Encode all key events correctly for terminal applications using both legacy xterm/VT sequences and the Kitty keyboard protocol. Wire keyboard dispatch through keybindings to PTY output with IME support for CJK input.

**Crate:** `oriterm` (binary)
**Dependencies:** `winit` (key events), `oriterm_core` (TermMode)
**Reference:** `_old/src/key_encoding/legacy.rs`, `_old/src/key_encoding/kitty.rs`, `_old/src/app/input_keyboard.rs`

**Prerequisite:** Section 03 complete (PTY running, event loop accepting input). Section 02 complete (TermMode flags available for mode-dependent encoding).

---

## 8.1 Legacy Key Encoding

Correctly encode all key events for the terminal using legacy xterm/VT sequences. This is the baseline encoding used when Kitty protocol is not active.

**File:** `oriterm/src/key_encoding/legacy.rs`

**Reference:** `_old/src/key_encoding/legacy.rs` — carries forward the proven LetterKey/TildeKey dispatch tables.

- [x] `encode_legacy(key: &KeyEvent, mode: TermMode) -> Option<Vec<u8>>`
  - [x] Main entry point: inspects key, modifiers, and terminal mode to produce the byte sequence
  - [x] Returns `None` for keys that should not be sent (e.g., bare modifier keys)
- [x] **Regular text input**:
  - [x] Printable characters: send UTF-8 bytes directly
  - [x] Enter: send `\r` (or `\r\n` if LINEFEED_MODE active)
  - [x] Backspace: send `\x7f` (DEL) by default, `\x08` (BS) if DECBKM active
  - [x] Tab: send `\t`; Shift+Tab (backtab): send `ESC[Z`
  - [x] Escape: send `\x1b`
  - [x] Space: send `\x20`; Ctrl+Space: send `\x00` (NUL)
- [x] **Arrow keys** (mode-dependent DECCKM):
  - [x] Normal mode: `ESC[A` (Up), `ESC[B` (Down), `ESC[C` (Right), `ESC[D` (Left)
  - [x] Application cursor mode (DECCKM): `ESCOA`, `ESCOB`, `ESCOC`, `ESCOD`
  - [x] Modifiers override SS3 to CSI format: Ctrl+Up = `ESC[1;5A` (even in app mode)
- [x] **Home/End**:
  - [x] Normal: `ESC[H` / `ESC[F`
  - [x] Application cursor mode: `ESCOH` / `ESCOF`
  - [x] With modifiers: `ESC[1;{mod}H` / `ESC[1;{mod}F`
- [x] **PageUp/PageDown, Insert/Delete**:
  - [x] PageUp: `ESC[5~`, PageDown: `ESC[6~`
  - [x] Insert: `ESC[2~`, Delete: `ESC[3~`
  - [x] With modifiers: `ESC[5;{mod}~` etc.
- [x] **Function keys F1-F12**:
  - [x] F1-F4 use SS3: `ESCOP`, `ESCOQ`, `ESCOR`, `ESCOS`
  - [x] F5-F12 use tilde: `ESC[15~`, `ESC[17~`, `ESC[18~`, `ESC[19~`, `ESC[20~`, `ESC[21~`, `ESC[23~`, `ESC[24~`
  - [x] With modifiers: F1 = `ESC[1;{mod}P`, F5 = `ESC[15;{mod}~`
- [x] **Ctrl+letter** — send C0 control codes:
  - [x] Ctrl+A = `\x01`, Ctrl+B = `\x02`, ..., Ctrl+Z = `\x1A`
  - [x] Ctrl+[ = `\x1b` (ESC), Ctrl+] = `\x1d`, Ctrl+\ = `\x1c`
  - [x] Ctrl+/ = `\x1f`, Ctrl+@ = `\x00`
- [x] **Alt+key** — ESC prefix:
  - [x] Alt+a = `\x1b a`, Alt+A = `\x1b A`
  - [x] Alt+Backspace = `\x1b \x7f`
  - [x] Alt+Ctrl combinations: ESC prefix + C0 byte (Alt+Ctrl+A = `\x1b \x01`)
- [x] **Modifier parameter encoding** (for named keys with modifiers):
  - [x] Parameter = `1 + modifier_bits` where Shift=1, Alt=2, Ctrl=4, Super=8
  - [x] Example: Ctrl+Shift+Up = `ESC[1;6A` (1 + 1 + 4 = 6)
- [x] **Application keypad mode** (DECKPAM/DECKPNM):
  - [x] Normal: numpad keys send their character values
  - [x] Application: numpad sends `ESCOp` through `ESCOy`, Enter = `ESCOM`, operators `ESCOk/m/j/n`
- [x] Helper structs:
  - [x] `LetterKey { term: u8, needs_app_cursor: bool }` — named key with letter terminator
  - [x] `TildeKey { num: u8 }` — named key with tilde terminator
  - [x] `fn letter_key(key: NamedKey) -> Option<LetterKey>` — lookup table
  - [x] `fn tilde_key(key: NamedKey) -> Option<TildeKey>` — lookup table
  - [x] `fn ctrl_key_byte(key: &Key) -> Option<u8>` — Ctrl+key to C0 byte
- [x] **Tests** (`oriterm/src/key_encoding/tests.rs`):
  - [x] Arrow Up in normal mode produces `ESC[A`
  - [x] Arrow Up in application cursor mode produces `ESCOA`
  - [x] Ctrl+Up produces `ESC[1;5A` (modifier parameter)
  - [x] Ctrl+C produces `\x03`
  - [x] Ctrl+A produces `\x01`
  - [x] Alt+A produces `\x1b A`
  - [x] Alt+Backspace produces `\x1b \x7f`
  - [x] Shift+Tab produces `ESC[Z`
  - [x] Shift+F5 produces `ESC[15;2~`
  - [x] Home in normal mode produces `ESC[H`
  - [x] Home in application cursor mode produces `ESCOH`
  - [x] F1 produces `ESCOP`, F5 produces `ESC[15~`
  - [x] Enter produces `\r`
  - [x] Numpad in application keypad mode sends ESC O sequences

---

## 8.2 Kitty Keyboard Protocol

Progressive enhancement keyboard protocol for modern terminal applications. Encodes keys in CSI u format with mode-dependent behavior.

**File:** `oriterm/src/key_encoding/kitty.rs`

**Reference:** `_old/src/key_encoding/kitty.rs`, Kitty keyboard protocol specification (https://sw.kovidgoyal.net/kitty/keyboard-protocol/), Ghostty `src/input/key_encode.zig` (Kitty + legacy encoding), Alacritty `alacritty_terminal/src/term/mod.rs` (key input handling)

- [x] `encode_kitty(input: &KeyInput) -> Vec<u8>`
  - [x] Main entry point: encodes key events using CSI u format
  - [x] Reads mode flags from `TermMode` bitflags
  - [x] Returns empty `Vec` for plain printable chars without modifiers when `REPORT_ALL_KEYS` inactive
- [x] **Mode flags** (5 progressive enhancement levels):
  - [x] Bit 0 — `DISAMBIGUATE_ESC_CODES` (1): use CSI u for ambiguous keys
  - [x] Bit 1 — `REPORT_EVENT_TYPES` (2): report press/repeat/release
  - [x] Bit 2 — `REPORT_ALTERNATE_KEYS` (4): report shifted/base key variants
  - [x] Bit 3 — `REPORT_ALL_KEYS_AS_ESC` (8): encode all keys including plain text as CSI u
  - [x] Bit 4 — `REPORT_ASSOCIATED_TEXT` (16): report text generated by key
- [x] **CSI u encoding format**: `ESC [ keycode ; modifiers u`
  - [x] Extended: `ESC [ keycode ; modifiers : event_type u`
  - [ ] With text: `ESC [ keycode ; modifiers : event_type ; text u`
- [x] **Keycode mapping** (`fn kitty_codepoint(key: NamedKey) -> Option<u32>`):
  - [x] Escape=27, Enter=13, Tab=9, Backspace=127
  - [x] Insert=57348, Delete=57349, Left=57350, Right=57351, Up=57352, Down=57353
  - [x] PageUp=57354, PageDown=57355, Home=57356, End=57357
  - [x] F1=57364 through F35=57398
  - [x] CapsLock=57358, ScrollLock=57359, NumLock=57360
  - [x] Character keys: use Unicode codepoint directly
- [x] **Modifier encoding**:
  - [x] Modifier parameter = `1 + bits` where Shift=1, Alt=2, Ctrl=4, Super=8
  - [x] Omit modifier parameter if value is 1 (no modifiers) and no event type needed
- [x] **Event types** (when `REPORT_EVENT_TYPES` active):
  - [x] 1 = press (omitted as default when `REPORT_EVENT_TYPES` not active)
  - [x] 2 = repeat
  - [x] 3 = release
  - [x] Format: `ESC [ keycode ; modifiers : event_type u`
  - [x] Key release events pass through app shortcuts to PTY when `REPORT_EVENT_TYPES` active
- [x] **Mode stack management** (wired through VTE Handler trait):
  - [x] `push_keyboard_mode(mode)` — push onto stack, apply
  - [x] `pop_keyboard_modes(n)` — pop n entries, apply top or clear
  - [x] `set_keyboard_mode(mode, behavior)` — Replace/Union/Difference on top
  - [x] `report_keyboard_mode()` — respond `ESC[?{bits}u`
  - [x] Stack save/restore on alt screen switch
  - [x] Stack clear on terminal reset
- [x] **Tests** (`oriterm/src/key_encoding/tests.rs`):
  - [x] `'a'` with mode 1 (disambiguate): plain `a` (no encoding needed, not ambiguous)
  - [x] Ctrl+A with mode 1: `ESC[97;5u` (codepoint 97, modifier 5)
  - [x] Enter with mode 1: `ESC[13u` (disambiguated from legacy)
  - [x] Escape with mode 1: `ESC[27u`
  - [x] Key release with mode 2: `ESC[97;1:3u` (event type 3)
  - [x] Key repeat with mode 2: `ESC[97;1:2u`
  - [x] `'a'` with mode 8 (report all): `ESC[97u`
  - [x] F1 with mode 1: `ESC[57364u`
  - [x] Shift+A with mode 1: `ESC[65;2u`

---

## 8.3 Keyboard Input Dispatch

Route keyboard events through keybindings, then through key encoding, then to the PTY. Single decision tree: each input event handled by exactly one handler.

**File:** `oriterm/src/app/mod.rs` (keyboard dispatch in `handle_keyboard_input`)

**Reference:** `_old/src/app/input_keyboard.rs`

- [x] `handle_keyboard_input(&mut self, event: &KeyEvent)`
  - [x] Main entry point called from the winit event loop on `WindowEvent::KeyboardInput`
- [x] **Dispatch priority** (first match wins):
  1. [ ] Check keybindings table: if key+modifiers match a bound action, execute the action and return <!-- blocked-by:13 -->
  2. [x] Check Kitty keyboard mode on active tab:
     - [x] Read `keyboard_mode_stack` from active tab's terminal state
     - [x] If Kitty mode active: call `encode_kitty()`, send result to PTY
     - [x] If REPORT_EVENT_TYPES active: also send release events
  3. [x] Fall through to legacy encoding:
     - [x] Call `encode_legacy()`, send result to PTY
  4. [x] If encoding returns None: key not handled (bare modifier press, etc.)
- [x] **Cursor blink reset**:
  - [x] On any keypress that sends to PTY: reset cursor blink timer (cursor becomes visible)
- [x] **Scroll to bottom on input**:
  - [x] If display_offset > 0 (viewing scrollback): scroll to live position on keypress
- [ ] **Smart Ctrl+C**: <!-- blocked-by:9 -->
  - [ ] If selection exists and Ctrl+C pressed: copy selection to clipboard, do NOT send SIGINT
  - [ ] If no selection and Ctrl+C pressed: send `\x03` to PTY
- [x] **IME handling** (`WindowEvent::Ime`):
  - [x] `Ime::Commit(text)`: send committed text bytes to PTY
  - [ ] `Ime::Preedit(text, cursor)`: display composition text at cursor position (overlay rendering) <!-- blocked-by:9 -->
  - [ ] `Ime::Enabled` / `Ime::Disabled`: track IME state, suppress raw key events during composition <!-- blocked-by:9 -->
  - [ ] Position IME candidate window near terminal cursor (call `window.set_ime_cursor_area()`) <!-- blocked-by:9 -->
  - [ ] Don't send raw key events to PTY during active IME preedit <!-- blocked-by:9 -->
- [ ] **Tests** (`oriterm/src/app/input_keyboard.rs` `#[cfg(test)]`):
  - [ ] Keybinding takes priority over PTY send <!-- blocked-by:13 -->
  - [x] Kitty mode takes priority over legacy encoding
  - [ ] Ctrl+C with selection copies, without selection sends `\x03` <!-- blocked-by:9 -->
  - [x] IME commit sends text to PTY

---

## 8.4 Section Completion

- [ ] All 8.1-8.3 items complete
- [x] `cargo test -p oriterm --target x86_64-pc-windows-gnu` — key encoding tests pass
- [x] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [x] All printable characters encoded correctly
- [x] Arrow keys work in both normal and application cursor modes
- [x] F1-F12 function keys produce correct sequences
- [x] Ctrl+letter sends correct C0 control codes
- [x] Alt+key sends ESC prefix correctly
- [x] Modifier combinations on special keys produce correct parameter encoding
- [x] Numpad keys work in both normal and application keypad modes
- [x] Kitty keyboard protocol level 1+ supported (all 5 mode flags)
- [x] Key release/repeat events reported when REPORT_EVENT_TYPES active
- [ ] Keybinding dispatch has priority over PTY encoding <!-- blocked-by:13 -->
- [x] IME commit text reaches PTY
- [ ] Smart Ctrl+C works (copy if selection, SIGINT if not) <!-- blocked-by:9 -->

**Exit Criteria:** All standard terminal applications receive correct key input. vim, tmux, htop, and other apps work with correct modifier handling. Kitty protocol apps (e.g., kitty-based tools) receive properly encoded events.
