//! Layout constants for the tab bar.
//!
//! All dimensions are in logical pixels. The caller multiplies by the window's
//! scale factor to convert to physical pixels for rendering. Constants follow
//! Chrome's tab bar proportions adapted for a terminal emulator.

/// Full height of the tab bar in logical pixels.
pub const TAB_BAR_HEIGHT: f32 = 46.0;

/// Minimum tab width before tabs start overlapping.
pub const TAB_MIN_WIDTH: f32 = 80.0;

/// Maximum tab width (tabs grow to fill available space, clamped here).
pub const TAB_MAX_WIDTH: f32 = 260.0;

/// Horizontal margin before the first tab.
pub const TAB_LEFT_MARGIN: f32 = 16.0;

/// Vertical margin between the top of the window and the top of tabs.
pub const TAB_TOP_MARGIN: f32 = 8.0;

/// Internal horizontal padding within each tab.
pub const TAB_PADDING: f32 = 8.0;

/// Clickable area width for the tab close (x) button.
pub const CLOSE_BUTTON_WIDTH: f32 = 24.0;

/// Spacing between the close button and the tab's right edge.
pub const CLOSE_BUTTON_RIGHT_PAD: f32 = 8.0;

/// Width of the new-tab "+" button.
pub const NEW_TAB_BUTTON_WIDTH: f32 = 38.0;

/// Width of the dropdown (settings/scheme) button.
pub const DROPDOWN_BUTTON_WIDTH: f32 = 30.0;

// Platform-specific window control button dimensions.

/// Total width reserved for window control buttons on Windows.
///
/// Derived from the window chrome's `CONTROL_BUTTON_WIDTH` — single source
/// of truth for control button sizing.
#[cfg(target_os = "windows")]
pub const CONTROLS_ZONE_WIDTH: f32 =
    crate::widgets::window_chrome::constants::CONTROL_BUTTON_WIDTH * 3.0;

/// Diameter of each circular window control button on Linux/macOS.
#[cfg(not(target_os = "windows"))]
pub const CONTROL_BUTTON_DIAMETER: f32 = 24.0;

/// Spacing between circular control buttons on Linux/macOS.
#[cfg(not(target_os = "windows"))]
pub const CONTROL_BUTTON_SPACING: f32 = 8.0;

/// Margin before and after the control button group on Linux/macOS.
#[cfg(not(target_os = "windows"))]
pub const CONTROL_BUTTON_MARGIN: f32 = 12.0;

/// Total width reserved for window control buttons on Linux/macOS.
///
/// `12 + 3×24 + 2×8 + 12 = 100px`.
#[cfg(not(target_os = "windows"))]
pub const CONTROLS_ZONE_WIDTH: f32 = CONTROL_BUTTON_MARGIN
    + 3.0 * CONTROL_BUTTON_DIAMETER
    + 2.0 * CONTROL_BUTTON_SPACING
    + CONTROL_BUTTON_MARGIN;

/// Pixels of mouse movement before a tab drag begins.
///
/// Matches Chrome's `tab_drag_controller.cc`.
pub const DRAG_START_THRESHOLD: f32 = 10.0;

/// Pixels outside the tab bar before a tab tears off into its own window.
pub const TEAR_OFF_THRESHOLD: f32 = 40.0;

/// Reduced tear-off threshold for upward dragging (more natural gesture).
pub const TEAR_OFF_THRESHOLD_UP: f32 = 15.0;
