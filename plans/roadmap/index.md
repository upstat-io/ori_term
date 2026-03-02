# ori_term Rebuild — Roadmap Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Priority Queue

Sections listed here are worked on **before** sequential scanning. When `/continue-roadmap` runs without arguments, check this list first — the first incomplete section here becomes the focus.

| Priority | Section | Reason |
|----------|---------|--------|
| ~~1~~ | ~~43 — Compositor Layer System~~ | ~~Complete~~ |

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
**File:** `section-05-window-gpu.md` | **Tier:** 2 | **Status:** In Progress (5.1–5.11 complete)

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

### Section 05B: Startup Performance
**File:** `section-05b-startup-perf.md` | **Tier:** 2 | **Status:** Not Started | **Blocks:** 06

```
startup, launch, performance, cold start, warm start
dwrote, FontCollection::system, DirectWrite, COM, font cache
parallel, thread, concurrent, GPU init, font discovery
pipeline cache, shader compilation, pre-cache, ASCII
profiling, timing, Instant, startup time, latency
```

---

### Section 05C: Window Chrome (Title Bar + Controls)
**File:** `section-05c-window-chrome.md` | **Tier:** 2 | **Status:** Complete

```
window chrome, title bar, caption bar, window controls
minimize, maximize, restore, close, control button
WindowChromeWidget, ChromeLayout, WindowControlButton, ControlKind
caption height, CAPTION_HEIGHT, CAPTION_HEIGHT_MAXIMIZED
CONTROL_BUTTON_WIDTH, RESIZE_BORDER_WIDTH
interactive rects, hit test, drag area, Aero Snap
active, inactive, focused, unfocused, caption background
grid offset, caption_px, terminal grid below chrome
WidgetAction, WindowMinimize, WindowMaximize, WindowClose
draw_chrome, NullMeasurer, append_ui_draw_list
enable_snap, set_client_rects, platform wiring
```

---

### Section 06: Font Pipeline + Best-in-Class Glyph Rendering
**File:** `section-06-font-pipeline.md` | **Tier:** 2 | **Status:** Complete

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
hinting, hint, grid-fitting, HintingMode, DPI-aware, auto-detect, crisp text
subpixel rendering, LCD, ClearType, Format::Subpixel, per-channel alpha, RGB, BGR
subpixel positioning, fractional offset, Vector, offset, subpx_bin, quantization
font synthesis, embolden, synthetic bold, synthetic italic, skew, Transform, oblique
visual regression, golden image, pixel comparison, reference PNG, FLIP, automated testing
synthetic bold, embolden, variable weight, wght axis, Synthesis
synthetic italic, skew, oblique, Transform, 14 degrees
underline, curly underline, dotted, dashed, double, strikethrough
text decoration, underline color, SGR 58, hyperlink hover
UI text, UiShapedGlyph, measure_text, ellipsis truncation, tab bar text
pre-cache, ASCII pre-cache, scratch buffer, face creation, perf
swash, rasterize, glyph, RasterizedGlyph, GlyphStyle, zeno
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
**File:** `section-10-mouse-input.md` | **Tier:** 3 | **Status:** Complete

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
**File:** `section-15-tab-management.md` | **Tier:** 4 | **Status:** Superseded → Sections 30, 32

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
tab icon, emoji, TabIcon, process icon, OSC 1, icon name, color emoji in tab
```

---

### Section 17: Drag & Drop
**File:** `section-17-drag-drop.md` | **Tier:** 4 | **Status:** In Progress (17.1 complete)

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
**File:** `section-18-multi-window.md` | **Tier:** 4 | **Status:** Superseded → Section 32

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
jump list, Jump List, ICustomDestinationList, IShellLinkW, taskbar right-click
dock menu, applicationDockMenu, NSMenu, dock right-click, macOS dock
desktop actions, .desktop file, Linux quicklist, New Window, New Tab
profile quick-launch, taskbar integration, start menu
```

---

### Section 22: Terminal Modes
**File:** `section-22-terminal-modes.md` | **Tier:** 5 | **Status:** Mostly Complete

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

