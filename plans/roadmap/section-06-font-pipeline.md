---
section: 6
title: Font Pipeline + Best-in-Class Glyph Rendering
status: not-started
tier: 2
goal: "Best font rendering of any terminal emulator. Full shaping pipeline with hinting, LCD subpixel rendering, subpixel positioning, proper font synthesis, and automated visual regression testing. The feature users switch terminals for."
sections:
  - id: "6.1"
    title: Multi-Face Font Loading
    status: not-started
  - id: "6.2"
    title: Fallback Chain + Cap-Height Normalization
    status: not-started
  - id: "6.3"
    title: Run Segmentation
    status: not-started
  - id: "6.4"
    title: Rustybuzz Text Shaping
    status: not-started
  - id: "6.5"
    title: Ligature + Multi-Cell Glyph Handling
    status: not-started
  - id: "6.6"
    title: Combining Marks + Zero-Width Characters
    status: not-started
  - id: "6.7"
    title: OpenType Feature Control
    status: not-started
  - id: "6.8"
    title: Advanced Atlas (Guillotine + LRU + Multi-Page)
    status: not-started
  - id: "6.9"
    title: Built-in Geometric Glyphs
    status: not-started
  - id: "6.10"
    title: Color Emoji
    status: not-started
  - id: "6.11"
    title: Font Synthesis (Bold + Italic)
    status: not-started
  - id: "6.12"
    title: Text Decorations
    status: not-started
  - id: "6.13"
    title: UI Text Shaping
    status: not-started
  - id: "6.14"
    title: Pre-Caching + Performance
    status: not-started
  - id: "6.15"
    title: Hinting
    status: not-started
  - id: "6.16"
    title: Subpixel Rendering (LCD)
    status: not-started
  - id: "6.17"
    title: Subpixel Glyph Positioning
    status: not-started
  - id: "6.18"
    title: Visual Regression Testing
    status: not-started
  - id: "6.19"
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

- [ ] `FaceData` struct
  - [ ] Fields:
    - `bytes: Arc<Vec<u8>>` — raw font file bytes (shared across variants from same file)
    - `face_index: u32` — index within .ttc collection
    - `offset: u32` — byte offset to font table directory
    - `cache_key: swash::CacheKey` — swash cache identifier
- [ ] `FaceIdx` newtype — `pub struct FaceIdx(pub u16)`
  - [ ] 0–3: primary styles (Regular=0, Bold=1, Italic=2, BoldItalic=3)
  - [ ] 4+: fallback fonts in priority order
- [ ] `FontCollection` expanded fields:
  - [ ] `primary: [Option<FaceData>; 4]` — Regular, Bold, Italic, BoldItalic
  - [ ] `has_variant: [bool; 4]` — true = real font file, false = fallback to Regular
  - [ ] `font_paths: [Option<PathBuf>; 4]`
  - [ ] `weight: u16` — CSS weight (100–900, default 400)
- [ ] Loading pipeline:
  - [ ] Load Regular (required — fail if missing)
  - [ ] Try loading Bold, Italic, BoldItalic from same family
  - [ ] If variant not found: `has_variant[i] = false` (will use Regular + synthetic styling)
  - [ ] Compute cell metrics from Regular face (cell_width from 'M' advance, cell_height from ascent + descent)
- [ ] Platform discovery (`font/discovery.rs`):
  - [ ] Windows (dwrote): enumerate via DirectWrite API by family name
  - [ ] Linux: scan `~/.local/share/fonts/`, `/usr/share/fonts/`, `/usr/local/share/fonts/`
  - [ ] Family search order: user-configured > JetBrains Mono > Cascadia Code > Consolas > Courier New
- [ ] `find_face_for_char(&self, ch: char, preferred_style: GlyphStyle) -> Option<FaceIdx>`
  - [ ] Try preferred style in primary
  - [ ] Fall back to Regular in primary
  - [ ] Fall back through fallback chain
  - [ ] Return None only if .notdef everywhere
- [ ] **Tests**:
  - [ ] Load a system font, all 4 variants attempted
  - [ ] `find_face_for_char('A', Bold)` returns Bold face if available
  - [ ] `find_face_for_char('A', Bold)` returns Regular if no Bold face
  - [ ] Unknown char falls to fallback chain

---

## 6.2 Fallback Chain + Cap-Height Normalization

Fallback fonts for characters missing from the primary (CJK, symbols, emoji). Visual consistency via cap-height normalization.

**File:** `oriterm/src/font/collection.rs` (continued)

**Reference:** `_old/src/font/collection.rs` (cap_height_px, FallbackMeta)

- [ ] `FallbackMeta` struct
  - [ ] Fields:
    - `features: Vec<rustybuzz::Feature>` — per-fallback OpenType features (override collection defaults)
    - `scale_factor: f32` — cap-height normalization ratio
    - `size_offset: f32` — user-configured size offset in points
- [ ] Fallback loading:
  - [ ] `fallbacks: Vec<FaceData>` — priority-ordered fallback fonts
  - [ ] `fallback_meta: Vec<FallbackMeta>` — per-fallback metadata (1:1 with fallbacks)
  - [ ] User-configured fallbacks loaded first (from config TOML)
  - [ ] System-discovered fallbacks loaded after
  - [ ] Lazy loading: `ensure_fallbacks_loaded()` called once on first use
- [ ] Cap-height normalization:
  - [ ] `cap_height_px(bytes, face_index, size) -> f32`
    - [ ] Read OS/2 table `sCapHeight` field via rustybuzz Face
    - [ ] If missing: estimate as `ascender * 0.75`
    - [ ] Convert from font units: `cap_units / upem * size`
  - [ ] `primary_cap_height_px: f32` — computed from Regular at load time
  - [ ] Per-fallback: `scale_factor = primary_cap_height / fallback_cap_height`
  - [ ] Effective size: `base_size * scale_factor + size_offset`
  - [ ] **Why:** Noto Sans CJK looks tiny next to JetBrains Mono at same pt size. Normalizing by cap-height makes glyphs visually consistent.
