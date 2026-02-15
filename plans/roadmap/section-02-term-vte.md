---
section: 2
title: Terminal State Machine + VTE
status: in-progress
tier: 0
goal: Build Term<T> and implement all ~50 VTE handler methods so escape sequences produce correct grid state
sections:
  - id: "2.1"
    title: Event System
    status: complete
  - id: "2.2"
    title: TermMode Flags
    status: complete
  - id: "2.3"
    title: CharsetState
    status: complete
  - id: "2.4"
    title: Color Palette
    status: complete
  - id: "2.5"
    title: "Term<T> Struct"
    status: complete
  - id: "2.6"
    title: "VTE Handler — Print + Execute"
    status: complete
  - id: "2.7"
    title: "VTE Handler — CSI Sequences"
    status: complete
  - id: "2.8"
    title: "VTE Handler — SGR (Select Graphic Rendition)"
    status: not-started
  - id: "2.9"
    title: "VTE Handler — OSC Sequences"
    status: not-started
  - id: "2.10"
    title: "VTE Handler — ESC Sequences"
    status: not-started
  - id: "2.11"
    title: "VTE Handler — DCS + Misc"
    status: not-started
  - id: "2.12"
    title: RenderableContent Snapshot
    status: not-started
  - id: "2.13"
    title: FairMutex
    status: not-started
  - id: "2.14"
    title: Damage Tracking Integration
    status: not-started
  - id: "2.15"
    title: Section Completion
    status: not-started
---

# Section 02: Terminal State Machine + VTE

**Status:** 📋 Planned
**Goal:** Build `Term<T: EventListener>` that implements `vte::ansi::Handler`. Feed escape sequences in, get correct grid state out. This is the core of terminal emulation.

**Crate:** `oriterm_core`
**Dependencies:** All from Section 01, plus `base64`, `parking_lot`
**Reference:** Alacritty `alacritty_terminal/src/term/mod.rs` for `Term<T>` pattern; old `_old/src/term_handler/` for VTE method implementations.

---

## 2.1 Event System

The bridge between terminal state changes and the UI layer. Terminal fires events; UI layer handles them.

**File:** `oriterm_core/src/event.rs`

- [x] `Event` enum — terminal events that flow outward
  - [x] `Wakeup` — new content available, trigger redraw
  - [x] `Bell` — BEL character received
  - [x] `Title(String)` — window title changed (OSC 2)
  - [x] `ResetTitle` — title reset to default
  - [x] `ClipboardStore(ClipboardType, String)` — OSC 52 clipboard store
  - [x] `ClipboardLoad(ClipboardType, Arc<dyn Fn(&str) -> String + Send + Sync>)` — OSC 52 clipboard load
  - [x] `ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Send + Sync>)` — OSC 4/10/11 color query
  - [x] `PtyWrite(String)` — response bytes to write back to PTY
  - [x] `CursorBlinkingChange` — cursor blink state toggled
  - [x] `MouseCursorDirty` — mouse cursor shape may need update
  - [x] `ChildExit(i32)` — child process exited with status
- [x] `ClipboardType` enum — `Clipboard`, `Selection` (primary)
- [x] `Rgb` struct — `{ r: u8, g: u8, b: u8 }`
- [x] `EventListener` trait
  - [x] `fn send_event(&self, event: Event) {}` — default no-op
  - [x] Bound: `Send + 'static`
- [x] `Notify` trait — for writing responses back to PTY
  - [x] `fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B);`
  - [x] Bound: `Send`
- [x] `VoidListener` struct — no-op implementation for testing
  - [x] `impl EventListener for VoidListener {}`
- [x] Re-export from `lib.rs`
- [x] **Tests**:
  - [x] `VoidListener` compiles and implements `EventListener`
  - [x] `Event` variants can be constructed

---

## 2.2 TermMode Flags

Bitflags for terminal mode state (DECSET/DECRST, SM/RM).

**File:** `oriterm_core/src/term/mode.rs`

