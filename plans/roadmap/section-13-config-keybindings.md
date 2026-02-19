---
section: 13
title: Configuration & Keybindings
status: not-started
tier: 3
goal: TOML configuration with file watching and hot reload, user-configurable keybindings with defaults
sections:
  - id: "13.1"
    title: Config Structs
    status: not-started
  - id: "13.2"
    title: Config I/O
    status: not-started
  - id: "13.3"
    title: Config File Watcher
    status: not-started
  - id: "13.4"
    title: Config Hot Reload
    status: not-started
  - id: "13.5"
    title: Keybinding System
    status: not-started
  - id: "13.6"
    title: Default Keybindings
    status: not-started
  - id: "13.7"
    title: Keybinding Config Parsing
    status: not-started
  - id: "13.8"
    title: CLI Subcommands
    status: not-started
  - id: "13.9"
    title: Shell Completion Scripts
    status: not-started
  - id: "13.10"
    title: Section Completion
    status: not-started
---

# Section 13: Configuration & Keybindings

**Status:** Not Started
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

- [ ] `Config` struct (top-level)
  - [ ] `#[derive(Debug, Clone, Default, Serialize, Deserialize)]`
  - [ ] `#[serde(default)]`
  - [ ] Fields:
    - `font: FontConfig`
    - `terminal: TerminalConfig`
    - `colors: ColorConfig`
    - `window: WindowConfig`
    - `behavior: BehaviorConfig`
    - `bell: BellConfig`
    - `keybind: Vec<KeybindConfig>` — user keybinding overrides
- [ ] `FontConfig` struct <!-- unblocks:6.15 -->
  - [ ] Fields:
    - `size: f32` — point size (default: from render::FONT_SIZE)
    - `family: Option<String>` — primary font family name
    - `weight: u16` — CSS font weight 100-900 (default: 400)
    - `tab_bar_font_weight: Option<u16>` — tab bar text weight (default: 600 via effective method)
    - `tab_bar_font_family: Option<String>` — tab bar font family (default: same as primary)
    - `features: Vec<String>` — OpenType features (default: `["calt", "liga"]`)
    - `fallback: Vec<FallbackFontConfig>` — ordered fallback font list
  - [ ] `effective_weight(&self) -> u16` — clamped to [100, 900]
  - [ ] `effective_bold_weight(&self) -> u16` — `min(900, weight + 300)` (CSS "bolder")
  - [ ] `effective_tab_bar_weight(&self) -> u16` — clamped, defaults to 600
- [ ] `FallbackFontConfig` struct
  - [ ] Fields:
    - `family: String` — font family name or absolute path
    - `features: Option<Vec<String>>` — per-fallback OpenType feature overrides
    - `size_offset: Option<f32>` — point size adjustment relative to primary
- [ ] `TerminalConfig` struct
  - [ ] Fields:
    - `shell: Option<String>` — override shell (default: system shell)
    - `scrollback: usize` — scrollback lines (default: 10_000)
    - `cursor_style: String` — "block", "bar"/"beam", "underline" (default: "block")
    - `cursor_blink: bool` — enable cursor blinking (default: true)
    - `cursor_blink_interval_ms: u64` — blink interval (default: 530)
- [ ] `ColorConfig` struct <!-- unblocks:3.7 -->
  - [ ] Fields:
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
  - [ ] `effective_minimum_contrast(&self) -> f32` — clamped to [1.0, 21.0]
- [ ] `AlphaBlending` enum — `Linear`, `LinearCorrected` (default)
- [ ] `WindowConfig` struct <!-- unblocks:3.6 -->
  - [ ] Fields:
    - `columns: usize` — initial terminal columns (default: 120)
    - `rows: usize` — initial terminal rows (default: 30)
    - `opacity: f32` — window opacity 0.0-1.0 (default: 1.0)
    - `tab_bar_opacity: Option<f32>` — independent tab bar opacity (falls back to opacity)
    - `blur: bool` — enable backdrop blur (default: true)
    - `decorations: Decorations` — window decoration mode (default: `None` for frameless CSD)
    - `resize_increments: bool` — snap resize to cell boundaries (default: false)
  - [ ] `effective_opacity(&self) -> f32` — clamped to [0.0, 1.0]
  - [ ] `effective_tab_bar_opacity(&self) -> f32` — clamped, falls back to opacity when None
