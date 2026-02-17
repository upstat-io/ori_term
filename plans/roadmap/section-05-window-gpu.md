---
section: 5
title: Window + GPU Rendering
status: in-progress
tier: 2
goal: Open a frameless window, initialize wgpu, render the terminal grid with a proper staged render pipeline — first visual milestone
sections:
  - id: "5.1"
    title: Render Pipeline Architecture
    status: complete
  - id: "5.2"
    title: winit Window Creation
    status: complete
  - id: "5.3"
    title: wgpu GpuState + Offscreen Render Targets
    status: complete
  - id: "5.4"
    title: WGSL Shaders + GPU Pipelines
    status: complete
  - id: "5.5"
    title: Uniform Buffer + Bind Groups
    status: complete
  - id: "5.6"
    title: Font Discovery + Rasterization
    status: complete
  - id: "5.7"
    title: Glyph Atlas
    status: not-started
  - id: "5.8"
    title: "Extract Phase (CPU)"
    status: not-started
  - id: "5.9"
    title: "Prepare Phase (CPU)"
    status: in-progress
  - id: "5.10"
    title: "Render Phase (GPU)"
    status: not-started
  - id: "5.11"
    title: App Struct + Event Loop
    status: not-started
  - id: "5.12"
    title: Basic Input + Cursor
    status: not-started
  - id: "5.13"
    title: Render Pipeline Testing
    status: not-started
  - id: "5.14"
    title: "Integration: Working Terminal"
    status: not-started
  - id: "5.15"
    title: Section Completion
    status: not-started
---

# Section 05: Window + GPU Rendering

**Status:** Not Started
**Goal:** The first visual milestone. Open a native frameless window, initialize wgpu (Vulkan/DX12 on Windows, Vulkan on Linux, Metal on macOS), and render the terminal grid through a **proper staged render pipeline** — not scattered GPU code. Every frame flows through: Extract → Prepare → Render. The CPU-side phases are pure functions, fully unit-testable without a GPU.

**Crate:** `oriterm` (binary)
**Dependencies:** `oriterm_core`, `winit`, `wgpu`, `swash`, `rustybuzz`, `window-vibrancy`, `dwrote` (Windows)
**Reference:** `_old/src/gpu/` (what NOT to do — scattered rendering with no pipeline), Bevy's staged render architecture, wgpu test suite patterns.

**Anti-pattern from prototype:** The old codebase had `render_tab_bar()`, `render_grid()`, `render_overlay()`, `render_settings()` as independent functions that each built their own instance buffers, managed their own state, and submitted their own draw calls. No shared pipeline, no separation between CPU and GPU work, no testability. This section builds it right.

---

## 5.1 Render Pipeline Architecture

The organizing principle for all rendering. Every frame flows through these phases in order. No phase reaches back into a previous phase. No phase touches the GPU until the Render phase.

**File:** `oriterm/src/gpu/pipeline_stages.rs` (types + documentation)

### The Three Phases

```
┌─────────┐      ┌─────────┐      ┌──────────┐
│ EXTRACT  │ ──→  │ PREPARE │ ──→  │  RENDER  │
│  (CPU)   │      │  (CPU)  │      │  (GPU)   │
│          │      │         │      │          │
│ Lock     │      │ Build   │      │ Upload   │
│ Snapshot │      │ DrawList│      │ Draw     │
│ Unlock   │      │ Instance│      │ Present  │
│          │      │ Buffers │      │          │
└─────────┘      └─────────┘      └──────────┘
  testable         testable        integration
  (unit test)      (unit test)     (headless GPU)
```

- [x] **Phase 1: Extract** — Lock terminal state, snapshot to `FrameInput`, unlock.
  - [x] Input: `&FairMutex<Term<EventProxy>>`, widget state, cursor state
  - [x] Output: `FrameInput` (owned, no references to locked state)
  - [x] Duration: microseconds. Lock is released before any other work.
  - [x] **Pure data copy.** No GPU types, no rendering logic.

- [x] **Phase 2: Prepare** — Convert `FrameInput` into GPU-ready instance buffers.
  - [x] Input: `&FrameInput`, `&FontCollection`, `&GlyphAtlas` (for UV lookups)
  - [x] Output: `PreparedFrame` containing `InstanceWriter` buffers (bg + fg + overlay)
  - [x] **Pure CPU computation.** Produces `Vec<u8>` byte buffers — no wgpu types, no device, no queue.
  - [x] This is where cell → pixel position math, glyph lookup, color resolution, cursor building all happen.
  - [x] **Fully unit-testable**: given a `FrameInput`, assert the exact bytes in the instance buffers.

