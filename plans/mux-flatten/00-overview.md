---
plan: "mux-flatten"
title: "Flatten Mux to Pure Pane Server"
status: complete
references:
  - "plans/roadmap/"
  - "plans/muxbackend-boundary/"
---

# Flatten Mux to Pure Pane Server

## Mission

Strip `oriterm_mux` of all UI-layer concepts (tabs, windows, layouts, "dirty"
flags, pixel geometry, tab bar awareness) so it becomes a flat pane server: spawn
panes, manage PTY lifecycles, route I/O. The UI layer (`oriterm`) owns all
grouping, arrangement, and presentation decisions.

**Litmus test:** If an SSH client connects to the mux daemon, it should see a
flat list of panes and interact with them directly. No tabs, no windows, no
layouts — just panes.

## Architecture

### Before (current)

```
oriterm (GUI)
  |
  v
oriterm_mux
  MuxWindow { tabs: Vec<TabId>, active_tab_idx }
  MuxTab { SplitTree, FloatingLayer, active_pane, zoom }
  SessionRegistry { windows, tabs }
  InProcessMux { window/tab/pane CRUD, layout compute }
  nav::navigate(PaneLayout)        <-- pixel-space navigation
  layout::compute(SplitTree, Rect) <-- pixel-space layout
  MuxNotification::WindowTabsChanged, TabLayoutChanged, PaneDirty, ...
  Protocol: CreateWindow, CreateTab, MoveTabToWindow, SetActiveTab, ...
  Server: window_to_client, subscriptions, snapshot_cache
```

### After (target)

```
oriterm (GUI)
  Session { windows, tabs, layouts, split trees, floating layers }
  TabBar, TabDrag, WindowManagement — all local
  layout::compute(), nav::navigate() — GUI-owned
  |
  | PaneId, spawn/close/resize/write
  v
oriterm_mux (flat pane server)
  PaneRegistry { PaneId -> PaneEntry }
  InProcessMux { pane CRUD only }
  MuxEvent { PaneOutput, PaneExited, PaneTitleChanged, ... }
  MuxNotification { PaneOutput, PaneClosed, PaneTitleChanged, ... }
  Protocol: SpawnPane, ClosePane, ResizePane, WriteToPty, GetSnapshot
  Server: pane subscriptions, pane I/O routing
```

## Design Principles

### 1. The mux is a process supervisor, not a window manager

The mux spawns shell processes, reads their output, and routes bytes. It has no
opinion on how those processes are presented to the user. A GUI client may render
them as tabs in windows with split panes. An SSH client may show one at a time.
A headless test may not render at all. The mux serves all of them identically.

**Motivated by:** The current mux has ~1,046 references to "window" across 39
files, comments that name `winit`, describe "tab bar order," and emit
"GUI notifications." An SSH attach client would need to either fake this entire
session model or ignore it — both are wrong.

### 2. Presentation state lives in the presenter

Tab grouping, window assignment, split layouts, floating pane geometry, zoom
state, active-tab tracking, navigation — all of these are presentation
decisions. Different clients will make different decisions. The mux must not
impose any of them.

### 3. Build the new before removing the old

The GUI currently depends on mux types for its session model. We must create
replacement types in `oriterm` first, migrate all consumers, then strip the mux.
At no point should both the old and new systems be partially wired — each phase
produces a fully buildable, testable state.

## Section Dependency Graph

```
            Section 01 (Target API + GUI Session Layer)
              |                         |
              v                         v
        Section 02                Section 04.1-04.4
        (Migrate oriterm)         (Copy Layout to oriterm)
              |                         |
              v                         |
        Section 03 (Flatten Mux Core)   |
              |                         |
              v                         |
        Section 05 (Flatten Protocol)   |
              |                         |
              v                         v
        Section 04.5 (Delete Mux Layout — after 03 + 05)
              |
              v
        Section 06 (Verification)
```

- Section 01 is the foundation — defines what the flat mux looks like and
  creates the GUI-side replacements.
- Sections 02 and 04.1-04.4 can be worked in parallel (02 swaps types,
  04.1-04.4 copies layout modules into oriterm).
- Section 03 depends on 02 (can't delete mux types until oriterm stops using
  them).
- Section 05 depends on 03 (protocol reflects the flattened mux).
- **Section 04.5 depends on 03 + 05**: the mux layout module cannot be
  deleted until all internal consumers (session, in_process, protocol,
  server, backend) have been stripped. This is a late-phase cleanup step.
- Section 06 depends on all.

**Cross-section interactions:**
- **Section 02 + 04.1-04.4**: Layout modules are copied to oriterm (04)
  while oriterm is being migrated to local types (02). Coordinate so the
  layout modules land in the right place with the right imports.
- **Section 04.5**: This is the DELETE step — removing `layout/` and `nav/`
  from `oriterm_mux`. It cannot happen until sections 03 and 05 have
  already stripped all internal consumers.

