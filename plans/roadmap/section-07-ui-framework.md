---
section: 7
title: 2D UI Framework
status: in-progress
tier: 2
goal: A lightweight GPU-rendered UI framework on top of wgpu — drawing primitives, layout engine, and widget kit for ori_term's rich cross-platform UI
sections:
  - id: "07.1"
    title: Drawing Primitives
    status: complete
  - id: "07.2"
    title: Text Rendering Integration
    status: complete
  - id: "07.3"
    title: Layout Engine
    status: in-progress
  - id: "07.4"
    title: Hit Testing & Input Routing
    status: complete
  - id: "07.5"
    title: Focus & Keyboard Navigation
    status: in-progress
  - id: "07.6"
    title: Core Widgets
    status: not-started
  - id: "07.7"
    title: Container Widgets
    status: not-started
  - id: "07.8"
    title: Overlay & Modal System
    status: not-started
  - id: "07.9"
    title: Animation
    status: not-started
  - id: "07.10"
    title: Theming & Styling
    status: not-started
  - id: "07.11"
    title: Terminal Grid Widget
    status: not-started
  - id: "07.12"
    title: Section Completion
    status: not-started
---

# Section 07: 2D UI Framework

**Status:** In Progress
**Goal:** Build a lightweight, GPU-rendered 2D UI framework on top of wgpu. This is what makes ori_term fundamentally different from Alacritty, Ghostty, and WezTerm — those terminals have essentially no UI. ori_term has a rich, cross-platform UI with settings panels, controls, command palette, context menus, and more. All GPU-rendered, all consistent across Windows/Linux/macOS.

**Crate:** `oriterm_ui` (created in Section 03.5 with geometry, scale, hit_test, window foundation) — reusable, not coupled to terminal logic
**Dependencies:** `wgpu`, `winit`, `oriterm_core` (for font pipeline)

**Reference:**
- Chrome's Views framework (widget tree, layout, hit testing, focus)
- Flutter's widget/render tree split
- egui's immediate-mode patterns (for inspiration, not architecture — we use retained mode)
- Zed's GPUI framework (GPU-rendered UI in Rust, similar goals)

**Design Principles:**
- Retained-mode widget tree (not immediate-mode — state lives in widgets, not rebuilt every frame)
- Layout is separate from rendering (compute layout once, render many frames until dirty)
- All rendering batched into GPU instance buffers (same pipeline as terminal grid)
- Pixel-perfect across platforms — no native widgets, no platform inconsistencies
- Damage-tracked — only re-layout and re-render what changed

---

## 07.1 Drawing Primitives

The low-level 2D drawing API. Everything visible on screen is drawn through these primitives.

**File:** `oriterm_ui/src/draw.rs`, `oriterm_ui/src/draw/rect.rs`, `oriterm_ui/src/draw/shadow.rs`

- [x] `DrawList` — ordered list of draw commands, batched into GPU instance buffers
  - [x] `push_rect(rect: Rect, style: RectStyle)` — filled rectangle
  - [x] `push_text(pos: Point, shaped: &ShapedText, color: Color)` — pre-shaped text run
  - [x] `push_line(from: Point, to: Point, width: f32, color: Color)` — line segment
  - [x] `push_image(rect: Rect, texture: TextureId, uv: UvRect)` — textured quad
  - [x] `push_clip(rect: Rect)` / `pop_clip()` — scissor rect stack
  - [x] `clear()` — reset for next frame
  - [x] `as_instances() -> (&[u8], usize)` — ready for GPU upload (implemented as `draw_list_convert` module in oriterm, keeping oriterm_ui GPU-agnostic)

- [x] `RectStyle` — how to draw a rectangle
  - [x] `fill: Option<Color>` — solid fill color
  - [x] `border: Option<Border>` — border (width, color, per-side)
  - [x] `corner_radius: [f32; 4]` — per-corner radius (TL, TR, BR, BL)
  - [x] `shadow: Option<Shadow>` — drop shadow (offset, blur, color)
  - [x] `gradient: Option<Gradient>` — linear/radial gradient fill

