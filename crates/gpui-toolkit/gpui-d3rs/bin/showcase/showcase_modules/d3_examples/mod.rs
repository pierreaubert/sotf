//! D3.js Observable Examples
//!
//! This module contains ports of D3.js examples from Observable.
//! Each example demonstrates both:
//! 1. Low-level API usage (direct scale/generator manipulation)
//! 2. High-level API usage (ready-made render functions)
//!
//! Source examples from: <https://observablehq.com/@d3>

pub mod calendar;
pub mod choropleth;
pub mod faithful_data;
pub mod flare_data;
pub mod histogram;
pub mod horizon;
pub mod kernel_density_estimation;
pub mod parallel_coordinates;
pub mod path_utils;
pub mod radial_line;
pub mod revenue;
pub mod sankey;
pub mod stacked_grouped_bars;
pub mod treemap;
pub mod versor;
pub mod volcano_contours;
pub mod volcano_data;

pub mod obs_box_plot;
pub mod obs_chord;
pub mod obs_donut_chart;
pub mod obs_force_directed;
pub mod obs_hexbin;
pub mod obs_line_chart;
pub mod obs_pie_chart;
pub mod obs_stacked_area;
pub mod obs_stacked_bar;
pub mod obs_streamgraph;

pub use kernel_density_estimation::KernelType;
pub use stacked_grouped_bars::BarLayout;
pub use treemap::TilingMethod;
pub use volcano_contours::render;
