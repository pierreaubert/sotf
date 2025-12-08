//! NumberInput component for numeric value entry
//!
//! A numeric input field with:
//! - Increment/decrement buttons (+ and -)
//! - Direct text editing of the value
//! - Keyboard navigation:
//!   - Arrow Up/Right: increase value
//!   - Arrow Down/Left: decrease value
//!   - Enter: confirm edit
//!   - Escape: cancel edit
//! - Scroll wheel adjustment
//! - Configurable step size, min/max bounds
//! - Value formatting (decimals, units)

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for number input styling
#[derive(Debug, Clone)]
pub struct NumberInputTheme {
    /// Background color
    pub background: Rgba,
    /// Text color
    pub text: Rgba,
    /// Button background
    pub button_bg: Rgba,
    /// Button hover background
    pub button_hover: Rgba,
    /// Button active (pressed) background
    pub button_active: Rgba,
    /// Button text color
    pub button_text: Rgba,
    /// Border color
    pub border: Rgba,
    /// Border focus color
    pub border_focus: Rgba,
    /// Label color
    pub label: Rgba,
    /// Disabled opacity
    pub disabled_opacity: f32,
}

impl Default for NumberInputTheme {
    fn default() -> Self {
        Self {
            background: rgba(0x1e1e1eff),
            text: rgba(0xffffffff),
            button_bg: rgba(0x2a2a2aff),
            button_hover: rgba(0x3a3a3aff),
            button_active: rgba(0x007accff),
            button_text: rgba(0xccccccff),
            border: rgba(0x3a3a3aff),
            border_focus: rgba(0x007accff),
            label: rgba(0xaaaaaaff),
            disabled_opacity: 0.5,
        }
    }
}

impl From<&Theme> for NumberInputTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            background: theme.background,
            text: theme.text_primary,
            button_bg: theme.surface,
            button_hover: theme.surface_hover,
            button_active: theme.accent,
            button_text: theme.text_secondary,
            border: theme.border,
            border_focus: theme.accent,
            label: theme.text_secondary,
            disabled_opacity: 0.5,
        }
    }
}

/// Number input size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberInputSize {
    /// Small size
    Sm,
    /// Medium size (default)
    #[default]
    Md,
    /// Large size
    Lg,
}

impl NumberInputSize {
    fn height(&self) -> f32 {
        match self {
            Self::Sm => 24.0,
            Self::Md => 32.0,
            Self::Lg => 40.0,
        }
    }

    fn button_width(&self) -> f32 {
        match self {
            Self::Sm => 20.0,
            Self::Md => 28.0,
            Self::Lg => 36.0,
        }
    }

    fn font_size(&self) -> f32 {
        match self {
            Self::Sm => 11.0,
            Self::Md => 13.0,
            Self::Lg => 15.0,
        }
    }

    fn padding(&self) -> f32 {
        match self {
            Self::Sm => 4.0,
            Self::Md => 8.0,
            Self::Lg => 12.0,
        }
    }
}

