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
    status: complete
  - id: "03.4"
    title: GPU Backend Selection
    status: complete
  - id: "03.5"
    title: "Window Management — oriterm_ui Crate Foundation"
    status: complete
  - id: "03.6"
    title: Platform-Specific Code Paths
    status: in-progress
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

- [x] Windows: `clipboard-win` crate (lightweight, Windows-specific)
  - [x] `get_text()` via `clipboard_win::get_clipboard_string()`
  - [x] `set_text()` via `clipboard_win::set_clipboard_string()`
- [x] Linux / macOS: `arboard` crate (cross-platform)
  - [x] `arboard` provides: X11, Wayland, macOS (NSPasteboard), and Windows support
  - [x] API: `Clipboard::new()?.get_text()`, `Clipboard::new()?.set_text(text)`
  - [x] X11: handles both PRIMARY (middle-click paste) and CLIPBOARD (Ctrl+V paste) selections
  - [x] Wayland: uses `wl_data_device` protocol for clipboard access
  - [x] macOS: uses `NSPasteboard` (general pasteboard)
- [x] Architecture decision: keep `clipboard-win` for Windows (lighter dependency), use `arboard` for Linux/macOS
  - [x] Alternative: use `arboard` everywhere for uniform API (simpler code, one more dependency on Windows)
  - [x] Behind `#[cfg(target_os)]` conditional compilation either way
- [x] OSC 52 clipboard (application-driven clipboard access):
  - [x] Already works on all platforms (base64 encode/decode is pure Rust)
  - [x] Applications can read/write clipboard via escape sequences
  - [x] Security: configurable — allow read, write, both, or neither  <!-- blocked-by:13 -->
- [x] Clipboard trait abstraction:
  - [x] `trait ClipboardProvider { fn get_text(&self) -> Option<String>; fn set_text(&self, text: &str) -> bool; }`
  - [x] Platform implementations behind the trait
  - [x] Testable with a mock implementation
- [x] **Tests:**
  - [x] Clipboard round-trip: set text, get text, verify match (integration test, may require windowed environment)
  - [x] OSC 52 base64 encoding/decoding is correct
  - [x] Clipboard trait mock works in unit tests

---

## 03.4 GPU Backend Selection

wgpu auto-selects the best GPU backend per platform. Platform-specific configuration is needed for transparency and compositing.

**Files:** `oriterm/src/gpu/state.rs`, `oriterm/src/gpu/pipeline.rs`

**Reference:** `_old/src/gpu/state.rs`, `_old/src/gpu/pipeline.rs`

- [x] wgpu backend selection:
  - [x] Windows: Vulkan and DX12 (both first-class, wgpu auto-selects best available)
  - [x] Linux: Vulkan
  - [x] macOS: Metal
  - [x] `wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL, .. })`
- [x] Windows transparency (DirectComposition):
  - [x] Use `wgpu::CompositeAlphaMode::PreMultiplied` with DComp surface
  - [x] Requires `CreateSwapChainForComposition` path in wgpu
  - [x] Acrylic/Mica blur via `DwmSetWindowAttribute` (Windows 11)
  - [x] Fallback: opaque background on Windows 10 without DWM composition
- [x] Linux transparency:
  - [x] X11: ARGB visual for composited transparency (requires compositor like Picom, KWin, Mutter)
  - [x] Wayland: compositor handles transparency natively via surface alpha
  - [x] Test with common compositors: Picom, KWin, Mutter, Sway
  - [x] Fallback: opaque background when no compositor is running
- [x] macOS transparency:
  - [x] `NSVisualEffectView` for vibrancy/blur effects
  - [x] `window-vibrancy` crate provides macOS support
  - [x] Standard alpha transparency via `NSWindow.isOpaque = false`
- [x] Surface format selection:
  - [x] Prefer sRGB formats (`Bgra8UnormSrgb`, `Rgba8UnormSrgb`) for correct color rendering
  - [x] Fallback to non-sRGB if preferred format is unavailable
  - [x] Log the selected adapter, backend, and surface format at startup
- [x] **Tests:**
  - [x] GPU adapter is successfully created on the current platform (integration test)
  - [x] Surface format is sRGB-capable
  - [x] Pipeline creation does not error

