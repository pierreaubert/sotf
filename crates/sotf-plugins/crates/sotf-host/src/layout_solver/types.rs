use crate::plugin_layout::ColumnRole;

/// Orientation of the overall layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Normal desktop: columns side-by-side.
    Horizontal,
    /// Mobile/very narrow: main on top, everything else below.
    Vertical,
}

/// Direction for arranging control groups within the main area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Groups arranged side-by-side (e.g., DYNAMICS | TIMING).
    Row,
    /// Groups stacked vertically.
    Column,
}

/// Knob size tier chosen by the solver based on available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobSize {
    /// Extra-compact for very narrow layouts.
    Xs,
    /// Default compact size.
    Sm,
    /// Medium size for wide layouts.
    Md,
}

/// A column that the solver decided should be visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolvedColumn {
    pub role: ColumnRole,
    /// Allocated width in pixels.
    pub width: f32,
}

/// A column that was collapsed into a tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollapsedTab {
    pub role: ColumnRole,
    /// Tab label derived from column role.
    pub name: &'static str,
}
