---
section: 6
title: Font Pipeline + Best-in-Class Glyph Rendering
status: in-progress
tier: 2
goal: "Best font rendering of any terminal emulator. Full shaping pipeline with hinting, LCD subpixel rendering, subpixel positioning, proper font synthesis, and automated visual regression testing. The feature users switch terminals for."
sections:
  - id: "6.1"
    title: Multi-Face Font Loading
    status: complete
  - id: "6.2"
    title: Fallback Chain + Cap-Height Normalization
    status: complete
  - id: "6.3"
    title: Run Segmentation
    status: complete
  - id: "6.4"
    title: Rustybuzz Text Shaping
    status: in-progress
  - id: "6.5"
    title: Ligature + Multi-Cell Glyph Handling
    status: in-progress
  - id: "6.6"
    title: Combining Marks + Zero-Width Characters
    status: complete
  - id: "6.7"
    title: OpenType Feature Control
    status: complete
  - id: "6.8"
    title: Advanced Atlas (Guillotine + LRU + Multi-Page)
    status: complete
  - id: "6.9"
    title: Built-in Geometric Glyphs
    status: complete
  - id: "6.10"
    title: Color Emoji
    status: complete
  - id: "6.11"
    title: Font Synthesis (Bold + Italic)
    status: complete
  - id: "6.12"
    title: Text Decorations
    status: in-progress
  - id: "6.13"
    title: UI Text Shaping
    status: complete
  - id: "6.14"
    title: Pre-Caching + Performance
    status: complete
  - id: "6.15"
    title: Hinting
    status: complete
  - id: "6.16"
    title: Subpixel Rendering (LCD)
    status: in-progress
  - id: "6.17"
    title: Subpixel Glyph Positioning
    status: complete
  - id: "6.18"
    title: Visual Regression Testing
    status: complete
  - id: "6.19"
    title: Variable Font Axes
    status: complete
  - id: "6.20"
    title: Font Codepoint Mapping
    status: complete
  - id: "6.21"
    title: Section Completion
    status: not-started
---

# Section 06: Font Pipeline + Best-in-Class Glyph Rendering

**Status:** 📋 Planned
**Goal:** Best font rendering of any terminal emulator — not just feature-complete but visually superior. Full rustybuzz shaping pipeline with ligatures, cap-height normalized fallback chains, pixel-perfect built-in glyphs, color emoji, proper font synthesis (embolden + skew, not crude hacks), hinting with auto-DPI detection, LCD subpixel rendering with per-channel blending, subpixel glyph positioning, and automated visual regression testing. This is the feature users switch terminals for.

**Crate:** `oriterm` (binary)
**Dependencies:** `swash`, `rustybuzz`, `zeno` (via swash, for `Format`, `Vector`, `Transform`), `dwrote` (Windows)
**Reference:** `_old/src/font/`, `_old/src/gpu/atlas.rs`, `_old/src/gpu/builtin_glyphs.rs`, WezTerm `wezterm-font/` (LCD rendering, hinting config), Ghostty `src/font/face/freetype.zig` (embolden formula), cosmic-text `src/swash.rs` (subpixel positioning)

**Prerequisite:** Section 05B complete (startup performance). This section replaces the basic font path with the full pipeline.

**Why this is the differentiator:**
- **Alacritty**: No ligatures, no shaping, no color emoji. Refuses to add them.
- **WezTerm**: Has ligatures but rendering artifacts, glyph bleeding, incorrect fallback sizing. 100+ open font issues.
- **Ghostty**: Platform-inconsistent (CoreText on macOS, freetype on Linux — same font looks different). No LCD subpixel.
- **Kitty**: Decent but quirks with certain fonts, no LCD subpixel.
- **ori_term**: Same rasterizer (swash) + same shaper (rustybuzz) = identical rendering on all platforms. Plus hinting, LCD subpixel, proper synthesis, and subpixel positioning — features no competitor offers together.

---

## 6.1 Multi-Face Font Loading

Load all 4 style variants (Regular, Bold, Italic, BoldItalic) from the primary font family.

**File:** `oriterm/src/font/collection.rs`

**Reference:** `_old/src/font/collection.rs`

- [x] `FaceData` struct
  - [x] Fields:
    - `bytes: Arc<Vec<u8>>` — raw font file bytes (shared across variants from same file)
    - `face_index: u32` — index within .ttc collection
    - `offset: u32` — byte offset to font table directory
    - `cache_key: swash::CacheKey` — swash cache identifier
- [x] `FaceIdx` newtype — `pub struct FaceIdx(pub u16)`
  - [x] 0–3: primary styles (Regular=0, Bold=1, Italic=2, BoldItalic=3)
  - [x] 4+: fallback fonts in priority order
- [x] `FontCollection` expanded fields:
  - [x] `primary: [Option<FaceData>; 4]` — Regular, Bold, Italic, BoldItalic
  - [x] `has_variant: [bool; 4]` — true = real font file, false = fallback to Regular
  - [x] `font_paths: [Option<PathBuf>; 4]` — stored in `FontSet` / `FamilyDiscovery`, consumed eagerly during loading
  - [x] `weight: u16` — CSS weight (100–900, default 400)
- [x] Loading pipeline:
  - [x] Load Regular (required — fail if missing)
  - [x] Try loading Bold, Italic, BoldItalic from same family
  - [x] If variant not found: `has_variant[i] = false` (will use Regular + synthetic styling)
  - [x] Compute cell metrics from Regular face (cell_width from 'M' advance, cell_height from ascent + descent)
- [x] Platform discovery (`font/discovery.rs`):
  - [x] Windows (dwrote): enumerate via DirectWrite API by family name
  - [x] Linux: scan `~/.local/share/fonts/`, `/usr/share/fonts/`, `/usr/local/share/fonts/`
  - [x] Family search order: user-configured > JetBrains Mono > Cascadia Code > Consolas > Courier New
- [x] `find_face_for_char(&self, ch: char, preferred_style: GlyphStyle) -> Option<FaceIdx>` — implemented as `resolve()` returning `ResolvedGlyph`
  - [x] Try preferred style in primary
  - [x] Fall back to Regular in primary
  - [x] Fall back through fallback chain
  - [x] Return None only if .notdef everywhere — returns .notdef (glyph_id=0) from Regular
- [x] **Tests**:
  - [x] Load a system font, all 4 variants attempted
  - [x] `find_face_for_char('A', Bold)` returns Bold face if available
  - [x] `find_face_for_char('A', Bold)` returns Regular if no Bold face
  - [x] Unknown char falls to fallback chain

---

## 6.2 Fallback Chain + Cap-Height Normalization

Fallback fonts for characters missing from the primary (CJK, symbols, emoji). Visual consistency via cap-height normalization.

**File:** `oriterm/src/font/collection.rs` (continued)

**Reference:** `_old/src/font/collection.rs` (cap_height_px, FallbackMeta)

- [x] `FallbackMeta` struct
  - [x] Fields:
    - `features: Vec<rustybuzz::Feature>` — per-fallback OpenType features (override collection defaults) — deferred to Section 6.7 (OpenType Feature Control)
    - `scale_factor: f32` — cap-height normalization ratio
    - `size_offset: f32` — user-configured size offset in points
