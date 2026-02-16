# ori_term Rebuild — Roadmap Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Cell + Grid
**File:** `section-01-cell-grid.md` | **Tier:** 0 | **Status:** Not Started

```
cell, Cell, CellFlags, CellExtra, rich cell, 24 bytes
grid, Grid, Row, rows, columns, viewport
cursor, Cursor, CursorShape, cursor position, cursor template
index, Point, Line, Column, Side, Direction, Boundary, newtypes
scroll, scroll_up, scroll_down, scroll region, DECSTBM
scrollback, ring buffer, ScrollbackBuffer, history, display_offset
editing, put_char, insert_blank, delete_chars, erase, erase_display, erase_line
navigation, move_up, move_down, move_forward, move_backward, CUP, CUU, CUD, CUF, CUB
tab stops, HT, HTS, TBC, tab_forward, tab_backward
wide char, CJK, WIDE_CHAR, WIDE_CHAR_SPACER, unicode-width
combining mark, zero-width, zerowidth, CellExtra, diacritics, ZWJ
dirty, DirtyTracker, damage, dirty tracking, mark dirty
wrap, WRAP, soft wrap, auto-wrap, line wrap
workspace, Cargo workspace, oriterm_core, multi-crate
```

---

### Section 02: Terminal State Machine + VTE
**File:** `section-02-term-vte.md` | **Tier:** 0 | **Status:** Not Started

```
Term, Term<T>, terminal state machine, terminal emulation
VTE, vte::ansi::Handler, escape sequences, ANSI, control codes
event, Event, EventListener, Notify, VoidListener, send_event
TermMode, mode flags, DECSET, DECRST, SM, RM
SHOW_CURSOR, DECTCEM, APP_CURSOR, DECCKM, LINE_WRAP, DECAWM
ALT_SCREEN, alternate screen, 1049, swap_alt
mouse mode, 1000, 1002, 1003, 1006, MOUSE_SGR, MOUSE_REPORT_CLICK
bracketed paste, 2004, focus events, 1004, sync output, 2026
charset, CharsetState, G0, G1, G2, G3, DEC special graphics, SS2, SS3
palette, Palette, color, 256-color, indexed color, RGB, truecolor
SGR, Select Graphic Rendition, bold, italic, underline, strikethrough
foreground, background, dim, blink, inverse, hidden
CSI, cursor movement, CUP, CUU, CUD, CUF, CUB, CHA, VPA, CNL, CPL
erase, ED, EL, ECH, erase display, erase line, erase chars
insert, delete, ICH, DCH, IL, DL, insert lines, delete lines
OSC, title, OSC 2, OSC 4, OSC 7, OSC 8, OSC 10, OSC 11, OSC 52
clipboard, OSC 52, base64, clipboard store, clipboard load
hyperlink, OSC 8, URL
ESC, DECSC, DECRC, save cursor, restore cursor
IND, NEL, RI, reverse index, full reset, RIS
DCS, DECRQSS, XTGETTCAP, device control string
kitty keyboard, progressive enhancement, CSI u, keyboard mode stack
cursor shape, DECSCUSR, block, underline, bar
RenderableContent, snapshot, renderable, RenderableCell, RenderableCursor
FairMutex, fair mutex, synchronization, lock, starvation
damage, DamageLine, dirty, damage tracking, incremental
```

---

### Section 03: Cross-Platform
**File:** `section-03-cross-platform.md` | **Tier:** 0 | **Status:** Not Started