- [ ] `Decorations` enum
  - [ ] `Full` — OS-native title bar and borders
  - [ ] `None` — frameless window with custom CSD (default)
  - [ ] On Windows/Linux: maps to `with_decorations(bool)` in winit
  - [ ] macOS extends with `Transparent` (transparent titlebar) and `Buttonless` (hide traffic lights) via winit macOS extensions
  - [ ] **Ref:** Alacritty `config/window.rs:183-189`, winit `WindowAttributes::with_decorations`
- [ ] `BehaviorConfig` struct
  - [ ] Fields:
    - `copy_on_select: bool` — auto-copy on selection release (default: true)
    - `bold_is_bright: bool` — bold text uses bright colors (default: true)
    - `shell_integration: bool` — enable shell integration injection (default: true)
- [ ] `BellConfig` struct
  - [ ] Fields:
    - `animation: String` — "ease_out", "linear", "none" (default: "ease_out")
    - `duration_ms: u16` — flash duration, 0 = disabled (default: 150)
    - `color: Option<String>` — flash color "#RRGGBB" (default: white)
  - [ ] `is_enabled(&self) -> bool` — `duration_ms > 0 && animation != "none"`

---

## 13.2 Config I/O

Path resolution, loading, saving, and cursor style parsing.

**File:** `oriterm/src/config/io.rs`

**Reference:** `_old/src/config/io.rs`

- [ ] `config_dir() -> PathBuf`
  - [ ] Windows: `%APPDATA%/ori_term`
  - [ ] Linux: `$XDG_CONFIG_HOME/ori_term` or `~/.config/ori_term`
  - [ ] Fallback: `./ori_term`
- [ ] `config_path() -> PathBuf` — `config_dir().join("config.toml")`
- [ ] `state_path() -> PathBuf` — `config_dir().join("state.toml")` (window geometry persistence)
- [ ] `WindowState` struct — `{ x: i32, y: i32, width: u32, height: u32 }`
  - [ ] `WindowState::load() -> Option<Self>` — read from state.toml, None on missing/invalid
  - [ ] `WindowState::save(&self)` — write to state.toml, create dir if needed
- [ ] `Config::load() -> Self`
  - [ ] Read from `config_path()`
  - [ ] `NotFound`: return defaults (first run)
  - [ ] Parse error: log warning, return defaults
  - [ ] Success: log path, return parsed config
- [ ] `Config::try_load() -> Result<Self, String>`
  - [ ] Preserves error distinction (file missing vs parse error)
  - [ ] Used by hot reload: parse error keeps previous config
- [ ] `Config::save(&self)` — serialize to TOML, write to config_path
- [ ] `parse_cursor_style(s: &str) -> CursorShape`
  - [ ] "block" | "Block" -> Block
  - [ ] "bar" | "beam" -> Beam
  - [ ] "underline" -> Underline
  - [ ] Unknown -> Block (default)
- [ ] `save_toml(value, path, label)` — private helper: serialize, create dirs, write
- [ ] **Tests** (`oriterm/src/config/tests.rs`):
  - [ ] Default config roundtrip: serialize then deserialize equals defaults
  - [ ] Partial TOML uses defaults for missing fields
  - [ ] Empty TOML gives full defaults
  - [ ] Cursor style parsing: all variants
  - [ ] Opacity clamping: values outside [0.0, 1.0] clamped
  - [ ] Minimum contrast clamping: values outside [1.0, 21.0] clamped
  - [ ] Color overrides roundtrip: foreground, background, cursor, selection
  - [ ] ANSI color overrides: per-index overrides, unset indices remain None
  - [ ] Font weight: defaults, clamping, bold derivation
  - [ ] Tab bar opacity: independent from window opacity, falls back when None
  - [ ] Alpha blending: defaults to LinearCorrected, parses from TOML
  - [ ] Config dir is non-empty, config path ends with .toml

---

## 13.3 Config File Watcher

Watch the config file for changes and send reload events through the event loop.

**File:** `oriterm/src/config/monitor.rs`

**Reference:** `_old/src/config/monitor.rs`

- [ ] `ConfigMonitor` struct
  - [ ] Fields:
    - `shutdown_tx: mpsc::Sender<()>` — signal to stop watcher thread
    - `thread: Option<JoinHandle<()>>` — watcher thread handle
