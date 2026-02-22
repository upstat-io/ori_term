---
section: 13
title: Configuration & Keybindings
status: complete
tier: 3
goal: TOML configuration with file watching and hot reload, user-configurable keybindings with defaults
sections:
  - id: "13.1"
    title: Config Structs
    status: complete
  - id: "13.2"
    title: Config I/O
    status: complete
  - id: "13.3"
    title: Config File Watcher
    status: complete
  - id: "13.4"
    title: Config Hot Reload
    status: complete
  - id: "13.5"
    title: Keybinding System
    status: complete
  - id: "13.6"
    title: Default Keybindings
    status: complete
  - id: "13.7"
    title: Keybinding Config Parsing
    status: complete
  - id: "13.8"
    title: CLI Subcommands
    status: complete
  - id: "13.9"
    title: Shell Completion Scripts
    status: complete
  - id: "13.10"
    title: Section Completion
    status: complete
---

# Section 13: Configuration & Keybindings

**Status:** Complete
**Goal:** TOML-based configuration file with typed structs, file system watching for hot reload, and a user-configurable keybinding system with sensible defaults.

**Crate:** `oriterm` (binary)
**Dependencies:** `serde`, `toml`, `notify`
**Reference:** `_old/src/config/` (mod.rs, io.rs, monitor.rs, tests.rs), `_old/src/keybindings/` (mod.rs, defaults.rs, parse.rs, tests.rs)

**Prerequisite:** Section 04 (window + GPU — need running app to apply config changes)

---

## 13.1 Config Structs

Top-level config and per-section structs. All fields have defaults via `#[serde(default)]`.

**File:** `oriterm/src/config/mod.rs`

**Reference:** `_old/src/config/mod.rs`

- [x] `Config` struct (top-level)
  - [x] `#[derive(Debug, Clone, Default, Serialize, Deserialize)]`
  - [x] `#[serde(default)]`
  - [x] Fields:
    - `font: FontConfig`
    - `terminal: TerminalConfig`
    - `colors: ColorConfig`
    - `window: WindowConfig`
    - `behavior: BehaviorConfig`
    - `bell: BellConfig`
    - `keybind: Vec<KeybindConfig>` — user keybinding overrides
- [x] `FontConfig` struct <!-- unblocks:6.15 --><!-- unblocks:6.20 -->
  - [x] Fields:
    - `size: f32` — point size (default: from render::FONT_SIZE)
    - `family: Option<String>` — primary font family name
    - `weight: u16` — CSS font weight 100-900 (default: 400)
    - `tab_bar_font_weight: Option<u16>` — tab bar text weight (default: 600 via effective method)
    - `tab_bar_font_family: Option<String>` — tab bar font family (default: same as primary)
    - `features: Vec<String>` — OpenType features (default: `["calt", "liga"]`)
    - `fallback: Vec<FallbackFontConfig>` — ordered fallback font list
  - [x] `effective_weight(&self) -> u16` — clamped to [100, 900]
  - [x] `effective_bold_weight(&self) -> u16` — `min(900, weight + 300)` (CSS "bolder")
  - [x] `effective_tab_bar_weight(&self) -> u16` — clamped, defaults to 600
- [x] `FallbackFontConfig` struct
  - [x] Fields:
    - `family: String` — font family name or absolute path
    - `features: Option<Vec<String>>` — per-fallback OpenType feature overrides
    - `size_offset: Option<f32>` — point size adjustment relative to primary
- [x] `TerminalConfig` struct
  - [x] Fields:
    - `shell: Option<String>` — override shell (default: system shell)
    - `scrollback: usize` — scrollback lines (default: 10_000)
    - `cursor_style: String` — "block", "bar"/"beam", "underline" (default: "block")
    - `cursor_blink: bool` — enable cursor blinking (default: true)
    - `cursor_blink_interval_ms: u64` — blink interval (default: 530)
- [x] `ColorConfig` struct <!-- unblocks:3.7 -->
  - [x] Fields:
    - `scheme: String` — color scheme name (default: "Catppuccin Mocha")
    - `minimum_contrast: f32` — WCAG 2.0 contrast ratio 1.0-21.0 (default: 1.0 = off)
    - `alpha_blending: AlphaBlending` — text alpha blending mode
    - `foreground: Option<String>` — override fg color "#RRGGBB"
    - `background: Option<String>` — override bg color "#RRGGBB"
    - `cursor: Option<String>` — override cursor color "#RRGGBB"
    - `selection_foreground: Option<String>` — override selection fg
    - `selection_background: Option<String>` — override selection bg
    - `ansi: HashMap<String, String>` — override ANSI colors 0-7 by index
    - `bright: HashMap<String, String>` — override bright colors 8-15 by index
  - [x] `effective_minimum_contrast(&self) -> f32` — clamped to [1.0, 21.0]
