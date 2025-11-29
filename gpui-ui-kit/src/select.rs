//! Select/Dropdown component
//!
//! A dropdown select component for choosing from options.

use gpui::prelude::*;
use gpui::*;

/// Select size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectSize {
    /// Small
    Sm,
    /// Medium (default)
    #[default]
    Md,
    /// Large
    Lg,
}

/// A select option
#[derive(Clone)]
pub struct SelectOption {
    /// Option value
    pub value: SharedString,
    /// Display label
    pub label: SharedString,
    /// Whether option is disabled
    pub disabled: bool,
}

impl SelectOption {
    /// Create a new select option
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A select dropdown component
pub struct Select {
    id: ElementId,
    options: Vec<SelectOption>,
    selected: Option<SharedString>,
    placeholder: Option<SharedString>,
    label: Option<SharedString>,
    size: SelectSize,
    disabled: bool,
    is_open: bool,
    on_change: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
}

impl Select {
    /// Create a new select
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            selected: None,
            placeholder: None,
            label: None,
            size: SelectSize::default(),
            disabled: false,
            is_open: false,
            on_change: None,
        }
    }

    /// Set options
    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    /// Set selected value
    pub fn selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected = Some(value.into());
        self
    }

    /// Set placeholder
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set size
    pub fn size(mut self, size: SelectSize) -> Self {
        self.size = size;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set open state (for controlled component)
    pub fn is_open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Set change handler
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let (py, text_size_class) = match self.size {
            SelectSize::Sm => (px(4.0), "sm"),
            SelectSize::Md => (px(8.0), "md"),
            SelectSize::Lg => (px(12.0), "lg"),
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

        // Find selected option label
        let selected_label = self.selected.as_ref().and_then(|val| {
            self.options
                .iter()
                .find(|o| &o.value == val)
                .map(|o| o.label.clone())
        });

        // Select trigger
        let mut trigger = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py(py)
            .min_w(px(120.0))
            .bg(rgb(0x1e1e1e))
            .border_1()
            .border_color(rgb(0x3a3a3a))
            .rounded_md()
            .cursor_pointer();

        // Apply text size
        trigger = match self.size {
            SelectSize::Sm => trigger.text_xs(),
            SelectSize::Md => trigger.text_sm(),
            SelectSize::Lg => trigger,
        };

        if self.disabled {
            trigger = trigger.opacity(0.5).cursor_not_allowed();
        } else {
            trigger = trigger.hover(|s| s.border_color(rgb(0x007acc)));
        }

        // Display value or placeholder
        let display_text = if let Some(label) = selected_label {
            div().text_color(rgb(0xffffff)).child(label)
        } else if let Some(placeholder) = self.placeholder {
            div().text_color(rgb(0x666666)).child(placeholder)
        } else {
            div().text_color(rgb(0x666666)).child("Select...")
        };

        trigger = trigger.child(display_text);

        // Dropdown arrow
        trigger = trigger.child(div().text_xs().text_color(rgb(0x666666)).child("▼"));

        container = container.child(trigger);

        // Dropdown menu (only shown when open)
        if self.is_open {
            let mut dropdown = div()
                .absolute()
                .top_full()
                .left_0()
                .right_0()
                .mt_1()
                .bg(rgb(0x2a2a2a))
                .border_1()
                .border_color(rgb(0x3a3a3a))
                .rounded_md()
                .shadow_lg()
                .max_h(px(200.0))
                .py_1();

            for option in self.options {
                let is_selected = self.selected.as_ref() == Some(&option.value);
                let option_value = option.value.clone();

                let mut option_el = div().px_3().py(px(6.0)).cursor_pointer();

                // Apply text size
                option_el = match self.size {
                    SelectSize::Sm => option_el.text_xs(),
                    SelectSize::Md => option_el.text_sm(),
                    SelectSize::Lg => option_el,
                };

                if option.disabled {
                    option_el = option_el.text_color(rgb(0x666666)).cursor_not_allowed();
                } else if is_selected {
                    option_el = option_el.bg(rgb(0x007acc)).text_color(rgb(0xffffff));
                } else {
                    option_el = option_el
                        .text_color(rgb(0xcccccc))
                        .hover(|s| s.bg(rgb(0x3a3a3a)));
                }

                option_el = option_el.child(option.label);
                dropdown = dropdown.child(option_el);
            }

            container = container.relative().child(dropdown);
        }

        container
    }
}

impl IntoElement for Select {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