- [x] **Phase 3: Render** — Upload buffers to GPU, execute draw calls, present.
  - [x] Input: `&PreparedFrame`, `&GpuState`, target `&wgpu::TextureView` (surface OR offscreen)
  - [x] Output: pixels on screen (or in offscreen texture)
  - [x] This phase is thin — just GPU plumbing. All logic is in Prepare.
  - [x] Accepts any `TextureView` as target (not hardcoded to surface). Enables: tab previews, headless testing, thumbnails.

### Key Data Types

- [x] `FrameInput` — everything needed to build a frame, no references
  - [x] `cells: Vec<RenderableCell>` — visible cells (via `content: RenderableContent`)
  - [x] `cursor: Option<RenderableCursor>` — cursor state (via `content.cursor`)
  - [x] `viewport: (u32, u32)` — viewport size in pixels (via `ViewportSize` newtype)
  - [x] `cell_size: (f32, f32)` — cell dimensions (via `CellMetrics` newtype, includes baseline)
  - [x] `baseline: f32` — font baseline (inside `CellMetrics`)
  - [x] `palette: FramePalette` — resolved colors for this frame
  - [x] `selection: Option<SelectionRange>` — active selection bounds (placeholder type)
  - [x] `search_matches: Vec<SearchMatch>` — highlighted search results (placeholder type)
  - [x] No `Arc`, no `Mutex`, no references — pure owned data.

- [x] `PreparedFrame` — GPU-ready output of the Prepare phase
  - [x] `bg_instances: InstanceWriter` — background quad instances (field: `backgrounds`)
  - [x] `fg_instances: InstanceWriter` — foreground glyph instances (field: `glyphs`)
  - [x] `overlay_instances: InstanceWriter` — overlay instances (field: `cursors`)
  - [x] `viewport: (u32, u32)` — for uniform buffer update (sourced from FrameInput at render time)
  - [x] `clear_color: [f32; 4]` — background clear color (`[f64; 4]` to match wgpu clear API)
  - [x] No wgpu types. Just bytes.

### Pipeline Rules (enforced by type system)

- [x] Extract returns owned `FrameInput` — cannot hold locks across phases
- [x] Prepare takes `&FrameInput`, returns owned `PreparedFrame` — pure function
- [x] Render takes `&PreparedFrame` + GPU resources — the only phase that touches wgpu
- [x] No function crosses phase boundaries (no "prepare and also render" functions)

---

## 5.2 winit Window Creation

**File:** `oriterm/src/window/mod.rs`

- [x] `TermWindow` struct (Chrome `WindowTreeHost` pattern — pure window wrapper, NO tabs/content)
  - [x] Fields:
    - `window: Arc<winit::window::Window>` — the winit window (Arc for wgpu surface)
    - `surface: wgpu::Surface<'static>` — wgpu rendering surface
    - `surface_config: wgpu::SurfaceConfiguration` — surface format, size, present mode
    - `size_px: (u32, u32)` — window size in physical pixels
    - `scale_factor: ScaleFactor` — DPI scale factor (oriterm_ui newtype, clamped)
    - `is_maximized: bool` — window maximized state
  - [x] `TermWindow::new(event_loop, config: &WindowConfig, gpu: &GpuState) -> Result<Self>`
    - [x] Window attributes: frameless (`decorations: false`), transparent, title "oriterm" (via `oriterm_ui::window::create_window`)
    - [x] Initial size: 1024×768 (from `WindowConfig::default()`)
    - [x] Create wgpu surface from window (via `GpuState::create_surface`)
    - [x] Configure surface: format, alpha mode (pre-multiplied for transparency)
    - [x] Store dimensions and scale factor
  - [x] `TermWindow::resize_surface(&mut self, width, height, gpu: &GpuState)`
    - [x] Update surface config with new size (min 1×1)
    - [x] `self.surface.configure(&gpu.device, &self.surface_config)`
  - [x] `TermWindow::request_redraw(&self)` — `self.window.request_redraw()`
  - [x] `TermWindow::scale_factor(&self) -> ScaleFactor`
  - [x] `TermWindow::size_px(&self) -> (u32, u32)`
  - [x] `TermWindow::update_scale_factor(&mut self, f64) -> bool` — DPI change handling
  - [x] `TermWindow::set_visible(&self, bool)` — show after first frame
  - [x] `TermWindow::has_surface_area(&self) -> bool` — skip render when minimized
  - [x] `TermWindow::window_id(&self) -> WindowId` — event routing
  - [x] `WindowCreateError` enum — `Window` + `Surface` variants with `Display`/`Error`/`From`
- [x] Window vibrancy (platform-specific):
  - [x] Windows: `window_vibrancy::apply_acrylic()` for translucent background (via `gpu::transparency`)
  - [x] Linux/macOS: compositor-dependent (via `gpu::transparency`, see Section 03)
  - [x] Fallback: opaque dark background if vibrancy not available (opacity >= 1.0 short-circuits)