- [ ] `ConfigMonitor::new(proxy: EventLoopProxy<TermEvent>) -> Option<Self>`
  - [ ] Get config file path and parent directory
  - [ ] If parent doesn't exist: return None (no config dir yet)
  - [ ] Create `notify::recommended_watcher` watching parent directory (NonRecursive)
  - [ ] Spawn watcher thread with name "config-watcher"
- [ ] `ConfigMonitor::watch_loop(...)` — private, runs on watcher thread
  - [ ] Loop on `notify_rx.recv()`:
    - [ ] Check shutdown signal before processing
    - [ ] Filter: only process events for the config file path (ignore other files in dir)
    - [ ] Debounce: drain events within 200ms window (editors save in multiple steps)
    - [ ] Check shutdown again after debounce
    - [ ] Send `TermEvent::ConfigReload` through event loop proxy
    - [ ] If proxy send fails: event loop closed, exit
- [ ] `ConfigMonitor::shutdown(mut self)`
  - [ ] Send shutdown signal
  - [ ] Join watcher thread
- [ ] Watcher keeps `_watcher` alive for thread lifetime (dropped on exit)

---

## 13.4 Config Hot Reload

Apply config changes to the running application without restart.

**File:** `oriterm/src/app/config_reload.rs`

**Reference:** `_old/src/app/config_reload.rs`

- [ ] On `TermEvent::ConfigReload`:
  - [ ] Call `Config::try_load()` — on error: log warning, keep previous config
  - [ ] Compare new config against current config
  - [ ] Apply deltas:
    - [ ] Font change (family, size, weight, features, fallback): rebuild FontCollection, clear glyph atlas, recompute cell metrics, resize all tabs/grids
    - [ ] Color change (scheme, overrides): rebuild palette, request redraw
    - [ ] Window change (opacity, blur): update window transparency/blur settings
    - [ ] Behavior change: update behavior flags
    - [ ] Bell change: update bell config
    - [ ] Keybinding change: rebuild merged keybinding table
  - [ ] Broadcast changes to ALL tabs in ALL windows (font metrics affect every grid)
  - [ ] Request redraw for all windows

---

## 13.5 Keybinding System

Map key + modifiers to application actions. Linear scan with O(1) expected-case lookup.

**File:** `oriterm/src/keybindings/mod.rs`

**Reference:** `_old/src/keybindings/mod.rs`

- [ ] `BindingKey` enum — key identifier independent of modifiers
  - [ ] `Named(NamedKey)` — named keys (Tab, PageUp, F1, etc.)
  - [ ] `Character(String)` — always stored lowercase
  - [ ] Derive: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
- [ ] `Action` enum — what to do when a keybinding matches
  - [ ] Variants: `Copy`, `Paste`, `SmartCopy`, `SmartPaste`, `NewTab`, `CloseTab`, `NextTab`, `PrevTab`, `ZoomIn`, `ZoomOut`, `ZoomReset`, `ScrollPageUp`, `ScrollPageDown`, `ScrollToTop`, `ScrollToBottom`, `OpenSearch`, `ReloadConfig`, `PreviousPrompt`, `NextPrompt`, `DuplicateTab`, `MoveTabToNewWindow`, `ToggleFullscreen`, `SendText(String)`, `None`
  - [ ] `SmartCopy`: copy if selection exists, else fall through to PTY (Ctrl+C sends SIGINT)
  - [ ] `SmartPaste`: paste from clipboard (Ctrl+V without Shift)
  - [ ] `SendText(String)`: send literal bytes to PTY (supports escape sequences)
  - [ ] `None`: explicitly unbinds a default binding
  - [ ] Derive: `Debug`, `Clone`, `PartialEq`, `Eq`
- [ ] `KeyBinding` struct — `{ key: BindingKey, mods: Modifiers, action: Action }`
  - [ ] Derive: `Debug`, `Clone`
- [ ] `KeybindConfig` struct — TOML-serializable entry
  - [ ] `{ key: String, mods: String, action: String }`
  - [ ] Derive: `Debug`, `Clone`, `Serialize`, `Deserialize`
- [ ] `key_to_binding_key(key: &Key) -> Option<BindingKey>`
  - [ ] Convert winit `Key` to `BindingKey`, normalizing characters to lowercase
- [ ] `find_binding(bindings: &[KeyBinding], key: &BindingKey, mods: Modifiers) -> Option<&Action>`
  - [ ] Linear scan: first match wins
  - [ ] More-specific modifier combos come first in the list (Ctrl+Shift+C before Ctrl+C)

