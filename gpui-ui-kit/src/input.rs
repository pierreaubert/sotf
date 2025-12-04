//! Input component
//!
//! Text input field with optional label, placeholder, and validation.

use gpui::prelude::*;
use gpui::*;

/// Input size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputSize {
    /// Small input
    Sm,
    /// Medium input (default)
    #[default]
    Md,
    /// Large input
    Lg,
}

/// Input visual variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputVariant {
    /// Default input style
    #[default]
    Default,
    /// Filled background
    Filled,
    /// Flushed (bottom border only)
    Flushed,
}

/// A text input component
pub struct Input {
    id: ElementId,
    value: SharedString,
    placeholder: Option<SharedString>,
    label: Option<SharedString>,
    size: InputSize,
    variant: InputVariant,
    disabled: bool,
    readonly: bool,
    error: Option<SharedString>,
    icon_left: Option<SharedString>,
    icon_right: Option<SharedString>,
}

impl Input {
    /// Create a new input
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: "".into(),
            placeholder: None,
            label: None,
            size: InputSize::default(),
            variant: InputVariant::default(),
            disabled: false,
            readonly: false,
            error: None,
            icon_left: None,
            icon_right: None,
        }
    }

    /// Set the input value
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// Set placeholder text
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set label text
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set input size
    pub fn size(mut self, size: InputSize) -> Self {
        self.size = size;
        self
    }

    /// Set input variant
    pub fn variant(mut self, variant: InputVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set readonly state
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Set error message
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Set left icon
    pub fn icon_left(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_left = Some(icon.into());
        self
    }

    /// Set right icon
    pub fn icon_right(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_right = Some(icon.into());
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let (py, _text_size) = match self.size {
            InputSize::Sm => (px(4.0), "text_xs"),
            InputSize::Md => (px(8.0), "text_sm"),
            InputSize::Lg => (px(12.0), "text_base"),
        };

        let has_error = self.error.is_some();
        let border_color = if has_error {
            rgb(0xcc3333)
        } else {
            rgb(0x3a3a3a)
        };

        let mut container = div().flex().flex_col().gap_1();

        // Label
        if let Some(label) = self.label {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xcccccc))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            );
        }

        // Input wrapper
        let mut input_wrapper = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py(py)
            .rounded_md()
            .border_1()
            .border_color(border_color);

        // Apply variant styling
        match self.variant {
            InputVariant::Default => {
                input_wrapper = input_wrapper.bg(rgb(0x1e1e1e));
            }
            InputVariant::Filled => {
                input_wrapper = input_wrapper
                    .bg(rgb(0x2a2a2a))
                    .border_color(rgba(0x00000000));
            }
            InputVariant::Flushed => {
                input_wrapper = input_wrapper
                    .bg(rgba(0x00000000))
                    .border_0()
                    .border_b_1()
                    .border_color(border_color)
                    .rounded_none();
            }
        }

        if self.disabled {
            input_wrapper = input_wrapper.opacity(0.5).cursor_not_allowed();
        } else if !self.readonly {
            input_wrapper = input_wrapper.hover(|s| s.border_color(rgb(0x007acc)));
        }

        // Left icon
        if let Some(icon) = self.icon_left {
            input_wrapper = input_wrapper.child(div().text_color(rgb(0x666666)).child(icon));
        }

        // Input text/placeholder
        let text_el = if self.value.is_empty() {
            if let Some(placeholder) = self.placeholder {
                div().flex_1().text_color(rgb(0x666666)).child(placeholder)
            } else {
                div().flex_1()
            }
        } else {
            div().flex_1().text_color(rgb(0xffffff)).child(self.value)
        };

        // Apply text size
        let text_el = match self.size {
            InputSize::Sm => text_el.text_xs(),
            InputSize::Md => text_el.text_sm(),
            InputSize::Lg => text_el,
        };

        input_wrapper = input_wrapper.child(text_el);

        // Right icon
        if let Some(icon) = self.icon_right {
            input_wrapper = input_wrapper.child(div().text_color(rgb(0x666666)).child(icon));
        }

        container = container.child(input_wrapper);

        // Error message
        if let Some(error) = self.error {
            container = container.child(div().text_xs().text_color(rgb(0xcc3333)).child(error));
        }

        container
    }
}

impl IntoElement for Input {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