- [x] `AlphaBlending` enum — `Linear`, `LinearCorrected` (default)
- [x] `WindowConfig` struct <!-- unblocks:3.6 -->
  - [x] Fields:
    - `columns: usize` — initial terminal columns (default: 120)
    - `rows: usize` — initial terminal rows (default: 30)
    - `opacity: f32` — window opacity 0.0-1.0 (default: 1.0)
    - `tab_bar_opacity: Option<f32>` — independent tab bar opacity (falls back to opacity)
    - `blur: bool` — enable backdrop blur (default: true)
    - `decorations: Decorations` — window decoration mode (default: `None` for frameless CSD)
    - `resize_increments: bool` — snap resize to cell boundaries (default: false)
  - [x] `effective_opacity(&self) -> f32` — clamped to [0.0, 1.0]
  - [x] `effective_tab_bar_opacity(&self) -> f32` — clamped, falls back to opacity when None
- [x] `Decorations` enum
  - [x] `Full` — OS-native title bar and borders
  - [x] `None` — frameless window with custom CSD (default)
  - [x] On Windows/Linux: maps to `with_decorations(bool)` in winit
  - [x] macOS extends with `Transparent` (transparent titlebar) and `Buttonless` (hide traffic lights) via winit macOS extensions
  - [x] **Ref:** Alacritty `config/window.rs:183-189`, winit `WindowAttributes::with_decorations`
- [x] `BehaviorConfig` struct <!-- unblocks:9.6 -->
  - [x] Fields:
    - `copy_on_select: bool` — auto-copy on selection release (default: true)
    - `bold_is_bright: bool` — bold text uses bright colors (default: true)
    - `shell_integration: bool` — enable shell integration injection (default: true)
- [x] `BellConfig` struct
  - [x] Fields:
    - `animation: String` — "ease_out", "linear", "none" (default: "ease_out")
    - `duration_ms: u16` — flash duration, 0 = disabled (default: 150)
    - `color: Option<String>` — flash color "#RRGGBB" (default: white)
  - [x] `is_enabled(&self) -> bool` — `duration_ms > 0 && animation != "none"`

---

## 13.2 Config I/O

Path resolution, loading, saving, and cursor style parsing.

**File:** `oriterm/src/config/io.rs`

**Reference:** `_old/src/config/io.rs`

- [x] `config_dir() -> PathBuf`
  - [x] Windows: `%APPDATA%/ori_term`
  - [x] Linux: `$XDG_CONFIG_HOME/ori_term` or `~/.config/ori_term`
  - [x] Fallback: `./ori_term`
- [x] `config_path() -> PathBuf` — `config_dir().join("config.toml")`
- [x] `state_path() -> PathBuf` — `config_dir().join("state.toml")` (window geometry persistence)
- [x] `WindowState` struct — `{ x: i32, y: i32, width: u32, height: u32 }`
  - [x] `WindowState::load() -> Option<Self>` — read from state.toml, None on missing/invalid
  - [x] `WindowState::save(&self)` — write to state.toml, create dir if needed
- [x] `Config::load() -> Self`
  - [x] Read from `config_path()`
  - [x] `NotFound`: return defaults (first run)
  - [x] Parse error: log warning, return defaults
  - [x] Success: log path, return parsed config
- [x] `Config::try_load() -> Result<Self, String>`
  - [x] Preserves error distinction (file missing vs parse error)
  - [x] Used by hot reload: parse error keeps previous config
- [x] `Config::save(&self)` — serialize to TOML, write to config_path
- [x] `parse_cursor_style(s: &str) -> CursorShape`
  - [x] "block" | "Block" -> Block
  - [x] "bar" | "beam" -> Beam
  - [x] "underline" -> Underline
  - [x] Unknown -> Block (default)
- [x] `save_toml(value, path, label)` — private helper: serialize, create dirs, write
- [x] **Tests** (`oriterm/src/config/tests.rs`):
  - [x] Default config roundtrip: serialize then deserialize equals defaults
  - [x] Partial TOML uses defaults for missing fields
  - [x] Empty TOML gives full defaults
  - [x] Cursor style parsing: all variants
  - [x] Opacity clamping: values outside [0.0, 1.0] clamped
  - [x] Minimum contrast clamping: values outside [1.0, 21.0] clamped
  - [x] Color overrides roundtrip: foreground, background, cursor, selection
  - [x] ANSI color overrides: per-index overrides, unset indices remain None
  - [x] Font weight: defaults, clamping, bold derivation
  - [x] Tab bar opacity: independent from window opacity, falls back when None
  - [x] Alpha blending: defaults to LinearCorrected, parses from TOML
  - [x] Config dir is non-empty, config path ends with .toml

