//! Core layer types for the compositor: `LayerType`, `LayerProperties`,
//! and `Layer`.
//!
//! A layer is a unit of composition — it has a type (textured content,
//! solid color, or group), visual properties (bounds, opacity, transform),
//! and dirty flags that drive the compositor's paint and composite passes.
//!
//! [`LayerId`] is re-exported from [`geometry`](crate::geometry) where the
//! canonical definition lives.

use crate::color::Color;
use crate::geometry::{LayerId, Rect, Transform2D};

/// What a layer renders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerType {
    /// Renders content via a `LayerDelegate`, backed by a texture.
    Textured,
    /// Flat color fill (modal dimming, separators).
    SolidColor(Color),
    /// No own content — groups children. Transform and opacity apply
    /// to the entire subtree.
    Group,
}

/// Visual properties for a compositor layer.
///
/// All properties can be animated by the `LayerAnimator`. The defaults
/// represent a fully visible, untransformed layer (the performance
/// fast-path — no intermediate texture needed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerProperties {
    /// Position and size in parent coordinates.
    pub bounds: Rect,
    /// Opacity multiplier (0.0 = transparent, 1.0 = opaque).
    ///
    /// Multiplied down the tree: a layer at 0.5 under a parent at 0.5
    /// renders at 0.25 effective opacity.
    pub opacity: f32,
    /// 2D affine transform applied to this layer's content.
    pub transform: Transform2D,
    /// Whether the layer is visible. Invisible layers are skipped
    /// entirely during composition (no texture allocation, no draw).
    pub visible: bool,
    /// Whether to clip children to this layer's bounds.
    pub clip_children: bool,
}

impl Default for LayerProperties {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            opacity: 1.0,
            transform: Transform2D::identity(),
            visible: true,
            clip_children: false,
        }
    }
}

/// A compositor layer — the unit of GPU-backed composition.
///
/// Layers are managed by a [`LayerTree`](super::layer_tree::LayerTree)
/// and rendered by the GPU compositor. Each layer can render to its own
/// texture; a composition pass blends visible layers with per-layer
/// opacity and transforms.
#[derive(Debug)]
pub struct Layer {
    /// Unique identifier.
    id: LayerId,
    /// What this layer renders.
    kind: LayerType,
    /// Visual properties (bounds, opacity, transform, visibility).
    properties: LayerProperties,
    /// Parent layer, or `None` for the root.
    pub(crate) parent: Option<LayerId>,
    /// Children in paint order (back-to-front).
    pub(crate) children: Vec<LayerId>,
    /// Content is dirty — needs re-render to texture.
    pub(crate) needs_paint: bool,
    /// Properties are dirty — needs re-composite.
    pub(crate) needs_composite: bool,
}

impl Layer {
    // Constructors

    /// Creates a new layer with the given type and properties.
    pub fn new(id: LayerId, kind: LayerType, properties: LayerProperties) -> Self {
        Self {
            id,
            kind,
            properties,
            parent: None,
            children: Vec::new(),
            needs_paint: true,
            needs_composite: true,
        }
    }

    // Accessors

    /// Returns this layer's unique identifier.
    pub fn id(&self) -> LayerId {
        self.id
    }

    /// Returns this layer's type.
    pub fn kind(&self) -> LayerType {
        self.kind
    }

    /// Returns a reference to this layer's visual properties.
    pub fn properties(&self) -> &LayerProperties {
        &self.properties
    }

    /// Returns the parent layer, or `None` for the root.
    pub fn parent(&self) -> Option<LayerId> {
        self.parent
    }

    /// Returns the children in paint order (back-to-front).
    pub fn children(&self) -> &[LayerId] {
        &self.children
    }

    /// Returns `true` if this layer's content needs re-rendering.
    pub fn needs_paint(&self) -> bool {
        self.needs_paint
    }

    /// Returns `true` if this layer's composition needs updating.
    pub fn needs_composite(&self) -> bool {
        self.needs_composite
    }

    // Predicates

    /// Returns `true` when the layer needs an intermediate texture.
    ///
    /// Layers with default properties (opacity = 1.0, identity transform)
    /// can render directly to the screen — no intermediate texture needed.
    /// This is the performance escape hatch: zero overhead when not animating.
    pub fn needs_texture(&self) -> bool {
        self.properties.opacity < 1.0 || !self.properties.transform.is_identity()
    }

    // Property mutation

    /// Sets the layer's bounds, marking it for re-composite.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.properties.bounds = bounds;
        self.needs_composite = true;
    }

    /// Sets the layer's opacity, marking it for re-composite.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.properties.opacity = opacity;
        self.needs_composite = true;
    }

    /// Sets the layer's transform, marking it for re-composite.
    pub fn set_transform(&mut self, transform: Transform2D) {
        self.properties.transform = transform;
        self.needs_composite = true;
    }

    /// Sets the layer's visibility, marking it for re-composite.
    pub fn set_visible(&mut self, visible: bool) {
        self.properties.visible = visible;
        self.needs_composite = true;
    }

    /// Marks this layer's content as needing re-render to texture.
    pub fn schedule_paint(&mut self) {
        self.needs_paint = true;
    }

    /// Clears both dirty flags after a frame.
    pub fn clear_dirty_flags(&mut self) {
        self.needs_paint = false;
        self.needs_composite = false;
    }
}
