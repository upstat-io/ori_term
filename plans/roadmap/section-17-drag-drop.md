---
section: 17
title: Drag & Drop
status: complete
tier: 4
goal: Chrome-style tab dragging with tear-off, OS-level drag, and merge detection
sections:
  - id: "17.1"
    title: Drag State Machine
    status: complete
  - id: "17.2"
    title: OS-Level Drag + Merge
    status: complete
  - id: "17.3"
    title: Section Completion
    status: complete
---

# Section 17: Drag & Drop

**Status:** In Progress
**Goal:** Chrome-style tab dragging with tear-off, OS-level drag, and merge detection. Two-phase drag: within-bar reorder and tear-off to new window. Seamless drag continuation across windows via synthesized mouse events.

**Crate:** `oriterm` (binary only — no core changes)
**Dependencies:** `winit`
**Reference:** `_old/src/drag.rs`, `_old/src/app/tab_drag.rs`, `_old/src/app/cursor_hover.rs`

**Prerequisite:** Section 13 complete (tab management operations). Section 14 complete (tab bar layout and hit testing).

---

## 17.1 Drag State Machine

Chrome-style tab dragging with two phases: within-bar reorder and tear-off to new window. The state machine handles click-vs-drag disambiguation, threshold-based transitions, and pixel-perfect cursor tracking.

**File:** `oriterm/src/chrome/drag.rs`

**Reference:** `_old/src/drag.rs`, `_old/src/app/tab_drag.rs`, `_old/src/app/cursor_hover.rs`

- [x] `TabDragState` struct with `DragPhase` enum (`oriterm/src/app/tab_drag/mod.rs`):
  - [x] `tab_id: TabId` — which tab is being dragged
  - [x] `original_index / current_index` — for undo and reorder tracking
  - [x] `origin_x / origin_y` — initial mouse-down position (for threshold detection)
  - [x] `phase: DragPhase` — current phase of the drag
  - [x] `mouse_offset_in_tab: f32` — horizontal distance from tab's left edge to cursor at mouse-down
  - [x] `tab_bar_y / tab_bar_bottom` — bar geometry for tear-off detection
- [x] `DragPhase` enum:
  - [x] `Pending` — mouse is down on a tab, haven't moved past threshold yet
  - [x] `DraggingInBar` — actively reordering within the tab bar
- [x] State transitions:
  - [x] **Mouse down on tab** — `try_start_tab_drag()`: create Pending state, acquire width lock
  - [x] **Mouse move while Pending, distance > `DRAG_START_THRESHOLD` (10px)** — transition to `DraggingInBar`
    - [x] If single-tab window: skip `DraggingInBar`, go directly to OS-level drag + tear-off (17.2)
    - [x] If multi-tab window: enter `DraggingInBar`
  - [x] **Mouse move while `DraggingInBar`**:
    - [x] Compute `drag_visual_x` from cursor position minus `mouse_offset_in_tab`
    - [x] Compute insertion index from **cursor center** (`drag_x + tab_width / 2`)
    - [x] If index changed: `reorder_tab_silent()` — mux reorder + sync without animation
    - [x] Store drag visual on widget via `set_drag_visual()`
    - [x] Check tear-off condition (detection only — action deferred to 17.2):
      - [x] Cursor Y above bar > `TEAR_OFF_THRESHOLD_UP` (15px)
      - [x] Cursor Y below bar > `TEAR_OFF_THRESHOLD` (40px)
  - [x] **Mouse up while `DraggingInBar`** — `try_finish_tab_drag()`:
    - [x] Clear drag visual, start settle animation via `start_tab_reorder_slide()`
    - [x] Release width lock
  - [x] **Mouse up while `Pending`** — was a click:
    - [x] Tab was already switched on mouse-down, release width lock
  - [x] **Escape pressed while dragging** — `cancel_tab_drag()`:
    - [x] Restore tab to original position via `reorder_tab_silent()`
    - [x] Clear drag visual, release width lock
  - [x] **CursorLeft** — `cancel_tab_drag()`: same as Escape
- [x] Pure computation helpers (unit tested):
  - [x] `compute_drag_visual_x()` — clamp to [0, max_x]
  - [x] `compute_insertion_index()` — center-based, clamped to [0, count-1]
  - [x] `exceeds_tear_off()` — directional threshold check
- [x] Post-drag animation via existing `start_tab_reorder_slide()` (compositor-driven)