---

## 03.5 Window Management — `oriterm_ui` Crate Foundation

Chrome-style frameless window management with client-side decorations (CSD) on all platforms. This section creates the `oriterm_ui` crate — the seed that Section 07 grows into a full UI framework. The architecture follows Chromium's `ui/aura` + `ui/gfx/geometry` patterns: platform-independent geometry and hit-test logic with thin per-platform glue layers.

**Crate:** `oriterm_ui` (new workspace member)
**Dependencies:** `log`, `winit`; `windows-sys` on Windows only

**Reference:**
- Chromium `ui/gfx/geometry/` — Point, Size, Rect, Insets (reference repo: `~/projects/reference_repos/chromium_ui/`)
- Chromium `ui/aura/window_targeter.h` — pluggable hit-test strategy
- Chromium `ui/aura/window_delegate.h` — `GetNonClientComponent(point)` = our `hit_test()`
- Chromium `chrome/browser/ui/views/frame/` — `BrowserFrameWin`, WndProc subclass for snap/shadow

**Architecture:**

| Layer | Chrome equivalent | Our module | Platform-specific? |
|-------|-------------------|------------|-------------------|
| Geometry | `ui/gfx/geometry/` | `geometry.rs` | No |
| Scale | `ui/gfx/geometry/dip_util.h` | `scale.rs` | No |
| Hit testing | `WindowDelegate::GetNonClientComponent` | `hit_test.rs` | No |
| Window creation | `WindowTreeHost` | `window.rs` + `platform.rs` | `#[cfg]` dispatch |
| Platform glue | `PlatformWindow` | `platform_windows.rs`, etc. | Yes, per-platform |

### Geometry Types (`geometry.rs`)

Modeled after Chrome's `ui/gfx/geometry/`. All f32 logical pixels. Pure data, no platform deps, fully `const`/testable.

- [x] `Point` — `{ x: f32, y: f32 }`, `Debug + Clone + Copy + PartialEq + Default`
  - [x] `offset(dx, dy)`, `scale(sx, sy)`, `distance_to(other)`
