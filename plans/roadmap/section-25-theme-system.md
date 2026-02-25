---
section: 25
title: Theme System
status: in-progress
tier: 6
goal: 100+ built-in themes, TOML theme files, discovery, live switching, light/dark auto-switch
sections:
  - id: "25.1"
    title: Theme Format & Loading
    status: in-progress
  - id: "25.2"
    title: Built-in Theme Library
    status: complete
  - id: "25.3"
    title: "Light/Dark Auto-Switch"
    status: in-progress
  - id: "25.4"
    title: Section Completion
    status: not-started
---

# Section 25: Theme System

**Status:** In Progress
**Goal:** Ship 100+ built-in themes selectable by name, with automatic light/dark mode switching based on system preference. Theme richness is a strong first impression signal -- users want to personalize their terminal immediately.

**Crate:** `oriterm` (palette + config layer)
**Dependencies:** `serde` (TOML deserialization), existing `toml` crate, platform crates for dark mode detection

**Inspired by:**
- Ghostty: 300+ built-in themes, single-line config, light/dark auto-switch
- iTerm2: hundreds of importable color schemes
- base16: standardized 16-color scheme format used across editors/terminals
- Kitty: theme kitten with preview and selection

---

## 25.1 Theme Format & Loading

Define a theme format and support loading from files.

**File:** `oriterm/src/scheme/mod.rs` (ColorScheme), `oriterm/src/scheme/loader/mod.rs` (TOML loading), `oriterm/src/scheme/builtin.rs` (built-in schemes)

- [x] TOML theme file format (flat format: `ansi = [16 hex strings]`, `foreground`, `background`, `cursor`, optional `selection_foreground`/`selection_background`)
- [x] `ThemeFile` struct with `Deserialize`
- [x] Parse hex color strings (`#RRGGBB`) to `Rgb`
  - [x] Validate format, return error for malformed strings
- [x] Load themes from:
  - [x] Embedded in binary (53 `const BuiltinScheme` definitions)
  - [x] User theme directory: `config_dir/themes/*.toml`
  - [x] Config: `colors.scheme = "nord"` (by name, case-insensitive)
  - [ ] Config: `colors.scheme = "/path/to/mytheme.toml"` (by absolute path)
- [ ] Theme discovery at startup:
  - [ ] Scan `config_dir/themes/` for `*.toml` files
  - [ ] Parse each, build `Vec<ColorScheme>` of user themes
  - [ ] Merge with built-in schemes (user themes can override built-in names)
- [ ] Theme hot-reload:
  - [ ] ConfigMonitor already watches config dir
  - [ ] Extend to watch `themes/` subdirectory
  - [ ] On theme file change: re-parse and apply if it's the active theme

**Tests:**
- [x] Parse valid TOML theme file to `ColorScheme`
- [x] Reject malformed hex colors with descriptive error
- [x] Case-insensitive name lookup finds built-in themes
- [ ] User theme overrides built-in theme with same name
- [ ] Absolute path loading works for custom theme file
- [x] Missing theme file returns error, does not crash

---

## 25.2 Built-in Theme Library

Port popular color schemes as embedded themes. Target 50+ built-in.

**File:** `oriterm/src/scheme/builtin.rs` (53 scheme constants)

**53 built-in schemes implemented:**
- [x] Catppuccin Mocha, Latte, Frappe, Macchiato
- [x] One Dark, One Light
- [x] Solarized Dark, Solarized Light
- [x] Dracula
- [x] Tokyo Night, Tokyo Night Storm, Tokyo Night Light
- [x] WezTerm Default
- [x] Gruvbox Dark, Gruvbox Light
- [x] Nord
- [x] Rose Pine, Rose Pine Moon, Rose Pine Dawn
- [x] Everforest Dark, Everforest Light
- [x] Kanagawa, Kanagawa Light
- [x] Ayu Dark, Ayu Light, Ayu Mirage
- [x] Material Dark, Material Light
- [x] Monokai
- [x] Nightfox, Dawnfox, Carbonfox
- [x] GitHub Dark, GitHub Light, GitHub Dimmed
- [x] Snazzy, Tomorrow Night, Tomorrow Light
- [x] Zenburn, Iceberg Dark, Iceberg Light
- [x] Night Owl, Palenight, Horizon, Poimandres, Vesper
- [x] Sonokai, OneDark Pro, Moonfly
- [x] PaperColor Dark, PaperColor Light
- [x] Oxocarbon, Andromeda

**Conversion tools:**
- [ ] Script to convert iTerm2 `.itermcolors` XML to TOML format
- [ ] Script to convert Ghostty theme format (key=value) to TOML format
- [ ] Script to convert base16 YAML to TOML format

**Tests:**
- [x] All built-in schemes have valid RGB values (no out-of-range)
- [x] All built-in schemes have unique names
- [x] `BUILTIN_SCHEMES` array contains 50+ defined schemes
- [x] `find_builtin()` returns correct scheme for each name

---

## 25.3 Light/Dark Auto-Switch

Automatically switch theme based on system appearance.

**File:** `oriterm/src/scheme/mod.rs` (parsing), `oriterm/src/app/mod.rs` (detection + switching), `oriterm/src/app/config_reload.rs` (palette building)

- [x] Config syntax: `scheme = "dark:Tokyo Night, light:Tokyo Night Light"`
- [x] Parse `scheme` value:
  - [x] If contains `dark:` / `light:` prefixes: conditional theme
  - [x] Otherwise: static theme
- [x] System dark/light mode detection (existing `platform::theme` module)
- [x] On system theme change:
  - [x] Swap palette to the appropriate scheme via `build_palette_from_config()`
  - [x] Mark all grid lines dirty for redraw
- [ ] Settings dropdown improvements:
  - [ ] Group themes by light/dark/universal
  - [ ] Show "(dark)" / "(light)" label next to theme names

**Tests:**
- [x] Parse `"dark:X, light:Y"` config syntax correctly
- [x] Parse plain `"X"` config syntax as static theme
- [x] Reversed order `"light:Y, dark:X"` parses correctly
- [x] Extra whitespace handled
- [x] Single prefix (e.g. `"dark:X"` without light) returns None

---

## 25.4 Section Completion

- [ ] All 25.1-25.3 items complete
- [x] 50+ themes available by name in config
- [x] Custom themes loadable from TOML files in theme directory
- [x] Light/dark auto-switching works
- [ ] Settings dropdown lists all available themes (built-in + user)
- [ ] Theme hot-reload works (edit theme file, see change)
- [ ] User themes in theme directory discovered automatically
- [ ] Theme conversion scripts for iTerm2/Ghostty/base16 formats

**Exit Criteria:** User can type `colors.scheme = "nord"` in config and get the Nord color scheme. System dark/light mode change auto-switches themes.
