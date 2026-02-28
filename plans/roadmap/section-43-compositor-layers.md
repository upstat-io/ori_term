---
section: 43
title: Compositor Layer System + Animation Architecture
status: not-started
tier: 5
goal: GPU-backed compositor layer system with render-to-texture composition, layer tree hierarchy, property animation (opacity, transform, bounds), animation sequences/groups, and integration with overlay fade, tab sliding, smooth scrolling
sections:
  - id: "43.1"
    title: Transform2D
    status: not-started
  - id: "43.2"
    title: Layer Primitives
    status: not-started
  - id: "43.3"
    title: Layer Tree
    status: not-started
  - id: "43.4"
    title: Layer Delegate
    status: not-started
  - id: "43.5"
    title: Lerp Additions
    status: not-started
  - id: "43.6"
    title: GPU Compositor
    status: not-started
  - id: "43.7"
    title: Layer Animator
    status: not-started
  - id: "43.8"
    title: Animation Delegate
    status: not-started
  - id: "43.9"
    title: Animation Sequences & Groups
    status: not-started
  - id: "43.10"
    title: Overlay Fade Integration
    status: not-started
  - id: "43.11"
    title: Tab Sliding Integration
    status: not-started
  - id: "43.12"
    title: Smooth Scrolling Integration
    status: not-started
  - id: "43.13"
    title: Section Completion
    status: not-started
---

# Section 43: Compositor Layer System + Animation Architecture

**Status:** Not Started
**Goal:** Add a proper compositor layer system to `oriterm_ui` with GPU-backed composition in `oriterm`. Each layer renders to a texture; a composition pass blends layers with per-layer opacity and transforms. Layer properties are animated by a `LayerAnimator`. This is the missing piece between widget-level animation (CPU, `AnimatedValue<T>`) and compositor-level animation (GPU, layer opacity/transform/bounds).

**Crate:** `oriterm_ui` (layer tree, animator, no GPU dependency), `oriterm` (GPU compositor, render-to-texture, composition pass)
**Dependencies:** Section 07 (UI framework — DrawList, Easing, Lerp, overlay system), Section 05 (GPU rendering — wgpu pipelines)

**Why this matters:** 28 roadmap features across 8 sections depend on compositor-level effects:

| Section | Feature | Compositor Need |
|---------|---------|-----------------|
| 07.9 | Overlay fade-in/fade-out | Layer opacity animation |
| 07.9 | Tab bar tab sliding | Layer transform animation |
| 07.9 | Smooth scrolling | Layer transform (Y offset) |
| 16.3 | Tab hover previews | Offscreen render → scaled layer |
| 24.5 | Smooth scrolling | Sub-line pixel offset, kinetic decay |
| 24.6 | Background images | Image layer below cells, opacity |
| 24.7 | Background gradients | Gradient layer, opacity blending |
| 24.8 | Window backdrop effects | Window opacity, layered composition |
| 27.2 | Quick Terminal (drop-down) | Slide animation (200ms ease-out/in) |
| 27.4 | Progress indicators | Pulsing animation |
| 33.4 | Floating pane shadows | Shadow layer behind pane content |
| 39.5 | Image protocols | Image texture compositing, z-order |
| 42.1-42.5 | Expose/Overview mode | Full-screen compositor: offscreen render pool, live thumbnails, staggered updates, scaled transforms |

**Design Principles:**
- **Render-to-texture correctness**: Per-instance opacity is WRONG. A layer at 50% opacity with text on a button causes double-blending. Each layer renders to its own texture first, then the texture composites at the layer's opacity — the layer fades as a visual unit.
- **Performance escape hatch**: Layers with default properties (opacity=1.0, transform=identity, visible=true) skip the intermediate texture and render directly to screen — zero overhead when not animating.
- **Parent-child nesting**: Expose mode needs a group layer containing N thumbnail layers with independent transform+opacity. Quick Terminal needs a container that slides as a unit. A flat list can't express "apply this transform to all these children."

**Inspired by:**
- Chrome's compositor (cc/): layer tree, render-to-texture, property animations on compositor thread
- Core Animation (macOS): CALayer hierarchy, implicit animations, opacity/transform/bounds
- Flutter's compositing layer tree: retained layers, repaint boundaries

