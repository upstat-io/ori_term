---
section: 3
title: Cross-Platform
status: in-progress
tier: 0
goal: Day-one first-class support for Windows, Linux, and macOS — all three platforms are equal targets from the start, with native PTY, fonts, clipboard, and GPU on each
sections:
  - id: "03.1"
    title: PTY Abstraction
    status: complete
  - id: "03.2"
    title: Platform Fonts
    status: complete
  - id: "03.3"
    title: Platform Clipboard
    status: not-started
  - id: "03.4"
    title: GPU Backend Selection
    status: not-started
  - id: "03.5"
    title: Window Management
    status: not-started
  - id: "03.6"
    title: Platform-Specific Code Paths
    status: not-started
  - id: "03.7"
    title: System Theme Detection
    status: not-started
  - id: "03.8"
    title: Section Completion
    status: not-started
---

# Section 03: Cross-Platform

**Status:** Not Started
**Goal:** ori_term runs natively on Windows, Linux, and macOS from day one. All three platforms are equal first-class targets — no platform is primary, no platform is an afterthought. Each uses its native PTY, font discovery, clipboard, and GPU backend.

**Crate:** `oriterm` (binary, platform-specific modules), `oriterm_core` (platform-agnostic)
**Dependencies:** `portable-pty`, `arboard` (or `clipboard-win`), `wgpu`, `winit`

**Reference:**
- Ghostty's platform abstraction with separate macOS/Linux/Windows implementations
- Alacritty's cross-platform support via `crossfont` and winit
- WezTerm's extensive cross-platform support including Wayland

**Current state:** This is a clean rebuild. All platform support is being built from scratch with cross-platform as a foundational design constraint, not a retrofit. The architecture uses `portable-pty` for cross-platform PTY (ConPTY on Windows, `openpty`/`forkpty` on Unix), `wgpu` for GPU rendering (Vulkan + DX12 on Windows, Vulkan on Linux, Metal on macOS), and `winit` for windowing. Every subsystem — PTY, fonts, clipboard, GPU, window management, config paths — must have working implementations for all three platforms before this section is considered complete. Platform-specific code is isolated behind `#[cfg(target_os)]` with no platform treated as the default or primary path.

---

## 03.1 PTY Abstraction

Cross-platform PTY via `portable-pty`. Each platform uses its native PTY implementation.

**Files:** `oriterm/src/pty/mod.rs`, `oriterm/src/pty/spawn.rs`, `oriterm/src/pty/reader.rs`, `oriterm/src/pty/signal.rs`

**Reference:** `_old/src/tab/mod.rs`, `portable-pty` crate docs

- [x] Cross-platform PTY via `portable-pty` crate:
  - [x] Windows: ConPTY (`portable_pty::native_pty_system()`) — Windows 10 1809+
  - [x] Linux: `openpty` / `forkpty` (same crate, automatic selection)
  - [x] macOS: POSIX PTY (same crate, automatic selection)
- [x] PTY resize via `pty_master.resize()` — works on all platforms
- [x] Background reader thread per tab:
  - [x] Reads PTY output in a dedicated thread
  - [x] Sends data to main thread via channel (or shared state)
  - [x] Thread exits cleanly when PTY is closed or child process exits
- [x] Shell detection:
  - [x] Windows: `cmd.exe` default (configurable via `terminal.shell` in config)
  - [x] Linux/macOS: reads `$SHELL` environment variable, defaults to `/bin/sh`
  - [x] Config override: `terminal.shell` takes priority on all platforms
- [x] Handle `SIGCHLD` on Unix for child process exit notification:
  - [x] Currently the PTY reader thread detects EOF when child exits
  - [x] Add explicit signal handling for robustness (catch zombie processes)
  - [x] Use `signal-hook` crate or manual `sigaction` setup
  - [x] On child exit: close the tab (or display "[process exited]" and await keypress)
- [x] Environment variable passthrough:
  - [x] Pass `TERM=xterm-256color` (or `oriterm` if terminfo is installed)
  - [x] Pass `COLORTERM=truecolor` for 24-bit color detection
  - [x] Pass `TERM_PROGRAM=oriterm` for shell integration detection
  - [x] Platform-specific: inherit `PATH`, `HOME`/`USERPROFILE`, `LANG`/`LC_*`
