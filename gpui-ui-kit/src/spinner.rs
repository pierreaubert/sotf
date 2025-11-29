//! Spinner component
//!
//! Loading indicators and spinners.

use gpui::prelude::*;
use gpui::*;

/// Spinner size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerSize {
    /// Extra small (12px)
    Xs,
    /// Small (16px)
    Sm,
    /// Medium (24px, default)
    #[default]
    Md,
    /// Large (32px)
    Lg,
    /// Extra large (48px)
    Xl,
}

impl SpinnerSize {
    fn size(&self) -> Pixels {
        match self {
            SpinnerSize::Xs => px(12.0),
            SpinnerSize::Sm => px(16.0),
            SpinnerSize::Md => px(24.0),
            SpinnerSize::Lg => px(32.0),
            SpinnerSize::Xl => px(48.0),
        }
    }

    fn border_width(&self) -> Pixels {
        match self {
            SpinnerSize::Xs => px(1.5),
            SpinnerSize::Sm => px(2.0),
            SpinnerSize::Md => px(2.5),
            SpinnerSize::Lg => px(3.0),
            SpinnerSize::Xl => px(4.0),
        }
    }
}

/// A spinner/loading indicator component
/// Note: True animation requires GPUI animation support
pub struct Spinner {
    size: SpinnerSize,
    color: Option<Rgba>,
    label: Option<SharedString>,
}

impl Spinner {
    /// Create a new spinner
    pub fn new() -> Self {
        Self {
            size: SpinnerSize::default(),
            color: None,
            label: None,
        }
    }

    /// Set size
    pub fn size(mut self, size: SpinnerSize) -> Self {
        self.size = size;
        self
    }

    /// Set custom color
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Set loading label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let size = self.size.size();
        let border_width = self.size.border_width();
        let color = self.color.unwrap_or(rgb(0x007acc));

        let mut container = div().flex().items_center().gap_2();

        // Spinner circle
        // Note: This is a static representation.
        // True spinning animation requires GPUI animation APIs
        let spinner = div()
            .w(size)
            .h(size)
            .rounded_full()
            .border(border_width)
            .border_color(color);

        container = container.child(spinner);

        // Label
        if let Some(label) = self.label {
            let label_el = match self.size {
                SpinnerSize::Xs | SpinnerSize::Sm => div().text_xs(),
                SpinnerSize::Md => div().text_sm(),
                SpinnerSize::Lg | SpinnerSize::Xl => div(),
            };
            container = container.child(label_el.text_color(rgb(0xcccccc)).child(label));
        }

        container
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Spinner {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A dots loading indicator
pub struct LoadingDots {
    size: SpinnerSize,
    color: Option<Rgba>,
}

impl LoadingDots {
    /// Create new loading dots
    pub fn new() -> Self {
        Self {
            size: SpinnerSize::default(),
            color: None,
        }
    }

    /// Set size
    pub fn size(mut self, size: SpinnerSize) -> Self {
        self.size = size;
        self
    }

    /// Set custom color
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let color = self.color.unwrap_or(rgb(0x007acc));
        let dot_size = match self.size {
            SpinnerSize::Xs => px(4.0),
            SpinnerSize::Sm => px(6.0),
            SpinnerSize::Md => px(8.0),
            SpinnerSize::Lg => px(10.0),
            SpinnerSize::Xl => px(12.0),
        };

        div()
            .flex()
            .items_center()
            .gap_1()
            .child(div().w(dot_size).h(dot_size).rounded_full().bg(color))
            .child(
                div()
                    .w(dot_size)
                    .h(dot_size)
                    .rounded_full()
                    .bg(color)
                    .opacity(0.7),
            )
            .child(
                div()
                    .w(dot_size)
                    .h(dot_size)
                    .rounded_full()
                    .bg(color)
                    .opacity(0.4),
            )
    }
}

impl Default for LoadingDots {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for LoadingDots {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