- [x] Forward-looking IME setup (no-op until Section 8.3 wires handlers):
  - [x] `window.set_ime_allowed(true)` — enable IME input
  - [x] `window.set_ime_purpose(ImePurpose::Terminal)` — hint for IME engine

---

## 5.3 wgpu GpuState + Offscreen Render Targets

**File:** `oriterm/src/gpu/state.rs`

- [x] `GpuState` struct
  - [x] Fields:
    - `instance: wgpu::Instance` — wgpu instance (Vulkan/DX12 on Windows, Vulkan on Linux, Metal on macOS)
    - `adapter: wgpu::Adapter` — selected GPU adapter (dropped after init, device/queue independent)
    - `device: wgpu::Device` — logical device
    - `queue: wgpu::Queue` — command queue
    - `surface_format: wgpu::TextureFormat` — negotiated format (plus `render_format` sRGB variant)
  - [x] `GpuState::new() -> Result<Self>`
    - [x] Create instance with Vulkan + DX12 + Metal backends (wgpu auto-selects best available)
    - [x] Request adapter (high performance preference)
    - [x] Request device with reasonable limits
    - [x] Determine surface format from adapter capabilities
  - [x] `GpuState::new_headless() -> Result<Self>`
    - [x] Same as `new()` but with `compatible_surface: None`
    - [x] Used for testing — no window or surface required
    - [x] Falls back to software rasterizer if no GPU available
  - [x] `GpuState::configure_surface(&self, surface: &wgpu::Surface, width: u32, height: u32) -> wgpu::SurfaceConfiguration`
    - [x] Select present mode: `Mailbox` preferred (low latency), `Fifo` fallback
    - [x] Alpha mode: `PreMultiplied` for transparency, `Opaque` fallback
    - [x] Return configuration
  - [x] Offscreen render targets:
    - [x] `create_render_target(width: u32, height: u32) -> RenderTarget`
    - [x] `RenderTarget` struct: `texture: wgpu::Texture`, `view: wgpu::TextureView`
    - [x] Same format as surface (`render_format`) so pipelines are reusable
    - [x] Used for: tab previews, headless test rendering, thumbnails
    - [x] `read_render_target(target: &RenderTarget) -> Vec<u8>` — read pixels back to CPU
      - [x] `buffer.slice(..).map_async(MapMode::Read, ...)` + `device.poll(PollType::wait_indefinitely())`
      - [x] Returns RGBA bytes — used by visual regression tests and thumbnail generation

---

## 5.4 WGSL Shaders + GPU Pipelines

**File:** `oriterm/src/gpu/shaders/bg.wgsl`, `oriterm/src/gpu/shaders/fg.wgsl`, `oriterm/src/gpu/pipeline.rs`

### Shaders

- [x] Background vertex shader:
  - [x] Input: instance data (pos, size, uv, fg_color, bg_color, kind)
  - [x] Output: screen-space quad with color
  - [x] Generate 4 vertices from instance (position + size → quad corners via TriangleStrip)
  - [x] Pass bg_color to fragment shader
- [x] Background fragment shader:
  - [x] Solid fill with bg_color
- [x] Foreground vertex shader:
  - [x] Input: instance data (pos, size, uv, fg_color, bg_color, kind)
  - [x] Output: screen-space quad with UV coordinates
- [x] Foreground fragment shader:
  - [x] Sample glyph alpha from atlas texture (R8Unorm)
  - [x] Output: fg_color with sampled alpha (pre-multiplied alpha blending)
- [x] Uniform buffer struct (shared by both shaders):
  - [x] `screen_size: vec2<f32>` — viewport dimensions in pixels (16B with padding)
  - [x] Used to convert pixel coordinates to NDC (-1..1)

### Pipelines

- [x] `create_bg_pipeline(gpu: &GpuState, uniform_layout: &BindGroupLayout) -> RenderPipeline`
  - [x] Vertex shader: bg vertex
  - [x] Fragment shader: bg fragment
  - [x] Instance buffer layout: stride 80 bytes
  - [x] Blend state: premultiplied alpha (for transparent windows)
  - [x] Target format: `gpu.render_format()`
- [x] `create_fg_pipeline(gpu: &GpuState, uniform_layout: &BindGroupLayout, atlas_layout: &BindGroupLayout) -> RenderPipeline`
  - [x] Vertex shader: fg vertex
  - [x] Fragment shader: fg fragment
  - [x] Same instance buffer layout
  - [x] Blend state: premultiplied alpha
  - [x] Two bind groups: uniforms + atlas texture
  - [x] Target format: `gpu.render_format()`