### Section 38: Terminal Protocol Extensions
**File:** `section-38-protocol-extensions.md` | **Tier:** 5 | **Status:** Not Started

```
DA, DA1, DA2, DA3, Device Attributes, primary, secondary, tertiary
DSR, Device Status Report, CSI 5 n, CSI 6 n, cursor position report, CPR
DECRQM, Request Mode, CSI ? Pm $ p, mode query, progressive enhancement
XTGETTCAP, DCS + q, terminfo query, capability discovery
DECRQSS, Request Setting, DCS $ q, SGR query, scroll region query
OSC 4, OSC 10, OSC 11, OSC 12, color query, palette query, theme detection
OSC 104, OSC 110, OSC 111, OSC 112, reset color
extended underline, SGR 4:3, curly underline, dotted, dashed, double
underline color, SGR 58, SGR 59, colored underline
overline, SGR 53, SGR 55
CSI t, window manipulation, window size, cell size, report, resize, iconify
DCS passthrough, tmux passthrough, nested terminal
capability reporting, progressive enhancement, feature discovery
```

---

### Section 39: Image Protocols
**File:** `section-39-image-protocols.md` | **Tier:** 5 | **Status:** Not Started

```
image, inline image, image protocol, image cache, ImageData, ImagePlacement
Kitty graphics, APC, ESC_G, kitty image, image transmission, chunked transfer
image placement, image ID, placement ID, z-index, image delete
Sixel, sixel graphics, DCS, sixel data, palette, sixel decode
iTerm2 image, OSC 1337, imgcat, inline image display
image rendering, GPU compositing, image texture, image atlas
image animation, frame, animated GIF, animation control
image memory, image eviction, LRU, memory limit, 256 MB
viu, timg, hologram, ranger preview, Jupyter inline plot
```

---

### Section 40: Vi Mode + Copy Mode
**File:** `section-40-vi-copy-mode.md` | **Tier:** 3 | **Status:** Not Started

```
vi mode, copy mode, keyboard navigation, modal input
vi cursor, hjkl, word motion, line motion, vertical motion
w, b, e, W, B, E, word boundary, WORD boundary
0, ^, $, gg, G, H, M, L, viewport motion
Ctrl+U, Ctrl+D, half page, Ctrl+B, Ctrl+F, full page
f, F, t, T, inline search, ;, comma, repeat
%, bracket matching, *, #, word under cursor
v, V, Ctrl+V, visual selection, line selection, block selection
y, yank, copy, vi copy, keyboard copy
/, ?, n, N, vi search, search forward, search backward
zz, center view, auto-scroll, scrollback navigation
Ctrl+Shift+Space, toggle vi mode, vi mode cursor
```

---

### Section 41: Hints + Quick Select
**File:** `section-41-hints-quick-select.md` | **Tier:** 3 | **Status:** Not Started

```
hints, hint mode, quick select, labeled selection
hint pattern, regex pattern, URL hint, path hint, hash hint
git hash, IP address, email, file path, custom pattern
hint label, alphabet, single char label, two char label
progressive filtering, label assignment, proximity sort
hint action, copy, open, CopyAndPaste, select
Ctrl+Shift+H, hint keybinding, per-pattern keybinding
url_template, JIRA, GitHub issue, custom URL
hint rendering, dim, label overlay, contrast color
vimium, quick copy, pattern registry
```

---

### Section 42: Expose / Overview Mode
**File:** `section-42-expose-overview.md` | **Tier:** 5 | **Status:** Not Started

```
expose, overview, Mission Control, thumbnail grid, pane overview
ExposeMode, ExposePhase, ExposeTile, expose state machine
Ctrl+Shift+Space, enter expose, exit expose, cancel expose
thumbnail, ThumbnailCache, offscreen render, RenderTarget, 320x200
ImagePipeline, textured quad, WGSL, image shader, bind group
compute_expose_grid, auto-columns, last-row centering, responsive grid
label_rect, thumbnail label, pane title, hint character, a-z hints
arrow navigation, grid-aware wrapping, Tab cycling, mouse click
type-to-filter, filter bar, case-insensitive, substring match
double-Escape, clear filter, no matches, empty state
staggered update, round-robin, burst render, selected always updates
live thumbnails, GPU thumbnails, pane switching, cross-window
```