**Architecture overview:**
```
oriterm_ui (no GPU dependency)          oriterm (wgpu)
================================        ================================
Layer, LayerId, LayerProperties         GpuCompositor
LayerTree (parent-child, z-order)       - render-to-texture per layer
LayerAnimator (property transitions)    - composition pass (blend layers)
AnimationSequence, AnimationGroup       - composition shader (opacity, transform)
AnimationBuilder (fluent API)           RenderTargetPool (texture reuse)
AnimationDelegate (callbacks)           ImagePipeline (textured quads)
Transform2D (affine math)
```

---

## 43.1 Transform2D

2D affine transform — the mathematical foundation for layer transforms.

**File:** `oriterm_ui/src/compositor/transform.rs`, `oriterm_ui/src/compositor/tests.rs`

```rust
/// 2D affine transform represented as a 3×2 column-major matrix.
///
/// Maps point (x,y) → (a*x + c*y + tx, b*x + d*y + ty).
pub struct Transform2D {
    matrix: [f32; 6],  // [a, b, c, d, tx, ty]
}
```

- [ ] `Transform2D` struct with `[f32; 6]` matrix
- [ ] `identity()` — no-op transform
- [ ] `translate(tx, ty)` — translation
- [ ] `scale(sx, sy)` — scaling (uniform and non-uniform)
- [ ] `rotate(radians)` — rotation around origin
- [ ] `concat(other)` — matrix multiplication (compose transforms)
- [ ] `pre_translate`, `pre_scale` — apply transform BEFORE self
- [ ] `apply(Point) -> Point` — transform a point
- [ ] `apply_rect(Rect) -> Rect` — transform bounding box (axis-aligned result)
- [ ] `inverse() -> Option<Transform2D>` — inverse for hit-testing through transforms
- [ ] `is_identity() -> bool` — fast check for performance escape hatch
- [ ] `to_mat3x2() -> [f32; 6]` — for GPU uniform upload
- [ ] `Lerp` impl — per-element lerp (sufficient for translate+scale animations)

**Tests:**
- [ ] identity roundtrip
- [ ] translate
- [ ] scale
- [ ] rotate (90°, 180°, 360°)
- [ ] concat associativity
- [ ] inverse roundtrip
- [ ] degenerate (zero scale → no inverse)
- [ ] `is_identity` true/false
- [ ] Lerp interpolation

---

## 43.2 Layer Primitives

Core layer types — `LayerId`, `LayerType`, `LayerProperties`, `Layer`.

**File:** `oriterm_ui/src/compositor/layer.rs`, `oriterm_ui/src/compositor/tests.rs`

```rust
pub struct LayerId(u64);  // Copy + Eq + Hash, auto-incrementing

pub enum LayerType {
    Textured,                // Renders content via LayerDelegate, backed by a texture
    SolidColor(Color),       // Flat color fill (modal dimming, separators)
    Group,                   // No own content — groups children, transform/opacity apply to subtree
}

pub struct LayerProperties {
    pub bounds: Rect,
    pub opacity: f32,            // 0.0–1.0, multiplied down tree
    pub transform: Transform2D,
    pub visible: bool,
    pub clip_children: bool,
}

pub struct Layer {
    id: LayerId,
    layer_type: LayerType,
    properties: LayerProperties,
    parent: Option<LayerId>,
    children: Vec<LayerId>,
    needs_paint: bool,      // Content dirty → re-render to texture
    needs_composite: bool,  // Properties dirty → re-composite
}
```

- [ ] `LayerId` — newtype, `Copy + Eq + Hash`, counter-based allocation
- [ ] `LayerType` — `Textured`, `SolidColor(Color)`, `Group`
- [ ] `LayerProperties` — bounds, opacity, transform, visible, clip_children
- [ ] `LayerProperties::default()` — identity transform, opacity 1.0, visible true
- [ ] `Layer` struct — id, type, properties, parent, children, dirty flags
- [ ] `Layer::needs_texture()` — true when properties differ from defaults (opacity != 1.0 or transform != identity)
- [ ] Dirty flag setters mark `needs_composite`

**Tests:**
- [ ] `LayerId` uniqueness via counter
- [ ] `LayerProperties::default()` is identity
- [ ] `needs_texture()` false for defaults, true when opacity < 1.0

---

## 43.3 Layer Tree

Parent-child hierarchy with z-ordering.