### Instance Buffer Layout

```
Offset  Size  Field           Type
0       8     pos             vec2<f32>
8       8     size            vec2<f32>
16      16    uv              vec4<f32>
32      16    fg_color        vec4<f32>
48      16    bg_color        vec4<f32>
64      4     kind            u32
68      12    _pad            3 × u32
Total:  80 bytes per instance
```

- [x] Vertex pulling: no vertex buffer, use `@builtin(vertex_index)` to generate 4 vertices per instance (TriangleStrip)

---

## 5.5 Uniform Buffer + Bind Groups

**File:** `oriterm/src/gpu/bind_groups/mod.rs`

- [x] Uniform buffer:
  - [x] Create `wgpu::Buffer` with `BufferUsages::UNIFORM | COPY_DST`
  - [x] Size: 16 bytes (`vec2<f32> screen_size` + `vec2<f32> _pad`)
  - [x] Updated on resize: `UniformBuffer::write_screen_size(&queue, width, height)`
- [x] Uniform bind group layout:
  - [x] Binding 0: uniform buffer, vertex visibility (created in 5.4 pipeline.rs)
- [x] Atlas bind group layout:
  - [x] Binding 0: texture view (atlas page), fragment visibility (created in 5.4 pipeline.rs)
  - [x] Binding 1: sampler (linear filtering), fragment visibility (created in 5.4 pipeline.rs)
- [x] Create bind groups from layouts + resources
  - [x] `UniformBuffer::new()` — buffer + bind group from uniform layout
  - [x] `AtlasBindGroup::new()` — sampler + bind group from atlas layout + texture view
  - [x] `AtlasBindGroup::rebuild()` — recreate bind group when atlas texture grows
  - [x] `create_placeholder_atlas_texture()` — 1x1 `R8Unorm` white pixel for pre-atlas bootstrapping

---

## 5.6 Font Discovery + Rasterization

**Files:** `oriterm/src/font/mod.rs`, `oriterm/src/font/collection/mod.rs`, `oriterm/src/font/collection/face.rs`, `oriterm/src/font/collection/tests.rs`

**Deviations from original plan:**
- Glyph-ID-based cache key (`RasterKey { glyph_id, face_idx, size_q6 }`) instead of char-based `GlyphKey`
- Separate resolve/rasterize: `resolve(char, style) -> ResolvedGlyph`, `rasterize(RasterKey) -> RasterizedGlyph`
- Subpixel rendering support via `GlyphFormat` enum (Alpha, SubpixelRgb, SubpixelBgr, Color)
- Synthetic bold/italic flags (`SyntheticFlags`) instead of silent fallback
- f32 metrics throughout (no integer truncation)
- `Arc<Vec<u8>>` for font bytes (shared with rustybuzz in Section 6)

- [x] Font discovery integration (via `discovery::discover_fonts()`):
  - [x] Platform discovery → load font bytes from system paths or embedded fallback
  - [x] `FontSet::load(family, weight) -> Result<Self, FontError>`
- [x] `FontData` struct: `data: Arc<Vec<u8>>`, `index: u32`
- [x] `FontSet` struct — 4 style variants + fallback chain:
  - [x] `regular`, `bold`, `italic`, `bold_italic`: `Option<FontData>`
  - [x] `fallbacks: Vec<FontData>` — fallback fonts for missing glyphs
- [x] `FontCollection` struct:
  - [x] Fields: `primary: [Option<FaceData>; 4]`, `fallbacks`, `size_px: f32`, `cell_width: f32`, `cell_height: f32`, `baseline: f32`, `glyph_cache`, `scale_context`
  - [x] `FontCollection::new(font_set, size_pt, dpi, format, weight) -> Result<Self, FontError>`
  - [x] `rasterize(&mut self, key: RasterKey) -> Option<&RasterizedGlyph>` — cache check → face lookup → swash render → store
  - [x] `resolve(&self, ch, style) -> ResolvedGlyph` — style substitution with synthetic flags
  - [x] `cell_metrics(&self) -> CellMetrics` — produces GPU-ready `CellMetrics`
  - [x] `find_face_for_char(&self, ch, style) -> ResolvedGlyph`
  - [x] Pre-cache ASCII (0x20–0x7E) at construction time
- [x] Shared types in `font/mod.rs`: `GlyphFormat`, `GlyphStyle`, `RasterKey`, `SyntheticFlags`, `ResolvedGlyph`, `FontError`
- [x] `RasterizedGlyph`: `width: u32`, `height: u32`, `bearing_x/y: i32`, `advance: f32`, `format: GlyphFormat`, `bitmap: Vec<u8>`
- [x] Internal `FaceData` + helpers: `validate_font()`, `font_ref()`, `has_glyph()`, `glyph_id()`, `rasterize_from_face()`, `compute_metrics()`, `cap_height_px()`, `size_key()`
- [x] 28 unit tests (embedded-only + system discovery)