---

### Section 43: Compositor Layer System + Animation Architecture
**File:** `section-43-compositor-layers.md` | **Tier:** 5 | **Status:** Not Started

```
compositor, layer, LayerId, LayerType, LayerProperties, LayerTree
layer tree, parent-child, z-order, root layer, Group layer
render-to-texture, offscreen render, intermediate texture, composition pass
GpuCompositor, RenderTargetPool, texture pool, power-of-two buckets
composition shader, composite.wgsl, textured quad, premultiplied alpha
LayerAnimator, PropertyTransition, AnimatableProperty, property animation
Opacity, Transform, Bounds, animate_opacity, animate_transform, animate_bounds
Transform2D, affine transform, translate, scale, rotate, concat, inverse
LayerDelegate, paint_layer, content provider, DrawCtx
AnimationDelegate, animation_ended, animation_canceled, lifecycle callbacks
AnimationSequence, AnimationStep, chain, delay, sequential animation
AnimationGroup, parallel animation, simultaneous transitions
AnimationBuilder, fluent API, builder pattern
PreemptionStrategy, ReplaceCurrent, Enqueue, animation interruption
Lerp, Rect lerp, Point lerp, Size lerp, Transform2D lerp
overlay fade, fade-in, fade-out, opacity animation, modal dim
tab sliding, tab reorder animation, transform animation
needs_paint, needs_composite, dirty flags, damage tracking
accumulated_opacity, accumulated_transform, tree traversal
performance escape hatch, default properties, zero overhead
```

---

### Section 24: Visual Polish
**File:** `section-24-visual-polish.md` | **Tier:** 6 | **Status:** Not Started

```
cursor blink, blink timer, DECSCUSR, steady vs blinking
hide cursor while typing, set_cursor_visible, mouse move restore
minimum contrast, WCAG 2.0, luminance, shader
HiDPI, scale_factor, ScaleFactorChanged, DPI-aware
visual effects, polish, refinement
vector icons, tiny_skia, icon rasterization, anti-aliased icons, IconPath, PathCommand
close button icon, minimize icon, maximize icon, restore icon, chevron icon, plus icon
jagged lines, staircase, pixel stepping, smooth icons, Chrome-style icons
background image, PNG, JPEG, texture, opacity, position
window shadow, padding, margin, GRID_PADDING
```

---

### Section 25: Theme System
**File:** `section-25-theme-system.md` | **Tier:** 6 | **Status:** In Progress

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
**File:** `section-26-split-panes.md` | **Tier:** 7 | **Status:** Superseded → Sections 29, 31, 33

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

### Section 29: Mux Crate + Layout Engine
**File:** `section-29-mux-layout-engine.md` | **Tier:** 4M | **Status:** Not Started

```
oriterm_mux, mux crate, multiplexing foundation, workspace member
PaneId, TabId, WindowId, SessionId, DomainId, newtype IDs, IdAllocator
SplitTree, immutable tree, structural sharing, Arc, COW
SplitDirection, Horizontal, Vertical, ratio, split_at, remove, equalize
FloatingLayer, FloatingPane, z_order, hit_test, floating overlay
LayoutDescriptor, PaneLayout, DividerLayout, compute_layout, compute_dividers
pixel rect, cell grid snapping, divider_px, min_pane_cells
spatial navigation, navigate, Direction, Up, Down, Left, Right, cycle
nearest_pane, ray cast, directional movement
```

---

### Section 30: Pane Extraction + Domain System
**File:** `section-30-pane-domain.md` | **Tier:** 4M | **Status:** Complete