**File:** `oriterm_ui/src/compositor/layer_tree.rs`, `oriterm_ui/src/compositor/tests.rs`

```rust
pub struct LayerTree {
    layers: HashMap<LayerId, Layer>,
    root: LayerId,
    next_id: u64,
}
```

- [ ] `new(viewport: Rect)` — creates tree with root group layer
- [ ] `add(parent, layer_type, properties) -> LayerId`
- [ ] `remove(id) -> bool` — removes layer, reparents children to parent
- [ ] `remove_subtree(id)` — removes layer and all descendants
- [ ] `get(id) -> Option<&Layer>`, `get_mut(id) -> Option<&mut Layer>`
- [ ] Property setters: `set_opacity`, `set_transform`, `set_bounds`, `set_visible` — mark `needs_composite`
- [ ] `schedule_paint(id)` — mark `needs_paint`
- [ ] Z-order: `stack_above(id, sibling)`, `stack_below(id, sibling)`
- [ ] `reparent(id, new_parent)` — move layer to different parent
- [ ] `iter_back_to_front()` — depth-first traversal in paint order
- [ ] `accumulated_opacity(id) -> f32` — walk ancestors, multiply opacities
- [ ] `accumulated_transform(id) -> Transform2D` — walk ancestors, concat transforms
- [ ] `layers_needing_paint() -> Vec<LayerId>` — dirty query
- [ ] `layers_needing_composite() -> Vec<LayerId>` — dirty query
- [ ] `clear_dirty_flags()` — after frame

**Tests:**
- [ ] add single layer, verify parent-child
- [ ] add nested layers, verify hierarchy
- [ ] remove with reparenting
- [ ] remove_subtree cleans all descendants
- [ ] z-order: stack_above/stack_below reorder children
- [ ] reparent moves layer
- [ ] iter_back_to_front paint order
- [ ] accumulated_opacity multiplies chain
- [ ] accumulated_transform concatenates chain
- [ ] dirty tracking (paint + composite flags)
- [ ] clear_dirty_flags

---

## 43.4 Layer Delegate

Content provider — decouples "what to render" from "how to composite."

**File:** `oriterm_ui/src/compositor/delegate.rs`

```rust
pub trait LayerDelegate {
    fn paint_layer(&self, layer_id: LayerId, ctx: &mut DrawCtx<'_>);
}
```

- [ ] `LayerDelegate` trait with `paint_layer` method
- [ ] Documentation: called by compositor when `needs_paint` is true
- [ ] `DrawCtx` bounds are the layer's own bounds (origin at 0,0)

Future consumers: overlay manager, tab bar widget, terminal grid, search bar, context menu, settings panel, Quick Terminal panel, expose mode thumbnails.

---

## 43.5 Lerp Additions

`Lerp` impls for compositor types.

**File:** `oriterm_ui/src/animation/mod.rs` (or appropriate animation file)

- [ ] `Lerp for Rect` — per-field (x, y, width, height)
- [ ] `Lerp for Transform2D` — per-element matrix lerp
- [ ] `Lerp for Point` — per-field (x, y)
- [ ] `Lerp for Size` — per-field (width, height)

**Tests:**
- [ ] Rect lerp at 0.0, 0.5, 1.0
- [ ] Transform2D lerp between translate and identity
- [ ] Point lerp
- [ ] Size lerp

---

## 43.6 GPU Compositor

The GPU side — manages textures and the composition pass.

**Files:** `oriterm/src/gpu/compositor/mod.rs`, `oriterm/src/gpu/compositor/render_target_pool.rs`, `oriterm/src/gpu/compositor/composition_pass.rs`, `oriterm/src/gpu/shaders/composite.wgsl`

### 43.6a RenderTargetPool

```rust
pub struct RenderTargetPool {
    targets: Vec<PoolEntry>,
}

struct PoolEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    in_use: bool,
}
```

- [ ] `acquire(device, width, height) -> &TextureView` — allocate or reuse
- [ ] `release(view)` — return to pool
- [ ] `trim()` — reclaim unused textures
- [ ] Sizing: round up to power-of-two buckets (256, 512, 1024, 2048) to maximize reuse

### 43.6b GpuCompositor

```rust
pub struct GpuCompositor {
    pool: RenderTargetPool,
    composition_pipeline: wgpu::RenderPipeline,
    layer_uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    layer_textures: HashMap<LayerId, TextureAssignment>,
}
```