- [x] Fallback loading:
  - [x] `fallbacks: Vec<FaceData>` — priority-ordered fallback fonts
  - [x] `fallback_meta: Vec<FallbackMeta>` — per-fallback metadata (1:1 with fallbacks)
  - [x] User-configured fallbacks loaded first (from config TOML)
  - [x] System-discovered fallbacks loaded after
  - [x] Lazy loading: `ensure_fallbacks_loaded()` called once on first use — implemented as eager loading during `FontCollection::new()` (design evolved; no lazy loading needed)
- [x] Cap-height normalization:
  - [x] `cap_height_px(bytes, face_index, size) -> f32`
    - [x] Read OS/2 table `sCapHeight` field via rustybuzz Face
    - [x] If missing: estimate as `ascender * 0.75`
    - [x] Convert from font units: `cap_units / upem * size`
  - [x] `primary_cap_height_px: f32` — computed from Regular at load time
  - [x] Per-fallback: `scale_factor = primary_cap_height / fallback_cap_height`
  - [x] Effective size: `base_size * scale_factor + size_offset`
  - [x] **Why:** Noto Sans CJK looks tiny next to JetBrains Mono at same pt size. Normalizing by cap-height makes glyphs visually consistent.
- [x] `effective_size(&self, face_idx: FaceIdx) -> f32`
  - [x] Primary faces: base size
  - [x] Fallback faces: `base_size * meta.scale_factor + meta.size_offset`
- [x] User-configurable per-fallback:
  ```toml
  [[font.fallback]]
  family = "Noto Sans CJK"
  features = ["-liga"]
  size_offset = -2.0
  ```
- [x] **Tests**:
  - [x] Fallback chain resolves CJK char to CJK font
  - [x] Cap-height scale factor computed correctly (known font pair) — tested via `effective_size_for_with_scaling`
  - [x] Effective size for fallback differs from primary
  - [x] User size_offset applied

---

## 6.3 Run Segmentation

Break a terminal row into shaping runs. Each run is a contiguous sequence of characters that can be shaped together (same font face, no breaks).

**File:** `oriterm/src/font/shaper.rs`

**Reference:** `_old/src/font/shaper.rs` (prepare_line)

- [x] `ShapingRun` struct
  - [x] Fields:
    - `text: String` — base characters + combining marks for this run
    - `face_idx: FaceIdx` — which font face to shape with
    - `col_start: usize` — grid column where run starts
    - `byte_to_col: Vec<usize>` — maps byte offset in `text` → grid column
  - [x] byte_to_col is critical for mapping rustybuzz cluster indices back to grid positions