- [x] `Shadow` — box shadow via blurred rect behind the element
  - [x] `offset: (f32, f32)`, `blur_radius: f32`, `spread: f32`, `color: Color`
  - [x] Rendered as a separate instance with expanded bounds and alpha falloff
  - [x] No multi-pass blur needed — approximate with pre-computed Gaussian texture or SDF

- [x] `Color` — `[f32; 4]` RGBA, with helper constructors
  - [x] `Color::hex(0xRRGGBB)`, `Color::rgba(r, g, b, a)`, `Color::WHITE`, `Color::TRANSPARENT`

- [x] `Point`, `Size`, `Rect`, `Insets` — already established in Section 03.5 (`oriterm_ui/src/geometry.rs`)
  - [x] Extend as needed for drawing (e.g., `Rect` already has `contains`, `intersects`, `inset`, `offset`, `union`, `from_ltrb`)

- [x] **Type-safe coordinate spaces** — add phantom type parameters to geometry types
  - [x] Marker types: `Logical` (device-independent pixels), `Physical` (hardware pixels), `Screen` (screen-absolute)
  - [x] `Point<U = Logical>`, `Size<U = Logical>`, `Rect<U = Logical>` — default parameter preserves all existing code unchanged
  - [x] Manual `Copy`/`Clone`/`Debug`/`PartialEq`/`Default` impls (derive doesn't work with phantom generics — Rust issue #26925)
  - [x] `Insets` stays unit-agnostic (deltas, not positions)
  - [x] `Scale<Src, Dst>` type replaces `ScaleFactor` — encodes conversion direction at type level
    - [x] `Scale::uniform(factor)` for common case, `Scale::new(x, y)` for non-square pixel displays
    - [x] `transform_point(Point<Src>) -> Point<Dst>`, `transform_size`, `transform_rect`
    - [x] `inverse() -> Scale<Dst, Src>` — flips direction
  - [x] Boundary annotations: `hit_test()` takes `Point<Logical>`, GPU submission uses `Point<Physical>`, Win32 FFI uses `Point<Screen>`
  - [x] **Reference:** WezTerm's `PixelUnit`/`ScreenPixelUnit` phantom types, euclid's `Scale<T, Src, Dst>`, Chromium's `dip_util.h` conversion functions
  - [x] **Migration:** incremental — existing code stays on `Point` (= `Point<Logical>`), new boundary code annotates explicitly

- [x] Shader support:
  - [x] Rounded rectangle SDF in fragment shader (same shader, branched on corner_radius > 0)
  - [x] Border rendering via SDF edge detection
  - [x] All primitives batch into the existing instance buffer pipeline (no separate draw calls per shape)

---

## 07.2 Text Rendering Integration

Bridge between the font pipeline (Section 06) and the UI framework.

**File:** `oriterm_ui/src/text.rs`

- [x] `ShapedText` — pre-shaped, ready-to-draw text
  - [x] `glyphs: Vec<ShapedGlyph>` — positioned glyphs from rustybuzz
  - [x] `width: f32` — total advance width
  - [x] `height: f32` — line height
  - [x] `baseline: f32` — baseline offset

- [x] `TextStyle` — how to render text
  - [x] `font_family: Option<String>` — override font (default: UI font)
  - [x] `size: f32` — font size in points
  - [x] `weight: FontWeight` — Regular, Bold, etc.
  - [x] `color: Color`
  - [x] `align: TextAlign` — Left, Center, Right
  - [x] `overflow: TextOverflow` — Clip, Ellipsis, Wrap

- [x] `measure_text(text: &str, style: &TextStyle, max_width: f32) -> TextMetrics`
  - [x] Returns width, height, line count — used by layout engine
  - [x] Does NOT rasterize — only measures

- [x] `shape_text(text: &str, style: &TextStyle, max_width: f32) -> ShapedText`
  - [x] Full shaping via rustybuzz
  - [x] Handles wrapping at word boundaries if `overflow == Wrap`
  - [x] Handles ellipsis truncation if `overflow == Ellipsis`

- [x] UI font vs terminal font:
  - [x] Terminal grid uses the configured monospace font
  - [x] UI elements (buttons, labels, menus) use a proportional UI font
  - [x] Default UI font: system sans-serif (Segoe UI / SF Pro / Cantarell)
  - [x] Both go through the same atlas and shaping pipeline

---

## 07.3 Layout Engine

Flexbox-inspired layout system. Compute positions and sizes for all widgets before rendering.

**File:** `oriterm_ui/src/layout.rs`, `oriterm_ui/src/layout/flex.rs`

- [x] `LayoutNode` — computed layout result for one widget
  - [x] `rect: Rect` — final position and size in screen coordinates
  - [x] `content_rect: Rect` — rect minus padding
  - [x] `children: Vec<LayoutNode>` — child layout results

- [x] `LayoutConstraints` — size constraints passed from parent to child
  - [x] `min_width: f32`, `max_width: f32`
  - [x] `min_height: f32`, `max_height: f32`

- [x] `SizeSpec` enum — how a widget sizes itself (named `SizeSpec`, not `Size`, to avoid collision with geometry `Size`)
  - [x] `Fixed(f32)` — exact pixel size
  - [x] `Fill` — expand to fill available space
  - [x] `FillPortion(u32)` — proportional fill (like CSS flex-grow)
  - [x] `Hug` — shrink to content size
  - [x] Min/Max constraints — handled via `LayoutBox` fields (standard Flutter/Iced pattern)

- [x] `Insets` struct — padding and margin (named `Insets` following Chromium/Flutter convention)
  - [x] `top: f32`, `right: f32`, `bottom: f32`, `left: f32`
  - [x] `Insets::all(v)`, `Insets::vh(v, h)`, `Insets::ZERO`

- [x] Flex layout algorithm:
  - [x] `Direction` — `Row` (horizontal) or `Column` (vertical)
  - [x] `Align` — `Start`, `Center`, `End`, `Stretch` (cross-axis)
  - [x] `Justify` — `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround` (main-axis)
  - [x] `Gap` — spacing between children
  - [x] Two-pass layout:
    1. Measure pass: each child reports preferred size given constraints
    2. Arrange pass: distribute remaining space among `Fill` children
  - [x] Handle `Hug` containers that shrink-wrap their children

- [x] `compute_layout(root: &LayoutBox, viewport: Rect) -> LayoutNode`
  - [x] Top-down constraint propagation, bottom-up size resolution
  - [ ] Cache layout results — only recompute when dirty <!-- blocked-by:7.6 -->

---

## 07.4 Hit Testing & Input Routing

Determine which widget is under the cursor and route mouse/keyboard events.

**File:** `oriterm_ui/src/input/` (event types, hit testing, routing), `oriterm_ui/src/widget_id.rs`

- [x] `layout_hit_test(root: &LayoutNode, point: Point) -> Option<WidgetId>`
  - [x] Walk layout tree back-to-front (last child drawn = frontmost = tested first)
  - [x] Respect clip rects (widget outside clip is not hittable) — via `layout_hit_test_clipped`
  - [x] Return the deepest widget containing the point

- [x] Mouse event routing:
  - [x] `MouseEvent` — `{ kind: MouseEventKind, pos: Point, modifiers: Modifiers }`
  - [x] `MouseEventKind` — `Down`, `Up`, `Move`, `Scroll`; `HoverEvent` — `Enter`, `Leave`
  - [x] Events dispatched to the hit-tested widget via `InputState::process_mouse_event`
  - [x] Hover state tracked: `Enter`/`Leave` generated automatically on cursor movement
  - [x] Capture: widget can capture mouse on `Down`, receives all events until `Up`

- [x] Keyboard event routing:
  - [x] Events go to the focused widget — `InputState::keyboard_target(focus)` returns focused WidgetId
  - [x] Unhandled events bubble up to parent — caller responsibility (documented contract)
  - [x] `KeyEvent` — reuse winit's `KeyEvent` structure (no wrapper needed)

- [x] Event response:
  - [x] `EventResponse` — `Handled`, `Ignored`, `RequestFocus`, `RequestRedraw`
  - [x] Widgets return response to indicate whether they consumed the event

---

## 07.5 Focus & Keyboard Navigation

Focus ring for keyboard-driven UI navigation.

**File:** `oriterm_ui/src/focus/mod.rs`

- [x] `FocusManager` — tracks which widget has keyboard focus
  - [x] `focused: Option<WidgetId>`
  - [x] `focus_order: Vec<WidgetId>` — tab order (built from widget tree traversal)
  - [x] `set_focus(id: WidgetId)`
  - [x] `clear_focus()`
  - [x] `focus_next()` — Tab key advances focus
  - [x] `focus_prev()` — Shift+Tab moves focus backward

- [ ] Focus visual: <!-- blocked-by:7.6 -->
  - [ ] Focused widget renders a focus ring (2px outline, accent color) <!-- blocked-by:7.6 -->
  - [ ] Optional per-widget: `focusable: bool` <!-- blocked-by:7.6 -->

- [ ] Keyboard shortcuts: <!-- blocked-by:7.6 -->
  - [ ] `Tab` / `Shift+Tab` — cycle focus <!-- blocked-by:7.6 -->
  - [ ] `Enter` / `Space` — activate focused button/checkbox <!-- blocked-by:7.6 -->
  - [ ] `Escape` — close overlay, unfocus <!-- blocked-by:7.6 -->
  - [ ] `Arrow keys` — navigate within lists, dropdowns <!-- blocked-by:7.6 -->

---

## 07.6 Core Widgets

The basic building blocks.

**File:** `oriterm_ui/src/widgets/` — one file per widget

### Label
- [ ] Static or dynamic text display
- [ ] `LabelWidget { text: String, style: TextStyle }`
- [ ] Supports single-line and multi-line
- [ ] Ellipsis truncation when constrained

### Button
- [ ] `ButtonWidget { label: String, on_click: Callback, style: ButtonStyle }`
- [ ] States: Default, Hover, Pressed, Disabled, Focused
- [ ] Visual: rounded rect background, centered text, hover highlight
- [ ] Keyboard: activatable via Enter/Space when focused

### Checkbox
- [ ] `CheckboxWidget { checked: bool, label: String, on_toggle: Callback }`
- [ ] Visual: box with checkmark, label to the right
- [ ] Keyboard: toggle via Space when focused

### Toggle
- [ ] `ToggleWidget { on: bool, on_toggle: Callback }`
- [ ] Visual: sliding pill (iOS-style toggle)
- [ ] Animated transition between on/off states

### Slider
- [ ] `SliderWidget { value: f32, min: f32, max: f32, on_change: Callback }`
- [ ] Visual: track with draggable thumb
- [ ] Keyboard: arrow keys adjust value

### Text Input
- [ ] `TextInputWidget { text: String, placeholder: String, on_change: Callback }`
- [ ] Single-line text entry with cursor, selection, copy/paste
- [ ] Visual: bordered rect, blinking cursor, selection highlight

### Dropdown
- [ ] `DropdownWidget { items: Vec<String>, selected: usize, on_select: Callback }`
- [ ] Visual: button that opens a floating list
- [ ] Uses overlay system (07.8) for the dropdown list

### Separator
- [ ] Horizontal or vertical line with optional label
- [ ] `SeparatorWidget { direction: Direction, label: Option<String> }`

---

## 07.7 Container Widgets

Widgets that contain and arrange other widgets.

**File:** `oriterm_ui/src/widgets/containers.rs`

### Row / Column (Flex Container)
- [ ] `FlexWidget { direction: Direction, children: Vec<Widget>, gap: f32, align: Align, justify: Justify }`
- [ ] The primary layout container — everything is nested Rows and Columns
- [ ] Delegates to the flex layout algorithm (07.3)

### Scroll Container
- [ ] `ScrollWidget { child: Widget, scroll_offset: f32, direction: ScrollDirection }`
- [ ] Clips child to container bounds
- [ ] Scrollbar: thin overlay scrollbar (appears on hover/scroll, fades out)
- [ ] Mouse wheel scrolling, trackpad smooth scroll
- [ ] Keyboard: PageUp/PageDown, Home/End

### Panel
- [ ] `PanelWidget { child: Widget, style: PanelStyle }`
- [ ] Visual container with background, border, rounded corners, shadow
- [ ] Used for settings panels, dialog backgrounds, card-style layouts

### Spacer
- [ ] `SpacerWidget { size: Size }` — flexible or fixed empty space
- [ ] `Spacer::fill()` — pushes siblings to opposite ends

### Stack (Z-axis)
- [ ] `StackWidget { children: Vec<Widget> }` — children overlaid on top of each other
- [ ] Used for positioning elements absolutely within a relative container
- [ ] Last child is frontmost

---

## 07.8 Overlay & Modal System

Floating UI that renders above the main widget tree.

**File:** `oriterm_ui/src/overlay.rs`

- [ ] `OverlayManager` — manages floating layers above the main content
  - [ ] `overlays: Vec<Overlay>` — stack of active overlays (frontmost = last)
  - [ ] `push_overlay(widget: Widget, anchor: Rect, placement: Placement) -> OverlayId`
  - [ ] `pop_overlay(id: OverlayId)`
  - [ ] `clear_all()`

- [ ] `Placement` — where to position the overlay relative to its anchor
  - [ ] `Below`, `Above`, `Left`, `Right` — auto-flip if insufficient space
  - [ ] `Center` — centered on screen (for modals)
  - [ ] `AtCursor` — positioned at mouse cursor (for context menus)

- [ ] Overlay rendering:
  - [ ] Overlays render after the main widget tree (on top)
  - [ ] Background dimming for modals (semi-transparent black layer)
  - [ ] Click-outside-to-dismiss behavior for non-modal overlays

- [ ] Rich overlay content — overlays can contain any widget, not just text:
  - [ ] Terminal preview thumbnails (scaled-down live terminal renders)
  - [ ] Image previews
  - [ ] Multi-line formatted content

- [ ] Used by:
  - [ ] Context menus (right-click)
  - [ ] Dropdown lists
  - [ ] Command palette
  - [ ] Settings panel
  - [ ] Tooltips
  - [ ] Search bar
  - [ ] **Tab hover previews** (Chrome/Windows-style terminal thumbnails)

---

## 07.9 Animation

Smooth transitions for UI state changes.

**File:** `oriterm_ui/src/animation.rs`

- [ ] `Animation` — interpolates a value over time
  - [ ] `from: f32`, `to: f32`, `duration: Duration`, `easing: Easing`
  - [ ] `progress(now: Instant) -> f32` — returns current interpolated value
  - [ ] `is_finished(now: Instant) -> bool`

- [ ] `Easing` — timing functions
  - [ ] `Linear`, `EaseIn`, `EaseOut`, `EaseInOut`
  - [ ] Cubic bezier for custom curves

- [ ] `AnimatedValue<T>` — wrapper that animates between old and new values
  - [ ] `set(new_value: T)` — starts animation from current to new
  - [ ] `get(now: Instant) -> T` — returns interpolated value
  - [ ] Triggers redraw while animation is in progress

- [ ] Used for:
  - [ ] Toggle switch sliding
  - [ ] Hover color transitions
  - [ ] Overlay fade-in/fade-out
  - [ ] Tab bar tab sliding
  - [ ] Scroll position smooth scrolling

---

## 07.10 Theming & Styling

Consistent visual styling across all widgets.

**File:** `oriterm_ui/src/theme.rs`

- [ ] `UiTheme` — all UI colors, sizes, and spacing in one struct
  - [ ] `bg_primary: Color` — main background
  - [ ] `bg_secondary: Color` — panel/card background
  - [ ] `bg_hover: Color` — hover highlight
  - [ ] `bg_active: Color` — pressed/active state
  - [ ] `fg_primary: Color` — primary text
  - [ ] `fg_secondary: Color` — secondary/dimmed text
  - [ ] `fg_disabled: Color` — disabled state text
  - [ ] `accent: Color` — accent color (focus ring, toggle on, selection)
  - [ ] `border: Color` — default border color
  - [ ] `shadow: Color` — shadow color (semi-transparent black)
  - [ ] `corner_radius: f32` — default corner radius
  - [ ] `spacing: f32` — default gap between elements
  - [ ] `font_size: f32` — default UI font size
  - [ ] `font_size_small: f32` — small text
  - [ ] `font_size_large: f32` — headings

- [ ] `UiTheme::dark() -> Self` — dark theme defaults
- [ ] `UiTheme::light() -> Self` — light theme defaults
- [ ] Theme propagates through the widget tree (widgets inherit from parent unless overridden)
- [ ] Integrates with Section 03 system theme detection (auto dark/light)

---

## 07.11 Terminal Grid Widget

The terminal grid itself is a widget within the UI framework.

**File:** `oriterm_ui/src/widgets/terminal_grid.rs`

- [ ] `TerminalGridWidget` — renders a terminal's visible content
  - [ ] Takes `RenderableContent` from `oriterm_core`
  - [ ] Renders cell backgrounds and glyphs using the drawing primitives
  - [ ] Handles its own cursor rendering, selection highlight, search highlight
  - [ ] Reports its preferred size based on cell dimensions and grid size
  - [ ] **Supports rendering to any target** — screen surface OR offscreen texture
  - [ ] Accepts optional `scale` parameter for thumbnail rendering (e.g. 0.25x for previews)

- [ ] `TerminalPreviewWidget` — scaled-down live preview of a terminal tab
  - [ ] Renders a terminal at thumbnail resolution to an offscreen texture (Section 05 render targets)
  - [ ] Displayed in an overlay on tab hover (Chrome/Windows-style tab preview)
  - [ ] Re-renders only when the source terminal's content is dirty
  - [ ] Configurable preview size (e.g. 320x200 logical pixels)
  - [ ] Rounded corners, subtle shadow, smooth fade-in animation
  - [ ] Used by: tab bar hover, taskbar window preview, window switcher

- [ ] Integration:
  - [ ] The main window layout is: `Column { TabBar, TerminalGrid, StatusBar(optional) }`
  - [ ] The terminal grid fills the remaining space after UI chrome
  - [ ] Grid receives keyboard input when focused (which is the default state)
  - [ ] Mouse events within the grid are routed to terminal mouse handling

- [ ] This means the tab bar, context menus, settings, search overlay, terminal previews, and terminal grid ALL go through the same rendering pipeline — consistent, composable, GPU-accelerated

---

## 07.12 Section Completion

- [ ] All 07.1-07.11 items complete
- [ ] Drawing primitives render correctly: rects, rounded rects, shadows, text, lines
- [ ] Layout engine computes correct positions for nested flex containers
- [ ] Hit testing correctly identifies the widget under the cursor
- [ ] Focus management: Tab cycles through focusable widgets
- [ ] Core widgets render and respond to input: Button, Checkbox, Toggle, Slider, TextInput, Dropdown
- [ ] Overlays render above main content, dismiss on click-outside
- [ ] Animations interpolate smoothly (no jank, no allocation per frame)
- [ ] Theme system provides consistent dark/light styling
- [ ] Terminal grid renders as a widget within the framework
- [ ] Tab bar renders as a widget within the framework
- [ ] All widgets are GPU-rendered — no native OS widgets used
- [ ] Performance: UI framework adds negligible overhead to frame time
- [ ] No platform-specific code in the UI framework (pure Rust + wgpu)
- [ ] `cargo clippy -p oriterm_ui` — no warnings

**Exit Criteria:** A complete, lightweight, GPU-rendered UI framework that can build settings panels, context menus, command palette, and any future UI. The terminal grid is just another widget. All rendering is consistent, cross-platform, and fast.