Frame workflow:
1. **Paint phase**: For each layer with `needs_paint` → acquire texture from pool, render layer's DrawList into texture
2. **Composition phase**: Single render pass to screen → for each visible layer back-to-front:
   - Default properties → render directly (no intermediate texture)
   - Non-default properties → draw textured quad with opacity + transform

- [ ] `GpuCompositor::new()` — create pipeline, sampler, uniform buffer
- [ ] `paint_dirty_layers()` — render dirty layers to textures
- [ ] `compose()` — blend all visible layers to screen
- [ ] Direct-render fast path for layers with default properties

### 43.6c Composition Shader

```wgsl
struct LayerUniform {
    transform: mat3x3<f32>,  // 2D affine padded to mat3x3
    bounds: vec4<f32>,       // x, y, w, h in screen space
    opacity: f32,
    _pad: vec3<f32>,
}
```

- [ ] Vertex shader: generate quad from vertex_index, apply transform + bounds → NDC
- [ ] Fragment shader: sample layer texture, multiply by layer opacity (premultiplied alpha)

---

## 43.7 Layer Animator

Drives property transitions. Lives in `oriterm_ui` (no GPU dependency).

**File:** `oriterm_ui/src/compositor/layer_animator.rs`, `oriterm_ui/src/compositor/tests.rs`

```rust
pub enum AnimatableProperty { Opacity, Transform, Bounds }

pub enum PreemptionStrategy {
    ReplaceCurrent,  // Cancel running, start from current value (default)
    Enqueue,         // Queue after current finishes
}

enum TransitionKind {
    Opacity { from: f32, to: f32 },
    Transform { from: Transform2D, to: Transform2D },
    Bounds { from: Rect, to: Rect },
}

pub struct LayerAnimator {
    transitions: HashMap<(LayerId, AnimatableProperty), PropertyTransition>,
    queue: Vec<QueuedTransition>,
    delegate: Option<Box<dyn AnimationDelegate>>,
    preemption: PreemptionStrategy,
}
```

- [ ] `animate_opacity(id, target, duration, easing)` — start opacity transition
- [ ] `animate_transform(id, target, duration, easing)` — start transform transition
- [ ] `animate_bounds(id, target, duration, easing)` — start bounds transition
- [ ] `tick(tree: &mut LayerTree, now: Instant) -> bool` — apply current values, return true if animating
- [ ] `is_animating(id, property) -> bool`
- [ ] `is_any_animating() -> bool`
- [ ] `target_opacity(id) -> Option<f32>` — query animation end state
- [ ] `target_transform(id) -> Option<Transform2D>`
- [ ] `cancel(id, property)` — stop animation, keep current value
- [ ] `cancel_all(id)` — stop all animations on a layer
- [ ] `ReplaceCurrent` preemption: cancel running, start from current interpolated value
- [ ] `Enqueue` preemption: queue after current finishes

`tick()` per frame: walk all transitions → interpolate via `Easing::apply()` + `Lerp` → apply to `LayerTree` → mark `needs_composite` → fire delegate callbacks for ended/canceled → remove finished.

**Tests:**
- [ ] opacity animation start to end
- [ ] transform animation start to end
- [ ] bounds animation start to end
- [ ] tick advances interpolation correctly
- [ ] animation completes and is removed
- [ ] preemption replaces running animation
- [ ] cancel keeps current value
- [ ] `is_any_animating()` tracks state

---

## 43.8 Animation Delegate

Lifecycle callbacks for animation events.

**File:** `oriterm_ui/src/animation/delegate.rs`

```rust
pub trait AnimationDelegate {
    fn animation_ended(&mut self, layer_id: LayerId, property: AnimatableProperty);
    fn animation_canceled(&mut self, layer_id: LayerId, property: AnimatableProperty);
}
```

- [ ] `AnimationDelegate` trait
- [ ] `animation_ended` — fired when animation reaches target
- [ ] `animation_canceled` — fired when animation is interrupted

Use cases: overlay manager (remove layer after fade-out), expose mode (remove thumbnail after exit animation), Quick Terminal (hide panel after slide-out).

---

## 43.9 Animation Sequences & Groups

Chain and parallelize animations.

**Files:** `oriterm_ui/src/animation/sequence.rs`, `oriterm_ui/src/animation/group.rs`, `oriterm_ui/src/animation/builder.rs`

