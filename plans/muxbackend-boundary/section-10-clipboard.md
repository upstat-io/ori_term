---
section: "10"
title: Clipboard Text Extraction Through MuxBackend
status: complete
goal: Copy-to-clipboard uses MuxBackend for text extraction — no terminal lock from GUI
sections:
  - id: "10.1"
    title: Add extract_text / extract_html to MuxBackend
    status: complete
  - id: "10.2"
    title: Add PDUs for text extraction
    status: complete
  - id: "10.3"
    title: Refactor clipboard_ops
    status: complete
  - id: "10.4"
    title: Completion Checklist
    status: complete
---

# Section 10: Clipboard Text Extraction Through MuxBackend

**Status:** Complete
**Goal:** Copy-to-clipboard extracts text through `MuxBackend`, not by locking the terminal and reading grid cells directly.

**Rationale:** Selection can span scrollback rows beyond the snapshot viewport. Text extraction must read from the full grid, so it's a server-side operation.

**Crate:** `oriterm_mux` (trait + impls + protocol), `oriterm` (clipboard_ops)
**Depends on:** Section 07 (selection state on App)
**Key files:**
- `oriterm_mux/src/backend/mod.rs`
- `oriterm/src/app/clipboard_ops/mod.rs`

---

## 10.1 Add `extract_text` / `extract_html` to MuxBackend

- [x] Added `extract_text(&mut self, pane_id, selection) -> Option<String>` to MuxBackend trait
- [x] Added `extract_html(&mut self, pane_id, selection, font_family, font_size) -> Option<(String, String)>` to MuxBackend trait
- [x] EmbeddedMux: locks terminal, delegates to `oriterm_core::selection::{extract_text, extract_html_with_text}`
- [x] MuxClient: sends `ExtractText`/`ExtractHtml` RPC, receives response

---

## 10.2 Add PDUs for Text Extraction

- [x] Added `WireSelection` type to `protocol/snapshot.rs` with `from_selection()` and `to_selection()` conversion methods
- [x] Added PDU variants: `ExtractText`, `ExtractTextResp`, `ExtractHtml`, `ExtractHtmlResp`
- [x] `font_size_x100: u16` wire encoding keeps `MuxPdu: Eq` valid
- [x] Added `MsgType` variants (0x0120–0x0121 requests, 0x0213–0x0214 responses)
- [x] Added `from_u16` and `msg_type()` mappings
- [x] Server dispatch handles both variants, locks terminal for text extraction

---

## 10.3 Refactor `clipboard_ops`

- [x] `extract_selection_text()`: uses `mux.extract_text(pane_id, &sel)` instead of `pane.terminal().lock()`
- [x] `extract_selection_html()`: uses `mux.extract_html(pane_id, &sel, &family, font_size)` instead of `pane.terminal().lock()`
- [x] Removed `use oriterm_core::selection::{extract_html_with_text, extract_text}` imports from clipboard_ops
- [x] Removed `self.active_pane()` calls — selection read from `self.pane_selection(pane_id)`
- [x] Zero `pane.terminal().lock()` calls in clipboard_ops

---

## 10.4 Completion Checklist

- [x] `MuxBackend` has `extract_text` and `extract_html`, implemented on both backends
- [x] PDUs added, server handles them
- [x] Zero `pane.terminal().lock()` in `clipboard_ops/`
- [x] `./build-all.sh` passes
- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes
