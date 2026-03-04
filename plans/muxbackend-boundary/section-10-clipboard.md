---
section: "10"
title: Clipboard Text Extraction Through MuxBackend
status: not-started
goal: Copy-to-clipboard uses MuxBackend for text extraction — no terminal lock from GUI
sections:
  - id: "10.1"
    title: Add extract_text / extract_html to MuxBackend
    status: not-started
  - id: "10.2"
    title: Add PDUs for text extraction
    status: not-started
  - id: "10.3"
    title: Refactor clipboard_ops
    status: not-started
  - id: "10.4"
    title: Completion Checklist
    status: not-started
---

# Section 10: Clipboard Text Extraction Through MuxBackend

**Status:** 📋 Planned
**Goal:** Copy-to-clipboard extracts text through `MuxBackend`, not by locking the terminal and reading grid cells directly.

**Rationale:** Selection can span scrollback rows beyond the snapshot viewport. Text extraction must read from the full grid, so it's a server-side operation.

**Crate:** `oriterm_mux` (trait + impls + protocol), `oriterm` (clipboard_ops)
**Depends on:** Section 07 (selection state on App)
**Key files:**
- `oriterm_mux/src/backend/mod.rs`
- `oriterm/src/app/clipboard_ops/mod.rs`

---

## 10.1 Add `extract_text` / `extract_html` to MuxBackend

**File:** `oriterm_mux/src/backend/mod.rs`

- [ ] Add methods:
  ```rust
  /// Extract plain text from a selection.
  ///
  /// Returns `None` if the pane doesn't exist.
  fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String>;

  /// Extract HTML and plain text from a selection.
  ///
  /// `font_family` and `font_size` are used for the HTML wrapper.
  /// Returns `None` if the pane doesn't exist.
  fn extract_html(
      &mut self,
      pane_id: PaneId,
      selection: &Selection,
      font_family: &str,
      font_size: f32,
  ) -> Option<(String, String)>;
  ```

**File:** `oriterm_mux/src/backend/embedded/mod.rs`

- [ ] Implement by locking the terminal:
  ```rust
  fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String> {
      let pane = self.panes.get(&pane_id)?;
      let term = pane.terminal().lock();
      Some(oriterm_core::selection::extract_text(term.grid(), selection))
  }
  fn extract_html(&mut self, pane_id: PaneId, selection: &Selection, family: &str, size: f32) -> Option<(String, String)> {
      let pane = self.panes.get(&pane_id)?;
      let term = pane.terminal().lock();
      Some(oriterm_core::selection::extract_html_with_text(term.grid(), selection, family, size))
  }
  ```

**File:** `oriterm_mux/src/backend/client/rpc_methods.rs`

- [ ] Implement as RPC:
  ```rust
  fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String> {
      let resp = self.rpc(MuxPdu::ExtractText { pane_id, selection: selection.to_wire() }).ok()?;
      match resp { MuxPdu::ExtractTextResp { text } => Some(text), _ => None }
  }
  ```
  - Need `Selection::to_wire()` / `WireSelection` type for serialization
  - Or serialize `Selection` directly if it derives `Serialize`
  - On RPC timeout/error: return `None`. Copy operation silently fails (no clipboard change). Log at `warn` level.

---

## 10.2 Add PDUs for Text Extraction

**File:** `oriterm_mux/src/protocol/messages.rs`

- [ ] Add wire selection type (or make Selection serializable):
  ```rust
  ExtractText { pane_id: PaneId, selection: WireSelection },
  ExtractTextResp { text: String },
  ExtractHtml { pane_id: PaneId, selection: WireSelection, font_family: String, font_size_x100: u16 },
  ExtractHtmlResp { html: String, text: String },
  ```
- [ ] Use integer font-size wire encoding (for example `font_size_x100`) to keep `MuxPdu: Eq` derivation valid
- [ ] Avoid `usize` in wire structs (`bincode` portability across 32/64-bit): use fixed-width integers
- [ ] Update `MsgType::from_u16` and `MuxPdu::msg_type()` mappings for extraction request/response variants
- [ ] Define `WireSelection` if Selection doesn't derive Serialize:
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct WireSelection {
      pub mode: u8,  // Char=0, Word=1, Line=2, Block=3
      pub anchor_row: u64, pub anchor_col: u32, pub anchor_side: u8,
      pub pivot_row: u64, pub pivot_col: u32, pub pivot_side: u8,
      pub end_row: u64, pub end_col: u32, pub end_side: u8,
  }
  ```

**File:** `oriterm_mux/src/server/dispatch.rs`

- [ ] Handle `ExtractText`: convert `WireSelection` → `Selection`, lock terminal, call `extract_text(grid, &sel)`, return `ExtractTextResp`
- [ ] Handle `ExtractHtml`: same pattern, call `extract_html_with_text`

---

## 10.3 Refactor `clipboard_ops`

**File:** `oriterm/src/app/clipboard_ops/mod.rs`

- [ ] Replace smart copy text extraction (lines ~31-32):
  ```rust
  // Before:
  let term = pane.terminal().lock();
  let text = extract_text(term.grid(), &sel);

  // After:
  let text = mux.extract_text(pane_id, &sel)?;
  ```
- [ ] Replace HTML extraction (lines ~87-89):
  ```rust
  // Before:
  let term = pane.terminal().lock();
  let (html, text) = extract_html_with_text(term.grid(), &sel, family, size);

  // After:
  let (html, text) = mux.extract_html(pane_id, &sel, family, size)?;
  ```
- [ ] Selection is now read from `self.pane_selection(pane_id)` (Section 07), not `pane.selection()`

---

## 10.4 Completion Checklist

- [ ] `MuxBackend` has `extract_text` and `extract_html`, implemented on both backends
- [ ] PDUs added, server handles them
- [ ] Zero `pane.terminal().lock()` in `clipboard_ops/`
- [ ] Copy works in daemon mode (Ctrl+C copies selected text)
- [ ] HTML copy works for rich paste
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -rn 'terminal().lock' oriterm/src/app/clipboard_ops/` returns zero matches.
