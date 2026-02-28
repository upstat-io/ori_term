---
section: 17
title: Drag & Drop
status: not-started
tier: 4
goal: Chrome-style tab dragging with tear-off, OS-level drag, and merge detection
sections:
  - id: "17.1"
    title: Drag State Machine
    status: not-started
  - id: "17.2"
    title: OS-Level Drag + Merge
    status: not-started
  - id: "17.3"
    title: Section Completion
    status: not-started
---

# Section 17: Drag & Drop

**Status:** Not Started
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

- [ ] `DragState` struct (not an enum — single struct with `DragPhase` enum):
  - [ ] `tab_id: TabId` — which tab is being dragged
  - [ ] `source_window: WindowId` — window the drag started in
  - [ ] `origin: PhysicalPosition<f64>` — initial mouse-down position (for threshold detection)
  - [ ] `phase: DragPhase` — current phase of the drag
  - [ ] `mouse_offset_in_tab: f64` — horizontal distance from tab's left edge to cursor at mouse-down. Preserved throughout the entire drag so the tab follows the cursor with the same offset. Example: grab a tab 20px from its left edge — that 20px offset is maintained as you drag.
- [ ] `DragPhase` enum:
  - [ ] `Pending` — mouse is down on a tab, haven't moved past threshold yet. If mouse-up arrives in this state, it was a click (switch to tab), not a drag.
  - [ ] `DraggingInBar` — actively reordering within the tab bar. Visual tab follows cursor, other tabs snap to make room.
- [ ] State transitions:
  - [ ] **Mouse down on tab** — create `DragState` with `phase: Pending`, record `origin` and `mouse_offset_in_tab`
  - [ ] **Mouse move while Pending, distance > `DRAG_START_THRESHOLD` (10px)** — transition to `DraggingInBar`
    - [ ] If single-tab window: skip `DraggingInBar`, go directly to OS-level drag + tear-off (17.2)
    - [ ] If multi-tab window: enter `DraggingInBar`
  - [ ] **Mouse move while `DraggingInBar`**:
    - [ ] Compute `drag_visual_x` from cursor position minus `mouse_offset_in_tab`
    - [ ] Compute insertion index from **cursor center** (`drag_x + tab_width / 2`), not left edge — creates a natural "sweet spot"
    - [ ] If index changed: swap tab in `tw.tabs` vec (immediate snap, no dodge animation during drag)
    - [ ] Store `drag_visual_x` for renderer
    - [ ] Check tear-off condition:
      - [ ] Cursor Y distance from tab bar > `TEAR_OFF_THRESHOLD` (40px) for downward/lateral
      - [ ] Cursor Y distance from tab bar > `TEAR_OFF_THRESHOLD_UP` (15px) for upward — lower threshold because upward tear-off feels more natural
      - [ ] If tear-off triggered: call `tear_off_tab()` (see 15.2)
  - [ ] **Mouse up while `DraggingInBar`** — finalize drop:
    - [ ] Tab is already in the correct position (swapped during drag)
    - [ ] Clear `drag_visual_x`, clear `DragState`
    - [ ] Rebuild tab bar cache
  - [ ] **Mouse up while `Pending`** — was a click, not a drag:
    - [ ] Tab was already switched to on mouse-down
    - [ ] Clear `DragState`
  - [ ] **Escape pressed while dragging** — cancel:
    - [ ] Return tab to original position in the vec
    - [ ] Clear `drag_visual_x`, clear `DragState`
- [ ] `update_drag_in_bar()` — called on every mouse move during `DraggingInBar`:
  - [ ] Clamp `drag_x` within `[0, max_x]` where `max_x` reserves space for buttons/controls
  - [ ] Compute insertion index: `((drag_x + tab_w / 2 - left_margin) / tab_w).floor()` clamped to `[0, tab_count - 1]`
  - [ ] If index differs from current: `tw.tabs.swap(current_idx, new_idx)`, adjust `tw.active_tab`
  - [ ] Mark `tab_bar_dirty` only if swap occurred
- [ ] Animation after drag:
  - [ ] On drag end: displaced tabs get animation offsets that decay to 0 over ~100ms
  - [ ] `decay_tab_animations() -> bool`: linear interpolation per frame, returns true if any offset is non-zero

---

## 17.2 OS-Level Drag + Merge

