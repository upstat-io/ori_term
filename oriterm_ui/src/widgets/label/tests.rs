use crate::text::TextOverflow;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{LayoutCtx, Widget};

use super::{LabelStyle, LabelWidget};

#[test]
fn default_style() {
    let label = LabelWidget::new("hello");
    assert_eq!(label.text(), "hello");
    assert!(!label.is_focusable());
}

#[test]
fn layout_uses_measurer() {
    let label = LabelWidget::new("test");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = label.layout(&ctx);

    // "test" = 4 chars * 8px = 32px wide, 16px tall.
    if let crate::layout::BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        assert_eq!(*intrinsic_width, 32.0);
        assert_eq!(*intrinsic_height, 16.0);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn layout_has_widget_id() {
    let label = LabelWidget::new("x");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = label.layout(&ctx);
    assert_eq!(layout.widget_id, Some(label.id()));
}

#[test]
fn set_text_updates() {
    let mut label = LabelWidget::new("before");
    label.set_text("after");
    assert_eq!(label.text(), "after");
}

#[test]
fn with_style_applies() {
    let style = LabelStyle {
        font_size: 20.0,
        overflow: TextOverflow::Ellipsis,
        ..LabelStyle::default()
    };
    let label = LabelWidget::new("styled").with_style(style.clone());
    assert_eq!(label.style.font_size, 20.0);
    assert_eq!(label.style.overflow, TextOverflow::Ellipsis);
}

#[test]
fn empty_text_layout() {
    let label = LabelWidget::new("");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = label.layout(&ctx);
    if let crate::layout::BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        assert_eq!(*intrinsic_width, 0.0);
    } else {
        panic!("expected leaf layout");
    }
}
