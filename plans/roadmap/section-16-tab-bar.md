---
section: 16
title: Tab Bar & Chrome
status: in-progress
tier: 4
goal: Tab bar layout, rendering, and hit testing with DPI awareness
sections:
  - id: "16.1"
    title: Tab Bar Layout + Constants
    status: complete
  - id: "16.2"
    title: Tab Bar Rendering
    status: complete
  - id: "16.3"
    title: Tab Bar Hit Testing
    status: in-progress
  - id: "16.4"
    title: Section Completion
    status: in-progress
---

# Section 16: Tab Bar & Chrome

**Status:** Not Started
**Goal:** Tab bar layout, rendering, and hit testing with DPI awareness. Deterministic layout computation, GPU-rendered tab bar with bell pulse animation and drag overlay, and priority-based hit testing for click/hover dispatch.

**Crate:** `oriterm` (binary only — no core changes)
**Dependencies:** `wgpu`, `winit`
**Reference:** `_old/src/tab_bar.rs`, `_old/src/gpu/render_tab_bar.rs`, `_old/src/gpu/render_overlay.rs`

**Prerequisite:** Section 13 complete (Tab struct and management operations available).

---

## 16.1 Tab Bar Layout + Constants

Compute the pixel layout of tabs in the tab bar. All measurements are DPI-scaled. The layout is deterministic — given tab count, window width, and scale factor, the output is identical.

**File:** `oriterm_ui/src/widgets/tab_bar/` (constants, layout, colors modules)

**Reference:** `_old/src/tab_bar.rs`

**Deviation:** Layout computes in logical pixels (matching `ChromeLayout` pattern); scale applied at render boundary. Colors use `oriterm_ui::color::Color` (not `[f32; 4]`) and derive from `UiTheme` (not `Palette`), matching existing widget conventions. `window_width: f32` stored instead of `scale: f64` since scale is not needed for logical-pixel layout.