When a tab is torn off the bar, it creates a new window that follows the cursor via the OS window-drag mechanism. On Windows, this uses `drag_window()` which enters a modal message loop (WM_MOVING). During this loop, we detect if the cursor passes over another oriterm window's tab bar — if so, merge the tab into that window.

**File:** `oriterm/src/app/tab_drag.rs` (continued), platform-specific

**Reference:** `_old/src/app/tab_drag.rs`

- [ ] `tear_off_tab(&mut self, tab_id: TabId, source_wid: WindowId, event_loop: &ActiveEventLoop) -> Option<(WindowId, (i32, i32))>`  <!-- unblocks:32.4 -->
  - [ ] Remove tab from source window's tab list
  - [ ] Compute grab offset: where cursor appears in the new window's client area
    - [ ] Account for `TAB_LEFT_MARGIN` — the tab doesn't start at x=0
    - [ ] Preserve Y position relative to tab bar
  - [ ] Create new window (`create_window()`, initially hidden)
  - [ ] Position new window so cursor is at `grab_offset` within client area
  - [ ] Render new window (hidden) — fill GPU buffers before showing
  - [ ] Show new window, then render source window (ensures correct z-order)
  - [ ] Update `drag.source_window` to new window
  - [ ] If source window is now empty: close it
  - [ ] Return `(new_window_id, grab_offset)` for OS drag
- [ ] `begin_os_tab_drag()` — Windows-specific:
  - [ ] Collect merge target rects from other windows' tab bars
  - [ ] Configure WM_MOVING handler to detect cursor over merge targets
  - [ ] Set `torn_off_pending` state
  - [ ] Call `window.drag_window()` — enters OS modal move loop, blocks until mouse-up
- [ ] `check_torn_off_merge()` — called every event loop iteration during/after OS drag:
  - [ ] Check if WM_MOVING detected a merge target
  - [ ] If merge detected:
    - [ ] Find target window
    - [ ] Compute insertion index via `compute_drop_index(target_wid, screen_x)`
    - [ ] Remove tab from torn window, insert into target window at index
    - [ ] Resize tab to match target window's grid dimensions
    - [ ] Close the torn (now empty) window
    - [ ] Activate target window
  - [ ] If merge was **live** (detected during WM_MOVING, not after):
    - [ ] Start a new `DraggingInBar` state in the target window
    - [ ] **Synthesize mouse-down**: `self.left_mouse_down = true` — because the OS modal loop consumed the original button-down event
    - [ ] Set `merge_drag_suppress_release = true` — ignore the stale `WM_LBUTTONUP` that arrives after the modal loop ends
    - [ ] This allows **seamless drag**: user drags tab out, over another window, and continues dragging within the target window without releasing the mouse button
  - [ ] If no merge target: show the torn window (OS modal loop may have hidden it)
- [ ] `compute_drop_index(&self, target_wid: WindowId, screen_x: f64) -> usize`
  - [ ] Get target window bounds (using visible frame bounds on Windows — accounts for DWM invisible borders)
  - [ ] Convert screen X to local X within target window
  - [ ] Compute tab index from local X position: `((local_x - left_margin) / tab_width).floor()`
  - [ ] Clamp to `[0, target_tab_count]`
- [ ] `merge_drag_suppress_release: bool` on App:
  - [ ] Set to true after seamless merge
  - [ ] Checked in mouse-up handler: if true, ignore the release and clear the flag
  - [ ] Prevents the stale button-up from finalizing a non-existent drag

---

## 17.3 Section Completion

- [ ] All 17.1–17.2 items complete
- [ ] Drag: 10px threshold, center-based insertion, tear-off with directional thresholds, mouse offset preservation
- [ ] OS drag + merge: WM_MOVING detection, seamless drag continuation, synthesized mouse-down, stale button-up suppression
- [ ] Escape cancels drag and restores original tab position
- [ ] Single-tab windows skip in-bar drag, go directly to OS-level tear-off
- [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — compiles
- [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [ ] **Drag stress test**: rapid drag reorder across multiple windows, tear-off and merge in quick succession — no crash, no orphaned tabs
- [ ] **Seamless merge test**: drag tab out of one window, over another window's tab bar, continue dragging without releasing mouse — tab seamlessly continues in target window

**Exit Criteria:** Chrome-style tab dragging works with click-vs-drag disambiguation, threshold-based tear-off, OS-level drag with merge detection, and seamless drag continuation across windows. No orphaned tabs, no stale mouse state.
