pub mod axes;
pub mod bar_charts;
pub mod colors;
pub mod contours;
pub mod geo;
pub mod line_charts;
pub mod overview;
pub mod quadtree;
pub mod scales;
pub mod scatter_plots;
pub mod surface_plots;
pub mod transitions;

// Re-export the main types that the modules need
pub use crate::{ContourRenderMode, ShowcaseApp};
