# ori_term Rebuild — Overview

## Mandate

Rebuild ori_term from scratch. The old prototype proved the feature set (GPU window, PTY, VTE, tabs, fonts) but the architecture grew organically and became untenable: god objects, single-mutex contention, coupled VTE handler, rendering holding locks during GPU work, circular imports. The rebuild keeps all features but fixes the foundation with a multi-crate workspace, clean threading, and proper separation of concerns.

## Design Principles

1. **Bottom-up, one layer at a time** — Each layer solid and tested before the next begins.
2. **Crate boundary enforces separation** — `oriterm_core` knows nothing about GUI, fonts, PTY, config, or platform.
3. **Lock discipline** — Snapshot terminal state under lock (microseconds), release, then GPU work without lock.
4. **No god objects** — No struct exceeds ~12 fields. Responsibilities are singular.
5. **Term<T: EventListener>** — Generic terminal state machine, testable with `VoidListener`.
6. **Do it properly** — No workarounds, no hacks, no atomics-as-contention-fix. If it feels wrong, stop and redesign.

## Workspace Structure

```
ori_term/                           Workspace root
├── Cargo.toml                      [workspace] members
├── oriterm_core/                   Pure terminal library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── cell.rs                 Cell, CellFlags, CellExtra
│       ├── index.rs                Point, Line, Column newtypes
│       ├── event.rs                Event enum, EventListener trait
│       ├── sync.rs                 FairMutex
│       ├── grid/                   Grid, Row, Cursor, ring, scroll, editing
│       ├── term/                   Term<T>, VTE Handler, TermMode, CharsetState
│       ├── color/                  Palette, color resolution
│       ├── selection/              Selection model, boundaries, text extraction
│       └── search/                 SearchState, find_matches
├── oriterm_mux/                    Multiplexing layer (layout, domains, IPC)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── id.rs                   PaneId, TabId, WindowId, SessionId, DomainId
│       ├── layout/                 SplitTree, FloatingLayer, compute_layout
│       │   ├── mod.rs
│       │   ├── split_tree.rs       Immutable binary layout tree (Arc COW)
│       │   ├── floating.rs         FloatingPane, FloatingLayer, z-order
│       │   ├── compute.rs          LayoutDescriptor → PaneLayout + DividerLayout
│       │   └── history.rs          Undo/redo stack for split tree mutations
│       ├── nav.rs                  Spatial navigation (directional, cycling)
│       ├── domain.rs               Domain trait, SpawnConfig, DomainState
│       ├── registry.rs             PaneRegistry, SessionRegistry, MuxTab, MuxWindow
│       ├── mux.rs                  InProcessMux (synchronous fast path)
│       ├── protocol.rs             Wire protocol (15-byte header, bincode+zstd)
│       ├── server.rs               MuxServer daemon, OutputCoalescer
│       ├── client.rs               MuxClient, MuxBackend trait
│       └── persistence/            Session save/load, scrollback archive
├── oriterm/                        Binary (GUI, GPU, PTY, platform)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── app/                    App, event loop, input dispatch
│       ├── window.rs               TermWindow (winit + wgpu surface)
│       ├── pane.rs                 Pane (Arc<FairMutex<Term<MuxEventProxy>>>)
│       ├── mux_event.rs            MuxEventProxy, MuxEvent, MuxNotification
│       ├── domain/                 LocalDomain, WslDomain implementations
│       ├── pty/                    PTY event loop, shell spawning
│       ├── gpu/                    GpuState, renderer, atlas, pipelines
│       ├── font/                   FontCollection, shaping, discovery
│       ├── chrome/                 Tab bar, drag, context menu
│       ├── config/                 TOML config, file watcher
│       ├── key_encoding/           Kitty + legacy encoding
│       └── clipboard.rs
├── oriterm_tui/                    TUI client binary (terminal-in-terminal)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                 CLI entry point (clap)
│       ├── app.rs                  TuiApp, event loop, MuxClient
│       ├── render.rs               Terminal-in-terminal rendering (crossterm)
│       ├── input.rs                Input routing, prefix key handling
│       ├── session.rs              Attach/detach/list/new-session
│       ├── status_bar.rs           Bottom status bar
│       └── theme.rs                Host terminal color adaptation
├── _old/                           Old prototype (reference only)
├── assets/
└── plans/
```