- [x] `TermMode` — `bitflags! { struct TermMode: u32 { ... } }`
  - [x] `SHOW_CURSOR` — DECTCEM (cursor visible)
  - [x] `APP_CURSOR` — DECCKM (application cursor keys)
  - [x] `APP_KEYPAD` — DECKPAM/DECKPNM (application keypad)
  - [x] `MOUSE_REPORT_CLICK` — mode 1000
  - [x] `MOUSE_DRAG` — mode 1002
  - [x] `MOUSE_MOTION` — mode 1003
  - [x] `MOUSE_SGR` — mode 1006 (SGR mouse encoding)
  - [x] `MOUSE_UTF8` — mode 1005 (UTF8 mouse encoding)
  - [x] `ALT_SCREEN` — mode 1049 (alternate screen)
  - [x] `LINE_WRAP` — DECAWM (auto-wrap)
  - [x] `ORIGIN` — DECOM (origin mode)
  - [x] `INSERT` — IRM (insert mode)
  - [x] `FOCUS_IN_OUT` — mode 1004 (focus events)
  - [x] `BRACKETED_PASTE` — mode 2004
  - [x] `SYNC_UPDATE` — mode 2026 (synchronized output)
  - [x] `URGENCY_HINTS` — mode 1042
  - [x] `ANY_MOUSE` — computed: CLICK | DRAG | MOTION
  - [x] `KITTY_KEYBOARD` — progressive keyboard enhancement
  - [x] `CURSOR_BLINKING` — ATT610
  - [x] Default: `SHOW_CURSOR | LINE_WRAP`
- [x] **Tests**:
  - [x] Default mode has SHOW_CURSOR and LINE_WRAP set
  - [x] Can set/clear individual modes
  - [x] `ANY_MOUSE` is the union of all mouse modes

---

## 2.3 CharsetState

Character set translation (G0-G3, single shifts). Needed for DEC special graphics and national character sets.

**File:** `oriterm_core/src/term/charset.rs`

- [x] `Charset` enum — `Ascii`, `DecSpecialGraphics`, `DecSupplemental`
- [x] `CharsetIndex` enum — `G0`, `G1`, `G2`, `G3`
- [x] `CharsetState` struct
  - [x] Fields:
    - `charsets: [Charset; 4]` — G0-G3 (default: all ASCII)
    - `active: CharsetIndex` — currently active charset (default: G0)
    - `single_shift: Option<CharsetIndex>` — SS2/SS3 single shift
  - [x] `translate(&mut self, ch: char) -> char` — apply charset mapping to character
    - [x] If single_shift is set, use that charset for one char, then clear
    - [x] DEC special graphics maps `0x5F..=0x7E` to box-drawing characters
  - [x] `set_charset(&mut self, index: CharsetIndex, charset: Charset)`
  - [x] `set_active(&mut self, index: CharsetIndex)`
  - [x] `set_single_shift(&mut self, index: CharsetIndex)`
- [x] **Tests**:
  - [x] Default: all ASCII, no translation
  - [x] DEC special graphics: `'q'` (0x71) → `'─'` (U+2500)
  - [x] Single shift: applies for one char then reverts
  - [x] G0/G1 switching

---

## 2.4 Color Palette

270-entry color palette: 16 ANSI + 216 cube + 24 grayscale + named colors. Resolves `vte::ansi::Color` enum to `Rgb`.

**File:** `oriterm_core/src/color/palette.rs`, `oriterm_core/src/color/mod.rs`

- [x] `Palette` struct
  - [x] Fields:
    - `colors: [Rgb; 270]` — full palette (0..=255 = indexed, 256..269 = foreground, background, cursor, etc.)
    - `scheme_name: String` — name of the loaded scheme
  - [x] `Palette::default()` — standard xterm-256 colors + sensible defaults for named slots
  - [x] `resolve(&self, color: &vte::ansi::Color, is_fg: bool) -> Rgb` — resolve Color enum to RGB
    - [x] `Color::Named(n)` → `self.colors[n as usize]`
    - [x] `Color::Spec(rgb)` → direct RGB
    - [x] `Color::Indexed(idx)` → `self.colors[idx as usize]`
  - [x] `set_indexed(&mut self, index: usize, color: Rgb)` — OSC 4
  - [x] `reset_indexed(&mut self, index: usize)` — OSC 104
  - [x] `foreground(&self) -> Rgb` — default foreground
  - [x] `background(&self) -> Rgb` — default background
  - [x] `cursor_color(&self) -> Rgb` — cursor color
