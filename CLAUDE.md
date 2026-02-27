# ori_term

GPU-accelerated terminal emulator in Rust (same category as Alacritty, WezTerm, Ghostty). Opens a native frameless window, renders a terminal grid via wgpu, runs shell processes through ConPTY/PTY. Cross-compiled from WSL targeting `x86_64-pc-windows-gnu`.

**Broken Window Policy**: Fix EVERY issue you encounter — no exceptions. Never say "this is pre-existing", "this is unrelated", or "outside the scope". If you see it, you own it. Leaving broken code because "it was already broken" is explicitly forbidden.

**Do it properly, not just simply. Correct architecture over quick hacks; no shortcuts or "good enough" solutions.**

**NO WORKAROUNDS. NO HACKS. NO SHORTCUTS.**
- **Proper fixes only** — If a fix feels hacky, it IS hacky. Find the right solution.
- **When unsure, STOP and ASK** — Do not guess. Do not assume. Pause and ask the user for guidance.
- **Fact-check everything** — Verify behavior against reference implementations. Test your assumptions. Read the code you're modifying.
- **Consult reference repos** — Check `~/projects/reference_repos/console_repos/` for established patterns and idioms.
- **No "temporary" fixes** — There is no such thing. Today's temporary fix is tomorrow's permanent tech debt.
- **If you can't do it right, say so** — Communicate blockers rather than shipping bad code.

---

## Coding Standards

**Extracted from**: Alacritty, WezTerm, Ghostty, Ptyxis, Ratatui, Crossterm, Bubbletea, Lipgloss, Termenv — the patterns every serious terminal project agrees on.

**Error Handling**: No `unwrap()` in library code — return `Result` or provide a default. No `panic!` on user-recoverable errors. Use `std::io::Result<T>` for I/O operations. Custom `Error` enum with `From` impls for domain-specific errors. Error chains via `.context()` or `source()`.

**Unsafe**: `unsafe_code = "deny"` in Cargo.toml. Zero unsafe in library code (Ratatui forbids it entirely). Only justified platform FFI in clearly marked modules.

**Linting**: Clippy warnings are errors (`all = deny`). Pedantic + nursery enabled as warnings. No `#[allow(clippy)]` without written justification. `enum_glob_use = deny`, `if_not_else = deny`.

**Formatting**: `imports_granularity = "Module"`. Group imports: std, external, crate. Comments wrapped at 100 chars. Format code in doc comments.

**Module Organization**: Separate terminal logic from GUI (Alacritty pattern: pure terminal lib vs. rendering binary). One primary type per module file. Re-export key types at parent `mod.rs`. Two-file pattern: `style.rs` + `style/` directory for sub-modules. Platform-specific code behind `#[cfg()]` in dedicated files. **Source files (excluding `tests.rs`) must not exceed 500 lines** — when writing new code, proactively split into submodules before hitting the limit rather than writing a large file and splitting later.

**Public API**: Keep surface small — expose primitives, not internals. `#[must_use]` on builder methods. `impl Into<T>` and `impl AsRef<str>` for ergonomic APIs. Document every public item with `///`. First line: summary. Second: blank. Then details.

**Functions**: < 50 lines (target < 30). No 20+ arm match blocks — extract helpers at 3+ similar arms. No boolean flag parameters (split function or use enum). > 3 params → config/options struct.

**Memory**: Newtypes for IDs (`TabId(u64)`, not bare `u64`). `Arc` only when shared ownership is required. No `Arc` cloning in hot paths. Intern/cache repeated strings. `#[cold]` on error-path factory functions.

**Performance**: O(n^2) → O(n) or O(n log n). Hash lookups not linear scans. No allocation in hot loops. Iterators over indexing. Buffer output, flush atomically — never write char-by-char. Damage tracking to minimize GPU work.

**Testing**: Buffer/TestBackend approach for rendering tests (from Ratatui). Test Unicode width with CJK, emoji, combining marks, ZWJ sequences. Test every env var combination for color detection. Platform matrix in CI. Visual regression tests where applicable. Verify behavior not implementation.

**Style**: No dead/commented code, no banners. `//!`/`///` doc comments. Full sentences with periods in comments. No `println!` debugging — use `log` macros.

---

## Terminal Emulator Rules

Non-negotiable. Every one comes from a real bug observed across the reference repos.

**Color Detection Priority** (every project agrees on this order):
```
NO_COLOR set (any value)          → disabled (highest priority)
CLICOLOR_FORCE != "0"             → force color even if not TTY
CLICOLOR == "0"                   → disabled
COLORTERM=truecolor|24bit         → TrueColor
COLORTERM/TERM contains 256color  → ANSI256
TERM set + not "dumb"             → ANSI (16 color)
TERM=dumb or not a TTY            → None
```
Colors downgrade gracefully: TrueColor → nearest ANSI256 → nearest ANSI → stripped.

**Width = Unicode, not `len()`**: Never use `str.len()` or `chars().count()` for display width. Use `unicode-width` crate. CJK = width 2. Combining marks = width 0. Strip ANSI before measuring. Wrap and truncate by display width, not bytes. Ellipsis is `…` (U+2026, width 1), not `...`.

**Buffer Output**: Never write char-by-char. Buffer the full frame, flush once. Synchronized output (Mode 2026). Double-buffer and diff (only write changed cells). This prevents flicker.

