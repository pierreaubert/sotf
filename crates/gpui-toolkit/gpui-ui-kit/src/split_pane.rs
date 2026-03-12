//! SplitPane component
//!
//! A resizable split view with a draggable divider between two panes.
//!
//! # Usage
//!
//! ```ignore
//! SplitPane::new("main-split")
//!     .direction(SplitDirection::Horizontal)
//!     .first(sidebar_element)
//!     .second(content_element)
//!     .initial_ratio(0.3)
//! ```

use crate::ComponentTheme;
use crate::theme::ThemeExt;
use gpui::prelude::*;
use gpui::*;

/// Split direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDirection {
    /// Side by side (default)
    #[default]
    Horizontal,
    /// Stacked top/bottom
    Vertical,
}

/// Theme colors for split pane styling
#[derive(Debug, Clone, ComponentTheme)]
pub struct SplitPaneTheme {
    /// Divider color
    #[theme(default = 0x3a3a3aff, from = border)]
    pub divider: Rgba,
    /// Divider hover color
    #[theme(default = 0x007accff, from = accent)]
    pub divider_hover: Rgba,
    /// Divider active/dragging color
    #[theme(default = 0x007accff, from = accent)]
    pub divider_active: Rgba,
}

/// A split pane component with a draggable divider
pub struct SplitPane {
    id: ElementId,
    direction: SplitDirection,
    first: Option<AnyElement>,
    second: Option<AnyElement>,
    ratio: f32,
    min_first: Pixels,
    min_second: Pixels,
    divider_width: Pixels,
    on_resize: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
}

impl SplitPane {
    /// Create a new split pane
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            direction: SplitDirection::default(),
            first: None,
            second: None,
            ratio: 0.5,
            min_first: px(100.0),
            min_second: px(100.0),
            divider_width: px(4.0),
            on_resize: None,
        }
    }

    /// Set split direction
    pub fn direction(mut self, direction: SplitDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set the first (left/top) pane content
    pub fn first(mut self, element: impl IntoElement) -> Self {
        self.first = Some(element.into_any_element());
        self
    }

    /// Set the second (right/bottom) pane content
    pub fn second(mut self, element: impl IntoElement) -> Self {
        self.second = Some(element.into_any_element());
        self
    }

    /// Set initial split ratio (0.0 to 1.0, default 0.5)
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Set minimum size for the first pane
    pub fn min_first(mut self, min: Pixels) -> Self {
        self.min_first = min;
        self
    }

    /// Set minimum size for the second pane
    pub fn min_second(mut self, min: Pixels) -> Self {
        self.min_second = min;
        self
    }

    /// Set divider width
    pub fn divider_width(mut self, width: Pixels) -> Self {
        self.divider_width = width;
        self
    }

    /// Called when the user drags the divider (receives new ratio)
    pub fn on_resize(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Box::new(handler));
        self
    }

    /// Build the split pane with theme
    pub fn build_with_theme(self, theme: &SplitPaneTheme) -> Stateful<Div> {
        let divider_color = theme.divider;
        let divider_hover = theme.divider_hover;

        let mut container = div().id(self.id).size_full().flex().overflow_hidden();

        container = match self.direction {
            SplitDirection::Horizontal => container.flex_row(),
            SplitDirection::Vertical => container.flex_col(),
        };

        // First pane
        let first_pane = div().flex_shrink_0().overflow_hidden().children(self.first);

        let first_pane = match self.direction {
            SplitDirection::Horizontal => first_pane
                .h_full()
                .w(relative(self.ratio))
                .min_w(self.min_first),
            SplitDirection::Vertical => first_pane
                .w_full()
                .h(relative(self.ratio))
                .min_h(self.min_first),
        };

        // Divider
        let divider = match self.direction {
            SplitDirection::Horizontal => div()
                .id("split-divider")
                .w(self.divider_width)
                .h_full()
                .flex_shrink_0()
                .bg(divider_color)
                .cursor_col_resize()
                .hover(move |s| s.bg(divider_hover)),
            SplitDirection::Vertical => div()
                .id("split-divider")
                .h(self.divider_width)
                .w_full()
                .flex_shrink_0()
                .bg(divider_color)
                .cursor_row_resize()
                .hover(move |s| s.bg(divider_hover)),
        };

        // Second pane
        let second_pane = div().flex_1().overflow_hidden().children(self.second);

        let second_pane = match self.direction {
            SplitDirection::Horizontal => second_pane.h_full().min_w(self.min_second),
            SplitDirection::Vertical => second_pane.w_full().min_h(self.min_second),
        };

        container
            .child(first_pane)
            .child(divider)
            .child(second_pane)
    }
}

impl RenderOnce for SplitPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = SplitPaneTheme::from(&global_theme);
        self.build_with_theme(&theme)
    }
}

impl IntoElement for SplitPane {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