- [x] `mod.rs`: re-export `Palette`, `Rgb`
- [x] **Tests**:
  - [x] Default palette: color 0 is black, color 7 is white, color 15 is bright white
  - [x] 256-color cube: indices 16–231 map correctly
  - [x] Grayscale ramp: indices 232–255
  - [x] `resolve` handles Named, Spec, Indexed variants
  - [x] `set_indexed` / `reset_indexed` work

---

## 2.5 Term\<T\> Struct

The terminal state machine. Owns two grids (primary + alternate), mode flags, palette, charset, title, keyboard mode stack. Generic over `EventListener` for decoupling from UI.

**File:** `oriterm_core/src/term/mod.rs`

- [x] `Term<T: EventListener>` struct
  - [x] Fields:
    - `grid: Grid` — primary grid (active when not in alt screen)
    - `alt_grid: Grid` — alternate grid (active during alt screen)
    - `active_is_alt: bool` — which grid is active
    - `mode: TermMode` — terminal mode flags
    - `palette: Palette` — color palette
    - `charset: CharsetState` — character set state
    - `title: String` — window title
    - `title_stack: Vec<String>` — pushed titles (xterm extension)
    - `cursor_shape: CursorShape` — cursor shape for rendering
    - `keyboard_mode_stack: Vec<u8>` — kitty keyboard enhancement stack
    - `inactive_keyboard_mode_stack: Vec<u8>` — stack for inactive screen
    - `event_listener: T` — event sink
  - [x] `Term::new(lines: usize, cols: usize, scrollback: usize, listener: T) -> Self`
    - [x] Create primary grid with scrollback
    - [x] Create alt grid (no scrollback — alt screen never has scrollback)
    - [x] Default mode, palette, charset, empty title
  - [x] `grid(&self) -> &Grid` — active grid
  - [x] `grid_mut(&mut self) -> &mut Grid` — active grid (mutable)
  - [x] `mode(&self) -> TermMode`
  - [x] `palette(&self) -> &Palette`
  - [x] `title(&self) -> &str`
  - [x] `cursor_shape(&self) -> CursorShape`
  - [x] `swap_alt(&mut self)` — switch between primary and alt screen
    - [x] Save/restore cursor
    - [x] Toggle `active_is_alt`
    - [x] Swap keyboard mode stacks
    - [x] Mark all dirty
- [x] **Tests**:
  - [x] `Term::<VoidListener>::new(24, 80, 1000, VoidListener)` creates a working terminal
  - [x] `grid()` returns primary grid by default
  - [x] `swap_alt()` switches to alt grid and back
  - [x] Mode defaults include SHOW_CURSOR and LINE_WRAP

---

## 2.6 VTE Handler — Print + Execute

`impl vte::ansi::Handler for Term<T>`. The `input` method (print) and control character execution.

**File:** `oriterm_core/src/term/handler.rs`

- [x] `impl<T: EventListener> vte::ansi::Handler for Term<T>`
- [x] `fn input(&mut self, ch: char)`
  - [x] Translate through charset (`self.charset.translate(ch)`)
  - [x] If auto-wrap pending (cursor at last col with WRAP): advance to next line, scroll if needed
  - [x] Call `self.grid_mut().put_char(translated_ch)`
- [x] Control characters (dispatched by `fn execute`):
  - [x] `\x07` BEL — `self.event_listener.send_event(Event::Bell)`
  - [x] `\x08` BS — move cursor left by 1
  - [x] `\x09` HT — tab forward
  - [x] `\x0A` LF — linefeed
  - [x] `\x0B` VT — same as LF
  - [x] `\x0C` FF — same as LF
  - [x] `\x0D` CR — carriage return
  - [x] `\x0E` SO — activate G1 charset
  - [x] `\x0F` SI — activate G0 charset
- [x] **Tests** (feed bytes through `vte::ansi::Processor`):
  - [x] `"hello"` → cells 0..5 contain h,e,l,l,o; cursor at col 5
  - [x] `"hello\nworld"` → "hello" on line 0, "world" on line 1
  - [x] `"hello\rworld"` → "world" on line 0 (overwrites "hello")
  - [x] `"\t"` → cursor advances to column 8
  - [x] `"\x08"` → cursor moves left
  - [x] BEL triggers Event::Bell on a recording listener