- [x] **Tests:**
  - [x] PTY creation succeeds on the current platform
  - [x] Shell detection returns a valid shell path
  - [x] Environment variables are set correctly in child process
  - [x] PTY resize does not error

---

## 03.2 Platform Fonts

Font discovery and loading using platform-native mechanisms. Current approach scans known filesystem paths; the goal is to also support platform font APIs for robustness.

**Files:** `oriterm/src/font/mod.rs`, `oriterm/src/font/discovery/mod.rs`, `oriterm/src/font/discovery/families.rs`, `oriterm/src/font/discovery/{linux,windows,macos}.rs`

**Reference:** `_old/src/render/font_discovery.rs`, `_old/src/font/collection.rs`, WezTerm `FontLocator`, Ghostty compile-time backend selection

### Windows Font Discovery

- [x] DirectWrite primary: `dwrote` crate resolves family name → file paths
  - [x] Weight-aware: Regular weight + CSS "bolder" (`min(weight+300, 900)`) for Bold
  - [x] Duplicate path filtering: if Bold path == Regular path, variant unavailable
- [x] Static path fallback: `C:\Windows\Fonts\` for known families
  - [x] JetBrainsMono > JetBrainsMonoNerdFont > CascadiaMonoNF > CascadiaMono > Consolas > Courier
- [x] Fallback fonts: Segoe UI Symbol (symbols), MS Gothic (CJK), Segoe UI (general)
  - [x] DirectWrite fallback first, then static paths (deduplicated)

### Linux Font Discovery

- [x] Recursive directory scan: `~/.local/share/fonts` → `/usr/share/fonts` → `/usr/local/share/fonts`
- [x] Build filename → path `HashMap` index (first-seen wins for priority)
- [x] Font family priority: JetBrainsMono > UbuntuMono > DejaVuSansMono > LiberationMono
- [x] Fallback fonts: NotoSansMono, NotoSansSymbols2, NotoSansCJK, DejaVuSans

### macOS Font Discovery

- [x] Same directory-scanning approach as Linux with macOS-specific paths
- [x] Scan: `~/Library/Fonts` → `/Library/Fonts` → `/System/Library/Fonts` → `/System/Library/Fonts/Supplemental`
- [x] Font family priority: JetBrainsMono > SF Mono > Menlo > Monaco > Courier
- [x] Fallback fonts: Apple Symbols, Hiragino Sans (CJK), Apple Color Emoji

### Embedded Fallback Font

- [x] Bundle JetBrains Mono Regular (~270KB) via `include_bytes!`
  - [x] SIL Open Font License (OFL.txt included in `oriterm/fonts/`)
  - [x] Prevents panic if no system fonts are found
  - [x] Load embedded font only as last resort after all platform paths fail
  - [x] Regular weight only — Bold/Italic/BoldItalic synthesized by renderer

### Config Font Override

- [x] `discover_fonts(family_override, weight)` accepts user-specified family name
  - [x] Windows: DirectWrite first, then static path
  - [x] Linux/macOS: directory scan with naming convention matching
  - [x] Absolute path support on all platforms
  - [x] Falls back to default priority list if override not found (with log warning)
- [x] `resolve_user_fallback(family)` resolves individual fallback font names

- [x] **Tests:** (12 tests total)
  - [x] `embedded_font_is_valid` — swash parses the embedded bytes
  - [x] `embedded_family_has_correct_origin` — origin/variants/paths correct
  - [x] `family_spec_consistency` — all FamilySpec entries have non-empty regular
  - [x] `fallback_spec_consistency` — all FallbackSpec entries have non-empty filenames
  - [x] `discover_finds_at_least_one_font` — always succeeds (embedded guarantees)
  - [x] `unknown_family_falls_back` — bogus name doesn't panic
  - [x] `discovered_regular_path_exists` — if path is Some, file exists
  - [x] `discovered_fallback_paths_exist` — all fallback paths exist
  - [x] `resolve_user_fallback_nonexistent` — returns None for bogus name
  - [x] `different_weights_succeed` — weights 100–900 all work
  - [x] `embedded_font_size_reasonable` — > 50KB sanity check
  - [x] `discovery_result_consistency` — has_variant matches paths, origin consistency
  - [x] `font_index_finds_files` (Linux-only) — indexed paths exist
  - [x] `linux_finds_dejavu` (Linux-only) — DejaVu found if installed

---

## 03.3 Platform Clipboard

Clipboard read/write for copy and paste operations.

**Files:** `oriterm/src/clipboard.rs`

**Reference:** `_old/src/clipboard.rs`, `arboard` crate

- [ ] Windows: `clipboard-win` crate (lightweight, Windows-specific)
  - [ ] `get_text()` via `clipboard_win::get_clipboard_string()`
  - [ ] `set_text()` via `clipboard_win::set_clipboard_string()`
- [ ] Linux / macOS: `arboard` crate (cross-platform)
  - [ ] `arboard` provides: X11, Wayland, macOS (NSPasteboard), and Windows support
  - [ ] API: `Clipboard::new()?.get_text()`, `Clipboard::new()?.set_text(text)`
  - [ ] X11: handles both PRIMARY (middle-click paste) and CLIPBOARD (Ctrl+V paste) selections
  - [ ] Wayland: uses `wl_data_device` protocol for clipboard access
  - [ ] macOS: uses `NSPasteboard` (general pasteboard)
- [ ] Architecture decision: keep `clipboard-win` for Windows (lighter dependency), use `arboard` for Linux/macOS
  - [ ] Alternative: use `arboard` everywhere for uniform API (simpler code, one more dependency on Windows)
  - [ ] Behind `#[cfg(target_os)]` conditional compilation either way