```
cross-platform, day one, Windows, Linux, macOS, platform abstraction
ConPTY, portable-pty, openpty, forkpty, SIGCHLD
DirectWrite, fontconfig, CoreText, font discovery, platform fonts
clipboard-win, arboard, platform clipboard, ClipboardProvider trait
Vulkan, DX12, Metal, wgpu backend, GPU backend selection
frameless window, WndProc, WM_NCHITTEST, drag_window, CSD, SSD
oriterm_ui, geometry, Point, Size, Rect, Insets, half-open interval
ScaleFactor, DPI, scale, unscale, logical pixels, physical pixels
hit_test, HitTestResult, ResizeDirection, Caption, Client, ResizeBorder
WindowConfig, create_window, load_icon, WindowError
platform_windows, platform_macos, platform_linux, platform glue
Aero Snap, WndProc subclass, DWM, WM_NCCALCSIZE, WM_DPICHANGED
Chromium ui/aura, ui/gfx/geometry, WindowDelegate, GetNonClientComponent
DWM, Mica, acrylic, vibrancy, transparency, NSVisualEffectView
X11, Wayland, NSWindow, WM integration, Sway, GNOME, KDE
system theme, dark mode, light mode, AppsUseLightTheme, D-Bus
config paths, APPDATA, XDG_CONFIG_HOME, ~/Library/Application Support
embedded fallback font, JetBrains Mono, include_bytes
URL opening, ShellExecuteW, xdg-open, open
signal handling, SIGCHLD, SetConsoleCtrlHandler
shell detection, $SHELL, cmd.exe, TERM, COLORTERM
#[cfg(target_os)], conditional compilation, platform-specific
```

---

### Section 04: PTY + Event Loop
**File:** `section-04-pty-eventloop.md` | **Tier:** 1 | **Status:** Not Started

```
PTY, pty, ConPTY, portable-pty, pseudo-terminal
spawn, shell, cmd.exe, powershell, spawn_shell, PtyHandle
reader thread, PTY reader, pty-reader, PtyEventLoop
Tab, TabId, tab, tab struct, per-tab state
EventProxy, event proxy, EventListener impl, winit proxy
Notifier, Notify impl, send input, write to PTY
Msg, message channel, mpsc, Input, Resize, Shutdown
TermEvent, user event, terminal event, wakeup
binary crate, oriterm, workspace setup
thread lifecycle, spawn, join, shutdown, drop
FairMutex, lock discipline, lease, lock_unfair, try_lock_unfair
contention, starvation, fair lock, reader thread lock
```

---

### Section 05: Window + GPU Rendering
**File:** `section-05-window-gpu.md` | **Tier:** 2 | **Status:** Not Started

```
render pipeline, staged pipeline, Extract, Prepare, Render, 3-phase
FrameInput, PreparedFrame, pipeline_stages.rs, phase separation
Extract phase, extract_frame, snapshot, lock, unlock, FairMutex
Prepare phase, prepare_frame, pure function, CPU-only, no wgpu
Render phase, render_frame, render_to_surface, GPU submission
window, winit, TermWindow, frameless, decorations, transparent
wgpu, GPU, GpuState, device, queue, adapter, surface
Vulkan, DX12, Metal, backend, render format
surface, SurfaceConfiguration, present mode, alpha mode
vibrancy, Mica, acrylic, DWM, window-vibrancy, transparency
GpuRenderer, bg_pipeline, fg_pipeline, render pass
offscreen, RenderTarget, create_render_target, read_render_target
headless, new_headless, compatible_surface: None, software rasterizer
font, FontCollection, FontSet, FontData, font discovery
swash, rasterize, glyph, RasterizedGlyph, GlyphKey, GlyphStyle
atlas, GlyphAtlas, glyph atlas, texture atlas, shelf packing, 1024x1024
AtlasEntry, AtlasLookup, UV, texture page, R8Unorm
WGSL, shader, vertex shader, fragment shader, pipeline
instance buffer, InstanceWriter, stride, 80 bytes, vertex pulling
uniform buffer, bind group, screen size, NDC
App, ApplicationHandler, event loop, Resumed, RedrawRequested
about_to_wait, event batching, coalesce, dirty flag
cursor, cursor rendering, block, bar, underline, blink
input forwarding, keyboard, basic input, Enter, Backspace
cell_width, cell_height, baseline, font metrics
testing, unit test, headless GPU, visual regression, reference PNG
determinism, pixel readback, decode_instance, assert_instance_count
```

---

### Section 06: Font Pipeline
**File:** `section-06-font-pipeline.md` | **Tier:** 2 | **Status:** Not Started

