---
section: "14"
title: E2E MuxServer Integration Tests
status: not-started
goal: Integration tests that spin up a MuxServer and exercise the full MuxBackend contract
sections:
  - id: "14.1"
    title: Test harness setup
    status: not-started
  - id: "14.2"
    title: Core operation tests
    status: not-started
  - id: "14.3"
    title: Snapshot + rendering contract tests
    status: not-started
  - id: "14.4"
    title: Search + clipboard integration tests
    status: not-started
  - id: "14.5"
    title: Shared contract test suite
    status: not-started
  - id: "14.6"
    title: Completion Checklist
    status: not-started
---

# Section 14: E2E MuxServer Integration Tests

**Status:** 📋 Planned
**Goal:** Comprehensive integration tests that spin up a `MuxServer`, connect a `MuxClient`, and exercise every `MuxBackend` method. Both backends pass the same contract tests.

**Crate:** `oriterm_mux` (tests)
**Key files:**
- `oriterm_mux/tests/e2e.rs` — existing e2e tests (extend)
- `oriterm_mux/src/backend/client/` — MuxClient tests

---

## 14.1 Test Harness Setup

MuxServer is headless — no GUI required. This gives us strong integration coverage for the full backend boundary (MuxClient ↔ MuxServer) without platform dependencies. The existing `e2e.rs` already spins up a server and connects a client. Extend it for the new MuxBackend methods.

**File:** `oriterm_mux/tests/e2e.rs` (or new `tests/muxbackend_contract.rs`)

- [ ] Verify the existing test harness works: spin up server → connect client → handshake
- [ ] Add helper: `fn create_tab_with_shell(client: &mut MuxClient, window_id: WindowId) -> (TabId, PaneId)` — creates a tab and waits for the shell to produce initial output
- [ ] Add helper: `fn wait_for_snapshot(client: &mut MuxClient, pane_id: PaneId) -> PaneSnapshot` — polls until a non-empty snapshot is available (return owned snapshot to avoid borrow/lifetime issues in loops)
- [ ] Add deterministic output helper (avoid shell/profile variability):
  - write explicit bytes via `send_input` and wait for known markers in snapshot/extracted text
  - prefer `sh -c`/`cmd /C` fixed commands in `SpawnConfig` for CI portability

---

## 14.2 Core Operation Tests

Test the new MuxBackend methods via IPC.

- [ ] **test_resize_pane**: Create tab, resize via `resize_pane_grid(pane_id, 40, 100)`, refresh snapshot, verify `snapshot.cols == 100` and `snapshot.cells.len() == 40`
- [ ] **test_scroll_display**: Create tab, generate enough output to fill scrollback, call `scroll_display(pane_id, 10)`, refresh snapshot, verify `display_offset == 10`
- [ ] **test_scroll_to_bottom**: After scrolling up, call `scroll_to_bottom(pane_id)`, verify `display_offset == 0`
- [ ] **test_scroll_to_prompt**: (If shell supports OSC 133) Generate prompts, verify `scroll_to_previous_prompt` changes display_offset
- [ ] Gate prompt-navigation tests behind shell-integration capability or feature flag to avoid flaky failures on shells that do not emit OSC 133
- [ ] **test_pane_mode**: Create tab, verify `pane_mode(pane_id)` returns valid mode bits. Send `\x1b[?1000h` (enable mouse), refresh snapshot, verify mouse mode bit is set.
- [ ] **test_set_theme**: Call `set_pane_theme(pane_id, Dark, palette)`, refresh snapshot, verify palette changed
- [ ] **test_set_cursor_shape**: Call `set_cursor_shape(pane_id, Bar)`, refresh snapshot, verify `cursor.shape == WireCursorShape::Bar`
- [ ] **test_protocol_mismatch_error**: connect with incompatible feature/version flags and assert a clear `Error` response (fail-fast compatibility check)

---

## 14.3 Snapshot + Rendering Contract Tests

Verify snapshot enrichment works correctly.

- [ ] **test_snapshot_stable_row_base_no_eviction**: with large scrollback limit (no eviction), verify `stable_row_base == scrollback_len - display_offset`
- [ ] **test_snapshot_stable_row_base_with_eviction**: with tiny scrollback limit, generate heavy output and verify `stable_row_base` continues increasing even when `scrollback_len` is capped
- [ ] **test_snapshot_cols**: Create tab, resize to 120 cols, refresh snapshot. Verify `snapshot.cols == 120`
- [ ] **test_snapshot_dirty_flag**: Create tab, verify `is_pane_snapshot_dirty` is true after pane output, false after `clear_pane_snapshot_dirty`

---

## 14.4 Search + Clipboard + Notification Integration Tests

- [ ] **test_search_lifecycle**: Create tab, send known text (`echo "needle"`), open search, set query "needle", refresh snapshot, verify `search_matches` is non-empty, verify `search_query == "needle"`, close search, verify search data cleared
- [ ] **test_search_navigation**: Open search with multiple matches, call `search_next_match` / `search_prev_match`, verify `search_focused` changes
- [ ] **test_extract_text**: Create tab, send known text, create a selection spanning that text, call `extract_text(pane_id, &sel)`, verify the returned string matches
- [ ] **test_notification_flow**: Subscribe to a pane, send input that triggers output, verify `PaneDirty` notification received. Verify `PaneTitleChanged` after `echo -ne '\033]0;test title\a'`. Verify `PaneClosed` after pane exit.
- [ ] **test_snapshot_subscribe_lifecycle**: Subscribe to pane, verify dirty notifications arrive on output. Unsubscribe, verify no more notifications.

---

## 14.5 Shared Contract Test Suite

Create a test trait or macro that runs the same tests against both `EmbeddedMux` and `MuxClient`, ensuring they behave identically.

**File:** `oriterm_mux/src/backend/contract_tests.rs` (or `tests/contract.rs`)

- [ ] Define a macro or trait-based test runner:
  ```rust
  macro_rules! muxbackend_contract_tests {
      ($create_backend:expr) => {
          #[test]
          fn contract_create_tab() { /* ... */ }
          #[test]
          fn contract_resize() { /* ... */ }
          #[test]
          fn contract_scroll() { /* ... */ }
          #[test]
          fn contract_mode_query() { /* ... */ }
          #[test]
          fn contract_snapshot_lifecycle() { /* ... */ }
          // ...
      }
  }
  ```
- [ ] Instantiate for `EmbeddedMux`: `muxbackend_contract_tests!(|| EmbeddedMux::new(...))`
- [ ] Instantiate for `MuxClient` (via test server): `muxbackend_contract_tests!(|| setup_server_and_client())`
- [ ] Both must pass the same assertions

---

## 14.6 Completion Checklist

- [ ] Test harness spins up MuxServer and connects MuxClient reliably
- [ ] All new MuxBackend methods tested via IPC (resize, scroll, theme, mode, search, clipboard)
- [ ] Snapshot enrichment verified (stable_row_base, cols)
- [ ] Contract test suite passes on both EmbeddedMux and MuxClient
- [ ] Tests are not flaky (proper timeouts, retry-free)
- [ ] Use bounded waits with explicit failure diagnostics (last snapshot metadata, pending notifications) for easier CI debugging
- [ ] `./test-all.sh` includes the new tests
- [ ] CI runs integration tests

**Exit Criteria:** Spinning up a MuxServer and exercising the full MuxBackend API is a tested, repeatable workflow. Both backends are verified to have identical behavior.
