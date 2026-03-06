---
section: "06"
title: "Verification"
status: complete
goal: "Full test suite passes, no UI leaks remain, behavioral equivalence confirmed"
depends_on: ["02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Audit for UI Leaks"
    status: complete
  - id: "06.2"
    title: "Test Migration"
    status: complete
  - id: "06.3"
    title: "Behavioral Equivalence"
    status: deferred
  - id: "06.4"
    title: "Documentation"
    status: complete
  - id: "06.5"
    title: "Completion Checklist"
    status: complete
---

# Section 06: Verification

**Status:** Complete
**Goal:** The flattened mux is verified correct: all tests pass, no UI
concepts leak, and the GUI behaves identically to before the refactor.

**Depends on:** All previous sections.

---

## 06.1 Audit for UI Leaks

- [x] Run targeted greps across `oriterm_mux/src/`:
  ```
  grep -rn "TabId\|WindowId\|SessionId\|MuxTab\|MuxWindow\|SessionRegistry" \
    oriterm_mux/src/ --include="*.rs"
  grep -rn "GUI\|winit\|tab.bar\|frontend" \
    oriterm_mux/src/ --include="*.rs"
  ```
  First grep: zero tab/window/session type references remain.
  Second grep: zero UI-layer concept references remain.

- [x] Verify `oriterm_mux/Cargo.toml` has no GUI-related dependencies
  (no winit, wgpu, softbuffer — confirmed clean)

- [x] Verify one-way dependency: `oriterm_mux/Cargo.toml` does NOT list `oriterm`
      as a dependency (mux must not know about the GUI)

- [x] Verify `oriterm_mux/Cargo.toml` dependencies after flattening — all current
      deps serve the pane server (no unused deps from removed layout/session code)

- [x] Verify no circular imports within `oriterm/src/session/`: layout modules
      import `PaneId` from `oriterm_mux`, not from `crate::session::id`

- [x] Verify `oriterm_mux` public API surface:
  - Exports only: `PaneId`, `DomainId`, `ClientId`, `IdAllocator`,
    `MuxId`, `Pane`, `MarkCursor`, `PaneEntry`, `PaneRegistry`,
    `InProcessMux`, `ClosePaneResult`,
    `MuxEvent`, `MuxEventProxy`, `MuxNotification`,
    `Domain`, `DomainState`, `SpawnConfig`,
    `PtyConfig`, `PtyHandle`, `PtyControl`, `ExitStatus`, `spawn_pty`,
    `MuxBackend`, `EmbeddedMux`, `MuxClient`,
    protocol wire types (pane-only: `PaneSnapshot`, `WireCell`, etc.),
    server types
  - Does NOT export: `TabId`, `WindowId`, `SessionId`, `MuxTab`,
    `MuxWindow`, `SessionRegistry`, `SplitTree`, `SplitDirection`,
    `FloatingLayer`, `FloatingPane`, `Rect`, `PaneLayout`,
    `DividerLayout`, `Direction`, `MuxTabInfo`, `MuxWindowInfo`

---

## 06.2 Test Migration

- [x] All mux unit tests updated for flat pane model

### Integration tests: `tests/contract.rs`

- [x] `TestContext` struct: removed `window_id: WindowId` and `tab_id: TabId` fields
      (keeps only `pane_id: PaneId`)
- [x] Removed `use oriterm_mux::{TabId, WindowId}` import
- [x] Rewrote `embedded_context()`: no `create_window()`, uses `spawn_pane()` directly
- [x] Rewrote `daemon_context()`: no `create_window()`/`claim_window()`, uses `spawn_pane()`
- [x] All `muxbackend_contract_tests!` test bodies are pane-only

### Integration tests: `tests/e2e.rs`

- [x] Same factory function changes as contract.rs
- [x] Removed `TabId`, `WindowId` imports
- [x] Updated test helper struct to remove window/tab ID fields
- [x] All tests pane-centric (send_input, snapshot, search)

### Other test files

- [x] Layout tests relocated and passing in `oriterm/src/session/`
- [x] Test count verified: 4,620 tests passing across workspace (0 failures, 0 filtered)
  ```
  oriterm_core:  1900 passed
  oriterm_mux:   1271 passed
  oriterm_gpu:      6 passed
  oriterm:        382 passed
  contract:        20 passed
  e2e:             23 passed
  oriterm_ui:    1018 passed
  ```

---

## 06.3 Behavioral Equivalence

**Status:** Deferred — requires running the GUI binary on Windows. Cannot be
executed in the WSL cross-compilation environment. All automated tests pass;
manual smoke testing is required when the binary is next run.

- [ ] GUI starts, spawns a pane, displays terminal output
- [ ] Tab creation works (local session creates tab + mux spawns pane)
- [ ] Tab closing works (local session removes tab + mux closes pane)
- [ ] Split pane works (local session splits + mux spawns second pane)
- [ ] Window creation/closing works (local session only)
- [ ] Tab drag/tear-off/merge works (local session only)
- [ ] Pane title updates flow: mux notification -> GUI session -> tab bar
- [ ] Bell/alert flows: mux notification -> GUI session -> tab bar
- [ ] Resize flows: GUI -> `mux.resize_pane_grid()` -> PTY resize
- [ ] Keyboard input flows: GUI -> `mux.send_input()` -> PTY stdin
- [ ] Daemon disconnect cleanup

---

## 06.4 Documentation

- [x] Updated `CLAUDE.md` "Key Paths" to reflect workspace structure with
      session module, pane server, core, and GPU crate paths
- [x] No stale mux session references in `CLAUDE.md`
- [x] Updated memory files: architecture section reflects flat pane server
      and GUI-owned session model
- [x] `oriterm_mux/src/lib.rs` module doc describes the flat pane server
- [x] `oriterm/src/session/mod.rs` module doc describes the GUI session model
- [x] `oriterm_mux/Cargo.toml` description updated: "Pane server" not "Multiplexer"
- [x] `plans/muxbackend-boundary/` — completed plan, stale references are
      historical (describing what was migrated), no updates needed

---

## 06.5 Completion Checklist

- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes
- [x] Zero UI references in `oriterm_mux/src/` (verified by grep)
- [x] Test count: 4,620 tests passing, 0 failures
- [ ] GUI behavioral equivalence confirmed (deferred — requires Windows)
- [x] Documentation updated

**Exit Criteria:** `oriterm_mux` is a flat pane server with zero UI
awareness. `oriterm` owns all session/layout/presentation state. All
automated tests pass. Documentation is current. Manual smoke test
deferred to next Windows run.