```
font, FontCollection, FontSet, FaceData, FaceIdx, font discovery
multi-face, Regular, Bold, Italic, BoldItalic, font variant
fallback, fallback chain, FallbackMeta, cap-height, normalization, scale_factor, size_offset
shaping, rustybuzz, ShapingRun, ShapedGlyph, two-phase shaping, prepare_runs, shape_run
ligature, multi-cell glyph, col_glyph_map, col_start, col_span, glyph cluster
combining mark, zero-width, CellExtra.zerowidth, ZWJ, diacritics
OpenType, font features, liga, calt, kern, feature parsing, per-fallback features
atlas, GlyphAtlas, guillotine packing, best-short-side-fit, texture array
texture page, 2048x2048, multi-page, LRU eviction, R8Unorm, Rgba8Unorm
AtlasEntry, GlyphKey, Q6 fixed-point, subpixel, page_index
built-in glyphs, box drawing, block elements, braille, powerline
geometric, BOX_DRAWING_TABLE, lookup table, rasterize_builtin
color emoji, RGBA, ColorOutline, ColorBitmap, VS15, VS16, presentation selector
synthetic bold, double-strike, variable weight, wght axis
underline, curly underline, dotted, dashed, double, strikethrough
text decoration, underline color, SGR 58, hyperlink hover
UI text, UiShapedGlyph, measure_text, ellipsis truncation, tab bar text
pre-cache, ASCII pre-cache, scratch buffer, face creation, perf
swash, rasterize, glyph, RasterizedGlyph, GlyphStyle
dwrote, DirectWrite, Windows font, system fonts
```

---

### Section 07: 2D UI Framework
**File:** `section-07-ui-framework.md` | **Tier:** 2 | **Status:** Not Started

```
UI framework, oriterm_ui, widget, widget tree, retained mode
DrawList, draw primitives, rect, rounded rect, shadow, gradient
RectStyle, Border, Shadow, Color, Rect, Point
text rendering, ShapedText, TextStyle, measure_text, UI font
layout, LayoutNode, LayoutConstraints, flex, Row, Column
Size, Fixed, Fill, Hug, FillPortion, Spacing, padding, margin
Align, Justify, Gap, Direction, two-pass layout
hit testing, hit_test, WidgetId, mouse capture
focus, FocusManager, Tab order, focus ring, keyboard navigation
Button, Checkbox, Toggle, Slider, TextInput, Dropdown, Label
Separator, Spacer, Panel, ScrollWidget, Stack
FlexWidget, ScrollContainer, scroll, scrollbar
overlay, OverlayManager, Placement, modal, context menu
animation, Animation, Easing, AnimatedValue, transition
UiTheme, dark theme, light theme, accent color, styling
TerminalGridWidget, terminal as widget, tab bar widget
```

---

### Section 08: Keyboard Input
**File:** `section-08-keyboard-input.md` | **Tier:** 3 | **Status:** Not Started

```
keyboard, key encoding, legacy, xterm, key event
kitty keyboard, progressive enhancement, CSI u, disambiguate
arrow keys, function keys, F1-F12, Home, End, PageUp, PageDown
Ctrl, Alt, Shift, modifiers, modifier encoding
escape sequence, key sequence, application cursor mode, DECCKM
keyboard dispatch, handle_keyboard_input, keybinding lookup
IME, input method, Ime::Commit, Ime::Preedit, composition
```

---

### Section 09: Selection & Clipboard
**File:** `section-09-selection-clipboard.md` | **Tier:** 3 | **Status:** Not Started

```
selection, Selection, SelectionRange, SelectionPoint, SelectionMode
anchor, pivot, end, 3-point selection, Windows Terminal model
char selection, word selection, line selection, block selection
click, double-click, triple-click, drag, mouse selection
drag threshold, drag_start_threshold, 1/4 cell width
word boundary, word_start, word_end, soft wrap, delimiter class
text extraction, selection_to_string, copy text
clipboard, clipboard_get, clipboard_set, copy, paste
clipboard-win, arboard, platform clipboard
bracketed paste, ESC[200~, ESC[201~, FilterOnPaste
CopyOnSelect, copy on release, auto-copy
HTML Format, RTF, CF_UNICODETEXT, formatted copy
keyboard selection, mark mode, Shift+arrows
file drag-and-drop, auto-quote paths
selection rendering, invert colors, highlight
```