- [ ] OSC 52 clipboard (application-driven clipboard access):
  - [ ] Already works on all platforms (base64 encode/decode is pure Rust)
  - [ ] Applications can read/write clipboard via escape sequences
  - [ ] Security: configurable — allow read, write, both, or neither
- [ ] Clipboard trait abstraction:
  - [ ] `trait ClipboardProvider { fn get_text(&self) -> Option<String>; fn set_text(&self, text: &str) -> bool; }`
  - [ ] Platform implementations behind the trait
  - [ ] Testable with a mock implementation
- [ ] **Tests:**
  - [ ] Clipboard round-trip: set text, get text, verify match (integration test, may require windowed environment)
  - [ ] OSC 52 base64 encoding/decoding is correct
  - [ ] Clipboard trait mock works in unit tests

---

## 03.4 GPU Backend Selection

wgpu auto-selects the best GPU backend per platform. Platform-specific configuration is needed for transparency and compositing.

**Files:** `oriterm/src/gpu/state.rs`, `oriterm/src/gpu/pipeline.rs`

**Reference:** `_old/src/gpu/state.rs`, `_old/src/gpu/pipeline.rs`

- [ ] wgpu backend selection:
  - [ ] Windows: Vulkan and DX12 (both first-class, wgpu auto-selects best available)
  - [ ] Linux: Vulkan
  - [ ] macOS: Metal
  - [ ] `wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL, .. })`
- [ ] Windows transparency (DirectComposition):
  - [ ] Use `wgpu::CompositeAlphaMode::PreMultiplied` with DComp surface
  - [ ] Requires `CreateSwapChainForComposition` path in wgpu
  - [ ] Acrylic/Mica blur via `DwmSetWindowAttribute` (Windows 11)
  - [ ] Fallback: opaque background on Windows 10 without DWM composition
- [ ] Linux transparency:
  - [ ] X11: ARGB visual for composited transparency (requires compositor like Picom, KWin, Mutter)
  - [ ] Wayland: compositor handles transparency natively via surface alpha
  - [ ] Test with common compositors: Picom, KWin, Mutter, Sway
  - [ ] Fallback: opaque background when no compositor is running
- [ ] macOS transparency:
  - [ ] `NSVisualEffectView` for vibrancy/blur effects
  - [ ] `window-vibrancy` crate provides macOS support
  - [ ] Standard alpha transparency via `NSWindow.isOpaque = false`
- [ ] Surface format selection:
  - [ ] Prefer sRGB formats (`Bgra8UnormSrgb`, `Rgba8UnormSrgb`) for correct color rendering
  - [ ] Fallback to non-sRGB if preferred format is unavailable
  - [ ] Log the selected adapter, backend, and surface format at startup
