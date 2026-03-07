use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{MenuEntry, MenuStyle, MenuWidget};

static MEASURER: MockMeasurer = MockMeasurer::STANDARD;

fn event_ctx(bounds: Rect) -> EventCtx<'static> {
    EventCtx {
        measurer: &MEASURER,
        bounds,
        is_focused: true,
        focused_widget: None,
        theme: &super::super::tests::TEST_THEME,
    }
}

fn layout_ctx() -> LayoutCtx<'static> {
    LayoutCtx {
        measurer: &MEASURER,
        theme: &super::super::tests::TEST_THEME,
    }
}

fn mouse_down(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_up(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_move(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn key_event(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::NONE,
    }
}

fn sample_entries() -> Vec<MenuEntry> {
    vec![
        MenuEntry::Item {
            label: "Copy".into(),
        },
        MenuEntry::Item {
            label: "Paste".into(),
        },
        MenuEntry::Separator,
        MenuEntry::Item {
            label: "Select All".into(),
        },
    ]
}

// Layout tests

#[test]
fn layout_min_width_enforced() {
    // Short labels should still produce at least min_width.
    let menu = MenuWidget::new(vec![MenuEntry::Item { label: "X".into() }]);
    let layout = menu.layout(&layout_ctx());

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        assert!(
            *intrinsic_width >= MenuStyle::default().min_width,
            "width {} should be >= min_width {}",
            intrinsic_width,
            MenuStyle::default().min_width
        );
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn layout_height_includes_all_entries() {
    let s = MenuStyle::default();
    let menu = MenuWidget::new(sample_entries());
    let layout = menu.layout(&layout_ctx());

    // 3 items × item_height + 1 separator × separator_height + 2 × padding_y
    let expected = 3.0 * s.item_height + s.separator_height + 2.0 * s.padding_y;

    if let BoxContent::Leaf {
        intrinsic_height, ..
    } = &layout.content
    {
        assert_eq!(*intrinsic_height, expected);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn layout_empty_menu() {
    let menu = MenuWidget::new(vec![]);
    let layout = menu.layout(&layout_ctx());

    if let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        let s = MenuStyle::default();
        assert!(*intrinsic_width >= s.min_width);
        // Only vertical padding, no entries.
        assert_eq!(*intrinsic_height, s.padding_y * 2.0);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn layout_wide_label_exceeds_min_width() {
    // "A really long menu item label!!" = 31 chars × 8px = 248px
    let menu = MenuWidget::new(vec![MenuEntry::Item {
        label: "A really long menu item label!!".into(),
    }]);
    let layout = menu.layout(&layout_ctx());

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        assert!(
            *intrinsic_width > MenuStyle::default().min_width,
            "wide label should exceed min_width"
        );
    } else {
        panic!("expected leaf layout");
    }
}

// Mouse interaction tests

#[test]
fn click_emits_selected() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Move to first item (y = padding_y + half item_height).
    let item_y = s.padding_y + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item_y), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Press + release.
    menu.handle_mouse(&mouse_down(50.0, item_y), &ctx);
    let r = menu.handle_mouse(&mouse_up(50.0, item_y), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: menu.id(),
            index: 0
        })
    );
}

#[test]
fn separator_not_clickable() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Y in the separator region (after 2 items).
    let sep_y = s.padding_y + s.item_height * 2.0 + s.separator_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, sep_y), &ctx);
    assert_eq!(menu.hovered(), None, "separator should not be hoverable");
}

#[test]
fn hover_tracking_on_move() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Move to second item.
    let item2_y = s.padding_y + s.item_height + s.item_height / 2.0;
    let r = menu.handle_mouse(&mouse_move(50.0, item2_y), &ctx);
    assert_eq!(menu.hovered(), Some(1));
    assert_eq!(r.response, crate::input::EventResponse::RequestRedraw);
}

#[test]
fn hover_leave_clears() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Hover an item.
    let item_y = s.padding_y + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item_y), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Leave.
    let r = menu.handle_hover(HoverEvent::Leave, &ctx);
    assert_eq!(menu.hovered(), None);
    assert_eq!(r.response, crate::input::EventResponse::RequestRedraw);
}

// Keyboard navigation tests

#[test]
fn arrow_down_navigates() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // First arrow down → first item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Second → second item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(1));

    // Third → skips separator (index 2), goes to index 3.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(3));
}

#[test]
fn arrow_up_navigates() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Arrow up with no selection → wraps to last item (index 3).
    menu.handle_key(key_event(Key::ArrowUp), &ctx);
    assert_eq!(menu.hovered(), Some(3));

    // Arrow up → skips separator, goes to index 1.
    menu.handle_key(key_event(Key::ArrowUp), &ctx);
    assert_eq!(menu.hovered(), Some(1));
}

#[test]
fn enter_emits_selected() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Navigate to first item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);

    // Enter activates.
    let r = menu.handle_key(key_event(Key::Enter), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: menu.id(),
            index: 0
        })
    );
}

#[test]
fn escape_emits_dismiss() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    let r = menu.handle_key(key_event(Key::Escape), &ctx);
    assert_eq!(r.action, Some(WidgetAction::DismissOverlay(menu.id())));
}

#[test]
fn keyboard_wraps_around() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Navigate to last item (index 3).
    menu.hovered = Some(3);

    // Arrow down wraps to first item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));
}

// Check item tests