---

## 13.6 Default Keybindings

Built-in default keybindings. User bindings override these.

**File:** `oriterm/src/keybindings/defaults.rs`

**Reference:** `_old/src/keybindings/defaults.rs`

- [ ] `default_bindings() -> Vec<KeyBinding>`
  - [ ] Ordering: more-specific modifier combos first (Ctrl+Shift before Ctrl)
- [ ] Default table:
  - [ ] `Ctrl+Shift+C` -> Copy
  - [ ] `Ctrl+Shift+V` -> Paste
  - [ ] `Ctrl+Insert` -> Copy
  - [ ] `Shift+Insert` -> Paste
  - [ ] `Ctrl+Shift+R` -> ReloadConfig
  - [ ] `Ctrl+Shift+F` -> OpenSearch
  - [ ] `Ctrl+=` / `Ctrl++` -> ZoomIn
  - [ ] `Ctrl+-` -> ZoomOut
  - [ ] `Ctrl+0` -> ZoomReset
  - [ ] `Ctrl+T` -> NewTab
  - [ ] `Ctrl+W` -> CloseTab
  - [ ] `Ctrl+Tab` -> NextTab
  - [ ] `Ctrl+Shift+Tab` -> PrevTab
  - [ ] `Shift+PageUp` -> ScrollPageUp
  - [ ] `Shift+PageDown` -> ScrollPageDown
  - [ ] `Shift+Home` -> ScrollToTop
  - [ ] `Shift+End` -> ScrollToBottom
  - [ ] `Ctrl+Shift+ArrowUp` -> PreviousPrompt
  - [ ] `Ctrl+Shift+ArrowDown` -> NextPrompt
  - [ ] `Alt+Enter` -> ToggleFullscreen (Windows/Linux), `Ctrl+Cmd+F` -> ToggleFullscreen (macOS)
  - [ ] `Ctrl+C` -> SmartCopy (must come AFTER Ctrl+Shift+C)
  - [ ] `Ctrl+V` -> SmartPaste (must come AFTER Ctrl+Shift+V)

---

## 13.7 Keybinding Config Parsing

Parse keybinding entries from TOML and merge with defaults.

**File:** `oriterm/src/keybindings/parse.rs`

**Reference:** `_old/src/keybindings/parse.rs`