---

## 13.3 Config File Watcher

Watch the config file for changes and send reload events through the event loop.

**File:** `oriterm/src/config/monitor.rs`

**Reference:** `_old/src/config/monitor.rs`

- [x] `ConfigMonitor` struct
  - [x] Fields:
    - `shutdown_tx: mpsc::Sender<()>` — signal to stop watcher thread
    - `thread: Option<JoinHandle<()>>` — watcher thread handle
- [x] `ConfigMonitor::new(proxy: EventLoopProxy<TermEvent>) -> Option<Self>`
  - [x] Get config file path and parent directory
  - [x] If parent doesn't exist: return None (no config dir yet)
  - [x] Create `notify::recommended_watcher` watching parent directory (NonRecursive)
  - [x] Spawn watcher thread with name "config-watcher"
- [x] `ConfigMonitor::watch_loop(...)` — private, runs on watcher thread
  - [x] Loop on `notify_rx.recv()`:
    - [x] Check shutdown signal before processing
    - [x] Filter: only process events for the config file path (ignore other files in dir)
    - [x] Debounce: drain events within 200ms window (editors save in multiple steps)
    - [x] Check shutdown again after debounce
    - [x] Send `TermEvent::ConfigReload` through event loop proxy
    - [x] If proxy send fails: event loop closed, exit
- [x] `ConfigMonitor::shutdown(mut self)`
  - [x] Send shutdown signal
  - [x] Join watcher thread
- [x] Watcher keeps `_watcher` alive for thread lifetime (dropped on exit)

---

## 13.4 Config Hot Reload

Apply config changes to the running application without restart.

**File:** `oriterm/src/app/config_reload.rs`

**Reference:** `_old/src/app/config_reload.rs`

- [x] On `TermEvent::ConfigReload`:
  - [x] Call `Config::try_load()` — on error: log warning, keep previous config
  - [x] Compare new config against current config
  - [x] Apply deltas:
    - [x] Font change (family, size, weight, features, fallback): rebuild FontCollection, clear glyph atlas, recompute cell metrics, resize all tabs/grids
    - [x] Color change (scheme, overrides): rebuild palette, request redraw
    - [x] Window change (opacity, blur): update window transparency/blur settings
    - [x] Behavior change: update behavior flags
    - [x] Bell change: update bell config
    - [x] Keybinding change: rebuild merged keybinding table
  - [x] Broadcast changes to ALL tabs in ALL windows (font metrics affect every grid)
  - [x] Request redraw for all windows

---

## 13.5 Keybinding System <!-- unblocks:8.3 -->

Map key + modifiers to application actions. Linear scan with O(1) expected-case lookup.

**File:** `oriterm/src/keybindings/mod.rs`

**Reference:** `_old/src/keybindings/mod.rs`

- [x] `BindingKey` enum — key identifier independent of modifiers
  - [x] `Named(NamedKey)` — named keys (Tab, PageUp, F1, etc.)
  - [x] `Character(String)` — always stored lowercase
  - [x] Derive: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
- [x] `Action` enum — what to do when a keybinding matches
  - [x] Variants: `Copy`, `Paste`, `SmartCopy`, `SmartPaste`, `NewTab`, `CloseTab`, `NextTab`, `PrevTab`, `ZoomIn`, `ZoomOut`, `ZoomReset`, `ScrollPageUp`, `ScrollPageDown`, `ScrollToTop`, `ScrollToBottom`, `OpenSearch`, `ReloadConfig`, `PreviousPrompt`, `NextPrompt`, `DuplicateTab`, `MoveTabToNewWindow`, `ToggleFullscreen`, `SendText(String)`, `None`
  - [x] `SmartCopy`: copy if selection exists, else fall through to PTY (Ctrl+C sends SIGINT)
  - [x] `SmartPaste`: paste from clipboard (Ctrl+V without Shift)
  - [x] `SendText(String)`: send literal bytes to PTY (supports escape sequences)
  - [x] `None`: explicitly unbinds a default binding
  - [x] Derive: `Debug`, `Clone`, `PartialEq`, `Eq`
- [x] `KeyBinding` struct — `{ key: BindingKey, mods: Modifiers, action: Action }`
  - [x] Derive: `Debug`, `Clone`