#[test]
fn check_entries_affect_layout() {
    // Use a label long enough that both menus exceed min_width,
    // so the checkmark space difference is visible.
    let entries_no_check = vec![MenuEntry::Item {
        label: "A long enough menu item label here".into(),
    }];
    let entries_with_check = vec![MenuEntry::Check {
        label: "A long enough menu item label here".into(),
        checked: true,
    }];

    let menu_no = MenuWidget::new(entries_no_check);
    let menu_yes = MenuWidget::new(entries_with_check);

    let layout_no = menu_no.layout(&layout_ctx());
    let layout_yes = menu_yes.layout(&layout_ctx());

    if let (
        BoxContent::Leaf {
            intrinsic_width: w_no,
            ..
        },
        BoxContent::Leaf {
            intrinsic_width: w_yes,
            ..
        },
    ) = (&layout_no.content, &layout_yes.content)
    {
        // Check items add checkmark_size + checkmark_gap to the left margin.
        assert!(
            w_yes > w_no,
            "check menu should be wider: {} vs {}",
            w_yes,
            w_no
        );
    } else {
        panic!("expected leaf layouts");
    }
}

#[test]
fn menu_is_focusable() {
    let menu = MenuWidget::new(sample_entries());
    assert!(menu.is_focusable());
}

#[test]
fn not_focused_ignores_keys() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = EventCtx {
        measurer: &MEASURER,
        bounds: Rect::new(0.0, 0.0, 200.0, 200.0),
        is_focused: false,
        focused_widget: None,
        theme: &super::super::tests::TEST_THEME,
    };

    let r = menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn enter_without_hover_is_no_op() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // No hover selected.
    let r = menu.handle_key(key_event(Key::Enter), &ctx);
    assert!(r.action.is_none());
}

#[test]
fn all_separators_menu_navigate_returns_false() {
    let entries = vec![MenuEntry::Separator, MenuEntry::Separator];
    let mut menu = MenuWidget::new(entries);
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // No clickable items, so navigation should not change hover.
    let r = menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), None);
    // Should still be handled (not ignored).
    assert!(r.response.is_handled());
}

// Non-left mouse button tests

#[test]
fn right_click_ignored() {
    let mut menu = MenuWidget::new(sample_entries());
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(50.0, 20.0),
        modifiers: Modifiers::NONE,
    };
    let r = menu.handle_mouse(&down, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

// Out-of-bounds mouse Y tests

#[test]
fn mouse_y_above_padding_clears_hover() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Hover the first item.
    let item_y = s.padding_y + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item_y), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Move into top padding (y=1.0 is above first item at y=4.0).
    menu.handle_mouse(&mouse_move(50.0, 1.0), &ctx);
    assert_eq!(menu.hovered(), None);
}

#[test]
fn mouse_y_below_entries_clears_hover() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Hover the first item.
    let item_y = s.padding_y + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item_y), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Move into bottom padding (past all entries).
    let below_y = menu.total_height() - 1.0;
    menu.handle_mouse(&mouse_move(50.0, below_y), &ctx);
    assert_eq!(menu.hovered(), None);
}

// Hybrid keyboard/mouse interaction

#[test]
fn keyboard_then_mouse_hover_follows_mouse() {
    let mut menu = MenuWidget::new(sample_entries());
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Keyboard nav to first item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Mouse move to second item overrides keyboard hover.
    let item2_y = s.padding_y + s.item_height + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item2_y), &ctx);
    assert_eq!(menu.hovered(), Some(1));
}

// Space key activation

#[test]
fn space_key_activates() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    let r = menu.handle_key(key_event(Key::Space), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: menu.id(),
            index: 0
        })
    );
}

// Check item click

#[test]
fn click_check_item_emits_selected() {
    let entries = vec![
        MenuEntry::Check {
            label: "Option A".into(),
            checked: false,
        },
        MenuEntry::Check {
            label: "Option B".into(),
            checked: true,
        },
    ];
    let mut menu = MenuWidget::new(entries);
    let s = MenuStyle::default();
    let bounds = Rect::new(0.0, 0.0, 200.0, menu.total_height());
    let ctx = event_ctx(bounds);

    // Click on second check item.
    let item2_y = s.padding_y + s.item_height + s.item_height / 2.0;
    menu.handle_mouse(&mouse_move(50.0, item2_y), &ctx);
    assert_eq!(menu.hovered(), Some(1));

    menu.handle_mouse(&mouse_down(50.0, item2_y), &ctx);
    let r = menu.handle_mouse(&mouse_up(50.0, item2_y), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: menu.id(),
            index: 1
        })
    );
}

// Boundary: single item menu

#[test]
fn single_item_menu_works() {
    let mut menu = MenuWidget::new(vec![MenuEntry::Item {
        label: "Only".into(),
    }]);
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Arrow down selects the only item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Arrow down again wraps back to same item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Enter activates.
    let r = menu.handle_key(key_event(Key::Enter), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: menu.id(),
            index: 0
        })
    );
}

// Consecutive separators

#[test]
fn consecutive_separators_skipped() {
    let entries = vec![
        MenuEntry::Item { label: "A".into() },
        MenuEntry::Separator,
        MenuEntry::Separator,
        MenuEntry::Item { label: "B".into() },
    ];
    let mut menu = MenuWidget::new(entries);
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Arrow down → first item (index 0).
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // Arrow down → skips two separators, lands on index 3.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(3));
}

// Fresh instance contract

#[test]
fn new_menu_starts_with_no_hover() {
    let mut menu = MenuWidget::new(sample_entries());
    let ctx = event_ctx(Rect::new(0.0, 0.0, 200.0, 200.0));

    // Use the first menu — navigate to an item.
    menu.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(menu.hovered(), Some(0));

    // New menu instance starts with no hover.
    let menu2 = MenuWidget::new(sample_entries());
    assert_eq!(menu2.hovered(), None);
}