**RAII Cleanup**: Raw mode via Drop guards. Panic hook restores terminal state before printing. SIGINT/SIGTERM restore. Alternate screen: enter it → must leave it. No leaked terminal state on any exit path.

**Resize**: SIGWINCH on Unix. Re-query size after signal. Never cache stale terminal size. Fallback: 80x24. All layout relative to current terminal width — never hardcode.

**Piped Output**: `!stdout().is_terminal()` → no colors (unless CLICOLOR_FORCE), no cursor manipulation, no raw mode, plain text only. Check the actual output fd, not stdin.

**Dumb Terminals**: `TERM=dumb` or no TERM → no escape sequences, no cursor movement, no colors. Degrade gracefully, never crash.

---

## Commands

**Primary**: `./fmt-all.sh`, `./clippy-all.sh`, `./build-all.sh`, `./test-all.sh`
**Build**: `cargo build --target x86_64-pc-windows-gnu` (debug), `cargo build --target x86_64-pc-windows-gnu --release` (release)
**After EVERY change, run `./build-all.sh`, `./clippy-all.sh`, and `./test-all.sh`. No exceptions. Do not skip any of these.**

---

## Key Paths

`src/app.rs` — App struct, winit event loop, input dispatch | `src/tab.rs` — Tab (Grid + PTY + VTE) | `src/grid/mod.rs` — Grid (rows, cursor, scrollback, reflow) | `src/term_handler.rs` — VTE Handler impl (~50 methods) | `src/gpu/renderer.rs` — GPU rendering (wgpu, draw_frame) | `src/gpu/atlas.rs` — Glyph atlas (1024x1024 shelf packing) | `src/gpu/pipeline.rs` — WGSL shader pipelines | `src/tab_bar.rs` — Tab bar rendering + hit-testing | `src/drag.rs` — Chrome-style drag state machine | `src/cell.rs` — Rich Cell (24 bytes) + CellFlags | `src/key_encoding.rs` — Kitty + legacy key encoding | `src/config.rs` — TOML config | `src/render.rs` — FontSet (fontdue, 4 style variants + fallback chain) | `src/palette.rs` — 270-entry color palette | `src/selection.rs` — 3-point selection model | `src/search.rs` — Search (plain + regex) | `src/url_detect.rs` — Implicit URL detection

## Reference Repos (`~/projects/reference_repos/console_repos/`)

- **tmux** — C, the canonical terminal multiplexer. Grid/screen/tty separation, `input.c` (83k-line VT parser), `grid.c` (cell storage + extended cells for wide/RGB), `screen-write.c` (damage-tracked screen updates), `window-copy.c` (selection/search/vi-mode). Gold standard for PTY management, reflow, and session persistence
- **alacritty** — 4-crate workspace, OpenGL, `vte` parser, strict clippy (`deny(clippy::all)`), `rustfmt.toml` with module imports
- **wezterm** — 69-crate monorepo, `anyhow`+`thiserror` errors, Lua config, `portable-pty`, multiplexer architecture
- **ghostty** — Zig, Metal+OpenGL+WebGL, SIMD, comptime C ABI, AGENTS.md, Valgrind integration
- **ratatui** — 9-crate workspace, `unsafe_code = "forbid"`, Buffer-based widget tests, TestBackend, pedantic clippy
- **crossterm** — Single crate, Command trait pattern (`queue!`/`execute!` macros), `io::Result<T>` everywhere
- **bubbletea** — Go Elm Architecture (Model/Update/View), frame-based rendering (60/120 FPS), goroutine channels
- **lipgloss** — CSS-like fluent styling, AdaptiveColor/CompleteColor, lazy `sync.Once` renderer
- **ptyxis** — C/GTK4, GNOME's default terminal (Fedora/RHEL/Ubuntu). libvte consumer with GPU-accelerated rendering, `ptyxis-agent` out-of-process PTY helper for Flatpak sandboxing, `.palette` file format for color schemes with light/dark auto-adaptation, profile system (per-profile container/palette/shell), tab monitor for process tracking (`sudo`/SSH indicators), container-first architecture (Podman/Toolbox/Distrobox discovery), encrypted scrollback, terminal inspector for OSC/mouse debugging
- **termenv** — Color profile detection (NO_COLOR/CLICOLOR), `Environ` interface for testing, profile-aware downgrade

## Plans

Implementation plans live in `plans/`. Each plan is a directory with an `index.md`, `00-overview.md`, and numbered section files (`section-01-*.md`, `section-02-*.md`, etc.).

When the user says **"continue plan X"** or **"resume plan X"** or **"pick up plan X"**:
1. Look in `plans/` for a directory matching the name (fuzzy match — "threading" matches `threaded-pty`, "font" matches `font-rendering`, etc.).
2. Read `00-overview.md` for the full context and mandate.
3. Read each `section-*.md` to find the first section with `status: not-started` or `status: in-progress`.
4. Resume work from that section.
5. **After completing each section**, update the plan files: set YAML status to `complete`, check checkboxes, update `index.md`, and record any deviations.

Plans are the source of truth for multi-session work. Keep them in sync with reality.

---

## Current State

See [plans/roadmap/](plans/roadmap/) — the roadmap is the current state. 28 sections, 8 tiers. Use `/continue-roadmap` to resume work. Old prototype in `_old/` for reference.