```
Pane, PaneId, pane struct, per-shell state, pane lifecycle
Arc<FairMutex<Term<MuxEventProxy>>>, terminal lock, mode_cache, AtomicU32
PtyWriter, ConPTY deadlock, VTE responses outside lock, background thread drop
Domain, Domain trait, DomainId, DomainState, spawn_pane, can_spawn
LocalDomain, local shell, portable-pty, shell spawning
WslDomain, WSL, wsl.exe, distro, Windows Subsystem for Linux
SpawnConfig, shell, cwd, env, max_scrollback, cursor_shape
PaneRegistry, PaneEntry, register, unregister, panes_in_tab
SessionRegistry, MuxTab, MuxWindow, tree_history, undo stack
MuxEventProxy, MuxEvent, MuxNotification, event bridge, coalescing
PaneOutput, PaneExited, PaneTitleChanged, PaneBell
```

---

### Section 31: In-Process Mux + Multi-Pane Rendering
**File:** `section-31-in-process-mux.md` | **Tier:** 4M | **Status:** Not Started

```
InProcessMux, in-process mux, synchronous fast path, no daemon
spawn_pane, close_pane, split_pane, create_tab, close_tab
event pump, poll_events, MuxEvent drain, notification channel
App rewiring, mux integration, pane store, thin GUI shell
prepare_pane_into, origin offset, viewport offset, multi-pane frame loop
divider rendering, focus border, accent color, inactive dimming
PaneRenderCache, CachedPaneFrame, per-pane caching, dirty check
single-pane fast path, zero overhead, backward compatibility
```

---

### Section 32: Tab & Window Management (Mux-Aware)
**File:** `section-32-tab-window-mux.md` | **Tier:** 4M | **Status:** Not Started

```
mux-aware tab, tab CRUD, new_tab_in_window, close_tab, duplicate_tab
cycle_tab, switch_to_tab, move_tab, CWD inheritance, auto-close
TermWindow, multi-window, shared GPU, surface, window ID mapping
winit_to_mux, mux_to_winit, bidirectional lookup
window lifecycle, create_window, close_window, exit_app
no-flash startup, render before show, DPI, Aero Snap, WS_EX_NOREDIRECTIONBITMAP
ConPTY-safe shutdown, exit-before-drop, background thread cleanup
cross-window tab movement, move_tab_to_window, move_tab_to_new_window
tear-off, multi-pane tab move, fullscreen toggle
handle_resize, handle_scale_factor_changed, resize ALL panes
```

---

### Section 33: Split Navigation + Floating Panes
**File:** `section-33-split-nav-floating.md` | **Tier:** 4M | **Status:** In Progress

```
spatial navigation, Alt+Arrow, focus pane direction, cycle pane
Alt+[, Alt+], forward, backward, tree order, click focus
Ctrl+Shift+D, split horizontal, Ctrl+Shift+E, split vertical
Ctrl+W, close pane, collapse split, ClosePane action
divider drag, mouse resize, ColResize, RowResize, 5px hit zone
keyboard resize, Alt+Shift+Arrow, ratio adjustment, equalize
zoom, unzoom, Ctrl+Shift+Z, zoomed_pane, auto-unzoom
floating pane, Ctrl+Shift+F, ToggleFloatingPane, overlay
float-tile toggle, Ctrl+Shift+G, ToggleFloatTile
floating drag, floating resize, snap-to-edge, minimum size
render_frame_scissored, drop shadow, floating z-order, raise, lower
undo split, redo split, Ctrl+Shift+U, Ctrl+Shift+Y, SplitHistory
```

---

### Section 34: IPC Protocol + Daemon Mode
**File:** `section-34-ipc-daemon.md` | **Tier:** 7A | **Status:** Not Started

```
wire protocol, binary protocol, 15-byte header, frame format
magic, version, type, payload_len, flags, seq, COMPRESSED
bincode, zstd, serialization, compression, codec
version negotiation, Hello, HelloAck, VersionMismatch
MuxServer, oriterm-mux, daemon, server event loop
ClientConnection, ClientId, subscriptions, push notifications
OutputCoalescer, coalesce timer, 1ms, 16ms, 100ms, tiered
focused pane coalesce, visible pane coalesce, hidden pane coalesce
backpressure, latest value channel, drop intermediate
MuxClient, client API, MuxBackend trait, transparent switching
auto-start daemon, fallback InProcessMux, reconnection
Unix domain socket, named pipe, IPC transport
shadow grid, reconnect snapshot, PaneContent
PID file, daemon lifecycle, --daemon, --stop, --persist
```

