//! Vector font text rendering
//!
//! This module provides a simple stroke-based vector font that can be used
//! to render text as paths, allowing for rotation and other transformations.

mod vector_font;

pub use vector_font::{render_vector_text, VectorFontConfig};