- [x] `KeybindConfig` struct — TOML-serializable entry
  - [x] `{ key: String, mods: String, action: String }`
  - [x] Derive: `Debug`, `Clone`, `Serialize`, `Deserialize`
- [x] `key_to_binding_key(key: &Key) -> Option<BindingKey>`
  - [x] Convert winit `Key` to `BindingKey`, normalizing characters to lowercase
- [x] `find_binding(bindings: &[KeyBinding], key: &BindingKey, mods: Modifiers) -> Option<&Action>`
  - [x] Linear scan: first match wins
  - [x] More-specific modifier combos come first in the list (Ctrl+Shift+C before Ctrl+C)

---

## 13.6 Default Keybindings

Built-in default keybindings. User bindings override these.

**File:** `oriterm/src/keybindings/defaults.rs`

**Reference:** `_old/src/keybindings/defaults.rs`

- [x] `default_bindings() -> Vec<KeyBinding>`
  - [x] Ordering: more-specific modifier combos first (Ctrl+Shift before Ctrl)
- [x] Default table:
  - [x] `Ctrl+Shift+C` -> Copy
  - [x] `Ctrl+Shift+V` -> Paste
  - [x] `Ctrl+Insert` -> Copy
  - [x] `Shift+Insert` -> Paste
  - [x] `Ctrl+Shift+R` -> ReloadConfig
  - [x] `Ctrl+Shift+F` -> OpenSearch
  - [x] `Ctrl+=` / `Ctrl++` -> ZoomIn
  - [x] `Ctrl+-` -> ZoomOut
  - [x] `Ctrl+0` -> ZoomReset
  - [x] `Ctrl+T` -> NewTab
  - [x] `Ctrl+W` -> CloseTab
  - [x] `Ctrl+Tab` -> NextTab
  - [x] `Ctrl+Shift+Tab` -> PrevTab
  - [x] `Shift+PageUp` -> ScrollPageUp
  - [x] `Shift+PageDown` -> ScrollPageDown
  - [x] `Shift+Home` -> ScrollToTop
  - [x] `Shift+End` -> ScrollToBottom
  - [x] `Ctrl+Shift+ArrowUp` -> PreviousPrompt
  - [x] `Ctrl+Shift+ArrowDown` -> NextPrompt
  - [x] `Alt+Enter` -> ToggleFullscreen (Windows/Linux), `Ctrl+Cmd+F` -> ToggleFullscreen (macOS)
  - [x] `Ctrl+C` -> SmartCopy (must come AFTER Ctrl+Shift+C)
  - [x] `Ctrl+V` -> SmartPaste (must come AFTER Ctrl+Shift+V)

---

## 13.7 Keybinding Config Parsing

Parse keybinding entries from TOML and merge with defaults.

**File:** `oriterm/src/keybindings/parse.rs`

**Reference:** `_old/src/keybindings/parse.rs`