---

### Section 35: Session Persistence + Remote Domains
**File:** `section-35-persistence-remote.md` | **Tier:** 7A | **Status:** Not Started

```
session persistence, SessionSnapshot, save, load, restore
WindowSnapshot, TabSnapshot, PaneSnapshot, SplitTreeSnapshot
atomic write, auto-save, 30 seconds, JSON, session file
crash recovery, is_clean_shutdown, stale PID, restore prompt
restore_on_crash, ask, always, never, auto-recovery
scrollback archive, ScrollbackArchive, bincode, zstd, disk
unlimited scrollback, max_scrollback_memory, append-only
archive cleanup, retention, 7 days, 1GB limit
SshDomain, SSH, remote shell, openssh, thrussh, SSH channel
SshConfig, host, port, user, identity_file, proxy_command
SSH agent forwarding, X11 forwarding, keepalive, reconnect
WslDomain, WSL full, auto-detect, wsl --list, distro
path mapping, win_to_wsl, wsl_to_win, WSLENV
```

---

### Section 36: Remote Attach + Network Transport
**File:** `section-36-remote-attach.md` | **Tier:** 7A | **Status:** Not Started

```
remote attach, remote mux, network transport, TCP, TLS, rustls
Transport trait, LocalTransport, TcpTlsTransport, SshTunnelTransport
TOFU, trust on first use, certificate pinning, known_hosts, self-signed
listen_address, port 4622, dual-stack, IPv4, IPv6, connection limit
authentication, AuthMethod, SshKey, Token, challenge-response, nonce
authorized_keys, SSH agent, token auth, pre-shared secret, rate limiting
session token, reconnect token, brute-force resistant
RemoteMuxDomain, Domain trait, remote proxy, mixed local+remote
auto_reconnect, exponential backoff, reconnecting overlay, domain picker
oriterm connect, CLI, --ssh, --list, --status, SSH tunnel auto-detect
bandwidth-aware, RTT, EWMA, ping, pong, connection quality, jitter
adaptive coalescing, adaptive compression, zstd level, delta encoding
viewport-first rendering, scrollback on demand, progressive sync
predictive local echo, Mosh-style, reconcile, cautious, aggressive
connection quality indicator, green, yellow, red, latency display
```

---

### Section 37: TUI Client
**File:** `section-37-tui-client.md` | **Tier:** 7A | **Status:** Not Started

```
oriterm-tui, TUI client, terminal-in-terminal, headless, crossterm
tmux replacement, attach, detach, session management, prefix key
TuiApp, TuiRenderer, BufWriter, synchronized output, diff rendering
tab bar TUI, status bar, pane area, box-drawing, split borders
cell-by-cell rendering, escape sequence, SGR, color passthrough
color adaptation, truecolor, 256-color, 16-color, NO_COLOR, downgrade
floating pane TUI, overlay, z-order, shadow effect, box-drawing border
cursor passthrough, DECSCUSR, focused pane cursor, hide unfocused
prefix key, Ctrl+B, prefix mode, normal mode, copy mode
split horizontal, split vertical, zoom, unzoom, pane navigation
mouse input, click focus, tab switch, drag resize, scroll forward
attach flow, raw mode, subscribe, event loop, crossterm event poll
detach, Prefix+d, unsubscribe, restore terminal, RAII cleanup
TuiCleanup, Drop guard, panic hook, SIGINT, SIGTERM, SIGHUP
session list, new-session, kill-session, multi-client, shared session
remote attach TUI, --ssh, --host, connection status, auto-detach
```

---

## Quick Reference

