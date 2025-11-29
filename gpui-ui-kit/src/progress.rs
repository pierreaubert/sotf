//! Progress component
//!
//! Progress bars and indicators.

use gpui::prelude::*;
use gpui::*;

/// Progress variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressVariant {
    /// Default blue
    #[default]
    Default,
    /// Success green
    Success,
    /// Warning yellow
    Warning,
    /// Error red
    Error,
}

impl ProgressVariant {
    fn color(&self) -> Rgba {
        match self {
            ProgressVariant::Default => rgb(0x007acc),
            ProgressVariant::Success => rgb(0x2da44e),
            ProgressVariant::Warning => rgb(0xd29922),
            ProgressVariant::Error => rgb(0xcc3333),
        }
    }
}

/// Progress size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressSize {
    /// Extra small (2px)
    Xs,
    /// Small (4px)
    Sm,
    /// Medium (8px, default)
    #[default]
    Md,
    /// Large (12px)
    Lg,
}

impl ProgressSize {
    fn height(&self) -> Pixels {
        match self {
            ProgressSize::Xs => px(2.0),
            ProgressSize::Sm => px(4.0),
            ProgressSize::Md => px(8.0),
            ProgressSize::Lg => px(12.0),
        }
    }
}

/// A progress bar component
pub struct Progress {
    value: f32,
    max: f32,
    variant: ProgressVariant,
    size: ProgressSize,
    show_label: bool,
    striped: bool,
    animated: bool,
}

impl Progress {
    /// Create a new progress bar
    pub fn new(value: f32) -> Self {
        Self {
            value,
            max: 100.0,
            variant: ProgressVariant::default(),
            size: ProgressSize::default(),
            show_label: false,
            striped: false,
            animated: false,
        }
    }

    /// Set maximum value
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Set variant
    pub fn variant(mut self, variant: ProgressVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set size
    pub fn size(mut self, size: ProgressSize) -> Self {
        self.size = size;
        self
    }

    /// Show percentage label
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Enable striped appearance
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Enable animation
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let height = self.size.height();
        let color = self.variant.color();
        let percentage = (self.value / self.max * 100.0).clamp(0.0, 100.0);

        let mut container = div().flex().flex_col().gap_1().w_full();

        // Label
        if self.show_label {
            container = container.child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(0xcccccc))
                    .child(format!("{:.0}%", percentage)),
            );
        }

        // Track
        let track = div()
            .w_full()
            .h(height)
            .bg(rgb(0x2a2a2a))
            .rounded_full()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(color)
                    .rounded_full()
                    .w(relative(percentage / 100.0)),
            );

        container = container.child(track);

        container
    }
}

impl IntoElement for Progress {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A circular progress indicator
pub struct CircularProgress {
    value: f32,
    max: f32,
    size: Pixels,
    thickness: Pixels,
    variant: ProgressVariant,
    show_label: bool,
}

impl CircularProgress {
    /// Create a new circular progress
    pub fn new(value: f32) -> Self {
        Self {
            value,
            max: 100.0,
            size: px(48.0),
            thickness: px(4.0),
            variant: ProgressVariant::default(),
            show_label: false,
        }
    }

    /// Set maximum value
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Set size
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Set thickness
    pub fn thickness(mut self, thickness: Pixels) -> Self {
        self.thickness = thickness;
        self
    }

    /// Set variant
    pub fn variant(mut self, variant: ProgressVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Show percentage label in center
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Build into element
    /// Note: True circular progress requires canvas/SVG rendering.
    /// This is a simplified box-based representation.
    pub fn build(self) -> Div {
        let percentage = (self.value / self.max * 100.0).clamp(0.0, 100.0);
        let color = self.variant.color();

        let mut container = div()
            .flex()
            .items_center()
            .justify_center()
            .w(self.size)
            .h(self.size)
            .rounded_full()
            .border(self.thickness)
            .border_color(rgb(0x2a2a2a))
            .relative();

        // Progress arc approximation (using border color)
        // Note: This is a simplified version - true circular progress needs SVG
        if percentage > 0.0 {
            container = container.border_color(color);
        }

        // Center label
        if self.show_label {
            container = container.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xcccccc))
                    .child(format!("{:.0}%", percentage)),
            );
        }

        container
    }
}

impl IntoElement for CircularProgress {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
