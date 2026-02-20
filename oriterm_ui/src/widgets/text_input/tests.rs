use crate::geometry::Rect;
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{TextInputStyle, TextInputWidget};

static MEASURER: MockMeasurer = MockMeasurer::STANDARD;

fn event_ctx() -> EventCtx<'static> {
    EventCtx {
        measurer: &MEASURER,
        bounds: Rect::new(0.0, 0.0, 200.0, 28.0),
        is_focused: true,
    }
}

fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        modifiers: Modifiers::NONE,
    }
}

fn shift_key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        modifiers: Modifiers::SHIFT_ONLY,
    }
}

fn ctrl_key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        modifiers: Modifiers::CTRL_ONLY,
    }
}

fn char_key(ch: char) -> KeyEvent {
    key(Key::Character(ch))
}

#[test]
fn default_state() {
    let ti = TextInputWidget::new();
    assert_eq!(ti.text(), "");
    assert_eq!(ti.cursor(), 0);
    assert!(ti.selection_anchor().is_none());
    assert!(!ti.is_disabled());
    assert!(ti.is_focusable());
}

#[test]
fn type_characters() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.handle_key(char_key('h'), &ctx);
    ti.handle_key(char_key('i'), &ctx);
    assert_eq!(ti.text(), "hi");
    assert_eq!(ti.cursor(), 2);
}

#[test]
fn type_emits_text_changed() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    let r = ti.handle_key(char_key('a'), &ctx);
    assert_eq!(
        r.action,
        Some(WidgetAction::TextChanged {
            id: ti.id(),
            text: "a".to_string(),
        })
    );
}

#[test]
fn backspace_deletes() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.handle_key(char_key('a'), &ctx);
    ti.handle_key(char_key('b'), &ctx);
    assert_eq!(ti.text(), "ab");

    let r = ti.handle_key(key(Key::Backspace), &ctx);
    assert_eq!(ti.text(), "a");
    assert_eq!(ti.cursor(), 1);
    assert!(r.action.is_some());
}

#[test]
fn backspace_at_start_no_op() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    let r = ti.handle_key(key(Key::Backspace), &ctx);
    assert_eq!(ti.text(), "");
    assert!(r.action.is_none());
}

#[test]
fn delete_forward() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("abc");
    ti.cursor = 1; // After 'a'.

    let r = ti.handle_key(key(Key::Delete), &ctx);
    assert_eq!(ti.text(), "ac");
    assert_eq!(ti.cursor(), 1);
    assert!(r.action.is_some());
}

#[test]
fn delete_at_end_no_op() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("abc");
    // Cursor is at end after set_text.

    let r = ti.handle_key(key(Key::Delete), &ctx);
    assert_eq!(ti.text(), "abc");
    assert!(r.action.is_none());
}

#[test]
fn arrow_keys_move_cursor() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("abc");
    ti.cursor = 3;

    ti.handle_key(key(Key::ArrowLeft), &ctx);
    assert_eq!(ti.cursor(), 2);

    ti.handle_key(key(Key::ArrowLeft), &ctx);
    assert_eq!(ti.cursor(), 1);

    ti.handle_key(key(Key::ArrowRight), &ctx);
    assert_eq!(ti.cursor(), 2);
}

#[test]
fn home_end_keys() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 2;

    ti.handle_key(key(Key::Home), &ctx);
    assert_eq!(ti.cursor(), 0);

    ti.handle_key(key(Key::End), &ctx);
    assert_eq!(ti.cursor(), 5);
}

#[test]
fn shift_arrow_selects() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 2;

    ti.handle_key(shift_key(Key::ArrowRight), &ctx);
    assert_eq!(ti.cursor(), 3);
    assert_eq!(ti.selection_anchor(), Some(2));
    assert_eq!(ti.selection_range(), Some((2, 3)));

    ti.handle_key(shift_key(Key::ArrowRight), &ctx);
    assert_eq!(ti.selection_range(), Some((2, 4)));
}

#[test]
fn ctrl_a_selects_all() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 2;

    ti.handle_key(ctrl_key(Key::Character('a')), &ctx);
    assert_eq!(ti.selection_anchor(), Some(0));
    assert_eq!(ti.cursor(), 5);
    assert_eq!(ti.selection_range(), Some((0, 5)));
}

#[test]
fn type_replaces_selection() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.selection_anchor = Some(1);
    ti.cursor = 4; // Select "ell".

    ti.handle_key(char_key('X'), &ctx);
    assert_eq!(ti.text(), "hXo");
    assert_eq!(ti.cursor(), 2);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn backspace_deletes_selection() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.selection_anchor = Some(1);
    ti.cursor = 4;

    ti.handle_key(key(Key::Backspace), &ctx);
    assert_eq!(ti.text(), "ho");
    assert_eq!(ti.cursor(), 1);
}