### AnimationSequence

```rust
pub struct AnimationSequence {
    steps: Vec<AnimationStep>,
    current: usize,
    start_time: Instant,
}

pub enum AnimationStep {
    Animate { property: AnimatableProperty, target: TransitionTarget, duration: Duration, easing: Easing },
    Delay(Duration),
    Callback(Box<dyn FnOnce()>),
}
```

- [ ] Chain animations end-to-end
- [ ] `Delay` step for pauses
- [ ] `Callback` step for side effects between animations
- [ ] Use case: toast notification — slide in (200ms) → hold (3s) → slide out (150ms) → remove

### AnimationGroup

```rust
pub struct AnimationGroup {
    layer_id: LayerId,
    transitions: Vec<(AnimatableProperty, TransitionTarget, Duration, Easing)>,
}
```

- [ ] Run multiple property animations in parallel
- [ ] Use case: overlay appear — opacity 0→1 + scale 0.95→1.0 simultaneously

### AnimationBuilder

```rust
AnimationBuilder::new(layer_id)
    .duration(Duration::from_millis(150))
    .easing(Easing::EaseOut)
    .opacity(0.0, 1.0)
    .transform(Transform2D::scale(0.95, 0.95), Transform2D::identity())
    .on_end(|id| { /* cleanup */ })
    .build()  // -> AnimationGroup
```

- [ ] Fluent API for creating animations
- [ ] Default duration and easing overridable per-property
- [ ] `on_end` callback

**Tests:**
- [ ] Sequence steps execute in order
- [ ] Delay pauses between steps
- [ ] Group runs all transitions in parallel
- [ ] Builder produces correct AnimationGroup

---

## 43.10 Overlay Fade Integration

First consumer — proves the full pipeline works.

**File:** `oriterm_ui/src/overlay/manager.rs`, `oriterm/src/gpu/renderer/mod.rs`

- [ ] `OverlayManager` creates compositor layers for overlays
- [ ] `push_overlay` → add `Textured` layer, animate opacity 0→1 (150ms EaseOut)
- [ ] `push_modal` → add `SolidColor` dim layer (animated opacity) + `Textured` content layer
- [ ] Dismiss → animate opacity 1→0, `AnimationDelegate::animation_ended` removes layer
- [ ] Dismissing overlays invisible to event routing (already "dismissed" semantically)
- [ ] `clear_all` → instant removal, cancel animations

---

## 43.11 Tab Sliding Integration

Tab reorder and close use compositor transforms instead of CPU-side offsets.

**File:** `oriterm/src/app/chrome/mod.rs` (or tab bar widget)

- [ ] Tab reorder on drag-end → set `Transform2D::translate(offset, 0)` on displaced tabs, animate to `identity()`
- [ ] Replaces `anim_offsets` + `decay_tab_animations` with compositor transforms
- [ ] Tab close → neighboring tabs slide closed via transform animation

---

## 43.12 Smooth Scrolling Integration

Grid content as a compositor layer with animated Y transform.

**File:** `oriterm/src/app/redraw/mod.rs`, `oriterm/src/gpu/renderer/mod.rs`

- [ ] Grid content rendered into a compositor layer
- [ ] Keyboard Page-Up/Down → animate layer Y transform (100ms EaseOut)
- [ ] Mouse wheel → accumulate pixel delta into layer Y transform
- [ ] Kinetic scroll: track velocity, animate with deceleration (friction 0.95)
- [ ] Snap: when animation settles within 0.5px of line boundary → snap to line, clear transform

---

## 43.13 Section Completion

- [ ] Transform2D math correct (identity, translate, scale, concat, inverse)
- [ ] Layer primitives tested (create, properties, dirty flags)
- [ ] Layer tree tested (add, remove, reparent, z-order, accumulated properties)
- [ ] Layer delegate trait defined and documented
- [ ] GPU compositor renders layers to textures
- [ ] Composition pass blends layers with opacity + transform
- [ ] RenderTargetPool allocates and reuses textures
- [ ] Layer animator drives property transitions
- [ ] Animation delegate fires on end/cancel
- [ ] Animation sequences chain correctly
- [ ] Animation groups run in parallel
- [ ] AnimationBuilder fluent API works
- [ ] Lerp impls for Rect, Transform2D, Point, Size
- [ ] Overlay fade-in/fade-out working via compositor
- [ ] Tab sliding working via compositor transforms
- [ ] Smooth scrolling working via compositor transform
- [ ] Performance: zero overhead when no layers are animating
- [ ] Forward compatibility verified for Sections 16.3, 24, 27.2, 33.4, 39.5, 42
- [ ] `./clippy-all.sh` — no warnings
- [ ] `./test-all.sh` — all pass
- [ ] `./build-all.sh` — cross-compilation succeeds