## Dependency Graph

```
oriterm (GUI binary) ──depends──> oriterm_mux (mux) ──depends──> oriterm_core (lib)
     │                              │                              │
     ├── winit                      ├── serde (optional)           ├── vte
     ├── wgpu                       ├── bincode (server mode)      ├── bitflags
     ├── swash                      ├── zstd (server mode)         ├── parking_lot
     ├── rustybuzz                  ├── rustls (remote transport)  ├── unicode-width
     ├── portable-pty               └── (no GUI, no PTY)           ├── base64
     ├── serde, toml, notify                                       ├── log
     ├── window-vibrancy                                           └── regex
     ├── clipboard-win / arboard
     ├── oriterm_mux
     └── oriterm_core

oriterm_tui (TUI binary) ──depends──> oriterm_mux ──depends──> oriterm_core
     │                                     │
     ├── crossterm                         (shared: protocol, client, domain types)
     ├── clap
     ├── oriterm_mux
     └── oriterm_core
```

Strictly one-way. `oriterm_core` has zero knowledge of GUI, fonts, PTY, config, mux, or platform APIs. `oriterm_mux` has zero knowledge of GUI, fonts, or platform APIs — it is a pure multiplexing layer that depends only on `oriterm_core` for terminal types. `oriterm_tui` has zero knowledge of GPU, fonts, or windowing — it renders entirely via terminal escape sequences.

## Threading Model

| Thread | Per | Owns | Lock Holds |
|--------|-----|------|------------|
| Main (UI) | process | winit EventLoop, windows, GpuState, GpuRenderer, FontCollection | microseconds (snapshot) |
| PTY Reader | pane | PTY read handle, read buffer, VTE Processor | microseconds (parse chunk) |
| Mux Server | process (daemon mode) | InProcessMux, socket listener, connections | microseconds (dispatch) |
| TUI Main | process (oriterm-tui) | crossterm event loop, MuxClient, TuiRenderer | microseconds (render frame) |

| Primitive | Per | Purpose |
|-----------|-----|---------|
| `FairMutex<Term<MuxEventProxy>>` | pane | Terminal state |
| `mpsc::channel<MuxEvent>` | process | Pane reader threads → mux event pump |
| `mpsc::channel<MuxNotification>` | process | Mux → GUI notification channel |
| `EventLoopProxy<TermEvent>` | process | Mux notifications → winit event loop wakeup |

**Critical pattern:** Lock → snapshot `RenderableContent` → unlock → GPU work (no lock held).

**Mux event flow (in-process):** PTY Reader → `MuxEvent` channel → `InProcessMux::poll_events()` → `MuxNotification` channel → GUI `about_to_wait()` → redraw.

**Mux event flow (daemon mode):** PTY Reader → `MuxEvent` → MuxServer → `OutputCoalescer` (1ms/16ms/100ms tiered) → push to client via IPC → GUI renders.

## Section Overview (43 Sections, 10 Tiers)

### Tier 0 — Core Library + Cross-Platform Architecture
| Section | Title | What |
|---------|-------|------|
| 01 | Cell + Grid | Cell, Row, Grid, Cursor, scrollback, editing, navigation |
| 02 | Term + VTE | Terminal state machine, VTE Handler, modes, palette, SGR |
| 03 | Cross-Platform | Platform abstractions for PTY, fonts, clipboard, GPU, window (day one) |

### Tier 1 — Process Layer
| Section | Title | What |
|---------|-------|------|
| 04 | PTY + Event Loop | PTY spawning, reader thread, event proxy, lock discipline |

### Tier 2 — Rendering Foundation
| Section | Title | What |
|---------|-------|------|
| 05 | Window + GPU | winit window, wgpu pipeline (Vulkan/DX12/Metal), staged render pipeline (Extract→Prepare→Render), atlas, offscreen targets |
| 06 | Font Pipeline | Multi-face loading, shaping, ligatures, fallback, built-in glyphs, emoji |
| 07 | 2D UI Framework | Drawing primitives, layout engine, widgets, overlay system (oriterm_ui crate) |

