//! Width-based layout selection for the compact EQ UI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqCompactLayout {
    /// Existing large-window stacked layout.
    Current,
    /// Graph on top, horizontal band strip + inline editor below.
    BottomStrip,
    /// Scrollable band list; graph hidden by default.
    Inspector,
}

impl EqCompactLayout {
    pub fn from_width(width: f32) -> Self {
        if width >= 900.0 {
            Self::Current
        } else if width >= 600.0 {
            Self::BottomStrip
        } else {
            Self::Inspector
        }
    }
}

/// ui.md Phase 3 pilot: the medium-workbench EQ chart sizes to its container
/// — the solved editor width minus the fixed band rail — instead of enforcing
/// a pixel floor that overflows narrow editors. Short windows reduce chart
/// height and scroll the editor vertically; they must not clip a phantom
/// minimum width.
pub fn medium_graph_width(available_width: f32, layout_scale: f32) -> f32 {
    (available_width - 104.0 * layout_scale).max(1.0)
}

/// ui.md Phase 3 pilot: the narrow-inspector EQ chart fills the container
/// width with no minimum-width floor.
pub fn narrow_graph_width(available_width: f32) -> f32 {
    available_width.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::EqCompactLayout;

    #[test]
    fn chart_widths_track_the_container_without_pixel_floors() {
        // Medium workbench: container minus the band rail, no 360px floor.
        assert_eq!(super::medium_graph_width(800.0, 1.0), 696.0);
        assert_eq!(super::medium_graph_width(320.0, 1.0), 216.0);
        // Narrow inspector: full container width, no 320px floor.
        assert_eq!(super::narrow_graph_width(800.0), 800.0);
        assert_eq!(super::narrow_graph_width(320.0), 320.0);
        assert_eq!(super::narrow_graph_width(240.0), 240.0);
    }

    #[test]
    fn degenerate_chart_widths_clamp_to_a_positive_size() {
        assert_eq!(super::medium_graph_width(0.0, 1.0), 1.0);
        assert_eq!(super::medium_graph_width(50.0, 1.0), 1.0);
        assert_eq!(super::narrow_graph_width(0.0), 1.0);
        assert_eq!(super::narrow_graph_width(-10.0), 1.0);
    }

    #[test]
    fn layout_selection_breakpoints() {
        assert_eq!(
            EqCompactLayout::from_width(1000.0),
            EqCompactLayout::Current
        );
        assert_eq!(EqCompactLayout::from_width(900.0), EqCompactLayout::Current);
        assert_eq!(
            EqCompactLayout::from_width(750.0),
            EqCompactLayout::BottomStrip
        );
        assert_eq!(
            EqCompactLayout::from_width(600.0),
            EqCompactLayout::BottomStrip
        );
        assert_eq!(
            EqCompactLayout::from_width(599.0),
            EqCompactLayout::Inspector
        );
        assert_eq!(
            EqCompactLayout::from_width(320.0),
            EqCompactLayout::Inspector
        );
    }
}