---

## Forward Compatibility

Features this compositor enables in future sections (no work here — design must support them):

- **Tab hover previews (16.3)**: Render terminal to offscreen texture via `RenderTargetPool`, display as `Textured` layer with `Transform2D::scale(0.25, 0.25)`
- **Quick Terminal (27.2)**: Container `Group` layer with slide transform animation (200ms ease-out from screen edge)
- **Background layers (24.6-24.8)**: Background image/gradient as lowest-z `Textured` layer with independent opacity
- **Image protocols (39.5)**: Image textures composited as layers with z-ordering (above or below cell content)
- **Expose mode (42.1-42.5)**: `Group` layer containing N `Textured` child layers, each thumbnail rendered to offscreen texture, transforms position tiles in grid, staggered update via round-robin `schedule_paint`
- **Floating pane shadows (33.4)**: Shadow as `SolidColor` layer behind pane content layer

---

## Dependency Graph

```
43.1 Transform2D ──────────────────────────────┐
43.2 Layer Primitives ─────────────────────────┤
43.3 Layer Tree ───────────────────────────────┤
43.4 Layer Delegate ───────────────────────────┤
                                               ├──→ 43.10 Overlay Integration
43.5 Lerp Additions ──────────────────────────┤    43.11 Tab Sliding
   depends on 43.1                             │    43.12 Smooth Scrolling
                                               │
43.6 GPU Compositor ──────────────────────────┤
   depends on 43.1-43.4                        │
                                               │
43.7 Layer Animator ──────────────────────────┤
   depends on 43.1-43.3, 43.5                 │
                                               │
43.8 Animation Delegate ─────────────────────┘
   depends on 43.7

43.9 Animation Sequences & Groups
   depends on 43.7-43.8
```

Build order: 43.1 → 43.2 → 43.3 → 43.4 → 43.5 → 43.6 and 43.7 in parallel → 43.8 → 43.9 → 43.10 → 43.11 → 43.12

---

## Files Created/Modified

### New: `oriterm_ui/src/compositor/`
- `mod.rs` — module root, re-exports
- `layer.rs` — `Layer`, `LayerId`, `LayerType`, `LayerProperties`
- `layer_tree.rs` — `LayerTree` (parent-child hierarchy with z-order)
- `layer_animator.rs` — `LayerAnimator` (property transition driver)
- `delegate.rs` — `LayerDelegate` trait (content provider)
- `transform.rs` — `Transform2D` (2D affine math)
- `tests.rs` — unit tests for all compositor types

### New: `oriterm_ui/src/animation/`
- `sequence.rs` — `AnimationSequence` (chain animations)
- `group.rs` — `AnimationGroup` (parallel animations)
- `builder.rs` — `AnimationBuilder` (fluent API)
- `delegate.rs` — `AnimationDelegate` trait (lifecycle callbacks)
- `preemption.rs` — `PreemptionStrategy` enum

### New: `oriterm/src/gpu/compositor/`
- `mod.rs` — `GpuCompositor` (orchestrates render-to-texture + composition)
- `render_target_pool.rs` — `RenderTargetPool` (texture allocation/reuse)
- `composition_pass.rs` — records composition draw calls

### New: `oriterm/src/gpu/shaders/`
- `composite.wgsl` — composition shader (sample layer texture, apply opacity + transform)

### Modified
- `oriterm_ui/src/lib.rs` — export `compositor` module
- `oriterm_ui/src/animation/mod.rs` — export new animation submodules
- `oriterm_ui/src/overlay/manager.rs` — use compositor layers for overlay lifecycle
- `oriterm/src/gpu/renderer/mod.rs` — integrate compositor into render pipeline
- `oriterm/src/gpu/pipeline/mod.rs` — add composition pipeline
- `oriterm/src/app/redraw/mod.rs` — drive compositor in frame loop
