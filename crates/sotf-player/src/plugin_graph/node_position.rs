use serde::{Deserialize, Serialize};

/// Position on the 2D canvas
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

impl NodePosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