---

## 2.7 VTE Handler — CSI Sequences

Cursor movement, erase, scroll, insert/delete, device status, mode setting.

**File:** `oriterm_core/src/term/handler.rs` (continued)

- [x] Cursor movement CSIs:
  - [x] `CUU` (CSI n A) — `move_up(n)`
  - [x] `CUD` (CSI n B) — `move_down(n)`
  - [x] `CUF` (CSI n C) — `move_forward(n)`
  - [x] `CUB` (CSI n D) — `move_backward(n)`
  - [x] `CNL` (CSI n E) — move down n, column 0
  - [x] `CPL` (CSI n F) — move up n, column 0
  - [x] `CHA` (CSI n G) — `move_to_column(n-1)` (1-based)
  - [x] `CUP` (CSI n;m H) — `move_to(n-1, m-1)` (1-based)
  - [x] `VPA` (CSI n d) — `move_to_line(n-1)` (1-based)
  - [x] `HVP` (CSI n;m f) — same as CUP
- [x] Erase CSIs:
  - [x] `ED` (CSI n J) — `erase_display(mode)`
  - [x] `EL` (CSI n K) — `erase_line(mode)`
  - [x] `ECH` (CSI n X) — `erase_chars(n)`
- [x] Insert/Delete CSIs:
  - [x] `ICH` (CSI n @) — `insert_blank(n)`
  - [x] `DCH` (CSI n P) — `delete_chars(n)`
  - [x] `IL` (CSI n L) — `insert_lines(n)`
  - [x] `DL` (CSI n M) — `delete_lines(n)`
- [x] Scroll CSIs:
  - [x] `SU` (CSI n S) — `scroll_up(n)`
  - [x] `SD` (CSI n T) — `scroll_down(n)`
- [x] Tab CSIs:
  - [x] `CHT` (CSI n I) — tab forward n times
  - [x] `CBT` (CSI n Z) — tab backward n times
  - [x] `TBC` (CSI n g) — clear tab stops
- [x] Mode CSIs:
  - [x] `SM` (CSI n h) — set ANSI mode
  - [x] `RM` (CSI n l) — reset ANSI mode
  - [x] `DECSET` (CSI ? n h) — set DEC private mode
  - [x] `DECRST` (CSI ? n l) — reset DEC private mode
  - [x] Supported DECSET/DECRST modes: 1 (DECCKM), 6 (DECOM), 7 (DECAWM), 12 (cursor blinking), 25 (DECTCEM), 47/1047/1049 (alt screen), 1000/1002/1003/1005/1006 (mouse), 1004 (focus), 2004 (bracketed paste), 2026 (sync output)
- [x] Device status:
  - [x] `DSR` (CSI 6 n) — report cursor position (CPR response)
  - [x] `DA` (CSI c) — primary device attributes response
  - [x] `DA2` (CSI > c) — secondary device attributes response
- [x] Scroll region:
  - [x] `DECSTBM` (CSI n;m r) — `set_scroll_region(n-1, m)`
- [x] `DECSC` (CSI s when not in alt screen) — save cursor
- [x] `DECRC` (CSI u when not in alt screen) — restore cursor
- [x] `DECRPM` (CSI ? n $ p) — report mode (respond if mode is set/reset)
- [x] **Tests** (feed CSI sequences through processor):
  - [x] `ESC[5A` moves cursor up 5
  - [x] `ESC[10;20H` moves cursor to line 9, column 19 (0-based)
  - [x] `ESC[2J` clears screen
  - [x] `ESC[K` clears to end of line
  - [x] `ESC[5@` inserts 5 blanks
  - [x] `ESC[3P` deletes 3 chars
  - [x] `ESC[2L` inserts 2 lines
  - [x] `ESC[3M` deletes 3 lines
  - [x] `ESC[?25l` hides cursor (DECTCEM)
  - [x] `ESC[?25h` shows cursor
  - [x] `ESC[?1049h` switches to alt screen
  - [x] `ESC[?1049l` switches back to primary
  - [x] `ESC[3;20r` sets scroll region lines 3–20
  - [x] `ESC[6n` produces cursor position report (`ESC[line;colR`)