---

### Section 10: Mouse Input & Reporting
**File:** `section-10-mouse-input.md` | **Tier:** 3 | **Status:** Not Started

```
mouse reporting, mouse mode, 1000, 1002, 1003, 1006
SGR mouse, X10 mouse, UTF8 mouse, URXVT mouse
button encoding, scroll, middle click
click count, double-click, triple-click, Alt+click block selection
Ctrl+click URL, Shift override, mouse reporting bypass
mouse selection state machine, SelectionState
auto-scroll, drag above/below viewport
```

---

### Section 11: Search
**File:** `section-11-search.md` | **Tier:** 3 | **Status:** Not Started

```
search, SearchState, SearchMatch, regex, plain text
find, Ctrl+F, next match, prev match, highlight
search overlay, search bar, search UI
row_text, text extraction, byte offset mapping
```

---

### Section 12: Resize & Reflow
**File:** `section-12-resize-reflow.md` | **Tier:** 3 | **Status:** Not Started

```
reflow, resize, Grid::resize, logical line, re-wrap
wide char boundary, cursor position, scrollback reflow
reflow_grow_cols, reflow_shrink_cols, WRAPLINE
LEADING_WIDE_CHAR_SPACER, split point
PTY resize, TIOCSWINSZ, ConPTY, PtySize
window resize, grid_dims_for_size, SIGWINCH
zero dimension guard, alternate screen resize
```

---

### Section 13: Configuration & Keybindings
**File:** `section-13-config-keybindings.md` | **Tier:** 3 | **Status:** Not Started

```
config, Config, TOML, config.toml, hot reload
FontConfig, ColorConfig, WindowConfig, BehaviorConfig
file watcher, notify, config monitor
keybindings, KeyBinding, Action, shortcut
Ctrl+Shift+C, Ctrl+Shift+V, Ctrl+Tab, Ctrl+T
zoom, font size, Ctrl+=, Ctrl+-
```

---

### Section 14: URL Detection
**File:** `section-14-url-detection.md` | **Tier:** 3 | **Status:** Not Started

```
URL, url_detect, hover, Ctrl+click, hyperlink
regex URL detection, scheme validation
hover underline, CursorIcon::Pointer
implicit URL, OSC 8 hyperlink
```

---

### Section 15: Tab Struct & Management
**File:** `section-15-tab-management.md` | **Tier:** 4 | **Status:** Not Started

```
Tab, TabId, tab struct, per-tab state, tab lifecycle
Arc<FairMutex<Term<EventProxy>>>, terminal lock, mode_cache, AtomicU32
PtyWriter, Arc<Mutex<Write>>, ConPTY deadlock, VTE responses outside lock
SpawnConfig, Tab::spawn, Tab::shutdown, background thread drop
grid_dirty, AtomicBool, wakeup_pending, lock-free, coalescing
tab management, create tab, close tab, duplicate tab, cycle tab
CWD inheritance, alloc_tab_id, spawn_tab, close_tab
active_tab, active_tab_id, Vec<TabId>, HashMap<TabId, Tab>
auto-close, PtyExited, background thread drop
```

---

### Section 16: Tab Bar & Chrome
**File:** `section-16-tab-bar.md` | **Tier:** 4 | **Status:** Not Started

```
tab bar, TabBarLayout, TabBarColors, TAB_BAR_HEIGHT, TAB_MIN_WIDTH, TAB_MAX_WIDTH
tab_width_lock, rapid close, close button stability
tab bar rendering, separator suppression, bell pulse, lerp_color
dragged tab overlay, drag_visual_x, tab animation offsets, decay
tab bar hit testing, TabBarHit, CloseTab, NewTab, DropdownButton, DragArea
close button inset, platform-specific controls, Windows rectangular, macOS circular
tab hover preview, terminal preview, thumbnail, TerminalPreviewWidget, offscreen render
```

