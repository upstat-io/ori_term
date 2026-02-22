<img src="assets/icon.svg" width="128" height="128" alt="ori-term">

# ori-term

A GPU-accelerated terminal emulator with a built-in multiplexer, written from scratch in Rust.

Most terminal emulators make you choose: a fast GPU renderer (Alacritty, Ghostty) or a built-in multiplexer (WezTerm), but never both done well — and you still end up running tmux inside any of them. ori-term eliminates that entire layer. Splits, floating panes, tabs, sessions, and daemon mode are native. Close your laptop, reopen it, and your session is exactly where you left it. SSH into a remote machine and your panes feel local with Mosh-style predictive echo. Or skip the GUI entirely — `oriterm-tui` attaches to the same daemon from any terminal, giving you a tmux-like experience backed by the same multiplexer.

Built by studying 18 terminal projects from the inside out — Alacritty, Ghostty, WezTerm, tmux, Mosh, Chrome, VS Code, Windows Terminal, and others. Cross-platform (Windows, Linux, macOS) from day one — no platform is primary, no platform is an afterthought.

### What sets it apart

- **Terminal + multiplexer + session manager in one** — no tmux, no screen, no extra layer between you and your shells
- **Daemon mode** — sessions survive after the GUI closes, crash recovery with auto-restore
- **Two clients, one daemon** — `oriterm` (GPU window) and `oriterm-tui` (terminal-in-terminal) connect to the same sessions
- **Floating panes** — not just tiled splits; overlay panes that float, drag, resize, and toggle between float and tile
- **Remote attach with predictive echo** — SSH/WSL domains with bandwidth-aware rendering and Mosh-style local echo
- **GPU-rendered UI framework** — the tab bar, context menus, settings, and overlays are all custom widgets rendered on the GPU, not native OS controls
- **Chrome-style tab drag** — tear tabs into new windows, drag them back in, reorder with animation
- **Image protocols** — Kitty graphics, Sixel, and iTerm2 inline images composited on the GPU
- **Lua scripting** — event hooks, custom commands, and post-processing shaders (WGSL)

## Architecture

Four-crate workspace with strictly one-way dependencies:

- **oriterm_core** — Pure terminal library (grid, VTE, selection, search). No GUI, no PTY, no platform.
- **oriterm_mux** — Multiplexing layer (split trees, domains, IPC, session persistence). No GUI, no fonts.
- **oriterm** — GUI binary (window, GPU rendering, PTY, fonts, chrome, platform integration).
- **oriterm_tui** — TUI client binary (terminal-in-terminal rendering via crossterm, tmux replacement).

## Features

### Window & GPU Rendering
- **GPU-accelerated** — wgpu with Vulkan + DX12 on Windows, Vulkan on Linux, Metal on macOS
- **Staged render pipeline** — Extract (lock+snapshot+unlock) → Prepare (pure CPU, testable) → Render (GPU submission)
- **Custom window chrome** — frameless window, tab bar as title bar, pixel-drawn window controls
- **Window transparency** — compositor-backed glass (Mica/Acrylic on Windows, vibrancy on macOS)
- **Offscreen render targets** — tab previews, headless testing, thumbnails
- **Frame pacing** — 120 FPS budget with dirty aggregation and coalesced redraws
- **Damage tracking** — per-row dirty tracking, instance buffer caching, partial updates, skip-present when idle

### Terminal Core
- **Full VTE escape sequence handling** — SGR, cursor, erase, scroll regions, alternate screen, OSC, DCS, CSI
- **Text reflow on resize** — cell-by-cell reflow handling wide characters, wrapped lines, and scrollback
- **Scrollback** — ring buffer with configurable history and O(1) push
- **Mouse reporting** — X10, normal, button-event, any-event with SGR encoding
- **Synchronized output** — Mode 2026 for flicker-free rendering
- **Hyperlinks** — OSC 8 explicit hyperlinks + implicit URL detection with hover underline and Ctrl+click open
- **Kitty keyboard protocol** — progressive enhancement with modifier reporting, CSI u encoding, mode stack
- **Bracketed paste** — escape sequence wrapping for paste-aware applications
- **Focus events** — Mode 1004 focus-in/focus-out reporting
- **Charset support** — G0/G1/G2/G3, DEC special graphics, SS2/SS3

