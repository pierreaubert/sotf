mod interpolate;
pub mod rgb;
pub mod scheme;
pub mod chromatic;

pub use interpolate::{interpolate_colors, sequential_color};
pub use rgb::D3Color;
pub use scheme::ColorScheme;