---

### Section 17: Drag & Drop
**File:** `section-17-drag-drop.md` | **Tier:** 4 | **Status:** Not Started

```
drag, DragState, DragPhase, Pending, DraggingInBar
DRAG_START_THRESHOLD, TEAR_OFF_THRESHOLD, TEAR_OFF_THRESHOLD_UP
mouse_offset_in_tab, cursor center insertion, Chrome-style snap
tear-off, tear_off_tab, OS drag, WM_MOVING, merge detection
seamless drag, synthesize mouse-down, merge_drag_suppress_release
compute_drop_index, DWM invisible borders, screen to local
```

---

### Section 18: Multi-Window & Window Lifecycle
**File:** `section-18-multi-window.md` | **Tier:** 4 | **Status:** Not Started

```
multi-window, TermWindow, window management, WindowId
cross-window, tab movement, focus tracking, FOCUS_IN_OUT
window lifecycle, create_window, close_window, exit_app
no-flash startup, render before show, DPI scale, WS_EX_NOREDIRECTIONBITMAP
Aero Snap, WndProc subclass, WM_NCHITTEST, WM_DPICHANGED
resize ALL tabs, process::exit, ConPTY-safe cleanup
```

---

### Section 19: Event Routing & Render Scheduling
**File:** `section-19-event-routing.md` | **Tier:** 4 | **Status:** Not Started

```
coordinate systems, pixel_to_cell, pixel_to_side, grid_top, grid_dims_for_size
TAB_BAR_HEIGHT, GRID_PADDING_TOP, GRID_PADDING_BOTTOM, GRID_PADDING_LEFT
event routing, keyboard dispatch, mouse dispatch, priority chain
key release, Kitty REPORT_EVENT_TYPES, search mode intercept
keybinding lookup, execute_action, PTY dispatch
mouse reporting, Shift override, context menu intercept
click count, double-click, triple-click, Alt+click block selection, Ctrl+click URL
render scheduling, about_to_wait, frame budget, 8ms, 120 FPS
dirty aggregation, pending_redraw, tab_bar_dirty, grid_dirty, cursor_blink
ControlFlow, WaitUntil, cursor blink scheduling, performance stats
```

---

### Section 20: Shell Integration
**File:** `section-20-shell-integration.md` | **Tier:** 4 | **Status:** Not Started

```
shell integration, Shell, detect_shell, inject, ZDOTDIR, XDG_DATA_DIRS
OSC 7, OSC 133, prompt state, PromptState, prompt_mark_pending
two-parser strategy, raw interceptor, vte::Parser, Perform
effective_title, has_explicit_title, CWD short path, title_dirty
keyboard mode stack swap, inactive_keyboard_mode_stack, swap_alt
XTVERSION, CSI >q, DCS response, notifications, OSC 9, OSC 99, OSC 777
version stamping, .version file, shell-integration directory
bash-preexec, oriterm.bash, oriterm.fish, oriterm.ps1, .zshenv
```

---

### Section 21: Context Menu & Window Controls
**File:** `section-21-context-menu.md` | **Tier:** 4 | **Status:** Not Started

```
context menu, MenuOverlay, MenuEntry, Item, Check, Separator
tab context menu, grid context menu, dropdown menu, color scheme selector
GPU-rendered menu, shadow, rounded corners, hover highlight
config reload, apply_config_reload, broadcast ALL tabs, font rebuild
atlas rebuild, resize all tabs all windows, keybinding rebuild
settings UI, settings_window, color scheme list, checkmark
window controls, minimize, maximize, close, platform-specific
frameless, drag window, Aero Snap, double-click maximize
```

---

### Section 22: Terminal Modes
**File:** `section-22-terminal-modes.md` | **Tier:** 5 | **Status:** Not Started