---

## 5.7 Glyph Atlas

Texture atlas for glyph bitmaps. Shelf-packing on 1024×1024 texture pages.

**File:** `oriterm/src/gpu/atlas.rs`

- [ ] `GlyphAtlas` struct
  - [ ] Fields: `pages: Vec<wgpu::Texture>`, `page_views`, `shelves`, `cache: HashMap<GlyphKey, AtlasEntry>`, `page_size: u32`
  - [ ] `Shelf` struct: `y: u32`, `height: u32`, `x_cursor: u32`
  - [ ] `GlyphAtlas::new(gpu: &GpuState) -> Self` — create first 1024×1024 R8Unorm page
  - [ ] `insert(&mut self, key, glyph, gpu, queue) -> AtlasEntry` — shelf-pack + upload
  - [ ] `lookup(&self, key) -> Option<&AtlasEntry>`
- [ ] `AtlasEntry`: `page: u32`, `uv_x/y/w/h: f32`, `width/height: u32`, `bearing_x/y: i32`
- [ ] Pre-cache ASCII (0x20–0x7E) at creation time

---

## 5.8 Extract Phase (CPU)

Lock terminal state, copy to owned snapshot, release lock immediately. No GPU types.

**File:** `oriterm/src/gpu/extract.rs`

- [ ] `extract_frame(terminal: &FairMutex<Term<EventProxy>>, cursor_state: &CursorState, viewport: (u32, u32), cell_size: (f32, f32), baseline: f32) -> FrameInput`
  - [ ] `let term = terminal.lock();` — fair lock
  - [ ] Copy visible cells to `Vec<RenderableCell>`
  - [ ] Copy cursor position/shape/visibility
  - [ ] Copy active selection bounds (if any)
  - [ ] Copy palette colors needed for this frame
  - [ ] `drop(term);` — release lock immediately
  - [ ] Total lock hold time: microseconds
  - [ ] Return `FrameInput` (fully owned, no references)
- [ ] `log::trace!` timing around lock acquire/release for profiling
- [ ] **Rule**: after `extract_frame` returns, the terminal lock is NEVER touched again during this frame

### Testability

- [ ] `FrameInput` can be constructed manually in tests (no terminal or lock needed)
- [ ] `FrameInput` implements `Debug` for snapshot testing
- [ ] Factory helpers: `FrameInput::test_grid(cols: usize, rows: usize, text: &str)` — build a test frame from a string

---

## 5.9 Prepare Phase (CPU)

Convert `FrameInput` into GPU-ready instance buffers. **Pure CPU, no wgpu types, fully unit-testable.**

**File:** `oriterm/src/gpu/prepare.rs`

- [x] `InstanceWriter` struct — reusable CPU-side byte buffer
  - [x] Fields: `buf: Vec<u8>`, `count: usize`, `stride: usize` (80)
  - [x] `new(stride)`, `clear()`, `push(data: &[u8])`, `count()`, `as_bytes()`, `into_buffer()`
  - [x] Grows but never shrinks — reused across frames

- [ ] `prepare_frame(input: &FrameInput, atlas: &AtlasLookup) -> PreparedFrame`
  - [ ] `AtlasLookup` — trait or struct that maps `GlyphKey → AtlasEntry` (no GPU types)
    - [ ] Production: backed by `GlyphAtlas` cache
    - [ ] Tests: backed by `HashMap<GlyphKey, AtlasEntry>` — no GPU needed
  - [ ] Clear instance writers (reuse buffers from previous frame)
  - [ ] For each visible row in `input.cells`:
    - [ ] For each cell in row:
      - [ ] Compute pixel position: `(col * cell_width, row * cell_height)`
      - [ ] Build 80-byte bg instance: position, size, bg_color, corner_radius=0
      - [ ] Push to `bg_instances`
      - [ ] If cell has a visible character (not space):
        - [ ] Look up glyph UV in `atlas` by `GlyphKey`
        - [ ] Build 80-byte fg instance: position + bearing offset, glyph size, UV, fg_color
        - [ ] Push to `fg_instances`
  - [ ] Build cursor instance(s):
    - [ ] `Block` → bg rect (cursor color) + fg glyph (inverted)
    - [ ] `Bar` → 2px vertical line at left edge
    - [ ] `Underline` → thin horizontal line at bottom
    - [ ] `HollowBlock` → 4 thin outline rects
    - [ ] Respect `SHOW_CURSOR` flag, blink state
  - [ ] Build selection highlight instances (inverted bg/fg for selected cells)
  - [ ] Return `PreparedFrame` with populated instance writers + clear color + viewport

