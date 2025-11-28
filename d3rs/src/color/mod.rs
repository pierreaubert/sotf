//! Color module for color representation and manipulation

mod rgb;
mod interpolate;
mod scheme;

pub use rgb::D3Color;
pub use interpolate::interpolate_colors;
pub use scheme::ColorScheme;
