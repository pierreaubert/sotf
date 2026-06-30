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

#[cfg(test)]
mod tests {
    use super::EqCompactLayout;

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