### Testability

- [ ] `prepare_frame` is a pure function: same `FrameInput` + same `AtlasLookup` = identical `PreparedFrame`
- [ ] Instance buffer contents are deterministic — snapshot-testable
- [ ] No wgpu, no device, no queue — runs in `cargo test` without GPU
- [ ] Test helpers:
  - [ ] `assert_instance_count(prepared, expected_bg, expected_fg)` — verify instance counts
  - [ ] `decode_instance(bytes: &[u8]) -> DecodedInstance` — parse 80-byte instance for assertions
  - [ ] `DecodedInstance` has `pos`, `size`, `fg_color`, `bg_color`, `uv_pos`, `flags` fields

---

## 5.10 Render Phase (GPU)

Upload prepared buffers to GPU, execute draw calls, present. This phase is thin — all logic is in Prepare.

**File:** `oriterm/src/gpu/render.rs`

- [ ] `GpuRenderer` struct
  - [ ] Fields:
    - `bg_pipeline: wgpu::RenderPipeline`
    - `fg_pipeline: wgpu::RenderPipeline`
    - `uniform_buffer: wgpu::Buffer`
    - `uniform_bind_group: wgpu::BindGroup`
    - `atlas: GlyphAtlas`
    - `atlas_bind_group: wgpu::BindGroup`
    - `bg_gpu_buffer: Option<wgpu::Buffer>` — GPU-side, grows as needed
    - `fg_gpu_buffer: Option<wgpu::Buffer>` — GPU-side, grows as needed
    - `font_collection: FontCollection`
  - [ ] `GpuRenderer::new(gpu: &GpuState, font_collection: FontCollection) -> Self`
    - [ ] Create pipelines, uniform buffer, bind groups, atlas
    - [ ] Pre-cache ASCII glyphs in atlas

- [ ] `render_frame(&mut self, prepared: &PreparedFrame, gpu: &GpuState, target: &wgpu::TextureView)`
  - [ ] **Note: accepts any `TextureView`** — not coupled to a surface
  - [ ] Update uniform buffer with viewport size: `queue.write_buffer(&uniform_buf, 0, &[w, h])`
  - [ ] Ensure GPU buffers are large enough (grow if needed, never shrink)
  - [ ] Upload instance data: `queue.write_buffer(&bg_gpu_buf, 0, prepared.bg_instances.as_bytes())`
  - [ ] Upload fg instances similarly
  - [ ] Create command encoder
  - [ ] **Render pass 1: Backgrounds**
    - [ ] Load op: `Clear` with `prepared.clear_color`
    - [ ] Set bg_pipeline, uniform bind group, bg_gpu_buffer
    - [ ] `draw(0..6, 0..bg_instance_count)` — 6 vertices per quad, instanced
  - [ ] **Render pass 2: Foregrounds**
    - [ ] Load op: `Load` (preserve backgrounds)
    - [ ] Set fg_pipeline, uniform + atlas bind groups, fg_gpu_buffer
    - [ ] `draw(0..6, 0..fg_instance_count)`
  - [ ] `gpu.queue.submit([encoder.finish()])`

- [ ] `render_to_surface(&mut self, prepared: &PreparedFrame, gpu: &GpuState, surface: &wgpu::Surface) -> Result<()>`
  - [ ] Acquire surface texture: `surface.get_current_texture()`
  - [ ] Create view from surface texture
  - [ ] Call `render_frame(prepared, gpu, &view)`
  - [ ] `output.present()`
  - [ ] Handle surface errors: `Lost` → reconfigure, `OutOfMemory` → skip, `Timeout` → skip

- [ ] GPU buffer management:
  - [ ] `ensure_buffer(device, existing: &mut Option<Buffer>, needed_bytes, usage) -> &Buffer`
  - [ ] If existing buffer is large enough, reuse it
  - [ ] Otherwise, create new buffer (round up to power of 2)
  - [ ] Prevents per-frame GPU buffer allocation

---

## 5.11 App Struct + Event Loop

The main application struct. Implements winit's `ApplicationHandler`. Orchestrates the pipeline phases.

**File:** `oriterm/src/app/mod.rs`

- [ ] `App` struct
  - [ ] Fields:
    - `gpu: Option<GpuState>` — initialized on `Resumed` event
    - `renderer: Option<GpuRenderer>` — initialized after GPU + fonts
    - `window: Option<TermWindow>` — created on `Resumed`
    - `tabs: HashMap<TabId, Tab>` — active tabs (initially one)
    - `active_tab: Option<TabId>` — currently focused tab
    - `event_proxy: EventLoopProxy<TermEvent>` — for creating EventProxy instances
    - `frame_input_scratch: Option<FrameInput>` — reusable allocation
  - [ ] Max ~10 fields. Additional state goes in dedicated sub-structs.