| ID | Title | File | Tier | Status |
|----|-------|------|------|--------|
| 01 | Cell + Grid | `section-01-cell-grid.md` | 0 | Not Started |
| 02 | Terminal State Machine + VTE | `section-02-term-vte.md` | 0 | Not Started |
| 03 | Cross-Platform | `section-03-cross-platform.md` | 0 | Not Started |
| 04 | PTY + Event Loop | `section-04-pty-eventloop.md` | 1 | Not Started |
| 05 | Window + GPU Rendering | `section-05-window-gpu.md` | 2 | In Progress |
| 05B | Startup Performance | `section-05b-startup-perf.md` | 2 | Not Started (blocks 06) |
| 05C | Window Chrome | `section-05c-window-chrome.md` | 2 | Complete |
| 06 | Font Pipeline | `section-06-font-pipeline.md` | 2 | Complete |
| 07 | 2D UI Framework | `section-07-ui-framework.md` | 2 | Not Started |
| 08 | Keyboard Input | `section-08-keyboard-input.md` | 3 | Not Started |
| 09 | Selection & Clipboard | `section-09-selection-clipboard.md` | 3 | Not Started |
| 10 | Mouse Input & Reporting | `section-10-mouse-input.md` | 3 | Complete |
| 11 | Search | `section-11-search.md` | 3 | Not Started |
| 12 | Resize & Reflow | `section-12-resize-reflow.md` | 3 | Not Started |
| 13 | Configuration & Keybindings | `section-13-config-keybindings.md` | 3 | Not Started |
| 14 | URL Detection | `section-14-url-detection.md` | 3 | Not Started |
| 15 | Tab Struct & Management | `section-15-tab-management.md` | 4 | Superseded → 30, 32 |
| 16 | Tab Bar & Chrome | `section-16-tab-bar.md` | 4 | Not Started |
| 17 | Drag & Drop | `section-17-drag-drop.md` | 4 | Not Started |
| 18 | Multi-Window & Window Lifecycle | `section-18-multi-window.md` | 4 | Superseded → 32 |
| 19 | Event Routing & Render Scheduling | `section-19-event-routing.md` | 4 | Not Started |
| 20 | Shell Integration | `section-20-shell-integration.md` | 4 | Not Started |
| 21 | Context Menu & Window Controls | `section-21-context-menu.md` | 4 | Not Started |
| 22 | Terminal Modes | `section-22-terminal-modes.md` | 5 | Mostly Complete |
| 23 | Performance & Damage Tracking | `section-23-performance.md` | 5 | Not Started |
| 38 | Terminal Protocol Extensions | `section-38-protocol-extensions.md` | 5 | Not Started |
| 39 | Image Protocols | `section-39-image-protocols.md` | 5 | Not Started |
| 40 | Vi Mode + Copy Mode | `section-40-vi-copy-mode.md` | 3 | Not Started |
| 41 | Hints + Quick Select | `section-41-hints-quick-select.md` | 3 | Not Started |
| 24 | Visual Polish | `section-24-visual-polish.md` | 6 | Not Started |
| 25 | Theme System | `section-25-theme-system.md` | 6 | In Progress |
| 26 | Split Panes | `section-26-split-panes.md` | 7 | Superseded → 29, 31, 33 |
| 27 | Command Palette & Quick Terminal | `section-27-command-palette.md` | 7 | Not Started |
| 28 | Extensibility | `section-28-extensibility.md` | 7 | Not Started |
| 29 | Mux Crate + Layout Engine | `section-29-mux-layout-engine.md` | 4M | Not Started |
| 30 | Pane Extraction + Domain System | `section-30-pane-domain.md` | 4M | Complete |
| 31 | In-Process Mux + Multi-Pane Rendering | `section-31-in-process-mux.md` | 4M | Not Started |
| 32 | Tab & Window Management (Mux-Aware) | `section-32-tab-window-mux.md` | 4M | Not Started |
| 33 | Split Navigation + Floating Panes | `section-33-split-nav-floating.md` | 4M | In Progress |
| 34 | IPC Protocol + Daemon Mode | `section-34-ipc-daemon.md` | 7A | Not Started |
| 35 | Session Persistence + Remote Domains | `section-35-persistence-remote.md` | 7A | Not Started |
| 36 | Remote Attach + Network Transport | `section-36-remote-attach.md` | 7A | Not Started |
| 37 | TUI Client | `section-37-tui-client.md` | 7A | Not Started |
| 42 | Expose / Overview Mode | `section-42-expose-overview.md` | 5 | Not Started |
| 43 | Compositor Layer System + Animation Architecture | `section-43-compositor-layers.md` | 5 | Not Started |

