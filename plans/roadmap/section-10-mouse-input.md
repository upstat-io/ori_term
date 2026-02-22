---
section: 10
title: Mouse Input & Reporting
status: complete
tier: 3
goal: Mouse reporting for terminal apps + mouse selection state machine
sections:
  - id: "10.1"
    title: Mouse Selection State Machine
    status: complete
  - id: "10.2"
    title: Mouse Reporting
    status: complete
  - id: "10.3"
    title: Section Completion
    status: complete
---

# Section 10: Mouse Input & Reporting

**Status:** Complete
**Goal:** Implement the mouse input layer: a state machine for tracking selection gestures, and mouse event reporting to the PTY for terminal applications that request it (vim, tmux, htop, etc.). Mouse reporting supports all three encoding formats (X10 normal, UTF-8, SGR) and all tracking modes.

**Crate:** `oriterm` (binary)
**Dependencies:** `winit` (mouse events), `oriterm_core` (TermMode, Grid)
**Reference:** `_old/src/app/mouse_report.rs`, `_old/src/app/mouse_selection.rs`, `_old/src/app/input_mouse.rs`

**Prerequisite:** Section 07 complete (Selection model and rendering). Section 03 complete (PTY send channel). Section 02 complete (TermMode flags for mouse mode detection).

---

## 10.1 Mouse Selection State Machine

Centralized state machine for tracking mouse gesture state. Coordinates between selection creation (Section 08) and mouse reporting (10.2), ensuring clean separation of concerns.

**File:** `oriterm/src/app/mouse_selection/mod.rs`

**Implementation note:** The existing architecture (free functions + `MouseState` + `Tab`-owned selection) is cleaner than the `SelectionAction`/`SelectionState` enum described in the original spec. All functionality is covered.

- [x] `MouseState` struct (tracks left_down, touchdown, drag_active, click_detector, cursor_pos, last_reported_cell)
- [x] `handle_press` — click detection, shift-extend, word/line boundary computation
- [x] `handle_drag` — threshold check, endpoint update with mode-aware snapping
- [x] `handle_release` — clears drag state
- [x] `pixel_to_cell` / `pixel_to_side` — coordinate conversion
- [x] `classify_press` — pure logic for determining selection action
- [x] `redirect_spacer` — wide char spacer handling
- [x] `handle_auto_scroll` — viewport scrolling when dragging outside grid
- [x] Comprehensive tests in `mouse_selection/tests.rs`

---

## 10.2 Mouse Reporting

Encode mouse events and send to PTY when terminal applications request mouse tracking. Supports all three encoding formats and all tracking modes.

**Files:**
- `oriterm/src/app/mouse_report/mod.rs` — encoding functions + `impl App` dispatch
- `oriterm/src/app/mouse_report/tests.rs` — 31 encoding + dispatch tests
- `oriterm_core/src/term/mode/mod.rs` — `ALTERNATE_SCROLL` flag added
- `oriterm_core/src/term/handler/modes.rs` — DECSET/DECRST wired for AlternateScroll
- `oriterm_core/src/term/handler/helpers.rs` — mode flag mapping wired

- [x] **Mouse tracking modes** (checked via TermMode flags):
  - [x] `MOUSE_REPORT_CLICK` (DECSET 1000) — report button press/release only
  - [x] `MOUSE_DRAG` (DECSET 1002) — report press/release + drag motion (button held)
  - [x] `MOUSE_MOTION` (DECSET 1003) — report all motion (even without button)
  - [x] No flag set: mouse events are local-only (selection, no PTY reporting)
- [x] **Mouse encoding modes** (checked via TermMode flags):
  - [x] `MOUSE_SGR` (DECSET 1006) — preferred: `ESC[<code;col;row M/m`
  - [x] `MOUSE_UTF8` (DECSET 1005) — coordinates UTF-8 encoded
  - [x] Default (X10 normal) — `ESC[M cb cx cy` (coordinates limited to 222)
- [x] **Button encoding**: 0=left, 1=middle, 2=right, 3=release(normal), 64=scroll up, 65=scroll down, +32=motion
- [x] **Modifier bits**: +4 Shift, +8 Alt, +16 Ctrl
- [x] **SGR encoding**: `\x1b[<{code};{col+1};{row+1}{M|m}` — stack-allocated, no coord limit
- [x] **UTF-8 encoding**: `\x1b[M` + UTF-8 values, custom 2-byte for coords >= 95
- [x] **Normal (X10) encoding**: `\x1b[M` + 3 bytes, coords clamped to 222
- [x] **Mouse mode priority over selection**: when ANY_MOUSE active, events go to PTY
- [x] **Shift bypasses mouse reporting**: Shift+click always does local selection
- [x] **Motion deduplication**: `last_reported_cell` on MouseState, only report on cell change
- [x] **Alternate scroll mode** (DECSET 1007):
  - [x] `ALTERNATE_SCROLL` TermMode flag (default on, matching xterm)
  - [x] Alt screen + ALTERNATE_SCROLL: scroll wheel → `\x1bOA`/`\x1bOB` (SS3 arrow keys)
- [x] **Mouse event dispatch**:
  - [x] `should_report_mouse()` — checks ANY_MOUSE + !Shift
  - [x] `report_mouse_button()` — encode + write to PTY
  - [x] `report_mouse_motion()` — motion dedup + encode
  - [x] `handle_mouse_wheel()` — 3-tier: report → alt scroll → viewport scroll
  - [x] `handle_mouse_input()` — left/middle/right button dispatch
- [x] **Tests** (31 tests in `mouse_report/tests.rs`):
  - [x] SGR encoding (8 tests): left/middle/right, release, coords, modifiers, scroll, motion, large coords
  - [x] Normal encoding (3 tests): correct format, coord clamping, release code
  - [x] UTF-8 encoding (3 tests): small coords, multi-byte, out-of-range
  - [x] button_code (6 tests): all buttons + motion offset
  - [x] apply_modifiers (5 tests): none, shift, alt, ctrl, combined
  - [x] Dispatch (6 tests): SGR/UTF-8/Normal selection, SGR priority, release codes

---

## 10.3 Section Completion

- [x] All 10.1-10.2 items complete
- [x] `./test-all.sh` — all 1062+ tests pass
- [x] `./clippy-all.sh` — no warnings
- [x] Mouse selection state machine handles all gesture types (single/double/triple click, drag, release)
- [x] Drag threshold prevents accidental selection
- [x] Mouse reporting sends correct sequences for all three encoding formats (SGR, UTF-8, X10)
- [x] All tracking modes work: click-only, drag, all-motion
- [x] Modifier bits correct in mouse reports (Shift, Alt, Ctrl)
- [x] Scroll wheel events reported correctly
- [x] Shift bypasses mouse reporting for local selection
- [x] Motion events deduplicated (only report on cell change)
- [x] Alternate scroll mode converts scroll to arrow keys in alt screen
- [x] Mouse mode and selection mode coexist correctly (mutual exclusion with Shift override)

**Exit Criteria:** Mouse reporting works correctly for all terminal applications that use it. vim, tmux, htop, and other mouse-aware apps receive correct mouse events. Selection and reporting coexist cleanly with Shift-override convention.