- [x] Layout constants (all in logical pixels, multiply by `scale_factor` for physical):
  - [x] `TAB_BAR_HEIGHT: f32 = 46.0` — full height of the tab bar
  - [x] `TAB_MIN_WIDTH: f32 = 80.0` — minimum tab width before they start overlapping
  - [x] `TAB_MAX_WIDTH: f32 = 260.0` — maximum tab width (tabs grow to fill available space, clamped here)
  - [x] `TAB_LEFT_MARGIN: f32 = 16.0` — padding before the first tab
  - [x] `TAB_PADDING: f32 = 8.0` — internal horizontal padding within each tab
  - [x] `CLOSE_BUTTON_WIDTH: f32 = 24.0` — clickable area for the x button
  - [x] `CLOSE_BUTTON_RIGHT_PAD: f32 = 8.0` — spacing between x button and tab's right edge
  - [x] `NEW_TAB_BUTTON_WIDTH: f32 = 38.0` — width of the "+" button
  - [x] `DROPDOWN_BUTTON_WIDTH: f32 = 30.0` — width of the dropdown (settings/scheme) button
  - [x] `CONTROLS_ZONE_WIDTH` — platform-specific:
    - [x] Windows: `174.0` (three 58px buttons: minimize, maximize, close)
    - [x] Linux/macOS: `100.0` (three circular buttons with spacing)
  - [x] `DRAG_START_THRESHOLD: f32 = 10.0` — pixels of movement before drag begins (matches Chrome's `tab_drag_controller.cc`)
  - [x] `TEAR_OFF_THRESHOLD: f32 = 40.0` — pixels outside tab bar before tear-off
  - [x] `TEAR_OFF_THRESHOLD_UP: f32 = 15.0` — reduced threshold for upward dragging (more natural for tear-off)
- [x] `TabBarLayout` struct:
  - [x] `tab_width: f32` — computed width per tab (all tabs same width)
  - [x] `tab_count: usize` — number of tabs
  - [x] `window_width: f32` — window width used for layout (replaces `scale: f64` since layout is in logical pixels)
- [x] `TabBarLayout::compute(tab_count: usize, window_width: f32, tab_width_lock: Option<f32>) -> Self`
  - [x] If `tab_width_lock` is `Some(w)`: use locked width (prevents jitter during rapid close clicks or drag)
  - [x] Available width = `window_width - TAB_LEFT_MARGIN - NEW_TAB_BUTTON_WIDTH - DROPDOWN_BUTTON_WIDTH - CONTROLS_ZONE_WIDTH`
  - [x] `tab_width = (available / tab_count).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)`
  - [x] Return layout struct
- [x] `tab_width_lock: Option<f32>` on App:
  - [x] **Acquired** when: cursor enters tab bar (hovering), prevents tabs from expanding when quickly closing tabs
  - [x] **Released** when: cursor leaves tab bar, window resizes, tab count changes in ways that invalidate the lock (new tab, drag reorder)
  - [x] Purpose: If you have 5 tabs and close one, the remaining 4 tabs would normally expand. But if you're rapidly clicking close buttons, the expansion moves the next close button, causing you to miss. The lock freezes tab width during hover, so close buttons don't move.
- [x] `TabBarColors` struct — all colors needed for tab bar rendering:
  - [x] `bar_bg: Color` — tab bar background
  - [x] `active_bg: Color` — active tab background (rendered with rounded corners)
  - [x] `inactive_bg: Color` — inactive tab background
  - [x] `tab_hover_bg: Color` — inactive tab background on hover
  - [x] `text_fg: Color` — active tab title text
  - [x] `inactive_text: Color` — inactive tab title text (dimmer)
  - [x] `separator: Color` — 1px vertical separator between tabs
  - [x] `close_fg: Color` — close button color (unhovered)
  - [x] `button_hover_bg: Color` — "+" and dropdown hover background
  - [x] `control_hover_bg: Color` — window control button hover
  - [x] `control_fg: Color` — window control icon color
  - [x] `control_fg_dim: Color` — dimmed window control icon
  - [x] `control_close_hover_bg: Color` — close button red hover (platform standard)
  - [x] `control_close_hover_fg: Color` — close button text on red hover (white)
  - [x] Derived from theme: `TabBarColors::from_theme(theme: &UiTheme) -> Self`

---

## 16.2 Tab Bar Rendering

Render the tab bar as GPU instances. The tab bar is rendered in the overlay pass, after the terminal grid bg+fg passes. The dragged tab is rendered separately in a second overlay pass so it floats above everything.

**File:** `oriterm_ui/src/widgets/tab_bar/widget/` (TabBarWidget + draw), `oriterm/src/app/redraw.rs` (pipeline integration)

**Reference:** `_old/src/gpu/render_tab_bar.rs`, `_old/src/gpu/render_overlay.rs`

**Deviation:** Implemented as `TabBarWidget` (Widget→DrawList→GPU pipeline) rather than `build_tab_bar_instances()` with `InstanceWriter`. The architecture evolved after the plan was written — widgets draw to a `DrawList` in logical pixels, which is converted to GPU instances at the rendering boundary. `drag_visual_x` simplified from `Option<(WindowId, f32)>` to `Option<(usize, f32)>` on the widget (single window). Animation offsets simplified from `HashMap<WindowId, Vec<f32>>` to `Vec<f32>` on the widget. Window control buttons (item 7) already handled by `WindowChromeWidget`.

- [x] `build_tab_bar_instances()` — primary rendering function
  - [x] Input: `InstanceWriter` (bg + fg), `FrameParams`, `TabBarColors`, `FontCollection`, `wgpu::Queue`
  - [x] Output: populated instance buffers ready for GPU submission
- [x] Rendering order (draw order matters for layering):
  1. [x] Tab bar background: full-width rectangle across top of window
  2. [x] Inactive tabs (drawn first, behind active tab):
     - [x] Background rectangle (with hover color if `hover_hit == Tab(idx)`)
     - [x] Title text: shaped with UI font collection, truncated with ellipsis if too wide
     - [x] Close button: vector x icon (visible on hover only, or always — configurable)
  3. [x] Active tab (drawn on top of inactive tabs):
     - [x] Background rectangle with **rounded top corners** (radius ~8px x scale)
     - [x] Title text: brighter color than inactive
     - [x] Close button: always visible
  4. [x] Separators: 1px vertical lines between tabs, with **suppression rules**:
     - [x] No separator adjacent to active tab (left or right edge)
     - [x] No separator adjacent to hovered tab
     - [x] No separator adjacent to dragged tab
  5. [x] New tab "+" button: after the last tab
  6. [x] Dropdown button: after "+" button
  7. [x] Window control buttons: rightmost (see section 16)
- [x] Bell badge animation:
  - [x] `bell_phase: f32` (0.0–1.0) — sine wave pulse
  - [x] Inactive tab with bell: `lerp_color(inactive_bg, tab_hover_bg, bell_phase)` — smooth pulsing background
  - [x] Phase computed from `bell_start: Option<Instant>` on the tab's terminal state
  - [x] Clear badge when tab becomes active
- [x] Dragged tab overlay:
  - [x] When dragging: the dragged tab is **not rendered in the normal tab bar pass**
  - [x] Instead, rendered in a separate overlay pass via `build_dragged_tab_overlay()`
  - [x] Rendering:
    1. Opaque backing rect (hides underlying text from fg pass)
    2. Rounded tab shape with active background
    3. Tab content (text + close button) at `drag_visual_x` position
  - [x] "+" and dropdown buttons reposition during drag: `max(default_x, drag_x + tab_w)` — keeps buttons visible even when dragging far right
- [x] `drag_visual_x: Option<(WindowId, f32)>` on App:
  - [x] The pixel X position where the dragged tab is drawn
  - [x] Separate from the tab's actual index in the vec — allows smooth visual feedback without real-time list manipulation
  - [x] Updated on every mouse move during drag
- [x] Tab animation offsets:
  - [x] `tab_anim_offsets: HashMap<WindowId, Vec<f32>>` — per-tab pixel offsets for smooth transitions
  - [x] When tabs reorder during drag: displaced tabs get a non-zero offset that decays to 0 over ~100ms
  - [x] `decay_tab_animations(&mut self) -> bool` — returns true if any animation is still active (needs continued rendering)
  - [x] Chrome-style behavior: tabs **snap immediately** to new positions during drag. Animation only applies on drag-end.
- [x] Tab title rendering: <!-- unblocks:6.13 -->
  - [x] Use UI font collection (separate from terminal font, possibly different family/weight)
  - [x] `ui_collection.truncate_to_pixel_width(title, max_text_px)` — truncates with `...` (U+2026) if too wide
  - [x] Max text width = `tab_width - 2*TAB_PADDING - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD`

---

## 16.3 Tab Bar Hit Testing

Map mouse coordinates to tab bar actions. Hit testing determines whether a click or hover targets a tab, a button, or the drag area.

**File:** `oriterm/src/chrome/tab_bar.rs`

**Reference:** `_old/src/tab_bar.rs`

- [x] `TabBarHit` enum:
  - [x] `Tab(usize)` — clicked on tab at index
  - [x] `CloseTab(usize)` — clicked close button on tab at index
  - [x] `NewTab` — clicked the "+" button
  - [x] `DropdownButton` — clicked the dropdown/settings button (named `Dropdown` in code)
  - [x] `Minimize` — clicked window minimize
  - [x] `Maximize` — clicked window maximize/restore
  - [x] `CloseWindow` — clicked window close
  - [x] `DragArea` — clicked empty tab bar area (for window dragging or double-click maximize)
  - [x] `None` — click is below tab bar (terminal area)
- [x] `hit_test(x: f32, y: f32, layout: &TabBarLayout) -> TabBarHit` (logical pixels, no scale param)
  - [x] Priority order (checked first = higher priority):
    1. [x] If `y` outside `0..TAB_BAR_HEIGHT`: return `None` (below/above tab bar)
    2. [x] Check window controls zone (rightmost):
       - [x] **Windows**: three `CONTROL_BUTTON_WIDTH` buttons, left-to-right: Minimize, Maximize, Close
       - [x] **Linux/macOS**: three circular buttons (24px diameter, 8px spacing, 12px margins)
       - [x] Return `CloseWindow`, `Maximize`, or `Minimize`
    3. [x] Check tabs region (starts at `TAB_LEFT_MARGIN`):
       - [x] For each tab: check close button rect **first** (inset from right edge)
       - [x] Then check tab rect — return `Tab(idx)`
    4. [x] Check new-tab button (after last tab)
    5. [x] Check dropdown button (after new-tab button)
    6. [x] If still within tab bar height: return `DragArea`
- [x] Tab bar hover tracking:
  - [x] `hover_hit` on `TabBarWidget` (updated via `set_hover_hit`)
  - [x] Updated on every `CursorMoved` event (via `update_tab_bar_hover`)
  - [x] When hover changes: mark dirty, request redraw
  - [x] Hover entering tab bar: acquire `tab_width_lock`
  - [x] Hover leaving tab bar: release `tab_width_lock`
- [ ] Tab hover preview (Chrome/Windows-style): <!-- blocked-by:7 -->
  - [ ] When hovering an inactive tab for > 300ms, show a `TerminalPreviewWidget` overlay
  - [ ] Preview appears below the tab bar, anchored to the hovered tab
  - [ ] Preview shows a live scaled-down render of that tab's terminal content
  - [ ] Uses offscreen render target (Section 05) + `TerminalPreviewWidget` (Section 07)
  - [ ] Fade-in animation (07.9), dismiss on hover leave
  - [ ] Preview updates if the terminal content changes while hovering
  - [ ] No preview for the active tab (it's already visible)
- [x] Mouse press dispatch (in `try_tab_bar_mouse`):
  - [x] `Tab(idx)`: consumes click (multi-tab switching deferred to Section 15/30) <!-- blocked-by:30 -->
  - [x] `CloseTab(idx)`: acquire `tab_width_lock`, close tab
  - [x] `NewTab`: consumes click (tab creation deferred to Section 15/30) <!-- blocked-by:30 -->
  - [x] `DropdownButton`: consumes click (dropdown menu deferred to Section 21) <!-- blocked-by:21 -->
  - [x] `Minimize`: `window.set_minimized(true)`
  - [x] `Maximize`: toggle `window.set_maximized()`
  - [x] `CloseWindow`: close window
  - [x] `DragArea`:
    - [x] Double-click: toggle maximize (500ms threshold)
    - [x] Single-click: start window drag via `window.drag_window()`

---

## 16.4 Section Completion

- [ ] All 16.1–16.3 items complete
- [x] Tab bar layout: DPI-aware, width lock, platform-specific control zone
- [x] Tab bar rendering: separators with suppression, bell pulse, dragged tab overlay, animation offsets
- [x] Hit testing: correct priority order, close button inset, platform-specific controls
- [x] Tab width lock prevents close button shifting during rapid close clicks
- [x] `cargo build -p oriterm --target x86_64-pc-windows-gnu` — compiles
- [x] `cargo clippy -p oriterm --target x86_64-pc-windows-gnu` — no warnings
- [x] **Close stress test**: rapidly close many tabs while hovering tab bar — close buttons don't shift unexpectedly (tab width lock works)
- [ ] **Visual test**: tab bar renders correctly at 100%, 125%, 150%, 200% DPI scales

**Exit Criteria:** Tab bar layout computes deterministically for any tab count and window width. GPU-rendered tab bar includes bell animation, drag overlay, and separator suppression. Hit testing dispatches clicks with correct priority ordering.