- [ ] `merge_bindings(user: &[KeybindConfig]) -> Vec<KeyBinding>`
  - [ ] Start with `default_bindings()`
  - [ ] For each user entry:
    - [ ] Parse key and mods (log warning on unknown)
    - [ ] Parse action (log warning on unknown)
    - [ ] Remove any existing binding with same (key, mods) — retain filter
    - [ ] If action is `None`: unbind only (don't add replacement)
    - [ ] Otherwise: push new binding
  - [ ] Returns merged binding list
- [ ] `parse_key(s: &str) -> Option<BindingKey>`
  - [ ] Named keys: Tab, PageUp, PageDown, Home, End, Insert, Delete, Escape, Enter, Backspace, Space, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, F1-F24
  - [ ] Single characters: lowercased
- [ ] `parse_mods(s: &str) -> Modifiers`
  - [ ] Pipe-separated: "Ctrl|Shift", "Alt", "Super"
  - [ ] Empty string or "None": no modifiers
- [ ] `parse_action(s: &str) -> Option<Action>`
  - [ ] Direct match for each Action variant name
  - [ ] Special: `"SendText:..."` prefix → `Action::SendText(unescape_send_text(text))`
- [ ] `unescape_send_text(s: &str) -> String` — process escape sequences
  - [ ] `\x1b` -> ESC, `\n` -> newline, `\r` -> CR, `\t` -> tab, `\\` -> backslash
  - [ ] `\xHH` -> hex byte
- [ ] **Tests** (`oriterm/src/keybindings/tests.rs`):
  - [ ] Default bindings: Ctrl+Shift+C maps to Copy
  - [ ] Merge: user binding overrides default
  - [ ] Merge: Action::None removes default binding
  - [ ] Parse key: named keys, single chars, unknown returns None
  - [ ] Parse mods: "Ctrl|Shift" -> CONTROL | SHIFT
  - [ ] Parse action: all variants, SendText with escapes
  - [ ] Unescape: `\x1b` -> '\x1b', `\n` -> '\n', `\\` -> '\\'
  - [ ] SmartCopy/SmartPaste resolved correctly after Ctrl+Shift variants

---

## 13.8 CLI Subcommands

Utility subcommands for font discovery, keybinding reference, config validation, and theme browsing — diagnostic tools every terminal ships.

**File:** `oriterm/src/cli.rs` (clap subcommands)

**Reference:** Alacritty `alacritty msg`, Ghostty `ghostty +list-fonts`, WezTerm `wezterm ls-fonts`

- [ ] `oriterm ls-fonts` — list discovered fonts with fallback chain:
  - [ ] Show primary font family + all 4 style variants (Regular/Bold/Italic/BoldItalic)
  - [ ] Show fallback chain in priority order
  - [ ] For each face: family name, style, file path, format (TrueType/OpenType), variable axes
  - [ ] `--codepoint <char>` — show which font resolves a specific character
  - [ ] Output: plain text, one font per line
- [ ] `oriterm show-keys` — dump current keybindings:
  - [ ] Load config, merge defaults with user overrides
  - [ ] Show all active bindings: `Ctrl+Shift+C -> Copy`, etc.
  - [ ] `--default` — show only default bindings (ignore user config)
  - [ ] Group by category (clipboard, tabs, navigation, etc.)
- [ ] `oriterm list-themes` — browse available color schemes:
  - [ ] List all built-in themes by name
  - [ ] List user-defined themes from config directory
  - [ ] `--preview` — show ANSI color preview for each theme (16-color palette sample)
- [ ] `oriterm validate-config` — check config without launching:
  - [ ] Parse config file, report errors with line numbers
  - [ ] Validate font families exist on system
  - [ ] Validate color values parse correctly
  - [ ] Validate keybinding key names and action names
  - [ ] Exit 0 on valid, exit 1 on errors
- [ ] `oriterm show-config` — dump resolved config:
  - [ ] Load config with all defaults filled in
  - [ ] Serialize to TOML and print
  - [ ] Shows effective config (defaults + user overrides merged)
- [ ] Subcommand dispatch: all subcommands run without opening a window (headless)
- [ ] **Tests:**
  - [ ] `validate-config` on valid config returns exit 0
  - [ ] `validate-config` on invalid TOML returns exit 1 with error message
  - [ ] `show-config` output is valid TOML that can be re-parsed
  - [ ] `ls-fonts` includes primary font family

---

## 13.9 Shell Completion Scripts

Generate shell completion scripts for bash, zsh, fish, and PowerShell.

**File:** `oriterm/src/cli.rs` (clap `generate` integration)

**Reference:** WezTerm `wezterm shell-completion`, clap `clap_complete` crate

- [ ] Add `clap_complete` dependency
- [ ] `oriterm completions <shell>` subcommand:
  - [ ] `oriterm completions bash` — output bash completion script
  - [ ] `oriterm completions zsh` — output zsh completion script
  - [ ] `oriterm completions fish` — output fish completion script
  - [ ] `oriterm completions powershell` — output PowerShell completion script
  - [ ] Output to stdout (user redirects to appropriate file)
- [ ] Completions cover: all subcommands, `--config`, `--working-directory`, `--shell`, etc.
- [ ] Install instructions printed when run without redirection
- [ ] **Tests:**
  - [ ] Each shell variant produces non-empty output
  - [ ] Output contains expected subcommand names

---

## 13.10 Section Completion

- [ ] All 13.1-13.9 items complete
- [ ] `cargo test -p oriterm` — config and keybinding tests pass
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] Config loads from TOML file on startup (defaults if missing)
- [ ] Partial TOML fills in defaults for unspecified fields
- [ ] Invalid TOML logs warning, uses defaults (no crash)
- [ ] Config file watcher detects changes with 200ms debounce
- [ ] Hot reload applies font, color, window, behavior, bell, keybinding changes
- [ ] Font change triggers atlas rebuild + grid resize
- [ ] Default keybindings work out of the box
- [ ] User keybindings override defaults
- [ ] `Action::None` unbinds a default binding
- [ ] `SendText` action sends literal bytes (with escape sequences) to PTY
- [ ] Window state (geometry) persisted separately from user config

**Exit Criteria:** Config system loads, saves, and hot-reloads without interrupting the terminal session. Keybindings are user-configurable via TOML with sensible defaults. Invalid config never crashes the app.