- [x] `prepare_line(row: &[Cell], cols: usize, collection: &FontCollection, runs: &mut Vec<ShapingRun>)`
  - [x] Iterate cells left to right
  - [x] Skip `WIDE_CHAR_SPACER` cells (they're part of the preceding wide char)
  - [x] For each cell:
    - [x] Determine face via `find_face_for_char(cell.ch, style_from_flags(cell.flags))`
    - [x] If face differs from current run, or cell is space/null: start new run
    - [x] Append `cell.ch` to current run's text
    - [x] Record byte offset → column mapping
    - [x] Append zero-width characters (combining marks) from cell at same column mapping
  - [x] Run breaks on:
    - [x] Space (' ') or null ('\0') — spaces excluded from run text but don't break same-face runs
    - [x] Font face change (different glyph found in different face)
    - [x] Built-in glyph character (box drawing, blocks, braille, powerline)
    - [x] Wide char spacer
  - [x] Runs reuse a scratch `Vec<ShapingRun>` (cleared + refilled each frame, not reallocated)
- [x] **Tests**:
  - [x] `"hello world"` → same-face chars merge across spaces, space excluded from run text
  - [x] `"hello你好"` → two runs if CJK resolves to different face (wide char test covers face resolution)
  - [x] `"a\u{0301}"` (a + combining accent) → single run with "á" text, byte_to_col maps both to same column
  - [x] `"━"` (box drawing) → no run (handled by builtin glyph system)

---

## 6.4 Rustybuzz Text Shaping

Shape each run through rustybuzz to produce positioned glyphs with correct ligature substitution.

**File:** `oriterm/src/font/shaper.rs` (continued)

**Reference:** `_old/src/font/shaper.rs` (shape_prepared_runs)

- [x] Two-phase API:
  - [x] Phase 1: `prepare_line()` — segment into runs (immutable, reuses scratch buffers)
  - [x] Phase 2: `shape_prepared_runs()` — shape each run (needs rustybuzz Face references)
  - [x] **Why two phases?** Create rustybuzz `Face` objects once per frame, reuse across all rows. Faces borrow font bytes, so they must outlive shaping calls.
- [x] `shape_prepared_runs(runs: &[ShapingRun], faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<ShapedGlyph>)`
  - [x] For each run:
    - [x] Create `rustybuzz::UnicodeBuffer`, push run's text
    - [x] Set direction: `LeftToRight` (terminal is always LTR)
    - [x] Get features for this face: `collection.features_for_face(run.face_idx)`
    - [x] Call `rustybuzz::shape(face, &features, buffer)`
    - [x] Extract `glyph_infos()` and `glyph_positions()`
    - [x] Scale: `effective_size / upem`
    - [x] For each (info, position) pair:
      - [x] Map `info.cluster` (byte offset) → grid column via `run.byte_to_col`
      - [x] Compute `col_span` from advance: `(x_advance * scale / cell_width).round().max(1)`
      - [x] Emit `ShapedGlyph`
- [x] `ShapedGlyph` struct
  - [x] Fields:
    - `glyph_id: u16` — rustybuzz glyph ID (NOT codepoint — this is the shaped result)
    - `face_idx: FaceIdx` — which face this was shaped from
    - `col_start: usize` — first grid column this glyph occupies
    - `col_span: usize` — how many columns (1 = normal, 2+ = ligature or wide char)
    - `x_offset: f32` — shaper x positioning offset (pixels)
    - `y_offset: f32` — shaper y positioning offset (pixels)
- [x] Output reuses scratch `Vec<ShapedGlyph>` (cleared + refilled each row)
- [x] **Tests**:
  - [x] `"hello"` → 5 glyphs, each col_span=1
  - [ ] `"=>"` with ligature-supporting font → 1 glyph, col_span=2 <!-- blocked-by:6.5 -->
  - [ ] `"fi"` with liga feature → 1 glyph (fi ligature), col_span=2 <!-- blocked-by:6.5 -->
  - [x] `"好"` (wide char) → 1 glyph, col_span=2 — requires CJK fallback font in test env
  - [x] CJK char → shaped from fallback face, correct face_idx — requires CJK fallback font in test env

---

## 6.5 Ligature + Multi-Cell Glyph Handling

Map shaped glyphs back to grid columns. Ligatures span multiple columns — only the first column renders the glyph.

**File:** `oriterm/src/gpu/render_grid.rs` (rendering integration)

- [x] Column → glyph mapping:
  - [x] `col_glyph_map: Vec<Option<usize>>` — maps column index → index in shaped glyphs vec
  - [x] For each shaped glyph: `col_glyph_map[glyph.col_start] = Some(glyph_index)`
  - [x] Subsequent columns of a ligature (col_start+1, col_start+2, ...) remain `None`
  - [x] During rendering: if `col_glyph_map[col]` is `Some(i)` → render glyph; if `None` → skip (continuation of ligature)
  - [x] Implemented via `ShapedFrame` col map + `fill_frame_shaped` in `gpu/prepare/mod.rs`
- [x] Ligature background:
  - [x] Background color for each column still rendered independently (cell-by-cell)
  - [x] Only the foreground glyph spans multiple columns
- [ ] Ligature + selection interaction: <!-- blocked-by:9 -->
  - [ ] If selection covers part of a ligature, still render the full glyph
  - [ ] Selection highlighting applies to individual cells (not whole ligature)
- [x] Ligature + cursor interaction:
  - [x] Cursor on a ligature column renders on top of the glyph
  - [x] Cursor rendering is per-cell, unaffected by glyph span
- [x] **Tests** (shaped rendering):
  - [x] Ligature: 2-col glyph → 2 bg instances, 1 fg instance (at col 0 only)
  - [x] Background independence: ligature cells get individual bg rects
  - [ ] Selection of col 1 of a ligature doesn't duplicate glyph <!-- blocked-by:9 -->
  - [x] Mixed ligature + non-ligature on same line renders correctly

---

## 6.6 Combining Marks + Zero-Width Characters

Handle combining diacritics, ZWJ sequences, and other zero-width characters.

**Files:** `oriterm_core/src/cell.rs` (storage), `oriterm/src/font/shaper.rs` (shaping)

- [x] Cell storage for zero-width characters:
  - [x] Add to `CellExtra`: `zerowidth: Vec<char>` — combining marks attached to this cell
  - [x] `Cell::push_zerowidth(&mut self, ch: char)` — add combining mark (lazy `Arc<CellExtra>` allocation)
  - [x] `Cell::zerowidth(&self) -> &[char]` — access via `cell.extra.zerowidth`
  - [x] Zero-width chars don't advance the cursor — they attach to the preceding cell
- [x] VTE handler integration:
  - [x] When `input(ch)` receives a character with `unicode_width == 0`:
    - [x] Don't advance cursor
    - [x] Push to previous cell's zerowidth list via `grid.push_zerowidth(c)`
  - [x] Grid backtracking handles wrap-pending state and wide-char spacers
- [x] Shaping integration:
  - [x] In `prepare_line()`: after appending base char, also append `cell.extra.zerowidth` chars to run text
  - [x] All zero-width chars get same column mapping as their base char
  - [x] Rustybuzz handles combining: base + accent → single positioned cluster
- [x] Rendering:
  - [x] Shaper produces multiple glyphs at same col_start (base + marks)
  - [x] Each glyph rendered with its own x_offset/y_offset from shaper
  - [x] Multiple glyphs at same column are all rendered via first-wins col map + forward iteration
  - [x] Implemented via `emit_shaped_glyphs` in `gpu/prepare/mod.rs`
- [x] **Tests**:
  - [x] `'e'` + `'\u{0301}'` (combining acute) → single shaping cluster at same column
  - [x] `'a'` + multiple combining marks → all stored in zerowidth vec
  - [x] ZWJ sequence: stored as base + zerowidth sequence
  - [x] Width: combining marks don't advance cursor (width 0)
  - [x] Edge cases: col 0 discard, wrap-pending, wide-char spacer backtrack

---

## 6.7 OpenType Feature Control

Collection-wide and per-fallback OpenType feature settings.

**File:** `oriterm/src/font/collection.rs` (continued)

- [x] Collection-wide features:
  - [x] `features: Vec<rustybuzz::Feature>` — applied to all primary faces
  - [x] Default: `["liga", "calt"]` (standard ligatures + contextual alternates)
  - [x] Parsed from config: `"liga"` → enable, `"-liga"` → disable (via `parse_features()`)
- [x] Per-fallback features:
  - [x] `FallbackMeta.features: Option<Vec<Feature>>` — overrides collection defaults for specific fallback
  - [x] Use case: disable ligatures for CJK fonts (`["-liga"]`)
- [x] `features_for_face(&self, face_idx: FaceIdx) -> &[rustybuzz::Feature]`
  - [x] Primary (0–3): return collection-wide features
  - [x] Fallback (4+): return fallback-specific features (or collection defaults if no override)
- [x] Feature parsing:
  - [x] `parse_features(input: &[&str]) -> Vec<rustybuzz::Feature>`
  - [x] `"liga"` → `Feature { tag: tag!("liga"), value: 1, start: 0, end: u32::MAX }`
  - [x] `"-dlig"` → `Feature { tag: tag!("dlig"), value: 0, start: 0, end: u32::MAX }`
- [x] Features passed to rustybuzz during shaping (`shape_run()` calls `features_for_face()`)
- [x] Config integration:
  ```toml
  [font]
  features = ["liga", "calt", "dlig"]
  ligatures = true  # Shorthand for liga + calt
  ```
- [x] **Tests**:
  - [x] Features parsed correctly: "liga" → value 1, "-liga" → value 0
  - [x] Collection features applied during shaping (features passed to `rustybuzz::shape()`)
  - [x] Fallback without override uses collection defaults
  - [x] Invalid feature tags skipped with warning
  - [x] Default features are liga + calt

---

## 6.8 Advanced Atlas (Guillotine + LRU + Multi-Page)

Replace Section 04's simple shelf packing with the production atlas: guillotine packing, 2D texture array, LRU eviction.

**File:** `oriterm/src/gpu/atlas.rs`

**Reference:** `_old/src/gpu/atlas.rs`

- [x] Guillotine rectangle packing:
  - [x] `RectPacker` struct (`gpu/atlas/rect_packer/mod.rs`)
    - [x] `free_rects: Vec<Rect>` — available rectangles
    - [x] `pack(w: u32, h: u32) -> Option<(u32, u32)>` — find best-short-side-fit
    - [x] Split: remove chosen rect, create up to 2 children (horizontal or vertical split based on leftover shape)
    - [x] Reset: clear to single full-page rect
  - [x] **Why guillotine over shelf?** Better packing density for mixed glyph sizes (CJK large + Latin small + accent tiny)
- [x] Multi-page texture array:
  - [x] `GlyphAtlas.texture: wgpu::Texture` — `Texture2DArray` format
  - [x] Page size: 2048×2048
  - [x] Max pages: 4 (= 16MB VRAM at R8Unorm)
  - [x] Start with 1 page, grow on demand up to max
  - [x] `pages: Vec<AtlasPage>` — per-page packing state + LRU frame counter
- [x] LRU eviction:
  - [x] Each page tracks `last_used_frame: u64`
  - [x] When all pages full and new glyph needs space:
    - [x] Find page with oldest `last_used_frame`
    - [x] Reset that page's packer
    - [x] Remove all cache entries pointing to that page
    - [x] Re-insert the new glyph on the now-empty page
- [x] Cache key: `RasterKey { glyph_id: u16, face_idx: FaceIdx, size_q6: u32 }`
  - [x] `size_q6 = (size * 64.0).round() as u32` — 26.6 fixed-point for precise DPI-aware keying
  - [x] **Why Q6?** Prevents rounding collisions at fractional DPI: 13.95pt vs 14.05pt get distinct keys
- [x] `get_or_insert(key, rasterize_fn, device, queue) -> Option<AtlasEntry>`
  - [x] Check cache HashMap
  - [x] If miss: call `rasterize_fn()` to get bitmap, upload to atlas, cache entry
  - [x] Update page's `last_used_frame` via `touch_page()`
  - [x] Return atlas entry (UV coordinates, metrics, page index)
- [x] `AtlasEntry` struct: `page`, `uv_x/y/w/h`, `width/height`, `bearing_x/y`
- [x] `begin_frame()` — increment frame counter (called from `GpuRenderer::prepare()`)
- [x] `clear()` — full atlas reset (called on font size change)
- [x] Shader + pipeline: `texture_2d_array`, 7 instance attributes (added `atlas_page`), `D2Array` bind layout
- [x] **Tests**:
  - [x] Guillotine packing: 50 varied-size rects with no-overlap verification
  - [x] Multi-page: fill page 0, overflow to page 1
  - [x] LRU eviction: fill all 4 pages, insert new glyph → oldest page evicted
  - [x] LRU eviction preserves newer pages when oldest is touched
  - [x] Cache hit: same key returns same entry
  - [x] Q6 keying: slightly different sizes produce different keys

---

## 6.9 Built-in Geometric Glyphs

Pixel-perfect rendering for box drawing, block elements, braille, and powerline glyphs. Bypasses the font pipeline entirely — generated as GPU rectangles.

**File:** `oriterm/src/gpu/builtin_glyphs.rs`

**Reference:** `_old/src/gpu/builtin_glyphs.rs`

- [x] `is_builtin(ch: char) -> bool` — fast check if character is handled by builtin system
- [x] Atlas-based rasterization: CPU → alpha bitmap → atlas → normal glyph pipeline
  - [x] `Canvas` struct for alpha bitmap drawing with `fill_rect`, `blend_pixel`, `fill_line` (SDF anti-aliased)
  - [x] `rasterize(ch, cell_w, cell_h) -> Option<RasterizedGlyph>` — dispatch to category
  - [x] `raster_key(ch, size_q6) -> RasterKey` — uses `FaceIdx::BUILTIN` sentinel
  - [x] `ensure_cached(input, size_q6, atlas, gpu)` — scan cells, rasterize + insert into atlas
- [x] **Box Drawing** (U+2500–U+257F):
  - [x] 128 characters, lookup table: `[left, right, up, down]` per char
  - [x] Values: 0=none, 1=light (thin), 2=heavy (thick), 3=double
  - [x] Render from cell center: horizontal segments left/right, vertical segments up/down
  - [x] Line thickness: thin = `max(1.0, round(cell_width / 8.0))`, heavy = `thin * 3.0`
  - [x] Double lines: two parallel lines with gap = `max(2.0, thin * 2.0)`
  - [x] Segments connect cleanly at cell boundaries (critical for box drawing to look right)
  - [x] Rounded corners (U+256D–U+2570): right-angle fallback
  - [x] Diagonals (U+2571–U+2573): anti-aliased via SDF line rendering
- [x] **Block Elements** (U+2580–U+259F):
  - [x] Full block `█` (U+2588): entire cell filled
  - [x] Upper half `▀` (U+2580): top half filled
  - [x] Lower N/8 blocks (U+2581–U+2587): fractional heights from bottom
  - [x] Left N/8 blocks (U+2589–U+258F): fractional widths from left
  - [x] Shade blocks: light `░` (25% alpha), medium `▒` (50%), dark `▓` (75%)
  - [x] Quadrant blocks (U+2596–U+259F): bitmask → fill selected quadrants
- [x] **Braille** (U+2800–U+28FF):
  - [x] 8-dot pattern in 2×4 grid
  - [x] Character value encodes which dots are filled (8-bit bitmask)
  - [x] Dot positions: 2 columns × 4 rows within cell
  - [x] Render as rectangles at fractional cell positions
- [x] **Powerline** (U+E0A0–U+E0D4):
  - [x] Right-pointing solid triangle (U+E0B0): filled triangle, scanline rendered
  - [x] Left-pointing solid triangle (U+E0B2): mirrored
  - [x] Right-pointing thin arrow (U+E0B1): outline only
  - [x] Left-pointing thin arrow (U+E0B3): mirrored outline
  - [x] Rounded separators (U+E0B4, U+E0B6): solid triangles
  - [x] Unrecognized powerline chars → return None (fall through to font)
- [x] Integration with rendering pipeline:
  - [x] `FaceIdx::BUILTIN` sentinel (u16::MAX) — reuses existing `RasterKey` type
  - [x] Builtins skipped in `segment_runs()` — not shaped through rustybuzz
  - [x] `ensure_cached()` phase between shaped glyph caching and prepare
  - [x] `fill_frame_shaped()` builtin branch: bypass bearing math, position at (x, y) directly
- [x] **Tests** (37 tests):
  - [x] `is_builtin()` range coverage and exclusion tests
  - [x] `raster_key` uses `FaceIdx::BUILTIN`
  - [x] Box drawing: horizontal line, vertical line, cross, double horizontal, rounded corner, diagonal (with AA)
  - [x] Block: full block, upper/lower half, right half, shades (25%/50%/75%)
  - [x] Braille: empty, single dot, six dots, all eight dots
  - [x] Powerline: right/left triangles, thin outline, unrecognized falls through
  - [x] Canvas: dimensions, fill_rect clipping, blend_pixel saturation, fill_line AA, glyph format

---

## 6.10 Color Emoji

Support for color emoji rendering (CBDT/CBLC bitmap emoji or COLR/CPAL outline emoji).

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/gpu/atlas.rs`

- [x] Rasterization with color source:
  - [x] Swash `Render::new(&[Source::ColorOutline, Source::ColorBitmap, Source::Outline])`
  - [x] `Format::Rgba` for color glyphs (4 bytes per pixel)
  - [x] `Format::Alpha` for non-color fallback (1 byte per pixel)
  - [x] Check render result: if RGBA → color glyph, if Alpha → normal glyph
- [x] Atlas support for color glyphs:
  - [x] Option A: Separate RGBA atlas (Rgba8Unorm texture) for color glyphs
  - [x] ~~Option B: Single atlas with mixed formats (more complex shader)~~ (not needed — Option A chosen)
  - [x] **Recommended: Option A** — separate atlas, separate pipeline pass
  - [x] Color atlas bind group separate from grayscale atlas
- [x] Rendering color glyphs:
  - [x] Color glyphs render with their own colors (not tinted by fg_color)
  - [x] Fragment shader: sample RGBA directly, blend with background
  - [x] No foreground color multiplication (unlike grayscale glyphs)
- [x] Emoji presentation:
  - [x] Characters like U+2764 (❤) can be text or emoji presentation
  - [x] VS15 (U+FE0E) forces text presentation
  - [x] VS16 (U+FE0F) forces emoji presentation
  - [x] Store variation selectors in cell's zerowidth list
  - [x] During face resolution: check for VS16 → prefer color emoji font
- [x] Fallback for emoji:
  - [x] Windows: Segoe UI Emoji
  - [x] Linux: Noto Color Emoji
  - [x] These should be high-priority in fallback chain for emoji codepoints
- [x] **Tests**:
  - [x] Emoji character rasterizes as RGBA bitmap
  - [x] Color glyph renders without fg tinting
  - [x] VS16 forces emoji presentation
  - [x] VS15 forces text presentation
  - [x] Emoji fallback resolves to color emoji font

---

## 6.11 Font Synthesis (Bold + Italic)

When a font lacks a Bold or Italic variant, synthesize it properly using swash's outline manipulation — not crude hacks like double-strike or missing-variant fallback-to-Regular.

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/font/rasterize.rs`

**Reference:** `_old/src/font/collection.rs` (weight_variation_for), Ghostty `src/font/face/freetype.zig` (embolden formula)

- [x] Variable font weight (preferred path):
  - [x] If font has `wght` axis: use font variations instead of separate Bold file
  - [x] `weight_variation_for(face_idx: FaceIdx, weight: u16) -> Option<f32>`
    - [x] Regular/Italic: use base weight (e.g., 400)
    - [x] Bold/BoldItalic: `min(weight + 300, 900)` — CSS "bolder" algorithm
    - [x] Fallbacks: `None` (use font's default weight)
  - [x] Pass to swash: `scale_ctx.builder(face).variations(&[("wght", value)])`
- [x] Synthetic bold via `Render::embolden(strength)` (when no real Bold and no wght axis):
  - [x] `embolden()` uniformly expands outlines before rasterization — strokes get thicker in all directions, not just a 1px horizontal shift
  - [x] Strength formula (from Ghostty): `(font_height_px * 64.0 / 2048.0).ceil()` — scales proportionally with font size so bold looks consistent at 8pt and 24pt
  - [x] Bounding box grows: adjust glyph metrics (bearing_x, width) to account for expansion so glyphs don't clip at cell edges
  - [x] Atlas key includes `synthetic_bold: bool` — emboldened glyphs cached separately from regular
- [x] Synthetic italic via `Render::transform(Transform::skew(14°, 0°))`:
  - [x] Standard 14-degree oblique angle (CSS spec, same as Ghostty and cosmic-text)
  - [x] `Transform::skew(Angle::from_degrees(14.0), Angle::from_degrees(0.0))`
  - [x] Applied when cell has ITALIC flag but face lacks real italic variant
  - [x] Atlas key includes `synthetic_italic: bool` — skewed glyphs cached separately
- [x] Use swash `Synthesis` for automatic detection:
  - [x] `font_attributes.synthesize(requested_attributes) -> Synthesis`
  - [x] `synthesis.embolden()` → apply embolden
  - [x] `synthesis.skew()` → apply transform with returned angle
  - [x] `synthesis.variations()` → apply weight/width settings
- [x] Synthesis combinations:
  - [x] BoldItalic with no variant: apply BOTH embolden and skew simultaneously
  - [x] Order: variations first, then embolden, then transform (swash applies in render order)
- [x] **Tests**:
  - [x] Variable font: weight variation applied (wght=700 produces thicker strokes)
  - [x] Synthetic bold: emboldened glyph is wider than regular (measure rasterized bitmap)
  - [x] Synthetic italic: skewed glyph has non-zero horizontal displacement
  - [x] Combined bold+italic: both embolden and skew applied
  - [x] Regular cells: no synthesis applied
  - [x] Synthesis detection: `Synthesis::any()` returns true only when variant is missing

---

## 6.12 Text Decorations

All underline styles, strikethrough, hyperlink underline, URL hover underline.

**File:** `oriterm/src/gpu/render_grid.rs`

**Reference:** `_old/src/gpu/render_grid.rs` (underline/strikethrough sections)

- [x] **Single underline** (CellFlags::UNDERLINE):
  - [x] Solid line at `y = cell_bottom - 2px`, thickness = 1px
  - [x] Spans cell width
- [x] **Double underline** (CellFlags::DOUBLE_UNDERLINE):
  - [x] Two solid lines: `y = cell_bottom - 2px` and `y = cell_bottom - 4px`
- [x] **Curly underline** (CellFlags::CURLY_UNDERLINE):
  - [x] Sine wave: `y = base_y + amplitude * sin(x * freq)`
  - [x] Rendered as a sequence of short horizontal rectangles (1px tall) at computed y positions
  - [x] Amplitude: ~2px, frequency: ~2π per cell_width
- [x] **Dotted underline** (CellFlags::DOTTED_UNDERLINE):
  - [x] Alternating 1px on, 1px off pattern
  - [x] Phase reset at start of each cell
- [x] **Dashed underline** (CellFlags::DASHED_UNDERLINE):
  - [x] 3px on, 2px off pattern
- [x] **Underline color** (SGR 58):
  - [x] `cell.underline_color` — resolved in extract phase
  - [x] If present: use this color for underline
  - [x] If absent: use foreground color
- [x] **Strikethrough** (CellFlags::STRIKETHROUGH):
  - [x] Solid line at `y = cell_top + cell_height / 2`, thickness = 1px
  - [x] Color: foreground color
- [x] **Hyperlink underline** (cell has hyperlink via OSC 8): <!-- blocked-by:10 -->
  - [x] Dotted underline when not hovered
  - [x] Solid underline when hovered (cursor over cell)
  - [x] Color: foreground color (or a distinct link color)
- [ ] **URL hover underline** (implicitly detected URL): <!-- blocked-by:14 -->
  - [ ] Solid underline on hover
  - [ ] Only visible when Ctrl held + mouse over URL range
- [x] All decorations emit background-layer instances (opaque rectangles)
- [x] **Tests**:
  - [x] Single underline: 1px line at correct y
  - [x] Curly underline: wave shape (per-pixel sine rects)
  - [x] Dotted: alternating pattern
  - [x] Underline color: uses SGR 58 color when set
  - [x] Strikethrough: centered horizontally
  - [x] Double underline: two rects at correct positions
  - [x] Dashed underline: 3-on-2-off pattern
  - [x] Underline and strikethrough coexist
  - [x] No flags: no extra decoration rects
  - [x] Wide char: underline spans double width
  - [x] Fg color fallback: underline uses fg when no SGR 58

---

## 6.13 UI Text Shaping

Shape non-grid text (tab bar titles, search bar, status text) through rustybuzz without grid-column mapping.

**File:** `oriterm/src/font/shaper.rs` (additional function)

**Reference:** `_old/src/font/shaper.rs` (shape_text_string)

- [x] `UiShapedGlyph` struct
  - [x] Fields:
    - `glyph_id: u16`
    - `face_idx: FaceIdx`
    - `x_advance: f32` — absolute pixel advance (for cursor positioning)
    - `x_offset: f32`
    - `y_offset: f32`
  - [x] No `col_start` / `col_span` — UI text is free-positioned, not grid-locked
- [x] `shape_text_string(text: &str, faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<UiShapedGlyph>)`
  - [x] Segment text into runs by font face (same as grid shaping)
  - [x] Shape through rustybuzz
  - [x] Emit glyphs with absolute x_advance (sum of advances = total text width)
  - [x] Spaces: emit as advance-only (glyph_id = 0, advance = space width)
- [x] `measure_text(text: &str, collection: &FontCollection) -> f32`
  - [x] Sum x_advances for all glyphs → total pixel width
  - [x] Used for tab bar layout, text truncation, centering
- [x] Text truncation with ellipsis:
  - [x] If text width > available width: truncate and append `…` (U+2026)
  - [x] Cell-width-based truncation (exact for monospace)
- [ ] Integration with tab bar and search bar rendering: <!-- blocked-by:16 --><!-- blocked-by:11 -->
  - [ ] Tab title → `shape_text_string` → glyph instances
  - [ ] Search query → `shape_text_string` → glyph instances
- [x] **Tests**:
  - [x] "Hello" → 5 glyphs with sequential advances
  - [x] Measure text returns correct total width
  - [x] Truncation: long text gets ellipsis at correct position

---

## 6.14 Pre-Caching + Performance

Eliminate first-frame stalls and optimize per-frame costs.

- [x] Pre-cache ASCII (0x20–0x7E) at font load time:
  - [x] Rasterize all printable ASCII for Regular style
  - [x] Insert into atlas immediately
  - [x] First frame renders without any rasterization stalls
- [x] Pre-cache bold ASCII if bold face available
- [x] Scratch buffer reuse:
  - [x] `runs_scratch: Vec<ShapingRun>` — cleared + reused per row (not reallocated)
  - [x] `shaped_scratch: Vec<ShapedGlyph>` — same pattern
  - [x] `col_glyph_map: Vec<Option<usize>>` — same pattern
  - [x] Allocated once at max expected size, never shrink
- [x] Face creation once per frame:
  - [x] `create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face>>` — creates Face references from FaceData
  - [x] Called once at start of frame, reused for all rows
  - [x] Faces borrow from `Arc<Vec<u8>>` in FaceData (zero-copy)
- [x] Font size change:
  - [x] Clear entire atlas
  - [x] Recompute cell metrics
  - [x] Re-pre-cache ASCII
  - [x] Invalidate all cached frame data
- [x] **Performance targets**:
  - [x] Shaping: < 2ms per frame for 80×24 terminal
  - [x] Atlas miss (new glyph): < 0.5ms per glyph (rasterize + upload)
  - [x] Atlas hit: HashMap lookup only (< 1μs)
  - [x] No allocation in per-cell rendering loop

---

## 6.15 Hinting

Control over glyph hinting — the grid-fitting process that snaps outlines to pixel boundaries for sharper rendering at small sizes. This is the single biggest visual quality factor on non-HiDPI displays (still the majority of monitors in use).

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/font/collection.rs`

**Reference:** WezTerm `wezterm-font/src/ftwrap.rs` (load targets), Ghostty `src/font/face/freetype.zig` (hinting flags)

- [x] Hinting mode enum:
  ```rust
  pub enum HintingMode {
      /// Full hinting (snaps to pixel grid). Crispest text on non-HiDPI.
      Full,
      /// No hinting (preserves outline shape). Best on HiDPI (2x+) where
      /// subpixel precision isn't needed for sharpness.
      None,
  }
  ```
  - [x] swash only supports `.hint(bool)` — no "light" mode. Two modes is honest.
- [x] Auto-detection based on display scale:
  - [x] `scale_factor < 2.0` → `HintingMode::Full` (non-HiDPI needs grid-fitting)
  - [x] `scale_factor >= 2.0` → `HintingMode::None` (Retina/4K has enough pixels)
  - [x] Re-evaluate on `ScaleFactorChanged` events
- [x] User override via config:
  ```toml
  [font]
  hinting = "full"  # or "none"
  ```
  - [x] Config value overrides auto-detection
- [x] Integration with rasterization:
  - [x] `ScalerBuilder::hint(mode == HintingMode::Full)` applied when building scaler
  - [x] Hinted glyphs produce different bitmaps — atlas key must include hinting state
  - [x] `RasterKey` expanded: add `hinted: bool` field
- [x] Grid-fitted cell metrics:
  - [x] When hinting is Full: compute cell_width/cell_height from hinted advances — swash metrics use `.ceil()` which provides integer snapping; no separate "hinted metrics" API in swash
  - [x] When hinting is None: use unhinted metric (floating-point, rounded)
  - [x] Hinted metrics are more consistent across glyphs (less cumulative rounding error)
- [x] Font size change or hinting mode change:
  - [x] Clear entire atlas (all cached glyphs are now wrong)
  - [x] Recompute cell metrics
  - [x] Re-pre-cache ASCII
- [x] **Tests**:
  - [x] Hinted glyph bitmap differs from unhinted at same size
  - [x] Auto-detection: scale 1.0 → Full, scale 2.0 → None
  - [x] Config override: explicit "none" at scale 1.0 disables hinting
  - [x] Atlas invalidated on hinting mode change

---

## 6.16 Subpixel Rendering (LCD)

LCD subpixel rendering uses the physical R/G/B subpixels of the display to achieve ~3x effective horizontal resolution. This is what makes ClearType text on Windows look sharp and what macOS used before Retina displays made it unnecessary. No GPU terminal except WezTerm implements this, and WezTerm's is buggy. Getting it right is a headline differentiator.

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/gpu/pipeline.rs` (shader), `oriterm/src/gpu/atlas.rs`

**Reference:** WezTerm `wezterm-font/src/rasterizer/freetype.rs` (LCD rendering), WezTerm `wezterm-gui/src/glyph-frag.glsl` (dual-source blending)

- [x] Subpixel rasterization via swash:
  - [x] `Render::format(Format::Subpixel)` — produces RGBA subpixel mask
  - [x] Output: 4 bytes per pixel. R/G/B channels contain per-subpixel coverage. A channel contains overall coverage.
  - [x] `Format::Subpixel` uses standard RGB subpixel order (1/3 pixel offsets for R and B)
  - [x] `zeno::Format::subpixel_bgra()` for BGR panel layouts
  - [x] `Content::SubpixelMask` indicates subpixel output (vs `Content::Mask` for grayscale)
- [x] Pixel geometry detection and configuration:
  ```toml
  [font]
  lcd_filter = "rgb"   # "rgb", "bgr", or "none" (disable subpixel)
  ```
  - [x] Default: `"rgb"` (vast majority of displays)
  - [x] `"none"` falls back to grayscale alpha rendering
  - [x] Auto-disable on HiDPI (scale >= 2.0) — Retina displays don't have visible subpixels
- [x] Separate atlas storage for subpixel glyphs:
  - [x] Grayscale glyphs: `R8Unorm` texture (1 byte/pixel) — existing path
  - [x] Subpixel glyphs: `Rgba8Unorm` texture (4 bytes/pixel) — new
  - [x] Atlas tracks per-entry format: `AtlasEntry.subpixel: bool`
  - [x] Color emoji always rasterized as `Format::Alpha` → `Rgba8Unorm` (no subpixel for bitmaps)
- [x] Shader changes for per-channel alpha blending:
  - [x] Grayscale path (existing): `output.rgb = fg_color.rgb; output.a = texture_sample.r * fg_color.a`
  - [x] Subpixel path (new): each color channel blended independently:
    ```wgsl
    let mask = textureSample(atlas, sampler, uv);  // RGBA subpixel mask
    output.r = mix(bg.r, fg.r, mask.r);
    output.g = mix(bg.g, fg.g, mask.g);
    output.b = mix(bg.b, fg.b, mask.b);
    output.a = max(mask.r, max(mask.g, mask.b));
    ```
  - [x] Alternative: dual-source blending (WezTerm approach) — more correct but requires `DUAL_SOURCE_BLENDING` wgpu feature
  - [x] Decision: start with the `mix()` approach (simpler, no feature requirement), upgrade to dual-source if quality demands it
- [x] Rendering pipeline integration:
  - [x] Subpixel glyphs need the background color at render time (for the `mix()`)
  - [x] Two approaches:
    - [x] **Read-back**: sample the current framebuffer at the glyph position (expensive)
    - [x] **Pass bg_color as uniform/instance data**: the Prepare phase already computes bg_color per cell — pass it to the fg shader as an instance attribute
  - [x] **Recommended**: pass bg_color in the fg instance buffer (add 4 bytes per glyph instance)
- [ ] Interaction with other features:
  - [x] Transparent backgrounds: subpixel rendering over transparency produces color fringing — fall back to grayscale alpha for cells with non-opaque backgrounds
  - [ ] Selection highlighting: when selection inverts colors, subpixel glyphs must be re-resolved or fall back to grayscale <!-- blocked-by:9 -->
  - [x] Color emoji: always grayscale alpha path (no subpixel for pre-colored bitmaps)
- [x] **Tests**:
  - [x] Subpixel-rasterized glyph has wider bitmap than grayscale equivalent (3x horizontal)
  - [x] RGB vs BGR: channel order swapped correctly
  - [x] Auto-disable: scale 2.0+ renders grayscale
  - [x] Config "none": forces grayscale regardless of scale
  - [x] Transparent cell: falls back to grayscale (no color fringing)
  - [x] Visual regression: compare subpixel vs grayscale rendering of reference string

---

## 6.17 Subpixel Glyph Positioning

Render glyphs at fractional pixel offsets for tighter, more natural spacing. Most visible in UI text (tab titles, search bar) and for combining marks / shaper offsets in grid text. No terminal currently does this.

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/gpu/atlas.rs`

**Reference:** cosmic-text `src/swash.rs` (x_bin/y_bin cache key)

- [x] Fractional offset via swash:
  - [x] `Render::offset(Vector::new(x_fract, y_fract))` — shifts rasterization grid
  - [x] A glyph at x=10.3 is rasterized with `offset(0.3, 0.0)` and placed at integer x=10
  - [x] The rasterizer produces a slightly different bitmap for each fractional offset — the anti-aliasing pattern shifts to represent the true sub-pixel position
- [x] Quantization to reduce atlas explosion:
  - [x] 4 horizontal phases: 0.00, 0.25, 0.50, 0.75 (snap fractional part to nearest quarter)
  - [x] 1 vertical phase: 0.0 only (vertical subpixel rarely matters for horizontal text)
  - [x] `subpx_bin(fract: f32) -> u8` — returns 0, 1, 2, or 3
  - [x] 4x atlas entries per glyph shape (acceptable — most grid text hits phase 0)
- [x] Atlas key expansion:
  - [x] Add `subpx_x: u8` (0–3) to `RasterKey`
  - [x] Grid text at integer cell boundaries → always phase 0 (no extra atlas entries)
  - [x] Shaper x_offset/y_offset → quantized to nearest phase
  - [x] UI text → free-positioned, each glyph may hit any phase
- [x] Where subpixel positioning applies:
  - [x] **UI text** (tab titles, search bar, overlays): full subpixel positioning — glyphs placed at fractional advances from shaper. Biggest visual improvement.
  - [x] **Combining marks**: shaper x_offset/y_offset are fractional — rasterize at correct subpixel offset for precise diacritic placement
  - [x] **Ligature internals**: multi-glyph ligatures may have fractional internal offsets
  - [x] **Base grid text**: integer cell boundaries → phase 0. No extra atlas cost.
- [x] Config:
  ```toml
  [font]
  subpixel_positioning = true  # default true; false snaps everything to integer
  ```
- [x] **Tests**:
  - [x] Phase 0 and phase 2 (0.5) produce different bitmaps for same glyph
  - [x] Quantization: 0.13 → phase 0, 0.37 → phase 1, 0.62 → phase 2, 0.88 → phase 3
  - [x] Grid text at integer position: always phase 0
  - [x] UI text: mixed phases across a shaped string
  - [x] Atlas key differs by subpx_x: cache stores separate entries

---

## 6.18 Visual Regression Testing

Automated pixel-level comparison of rendered text against golden reference images. This is what prevents regressions and validates that the font pipeline produces correct output across all character types, sizes, and DPI scales. Infrastructure investment that pays compound interest.

**File:** `oriterm/tests/visual/`, `oriterm/src/gpu/renderer/mod.rs` (headless rendering)

**Reference:** Rio/Sugarloaf `sugarloaf/tests/util/image.rs` (FLIP algorithm), Ghostty `test/cases/` (golden PNGs)

- [x] Headless rendering infrastructure:
  - [x] `GpuState::new_headless()` already exists — extended with `headless_env()` and `headless_env_with_config()`
  - [x] `render_to_pixels()` renders through full pipeline: shaping → atlas → GPU → pixel readback
  - [x] Returns RGBA pixel buffer for comparison
  - [x] Uses `FontSet::embedded()` for deterministic output regardless of system fonts
- [x] Golden image management:
  - [x] Store reference PNGs in `oriterm/tests/references/`
  - [x] Naming convention: `{test_name}.png` (size/DPI tests include params in name)
  - [x] Generated once, checked into git, reviewed on changes
  - [x] `ORITERM_UPDATE_GOLDEN=1 cargo test` regenerates golden images
- [x] Comparison algorithm:
  - [x] **Per-pixel difference**: compute per-channel absolute difference
  - [x] **Tolerance threshold**: allow ±2 per channel (anti-aliasing can vary by platform/driver)
  - [x] **Percentage threshold**: pass if ≥ 99.5% of pixels match within tolerance (`MAX_MISMATCH_PERCENT = 0.5`)
  - [x] On failure: write `{test_name}_actual.png` and `{test_name}_diff.png` for visual inspection
  - [x] Diff image: highlight mismatched pixels in red
- [x] Reference test strings (each becomes a golden image test):
  - [x] `ascii_regular`: `"The quick brown fox jumps over the lazy dog 0123456789"` — baseline Latin
  - [x] `ascii_bold_italic`: same string with Regular/Bold/Italic/BoldItalic rows — synthesis quality
  - [x] `ligatures`: `"=> -> != === !== >= <= |> <| :: <<"` — ligature shaping
  - [x] `box_drawing`: `┌─┬─┐` / `│ │ │` / `├─┼─┤` / `└─┴─┘` — pixel-perfect connections
  - [x] `block_elements`: `"█▓▒░▀▄▌▐▖▗▘▝▚▞"` — block element coverage
  - [x] `braille`: `"⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏⣿⠿⡿⢿"` — braille pattern correctness
  - [x] `cjk_notdef`: `"Hello你好世界"` with WIDE_CHAR flags — validates .notdef handling with embedded font
  - [x] `combining_marks`: base chars with combining acute, tilde, diaeresis, macron — diacritic positioning
  - [x] `powerline`: `\u{E0B0}` `\u{E0B1}` `\u{E0B2}` `\u{E0B3}` — powerline glyph shapes
  - [x] `mixed_styles`: Normal/Bold/Italic/BoldItalic interleaved in single row — variant switching
- [x] Multi-size testing:
  - [x] Test at 10pt, 14pt, 20pt at 96 DPI — catches size-dependent hinting/rounding issues
  - [x] Test at 14pt with 96 DPI and 192 DPI — catches HiDPI rendering differences
- [x] Integration with CI:
  - [x] `cargo test -p oriterm --target x86_64-pc-windows-gnu -- --ignored visual_regression` runs all golden image tests
  - [x] Tests skip gracefully if no GPU available (returns early with message)
  - [x] Font dependency: tests use `FontSet::embedded()` (JetBrains Mono Regular) for deterministic results
- [x] **Tests** (meta — testing the testing framework):
  - [x] Identical images: comparison passes (zero mismatches)
  - [x] 1-pixel-off images: comparison passes (within ±2 tolerance)
  - [x] Visually different images: all pixels differ, diff image is all red
  - [x] Percentage threshold: small differences within 0.5% threshold pass
  - [x] Missing golden: reference created and test passes

---

## 6.19 Variable Font Axes

Support variable fonts with configurable axis values (weight, width, slant, etc.) instead of requiring separate font files per style.

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/font/rasterize.rs`

**Reference:** Ghostty `font-variation-*` config, CSS `font-variation-settings`

- [x] Variable font axis detection:
  - [x] Query font for available axes: `wght` (weight), `wdth` (width), `slnt` (slant), `ital` (italic), custom axes
  - [x] Store discovered axes in `FaceData`: `axes: Vec<AxisInfo>` — tag, min, default, max
  - [x] `has_axis(axes, tag) -> bool` — check if font supports an axis
  - [x] `clamp_to_axis(axes, tag, value) -> f32` — clamp to axis range
- [x] Config integration:
  ```toml
  [font]
  variations = { wght = 450, wdth = 87.5 }  # per-axis values
  ```
  - [x] Parse `variations: HashMap<String, f32>` from TOML
  - [x] Validate values against axis min/max ranges
  - [x] Clamp out-of-range values with warning log
- [x] Rasterization with variations:
  - [x] Pass variations to swash: `scale_ctx.builder(face).variations(&[(tag, value), ...])`
  - [x] Variable weight replaces synthetic bold when `wght` axis available
  - [x] Variable slant replaces synthetic italic when `slnt` or `ital` axis available
  - [x] Synthesis detection: prefer real axis over synthetic when available
  - [x] `face_variations()` computes settings + suppression flags from face index, synthetic flags, weight, and axes
- [x] Per-style variation overrides:
  - [x] Regular: use base weight
  - [x] Bold: `wght = min(base_wght + 300, axis_max)`
  - [x] Italic: `slnt = -12` or `ital = 1` (preference: slnt over ital)
  - [x] BoldItalic: combine bold weight + italic slant
- [x] Atlas key implicitly covers variations (deterministic from face_idx + synthetic + weight, cache cleared on set_size)
- [x] **Tests:**
  - [x] Axis clamping: value beyond min/max clamped correctly
  - [x] Bold derivation: `wght + 300` capped at axis maximum
  - [x] Synthetic bold/italic suppression when axis exists
  - [x] Non-variable font: variations ignored gracefully, synthesis still works
  - [x] Fallback faces: empty variations
  - [x] slnt preferred over ital axis
  - [x] Config roundtrip: variations parsed from TOML correctly

---

## 6.20 Font Codepoint Mapping

Force specific Unicode ranges to render with specific fonts, overriding the normal fallback chain.

**File:** `oriterm/src/font/collection.rs`

**Reference:** Ghostty `font-codepoint-map` config

- [x] `CodepointMap` struct:
  - [x] `ranges: Vec<(RangeInclusive<u32>, FaceIdx)>` — codepoint range → font face
  - [x] Sorted by range start for binary search lookup
- [x] Config integration:
  ```toml
  [[font.codepoint_map]]
  range = "E000-F8FF"          # Private Use Area (Nerd Font symbols)
  family = "Symbols Nerd Font"

  [[font.codepoint_map]]
  range = "4E00-9FFF"          # CJK Unified Ideographs
  family = "Noto Sans CJK SC"
  ```
  - [x] Parse range as hex: `"E000-F8FF"` → `0xE000..=0xF8FF`
  - [x] Single codepoint: `"E0B0"` → `0xE0B0..=0xE0B0`
  - [x] Load referenced font family at collection init time
- [x] Integration with `find_face_for_char`:
  - [x] Check codepoint map FIRST, before primary and fallback chain
  - [x] If mapped: return mapped face directly (skip normal resolution)
  - [x] If not mapped: fall through to normal primary → fallback chain
- [x] Use cases:
  - [x] Force Nerd Font symbols to a specific Nerd Font (avoids wrong font picking up PUA)
  - [x] Force CJK to a specific CJK font (avoids system choosing wrong variant)
  - [x] Force emoji to a specific emoji font
- [x] **Tests:**
  - [x] Mapped codepoint resolves to configured font
  - [x] Unmapped codepoint falls through to normal chain
  - [x] Range parsing: hex range, single codepoint
  - [x] Multiple maps: first matching range wins
  - [x] Invalid font family: warning logged, fallback to normal chain

---

## 6.21 Section Completion

- [ ] All 6.1–6.20 items complete
- [ ] Full font pipeline: multi-face, fallback chain, cap-height normalization
- [ ] Rustybuzz shaping: ligatures, combining marks, OpenType features
- [ ] Advanced atlas: guillotine packing, multi-page, LRU eviction, Q6 keying
- [ ] Built-in glyphs: box drawing, blocks, braille, powerline — pixel-perfect
- [ ] Color emoji: RGBA atlas, correct rendering without fg tinting
- [ ] Font synthesis: proper embolden (outline expansion) + proper italic (14° skew) — no crude hacks
- [ ] Hinting: auto-detected by DPI, user-overridable, atlas-aware
- [ ] Subpixel rendering (LCD): per-channel alpha blending, RGB/BGR support, auto-disabled on HiDPI
- [ ] Subpixel glyph positioning: fractional offsets for UI text and combining marks
- [ ] All text decorations: single, double, curly, dotted, dashed underline + strikethrough
- [x] UI text shaping: tab bar titles, search bar, measure + truncate
- [ ] Pre-caching: no first-frame stall for ASCII
- [ ] Visual regression suite: golden image tests for all character types, sizes, and DPI scales
- [ ] **Visual tests** (automated via golden images):
  - [ ] Ligatures: `=>`, `->`, `!=` render as single glyphs
  - [ ] Box drawing: connected borders, pixel-perfect at all sizes
  - [ ] Braille: correct dot patterns
  - [ ] Powerline: triangle shapes render correctly
  - [ ] CJK: cap-height normalized, visually consistent with Latin text
  - [ ] Emoji: rendered in color, correct size
  - [ ] Combining marks: correctly positioned diacritics
  - [ ] Synthetic bold: visually heavier than regular, no clipping
  - [ ] Synthetic italic: 14° oblique, no artifacts
  - [ ] Hinted vs unhinted: both produce clean output at their target DPIs
  - [ ] Subpixel LCD: visibly sharper than grayscale on 1x displays
- [ ] `./clippy-all.sh` — no warnings
- [ ] `./test-all.sh` — all tests pass including visual regression suite
- [ ] `./build-all.sh` — cross-compilation succeeds

**Exit Criteria:** Font rendering is best-in-class — not just feature-complete but visually superior. Every character type renders correctly. Hinting produces crisp text on 1080p. LCD subpixel rendering provides measurably sharper text than any competing GPU terminal. Font synthesis (bold/italic) is indistinguishable from real variants at normal reading distance. Visual regression tests prevent quality regressions. This is the feature users switch terminals for.
