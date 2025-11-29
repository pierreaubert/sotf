//! Color module for color representation and manipulation

mod interpolate;
mod rgb;
mod scheme;

pub use interpolate::{interpolate_colors, sequential_color};
pub use rgb::D3Color;
pub use scheme::ColorScheme;