- [ ] `impl ApplicationHandler<TermEvent> for App`
  - [ ] `fn resumed(...)` — init GPU, window, fonts, renderer, first tab
  - [ ] `fn window_event(...)`:
    - [ ] `CloseRequested` → exit
    - [ ] `Resized(new_size)` → resize window + PTY
    - [ ] `RedrawRequested` → **run the 3-phase pipeline:**
      1. `let frame_input = extract_frame(&tab.terminal, ...);`
      2. `let prepared = prepare_frame(&frame_input, &renderer.atlas);`
      3. `renderer.render_to_surface(&prepared, &gpu, &window.surface);`
    - [ ] `KeyboardInput` → forward to PTY (basic)
    - [ ] `ScaleFactorChanged` → recalculate font metrics, resize
  - [ ] `fn user_event(...)` — handle terminal events (wakeup, title, bell, child exit)
  - [ ] `fn about_to_wait(...)` — coalesce: if dirty, render once, clear dirty

**File:** `oriterm/src/app/event_loop.rs`

- [ ] Event batching:
  - [ ] Collect `dirty` flag during event processing
  - [ ] In `about_to_wait`: if dirty, run pipeline once, clear dirty
  - [ ] Prevents per-keystroke renders when typing fast

---

## 5.12 Basic Input + Cursor

Minimal keyboard handling + cursor rendering. Just enough to type and see output.

- [ ] `WindowEvent::KeyboardInput` handler:
  - [ ] Extract `event.text` (logical text from keypress)
  - [ ] Send to active tab: `tab.write_input(text.as_bytes())`
  - [ ] Handle Enter (`\r`), Backspace (`\x7f`), Ctrl+C (`\x03`), Ctrl+D (`\x04`)
  - [ ] Ignore modifier-only presses, function keys (expanded in Section 08)
- [ ] Cursor rendering (handled in Prepare phase, 5.9):
  - [ ] Block, Bar, Underline, HollowBlock shapes
  - [ ] Blink: 530ms on, 530ms off (standard xterm timing)
  - [ ] Reset blink on keypress
  - [ ] Respect `TermMode::SHOW_CURSOR`

---

## 5.13 Render Pipeline Testing

Testing strategy for the render pipeline. Three layers of tests, from fast/cheap to slow/thorough.

**File:** `oriterm/src/gpu/tests/`

### Layer 1: Unit Tests — Prepare Phase (no GPU, runs in `cargo test`)

These test the CPU-side rendering logic. Fast, deterministic, run everywhere.

- [ ] **Instance buffer correctness:**
  - [ ] Given a `FrameInput` with known cells, verify the exact bytes in `PreparedFrame`
  - [ ] Test: single character 'A' at (0,0) → verify bg instance has correct position/size/color, fg instance has correct UV/position
  - [ ] Test: empty cell (space) → bg instance only, no fg instance
  - [ ] Test: wide character (CJK) → one bg instance spanning 2 cells, one fg instance
  - [ ] Test: cursor at (5, 3) → verify cursor instance position matches cell position

- [ ] **Instance count tests:**
  - [ ] 80×24 grid with all spaces → 1920 bg instances, 0 fg instances
  - [ ] 80×24 grid with all 'A' → 1920 bg + 1920 fg instances
  - [ ] Grid with selection → extra overlay instances for selection highlight

- [ ] **Color resolution tests:**
  - [ ] Default fg/bg → correct palette colors in instance bytes
  - [ ] Bold text → bold color variant
  - [ ] Inverse video → fg/bg swapped in instance
  - [ ] 256-color and truecolor → correct RGB in instance bytes

- [ ] **Layout tests:**
  - [ ] Cell positions are pixel-perfect: cell (c, r) → position (c * cell_width, r * cell_height)
  - [ ] Glyph bearing offsets applied correctly
  - [ ] Viewport bounds respected (no instances outside viewport)

- [ ] **Determinism test:**
  - [ ] Same `FrameInput` → identical `PreparedFrame` bytes (bitwise equal)
  - [ ] Run twice, compare — catches any hidden state or randomness

### Layer 2: Integration Tests — Headless GPU (needs GPU adapter, no window)

These test the full pipeline including GPU submission. Slower, but still automated.

- [ ] **Headless rendering setup:**
  - [ ] `GpuState::new_headless()` — creates adapter with `compatible_surface: None`
  - [ ] Create offscreen `RenderTarget` (e.g. 640×480)
  - [ ] Full pipeline: extract → prepare → render to offscreen target → read back pixels