#[test]
fn disabled_ignores() {
    let mut ti = TextInputWidget::new().with_disabled(true);
    let ctx = event_ctx();

    assert!(!ti.is_focusable());

    let r = ti.handle_key(char_key('a'), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = ti.handle_hover(HoverEvent::Enter, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn layout_uses_min_width() {
    let ti = TextInputWidget::new();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = ti.layout(&ctx);
    let s = TextInputStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        // Empty text → placeholder empty → min_width applies.
        assert!(*intrinsic_width >= s.min_width);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn placeholder_layout_measures_placeholder() {
    let ti = TextInputWidget::new().with_placeholder("Type here...");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = ti.layout(&ctx);

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        // "Type here..." = 12 chars * 8px = 96 + padding 16 = 112,
        // but min_width = 120 so it should be at least 120.
        assert!(*intrinsic_width >= 120.0);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn unicode_editing() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    // Type multi-byte chars.
    ti.handle_key(char_key('é'), &ctx);
    ti.handle_key(char_key('à'), &ctx);
    assert_eq!(ti.text(), "éà");
    assert_eq!(ti.cursor(), 4); // 2 bytes each.

    ti.handle_key(key(Key::Backspace), &ctx);
    assert_eq!(ti.text(), "é");
    assert_eq!(ti.cursor(), 2);
}

#[test]
fn arrow_left_collapses_selection_to_start() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.selection_anchor = Some(1);
    ti.cursor = 4;

    // Left arrow without shift collapses selection to start.
    ti.handle_key(key(Key::ArrowLeft), &ctx);
    assert_eq!(ti.cursor(), 1);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn arrow_right_collapses_selection_to_end() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.selection_anchor = Some(1);
    ti.cursor = 4;

    // Right arrow without shift collapses selection to end.
    ti.handle_key(key(Key::ArrowRight), &ctx);
    assert_eq!(ti.cursor(), 4);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn ctrl_a_then_type_replaces_all() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.handle_key(ctrl_key(Key::Character('a')), &ctx);
    assert_eq!(ti.selection_range(), Some((0, 5)));

    ti.handle_key(char_key('X'), &ctx);
    assert_eq!(ti.text(), "X");
    assert_eq!(ti.cursor(), 1);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn shift_home_selects_to_start() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 3;

    ti.handle_key(shift_key(Key::Home), &ctx);
    assert_eq!(ti.cursor(), 0);
    assert_eq!(ti.selection_anchor(), Some(3));
    assert_eq!(ti.selection_range(), Some((0, 3)));
}

#[test]
fn shift_end_selects_to_end() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 1;

    ti.handle_key(shift_key(Key::End), &ctx);
    assert_eq!(ti.cursor(), 5);
    assert_eq!(ti.selection_anchor(), Some(1));
    assert_eq!(ti.selection_range(), Some((1, 5)));
}

#[test]
fn delete_with_selection_removes_selected() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.selection_anchor = Some(1);
    ti.cursor = 4;

    ti.handle_key(key(Key::Delete), &ctx);
    assert_eq!(ti.text(), "ho");
    assert_eq!(ti.cursor(), 1);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn left_arrow_at_start_stays() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 0;

    ti.handle_key(key(Key::ArrowLeft), &ctx);
    assert_eq!(ti.cursor(), 0);
}

#[test]
fn right_arrow_at_end_stays() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    // cursor already at end from set_text.

    ti.handle_key(key(Key::ArrowRight), &ctx);
    assert_eq!(ti.cursor(), 5);
}

#[test]
fn four_byte_unicode_editing() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    // Emoji are 4 bytes each in UTF-8.
    ti.handle_key(char_key('\u{1F600}'), &ctx); // Grinning face.
    ti.handle_key(char_key('\u{1F680}'), &ctx); // Rocket.
    assert_eq!(ti.text().len(), 8); // 4 bytes each.
    assert_eq!(ti.cursor(), 8);

    ti.handle_key(key(Key::Backspace), &ctx);
    assert_eq!(ti.text(), "\u{1F600}");
    assert_eq!(ti.cursor(), 4);
}

#[test]
fn set_text_moves_cursor_to_end() {
    let mut ti = TextInputWidget::new();
    ti.set_text("abc");
    assert_eq!(ti.cursor(), 3);
    assert!(ti.selection_anchor().is_none());
}

#[test]
fn escape_key_ignored() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    let r = ti.handle_key(key(Key::Escape), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
    assert_eq!(ti.text(), "hello");
}

#[test]
fn shift_left_then_right_cancels_selection() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    ti.set_text("hello");
    ti.cursor = 3;

    // Select one char left.
    ti.handle_key(shift_key(Key::ArrowLeft), &ctx);
    assert_eq!(ti.selection_range(), Some((2, 3)));

    // Select one char right — cursor back to anchor.
    ti.handle_key(shift_key(Key::ArrowRight), &ctx);
    // Anchor is still 3, cursor is 3 — selection is (3,3) which is empty.
    assert_eq!(ti.cursor(), 3);
    assert_eq!(ti.selection_anchor(), Some(3));
}

#[test]
fn ctrl_a_on_empty_text() {
    let mut ti = TextInputWidget::new();
    let ctx = event_ctx();

    let r = ti.handle_key(ctrl_key(Key::Character('a')), &ctx);
    // Should still set selection (0,0) — anchor=0, cursor=0.
    assert_eq!(ti.selection_anchor(), Some(0));
    assert_eq!(ti.cursor(), 0);
    assert!(r.response.is_handled());
}
