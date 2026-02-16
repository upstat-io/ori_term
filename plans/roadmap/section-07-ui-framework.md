---
section: 7
title: 2D UI Framework
status: not-started
tier: 2
goal: A lightweight GPU-rendered UI framework on top of wgpu — drawing primitives, layout engine, and widget kit for ori_term's rich cross-platform UI
sections:
  - id: "07.1"
    title: Drawing Primitives
    status: not-started
  - id: "07.2"
    title: Text Rendering Integration
    status: not-started
  - id: "07.3"
    title: Layout Engine
    status: not-started
  - id: "07.4"
    title: Hit Testing & Input Routing
    status: not-started
  - id: "07.5"
    title: Focus & Keyboard Navigation
    status: not-started
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

**Status:** Not Started
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

- [ ] `DrawList` — ordered list of draw commands, batched into GPU instance buffers
  - [ ] `push_rect(rect: Rect, style: RectStyle)` — filled rectangle
  - [ ] `push_text(pos: Point, shaped: &ShapedText, color: Color)` — pre-shaped text run
  - [ ] `push_line(from: Point, to: Point, width: f32, color: Color)` — line segment
  - [ ] `push_image(rect: Rect, texture: TextureId, uv: UvRect)` — textured quad
  - [ ] `push_clip(rect: Rect)` / `pop_clip()` — scissor rect stack
  - [ ] `clear()` — reset for next frame
  - [ ] `as_instances() -> (&[u8], usize)` — ready for GPU upload

- [ ] `RectStyle` — how to draw a rectangle
  - [ ] `fill: Option<Color>` — solid fill color
  - [ ] `border: Option<Border>` — border (width, color, per-side)
  - [ ] `corner_radius: [f32; 4]` — per-corner radius (TL, TR, BR, BL)
  - [ ] `shadow: Option<Shadow>` — drop shadow (offset, blur, color)
  - [ ] `gradient: Option<Gradient>` — linear/radial gradient fill

- [ ] `Shadow` — box shadow via blurred rect behind the element
  - [ ] `offset: (f32, f32)`, `blur_radius: f32`, `spread: f32`, `color: Color`
  - [ ] Rendered as a separate instance with expanded bounds and alpha falloff
  - [ ] No multi-pass blur needed — approximate with pre-computed Gaussian texture or SDF

- [ ] `Color` — `[f32; 4]` RGBA, with helper constructors
  - [ ] `Color::hex(0xRRGGBB)`, `Color::rgba(r, g, b, a)`, `Color::WHITE`, `Color::TRANSPARENT`

- [ ] `Point`, `Size`, `Rect`, `Insets` — already established in Section 03.5 (`oriterm_ui/src/geometry.rs`)
  - [ ] Extend as needed for drawing (e.g., `Rect` already has `contains`, `intersects`, `inset`, `offset`, `union`)

- [ ] Shader support:
  - [ ] Rounded rectangle SDF in fragment shader (same shader, branched on corner_radius > 0)
  - [ ] Border rendering via SDF edge detection
  - [ ] All primitives batch into the existing instance buffer pipeline (no separate draw calls per shape)

---

## 07.2 Text Rendering Integration

Bridge between the font pipeline (Section 06) and the UI framework.

**File:** `oriterm_ui/src/text.rs`

- [ ] `ShapedText` — pre-shaped, ready-to-draw text
  - [ ] `glyphs: Vec<ShapedGlyph>` — positioned glyphs from rustybuzz
  - [ ] `width: f32` — total advance width
  - [ ] `height: f32` — line height
  - [ ] `baseline: f32` — baseline offset

- [ ] `TextStyle` — how to render text
  - [ ] `font_family: Option<String>` — override font (default: UI font)
  - [ ] `size: f32` — font size in points
  - [ ] `weight: FontWeight` — Regular, Bold, etc.
  - [ ] `color: Color`
  - [ ] `align: TextAlign` — Left, Center, Right
  - [ ] `overflow: TextOverflow` — Clip, Ellipsis, Wrap

- [ ] `measure_text(text: &str, style: &TextStyle, max_width: f32) -> TextMetrics`
  - [ ] Returns width, height, line count — used by layout engine
  - [ ] Does NOT rasterize — only measures

- [ ] `shape_text(text: &str, style: &TextStyle, max_width: f32) -> ShapedText`
  - [ ] Full shaping via rustybuzz
  - [ ] Handles wrapping at word boundaries if `overflow == Wrap`
  - [ ] Handles ellipsis truncation if `overflow == Ellipsis`

- [ ] UI font vs terminal font:
  - [ ] Terminal grid uses the configured monospace font
  - [ ] UI elements (buttons, labels, menus) use a proportional UI font
  - [ ] Default UI font: system sans-serif (Segoe UI / SF Pro / Cantarell)
  - [ ] Both go through the same atlas and shaping pipeline

