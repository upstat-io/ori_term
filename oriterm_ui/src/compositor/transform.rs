//! 2D affine transform for compositor layer positioning and animation.
//!
//! Represents a 3x2 column-major matrix that maps `(x, y)` to
//! `(a*x + c*y + tx, b*x + d*y + ty)`. Used for layer transforms in
//! the compositor: translation, scaling, rotation, and composition.

use std::fmt;

use crate::animation::Lerp;
use crate::geometry::{Point, Rect};

/// 2D affine transform represented as a 3x2 column-major matrix.
///
/// Stored as `[a, b, c, d, tx, ty]` where the transform maps
/// `(x, y)` to `(a*x + c*y + tx, b*x + d*y + ty)`.
///
/// As a 3x3 homogeneous matrix:
/// ```text
/// | a  c  tx |
/// | b  d  ty |
/// | 0  0  1  |
/// ```
#[derive(Clone, Copy, PartialEq)]
#[must_use]
pub struct Transform2D {
    matrix: [f32; 6], // [a, b, c, d, tx, ty]
}

impl fmt::Debug for Transform2D {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mat = self.matrix;
        fmt.debug_struct("Transform2D")
            .field("a", &mat[0])
            .field("b", &mat[1])
            .field("c", &mat[2])
            .field("d", &mat[3])
            .field("tx", &mat[4])
            .field("ty", &mat[5])
            .finish()
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform2D {
    // --- Constructors ---

    /// Creates the identity transform (no-op).
    pub fn identity() -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    /// Creates a translation transform.
    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, tx, ty],
        }
    }

    /// Creates a scaling transform (uniform or non-uniform).
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Creates a rotation transform around the origin.
    pub fn rotate(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            matrix: [cos, sin, -sin, cos, 0.0, 0.0],
        }
    }

    // --- Accessors ---

    /// Returns the raw `[a, b, c, d, tx, ty]` array for GPU uniform upload.
    pub fn to_mat3x2(self) -> [f32; 6] {
        self.matrix
    }

    /// Returns a column-major 3x3 matrix for GPU compositor uniforms.
    ///
    /// Converts the internal `[a, b, c, d, tx, ty]` representation to
    /// `[[a, b, 0], [c, d, 0], [tx, ty, 1]]` as required by the WGSL
    /// `mat3x3<f32>` layout.
    pub fn to_column_major_3x3(self) -> [[f32; 3]; 3] {
        let [a, b, c, d, tx, ty] = self.matrix;
        [[a, b, 0.0], [c, d, 0.0], [tx, ty, 1.0]]
    }

    // --- Predicates ---

    /// Returns `true` if this is the identity transform.
    ///
    /// Exact float comparison is intentional: identity transforms are always
    /// constructed with exact float literals, so bitwise equality is correct.
    /// Used as a fast-path check to skip intermediate render-to-texture.
    #[expect(
        clippy::float_cmp,
        reason = "identity is constructed with exact literals"
    )]
    pub fn is_identity(self) -> bool {
        self.matrix == [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    }

    // --- Composition ---

    /// Composes two transforms via matrix multiplication: `self * other`.
    ///
    /// The resulting transform applies `other` first, then `self`.
    pub fn concat(self, other: &Self) -> Self {
        let lhs = self.matrix;
        let rhs = other.matrix;
        Self {
            matrix: [
                lhs[0] * rhs[0] + lhs[2] * rhs[1],
                lhs[1] * rhs[0] + lhs[3] * rhs[1],
                lhs[0] * rhs[2] + lhs[2] * rhs[3],
                lhs[1] * rhs[2] + lhs[3] * rhs[3],
                lhs[0] * rhs[4] + lhs[2] * rhs[5] + lhs[4],
                lhs[1] * rhs[4] + lhs[3] * rhs[5] + lhs[5],
            ],
        }
    }

    /// Applies a translation before this transform: `self * translate(tx, ty)`.
    ///
    /// More efficient than `self.concat(&Transform2D::translate(tx, ty))`.
    pub fn pre_translate(self, tx: f32, ty: f32) -> Self {
        let mat = self.matrix;
        Self {
            matrix: [
                mat[0],
                mat[1],
                mat[2],
                mat[3],
                mat[0] * tx + mat[2] * ty + mat[4],
                mat[1] * tx + mat[3] * ty + mat[5],
            ],
        }
    }

    /// Applies a scale before this transform: `self * scale(sx, sy)`.
    ///
    /// More efficient than `self.concat(&Transform2D::scale(sx, sy))`.
    pub fn pre_scale(self, sx: f32, sy: f32) -> Self {
        let mat = self.matrix;
        Self {
            matrix: [
                mat[0] * sx,
                mat[1] * sx,
                mat[2] * sy,
                mat[3] * sy,
                mat[4],
                mat[5],
            ],
        }
    }

    // --- Point/rect mapping ---

    /// Transforms a point.
    pub fn apply(self, point: Point) -> Point {
        let mat = self.matrix;
        Point::new(
            mat[0] * point.x + mat[2] * point.y + mat[4],
            mat[1] * point.x + mat[3] * point.y + mat[5],
        )
    }

    /// Transforms a rectangle, returning the axis-aligned bounding box.
    ///
    /// Transforms all four corners and returns the smallest axis-aligned
    /// rectangle enclosing the results.
    pub fn apply_rect(self, rect: Rect) -> Rect {
        let p0 = self.apply(Point::new(rect.x(), rect.y()));
        let p1 = self.apply(Point::new(rect.right(), rect.y()));
        let p2 = self.apply(Point::new(rect.x(), rect.bottom()));
        let p3 = self.apply(Point::new(rect.right(), rect.bottom()));

        let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
        let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
        let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
        let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);

        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    // --- Inversion ---

    /// Returns the inverse transform, or `None` if the matrix is degenerate.
    ///
    /// A degenerate matrix (zero determinant) has no inverse — this happens
    /// with zero-scale transforms.
    pub fn inverse(self) -> Option<Self> {
        let mat = self.matrix;
        let det = mat[0] * mat[3] - mat[2] * mat[1];

        // Reject degenerate matrices (zero or denormalized determinant).
        if !det.is_normal() {
            return None;
        }

        let inv = 1.0 / det;
        Some(Self {
            matrix: [
                mat[3] * inv,
                -mat[1] * inv,
                -mat[2] * inv,
                mat[0] * inv,
                (mat[2] * mat[5] - mat[3] * mat[4]) * inv,
                (mat[1] * mat[4] - mat[0] * mat[5]) * inv,
            ],
        })
    }
}

impl Lerp for Transform2D {
    /// Per-element linear interpolation between two transforms.
    ///
    /// Sufficient for translate and scale animations. Rotation lerp is
    /// only correct for small angles; large rotations should decompose
    /// into angle + slerp (not needed for compositor use cases).
    fn lerp(start: Self, end: Self, t: f32) -> Self {
        let mut result = [0.0_f32; 6];
        let mut idx = 0;
        while idx < 6 {
            result[idx] = f32::lerp(start.matrix[idx], end.matrix[idx], t);
            idx += 1;
        }
        Self { matrix: result }
    }
}
