pub mod colors;
pub mod interpolation;

pub use colors::{cea2034_colors, interpolate_colors, CEA2034_CURVES};
pub use interpolation::{format_frequency, get_angle_range, interpolate_spl_at_frequency};