---

## 17.2 OS-Level Drag + Merge

When a tab is torn off the bar, it creates a new window that follows the cursor via the OS window-drag mechanism. On Windows, this uses `drag_window()` which enters a modal message loop (WM_MOVING). During this loop, we detect if the cursor passes over another oriterm window's tab bar — if so, merge the tab into that window.

**File:** `oriterm/src/app/tab_drag.rs` (continued), platform-specific

**Reference:** `_old/src/app/tab_drag.rs`

- [x] `tear_off_tab(&mut self, event_loop: &ActiveEventLoop)` (`oriterm/src/app/tab_drag/tear_off.rs`)
  - [x] Remove tab from source window's tab list
  - [x] Compute grab offset: where cursor appears in the new window's client area
    - [x] Account for `TAB_LEFT_MARGIN` — the tab doesn't start at x=0
    - [x] Preserve Y position relative to tab bar
  - [x] Create new window (`create_window_bare()`, initially hidden)
  - [x] Position new window so cursor is at `grab_offset` within client area
  - [x] Render new window (hidden) — clear surface before showing
  - [x] Show new window, then render source window (ensures correct z-order)
  - [x] If source window is now empty: close it
  - [x] Start OS drag on new window
- [x] `begin_os_tab_drag()` — Windows-specific:
  - [x] Collect merge target rects from other windows' tab bars
  - [x] Configure WM_MOVING handler to detect cursor over merge targets
  - [x] Set `torn_off_pending` state
  - [x] Call `window.drag_window()` — enters OS modal move loop, blocks until mouse-up
- [x] `check_torn_off_merge()` — called every event loop iteration in `about_to_wait` (`oriterm/src/app/tab_drag/merge.rs`):
  - [x] Check if WM_MOVING detected a merge target
  - [x] If merge detected:
    - [x] Find target window
    - [x] Compute insertion index via `compute_drop_index(target_wid, screen_x)`
    - [x] Move tab from torn window to target window at index via `move_tab_to_window_at`
    - [x] Resize tab to match target window's grid dimensions
    - [x] Close the torn (now empty) window
    - [x] Activate target window
  - [x] If merge was **live** (detected during WM_MOVING, not after):
    - [x] Start a new `DraggingInBar` state in the target window
    - [x] **Synthesize mouse-down**: `self.mouse.set_button_down(Left, true)` — because the OS modal loop consumed the original button-down event
    - [x] Set `merge_drag_suppress_release = true` — ignore the stale `WM_LBUTTONUP` that arrives after the modal loop ends
    - [x] This allows **seamless drag**: user drags tab out, over another window, and continues dragging within the target window without releasing the mouse button
  - [x] If no merge target: show the torn window (OS modal loop may have hidden it)
- [x] `compute_drop_index(&self, target_wid: WindowId, screen_x: f64) -> usize`
  - [x] Get target window bounds (using visible frame bounds on Windows — accounts for DWM invisible borders)
  - [x] Convert screen X to local X within target window
  - [x] Compute tab index from local X position: `((local_x - left_margin + tab_width/2) / tab_width).floor()`
  - [x] Clamp to `[0, target_tab_count]`
- [x] `merge_drag_suppress_release: bool` on App:
  - [x] Set to true after seamless merge
  - [x] Checked in mouse-up handler: if true, ignore the release and clear the flag
  - [x] Prevents the stale button-up from finalizing a non-existent drag

---

## 17.3 Section Completion

- [x] All 17.1–17.2 items complete
- [x] Drag: 10px threshold, center-based insertion, tear-off with directional thresholds, mouse offset preservation
- [x] OS drag + merge: WM_MOVING detection, seamless drag continuation, synthesized mouse-down, stale button-up suppression
- [x] Escape cancels drag and restores original tab position
- [x] Single-tab windows skip in-bar drag, go directly to OS-level tear-off
- [x] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — compiles
- [x] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [x] **Drag stress test**: rapid drag reorder across multiple windows, tear-off and merge in quick succession — no crash, no orphaned tabs
- [x] **Seamless merge test**: drag tab out of one window, over another window's tab bar, continue dragging without releasing mouse — tab seamlessly continues in target window

**Exit Criteria:** Chrome-style tab dragging works with click-vs-drag disambiguation, threshold-based tear-off, OS-level drag with merge detection, and seamless drag continuation across windows. No orphaned tabs, no stale mouse state.
