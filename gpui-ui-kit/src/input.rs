//! Input component
//!
//! Text input field with optional label, placeholder, and validation.
//!
//! Features:
//! - Full keyboard text editing support
//! - Click to focus and start editing
//! - Enter to confirm, Escape to cancel
//! - Text selection visual feedback
//! - Disabled and readonly states

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
    /// Border focus color
    pub border_focus: Rgba,
    /// Error color
    pub error: Rgba,
    /// Cursor color
    pub cursor: Rgba,
    /// Selection background
    pub selection_bg: Rgba,
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
            border_focus: rgb(0x007acc),
            error: rgb(0xcc3333),
            cursor: rgb(0x007acc),
            selection_bg: rgba(0x007acc44),
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
            border_focus: theme.accent,
            error: theme.error,
            cursor: theme.accent,
            selection_bg: Rgba {
                r: theme.accent.r,
                g: theme.accent.g,
                b: theme.accent.b,
                a: 0.3,
            },
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

/// A text input component with full keyboard editing support
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
    editing: bool,
    text_selected: bool,
    edit_text: Option<SharedString>,
    on_change: Option<OnChangeCallback>,
    on_edit_start: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_edit_end: Option<Box<dyn Fn(Option<String>, &mut Window, &mut App) + 'static>>,
    on_text_change: Option<Box<dyn Fn(String, &mut Window, &mut App) + 'static>>,
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
            editing: false,
            text_selected: false,
            edit_text: None,
            on_change: None,
            on_edit_start: None,
            on_edit_end: None,
            on_text_change: None,
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

    /// Set whether the input is currently being edited
    pub fn editing(mut self, editing: bool) -> Self {
        self.editing = editing;
        self
    }

    /// Set whether the text is fully selected (for visual feedback)
    pub fn text_selected(mut self, selected: bool) -> Self {
        self.text_selected = selected;
        self
    }

    /// Set the current edit text (when editing)
    pub fn edit_text(mut self, text: impl Into<SharedString>) -> Self {
        self.edit_text = Some(text.into());
        self
    }

    /// Set change handler (called when input value changes)
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set edit start handler (called when user clicks on input to edit)
    pub fn on_edit_start(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit_start = Some(Box::new(handler));
        self
    }

    /// Set edit end handler (called when user confirms or cancels edit)
    /// The Option<String> is Some(value) if confirmed, None if cancelled
    pub fn on_edit_end(
        mut self,
        handler: impl Fn(Option<String>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit_end = Some(Box::new(handler));
        self
    }

    /// Set text change handler (called when edit text changes)
    pub fn on_text_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_text_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Input {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = InputTheme::from(&global_theme);

        let (py, _text_size) = match self.size {
            InputSize::Sm => (px(4.0), "text_xs"),
            InputSize::Md => (px(8.0), "text_sm"),
            InputSize::Lg => (px(12.0), "text_base"),
        };

        let has_error = self.error.is_some();
        let disabled = self.disabled;
        let readonly = self.readonly;
        let editing = self.editing;
        let text_selected = self.text_selected;
        let current_value = self.value.clone();
        let edit_text_clone = self.edit_text.clone();

        let border_color = if has_error {
            theme.error
        } else if editing {
            theme.border_focus
        } else {
            self.border_color.unwrap_or(theme.border)
        };

        let mut container = div().flex().flex_col().gap_1();

        // Label
        if let Some(label) = &self.label {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(theme.label)
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            );
        }

        // Create a unique ID for the input field
        let field_id = ElementId::Name(format!("{:?}-field", self.id).into());

        // Input wrapper
        let mut input_wrapper = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py(py)
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .focusable(); // Make focusable for keyboard events

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
        if disabled {
            input_wrapper = input_wrapper.opacity(0.5).cursor_not_allowed();
        } else if !readonly {
            input_wrapper = input_wrapper
                .cursor_text()
                .hover(move |s| s.border_color(border_hover));
        }

        let placeholder_color = self.placeholder_color.unwrap_or(theme.placeholder);
        let text_color = self.text_color.unwrap_or(theme.text);
        let selection_bg = theme.selection_bg;

        // Wrap handlers in Rc for sharing
        let _on_change_rc = self.on_change.map(|h| std::rc::Rc::new(h));
        let on_edit_start_rc = self.on_edit_start.map(|h| std::rc::Rc::new(h));
        let on_edit_end_rc = self.on_edit_end.map(|h| std::rc::Rc::new(h));
        let on_text_change_rc = self.on_text_change.map(|h| std::rc::Rc::new(h));

        // Add click handler to start editing
        if !disabled && !readonly && !editing {
            if let Some(ref handler_rc) = on_edit_start_rc {
                let handler = handler_rc.clone();
                input_wrapper =
                    input_wrapper.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        handler(window, cx);
                    });
            }
        }

        // Add keyboard event handling
        if !disabled && !readonly {
            let on_edit_end_key = on_edit_end_rc.clone();
            let on_text_change_key = on_text_change_rc.clone();
            let is_editing = editing;
            let edit_text_for_key = edit_text_clone.clone();

            input_wrapper = input_wrapper.on_key_down(move |event, window, cx| {
                if is_editing {
                    // Editing mode keyboard handling
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            // Confirm edit
                            if let Some(ref handler) = on_edit_end_key {
                                let text = edit_text_for_key
                                    .as_ref()
                                    .map(|s| s.to_string())
                                    .unwrap_or_default();
                                handler(Some(text), window, cx);
                            }
                        }
                        "escape" => {
                            // Cancel edit
                            if let Some(ref handler) = on_edit_end_key {
                                handler(None, window, cx);
                            }
                        }
                        _ => {
                            // Handle text input
                            if let Some(ref handler) = on_text_change_key {
                                let current = edit_text_for_key
                                    .as_ref()
                                    .map(|s| s.to_string())
                                    .unwrap_or_default();

                                // Handle backspace
                                let new_text = if event.keystroke.key == "backspace" {
                                    if current.is_empty() {
                                        current
                                    } else {
                                        current[..current.len() - 1].to_string()
                                    }
                                } else if event.keystroke.key.len() == 1 {
                                    // Single character - append
                                    let ch = event.keystroke.key.chars().next().unwrap();
                                    format!("{}{}", current, ch)
                                } else {
                                    current
                                };

                                handler(new_text, window, cx);
                            }
                        }
                    }
                }
            });
        }

        // Left icon
        if let Some(icon) = &self.icon_left {
            input_wrapper =
                input_wrapper.child(div().text_color(placeholder_color).child(icon.clone()));
        }

        // Determine display text
        let display_text = if editing {
            edit_text_clone
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| current_value.to_string())
        } else if current_value.is_empty() {
            self.placeholder
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default()
        } else {
            current_value.to_string()
        };

        // Visual selection highlight: when text_selected is true, show accent background
        let (value_bg, value_text_color) = if editing && text_selected {
            // Selected text: selection background with text color
            (Some(selection_bg), text_color)
        } else if !editing && current_value.is_empty() {
            // Placeholder text
            (None, placeholder_color)
        } else {
            // Normal text
            (None, text_color)
        };

        let mut text_el = div()
            .id(field_id)
            .flex_1()
            .text_color(value_text_color)
            .child(display_text);

        // Apply selection background if selected
        if let Some(bg) = value_bg {
            text_el = text_el.bg(bg);
        }

        // Apply text size
        text_el = match self.size {
            InputSize::Sm => text_el.text_xs(),
            InputSize::Md => text_el.text_sm(),
            InputSize::Lg => text_el,
        };

        // Note: handlers moved to input_wrapper

        input_wrapper = input_wrapper.child(text_el);

        // Right icon
        if let Some(icon) = &self.icon_right {
            input_wrapper =
                input_wrapper.child(div().text_color(placeholder_color).child(icon.clone()));
        }

        container = container.child(input_wrapper);

        // Error message
        if let Some(error) = &self.error {
            container =
                container.child(div().text_xs().text_color(theme.error).child(error.clone()));
        }

        container
    }
}

impl IntoElement for Input {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