- [ ] `effective_size(&self, face_idx: FaceIdx) -> f32`
  - [ ] Primary faces: base size
  - [ ] Fallback faces: `base_size * meta.scale_factor + meta.size_offset`
- [ ] User-configurable per-fallback:
  ```toml
  [[font.fallback]]
  family = "Noto Sans CJK"
  features = ["-liga"]
  size_offset = -2.0
  ```
- [ ] **Tests**:
  - [ ] Fallback chain resolves CJK char to CJK font
  - [ ] Cap-height scale factor computed correctly (known font pair)
  - [ ] Effective size for fallback differs from primary
  - [ ] User size_offset applied

---

## 6.3 Run Segmentation

Break a terminal row into shaping runs. Each run is a contiguous sequence of characters that can be shaped together (same font face, no breaks).

**File:** `oriterm/src/font/shaper.rs`

**Reference:** `_old/src/font/shaper.rs` (prepare_line)

- [ ] `ShapingRun` struct
  - [ ] Fields:
    - `text: String` — base characters + combining marks for this run
    - `face_idx: FaceIdx` — which font face to shape with
    - `col_start: usize` — grid column where run starts
    - `byte_to_col: Vec<usize>` — maps byte offset in `text` → grid column
  - [ ] byte_to_col is critical for mapping rustybuzz cluster indices back to grid positions
