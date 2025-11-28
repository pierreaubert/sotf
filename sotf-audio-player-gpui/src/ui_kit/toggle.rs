//! Toggle/Switch component
//!
//! A toggle switch for boolean values.

use gpui::prelude::*;
use gpui::*;

/// Toggle size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleSize {
    /// Small
    Sm,
    /// Medium (default)
    #[default]
    Md,
    /// Large
    Lg,
}

impl ToggleSize {
    fn track_width(&self) -> Pixels {
        match self {
            ToggleSize::Sm => px(32.0),
            ToggleSize::Md => px(40.0),
            ToggleSize::Lg => px(52.0),
        }
    }

    fn track_height(&self) -> Pixels {
        match self {
            ToggleSize::Sm => px(18.0),
            ToggleSize::Md => px(22.0),
            ToggleSize::Lg => px(28.0),
        }
    }

    fn knob_size(&self) -> Pixels {
        match self {
            ToggleSize::Sm => px(14.0),
            ToggleSize::Md => px(18.0),
            ToggleSize::Lg => px(24.0),
        }
    }

    fn knob_offset(&self) -> Pixels {
        match self {
            ToggleSize::Sm => px(2.0),
            ToggleSize::Md => px(2.0),
            ToggleSize::Lg => px(2.0),
        }
    }
}

/// A toggle switch component
pub struct Toggle {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    size: ToggleSize,
    disabled: bool,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Toggle {
    /// Create a new toggle
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: None,
            size: ToggleSize::default(),
            disabled: false,
            on_change: None,
        }
    }

    /// Set checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set size
    pub fn size(mut self, size: ToggleSize) -> Self {
        self.size = size;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set change handler
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Stateful<Div> {
        let track_width = self.size.track_width();
        let track_height = self.size.track_height();
        let knob_size = self.size.knob_size();
        let knob_offset = self.size.knob_offset();
        let checked = self.checked;

        let track_bg = if checked {
            rgb(0x007acc)
        } else {
            rgb(0x3a3a3a)
        };

        let knob_left = if checked {
            track_width - knob_size - knob_offset
        } else {
            knob_offset
        };

        let mut container = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer();

        if self.disabled {
            container = container.opacity(0.5).cursor_not_allowed();
        }

        // Track
        let mut track = div()
            .relative()
            .w(track_width)
            .h(track_height)
            .rounded_full()
            .bg(track_bg);

        // Knob
        let knob = div()
            .absolute()
            .top(knob_offset)
            .left(knob_left)
            .w(knob_size)
            .h(knob_size)
            .rounded_full()
            .bg(rgb(0xffffff))
            .shadow_sm();

        track = track.child(knob);
        container = container.child(track);

        // Label
        if let Some(label) = self.label {
            let label_el = match self.size {
                ToggleSize::Sm => div().text_xs(),
                ToggleSize::Md => div().text_sm(),
                ToggleSize::Lg => div(),
            };
            container = container.child(label_el.text_color(rgb(0xcccccc)).child(label));
        }

        // Click handler
        if !self.disabled {
            if let Some(handler) = self.on_change {
                let new_checked = !checked;
                container = container.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                    handler(new_checked, window, cx);
                });
            }
        }

        container
    }
}

impl IntoElement for Toggle {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