## Implementation Sequence

```
Phase 0 - Foundation
  +-- 01: Define flat mux API (what stays, what goes)
  +-- 01: Create GUI session types in oriterm

Phase 1 - Migration (parallelizable)
  +-- 02: Swap oriterm from mux session types to local types
  +-- 04.1-04.4: Copy layout modules from mux to oriterm
  Gate: oriterm builds and tests pass with local session + layout types

Phase 2 - Mux Surgery
  NOTE: 03.1 must rewrite tests BEFORE deleting source files to maintain
  buildability. See section-03 Phase A/B/C execution order.
  +-- 03: Strip InProcessMux of tab/window CRUD (tests first, then source)
  +-- 03: Simplify MuxEvent/MuxNotification to pane-only
  +-- 03: Remove SessionRegistry, MuxTab, MuxWindow, TabId, WindowId
  Gate: oriterm_mux builds with zero tab/window references
        (layout/ and nav/ still exist but have no internal consumers
         except protocol/backend — stripped in Phase 3)

Phase 3 - Protocol & Server
  +-- 05: Strip tab/window messages from wire protocol
  +-- 05: Simplify server dispatch to pane operations
  +-- 05: Simplify MuxBackend trait
  Gate: Full stack builds, daemon mode works with pane-only protocol

Phase 3b - Layout Deletion (depends on Phase 2 + 3)
  +-- 04.5: Delete mux layout/ and nav/ directories
  Gate: oriterm_mux has zero layout/nav references

Phase 4 - Verification
  +-- 06: All tests pass
  +-- 06: Clippy clean
  +-- 06: Behavioral equivalence confirmed
```

**Why this order:**
- Phase 0 is additive — no existing code changes, just new types.
- Phase 1 migrates consumers before removing the source. Both tracks
  are independent and can be parallelized.
- Phase 2 is the destructive phase — only safe after all consumers are
  migrated.
- Phase 3 cleans up the protocol and server to match the new reality.
- Phase 3b deletes mux layout/nav modules — safe only after ALL internal
  consumers (session, in_process, protocol, server, backend) are stripped.

**File size warnings:**
- `rpc_methods.rs` is **832 lines** — already over the 500-line limit.
  Phase 3 deletes ~40 methods from it, which will shrink it below the
  limit. If intermediate states leave it over 500 lines, split before
  continuing.
- `messages.rs` is **761 lines** — already over. Phase 3 deletes ~15
  message types. Same rule: split if intermediate state exceeds 500 lines.
- `transport/` is a **directory module** (`transport/mod.rs` 396 lines +
  `transport/reader.rs` 331 lines). Phase 3 removes `TabLayoutUpdate`
  and related code from both files.
- `embedded/mod.rs` is **507 lines** — slightly over the 500-line limit.
  Removing tab/window methods in Phase 3 should bring it under.

## Metrics (Current State)

| Module | UI-Coupled Items | Files Affected |
|--------|-----------------|----------------|
| `id/` | `TabId`, `WindowId`, `SessionId` | 2 |
| `session/` | `MuxTab`, `MuxWindow` | 2 |
| `registry/` | `SessionRegistry` | 2 |
| `mux_event/` | 7 notification variants (5 removed, 2 renamed) | 2 |
| `in_process/` | tab/window CRUD | 4 |
| `layout/` | `SplitTree`, `FloatingLayer`, `Rect`, `compute`, `nav` | 10+ |
| `protocol/` | ~15 tab/window message types | 4 |
| `server/` | `window_to_client`, dispatch, notify | 8+ |
| `backend/` | session queries, snapshot cache | 7 |
| **Total** | **~1,046 "window" refs across 39 files** | |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Target API & GUI Session | ~400 new | Medium | — |
| 02 Migrate oriterm | ~600 changed | Medium | 01 |
| 03 Flatten Mux Core | ~800 deleted | **High** | 02 |
| 04.1-04.4 Copy Layout | ~500 copied | Medium | 01 |
| 05 Flatten Protocol & Server | ~600 changed | **High** | 03 |
| 04.5 Delete Mux Layout | ~100 deleted | Low | 03, 05 |
| 06 Verification | ~200 | Low | All |
| **Total new** | **~400** | | |
| **Total deleted** | **~1,500+** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Target API & GUI Session Layer | `section-01-target-api.md` | Complete |
| 02 | Migrate oriterm to Own Session Types | `section-02-migrate-oriterm.md` | Complete |
| 03 | Flatten Mux Core | `section-03-flatten-core.md` | Complete |
| 04 | Relocate Layout Modules | `section-04-relocate-layout.md` | Complete |
| 05 | Flatten Protocol & Server | `section-05-flatten-protocol.md` | Complete |
| 06 | Verification | `section-06-verification.md` | Complete |
