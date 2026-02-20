use crate::layout::{BoxContent, SizeSpec};
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{LayoutCtx, Widget};

use super::{SeparatorStyle, SeparatorWidget};

#[test]
fn horizontal_defaults() {
    let sep = SeparatorWidget::horizontal();
    assert!(!sep.is_focusable());
}

#[test]
fn horizontal_layout_fills_width() {
    let sep = SeparatorWidget::horizontal();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = sep.layout(&ctx);
    assert_eq!(layout.width, SizeSpec::Fill);
    if let BoxContent::Leaf {
        intrinsic_height, ..
    } = &layout.content
    {
        assert_eq!(*intrinsic_height, 1.0); // Default thickness.
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn horizontal_with_label_uses_text_height() {
    let sep = SeparatorWidget::horizontal().with_label("Section");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = sep.layout(&ctx);
    if let BoxContent::Leaf {
        intrinsic_height, ..
    } = &layout.content
    {
        // MockMeasurer line height = 16.0.
        assert_eq!(*intrinsic_height, 16.0);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn vertical_layout_fills_height() {
    let sep = SeparatorWidget::vertical();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = sep.layout(&ctx);
    assert_eq!(layout.height, SizeSpec::Fill);
    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        assert_eq!(*intrinsic_width, 1.0);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn has_widget_id() {
    let sep = SeparatorWidget::horizontal();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = sep.layout(&ctx);
    assert_eq!(layout.widget_id, Some(sep.id()));
}

#[test]
fn with_style_applies() {
    let style = SeparatorStyle {
        thickness: 3.0,
        ..SeparatorStyle::default()
    };
    let sep = SeparatorWidget::horizontal().with_style(style);
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = sep.layout(&ctx);
    if let BoxContent::Leaf {
        intrinsic_height, ..
    } = &layout.content
    {
        assert_eq!(*intrinsic_height, 3.0);
    } else {
        panic!("expected leaf layout");
    }
}