- [ ] **Tests:**
  - [ ] GPU adapter is successfully created on the current platform (integration test)
  - [ ] Surface format is sRGB-capable
  - [ ] Pipeline creation does not error

---

## 03.5 Window Management

Platform-appropriate window creation and management. The current approach uses a frameless (borderless) window with a custom title bar on Windows. Other platforms may need different strategies.

**Files:** `oriterm/src/window.rs`, `oriterm/src/app/event_loop.rs`

**Reference:** `_old/src/window.rs`, `_old/src/app/event_loop.rs`

- [ ] Windows:
  - [ ] Frameless window with custom title bar (current approach, working)
  - [ ] Custom drag regions for window move and resize
  - [ ] `drag_window()` and `drag_resize_window()` via winit
  - [ ] Window snap (Aero Snap) works with frameless windows
  - [ ] DPI awareness: handle `ScaleFactorChanged` for high-DPI displays
- [ ] Linux:
  - [ ] X11 window management:
    - [ ] Test `drag_window()` — may require `_NET_WM_MOVERESIZE` for some WMs
    - [ ] Test `drag_resize_window()` — may not work on all WMs
    - [ ] Decision: frameless by default or respect WM decorations?
    - [ ] If frameless: need to implement client-side decorations (CSD) or use GTK/libdecor
    - [ ] If decorated: use server-side decorations (SSD) from the WM
  - [ ] Wayland window management:
    - [ ] Wayland requires client-side decorations (SSD is optional and WM-dependent)
    - [ ] `winit` handles basic Wayland support; test with Sway and GNOME Wayland
    - [ ] `drag_window()` uses `xdg_toplevel.move` — should work
  - [ ] Test with common WMs/DEs: GNOME, KDE, Sway, i3, Hyprland
- [ ] macOS:
  - [ ] Native title bar with traffic light buttons, or frameless with custom title bar
  - [ ] Handle `NSWindow` full screen properly (green button, Mission Control)
  - [ ] Menu bar integration: File, Edit, View, Window, Help menus
  - [ ] Respect macOS window management conventions (snap, Spaces, tabs)
  - [ ] Handle Retina (HiDPI) displays via `ScaleFactorChanged`
- [ ] **Tests:**
  - [ ] Window creation succeeds on the current platform (integration test)
  - [ ] DPI scale factor is correctly detected
  - [ ] Window resize events are handled without panic

---

## 03.6 Platform-Specific Code Paths

Audit and implement all platform-conditional code paths. Every `#[cfg(target_os = "windows")]` block needs a working alternative for Linux and macOS.

**Files:** Various — `oriterm/src/app/event_loop.rs`, `oriterm/src/render/font_discovery.rs`, `oriterm/src/clipboard.rs`, `oriterm/src/config/io.rs`

**Reference:** `_old/src/app/event_loop.rs`, `_old/src/render/font_discovery.rs`, `_old/src/clipboard.rs`, `_old/src/config/io.rs`

### URL Opening

- [ ] Windows: `ShellExecuteW` (Win32 API) — current implementation
- [ ] Linux: `xdg-open <url>` subprocess
- [ ] macOS: `open <url>` subprocess
- [ ] Unified API: `fn open_url(url: &str) -> io::Result<()>` with `#[cfg]` dispatch
- [ ] Validate URL scheme before opening (prevent command injection)

### Config Paths

- [ ] Windows: `%APPDATA%\oriterm\config.toml`
- [ ] Linux: `$XDG_CONFIG_HOME/oriterm/config.toml` (fallback: `~/.config/oriterm/config.toml`)
- [ ] macOS: `~/Library/Application Support/oriterm/config.toml`
- [ ] Unified API: `fn config_dir() -> PathBuf` with `#[cfg]` dispatch
- [ ] Create config directory if it does not exist (with appropriate permissions)

### Transparency

- [ ] Windows: DirectComposition + DWM blur (see 03.4)
- [ ] Linux: compositor-dependent ARGB visual (see 03.4)
- [ ] macOS: `NSVisualEffectView` vibrancy (see 03.4)
- [ ] Config: `window.opacity` (0.0-1.0), `window.blur` (bool)
- [ ] Graceful degradation: if transparency is not supported, fall back to opaque