### Tier 3 — Interaction
| Section | Title | What |
|---------|-------|------|
| 08 | Keyboard Input | Legacy + Kitty encoding, keyboard dispatch, IME |
| 09 | Selection & Clipboard | 3-point selection, word/line/block modes, clipboard, paste filtering |
| 10 | Mouse Input & Reporting | Mouse reporting modes, selection state machine, auto-scroll |
| 11 | Search | Plain text + regex search, search UI overlay, match highlighting |
| 12 | Resize & Reflow | Window resize, grid reflow, PTY resize notification |
| 13 | Config & Keybindings | TOML config, hot reload, file watcher, keybinding system, CLI subcommands |
| 14 | URL Detection | Implicit URL detection, hover underline, Ctrl+click open |
| 40 | Vi/Copy Mode | Modal navigation (hjkl), word/line/bracket motions, visual selection, yank, search integration |
| 41 | Hints & Quick Select | Regex-based pattern matching, keyboard-selectable labels, configurable actions |

### Tier 4 — Chrome (Tab Bar, Drag, Routing, Shell, Menus)
| Section | Title | What |
|---------|-------|------|
| ~~15~~ | ~~Tab Struct & Management~~ | *Superseded → Sections 30, 32* |
| 16 | Tab Bar & Chrome | Layout, rendering, hit testing, bell pulse, tab hover preview |
| 17 | Drag & Drop | Chrome-style drag, tear-off, OS drag, merge detection |
| ~~18~~ | ~~Multi-Window & Lifecycle~~ | *Superseded → Section 32* |
| 19 | Event Routing & Scheduling | Coordinate systems, dispatch, frame budget, cursor blink |
| 20 | Shell Integration | Shell detection, injection, OSC 7/133, prompt state, two-parser, semantic zones, command notifications |
| 21 | Context Menu & Controls | GPU-rendered menus, config reload, settings UI, window controls |

### Tier 4M — Multiplexing Foundation (NEW)
| Section | Title | What |
|---------|-------|------|
| 29 | Mux Crate + Layout Engine | `oriterm_mux` crate, newtype IDs, immutable SplitTree, FloatingLayer, layout computation, spatial navigation |
| 30 | Pane Extraction + Domain System | Pane struct (from Tab), Domain trait, LocalDomain, WslDomain stub, registries, MuxEventProxy |
| 31 | In-Process Mux + Multi-Pane Rendering | InProcessMux, App rewiring, `prepare_pane_into()` with origin offsets, dividers, focus border, PaneRenderCache |
| 32 | Tab & Window Management (Mux-Aware) | Multi-tab via mux, multi-window shared GPU, tab CRUD, window lifecycle, cross-window tab movement, ConPTY-safe shutdown |
| 33 | Split Navigation + Floating Panes | Spatial navigation keybinds, divider drag resize, zoom/unzoom, floating pane creation/drag/resize, float↔tile toggle, undo/redo |

### Tier 5 — Hardening
| Section | Title | What |
|---------|-------|------|
| 22 | Terminal Modes | Comprehensive DECSET/DECRST table, mode interactions |
| 23 | Performance & Damage Tracking | Damage tracking, ring buffer, parsing optimization, benchmarks |
| 38 | Terminal Protocol Extensions | Capability reporting (DA, DECRQM, XTGETTCAP), color queries, extended SGR (underline styles/colors), window manipulation, DCS passthrough |
| 39 | Image Protocols | Kitty Graphics Protocol, Sixel, iTerm2 inline images, GPU compositing |
| 42 | Expose / Overview Mode | Mission Control-style live thumbnail grid of all panes, type-to-filter, keyboard/mouse navigation |

