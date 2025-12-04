//! Select/Dropdown component
//!
//! A dropdown select component for choosing from options with theming support.

use gpui::prelude::*;
use gpui::*;

/// Theme colors for select styling
#[derive(Debug, Clone)]
pub struct SelectTheme {
    /// Trigger background color
    pub trigger_bg: Rgba,
    /// Trigger border color
    pub trigger_border: Rgba,
    /// Trigger border color on hover
    pub trigger_border_hover: Rgba,
    /// Trigger border color when focused/open
    pub trigger_border_focused: Rgba,
    /// Dropdown background color
    pub dropdown_bg: Rgba,
    /// Dropdown border color
    pub dropdown_border: Rgba,
    /// Selected option background
    pub selected_bg: Rgba,
    /// Option hover background
    pub option_hover_bg: Rgba,
    /// Label text color
    pub label_color: Rgba,
    /// Text color for selected value
    pub text_color: Rgba,
    /// Placeholder text color
    pub placeholder_color: Rgba,
    /// Option text color
    pub option_text_color: Rgba,
    /// Selected option text color
    pub selected_text_color: Rgba,
    /// Disabled text color
    pub disabled_color: Rgba,
    /// Arrow/chevron color
    pub arrow_color: Rgba,
}

impl Default for SelectTheme {
    fn default() -> Self {
        Self {
            trigger_bg: rgba(0x1e1e1eff),
            trigger_border: rgba(0x3a3a3aff),
            trigger_border_hover: rgba(0x007accff),
            trigger_border_focused: rgba(0x007accff),
            dropdown_bg: rgba(0x2a2a2aff),
            dropdown_border: rgba(0x3a3a3aff),
            selected_bg: rgba(0x007accff),
            option_hover_bg: rgba(0x3a3a3aff),
            label_color: rgba(0xccccccff),
            text_color: rgba(0xffffffff),
            placeholder_color: rgba(0x666666ff),
            option_text_color: rgba(0xccccccff),
            selected_text_color: rgba(0xffffffff),
            disabled_color: rgba(0x666666ff),
            arrow_color: rgba(0x666666ff),
        }
    }
}

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

/// A select dropdown component with theming support
pub struct Select {
    id: ElementId,
    options: Vec<SelectOption>,
    selected: Option<SharedString>,
    placeholder: Option<SharedString>,
    label: Option<SharedString>,
    size: SelectSize,
    disabled: bool,
    is_open: bool,
    theme: Option<SelectTheme>,
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
            theme: None,
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

    /// Set theme
    pub fn theme(mut self, theme: SelectTheme) -> Self {
        self.theme = Some(theme);
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
        let default_theme = SelectTheme::default();
        let theme = self.theme.as_ref().unwrap_or(&default_theme);

        let (py, _text_size_class) = match self.size {
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
                    .text_color(theme.label_color)
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
        let border_color = if self.is_open {
            theme.trigger_border_focused
        } else {
            theme.trigger_border
        };

        let mut trigger = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py(py)
            .min_w(px(120.0))
            .bg(theme.trigger_bg)
            .border_1()
            .border_color(border_color)
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
            let hover_border = theme.trigger_border_hover;
            trigger = trigger.hover(move |s| s.border_color(hover_border));
        }

        // Display value or placeholder
        let display_text = if let Some(label) = selected_label {
            div().text_color(theme.text_color).child(label)
        } else if let Some(placeholder) = self.placeholder {
            div().text_color(theme.placeholder_color).child(placeholder)
        } else {
            div().text_color(theme.placeholder_color).child("Select...")
        };

        trigger = trigger.child(display_text);

        // Dropdown arrow
        trigger = trigger.child(div().text_xs().text_color(theme.arrow_color).child("▼"));

        container = container.child(trigger);

        // Dropdown menu (only shown when open)
        if self.is_open {
            let mut dropdown = div()
                .id("select-dropdown")
                .absolute()
                .top_full()
                .left_0()
                .right_0()
                .mt_1()
                .bg(theme.dropdown_bg)
                .border_1()
                .border_color(theme.dropdown_border)
                .rounded_md()
                .shadow_lg()
                .max_h(px(200.0))
                .overflow_y_scroll()
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
                    option_el = option_el
                        .text_color(theme.disabled_color)
                        .cursor_not_allowed();
                } else if is_selected {
                    option_el = option_el
                        .bg(theme.selected_bg)
                        .text_color(theme.selected_text_color);
                } else {
                    let hover_bg = theme.option_hover_bg;
                    option_el = option_el
                        .text_color(theme.option_text_color)
                        .hover(move |s| s.bg(hover_bg));

                    // Add click handler for non-disabled, non-selected options
                    if let Some(ref handler) = self.on_change {
                        let handler_ptr: *const dyn Fn(&SharedString, &mut Window, &mut App) =
                            handler.as_ref() as *const _;
                        option_el = option_el.on_mouse_up(
                            MouseButton::Left,
                            move |_event, window, cx| unsafe {
                                (*handler_ptr)(&option_value, window, cx);
                            },
                        );
                    }
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
