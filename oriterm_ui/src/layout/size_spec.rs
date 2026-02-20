//! Size specification for layout box dimensions.

/// Specifies how a layout box's width or height is determined.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeSpec {
    /// Exact pixel size.
    Fixed(f32),
    /// Expand to fill available space (weight 1).
    Fill,
    /// Expand proportionally by weight `n`.
    FillPortion(u32),
    /// Shrink to fit content.
    #[default]
    Hug,
}

impl SizeSpec {
    /// Returns `true` if this spec participates in fill distribution.
    ///
    /// `FillPortion(0)` returns `false` since zero weight receives no space.
    pub fn is_fill(self) -> bool {
        self.fill_weight() > 0
    }

    /// Returns the fill weight: 1 for `Fill`, `n` for `FillPortion(n)`, 0 otherwise.
    pub fn fill_weight(self) -> u32 {
        match self {
            Self::Fill => 1,
            Self::FillPortion(n) => n,
            Self::Fixed(_) | Self::Hug => 0,
        }
    }
}