```
DECSET, DECRST, terminal modes, mode table
mouse reporting, 1000, 1002, 1003, 1006, SGR mouse
cursor style, DECSCUSR, block, underline, bar, blink
focus events, 1004, ESC[I, ESC[O
synchronized output, 2026, BSU, ESU
hyperlink, OSC 8, URL, hover, Ctrl+click
image protocol, Kitty image, sixel, DCS
application keypad, DECKPAM, DECKPNM
origin mode, DECOM, scroll region
reverse wraparound, DECAWM
save/restore modes, XTSAVE, XTRESTORE
```

---

### Section 23: Performance & Damage Tracking
**File:** `section-23-performance.md` | **Tier:** 5 | **Status:** Not Started

```
damage tracking, DirtyTracker, per-row dirty, BitVec
instance buffer caching, partial update, skip present
ring buffer, ScrollbackRing, O(1) push, wrapping index
parsing performance, PTY buffer size, fast ASCII path
rendering performance, instance buffer reuse, frame pacing
memory optimization, Row pooling, compact representation
benchmarks, criterion, throughput, latency, FPS, regression
```

---

### Section 24: Visual Polish
**File:** `section-24-visual-polish.md` | **Tier:** 6 | **Status:** Not Started

```
cursor blink, blink timer, DECSCUSR, steady vs blinking
hide cursor while typing, set_cursor_visible, mouse move restore
minimum contrast, WCAG 2.0, luminance, shader
HiDPI, scale_factor, ScaleFactorChanged, DPI-aware
smooth scrolling, pixel offset, kinetic scroll, trackpad
background image, PNG, JPEG, texture, opacity, position
window shadow, padding, margin, GRID_PADDING
```

---

### Section 25: Theme System
**File:** `section-25-theme-system.md` | **Tier:** 6 | **Status:** Not Started

```
theme, ColorScheme, color scheme, palette, BUILTIN_SCHEMES
theme file, TOML theme, theme directory, theme discovery
built-in themes, Catppuccin, Dracula, Nord, Gruvbox, Solarized
light/dark auto-switch, system appearance, AppsUseLightTheme
theme hot-reload, live switching, theme preview
theme conversion, iTerm2, Ghostty, base16
```

---

### Section 26: Split Panes
**File:** `section-26-split-panes.md` | **Tier:** 7 | **Status:** Not Started

```
split, pane, PaneId, PaneNode, PaneTree, SplitDirection
horizontal split, vertical split, binary tree layout
pane navigation, Alt+Arrow, focus pane, click focus
pane resize, drag divider, ratio, equalize
zoom, unzoom, Ctrl+Shift+Z, full-tab pane
close pane, collapse tree, remove_pane
render panes, divider, active border, independent scroll
```

---

### Section 27: Command Palette & Quick Terminal
**File:** `section-27-command-palette.md` | **Tier:** 7 | **Status:** Not Started

```
command palette, Ctrl+Shift+P, fuzzy search, action registry
quick terminal, drop-down, global hotkey, Quake-style, F12
desktop notifications, OSC 9, OSC 777, OSC 99, toast
progress indicators, OSC 9;4, taskbar progress, ITaskbarList3
terminal inspector, Ctrl+Shift+I, debug overlay, escape sequence log
```

---

### Section 28: Extensibility
**File:** `section-28-extensibility.md` | **Tier:** 7 | **Status:** Not Started

```
Lua, mlua, scripting, event hooks, plugin
oriterm.on, oriterm.new_tab, oriterm.send_text, API
custom shaders, WGSL, post-processing, CRT effect
smart paste, multi-line warning, ESC sanitize, large paste
undo close tab, closed_tabs, Ctrl+Shift+T, restore CWD
session serialization, workspace presets, broadcast input
```

---

## Quick Reference