- [ ] `prepare_line(row: &[Cell], cols: usize, collection: &FontCollection, runs: &mut Vec<ShapingRun>)`
  - [ ] Iterate cells left to right
  - [ ] Skip `WIDE_CHAR_SPACER` cells (they're part of the preceding wide char)
  - [ ] For each cell:
    - [ ] Determine face via `find_face_for_char(cell.ch, style_from_flags(cell.flags))`
    - [ ] If face differs from current run, or cell is space/null/builtin: start new run
    - [ ] Append `cell.ch` to current run's text
    - [ ] Record byte offset → column mapping
    - [ ] Append zero-width characters (combining marks) from cell at same column mapping
  - [ ] Run breaks on:
    - [ ] Space (' ') or null ('\0')
    - [ ] Font face change (different glyph found in different face)
    - [ ] Built-in glyph character (box drawing, blocks, braille, powerline)
    - [ ] Wide char spacer
  - [ ] Runs reuse a scratch `Vec<ShapingRun>` (cleared + refilled each frame, not reallocated)
- [ ] **Tests**:
  - [ ] `"hello world"` → two runs: "hello" (face 0), "world" (face 0) — space breaks runs
  - [ ] `"hello你好"` → two runs if CJK resolves to different face
  - [ ] `"a\u{0301}"` (a + combining accent) → single run with "á" text, byte_to_col maps both to same column
  - [ ] `"━"` (box drawing) → no run (handled by builtin glyph system)

---

## 6.4 Rustybuzz Text Shaping

Shape each run through rustybuzz to produce positioned glyphs with correct ligature substitution.

**File:** `oriterm/src/font/shaper.rs` (continued)

**Reference:** `_old/src/font/shaper.rs` (shape_prepared_runs)

- [ ] Two-phase API:
  - [ ] Phase 1: `prepare_line()` — segment into runs (immutable, reuses scratch buffers)
  - [ ] Phase 2: `shape_prepared_runs()` — shape each run (needs rustybuzz Face references)
  - [ ] **Why two phases?** Create rustybuzz `Face` objects once per frame, reuse across all rows. Faces borrow font bytes, so they must outlive shaping calls.
- [ ] `shape_prepared_runs(runs: &[ShapingRun], faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<ShapedGlyph>)`
  - [ ] For each run:
    - [ ] Create `rustybuzz::UnicodeBuffer`, push run's text
    - [ ] Set direction: `LeftToRight` (terminal is always LTR)
    - [ ] Get features for this face: `collection.features_for_face(run.face_idx)`
    - [ ] Call `rustybuzz::shape(face, &features, buffer)`
    - [ ] Extract `glyph_infos()` and `glyph_positions()`
    - [ ] Scale: `effective_size / upem`
    - [ ] For each (info, position) pair:
      - [ ] Map `info.cluster` (byte offset) → grid column via `run.byte_to_col`
      - [ ] Compute `col_span` from advance: `(x_advance * scale / cell_width).round().max(1)`
      - [ ] Emit `ShapedGlyph`
- [ ] `ShapedGlyph` struct
  - [ ] Fields:
    - `glyph_id: u16` — rustybuzz glyph ID (NOT codepoint — this is the shaped result)
    - `face_idx: FaceIdx` — which face this was shaped from
    - `col_start: usize` — first grid column this glyph occupies
    - `col_span: usize` — how many columns (1 = normal, 2+ = ligature or wide char)
    - `x_offset: f32` — shaper x positioning offset (pixels)
    - `y_offset: f32` — shaper y positioning offset (pixels)
- [ ] Output reuses scratch `Vec<ShapedGlyph>` (cleared + refilled each row)
- [ ] **Tests**:
  - [ ] `"hello"` → 5 glyphs, each col_span=1
  - [ ] `"=>"` with ligature-supporting font → 1 glyph, col_span=2
  - [ ] `"fi"` with liga feature → 1 glyph (fi ligature), col_span=2
  - [ ] `"好"` (wide char) → 1 glyph, col_span=2
  - [ ] CJK char → shaped from fallback face, correct face_idx

---

## 6.5 Ligature + Multi-Cell Glyph Handling

Map shaped glyphs back to grid columns. Ligatures span multiple columns — only the first column renders the glyph.

**File:** `oriterm/src/gpu/render_grid.rs` (rendering integration)

- [ ] Column → glyph mapping:
  - [ ] `col_glyph_map: Vec<Option<usize>>` — maps column index → index in shaped glyphs vec
  - [ ] For each shaped glyph: `col_glyph_map[glyph.col_start] = Some(glyph_index)`
  - [ ] Subsequent columns of a ligature (col_start+1, col_start+2, ...) remain `None`
  - [ ] During rendering: if `col_glyph_map[col]` is `Some(i)` → render glyph; if `None` → skip (continuation of ligature)
- [ ] Ligature background:
  - [ ] Background color for each column still rendered independently (cell-by-cell)
  - [ ] Only the foreground glyph spans multiple columns
- [ ] Ligature + selection interaction:
  - [ ] If selection covers part of a ligature, still render the full glyph
  - [ ] Selection highlighting applies to individual cells (not whole ligature)
- [ ] Ligature + cursor interaction:
  - [ ] Cursor on a ligature column renders on top of the glyph
  - [ ] Cursor rendering is per-cell, unaffected by glyph span
- [ ] **Tests**:
  - [ ] `"=>"` ligature: col 0 gets glyph, col 1 is None
  - [ ] Selection of col 1 of a ligature doesn't duplicate glyph
  - [ ] Mixed ligature + non-ligature on same line renders correctly

---

## 6.6 Combining Marks + Zero-Width Characters

Handle combining diacritics, ZWJ sequences, and other zero-width characters.

**Files:** `oriterm_core/src/cell.rs` (storage), `oriterm/src/font/shaper.rs` (shaping)

- [ ] Cell storage for zero-width characters:
  - [ ] Add to `CellExtra`: `zerowidth: Option<Vec<char>>` — combining marks attached to this cell
  - [ ] `Cell::push_zerowidth(&mut self, ch: char)` — add combining mark
  - [ ] `Cell::zerowidth(&self) -> &[char]` — get combining marks (empty slice if none)
  - [ ] Zero-width chars don't advance the cursor — they attach to the preceding cell
- [ ] VTE handler integration:
  - [ ] When `input(ch)` receives a character with `unicode_width == 0`:
    - [ ] Don't advance cursor
    - [ ] Push to previous cell's zerowidth list
- [ ] Shaping integration:
  - [ ] In `prepare_line()`: after appending base char, also append `cell.zerowidth()` chars to run text
  - [ ] All zero-width chars get same column mapping as their base char
  - [ ] Rustybuzz handles combining: base + accent → single positioned cluster
- [ ] Rendering:
  - [ ] Shaper produces multiple glyphs at same col_start (base + marks)
  - [ ] Each glyph rendered with its own x_offset/y_offset from shaper
  - [ ] Multiple glyphs at same column are all rendered (not just first)
- [ ] **Tests**:
  - [ ] `'e'` + `'\u{0301}'` (combining acute) → single shaping cluster at same column
  - [ ] `'a'` + `'\u{0308}'` (combining diaeresis) → 'ä' appearance
  - [ ] ZWJ sequence (e.g., family emoji): stored as base + zerowidth sequence
  - [ ] Width: combining marks don't advance cursor (width 0)

---

## 6.7 OpenType Feature Control

Collection-wide and per-fallback OpenType feature settings.

**File:** `oriterm/src/font/collection.rs` (continued)

- [ ] Collection-wide features:
  - [ ] `features: Vec<rustybuzz::Feature>` — applied to all primary faces
  - [ ] Default: `["liga", "calt"]` (standard ligatures + contextual alternates)
  - [ ] Parsed from config: `"liga"` → enable, `"-liga"` → disable
- [ ] Per-fallback features:
  - [ ] `FallbackMeta.features` — overrides collection defaults for specific fallback
  - [ ] Use case: disable ligatures for CJK fonts (`["-liga"]`)
- [ ] `features_for_face(&self, face_idx: FaceIdx) -> &[rustybuzz::Feature]`
  - [ ] Primary (0–3): return collection-wide features
  - [ ] Fallback (4+): return fallback-specific features
- [ ] Feature parsing:
  - [ ] `parse_features(input: &[&str]) -> Vec<rustybuzz::Feature>`
  - [ ] `"liga"` → `Feature { tag: tag!("liga"), value: 1, start: 0, end: u32::MAX }`
  - [ ] `"-dlig"` → `Feature { tag: tag!("dlig"), value: 0, start: 0, end: u32::MAX }`
- [ ] Config integration:
  ```toml
  [font]
  features = ["liga", "calt", "dlig"]
  ligatures = true  # Shorthand for liga + calt
  ```
- [ ] **Tests**:
  - [ ] Features parsed correctly: "liga" → value 1, "-liga" → value 0
  - [ ] Collection features applied during shaping
  - [ ] Fallback override: CJK font uses different features than primary

---

## 6.8 Advanced Atlas (Guillotine + LRU + Multi-Page)

Replace Section 04's simple shelf packing with the production atlas: guillotine packing, 2D texture array, LRU eviction.

**File:** `oriterm/src/gpu/atlas.rs`

**Reference:** `_old/src/gpu/atlas.rs`

- [ ] Guillotine rectangle packing:
  - [ ] `RectPacker` struct
    - [ ] `free_rects: Vec<Rect>` — available rectangles
    - [ ] `pack(w: u32, h: u32) -> Option<(u32, u32)>` — find best-short-side-fit
    - [ ] Split: remove chosen rect, create up to 2 children (horizontal or vertical split based on leftover shape)
    - [ ] Reset: clear to single full-page rect
  - [ ] **Why guillotine over shelf?** Better packing density for mixed glyph sizes (CJK large + Latin small + accent tiny)
- [ ] Multi-page texture array:
  - [ ] `GlyphAtlas.texture: wgpu::Texture` — `Texture2DArray` format
  - [ ] Page size: 2048×2048 (configurable, old app used 2048)
  - [ ] Max pages: 4 (= 16MB VRAM at R8Unorm)
  - [ ] Start with 1 page, grow on demand up to max
  - [ ] `pages: Vec<AtlasPage>` — per-page packing state + LRU frame counter
- [ ] LRU eviction:
  - [ ] Each page tracks `last_used_frame: u64`
  - [ ] When all pages full and new glyph needs space:
    - [ ] Find page with oldest `last_used_frame`
    - [ ] Reset that page's packer
    - [ ] Remove all cache entries pointing to that page
    - [ ] Re-insert the new glyph on the now-empty page
- [ ] Cache key: `(glyph_id: u16, face_idx: FaceIdx, size_q6: u32, collection_id: u8)`
  - [ ] `size_q6 = (size * 64.0).round() as u32` — 26.6 fixed-point for precise DPI-aware keying
  - [ ] `collection_id` discriminates grid font (0) vs UI font (1)
  - [ ] **Why Q6?** Prevents rounding collisions at fractional DPI: 13.95pt vs 14.05pt get distinct keys
- [ ] `get_or_insert_shaped(glyph_id, face_idx, size_q6, collection_id, rasterize_fn, queue) -> &AtlasEntry`
  - [ ] Check `shaped_entries` HashMap
  - [ ] If miss: call `rasterize_fn()` to get bitmap, upload to atlas, cache entry
  - [ ] Update page's `last_used_frame`
  - [ ] Return atlas entry (UV coordinates, metrics, page index)
- [ ] `AtlasEntry` struct (same as old):
  - [ ] `uv_pos: [f32; 2]`, `uv_size: [f32; 2]`, `metrics: GlyphMetrics`, `page: u32`
- [ ] `begin_frame()` — increment frame counter
- [ ] `clear()` — full atlas reset (called on font size change)
- [ ] **Tests**:
  - [ ] Guillotine packing: insert 100 varied-size rects, all find positions
  - [ ] Multi-page: fill page 0, overflow to page 1
  - [ ] LRU eviction: fill all 4 pages, insert new glyph → oldest page evicted
  - [ ] Cache hit: same key returns same entry
  - [ ] Q6 keying: slightly different sizes produce different keys

---

## 6.9 Built-in Geometric Glyphs

Pixel-perfect rendering for box drawing, block elements, braille, and powerline glyphs. Bypasses the font pipeline entirely — generated as GPU rectangles.

**File:** `oriterm/src/gpu/builtin_glyphs.rs`

**Reference:** `_old/src/gpu/builtin_glyphs.rs`

- [ ] `is_builtin(ch: char) -> bool` — fast check if character is handled by builtin system
- [ ] `draw_builtin_glyph(ch: char, x: f32, y: f32, w: f32, h: f32, fg: [f32; 4], instances: &mut InstanceWriter) -> bool`
  - [ ] Returns true if handled, false to fall through to font pipeline
- [ ] **Box Drawing** (U+2500–U+257F):
  - [ ] 128 characters, lookup table: `[left, right, up, down]` per char
  - [ ] Values: 0=none, 1=light (thin), 2=heavy (thick), 3=double
  - [ ] Render from cell center: horizontal segments left/right, vertical segments up/down
  - [ ] Line thickness: thin = `max(1.0, round(cell_width / 8.0))`, heavy = `thin * 3.0`
  - [ ] Double lines: two parallel lines with gap = `max(2.0, thin * 2.0)`
  - [ ] Segments connect cleanly at cell boundaries (critical for box drawing to look right)
- [ ] **Block Elements** (U+2580–U+259F):
  - [ ] Full block `█` (U+2588): entire cell filled
  - [ ] Upper half `▀` (U+2580): top half filled
  - [ ] Lower N/8 blocks (U+2581–U+2587): fractional heights from bottom
  - [ ] Left N/8 blocks (U+2589–U+258F): fractional widths from left
  - [ ] Shade blocks: light `░` (25% alpha), medium `▒` (50%), dark `▓` (75%)
  - [ ] Quadrant blocks (U+2596–U+259F): bitmask → fill selected quadrants
- [ ] **Braille** (U+2800–U+28FF):
  - [ ] 8-dot pattern in 2×4 grid
  - [ ] Character value encodes which dots are filled (8-bit bitmask)
  - [ ] Dot positions: 2 columns × 4 rows within cell
  - [ ] Render as small filled circles or rectangles at fractional cell positions
- [ ] **Powerline** (U+E0A0–U+E0D4):
  - [ ] Right-pointing solid triangle (U+E0B0): filled triangle, scanline rendered
  - [ ] Left-pointing solid triangle (U+E0B2): mirrored
  - [ ] Right-pointing thin arrow (U+E0B1): outline only
  - [ ] Left-pointing thin arrow (U+E0B3): mirrored outline
  - [ ] Branch symbol (U+E0A0): git branch icon
  - [ ] Rounded separators, flame shapes, etc.
- [ ] Integration with rendering loop:
  - [ ] Before font glyph lookup: `if builtin_glyphs::draw_builtin_glyph(...) { continue; }`
  - [ ] Built-in glyphs emit background-layer instances (opaque rectangles, not atlas-textured)
- [ ] **Tests**:
  - [ ] Box drawing: `'─'` (U+2500) produces horizontal line
  - [ ] Box drawing: `'┼'` (U+253C) produces cross
  - [ ] Block: `'█'` fills entire cell
  - [ ] Block: `'▄'` fills lower half
  - [ ] Braille: `'⠿'` (U+283F) fills all 6 main dots
  - [ ] Powerline: `''` (U+E0B0) produces triangle

---

## 6.10 Color Emoji

Support for color emoji rendering (CBDT/CBLC bitmap emoji or COLR/CPAL outline emoji).

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/gpu/atlas.rs`

- [ ] Rasterization with color source:
  - [ ] Swash `Render::new(&[Source::ColorOutline, Source::ColorBitmap, Source::Outline])`
  - [ ] `Format::Rgba` for color glyphs (4 bytes per pixel)
  - [ ] `Format::Alpha` for non-color fallback (1 byte per pixel)
  - [ ] Check render result: if RGBA → color glyph, if Alpha → normal glyph
- [ ] Atlas support for color glyphs:
  - [ ] Option A: Separate RGBA atlas (Rgba8Unorm texture) for color glyphs
  - [ ] Option B: Single atlas with mixed formats (more complex shader)
  - [ ] **Recommended: Option A** — separate atlas, separate pipeline pass
  - [ ] Color atlas bind group separate from grayscale atlas
- [ ] Rendering color glyphs:
  - [ ] Color glyphs render with their own colors (not tinted by fg_color)
  - [ ] Fragment shader: sample RGBA directly, blend with background
  - [ ] No foreground color multiplication (unlike grayscale glyphs)
- [ ] Emoji presentation:
  - [ ] Characters like U+2764 (❤) can be text or emoji presentation
  - [ ] VS15 (U+FE0E) forces text presentation
  - [ ] VS16 (U+FE0F) forces emoji presentation
  - [ ] Store variation selectors in cell's zerowidth list
  - [ ] During face resolution: check for VS16 → prefer color emoji font
- [ ] Fallback for emoji:
  - [ ] Windows: Segoe UI Emoji
  - [ ] Linux: Noto Color Emoji
  - [ ] These should be high-priority in fallback chain for emoji codepoints
- [ ] **Tests**:
  - [ ] Emoji character rasterizes as RGBA bitmap
  - [ ] Color glyph renders without fg tinting
  - [ ] VS16 forces emoji presentation
  - [ ] VS15 forces text presentation
  - [ ] Emoji fallback resolves to color emoji font

---

## 6.11 Font Synthesis (Bold + Italic)

When a font lacks a Bold or Italic variant, synthesize it properly using swash's outline manipulation — not crude hacks like double-strike or missing-variant fallback-to-Regular.

**File:** `oriterm/src/font/collection.rs`, `oriterm/src/font/rasterize.rs`

**Reference:** `_old/src/font/collection.rs` (weight_variation_for), Ghostty `src/font/face/freetype.zig` (embolden formula)

- [ ] Variable font weight (preferred path):
  - [ ] If font has `wght` axis: use font variations instead of separate Bold file
  - [ ] `weight_variation_for(face_idx: FaceIdx, weight: u16) -> Option<f32>`
    - [ ] Regular/Italic: use base weight (e.g., 400)
    - [ ] Bold/BoldItalic: `min(weight + 300, 900)` — CSS "bolder" algorithm
    - [ ] Fallbacks: `None` (use font's default weight)
  - [ ] Pass to swash: `scale_ctx.builder(face).variations(&[("wght", value)])`
- [ ] Synthetic bold via `Render::embolden(strength)` (when no real Bold and no wght axis):
  - [ ] `embolden()` uniformly expands outlines before rasterization — strokes get thicker in all directions, not just a 1px horizontal shift
  - [ ] Strength formula (from Ghostty): `(font_height_px * 64.0 / 2048.0).ceil()` — scales proportionally with font size so bold looks consistent at 8pt and 24pt
  - [ ] Bounding box grows: adjust glyph metrics (bearing_x, width) to account for expansion so glyphs don't clip at cell edges
  - [ ] Atlas key includes `synthetic_bold: bool` — emboldened glyphs cached separately from regular
- [ ] Synthetic italic via `Render::transform(Transform::skew(14°, 0°))`:
  - [ ] Standard 14-degree oblique angle (CSS spec, same as Ghostty and cosmic-text)
  - [ ] `Transform::skew(Angle::from_degrees(14.0), Angle::from_degrees(0.0))`
  - [ ] Applied when cell has ITALIC flag but face lacks real italic variant
  - [ ] Atlas key includes `synthetic_italic: bool` — skewed glyphs cached separately
- [ ] Use swash `Synthesis` for automatic detection:
  - [ ] `font_attributes.synthesize(requested_attributes) -> Synthesis`
  - [ ] `synthesis.embolden()` → apply embolden
  - [ ] `synthesis.skew()` → apply transform with returned angle
  - [ ] `synthesis.variations()` → apply weight/width settings
- [ ] Synthesis combinations:
  - [ ] BoldItalic with no variant: apply BOTH embolden and skew simultaneously
  - [ ] Order: variations first, then embolden, then transform (swash applies in render order)
- [ ] **Tests**:
  - [ ] Variable font: weight variation applied (wght=700 produces thicker strokes)
  - [ ] Synthetic bold: emboldened glyph is wider than regular (measure rasterized bitmap)
  - [ ] Synthetic italic: skewed glyph has non-zero horizontal displacement
  - [ ] Combined bold+italic: both embolden and skew applied
  - [ ] Regular cells: no synthesis applied
  - [ ] Synthesis detection: `Synthesis::any()` returns true only when variant is missing

---

## 6.12 Text Decorations

All underline styles, strikethrough, hyperlink underline, URL hover underline.

**File:** `oriterm/src/gpu/render_grid.rs`

**Reference:** `_old/src/gpu/render_grid.rs` (underline/strikethrough sections)

- [ ] **Single underline** (CellFlags::UNDERLINE):
  - [ ] Solid line at `y = cell_bottom - 2px`, thickness = 1px
  - [ ] Spans cell width
- [ ] **Double underline** (CellFlags::DOUBLE_UNDERLINE):
  - [ ] Two solid lines: `y = cell_bottom - 2px` and `y = cell_bottom - 4px`
- [ ] **Curly underline** (CellFlags::CURLY_UNDERLINE):
  - [ ] Sine wave: `y = base_y + amplitude * sin(x * freq)`
  - [ ] Rendered as a sequence of short horizontal rectangles (1px tall) at computed y positions
  - [ ] Amplitude: ~2px, frequency: ~2π per cell_width
- [ ] **Dotted underline** (CellFlags::DOTTED_UNDERLINE):
  - [ ] Alternating 1px on, 1px off pattern
  - [ ] Phase reset at start of each cell
- [ ] **Dashed underline** (CellFlags::DASHED_UNDERLINE):
  - [ ] 3px on, 2px off pattern
- [ ] **Underline color** (SGR 58):
  - [ ] `cell.extra().underline_color` — resolved via palette
  - [ ] If present: use this color for underline
  - [ ] If absent: use foreground color
- [ ] **Strikethrough** (CellFlags::STRIKETHROUGH):
  - [ ] Solid line at `y = cell_top + cell_height / 2`, thickness = 1px
  - [ ] Color: foreground color
- [ ] **Hyperlink underline** (cell has hyperlink via OSC 8):
  - [ ] Dotted underline when not hovered
  - [ ] Solid underline when hovered (cursor over cell)
  - [ ] Color: foreground color (or a distinct link color)
- [ ] **URL hover underline** (implicitly detected URL):
  - [ ] Solid underline on hover
  - [ ] Only visible when Ctrl held + mouse over URL range
- [ ] All decorations emit background-layer instances (opaque rectangles)
- [ ] **Tests**:
  - [ ] Single underline: 1px line at correct y
  - [ ] Curly underline: wave shape (visual test)
  - [ ] Dotted: alternating pattern
  - [ ] Underline color: uses SGR 58 color when set
  - [ ] Strikethrough: centered horizontally

---

## 6.13 UI Text Shaping

Shape non-grid text (tab bar titles, search bar, status text) through rustybuzz without grid-column mapping.

**File:** `oriterm/src/font/shaper.rs` (additional function)

**Reference:** `_old/src/font/shaper.rs` (shape_text_string)

- [ ] `UiShapedGlyph` struct
  - [ ] Fields:
    - `glyph_id: u16`
    - `face_idx: FaceIdx`
    - `x_advance: f32` — absolute pixel advance (for cursor positioning)
    - `x_offset: f32`
    - `y_offset: f32`
  - [ ] No `col_start` / `col_span` — UI text is free-positioned, not grid-locked
- [ ] `shape_text_string(text: &str, faces: &[Option<rustybuzz::Face>], collection: &FontCollection, output: &mut Vec<UiShapedGlyph>)`
  - [ ] Segment text into runs by font face (same as grid shaping)
  - [ ] Shape through rustybuzz
  - [ ] Emit glyphs with absolute x_advance (sum of advances = total text width)
  - [ ] Spaces: emit as advance-only (glyph_id = 0, advance = space width)
- [ ] `measure_text(text: &str, collection: &FontCollection) -> f32`
  - [ ] Sum x_advances for all glyphs → total pixel width
  - [ ] Used for tab bar layout, text truncation, centering
- [ ] Text truncation with ellipsis:
  - [ ] If text width > available width: truncate and append `…` (U+2026)
  - [ ] Binary search for truncation point
- [ ] Integration with tab bar and search bar rendering:
  - [ ] Tab title → `shape_text_string` → glyph instances
  - [ ] Search query → `shape_text_string` → glyph instances
- [ ] **Tests**:
  - [ ] "Hello" → 5 glyphs with sequential advances
  - [ ] Measure text returns correct total width
  - [ ] Truncation: long text gets ellipsis at correct position

---

## 6.14 Pre-Caching + Performance

Eliminate first-frame stalls and optimize per-frame costs.

- [ ] Pre-cache ASCII (0x20–0x7E) at font load time:
  - [ ] Rasterize all printable ASCII for Regular style
  - [ ] Insert into atlas immediately
  - [ ] First frame renders without any rasterization stalls
- [ ] Pre-cache bold ASCII if bold face available
- [ ] Scratch buffer reuse:
  - [ ] `runs_scratch: Vec<ShapingRun>` — cleared + reused per row (not reallocated)
  - [ ] `shaped_scratch: Vec<ShapedGlyph>` — same pattern
  - [ ] `col_glyph_map: Vec<Option<usize>>` — same pattern
  - [ ] Allocated once at max expected size, never shrink
- [ ] Face creation once per frame:
  - [ ] `create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face>>` — creates Face references from FaceData
  - [ ] Called once at start of frame, reused for all rows
  - [ ] Faces borrow from `Arc<Vec<u8>>` in FaceData (zero-copy)
- [ ] Font size change:
  - [ ] Clear entire atlas
  - [ ] Recompute cell metrics
  - [ ] Re-pre-cache ASCII
  - [ ] Invalidate all cached frame data
- [ ] **Performance targets**:
  - [ ] Shaping: < 2ms per frame for 80×24 terminal
  - [ ] Atlas miss (new glyph): < 0.5ms per glyph (rasterize + upload)
  - [ ] Atlas hit: HashMap lookup only (< 1μs)
  - [ ] No allocation in per-cell rendering loop

---

## 6.15 Hinting

Control over glyph hinting — the grid-fitting process that snaps outlines to pixel boundaries for sharper rendering at small sizes. This is the single biggest visual quality factor on non-HiDPI displays (still the majority of monitors in use).

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/font/collection.rs`

**Reference:** WezTerm `wezterm-font/src/ftwrap.rs` (load targets), Ghostty `src/font/face/freetype.zig` (hinting flags)

- [ ] Hinting mode enum:
  ```rust
  pub enum HintingMode {
      /// Full hinting (snaps to pixel grid). Crispest text on non-HiDPI.
      Full,
      /// No hinting (preserves outline shape). Best on HiDPI (2x+) where
      /// subpixel precision isn't needed for sharpness.
      None,
  }
  ```
  - [ ] swash only supports `.hint(bool)` — no "light" mode. Two modes is honest.
- [ ] Auto-detection based on display scale:
  - [ ] `scale_factor < 2.0` → `HintingMode::Full` (non-HiDPI needs grid-fitting)
  - [ ] `scale_factor >= 2.0` → `HintingMode::None` (Retina/4K has enough pixels)
  - [ ] Re-evaluate on `ScaleFactorChanged` events
- [ ] User override via config:
  ```toml
  [font]
  hinting = "full"  # or "none"
  ```
  - [ ] Config value overrides auto-detection
- [ ] Integration with rasterization:
  - [ ] `ScalerBuilder::hint(mode == HintingMode::Full)` applied when building scaler
  - [ ] Hinted glyphs produce different bitmaps — atlas key must include hinting state
  - [ ] `RasterKey` expanded: add `hinted: bool` field
- [ ] Grid-fitted cell metrics:
  - [ ] When hinting is Full: compute cell_width/cell_height from hinted advances
  - [ ] When hinting is None: use unhinted metric (floating-point, rounded)
  - [ ] Hinted metrics are more consistent across glyphs (less cumulative rounding error)
- [ ] Font size change or hinting mode change:
  - [ ] Clear entire atlas (all cached glyphs are now wrong)
  - [ ] Recompute cell metrics
  - [ ] Re-pre-cache ASCII
- [ ] **Tests**:
  - [ ] Hinted glyph bitmap differs from unhinted at same size
  - [ ] Auto-detection: scale 1.0 → Full, scale 2.0 → None
  - [ ] Config override: explicit "none" at scale 1.0 disables hinting
  - [ ] Atlas invalidated on hinting mode change

---

## 6.16 Subpixel Rendering (LCD)

LCD subpixel rendering uses the physical R/G/B subpixels of the display to achieve ~3x effective horizontal resolution. This is what makes ClearType text on Windows look sharp and what macOS used before Retina displays made it unnecessary. No GPU terminal except WezTerm implements this, and WezTerm's is buggy. Getting it right is a headline differentiator.

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/gpu/pipeline.rs` (shader), `oriterm/src/gpu/atlas.rs`

**Reference:** WezTerm `wezterm-font/src/rasterizer/freetype.rs` (LCD rendering), WezTerm `wezterm-gui/src/glyph-frag.glsl` (dual-source blending)

- [ ] Subpixel rasterization via swash:
  - [ ] `Render::format(Format::Subpixel)` — produces RGBA subpixel mask
  - [ ] Output: 4 bytes per pixel. R/G/B channels contain per-subpixel coverage. A channel contains overall coverage.
  - [ ] `Format::Subpixel` uses standard RGB subpixel order (1/3 pixel offsets for R and B)
  - [ ] `zeno::Format::subpixel_bgra()` for BGR panel layouts
  - [ ] `Content::SubpixelMask` indicates subpixel output (vs `Content::Mask` for grayscale)
- [ ] Pixel geometry detection and configuration:
  ```toml
  [font]
  lcd_filter = "rgb"   # "rgb", "bgr", or "none" (disable subpixel)
  ```
  - [ ] Default: `"rgb"` (vast majority of displays)
  - [ ] `"none"` falls back to grayscale alpha rendering
  - [ ] Auto-disable on HiDPI (scale >= 2.0) — Retina displays don't have visible subpixels
- [ ] Separate atlas storage for subpixel glyphs:
  - [ ] Grayscale glyphs: `R8Unorm` texture (1 byte/pixel) — existing path
  - [ ] Subpixel glyphs: `Rgba8Unorm` texture (4 bytes/pixel) — new
  - [ ] Atlas tracks per-entry format: `AtlasEntry.subpixel: bool`
  - [ ] Color emoji always rasterized as `Format::Alpha` → `Rgba8Unorm` (no subpixel for bitmaps)
- [ ] Shader changes for per-channel alpha blending:
  - [ ] Grayscale path (existing): `output.rgb = fg_color.rgb; output.a = texture_sample.r * fg_color.a`
  - [ ] Subpixel path (new): each color channel blended independently:
    ```wgsl
    let mask = textureSample(atlas, sampler, uv);  // RGBA subpixel mask
    output.r = mix(bg.r, fg.r, mask.r);
    output.g = mix(bg.g, fg.g, mask.g);
    output.b = mix(bg.b, fg.b, mask.b);
    output.a = max(mask.r, max(mask.g, mask.b));
    ```
  - [ ] Alternative: dual-source blending (WezTerm approach) — more correct but requires `DUAL_SOURCE_BLENDING` wgpu feature
  - [ ] Decision: start with the `mix()` approach (simpler, no feature requirement), upgrade to dual-source if quality demands it
- [ ] Rendering pipeline integration:
  - [ ] Subpixel glyphs need the background color at render time (for the `mix()`)
  - [ ] Two approaches:
    - [ ] **Read-back**: sample the current framebuffer at the glyph position (expensive)
    - [ ] **Pass bg_color as uniform/instance data**: the Prepare phase already computes bg_color per cell — pass it to the fg shader as an instance attribute
  - [ ] **Recommended**: pass bg_color in the fg instance buffer (add 4 bytes per glyph instance)
- [ ] Interaction with other features:
  - [ ] Transparent backgrounds: subpixel rendering over transparency produces color fringing — fall back to grayscale alpha for cells with non-opaque backgrounds
  - [ ] Selection highlighting: when selection inverts colors, subpixel glyphs must be re-resolved or fall back to grayscale
  - [ ] Color emoji: always grayscale alpha path (no subpixel for pre-colored bitmaps)
- [ ] **Tests**:
  - [ ] Subpixel-rasterized glyph has wider bitmap than grayscale equivalent (3x horizontal)
  - [ ] RGB vs BGR: channel order swapped correctly
  - [ ] Auto-disable: scale 2.0+ renders grayscale
  - [ ] Config "none": forces grayscale regardless of scale
  - [ ] Transparent cell: falls back to grayscale (no color fringing)
  - [ ] Visual regression: compare subpixel vs grayscale rendering of reference string

---

## 6.17 Subpixel Glyph Positioning

Render glyphs at fractional pixel offsets for tighter, more natural spacing. Most visible in UI text (tab titles, search bar) and for combining marks / shaper offsets in grid text. No terminal currently does this.

**File:** `oriterm/src/font/rasterize.rs`, `oriterm/src/gpu/atlas.rs`

**Reference:** cosmic-text `src/swash.rs` (x_bin/y_bin cache key)

- [ ] Fractional offset via swash:
  - [ ] `Render::offset(Vector::new(x_fract, y_fract))` — shifts rasterization grid
  - [ ] A glyph at x=10.3 is rasterized with `offset(0.3, 0.0)` and placed at integer x=10
  - [ ] The rasterizer produces a slightly different bitmap for each fractional offset — the anti-aliasing pattern shifts to represent the true sub-pixel position
- [ ] Quantization to reduce atlas explosion:
  - [ ] 4 horizontal phases: 0.00, 0.25, 0.50, 0.75 (snap fractional part to nearest quarter)
  - [ ] 1 vertical phase: 0.0 only (vertical subpixel rarely matters for horizontal text)
  - [ ] `subpx_bin(fract: f32) -> u8` — returns 0, 1, 2, or 3
  - [ ] 4x atlas entries per glyph shape (acceptable — most grid text hits phase 0)
- [ ] Atlas key expansion:
  - [ ] Add `subpx_x: u8` (0–3) to `RasterKey`
  - [ ] Grid text at integer cell boundaries → always phase 0 (no extra atlas entries)
  - [ ] Shaper x_offset/y_offset → quantized to nearest phase
  - [ ] UI text → free-positioned, each glyph may hit any phase
- [ ] Where subpixel positioning applies:
  - [ ] **UI text** (tab titles, search bar, overlays): full subpixel positioning — glyphs placed at fractional advances from shaper. Biggest visual improvement.
  - [ ] **Combining marks**: shaper x_offset/y_offset are fractional — rasterize at correct subpixel offset for precise diacritic placement
  - [ ] **Ligature internals**: multi-glyph ligatures may have fractional internal offsets
  - [ ] **Base grid text**: integer cell boundaries → phase 0. No extra atlas cost.
- [ ] Config:
  ```toml
  [font]
  subpixel_positioning = true  # default true; false snaps everything to integer
  ```
- [ ] **Tests**:
  - [ ] Phase 0 and phase 2 (0.5) produce different bitmaps for same glyph
  - [ ] Quantization: 0.13 → phase 0, 0.37 → phase 1, 0.62 → phase 2, 0.88 → phase 3
  - [ ] Grid text at integer position: always phase 0
  - [ ] UI text: mixed phases across a shaped string
  - [ ] Atlas key differs by subpx_x: cache stores separate entries

---

## 6.18 Visual Regression Testing

Automated pixel-level comparison of rendered text against golden reference images. This is what prevents regressions and validates that the font pipeline produces correct output across all character types, sizes, and DPI scales. Infrastructure investment that pays compound interest.

**File:** `oriterm/tests/visual/`, `oriterm/src/gpu/renderer/mod.rs` (headless rendering)

**Reference:** Rio/Sugarloaf `sugarloaf/tests/util/image.rs` (FLIP algorithm), Ghostty `test/cases/` (golden PNGs)

- [ ] Headless rendering infrastructure:
  - [ ] `GpuState::new_headless()` already exists — extend to support offscreen render targets
  - [ ] `render_text_to_image(text: &str, font_config: &FontConfig, size: (u32, u32)) -> RgbaImage`
  - [ ] Renders text string through the full pipeline: shaping → atlas → GPU → pixel readback
  - [ ] Returns RGBA pixel buffer for comparison
- [ ] Golden image management:
  - [ ] Store reference PNGs in `oriterm/tests/visual/golden/`
  - [ ] Naming convention: `{test_name}_{size}pt_{dpi}dpi.png`
  - [ ] Generated once, checked into git, reviewed on changes
  - [ ] `ORITERM_UPDATE_GOLDEN=1 cargo test` regenerates golden images
- [ ] Comparison algorithm:
  - [ ] **Per-pixel difference**: compute per-channel absolute difference
  - [ ] **Tolerance threshold**: allow ±1 per channel (anti-aliasing can vary by platform/driver)
  - [ ] **Percentage threshold**: pass if ≥ 99.5% of pixels match within tolerance
  - [ ] On failure: write `{test_name}_actual.png` and `{test_name}_diff.png` for visual inspection
  - [ ] Diff image: highlight mismatched pixels in red
- [ ] Reference test strings (each becomes a golden image test):
  - [ ] `ascii_regular`: `"The quick brown fox jumps over the lazy dog 0123456789"` — baseline Latin
  - [ ] `ascii_bold_italic`: same string with bold and italic variants — synthesis quality
  - [ ] `ligatures`: `"=> -> != === !== >= <= |> <| :: <<"` — ligature shaping
  - [ ] `box_drawing`: `"┌─┬─┐│ │ ││ │ │├─┼─┤│ │ │└─┴─┘"` — pixel-perfect connections
  - [ ] `block_elements`: `"█▓▒░▀▄▌▐▖▗▘▝▚▞"` — block element coverage
  - [ ] `braille`: `"⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏⣿⠿⡿⢿"` — braille pattern correctness
  - [ ] `cjk_fallback`: `"Hello你好世界こんにちは안녕하세요"` — fallback chain + cap-height normalization
  - [ ] `combining_marks`: `"é ñ ü ā ö ẗ ḁ̈"` — diacritic positioning
  - [ ] `powerline`: `"  "` — powerline glyph shapes
  - [ ] `mixed_styles`: bold, italic, bold-italic interleaved — variant switching
- [ ] Multi-size testing:
  - [ ] Test at 10pt, 14pt, 20pt — catches size-dependent hinting/rounding issues
  - [ ] Test at 96 DPI and 192 DPI — catches HiDPI rendering differences
- [ ] Integration with CI:
  - [ ] `cargo test --test visual` runs all golden image tests
  - [ ] Tests skip gracefully if no GPU available (headless may require software rasterizer)
  - [ ] Font dependency: tests use an embedded font (e.g., JetBrains Mono subset) so results are reproducible regardless of system fonts
- [ ] **Tests** (meta — testing the testing framework):
  - [ ] Identical images: comparison passes
  - [ ] 1-pixel-off images: comparison passes (within tolerance)
  - [ ] Visually different images: comparison fails, diff image generated
  - [ ] Missing golden: test skipped with message to regenerate

---

## 6.19 Section Completion

- [ ] All 6.1–6.18 items complete
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
- [ ] UI text shaping: tab bar titles, search bar, measure + truncate
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
