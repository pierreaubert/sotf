//! Complete D3.js Observable example implementations.
//!
//! Each module implements a full visualization pipeline using d3rs,
//! mirroring the corresponding Observable notebook. The output is a
//! plain data struct with all computed geometry — no rendering dependency.
//!
//! These serve three purposes:
//! 1. **Documentation** — idiomatic d3rs usage examples
//! 2. **Golden tests** — validated against D3.js output
//! 3. **Showcase** — fed into GPUI for visual rendering

pub mod box_plot;
pub mod chord;
pub mod donut_chart;
pub mod force_directed;
pub mod hexbin;
pub mod line_chart;
pub mod pie_chart;
pub mod stacked_area;
pub mod stacked_bar;
pub mod streamgraph;