- [ ] **Pixel readback tests:**
  - [ ] Render a single colored cell → verify the pixel region has the expected color
  - [ ] Render white text on black background → verify non-zero alpha in glyph region
  - [ ] Render cursor → verify cursor pixels are present at expected position

- [ ] **Pipeline smoke tests:**
  - [ ] Pipeline creation does not error
  - [ ] GPU adapter is found
  - [ ] Offscreen render target creates successfully
  - [ ] A frame renders without GPU errors or validation warnings
  - [ ] `wgpu` validation layer enabled in tests to catch API misuse

### Layer 3: Visual Regression Tests (optional, CI-friendly)

Compare rendered output against reference images. Catches subtle rendering regressions.

- [ ] **Reference image workflow:**
  - [ ] Render known terminal content to PNG via headless pipeline
  - [ ] Compare against checked-in reference PNGs in `tests/references/`
  - [ ] Fuzzy comparison: allow per-pixel tolerance (±2 per channel) for anti-aliasing differences
  - [ ] On failure: save actual output + diff image for inspection
- [ ] **Test scenarios:**
  - [ ] `tests/references/basic_grid.png` — 80×24 grid with ASCII text
  - [ ] `tests/references/colors_16.png` — 16 ANSI colors
  - [ ] `tests/references/cursor_shapes.png` — all cursor shapes
  - [ ] `tests/references/bold_italic.png` — styled text
- [ ] **CI considerations:**
  - [ ] Headless GPU tests require a GPU adapter in CI (or software rasterizer like lavapipe/llvmpipe)
  - [ ] Mark as `#[ignore]` by default, run with `cargo test -- --ignored` in GPU-enabled CI
  - [ ] Non-GPU unit tests (Layer 1) always run in all CI environments

---

## 5.14 Integration: Working Terminal

The "it works" milestone. Everything comes together.

- [ ] Launch sequence:
  - [ ] `main.rs` creates `winit::EventLoop` with `TermEvent` user events
  - [ ] Creates `App` struct
  - [ ] `event_loop.run_app(&mut app)` — enters the event loop
  - [ ] On `Resumed`: GPU init, window, fonts, renderer, first tab
- [ ] Verify visually:
  - [ ] Window opens (frameless, transparent/vibrancy)
  - [ ] Terminal grid renders with monospace font
  - [ ] Shell prompt appears
  - [ ] Type `echo hello` → see "hello" in output
  - [ ] Colors work: `ls --color` shows colored output
  - [ ] Cursor is visible and blinks
  - [ ] Window resize works (grid re-renders at new size)
  - [ ] Scroll: output that exceeds screen scrolls correctly
- [ ] Verify pipeline discipline:
  - [ ] `log::trace!` timing shows: Extract < 100μs, Prepare < 1ms, Render < 2ms
  - [ ] Terminal lock is never held during Prepare or Render phases
  - [ ] No wgpu types appear in Extract or Prepare phase code
  - [ ] Frame builds are deterministic (same input → same instance buffer bytes)
- [ ] Verify threading:
  - [ ] PTY reader thread processes output without blocking renderer
  - [ ] No visible stutter when output is flowing

---

## 5.15 Section Completion

- [ ] All 5.1–5.14 items complete
- [ ] **Pipeline architecture:**
  - [ ] Extract → Prepare → Render phases are cleanly separated
  - [ ] No function crosses phase boundaries
  - [ ] Prepare phase has zero wgpu imports
  - [ ] Render phase accepts any `TextureView` (surface or offscreen)
- [ ] **Testing:**
  - [ ] Prepare phase unit tests pass (instance buffer correctness, counts, colors, determinism)
  - [ ] Headless GPU integration tests pass (pipeline creation, offscreen render, pixel readback)
  - [ ] Visual regression test infrastructure exists (even if initial reference set is small)
- [ ] **Functional:**
  - [ ] Binary launches, window appears, terminal grid renders <!-- unblocks:3.8 -->
  - [ ] Shell is functional: can type commands and see output
  - [ ] Colors render correctly
  - [ ] Cursor visible and blinks
  - [ ] Resize works
  - [ ] No visible rendering artifacts
- [ ] **Build:**
  - [ ] `cargo build -p oriterm --target x86_64-pc-windows-gnu --release` succeeds
  - [ ] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
  - [ ] `cargo test -p oriterm` — all prepare-phase unit tests pass
- [ ] No mouse selection, no search, no config, no tabs — just one terminal in one window

**Exit Criteria:** A working, visually correct terminal emulator with a clean, tested render pipeline. The pipeline architecture (Extract → Prepare → Render) is the foundation that all future rendering builds on. The Prepare phase is independently testable. Offscreen rendering works for tab previews and headless testing.
