---
section: "04"
title: Scroll Through MuxBackend
status: not-started
goal: All scroll operations go through MuxBackend — enables daemon mode scrollback
sections:
  - id: "04.1"
    title: Add scroll methods to MuxBackend
    status: not-started
  - id: "04.2"
    title: Add scroll PDUs to protocol
    status: not-started
  - id: "04.3"
    title: Server dispatch for scroll PDUs
    status: not-started
  - id: "04.4"
    title: Rewire GUI scroll callsites
    status: not-started
  - id: "04.5"
    title: Completion Checklist
    status: not-started
---

# Section 04: Scroll Through MuxBackend

**Status:** 📋 Planned
**Goal:** All viewport scroll operations route through `MuxBackend`. `display_offset` is server-side state — the next snapshot reflects the new scroll position.

**Crate:** `oriterm_mux` (trait + impls + protocol), `oriterm` (callsites)

---

## 04.1 Add Scroll Methods to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Add to trait:
  ```rust
  /// Scroll the viewport by `delta` lines (positive = toward history).
  fn scroll_display(&mut self, pane_id: PaneId, delta: isize);

  /// Scroll to the live terminal position (bottom).
  fn scroll_to_bottom(&mut self, pane_id: PaneId);

  /// Scroll to the nearest prompt above the current viewport.
  fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool;

  /// Scroll to the nearest prompt below the current viewport.
  fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool;
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement by delegating to `Pane` methods:
  ```rust
  fn scroll_display(&mut self, pane_id: PaneId, delta: isize) {
      if let Some(pane) = self.panes.get(&pane_id) {
          pane.scroll_display(delta);
      }
      self.snapshot_dirty.insert(pane_id);
  }
  fn scroll_to_bottom(&mut self, pane_id: PaneId) {
      if let Some(pane) = self.panes.get(&pane_id) {
          pane.scroll_to_bottom();
      }
      self.snapshot_dirty.insert(pane_id);
  }
  fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool {
      let scrolled = self.panes
          .get(&pane_id)
          .is_some_and(|p| p.scroll_to_previous_prompt());
      if scrolled { self.snapshot_dirty.insert(pane_id); }
      scrolled
  }
  fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool {
      let scrolled = self.panes
          .get(&pane_id)
          .is_some_and(|p| p.scroll_to_next_prompt());
      if scrolled { self.snapshot_dirty.insert(pane_id); }
      scrolled
  }
  ```

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Implement `scroll_display` and `scroll_to_bottom` as fire-and-forget PDUs (via `transport.fire_and_forget`)
- [ ] Implement `scroll_to_previous_prompt` and `scroll_to_next_prompt` as RPC returning `bool` from `ScrollToPromptAck`
- [ ] After each successful call, mark the client snapshot cache dirty: `self.dirty_panes.insert(pane_id)`
  - On RPC timeout/error: return `false` (no prompt found). GUI treats this the same as "no prompt markers in scrollback."

---

## 04.2 Add Scroll PDUs to Protocol

**File:** `oriterm_mux/src/protocol/messages.rs`

- [ ] Add PDU variants:
  ```rust
  ScrollDisplay { pane_id: PaneId, delta: i32 },      // fire-and-forget
  ScrollToBottom { pane_id: PaneId },                   // fire-and-forget
  ScrollToPrompt { pane_id: PaneId, direction: i8 },   // RPC request (-1 = prev, +1 = next)
  ScrollToPromptAck { scrolled: bool },                // RPC response
  ```
- [ ] Assign discriminants (next available after existing variants)
- [ ] Update `MsgType::from_u16` and `MuxPdu::msg_type()` mappings for all new variants
- [ ] `ScrollDisplay` and `ScrollToBottom` are fire-and-forget
- [ ] `ScrollToPrompt` is RPC because `MuxBackend` returns whether a prompt was found/scrolled
- [ ] Define conversion policy for `isize`↔`i32` delta:
  - `isize::MAX`/`isize::MIN` are used as semantic sentinels by existing code
  - clamp/cast explicitly when encoding the wire value to avoid overflow ambiguity

---

## 04.3 Server Dispatch for Scroll PDUs

**File:** `oriterm_mux/src/server/dispatch.rs`

- [ ] Handle `ScrollDisplay`: `pane.scroll_display(delta as isize)`
- [ ] Handle `ScrollToBottom`: `pane.scroll_to_bottom()`
- [ ] Handle `ScrollToPrompt`: call the corresponding pane method, capture `scrolled: bool`, return `ScrollToPromptAck { scrolled }`
- [ ] Mark pane dirty after successful scroll operations so the next snapshot reflects the new offset
- [ ] Emit/broadcast `NotifyPaneOutput { pane_id }` (or equivalent snapshot-invalidated notification) after scroll mutations so **other clients** subscribed to the pane refresh their caches too

---

## 04.4 Rewire GUI Scroll Callsites

Replace all direct `pane.scroll_display()`, `pane.scroll_to_bottom()`, `pane.scroll_to_previous_prompt()`, `pane.scroll_to_next_prompt()` calls.

**File:** `oriterm/src/app/keyboard_input/action_dispatch.rs`

- [ ] `Action::ScrollToTop` (line 50): Replace `pane.scroll_display(isize::MAX)` with `mux.scroll_display(pane_id, isize::MAX)`
- [ ] `Action::ScrollToBottom` (line 59): Replace `pane.scroll_to_bottom()` with `mux.scroll_to_bottom(pane_id)`
- [ ] `Action::SendText` (line 89): Replace `pane.scroll_to_bottom()` with `mux.scroll_to_bottom(pane_id)`
- [ ] `Action::PreviousPrompt` (line 156): Replace `pane.scroll_to_previous_prompt()` with `mux.scroll_to_previous_prompt(pane_id)`
- [ ] `Action::NextPrompt` (line 165): Replace `pane.scroll_to_next_prompt()` with `mux.scroll_to_next_prompt(pane_id)`

**File:** `oriterm/src/app/keyboard_input/mod.rs`

- [ ] `execute_scroll()` (line 260): Replace `pane.terminal().lock()` + `pane.scroll_display(lines)` with `mux.scroll_display(pane_id, delta)`. Page size must come from snapshot (`snapshot.cells.len()`) or a new `MuxBackend::scroll_page(pane_id, up: bool)` method. Note: `Action::ScrollPageUp`/`ScrollPageDown` in action_dispatch.rs delegates to this method.
- [ ] `encode_key_to_pty()` (line 248): Replace `pane.scroll_to_bottom()` with `mux.scroll_to_bottom(pane_id)`
- [ ] `handle_ime_commit()` (line 339): Replace `pane.scroll_to_bottom()` with `mux.scroll_to_bottom(pane_id)`

**File:** `oriterm/src/app/mouse_report/mod.rs`

- [ ] `handle_mouse_scroll()` (line 452): Replace `pane.scroll_display(scroll_lines)` with `mux.scroll_display(pane_id, scroll_lines)`

**File:** `oriterm/src/app/search_ui.rs` — `scroll_to_search_match()` (line 103–143)

- [ ] Replace `pane.scroll_display(delta)` with `mux.scroll_display(pane_id, delta)`
- [ ] Replace `pane.terminal().lock().grid()` reads with snapshot data:
  - `scrollback_len` → `snapshot.scrollback_len as usize`
  - `display_offset` → `snapshot.display_offset as usize`
  - `lines` → `snapshot.cells.len()`
- [ ] This method needs `pane_id` + access to snapshot + mux — may need signature change to `&mut self`

**File:** `oriterm/src/app/mouse_selection/helpers.rs` — `handle_auto_scroll()`

- [ ] Replace `pane.scroll_display(1)` / `pane.scroll_display(-1)` with `mux.scroll_display(pane_id, ±1)`
- [ ] This changes the function signature — will be fully addressed in Section 07 (selection refactor)
- [ ] For now, add a `MuxBackend` parameter or refactor the auto-scroll path

**File:** `oriterm/src/app/clipboard_ops/mod.rs`

- [ ] Replace `pane.scroll_to_bottom()` (line 195) with `mux.scroll_to_bottom(pane_id)`

---

## 04.5 Completion Checklist

- [ ] `MuxBackend` has 4 scroll methods, implemented on both backends
- [ ] Scroll PDUs added to protocol (`ScrollDisplay`, `ScrollToBottom`, `ScrollToPrompt`, `ScrollToPromptAck`)
- [ ] Zero calls to `pane.scroll_display()`, `pane.scroll_to_bottom()`, `pane.scroll_to_previous_prompt()`, `pane.scroll_to_next_prompt()` from `oriterm/`
- [ ] Scrollback works in daemon mode
- [ ] Prompt navigation works in daemon mode
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'scroll_display\|scroll_to_bottom\|scroll_to_previous_prompt\|scroll_to_next_prompt' oriterm/src/ | grep -v mux` returns zero matches (only MuxBackend calls remain).