- [x] `Size` — `{ width: f32, height: f32 }`, clamp near-zero to 0.0 (Chrome's epsilon pattern: `8 * f32::EPSILON`)
  - [x] `is_empty()`, `area()`, `scale(sx, sy)`
- [x] `Rect` — composed as `{ origin: Point, size: Size }` (Chrome pattern, not four independent fields)
  - [x] Half-open interval semantics: `contains()` uses `[x, x+w)` — standard for non-overlapping tiling
  - [x] `contains(point)`, `intersects(other)`, `intersection(other)`, `union(other)`
  - [x] `inset(insets)`, `offset(dx, dy)`, `center()`, `is_empty()`
  - [x] `from_origin_size(origin, size)`, `right()`, `bottom()`
- [x] `Insets` — `{ top: f32, right: f32, bottom: f32, left: f32 }`
  - [x] Factory methods: `Insets::all(v)`, `Insets::vh(v, h)`, `Insets::tlbr(t, l, b, r)`
  - [x] `width()` (left + right), `height()` (top + bottom)
  - [x] `Add`, `Sub`, `Neg` operator impls

### Scale Factor (`scale.rs`)

DPI scaling abstraction. Wraps winit's `f64` scale factor as a clamped newtype.

- [x] `ScaleFactor(f64)` — clamped to `[0.25, 8.0]`
  - [x] `new(factor)`, `factor(self) -> f64`
  - [x] `scale(logical) -> f64`, `unscale(physical) -> f64`
  - [x] `scale_u32(logical) -> u32` (rounded)
  - [x] `scale_point(Point) -> Point`, `scale_size(Size) -> Size`, `scale_rect(Rect) -> Rect`

### Hit Testing (`hit_test.rs`)

Chrome's `WM_NCHITTEST` equivalent as a **platform-independent pure function**. No OS types, no global state. The WndProc subclass on Windows calls this; the event loop calls it directly on Linux/macOS. 100% unit-testable on any platform.

- [x] `HitTestResult` enum — `Client`, `Caption`, `ResizeBorder(ResizeDirection)`
- [x] `ResizeDirection` enum — `Top`, `Bottom`, `Left`, `Right`, `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`
- [x] `hit_test(point, window_size, border_width, caption_height, interactive_rects, is_maximized) -> HitTestResult`
  - [x] Priority hierarchy (from Chrome's decision tree):
    1. Interactive rects within caption → `Client` (buttons/tabs are clickable, not draggable)
    2. Resize edges (unless maximized) → `ResizeBorder(direction)`
    3. Caption area → `Caption` (draggable title bar)
    4. Everything else → `Client`
  - [x] Corners take priority over edges (top-left corner = `TopLeft`, not `Top` or `Left`)
  - [x] Maximized windows have no resize borders

### Window Creation (`window.rs` + `platform.rs`)

Config-driven window creation. All platforms use frameless windows (Chrome-style CSD) from day one.

- [x] `WindowConfig` struct — `title`, `inner_size: Size`, `transparent: bool`, `blur: bool`, `position: Option<Point>` (scale factor queried from window post-creation)
- [x] `WindowError` enum — `Creation(winit::error::OsError)`
- [x] `create_window(event_loop, config) -> Result<Arc<Window>, WindowError>`
  - [x] Window created invisible (render first frame, then `set_visible(true)` to avoid flash)
- [x] `load_icon() -> Option<Icon>` — embedded application icon (RGBA, decoded at build time) (module-private)
- [x] `build_window_attributes(config) -> WindowAttributes` — per-platform `#[cfg]` dispatch (module-private):
  - [x] **All platforms:** `with_decorations(false)`, `with_visible(false)`, `with_transparent(config.transparent)`
  - [x] **Windows:** `with_no_redirection_bitmap(true)` when transparent
  - [x] **macOS:** `with_titlebar_transparent(true)`, `with_fullsize_content_view(true)`, `with_option_as_alt(Both)`
  - [x] **Linux:** `with_name("oriterm", "oriterm")` for X11 `WM_CLASS`

### Per-Platform Glue (thin layers, `#[cfg]`-gated)

Each platform needs a thin adapter that translates between OS window events and the platform-independent `hit_test()` function. These are the only files with platform-specific code.

- [x] **Windows** (`platform_windows.rs`):
  - [x] WndProc subclass for Aero Snap integration (Chrome pattern: `BrowserFrameWin`)
  - [x] `WM_NCHITTEST` handler calls `hit_test::hit_test()`, maps result to Windows HT constants
  - [x] `WM_NCCALCSIZE` — all-client-area trick + DWM 1px margin for shadow/snap
  - [x] `WM_DPICHANGED` — stores DPI for app to query
  - [x] `WM_MOVING` — position correction + merge detection for tab drag
  - [x] Public API: `enable_snap()`, `set_client_rects()`, `get_current_dpi()`, `begin_os_drag()`, `take_os_drag_result()`
- [x] **macOS** (`platform_macos.rs`):
  - [x] Frameless with transparent title bar + full-size content view
  - [x] Traffic light buttons positioned within custom chrome
  - [x] `NSWindow` full screen support (green button, Mission Control)
  - [x] Drag via winit's `drag_window()` — triggered by `hit_test() == Caption`
  - [x] Resize via winit's `drag_resize_window()` — triggered by `hit_test() == ResizeBorder`
  - [x] Retina (HiDPI) via `ScaleFactorChanged`
- [x] **Linux** (`platform_linux.rs`):
  - [x] Frameless CSD — same `hit_test()` drives drag/resize
  - [x] X11: `drag_window()` uses `_NET_WM_MOVERESIZE` (winit handles this)
  - [x] Wayland: `drag_window()` uses `xdg_toplevel.move` (winit handles this)
  - [x] Resize via winit's `drag_resize_window()` — triggered by `hit_test() == ResizeBorder`
  - [x] Test with GNOME, KDE, Sway, i3, Hyprland

### Workspace Integration

- [x] Add `oriterm_ui` to workspace `Cargo.toml` members
- [x] `oriterm_ui/Cargo.toml` — edition 2024, `[lints] workspace = true`
- [x] `oriterm/Cargo.toml` — add `oriterm_ui = { path = "../oriterm_ui" }` dependency
- [x] `oriterm_ui/src/lib.rs` — re-export modules:
  ```
  pub mod geometry;
  pub mod hit_test;
  pub mod scale;
  pub mod window;
  mod platform;
  #[cfg(target_os = "windows")] pub mod platform_windows;
  #[cfg(target_os = "macos")] pub mod platform_macos;
  #[cfg(target_os = "linux")] pub mod platform_linux;
  ```

### Tests (sibling `tests.rs` pattern)

- [x] `geometry/tests.rs`:
  - [x] `Rect::contains` — inside, outside, on-edge (half-open: left/top included, right/bottom excluded)
  - [x] `Rect::intersects` — overlapping, adjacent (no intersection), contained, disjoint
  - [x] `Rect::inset` — positive insets shrink, negative expand
  - [x] `Rect::union` — bounding box, one empty, both empty
  - [x] `Size` epsilon clamping — near-zero becomes 0.0
  - [x] `Point::offset`, `Point::distance_to`
- [x] `scale/tests.rs`:
  - [x] Clamping — values outside `[0.25, 8.0]` clamped
  - [x] `scale` / `unscale` roundtrip
  - [x] `scale_u32` rounding behavior
  - [x] `scale_rect` — origin and size both scaled
- [x] `hit_test/tests.rs`:
  - [x] Caption area — point in tab bar region returns `Caption`
  - [x] Client area — point in terminal grid returns `Client`
  - [x] All 8 resize directions — each edge and corner detected correctly
  - [x] Corner priority — point at corner returns corner, not edge
  - [x] Maximized — all resize borders suppressed, only `Caption` or `Client`
  - [x] Interactive rects — button within caption returns `Client`, not `Caption`
  - [x] Edge cases — point exactly on border width boundary

---

## 03.6 Platform-Specific Code Paths

Audit and implement all platform-conditional code paths. Every `#[cfg(target_os = "windows")]` block needs a working alternative for Linux and macOS.

**Files:** `oriterm/src/platform/url/mod.rs`, `oriterm/src/platform/config_paths/mod.rs`, `oriterm/src/platform/shutdown/mod.rs`, `oriterm/src/gpu/transparency.rs`

**Reference:** Chromium platform abstractions, Alacritty cross-platform modules, WezTerm platform support

### URL Opening

- [x] Windows: `ShellExecuteW` (Win32 API) — current implementation
- [x] Linux: `xdg-open <url>` subprocess
- [x] macOS: `open <url>` subprocess
- [x] Unified API: `fn open_url(url: &str) -> io::Result<()>` with `#[cfg]` dispatch
- [x] Validate URL scheme before opening (prevent command injection)

### Config Paths

- [x] Windows: `%APPDATA%\oriterm\config.toml`
- [x] Linux: `$XDG_CONFIG_HOME/oriterm/config.toml` (fallback: `~/.config/oriterm/config.toml`)
- [x] macOS: `~/Library/Application Support/oriterm/config.toml`
- [x] Unified API: `fn config_dir() -> PathBuf` with `#[cfg]` dispatch
- [x] Create config directory if it does not exist (with appropriate permissions)

### Transparency

- [x] Windows: DirectComposition + DWM blur (see 03.4)
- [x] Linux: compositor-dependent ARGB visual (see 03.4)
- [x] macOS: `NSVisualEffectView` vibrancy (see 03.4)
- [ ] Config: `window.opacity` (0.0-1.0), `window.blur` (bool) <!-- blocked-by:13 -->
- [x] Graceful degradation: if transparency is not supported, fall back to opaque

### Process Management

- [x] Windows: `CreateProcessW` via `portable-pty` (handled by crate)
- [x] Linux/macOS: `fork` + `exec` via `portable-pty` (handled by crate)
- [x] Signal handling: `SIGCHLD` (Unix only), `SIGTERM`/`SIGINT` for clean shutdown
- [x] Windows: no POSIX signals — use `SetConsoleCtrlHandler` for Ctrl+C handling

- [x] **Tests:**
  - [x] `config_dir()` returns a valid path on the current platform
  - [x] `open_url()` does not panic with a valid URL (integration test)
  - [x] Config file is created in the correct platform-specific directory

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
