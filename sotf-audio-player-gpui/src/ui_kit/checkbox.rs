//! Checkbox component
//!
//! A checkbox input with optional label.

use gpui::prelude::*;
use gpui::*;

/// Checkbox size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckboxSize {
    /// Small (14px)
    Sm,
    /// Medium (18px, default)
    #[default]
    Md,
    /// Large (22px)
    Lg,
}

impl CheckboxSize {
    fn size(&self) -> Pixels {
        match self {
            CheckboxSize::Sm => px(14.0),
            CheckboxSize::Md => px(18.0),
            CheckboxSize::Lg => px(22.0),
        }
    }
}

/// A checkbox component
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    indeterminate: bool,
    label: Option<SharedString>,
    size: CheckboxSize,
    disabled: bool,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
    /// Create a new checkbox
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            indeterminate: false,
            label: None,
            size: CheckboxSize::default(),
            disabled: false,
            on_change: None,
        }
    }

    /// Set checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set indeterminate state
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set size
    pub fn size(mut self, size: CheckboxSize) -> Self {
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
        let size = self.size.size();
        let checked = self.checked;
        let indeterminate = self.indeterminate;

        let (bg, border_color) = if checked || indeterminate {
            (rgb(0x007acc), rgb(0x007acc))
        } else {
            (rgba(0x00000000), rgb(0x555555))
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

        // Checkbox box
        let mut checkbox = div()
            .flex()
            .items_center()
            .justify_center()
            .w(size)
            .h(size)
            .rounded(px(3.0))
            .border_1()
            .border_color(border_color)
            .bg(bg);

        // Check mark or indeterminate line
        if indeterminate {
            checkbox = checkbox.child(
                div()
                    .w(size - px(6.0))
                    .h(px(2.0))
                    .bg(rgb(0xffffff))
                    .rounded(px(1.0)),
            );
        } else if checked {
            checkbox = checkbox.child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child("✓"),
            );
        }

        if !self.disabled {
            checkbox = checkbox.hover(|s| s.border_color(rgb(0x007acc)));
        }

        container = container.child(checkbox);

        // Label
        if let Some(label) = self.label {
            let label_el = match self.size {
                CheckboxSize::Sm => div().text_xs(),
                CheckboxSize::Md => div().text_sm(),
                CheckboxSize::Lg => div(),
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

impl IntoElement for Checkbox {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