### Process Management

- [ ] Windows: `CreateProcessW` via `portable-pty` (handled by crate)
- [ ] Linux/macOS: `fork` + `exec` via `portable-pty` (handled by crate)
- [ ] Signal handling: `SIGCHLD` (Unix only), `SIGTERM`/`SIGINT` for clean shutdown
- [ ] Windows: no POSIX signals — use `SetConsoleCtrlHandler` for Ctrl+C handling

- [ ] **Tests:**
  - [ ] `config_dir()` returns a valid path on the current platform
  - [ ] `open_url()` does not panic with a valid URL (integration test)
  - [ ] Config file is created in the correct platform-specific directory

---

## 03.7 System Theme Detection

Detect the operating system's dark/light mode preference and adapt the terminal's default color scheme.

**Files:** `oriterm/src/config/mod.rs`, `oriterm/src/platform.rs` (new platform abstraction module)

**Reference:** Ghostty `src/apprt/` (per-platform surface backends), WezTerm appearance detection

- [ ] Windows:
  - [ ] Read `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`
  - [ ] Value 0 = dark mode, value 1 = light mode
  - [ ] Use `winreg` crate or raw Win32 `RegGetValueW`
  - [ ] Optional: listen for registry change notifications to detect runtime theme switches
- [ ] macOS:
  - [ ] Query `NSAppearance.currentAppearance` via `objc` crate or `cocoa` bindings
  - [ ] `NSAppearanceNameDarkAqua` = dark mode, `NSAppearanceNameAqua` = light mode
  - [ ] Listen for `NSApplication.effectiveAppearance` KVO changes for runtime detection
- [ ] Linux:
  - [ ] Query `org.freedesktop.appearance.color-scheme` via D-Bus (`org.freedesktop.portal.Settings`)
  - [ ] Value 1 = dark, value 2 = light, value 0 = no preference
  - [ ] Use `zbus` crate for D-Bus communication
  - [ ] Fallback: check `GTK_THEME` environment variable for "dark" substring
  - [ ] Fallback: check `$XDG_CURRENT_DESKTOP` and query DE-specific settings
- [ ] Unified API:
  - [ ] `fn system_theme() -> Theme` where `Theme` is `Dark`, `Light`, or `Unknown`
  - [ ] Called at startup to select default color scheme
  - [ ] Config override: `appearance.theme = "dark" | "light" | "auto"` — `auto` uses system detection
- [ ] Adapt default palette:
  - [ ] Dark mode: dark background, light text (current default)
  - [ ] Light mode: light background, dark text
  - [ ] User-configured palette always takes priority over system theme
- [ ] **Tests:**
  - [ ] `system_theme()` returns a valid `Theme` variant on the current platform
  - [ ] Config override `"dark"` / `"light"` ignores system detection
  - [ ] `"auto"` uses system detection result

---

## 03.8 Section Completion

- [ ] All 03.1-03.7 items complete
- [ ] Terminal runs on Windows with ConPTY, Vulkan/DX12, and full functionality
- [ ] Terminal runs on Linux with openpty, Vulkan, and clipboard support
  - [ ] Tested on X11 and Wayland
- [ ] Terminal runs on macOS with openpty, Metal, and clipboard support
- [ ] Font discovery works on all three platforms (falls back to embedded font if needed)
- [ ] Clipboard copy/paste works on all three platforms
- [ ] GPU rendering works on all three platforms
- [ ] Default shell detected correctly per platform
- [ ] Window decorations appropriate per platform
- [ ] URL opening works per platform
- [ ] Config paths follow platform conventions
- [ ] Transparency works where compositor supports it
- [ ] System theme detection selects appropriate default palette
- [ ] No platform-specific panics or crashes
- [ ] CI builds for all three platforms
- [ ] `cargo test --target x86_64-pc-windows-gnu` — passes
- [ ] `cargo test` (native Linux) — passes
- [ ] `cargo clippy --target x86_64-pc-windows-gnu` — no warnings

**Exit Criteria:** ori_term builds and runs on Windows, Linux, and macOS with native PTY, font discovery, clipboard, GPU rendering, and system theme detection on each platform. No platform is broken or missing core functionality.