---

## 07.3 Layout Engine

Flexbox-inspired layout system. Compute positions and sizes for all widgets before rendering.

**File:** `oriterm_ui/src/layout.rs`, `oriterm_ui/src/layout/flex.rs`

- [ ] `LayoutNode` — computed layout result for one widget
  - [ ] `rect: Rect` — final position and size in screen coordinates
  - [ ] `content_rect: Rect` — rect minus padding
  - [ ] `children: Vec<LayoutNode>` — child layout results

- [ ] `LayoutConstraints` — size constraints passed from parent to child
  - [ ] `min_width: f32`, `max_width: f32`
  - [ ] `min_height: f32`, `max_height: f32`

- [ ] `Size` enum — how a widget sizes itself
  - [ ] `Fixed(f32)` — exact pixel size
  - [ ] `Fill` — expand to fill available space
  - [ ] `FillPortion(u32)` — proportional fill (like CSS flex-grow)
  - [ ] `Hug` — shrink to content size
  - [ ] `Min(f32)` / `Max(f32)` — constrained

- [ ] `Spacing` struct — padding and margin
  - [ ] `top: f32`, `right: f32`, `bottom: f32`, `left: f32`
  - [ ] `Spacing::all(v)`, `Spacing::xy(h, v)`, `Spacing::ZERO`

- [ ] Flex layout algorithm:
  - [ ] `Direction` — `Row` (horizontal) or `Column` (vertical)
  - [ ] `Align` — `Start`, `Center`, `End`, `Stretch` (cross-axis)
  - [ ] `Justify` — `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround` (main-axis)
  - [ ] `Gap` — spacing between children
  - [ ] Two-pass layout:
    1. Measure pass: each child reports preferred size given constraints
    2. Arrange pass: distribute remaining space among `Fill` children
  - [ ] Handle `Hug` containers that shrink-wrap their children

- [ ] `compute_layout(root: &WidgetTree, viewport: Rect) -> LayoutTree`
  - [ ] Top-down constraint propagation, bottom-up size resolution
  - [ ] Cache layout results — only recompute when dirty

---

## 07.4 Hit Testing & Input Routing

Determine which widget is under the cursor and route mouse/keyboard events.

**File:** `oriterm_ui/src/input.rs`, `oriterm_ui/src/hit_test.rs`

- [ ] `hit_test(layout: &LayoutTree, point: Point) -> Option<WidgetId>`
  - [ ] Walk layout tree back-to-front (last child drawn = frontmost = tested first)
  - [ ] Respect clip rects (widget outside clip is not hittable)
  - [ ] Return the deepest widget containing the point

- [ ] Mouse event routing:
  - [ ] `MouseEvent` — `{ kind: MouseEventKind, pos: Point, button: MouseButton, modifiers: Modifiers }`
  - [ ] `MouseEventKind` — `Down`, `Up`, `Move`, `Scroll`, `Enter`, `Leave`
  - [ ] Events dispatched to the hit-tested widget
  - [ ] Hover state tracked: `Enter`/`Leave` generated automatically on cursor movement
  - [ ] Capture: widget can capture mouse on `Down`, receives all events until `Up`

- [ ] Keyboard event routing:
  - [ ] Events go to the focused widget (see 07.5)
  - [ ] Unhandled events bubble up to parent
  - [ ] `KeyEvent` — reuse winit's `KeyEvent` structure

- [ ] Event response:
  - [ ] `EventResponse` — `Handled`, `Ignored`, `RequestFocus`, `RequestRedraw`
  - [ ] Widgets return response to indicate whether they consumed the event

---

## 07.5 Focus & Keyboard Navigation

Focus ring for keyboard-driven UI navigation.

**File:** `oriterm_ui/src/focus.rs`

- [ ] `FocusManager` — tracks which widget has keyboard focus
  - [ ] `focused: Option<WidgetId>`
  - [ ] `focus_order: Vec<WidgetId>` — tab order (built from widget tree traversal)
  - [ ] `set_focus(id: WidgetId)`
  - [ ] `clear_focus()`
  - [ ] `focus_next()` — Tab key advances focus
  - [ ] `focus_prev()` — Shift+Tab moves focus backward

- [ ] Focus visual:
  - [ ] Focused widget renders a focus ring (2px outline, accent color)
  - [ ] Optional per-widget: `focusable: bool`

- [ ] Keyboard shortcuts:
  - [ ] `Tab` / `Shift+Tab` — cycle focus
  - [ ] `Enter` / `Space` — activate focused button/checkbox
  - [ ] `Escape` — close overlay, unfocus
  - [ ] `Arrow keys` — navigate within lists, dropdowns

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