### Terminal Protocol Extensions
- **Device Attributes** — DA1, DA2, DA3 responses for capability discovery
- **Device Status Report** — CSI 5 n, CSI 6 n cursor position reporting
- **Mode query** — DECRQM for progressive enhancement detection
- **Terminfo query** — XTGETTCAP (DCS + q) for capability discovery
- **Settings query** — DECRQSS for SGR, scroll region, and other setting queries
- **Color queries** — OSC 4, OSC 10, OSC 11, OSC 12 for palette and foreground/background/cursor color
- **Color reset** — OSC 104, OSC 110, OSC 111, OSC 112
- **Extended underline** — SGR 4:3 curly, dotted, dashed, double underline styles
- **Underline color** — SGR 58/59 colored underlines
- **Overline** — SGR 53/55
- **Window manipulation** — CSI t for window size/cell size reporting and resize
- **DCS passthrough** — tmux passthrough for nested terminal support

### Image Protocols
- **Kitty Graphics Protocol** — image transmission (chunked), placement, z-indexing, deletion
- **Sixel graphics** — DCS-based sixel image decoding and display
- **iTerm2 inline images** — OSC 1337 imgcat-compatible image display
- **GPU compositing** — images rendered as GPU textures alongside the terminal grid
- **Image animation** — animated GIF frame support
- **Memory management** — LRU eviction with configurable memory limits (256 MB default)

### Font Pipeline
- **Font shaping** — rustybuzz (HarfBuzz) for ligatures and complex scripts with two-phase shaping
- **Multi-face fallback** — Regular/Bold/Italic/BoldItalic + configurable fallback chain with cap-height normalization
- **Built-in glyphs** — box drawing, block elements, braille, powerline via lookup table rasterization
- **Color emoji** — RGBA atlas pages, VS15/VS16 presentation selectors, color bitmap and outline support
- **Text decorations** — underline (single, double, dotted, dashed, curly), strikethrough, colored underlines
- **Hinting** — DPI-aware auto-detected hinting modes for crisp text
- **Subpixel rendering** — LCD ClearType-style per-channel alpha (RGB/BGR)
- **Subpixel positioning** — fractional offset quantization for precise glyph placement
- **Font synthesis** — synthetic bold (embolden) and synthetic italic (14-degree skew) when faces are unavailable
- **OpenType features** — configurable per-font feature tags (liga, calt, kern, and custom)
- **Glyph atlas** — guillotine-packed multi-page texture array (2048x2048), LRU eviction, R8Unorm + Rgba8Unorm pages
- **ASCII pre-cache** — scratch buffer pre-caching for fast startup
- **UI text** — shaped text measurement, ellipsis truncation for tab bar and UI elements

### 2D UI Framework
- **GPU-rendered widgets** — buttons, checkboxes, toggles, sliders, text inputs, dropdowns, labels, panels, scrollbars
- **Drawing primitives** — rects, rounded rects, shadows, gradients, borders
- **Layout engine** — flexbox-style two-pass layout with Row/Column containers, Fixed/Fill/Hug sizing, padding, gap
- **Overlay system** — modals, context menus, tooltips, terminal preview popups
- **Animation** — easing functions, property transitions, animated values
- **Theming** — dark/light themes derived from terminal palette with accent colors
- **Hit testing** — widget-level hit testing with mouse capture and focus management
- **Keyboard navigation** — tab order, focus ring, full keyboard accessibility

### Tabs & Chrome
- **Chrome-style tabs** — tear off into new window, drag back in, reorder with smooth animation
- **Tab hover preview** — scaled-down live terminal thumbnail rendered via offscreen targets
- **Tab width lock** — close buttons don't shift during rapid close clicks
- **Bell animation** — pulsing background on inactive tabs receiving bell
- **Tab bar hit testing** — close button, new tab button, dropdown button, drag area detection
- **GPU-rendered context menus** — shadows, rounded corners, hover highlighting, separator support
- **Settings UI** — color scheme selector with checkmark indicators
- **Window controls** — platform-specific minimize/maximize/close buttons (Windows rectangular, macOS circular)
- **Frameless drag** — double-click to maximize, full Aero Snap support on Windows