## Tier Summary

| Tier | Sections | Theme |
|------|----------|-------|
| 0 | 01-03 | Core library + cross-platform architecture |
| 1 | 04 | Process layer (PTY, threads) |
| 2 | 05, 05B, 06-07 | Rendering foundation (window, GPU, fonts, UI framework) |
| 3 | 08-14, 40-41 | Interaction (keyboard, mouse, selection, search, config, vi mode, hints) |
| 4 | ~~15~~, 16-17, ~~18~~, 19-21 | Chrome + tab bar + drag (15/18 superseded by 4M) |
| **4M** | **29-33** | **Multiplexing foundation (mux crate, panes, domains, splits, floating)** |
| 5 | 22-23, 38-39, 42-43 | Hardening + features (terminal modes, performance, protocol extensions, image protocols, expose/overview, compositor layers) |
| 6 | 24-25 | Polish (visual refinements, themes) |
| 7 | ~~26~~, 27-28 | Advanced (command palette, extensibility) (26 superseded by 4M) |
| **7A** | **34-37** | **Server + persistence + remote (daemon, IPC, sessions, SSH, WSL, remote attach, TUI client)** |

## Dependency DAG

```
01 Cell + Grid
 |
02 Term + VTE
 |
03 Cross-Platform         <- platform abstractions (PTY, fonts, clipboard, GPU, window)
 |
04 PTY + Event Loop       <- builds on platform PTY abstraction
 ├──────────────────────────────────────┐
 |                                      |
05 Window + GPU                         29 Mux Crate + Layout Engine  (oriterm_mux)
 |                                      |
05B Startup Performance                 30 Pane Extraction + Domains
 |                                      |
06 Font Pipeline                   ┌────┤
 |                                 |    |
07 2D UI Framework                 |    31 In-Process Mux + Multi-Pane Rendering
 |                                 |    |     (depends on 29, 30, 05)
08-14 Interaction                  |    |
 |                                 |    32 Tab & Window Mgmt (Mux-Aware)
40 Vi Mode + Copy Mode             |    |
 |   (depends on 08, 09, 11)       |    |
41 Hints + Quick Select             |    |
 |   (depends on 08, 14)           |    |
 |                                 |    |     (depends on 31)
16-17, 19-21 Chrome                |    |
 |  (tab bar, drag, routing,       |    33 Split Nav + Floating Panes
 |   shell integration, menus)     |    |     (depends on 31)
 |                                 |    |
 └─────────────┬───────────────────┘    |
               |                        |
          22-23 Hardening               |
               |                        |
          43 Compositor Layers          |
               |  (depends on 05, 07)   |
               |  (consumed by 24, 27,  |
               |   33.4, 39, 42)        |
               |                        |
          38 Protocol Extensions        |
               |  (depends on 02, 06,   |
               |   22)                  |
          39 Image Protocols            |
               |  (depends on 02, 05,   |
               |   06)                  |
               |                        |
          24-25 Polish                  |
               |                        |
          27-28 Advanced                |
               |                        |
               └────────────┬───────────┘
                            |
                       34 IPC Protocol + Daemon Mode
                            |     (depends on 32)
                            |
                       35 Session Persistence + Remote Domains
                            |     (depends on 34)
                            |
                       36 Remote Attach + Network Transport
                            |     (depends on 34; benefits from 35)
                            |
                       37 TUI Client (oriterm-tui)
                                  (depends on 36; can connect locally via 34)

  ~~15~~ Tab Struct           -> SUPERSEDED by 30, 32
  ~~18~~ Multi-Window         -> SUPERSEDED by 32
  ~~26~~ Split Panes          -> SUPERSEDED by 29, 31, 33
```