### Tier 6 — Polish
| Section | Title | What |
|---------|-------|------|
| 24 | Visual Polish | Cursor blink, hide-while-typing, minimum contrast, HiDPI, vector icons, background images, gradients, backdrop effects, scrollable menus |
| 25 | Theme System | 100+ themes, TOML theme files, discovery, light/dark auto-switch |
| 46 | macOS App Bundle & Platform Packaging | .app bundle, Info.plist, universal binary (x86_64+aarch64), DMG, CI build-macos job |

### Tier 7 — Advanced
| Section | Title | What |
|---------|-------|------|
| ~~26~~ | ~~Split Panes~~ | *Superseded → Sections 29, 31, 33* |
| 27 | Command Palette & Quick Terminal | Fuzzy search palette, global hotkey dropdown, notifications |
| 28 | Extensibility | Lua scripting, custom shaders, smart paste, undo close tab, session recording, workspaces |

### Tier 7A — Server + Persistence + Remote (NEW)
| Section | Title | What |
|---------|-------|------|
| 34 | IPC Protocol + Daemon Mode | Wire protocol (15-byte header, bincode+zstd), MuxServer daemon, OutputCoalescer (1ms push), MuxClient, auto-start daemon |
| 35 | Session Persistence + Remote Domains | Session save/load, crash recovery, scrollback archive, SshDomain, WslDomain full impl, tmux control mode |
| 36 | Remote Attach + Network Transport | TCP+TLS transport, SSH tunnel mode, authentication, MuxDomain for remote daemon, `oriterm connect` CLI, bandwidth-aware rendering |
| 37 | TUI Client | `oriterm-tui` binary — terminal-in-terminal client, attach/detach, prefix key, split/float rendering via crossterm, tmux replacement |

## Milestones

| Milestone | Section | What You See |
|-----------|---------|-------------|
| **M1: Lib compiles** | 01-02 complete | `cargo test -p oriterm_core` passes, Grid + VTE verified |
| **M2: Cross-platform foundations** | 03 complete | Platform abstractions defined for PTY, fonts, clipboard, GPU |
| **M3: Shell runs** | 04 complete | PTY spawns shell, I/O relayed (logged, no window) |
| **M4: Terminal renders** | 05 complete | Window opens, staged render pipeline, terminal grid visible, shell works |
| **M5: Full font pipeline** | 06 complete | Ligatures, emoji, fallback chains, box drawing, text decorations |
| **M6: UI framework** | 07 complete | Drawing primitives, layout engine, widgets, overlay system |
| **M7: Interactive** | 08-14, 40-41 complete | Keyboard, mouse, selection, clipboard, search, config, resize, URLs, vi mode, hints |
| **M8: Multiplexing** | 29-33 complete | Split panes, floating panes, multi-tab, multi-window — all through mux layer |
| **M8b: Chrome** | 16-17, 19-21 complete | Tab bar, drag/drop, event routing, shell integration, menus |
| **M9: Hardened** | 22-23, 38-39 complete | All terminal modes, protocol extensions, image protocols, performance optimized, damage tracking |
| **M10: Polished** | 24-25 complete | Cursor blink, 100+ themes, light/dark auto |
| **M11: Advanced** | 27-28 complete | Command palette, Lua scripting |
| **M12: Server mode** | 34-35 complete | Daemon keeps sessions alive, session persistence, SSH/WSL domains |
| **M13: Remote attach** | 36 complete | Connect GUI to remote daemon, SSH tunnel or TLS, bandwidth-aware rendering |
| **M14: TUI client** | 37 complete | `oriterm-tui` — headless attach/detach, terminal-in-terminal rendering, tmux replacement |

## Key References

All paths relative to `~/projects/reference_repos/console_repos/`.

### Terminal Core & State Machine
| What | Alacritty | Ghostty |
|------|-----------|---------|
| Terminal state (Term<T>) | `alacritty/alacritty_terminal/src/term/mod.rs` | `ghostty/src/terminal/Terminal.zig` |
| Event/callback system | `alacritty/alacritty_terminal/src/event.rs` | `ghostty/src/termio/message.zig` |
| Threading/synchronization | `alacritty/alacritty_terminal/src/sync.rs` (FairMutex) | `ghostty/src/Surface.zig` (3-thread model + mailboxes) |
| PTY event loop | `alacritty/alacritty_terminal/src/event_loop.rs` | `ghostty/src/termio/Termio.zig`, `ghostty/src/termio/Exec.zig` |