---

## 2.8 VTE Handler — SGR (Select Graphic Rendition)

Cell attribute setting: bold, italic, underline, colors. The most complex CSI.

**File:** `oriterm_core/src/term/handler.rs` (continued)

- [ ] `CSI n m` — SGR dispatch
  - [ ] `0` — reset all attributes (clear template flags and colors)
  - [ ] `1` — bold
  - [ ] `2` — dim
  - [ ] `3` — italic
  - [ ] `4` — underline (with sub-params: `4:0` none, `4:1` single, `4:3` curly, `4:4` dotted, `4:5` dashed)
  - [ ] `5` — blink
  - [ ] `7` — inverse
  - [ ] `8` — hidden
  - [ ] `9` — strikethrough
  - [ ] `21` — double underline
  - [ ] `22` — neither bold nor dim
  - [ ] `23` — not italic
  - [ ] `24` — not underline
  - [ ] `25` — not blink
  - [ ] `27` — not inverse
  - [ ] `28` — not hidden
  - [ ] `29` — not strikethrough
  - [ ] `30..=37` — set foreground (ANSI 0–7)
  - [ ] `38` — set foreground (extended): `38;5;n` (256-color) or `38;2;r;g;b` (truecolor)
  - [ ] `39` — default foreground
  - [ ] `40..=47` — set background (ANSI 0–7)
  - [ ] `48` — set background (extended)
  - [ ] `49` — default background
  - [ ] `58` — set underline color (extended): `58;5;n` or `58;2;r;g;b`
  - [ ] `59` — default underline color
  - [ ] `90..=97` — set bright foreground (ANSI 8–15)
  - [ ] `100..=107` — set bright background (ANSI 8–15)
- [ ] **Tests**:
  - [ ] `ESC[1m` sets bold on cursor template
  - [ ] `ESC[31m` sets fg to red (ANSI 1)
  - [ ] `ESC[38;5;196m` sets fg to 256-color index 196
  - [ ] `ESC[38;2;255;128;0m` sets fg to RGB(255, 128, 0)
  - [ ] `ESC[0m` resets all attributes
  - [ ] `ESC[1;31;42m` sets bold + red fg + green bg (compound)
  - [ ] `ESC[4:3m` sets curly underline
  - [ ] `ESC[58;2;255;0;0m` sets underline color to red (CellExtra)
  - [ ] `ESC[59m` clears underline color

---

## 2.9 VTE Handler — OSC Sequences

Operating System Commands: title, palette, clipboard.

**File:** `oriterm_core/src/term/handler.rs` (continued)

- [ ] `OSC 0` — set icon name + window title
  - [ ] `self.title = payload.to_string()`
  - [ ] `self.event_listener.send_event(Event::Title(...))`
- [ ] `OSC 1` — set icon name (ignored, just update title)
- [ ] `OSC 2` — set window title
- [ ] `OSC 4` — set/query indexed color
  - [ ] `OSC 4;index;rgb` → `palette.set_indexed(index, parse_rgb(rgb))`
  - [ ] `OSC 4;index;?` → query: respond with current color
- [ ] `OSC 7` — set working directory (shell integration)
  - [ ] Store as `Term.cwd: Option<String>`
- [ ] `OSC 8` — hyperlink
  - [ ] `OSC 8;;url` → set hyperlink on cursor template (CellExtra)
  - [ ] `OSC 8;;` → clear hyperlink
- [ ] `OSC 10` — set/query default foreground color
- [ ] `OSC 11` — set/query default background color
- [ ] `OSC 12` — set/query cursor color
- [ ] `OSC 52` — clipboard operations (base64 encoded)
  - [ ] `OSC 52;c;base64data` → decode, send `Event::ClipboardStore`
  - [ ] `OSC 52;c;?` → send `Event::ClipboardLoad`