| ID | Title | File | Tier | Status |
|----|-------|------|------|--------|
| 01 | Cell + Grid | `section-01-cell-grid.md` | 0 | Not Started |
| 02 | Terminal State Machine + VTE | `section-02-term-vte.md` | 0 | Not Started |
| 03 | Cross-Platform | `section-03-cross-platform.md` | 0 | Not Started |
| 04 | PTY + Event Loop | `section-04-pty-eventloop.md` | 1 | Not Started |
| 05 | Window + GPU Rendering | `section-05-window-gpu.md` | 2 | Not Started |
| 06 | Font Pipeline | `section-06-font-pipeline.md` | 2 | Not Started |
| 07 | 2D UI Framework | `section-07-ui-framework.md` | 2 | Not Started |
| 08 | Keyboard Input | `section-08-keyboard-input.md` | 3 | Not Started |
| 09 | Selection & Clipboard | `section-09-selection-clipboard.md` | 3 | Not Started |
| 10 | Mouse Input & Reporting | `section-10-mouse-input.md` | 3 | Not Started |
| 11 | Search | `section-11-search.md` | 3 | Not Started |
| 12 | Resize & Reflow | `section-12-resize-reflow.md` | 3 | Not Started |
| 13 | Configuration & Keybindings | `section-13-config-keybindings.md` | 3 | Not Started |
| 14 | URL Detection | `section-14-url-detection.md` | 3 | Not Started |
| 15 | Tab Struct & Management | `section-15-tab-management.md` | 4 | Not Started |
| 16 | Tab Bar & Chrome | `section-16-tab-bar.md` | 4 | Not Started |
| 17 | Drag & Drop | `section-17-drag-drop.md` | 4 | Not Started |
| 18 | Multi-Window & Window Lifecycle | `section-18-multi-window.md` | 4 | Not Started |
| 19 | Event Routing & Render Scheduling | `section-19-event-routing.md` | 4 | Not Started |
| 20 | Shell Integration | `section-20-shell-integration.md` | 4 | Not Started |
| 21 | Context Menu & Window Controls | `section-21-context-menu.md` | 4 | Not Started |
| 22 | Terminal Modes | `section-22-terminal-modes.md` | 5 | Not Started |
| 23 | Performance & Damage Tracking | `section-23-performance.md` | 5 | Not Started |
| 24 | Visual Polish | `section-24-visual-polish.md` | 6 | Not Started |
| 25 | Theme System | `section-25-theme-system.md` | 6 | Not Started |
| 26 | Split Panes | `section-26-split-panes.md` | 7 | Not Started |
| 27 | Command Palette & Quick Terminal | `section-27-command-palette.md` | 7 | Not Started |
| 28 | Extensibility | `section-28-extensibility.md` | 7 | Not Started |

## Tier Summary

| Tier | Sections | Theme |
|------|----------|-------|
| 0 | 01-03 | Core library + cross-platform architecture |
| 1 | 04 | Process layer (PTY, threads) |
| 2 | 05-07 | Rendering foundation (window, GPU, fonts, UI framework) |
| 3 | 08-14 | Interaction (keyboard, mouse, selection, search, config) |
| 4 | 15-21 | Multi-tab + chrome (feature parity with prototype) |
| 5 | 22-23 | Hardening (terminal modes, performance) |
| 6 | 24-25 | Polish (visual refinements, themes) |
| 7 | 26-28 | Advanced (split panes, command palette, extensibility) |

## Dependency DAG

```
01 Cell + Grid
 |
02 Term + VTE
 |
03 Cross-Platform         <- platform abstractions (PTY, fonts, clipboard, GPU, window)
 |
04 PTY + Event Loop       <- builds on platform PTY abstraction
 |
05 Window + GPU           <- first visual milestone (Vulkan/DX12/Metal via wgpu)
 |
06 Font Pipeline          <- advanced fonts, ligatures, emoji
 |
07 2D UI Framework        <- drawing primitives, layout, widgets (oriterm_ui crate)
 |
08-14 Interaction         <- keyboard, mouse, selection, search, config, URL
 |
15-21 Tabs + Chrome       <- tab bar, drag/drop, menus built on UI framework
 |
22-23 Hardening           <- terminal modes, performance
 |
24-25 Polish              <- cursor blink, smooth scroll, themes
 |
26-28 Advanced            <- split panes, command palette, extensibility
```