### Selection & Clipboard
- **3-point selection** — anchor/pivot/end with sub-cell precision (Windows Terminal model)
- **Word/line/block modes** — double-click word, triple-click line, Alt+click block selection
- **Drag threshold** — 1/4 cell width before drag initiates, prevents accidental selection
- **Word boundaries** — delimiter-class-aware word expansion across soft wraps
- **Bracketed paste** — ESC[200~/ESC[201~ wrapping for paste-aware applications
- **Copy on select** — configurable auto-copy on mouse release
- **Formatted copy** — HTML and RTF clipboard formats alongside plain text
- **Keyboard selection** — mark mode with Shift+arrow selection
- **File drag-and-drop** — auto-quoted path insertion
- **OSC 52 clipboard** — application read/write clipboard access

### Vi Mode & Copy Mode
- **Modal navigation** — hjkl movement, toggle with Ctrl+Shift+Space
- **Word motions** — w, b, e, W, B, E with word/WORD boundary awareness
- **Line motions** — 0, ^, $, gg, G, H, M, L for start/end/viewport movement
- **Page scrolling** — Ctrl+U/D half-page, Ctrl+B/F full-page
- **Inline search** — f, F, t, T with ; and , repeat
- **Bracket matching** — % to jump between matching brackets
- **Visual selection** — v character, V line, Ctrl+V block selection modes
- **Yank** — y to copy selected text
- **Search integration** — /, ? forward/backward search with n, N to cycle matches
- **Auto-scroll** — zz to center view, scrollback navigation

### Hints & Quick Select
- **Pattern matching** — regex-based detection of URLs, file paths, git hashes, IP addresses, emails
- **Keyboard labels** — vimium-style alphabetic labels with progressive filtering
- **Configurable actions** — copy, open, copy-and-paste, select per pattern
- **Custom patterns** — user-defined regex with optional URL templates (JIRA, GitHub issues)
- **Hint rendering** — dimmed terminal with high-contrast label overlays
- **Per-pattern keybindings** — Ctrl+Shift+H and custom bindings per hint type

### Search
- **Plain text and regex** — full regex search with plain text fallback
- **Ctrl+F overlay** — search bar UI with match count and navigation
- **Match highlighting** — highlighted matches across viewport and scrollback
- **Next/previous** — keyboard navigation between matches

### Color
- **Truecolor** — 24-bit RGB, 256-color palette, and 16-color ANSI
- **100+ built-in themes** — Catppuccin, Dracula, Nord, Gruvbox, Solarized, Tokyo Night, One Dark, Rosé Pine, Kanagawa, and many more
- **TOML theme files** — user-defined and community themes with hot-reload
- **Light/dark auto-switch** — follows system appearance (AppsUseLightTheme on Windows, D-Bus on Linux, NSAppearance on macOS)
- **Theme conversion** — import from iTerm2, Ghostty, and base16 formats
- **Color profile detection** — NO_COLOR, CLICOLOR, CLICOLOR_FORCE, COLORTERM priority cascade
- **Graceful downgrade** — TrueColor → ANSI256 → ANSI16 → stripped

### Configuration
- **TOML config** — fonts, colors, keybindings, behavior, window settings
- **Hot reload** — file watcher triggers live config updates without restart
- **Configurable keybindings** — user-definable shortcuts with action binding
- **Font size zoom** — Ctrl+=/Ctrl+- to adjust font size
- **CLI subcommands** — command-line configuration and control

### Shell Integration
- **Shell detection** — automatic detection of bash, zsh, fish, PowerShell
- **Script injection** — ZDOTDIR/XDG_DATA_DIRS-based injection of shell scripts
- **OSC 7 CWD tracking** — current working directory reported by shell
- **OSC 133 semantic zones** — prompt/input/output zone marking for smart navigation
- **Prompt state** — prompt mark detection for command-aware features
- **Title management** — effective title from explicit OSC 2 or CWD short path
- **Keyboard mode stack** — proper save/restore across alternate screen swaps
- **Notifications** — OSC 9, OSC 99, OSC 777 desktop notification support
- **XTVERSION** — CSI >q version identification response

### Split Panes
- **Horizontal and vertical splits** — Ctrl+Shift+D / Ctrl+Shift+E
- **Immutable split tree** — structural sharing via Arc for efficient copy-on-write
- **Spatial navigation** — Alt+Arrow to focus pane by direction, Alt+[/] to cycle
- **Drag-to-resize dividers** — mouse drag with 5px hit zone, keyboard resize with Alt+Shift+Arrow
- **Equalize** — evenly distribute pane sizes
- **Zoom/unzoom** — Ctrl+Shift+Z to toggle full-tab zoom on focused pane
- **Close pane** — Ctrl+W to close with automatic tree collapse
- **Undo/redo splits** — Ctrl+Shift+U / Ctrl+Shift+Y split history
- **Focus border** — accent-colored border on active pane, dimming on inactive panes
- **Per-pane render cache** — dirty-checked cached frames per pane
- **Single-pane fast path** — zero overhead when only one pane is open

### Floating Panes
- **Toggle floating** — Ctrl+Shift+F to create/toggle floating pane overlay
- **Float-tile toggle** — Ctrl+Shift+G to move pane between floating and tiled
- **Drag and resize** — mouse-driven floating pane positioning with snap-to-edge
- **Z-ordering** — raise/lower floating panes, proper layering with drop shadows
- **Minimum size** — enforced minimum dimensions for floating panes

### Multi-Window
- **Shared GPU** — multiple windows sharing GPU device and surface management
- **Cross-window tab movement** — drag tabs between windows, move tabs to new windows
- **Per-window DPI** — proper scale factor handling per monitor
- **Aero Snap** — full Windows Aero Snap support with WndProc subclass
- **ConPTY-safe shutdown** — exit-before-drop pattern for clean Windows process cleanup

### Daemon Mode & IPC
- **Binary wire protocol** — 15-byte header with bincode serialization and zstd compression
- **Version negotiation** — Hello/HelloAck handshake with version mismatch detection
- **MuxServer daemon** — background daemon keeping sessions alive after GUI closes
- **Output coalescing** — tiered coalescing (1ms focused, 16ms visible, 100ms hidden) to minimize IPC traffic
- **Backpressure** — latest-value channels dropping intermediate frames under load
- **MuxClient** — transparent switching between in-process and daemon backends
- **Auto-start daemon** — automatic daemon launch with fallback to in-process mode
- **Unix domain socket / named pipe** — platform-appropriate IPC transport
- **PID file** — daemon lifecycle management with --daemon, --stop, --persist flags

### Session Persistence
- **Auto-save** — session state saved every 30 seconds (window/tab/pane/split tree layout)
- **Crash recovery** — stale PID detection with restore prompt on next launch
- **Scrollback archive** — append-only bincode+zstd compressed scrollback to disk
- **Unlimited scrollback** — configurable memory limit with disk-backed overflow
- **Archive cleanup** — retention policy (7 days, 1 GB limit)
- **Atomic writes** — safe session file updates

### Remote Domains
- **SSH domain** — spawn remote shells via SSH with agent forwarding and keepalive
- **WSL domain** — Windows Subsystem for Linux integration with auto-detect and path mapping
- **Mixed local+remote** — local and remote panes side by side in the same window

### Remote Attach
- **TCP+TLS transport** — rustls-based encrypted connections with TOFU certificate pinning
- **SSH tunnel mode** — automatic SSH tunnel detection and setup
- **Authentication** — SSH key, token, and challenge-response authentication with rate limiting
- **Reconnection** — exponential backoff with reconnecting overlay and auto-reconnect
- **Bandwidth-aware rendering** — RTT/EWMA connection quality monitoring, adaptive coalescing and compression
- **Viewport-first sync** — visible content sent first, scrollback on demand
- **Predictive local echo** — Mosh-style speculative local echo with reconciliation
- **Connection quality indicator** — green/yellow/red latency display
- **`oriterm connect` CLI** — --ssh, --list, --status for managing remote connections

### TUI Client (oriterm-tui)
- **Terminal-in-terminal** — full multiplexer rendered via crossterm escape sequences
- **tmux replacement** — attach, detach, list sessions, new session, kill session
- **Prefix key** — Ctrl+B prefix mode for split/navigation/session commands
- **Split rendering** — box-drawing borders for pane splits, floating pane overlays
- **Color adaptation** — automatic downgrade from truecolor → 256 → 16 → none based on host terminal
- **Cursor passthrough** — focused pane cursor style forwarded to host terminal
- **Mouse support** — click to focus, tab switch, drag resize, scroll forwarding
- **RAII cleanup** — Drop guards, panic hooks, signal handlers for clean terminal restoration
- **Multi-client** — multiple TUI clients can attach to the same session
- **Remote attach** — --ssh and --host for connecting to remote daemons

### Visual Polish
- **Cursor blink** — configurable blink with DECSCUSR steady/blinking styles
- **Hide cursor while typing** — cursor reappears on mouse move
- **Minimum contrast** — WCAG 2.0 luminance-based contrast enforcement
- **HiDPI** — proper scale factor handling with DPI-aware rendering
- **Smooth scrolling** — pixel-offset kinetic scrolling for trackpads
- **Background images** — PNG/JPEG background textures with configurable opacity and position

### Performance
- **Damage tracking** — per-row dirty bits to minimize GPU work
- **Instance buffer caching** — reuse buffers across frames, partial updates for changed rows
- **Ring buffer scrollback** — O(1) push with wrapping index
- **Fast ASCII path** — optimized parsing for ASCII-heavy terminal output
- **Memory optimization** — row pooling, compact 24-byte cell representation
- **Benchmarks** — criterion-based throughput, latency, and FPS regression testing

### Advanced
- **Command palette** — Ctrl+Shift+P fuzzy-search action picker
- **Quick terminal** — global hotkey dropdown (Quake-style, F12)
- **Desktop notifications** — OSC 9, OSC 777, OSC 99 with native toast support
- **Progress indicators** — OSC 9;4 taskbar progress (ITaskbarList3 on Windows)
- **Terminal inspector** — Ctrl+Shift+I debug overlay with escape sequence log
- **Lua scripting** — event hooks, custom commands, full API (oriterm.on, oriterm.new_tab, oriterm.send_text)
- **Custom shaders** — WGSL post-processing effects (CRT, bloom, scanlines)
- **Smart paste** — multi-line paste warning, ESC sanitization, large paste confirmation
- **Undo close tab** — Ctrl+Shift+T restores recently closed tabs with CWD
- **Session serialization** — workspace presets, broadcast input to all panes

### Cross-Platform
- **Windows** — ConPTY, DirectWrite, Vulkan + DX12, frameless with Aero Snap, Mica/Acrylic transparency, WndProc subclass
- **Linux** — PTY, fontconfig, Vulkan, X11 + Wayland, D-Bus theme detection
- **macOS** — PTY, CoreText, Metal, native vibrancy (NSVisualEffectView)

All three platforms are equal first-class targets. No platform is primary, no platform is an afterthought.

## Building

```bash
# Debug
cargo build --target x86_64-pc-windows-gnu

# Release
cargo build --target x86_64-pc-windows-gnu --release

# Checks
./clippy-all.sh
./test-all.sh
./build-all.sh
```

Cross-compiled from WSL targeting `x86_64-pc-windows-gnu`.

## Inspiration

| Project | What inspired us |
|---------|-----------------|
| Ghostty | Cell-by-cell text reflow approach |
| Alacritty | Term\<T\> architecture, FairMutex, VTE crate, strict clippy |
| WezTerm | Cross-platform PTY abstraction, multiplexer domain model |
| Chrome | Tab drag state machine, GPU-rendered UI, tab previews |
| VS Code | Frameless window chrome pattern |
| Windows Terminal | Selection behavior and clipboard UX |
| Bevy | Staged render pipeline (Extract → Prepare → Render) |
| tmux | Session persistence, daemon architecture, TUI multiplexing |
| Mosh | Predictive local echo, bandwidth-aware rendering |
| Catppuccin | Default color palette (Mocha) |
| Ratatui | Clippy lint configuration, testing patterns |
| termenv / lipgloss | Color profile detection cascade |

## The Name

**ori** — from the Japanese 折り (folding). Tabs fold between windows the way you fold paper.

## License

MIT