- [ ] `OSC 104` — reset indexed color to default
- [ ] `OSC 110` — reset foreground color
- [ ] `OSC 111` — reset background color
- [ ] `OSC 112` — reset cursor color
- [ ] **Tests**:
  - [ ] `ESC]2;Hello World\x07` sets title to "Hello World"
  - [ ] `ESC]4;1;rgb:ff/00/00\x07` sets color 1 to red
  - [ ] `ESC]52;c;aGVsbG8=\x07` triggers clipboard store with "hello"
  - [ ] `ESC]8;;https://example.com\x07` sets hyperlink on template

---

## 2.10 VTE Handler — ESC Sequences

Escape sequences (non-CSI): charset, cursor save/restore, alt screen, index.

**File:** `oriterm_core/src/term/handler.rs` (continued)

- [ ] `ESC 7` / `DECSC` — save cursor position + attributes
- [ ] `ESC 8` / `DECRC` — restore cursor position + attributes
- [ ] `ESC D` / `IND` — index (linefeed without CR)
- [ ] `ESC E` / `NEL` — next line (CR + LF)
- [ ] `ESC H` / `HTS` — horizontal tab set
- [ ] `ESC M` / `RI` — reverse index
- [ ] `ESC c` / `RIS` — full reset (reset all state to initial)
- [ ] `ESC (` / `ESC )` / `ESC *` / `ESC +` — designate G0/G1/G2/G3 charset
  - [ ] `B` → ASCII, `0` → DEC Special Graphics
- [ ] `ESC =` / `DECKPAM` — application keypad mode
- [ ] `ESC >` / `DECKPNM` — normal keypad mode
- [ ] `ESC N` / `SS2` — single shift G2
- [ ] `ESC O` / `SS3` — single shift G3
- [ ] **Tests**:
  - [ ] `ESC7` + move cursor + `ESC8` restores original position
  - [ ] `ESCD` at bottom line scrolls up
  - [ ] `ESCM` at top line scrolls down
  - [ ] `ESCc` resets all state
  - [ ] `ESC(0` + `'q'` → box drawing char `'─'`
  - [ ] `ESC(B` → back to ASCII

---

## 2.11 VTE Handler — DCS + Misc

Device Control Strings and remaining handler methods.

**File:** `oriterm_core/src/term/handler.rs` (continued)

- [ ] DCS sequences:
  - [ ] `DECRQSS` — request selection or setting (respond with current state)
  - [ ] `XTGETTCAP` — xterm get termcap (respond with capabilities)
- [ ] Kitty keyboard protocol:
  - [ ] `CSI > u` — push keyboard mode onto stack
  - [ ] `CSI < u` — pop keyboard mode from stack
  - [ ] `CSI ? u` — query keyboard mode
  - [ ] Store modes in `keyboard_mode_stack: Vec<u8>`
- [ ] `CSI t` — window manipulation (report terminal size, etc.)
- [ ] `CSI q` — DECSCUSR: set cursor shape
  - [ ] 0/1 = blinking block, 2 = steady block, 3 = blinking underline, 4 = steady underline, 5 = blinking bar, 6 = steady bar
- [ ] Unhandled sequences:
  - [ ] Log at `debug!` level, do not panic or error
  - [ ] Return gracefully from handler methods
- [ ] **Tests**:
  - [ ] `ESC[1 q` sets cursor to blinking block
  - [ ] `ESC[5 q` sets cursor to blinking bar
  - [ ] `ESC[>1u` pushes keyboard mode 1
  - [ ] `ESC[<u` pops keyboard mode
  - [ ] Unknown sequences don't panic

---

## 2.12 RenderableContent Snapshot

A lightweight struct that captures everything the renderer needs from `Term`, extracted under lock and used without lock.

**File:** `oriterm_core/src/term/mod.rs` (additional types)

