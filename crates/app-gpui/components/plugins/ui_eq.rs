//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

mod calculate;
mod consts;
mod eq_chart_wrapper;
mod eq_control_point_drag;
mod eq_qhandle_drag;
mod get;
mod misc;
mod render;
mod types;

pub use calculate::*;
pub use consts::*;
pub use get::*;
pub use misc::*;
pub use render::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::types::EqCompactLayout;

    #[test]
    fn layout_selection_breakpoints() {
        assert_eq!(EqCompactLayout::from_width(1000.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(900.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(750.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(600.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(599.0), EqCompactLayout::Inspector);
        assert_eq!(EqCompactLayout::from_width(320.0), EqCompactLayout::Inspector);
    }
}