/// A numeric input component with increment/decrement buttons
#[derive(IntoElement)]
pub struct NumberInput {
    id: ElementId,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    unit: Option<SharedString>,
    label: Option<SharedString>,
    size: NumberInputSize,
    width: Option<f32>,
    disabled: bool,
    editing: bool,
    text_selected: bool,
    edit_text: Option<SharedString>,
    theme: Option<NumberInputTheme>,
    on_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_edit_start: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_edit_end: Option<Box<dyn Fn(Option<f64>, &mut Window, &mut App) + 'static>>,
    on_text_change: Option<Box<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl NumberInput {
    /// Create a new number input with the given ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            decimals: 0,
            unit: None,
            label: None,
            size: NumberInputSize::default(),
            width: None,
            disabled: false,
            editing: false,
            text_selected: false,
            edit_text: None,
            theme: None,
            on_change: None,
            on_edit_start: None,
            on_edit_end: None,
            on_text_change: None,
        }
    }

    /// Set the current value
    pub fn value(mut self, value: f64) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    /// Set the minimum value
    pub fn min(mut self, min: f64) -> Self {
        self.min = min;
        self
    }

    /// Set the maximum value
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// Set the step size for increment/decrement
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Set the number of decimal places to display
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Set the unit suffix (e.g., "Hz", "dB", "%")
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the size variant
    pub fn size(mut self, size: NumberInputSize) -> Self {
        self.size = size;
        self
    }

    /// Set fixed width (optional)
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// Set the theme
    pub fn theme(mut self, theme: NumberInputTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set value change handler (called on button click, scroll, keyboard arrows)
    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set edit start handler (called when user clicks on value to edit)
    pub fn on_edit_start(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit_start = Some(Box::new(handler));
        self
    }

    /// Set edit end handler (called when user confirms or cancels edit)
    /// The Option<f64> is Some(value) if confirmed, None if cancelled
    pub fn on_edit_end(
        mut self,
        handler: impl Fn(Option<f64>, &mut Window, &mut App) + 'static,
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

    /// Format the current value for display
    fn format_value(&self) -> String {
        let formatted = format!("{:.prec$}", self.value, prec = self.decimals);
        if let Some(ref unit) = self.unit {
            format!("{} {}", formatted, unit)
        } else {
            formatted
        }
    }

    /// Parse a string to a value, respecting bounds
    fn parse_value(&self, text: &str) -> Option<f64> {
        // Remove unit suffix if present
        let text = if let Some(ref unit) = self.unit {
            text.trim().trim_end_matches(unit.as_ref()).trim()
        } else {
            text.trim()
        };

        text.parse::<f64>()
            .ok()
            .map(|v| v.clamp(self.min, self.max))
    }
}

impl RenderOnce for NumberInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let default_theme = NumberInputTheme::from(&global_theme);
        let theme = self.theme.clone().unwrap_or(default_theme);

        let height = self.size.height();
        let button_width = self.size.button_width();
        let padding = self.size.padding();
        let disabled = self.disabled;
        let editing = self.editing;
        let text_selected = self.text_selected;
        let current_value = self.value;
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let decimals = self.decimals;
        let unit_clone = self.unit.clone();
        let edit_text_clone = self.edit_text.clone();

        // Create unique child IDs based on parent ID
        let parent_id = format!("{:?}", self.id);
        let dec_id = ElementId::Name(format!("{}-dec", parent_id).into());
        let value_id = ElementId::Name(format!("{}-value", parent_id).into());
        let inc_id = ElementId::Name(format!("{}-inc", parent_id).into());

        // Wrap handlers in Rc for sharing
        let on_change_rc = self.on_change.map(|h| std::rc::Rc::new(h));
        let on_edit_start_rc = self.on_edit_start.map(|h| std::rc::Rc::new(h));
        let on_edit_end_rc = self.on_edit_end.map(|h| std::rc::Rc::new(h));
        let on_text_change_rc = self.on_text_change.map(|h| std::rc::Rc::new(h));

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

        // Input row: [−] [value] [+]
        let mut input_row = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .h(px(height))
            .rounded_md()
            .border_1()
            .border_color(if editing {
                theme.border_focus
            } else {
                theme.border
            })
            .bg(theme.background)
            .overflow_hidden();

        if let Some(width) = self.width {
            input_row = input_row.w(px(width));
        }

        if disabled {
            input_row = input_row.opacity(theme.disabled_opacity);
        }

        // Decrement button (−)
        let button_bg = theme.button_bg;
        let button_hover = theme.button_hover;
        let button_active = theme.button_active;
        let button_text = theme.button_text;
        let text_color = theme.text;

        let mut dec_button = div()
            .id(dec_id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(button_width))
            .h_full()
            .bg(button_bg)
            .text_color(button_text)
            .font_weight(FontWeight::BOLD)
            .child("−");

        if !disabled {
            dec_button = dec_button
                .cursor_pointer()
                .hover(move |s| s.bg(button_hover))
                .active(move |s| s.bg(button_active));

            if let Some(ref handler_rc) = on_change_rc {
                let handler = handler_rc.clone();
                dec_button = dec_button.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    let new_value = (current_value - step).clamp(min, max);
                    handler(new_value, window, cx);
                });
            }
        } else {
            dec_button = dec_button.cursor_not_allowed();
        }

        input_row = input_row.child(dec_button);

        // Value display / edit field
        let display_text = if editing {
            edit_text_clone
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let formatted = format!("{:.prec$}", current_value, prec = decimals);
                    if let Some(ref unit) = unit_clone {
                        format!("{} {}", formatted, unit)
                    } else {
                        formatted
                    }
                })
        } else {
            let formatted = format!("{:.prec$}", current_value, prec = decimals);
            if let Some(ref unit) = unit_clone {
                format!("{} {}", formatted, unit)
            } else {
                formatted
            }
        };

        // Visual selection highlight: when text_selected is true, show accent background
        let (value_bg, value_text_color) = if editing && text_selected {
            // Selected text: accent background with contrasting text
            (Some(theme.button_active), rgba(0xffffffff))
        } else {
            (None, text_color)
        };

        let mut value_field = div()
            .id(value_id)
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .px(px(padding))
            .text_color(value_text_color)
            .child(display_text);

        // Apply selection background if selected
        if let Some(bg) = value_bg {
            value_field = value_field.bg(bg);
        }

        // Apply font size
        value_field = match self.size {
            NumberInputSize::Sm => value_field.text_xs(),
            NumberInputSize::Md => value_field.text_sm(),
            NumberInputSize::Lg => value_field,
        };

        if !disabled && !editing {
            // Click to start editing
            value_field = value_field.cursor_text();
            if let Some(ref handler_rc) = on_edit_start_rc {
                let handler = handler_rc.clone();
                value_field = value_field.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    handler(window, cx);
                });
            }
        }

        // Keyboard handling for the value field
        if !disabled {
            // Clone handlers for keyboard events
            let on_change_key = on_change_rc.clone();
            let on_edit_end_key = on_edit_end_rc.clone();
            let on_text_change_key = on_text_change_rc.clone();
            let is_editing = editing;
            let edit_text_for_key = edit_text_clone.clone();
            let unit_for_key = unit_clone.clone();

            value_field = value_field.on_key_down(move |event, window, cx| {
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

                                // Remove unit suffix if present
                                let text = if let Some(ref unit) = unit_for_key {
                                    text.trim().trim_end_matches(unit.as_ref()).trim()
                                } else {
                                    text.trim()
                                };

                                let parsed = text.parse::<f64>().ok().map(|v| v.clamp(min, max));
                                handler(parsed, window, cx);
                            }
                        }
                        "escape" => {
                            // Cancel edit
                            if let Some(ref handler) = on_edit_end_key {
                                handler(None, window, cx);
                            }
                        }
                        _ => {
                            // Handle text input (simplified - real implementation would need full text editing)
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
                                    // Single character - append if valid for number
                                    let ch = event.keystroke.key.chars().next().unwrap();
                                    if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' {
                                        format!("{}{}", current, ch)
                                    } else {
                                        current
                                    }
                                } else {
                                    current
                                };

                                handler(new_text, window, cx);
                            }
                        }
                    }
                } else {
                    // Non-editing mode - arrow keys adjust value
                    if let Some(ref handler) = on_change_key {
                        let new_value = match event.keystroke.key.as_str() {
                            "up" | "right" => Some((current_value + step).clamp(min, max)),
                            "down" | "left" => Some((current_value - step).clamp(min, max)),
                            _ => None,
                        };

                        if let Some(v) = new_value {
                            handler(v, window, cx);
                        }
                    }
                }
            });
        }

        input_row = input_row.child(value_field);

        // Increment button (+)
        let mut inc_button = div()
            .id(inc_id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(button_width))
            .h_full()
            .bg(button_bg)
            .text_color(button_text)
            .font_weight(FontWeight::BOLD)
            .child("+");

        if !disabled {
            inc_button = inc_button
                .cursor_pointer()
                .hover(move |s| s.bg(button_hover))
                .active(move |s| s.bg(button_active));

            if let Some(ref handler_rc) = on_change_rc {
                let handler = handler_rc.clone();
                inc_button = inc_button.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    let new_value = (current_value + step).clamp(min, max);
                    handler(new_value, window, cx);
                });
            }
        } else {
            inc_button = inc_button.cursor_not_allowed();
        }

        input_row = input_row.child(inc_button);

        // Scroll wheel on the whole input
        if !disabled {
            if let Some(ref handler_rc) = on_change_rc {
                let handler_scroll = handler_rc.clone();
                input_row = input_row.on_scroll_wheel(move |event, window, cx| {
                    let delta = event.delta.pixel_delta(px(20.0)).y;
                    let scroll_up = delta < px(0.0);
                    let change = if scroll_up { step } else { -step };
                    let new_value = (current_value + change).clamp(min, max);
                    handler_scroll(new_value, window, cx);
                });
            }
        }

        container.child(input_row)
    }
}
