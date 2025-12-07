//! Input component
//!
//! Text input field with optional label, placeholder, and validation.

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for input styling
#[derive(Debug, Clone)]
pub struct InputTheme {
    /// Background color
    pub background: Rgba,
    /// Filled variant background
    pub filled_bg: Rgba,
    /// Text color
    pub text: Rgba,
    /// Placeholder color
    pub placeholder: Rgba,
    /// Label color
    pub label: Rgba,
    /// Border color
    pub border: Rgba,
    /// Border hover color
    pub border_hover: Rgba,
    /// Error color
    pub error: Rgba,
}

impl Default for InputTheme {
    fn default() -> Self {
        Self {
            background: rgb(0x1e1e1e),
            filled_bg: rgb(0x2a2a2a),
            text: rgb(0xffffff),
            placeholder: rgb(0x666666),
            label: rgb(0xcccccc),
            border: rgb(0x3a3a3a),
            border_hover: rgb(0x007acc),
            error: rgb(0xcc3333),
        }
    }
}

impl From<&Theme> for InputTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            background: theme.background,
            filled_bg: theme.surface,
            text: theme.text_primary,
            placeholder: theme.text_muted,
            label: theme.text_secondary,
            border: theme.border,
            border_hover: theme.accent,
            error: theme.error,
        }
    }
}

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

/// Callback type for input changes
type OnChangeCallback = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>;

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
    bg_color: Option<Rgba>,
    text_color: Option<Rgba>,
    border_color: Option<Rgba>,
    placeholder_color: Option<Rgba>,
    on_change: Option<OnChangeCallback>,
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
            bg_color: None,
            text_color: None,
            border_color: None,
            placeholder_color: None,
            on_change: None,
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

    /// Set background color
    pub fn bg_color(mut self, color: impl Into<Rgba>) -> Self {
        self.bg_color = Some(color.into());
        self
    }

    /// Set text color
    pub fn text_color(mut self, color: impl Into<Rgba>) -> Self {
        self.text_color = Some(color.into());
        self
    }

    /// Set border color
    pub fn border_color(mut self, color: impl Into<Rgba>) -> Self {
        self.border_color = Some(color.into());
        self
    }

    /// Set placeholder color
    pub fn placeholder_color(mut self, color: impl Into<Rgba>) -> Self {
        self.placeholder_color = Some(color.into());
        self
    }

    /// Set change handler (called when input value changes)
    pub fn on_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Build into element with theme
    pub fn build_with_theme(self, theme: &InputTheme) -> Div {
        let (py, _text_size) = match self.size {
            InputSize::Sm => (px(4.0), "text_xs"),
            InputSize::Md => (px(8.0), "text_sm"),
            InputSize::Lg => (px(12.0), "text_base"),
        };

        let has_error = self.error.is_some();
        let border_color = if has_error {
            theme.error
        } else {
            self.border_color.unwrap_or(theme.border)
        };

        let mut container = div().flex().flex_col().gap_1();

        // Label
        if let Some(label) = self.label {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(theme.label)
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
                input_wrapper = input_wrapper.bg(self.bg_color.unwrap_or(theme.background));
            }
            InputVariant::Filled => {
                input_wrapper = input_wrapper
                    .bg(self.bg_color.unwrap_or(theme.filled_bg))
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

        let border_hover = theme.border_hover;
        if self.disabled {
            input_wrapper = input_wrapper.opacity(0.5).cursor_not_allowed();
        } else if !self.readonly {
            input_wrapper = input_wrapper.hover(move |s| s.border_color(border_hover));
        }

        let placeholder_color = self.placeholder_color.unwrap_or(theme.placeholder);
        let text_color = self.text_color.unwrap_or(theme.text);

        // Left icon
        if let Some(icon) = self.icon_left {
            input_wrapper = input_wrapper.child(div().text_color(placeholder_color).child(icon));
        }

        // Input text/placeholder
        let text_el = if self.value.is_empty() {
            if let Some(placeholder) = self.placeholder {
                div()
                    .flex_1()
                    .text_color(placeholder_color)
                    .child(placeholder)
            } else {
                div().flex_1()
            }
        } else {
            div().flex_1().text_color(text_color).child(self.value)
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
            input_wrapper = input_wrapper.child(div().text_color(placeholder_color).child(icon));
        }

        container = container.child(input_wrapper);

        // Error message
        if let Some(error) = self.error {
            container = container.child(div().text_xs().text_color(theme.error).child(error));
        }

        container
    }
}

impl RenderOnce for Input {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let input_theme = InputTheme::from(&global_theme);
        self.build_with_theme(&input_theme)
    }
}

impl IntoElement for Input {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