- [ ] `RenderableContent` struct
  - [ ] Fields:
    - `cells: Vec<RenderableCell>` — flattened visible cells (or row-by-row)
    - `cursor: RenderableCursor` — cursor position, shape, visibility
    - `selection: Option<SelectionRange>` — current selection (if any)
    - `display_offset: usize` — scrollback offset
    - `mode: TermMode` — terminal mode flags
    - `palette: Palette` — snapshot of color palette
    - `damage: Vec<DamageLine>` — which lines changed
  - [ ] `Term::renderable_content(&self) -> RenderableContent`
    - [ ] Iterate visible rows (accounting for display_offset + scrollback)
    - [ ] Include cursor info
    - [ ] Include damage info
    - [ ] This is called under lock, so it must be fast (copy, don't clone strings)
- [ ] `RenderableCell` struct
  - [ ] `ch: char`, `fg: Rgb`, `bg: Rgb`, `flags: CellFlags`, `underline_color: Option<Rgb>`
  - [ ] Colors are **resolved** (palette lookup done here, not in renderer)
  - [ ] Bold-as-bright applied here if enabled
- [ ] `RenderableCursor` struct
  - [ ] `point: Point`, `shape: CursorShape`, `visible: bool`
- [ ] `DamageLine` struct
  - [ ] `line: usize`, `left: Column`, `right: Column`
- [ ] **Tests**:
  - [ ] Create term, write some chars, extract RenderableContent, verify cells match
  - [ ] Cursor position in RenderableContent matches term cursor
  - [ ] Colors are resolved from palette (not raw Color enum)

---

## 2.13 FairMutex

Prevents starvation between PTY reader thread and render thread. Ported from Alacritty.

**File:** `oriterm_core/src/sync.rs`

**Reference:** `~/projects/reference_repos/console_repos/alacritty/alacritty_terminal/src/sync.rs`

- [ ] `FairMutex<T>` struct
  - [ ] Fields:
    - `data: parking_lot::Mutex<T>` — the actual data
    - `next: parking_lot::Mutex<()>` — fairness lock
  - [ ] `FairMutex::new(data: T) -> Self`
  - [ ] `lock(&self) -> FairMutexGuard<'_, T>` — fair lock: acquire `next`, then `data`
  - [ ] `lock_unfair(&self) -> parking_lot::MutexGuard<'_, T>` — skip fairness (for PTY thread)
  - [ ] `try_lock_unfair(&self) -> Option<parking_lot::MutexGuard<'_, T>>` — non-blocking try
  - [ ] `lease(&self) -> FairMutexLease<'_>` — reserve the `next` lock (PTY thread signals intent)
- [ ] `FairMutexGuard<'_, T>` — RAII guard that releases both locks on drop
- [ ] `FairMutexLease<'_>` — RAII guard for the `next` lock only
- [ ] **Tests**:
  - [ ] Basic lock/unlock works
  - [ ] Two threads can take turns locking
  - [ ] `try_lock_unfair` returns None when locked
  - [ ] Lease prevents fair lock from starving unfair lock

---

## 2.14 Damage Tracking Integration

Wire dirty tracking from Grid into the RenderableContent snapshot.

- [ ] `Term::damage(&self) -> impl Iterator<Item = DamageLine>`
  - [ ] Returns dirty lines from active grid's DirtyTracker
  - [ ] After reading damage, marks are cleared (drain semantics)
- [ ] `Term::reset_damage(&mut self)` — mark all clean (called after renderer consumes)
- [ ] `RenderableContent` includes damage info
  - [ ] If `all_dirty`, damage list is empty (signals full redraw)
  - [ ] Otherwise, damage list contains only changed lines
- [ ] **Tests**:
  - [ ] Write char → line is damaged
  - [ ] Read damage → line no longer damaged
  - [ ] scroll_up → all lines damaged
  - [ ] No changes → no damage

---

## 2.15 Section Completion

- [ ] All 2.1–2.14 items complete
- [ ] `cargo test -p oriterm_core` — all tests pass (Grid + Term + VTE)
- [ ] `cargo clippy -p oriterm_core --target x86_64-pc-windows-gnu` — no warnings
- [ ] Feed `echo "hello world"` through Term<VoidListener> → correct grid state
- [ ] Feed CSI sequences (cursor move, erase, SGR) → correct results
- [ ] Feed OSC sequences (title, palette) → correct events fired
- [ ] Alt screen switch works correctly
- [ ] RenderableContent snapshot extracts correct data
- [ ] FairMutex compiles and basic tests pass
- [ ] No GPU, no PTY, no window — purely in-memory terminal emulation

**Exit Criteria:** Full VTE processing works in-memory. `Term<VoidListener>` can process any escape sequence and produce correct grid state. `RenderableContent` snapshots work.