- [x] `merge_bindings(user: &[KeybindConfig]) -> Vec<KeyBinding>`
  - [x] Start with `default_bindings()`
  - [x] For each user entry:
    - [x] Parse key and mods (log warning on unknown)
    - [x] Parse action (log warning on unknown)
    - [x] Remove any existing binding with same (key, mods) — retain filter
    - [x] If action is `None`: unbind only (don't add replacement)
    - [x] Otherwise: push new binding
  - [x] Returns merged binding list
- [x] `parse_key(s: &str) -> Option<BindingKey>`
  - [x] Named keys: Tab, PageUp, PageDown, Home, End, Insert, Delete, Escape, Enter, Backspace, Space, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, F1-F24
  - [x] Single characters: lowercased
- [x] `parse_mods(s: &str) -> Modifiers`
  - [x] Pipe-separated: "Ctrl|Shift", "Alt", "Super"
  - [x] Empty string or "None": no modifiers
- [x] `parse_action(s: &str) -> Option<Action>`
  - [x] Direct match for each Action variant name
  - [x] Special: `"SendText:..."` prefix → `Action::SendText(unescape_send_text(text))`
- [x] `unescape_send_text(s: &str) -> String` — process escape sequences
  - [x] `\x1b` -> ESC, `\n` -> newline, `\r` -> CR, `\t` -> tab, `\\` -> backslash
  - [x] `\xHH` -> hex byte
- [x] **Tests** (`oriterm/src/keybindings/tests.rs`):
  - [x] Default bindings: Ctrl+Shift+C maps to Copy
  - [x] Merge: user binding overrides default
  - [x] Merge: Action::None removes default binding
  - [x] Parse key: named keys, single chars, unknown returns None
  - [x] Parse mods: "Ctrl|Shift" -> CONTROL | SHIFT
  - [x] Parse action: all variants, SendText with escapes
  - [x] Unescape: `\x1b` -> '\x1b', `\n` -> '\n', `\\` -> '\\'
  - [x] SmartCopy/SmartPaste resolved correctly after Ctrl+Shift variants

---

## 13.8 CLI Subcommands

Utility subcommands for font discovery, keybinding reference, config validation, and theme browsing — diagnostic tools every terminal ships.

**File:** `oriterm/src/cli.rs` (clap subcommands)

**Reference:** Alacritty `alacritty msg`, Ghostty `ghostty +list-fonts`, WezTerm `wezterm ls-fonts`

- [x] `oriterm ls-fonts` — list discovered fonts with fallback chain:
  - [x] Show primary font family + all 4 style variants (Regular/Bold/Italic/BoldItalic)
  - [x] Show fallback chain in priority order
  - [x] For each face: family name, style, file path, format (TrueType/OpenType), variable axes
  - [x] `--codepoint <char>` — show which font resolves a specific character
  - [x] Output: plain text, one font per line
- [x] `oriterm show-keys` — dump current keybindings:
  - [x] Load config, merge defaults with user overrides
  - [x] Show all active bindings: `Ctrl+Shift+C -> Copy`, etc.
  - [x] `--default` — show only default bindings (ignore user config)
  - [x] Group by category (clipboard, tabs, navigation, etc.)
- [x] `oriterm list-themes` — browse available color schemes:
  - [x] List all built-in themes by name
  - [x] List user-defined themes from config directory
  - [x] `--preview` — show ANSI color preview for each theme (16-color palette sample)
- [x] `oriterm validate-config` — check config without launching:
  - [x] Parse config file, report errors with line numbers
  - [x] Validate font families exist on system
  - [x] Validate color values parse correctly
  - [x] Validate keybinding key names and action names
  - [x] Exit 0 on valid, exit 1 on errors
- [x] `oriterm show-config` — dump resolved config:
  - [x] Load config with all defaults filled in
  - [x] Serialize to TOML and print
  - [x] Shows effective config (defaults + user overrides merged)
- [x] Subcommand dispatch: all subcommands run without opening a window (headless)
- [x] **Tests:**
  - [x] `validate-config` on valid config returns exit 0
  - [x] `validate-config` on invalid TOML returns exit 1 with error message
  - [x] `show-config` output is valid TOML that can be re-parsed
  - [x] `ls-fonts` includes primary font family

---

## 13.9 Shell Completion Scripts

Generate shell completion scripts for bash, zsh, fish, and PowerShell.

**File:** `oriterm/src/cli.rs` (clap `generate` integration)

**Reference:** WezTerm `wezterm shell-completion`, clap `clap_complete` crate

- [x] Add `clap_complete` dependency
- [x] `oriterm completions <shell>` subcommand:
  - [x] `oriterm completions bash` — output bash completion script
  - [x] `oriterm completions zsh` — output zsh completion script
  - [x] `oriterm completions fish` — output fish completion script
  - [x] `oriterm completions powershell` — output PowerShell completion script
  - [x] Output to stdout (user redirects to appropriate file)
- [x] Completions cover: all subcommands, `--config`, `--working-directory`, `--shell`, etc.
- [x] Install instructions printed when run without redirection
- [x] **Tests:**
  - [x] Each shell variant produces non-empty output
  - [x] Output contains expected subcommand names

---

## 13.10 Section Completion

- [x] All 13.1-13.9 items complete
- [x] `cargo test -p oriterm` — config and keybinding tests pass
- [x] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [x] Config loads from TOML file on startup (defaults if missing)
- [x] Partial TOML fills in defaults for unspecified fields
- [x] Invalid TOML logs warning, uses defaults (no crash)
- [x] Config file watcher detects changes with 200ms debounce
- [x] Hot reload applies font, color, window, behavior, bell, keybinding changes
- [x] Font change triggers atlas rebuild + grid resize
- [x] Default keybindings work out of the box
- [x] User keybindings override defaults
- [x] `Action::None` unbinds a default binding
- [x] `SendText` action sends literal bytes (with escape sequences) to PTY
- [x] Window state (geometry) persisted separately from user config

**Exit Criteria:** Config system loads, saves, and hot-reloads without interrupting the terminal session. Keybindings are user-configurable via TOML with sensible defaults. Invalid config never crashes the app.
