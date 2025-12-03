//! Primitive types for GPU 2D rendering

mod line;
mod rect;
mod circle;
mod triangle;

pub use line::{LineVertex, LineBatch};
pub use rect::{RectVertex, RectBatch};
pub use circle::{CircleVertex, CircleBatch};
pub use triangle::{TriangleVertex, TriangleBatch};

/// A color represented as RGBA floats [0.0, 1.0]
pub type Color4 = [f32; 4];

/// A 2D rectangle in pixel coordinates
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn min(&self) -> [f32; 2] {
        [self.x, self.y]
    }

    pub fn max(&self) -> [f32; 2] {
        [self.x + self.width, self.y + self.height]
    }
}