### Grid, Memory & Storage
| What | Alacritty | Ghostty |
|------|-----------|---------|
| Screen/grid | `alacritty/alacritty_terminal/src/grid/mod.rs` | `ghostty/src/terminal/Screen.zig` |
| Storage backend | `alacritty/alacritty_terminal/src/grid/storage.rs` (ring buffer) | `ghostty/src/terminal/PageList.zig` (page linked list + memory pools) |
| Page-based memory | — | `ghostty/src/terminal/page.zig` (contiguous page layout, offset pointers) |
| Resize/reflow | `alacritty/alacritty_terminal/src/grid/resize.rs` | `ghostty/src/terminal/PageList.zig` (resize within page structure) |

### Parsing & Performance
| What | Alacritty | Ghostty |
|------|-----------|---------|
| VTE parser | `alacritty/alacritty_terminal/src/vte/` (crate) | `ghostty/src/terminal/Parser.zig` |
| Stream processing | — | `ghostty/src/terminal/stream.zig` (SIMD-optimized) |
| SIMD acceleration | — | `ghostty/src/simd/vt.zig`, `ghostty/src/simd/codepoint_width.zig` |
| Damage tracking | `alacritty/alacritty_terminal/src/term/mod.rs` (dirty state) | `ghostty/src/terminal/page.zig` (Row.dirty), `ghostty/src/terminal/render.zig` |

### Terminal Features
| What | Alacritty | Ghostty |
|------|-----------|---------|
| Modes (DECSET/DECRST) | `alacritty/alacritty_terminal/src/term/mod.rs` | `ghostty/src/terminal/modes.zig` (comptime-generated, 8-byte packed) |
| Color/palette | `alacritty/alacritty_terminal/src/term/color.rs` | `ghostty/src/terminal/color.zig` (DynamicPalette with mask) |
| SGR attributes | `alacritty/alacritty_terminal/src/vte/ansi.rs` | `ghostty/src/terminal/sgr.zig` |
| Selection | `alacritty/alacritty_terminal/src/selection.rs` | `ghostty/src/terminal/Selection.zig` (3-point, tracked/untracked) |
| Key encoding | `alacritty/alacritty_terminal/src/term/mod.rs` | `ghostty/src/input/key_encode.zig` (Kitty + legacy) |
| OSC/DCS/CSI | `alacritty/alacritty_terminal/src/vte/ansi.rs` | `ghostty/src/terminal/osc.zig`, `ghostty/src/terminal/dcs.zig` |

### Rendering & Threading
| What | Alacritty | Ghostty |
|------|-----------|---------|
| Renderer thread | `alacritty/alacritty/src/renderer/mod.rs` | `ghostty/src/renderer/Thread.zig` (120 FPS timer, cursor blink) |
| Platform abstractions | `alacritty/alacritty/src/platform/` | `ghostty/src/apprt/` (macOS/Linux/Windows backends) |

### Old Prototype
| What | Where |
|------|-------|
| Old Cell/CellFlags | `_old/src/cell.rs` |
| Old GPU renderer | `_old/src/gpu/renderer.rs` |
| Old Grid | `_old/src/grid/mod.rs` |
| Old VTE handler | `_old/src/term_handler/mod.rs` |

## Anti-Patterns (explicitly forbidden)

1. **No god objects** — Max ~12 fields per struct. Split responsibilities.
2. **No lock during GPU work** — Snapshot under lock, render without lock.
3. **No separate VTE handler struct** — `Term<T>` implements `Handler` directly.
4. **No atomic workarounds** — Lock is held microseconds, no shadow state needed.
5. **No circular imports** — Crate boundary prevents it. Within binary, renderer receives data not domain objects.
6. **No rendering constants in grid** — Grid knows nothing about pixels.
7. **No `unwrap()` in library code** — Return `Result` or provide defaults.
