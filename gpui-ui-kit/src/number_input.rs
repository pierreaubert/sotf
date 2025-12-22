//! NumberInput component for numeric value entry
//!
//! A numeric input field with:
//! - Increment/decrement buttons (+ and -)
//! - Direct text editing of the value (click on value to edit)
//! - Keyboard navigation:
//!   - Arrow Up/Right: increase value
//!   - Arrow Down/Left: decrease value
//!   - Enter: confirm edit
//!   - Escape: cancel edit
//! - Scroll wheel adjustment
//! - Configurable step size, min/max bounds
//! - Value formatting (decimals, units)
//!
//! The component handles its own editing state internally - just provide
//! an `on_change` callback to receive value updates.

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Thread-local registry for focus handles, keyed by element ID.
thread_local! {
    static NUMBER_INPUT_FOCUS_HANDLES: RefCell<HashMap<ElementId, FocusHandle>> = RefCell::new(HashMap::new());
}

// Thread-local registry for edit state, keyed by element ID.
thread_local! {
    static NUMBER_INPUT_EDIT_STATES: RefCell<HashMap<ElementId, Rc<RefCell<NumberEditState>>>> = RefCell::new(HashMap::new());
}

/// Internal editing state for the number input
#[derive(Clone, Default)]
struct NumberEditState {
    /// Whether currently editing
    editing: bool,
    /// Current edit text
    text: String,
    /// Cursor position (character index)
    cursor: usize,
    /// Whether all text is selected
    text_selected: bool,
}

impl NumberEditState {
    fn new(value: &str) -> Self {
        Self {
            editing: true,
            text: value.to_string(),
            cursor: value.chars().count(),
            text_selected: true,
        }
    }

    fn select_all(&mut self) {
        self.text_selected = true;
        self.cursor = self.text.chars().count();
    }

    fn do_backspace(&mut self) {
        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        } else if self.cursor > 0 {
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.remove(self.cursor - 1);
            self.text = chars.into_iter().collect();
            self.cursor -= 1;
        }
    }

    fn do_delete(&mut self) {
        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        } else {
            let len = self.text.chars().count();
            if self.cursor < len {
                let mut chars: Vec<char> = self.text.chars().collect();
                chars.remove(self.cursor);
                self.text = chars.into_iter().collect();
            }
        }
    }

    fn insert_char(&mut self, ch: char) {
        // Only allow valid numeric characters
        if !ch.is_ascii_digit() && ch != '.' && ch != '-' && ch != '+' {
            return;
        }

        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        }

        let mut chars: Vec<char> = self.text.chars().collect();
        chars.insert(self.cursor, ch);
        self.text = chars.into_iter().collect();
        self.cursor += 1;
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.text_selected = false;
    }

    fn move_right(&mut self) {
        let len = self.text.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
        self.text_selected = false;
    }

    fn move_to_start(&mut self) {
        self.cursor = 0;
        self.text_selected = false;
    }

    fn move_to_end(&mut self) {
        self.cursor = self.text.chars().count();
        self.text_selected = false;
    }
}

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
///
/// The component handles its own editing state internally. Just provide
/// an `on_change` callback to receive value updates.
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
    theme: Option<NumberInputTheme>,
    on_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
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
            theme: None,
            on_change: None,
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

    /// Set the theme
    pub fn theme(mut self, theme: NumberInputTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set value change handler (called on button click, scroll, keyboard, or text edit confirm)
    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Format value for display
    fn format_value_str(value: f64, decimals: usize, unit: Option<&SharedString>) -> String {
        let formatted = format!("{:.prec$}", value, prec = decimals);
        if let Some(unit) = unit {
            format!("{} {}", formatted, unit)
        } else {
            formatted
        }
    }

    /// Parse a string to a value, removing unit suffix
    fn parse_value_str(text: &str, unit: Option<&SharedString>, min: f64, max: f64) -> Option<f64> {
        let text = if let Some(unit) = unit {
            text.trim().trim_end_matches(unit.as_ref()).trim()
        } else {
            text.trim()
        };

        text.parse::<f64>().ok().map(|v| v.clamp(min, max))
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
        let current_value = self.value;
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let decimals = self.decimals;
        let unit_clone = self.unit.clone();

        // Get or create focus handle for this element
        let focus_handle = NUMBER_INPUT_FOCUS_HANDLES.with(|handles| {
            let mut handles = handles.borrow_mut();
            handles
                .entry(self.id.clone())
                .or_insert_with(|| cx.focus_handle())
                .clone()
        });

        // Get or create edit state for this element
        let edit_state = NUMBER_INPUT_EDIT_STATES.with(|states| {
            let mut states = states.borrow_mut();
            states
                .entry(self.id.clone())
                .or_insert_with(|| Rc::new(RefCell::new(NumberEditState::default())))
                .clone()
        });

        // Read current edit state
        let state = edit_state.borrow();
        let editing = state.editing;
        let text_selected = state.text_selected;
        let edit_text = if editing {
            state.text.clone()
        } else {
            Self::format_value_str(current_value, decimals, unit_clone.as_ref())
        };
        let cursor_pos = state.cursor;
        drop(state);

        // Create unique child IDs based on parent ID
        let parent_id = format!("{:?}", self.id);
        let dec_id = ElementId::Name(format!("{}-dec", parent_id).into());
        let value_id = ElementId::Name(format!("{}-value", parent_id).into());
        let inc_id = ElementId::Name(format!("{}-inc", parent_id).into());

        // Wrap handler in Rc for sharing
        let on_change_rc = self.on_change.map(Rc::new);

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
                dec_button = dec_button.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let new_value = (current_value - step).clamp(min, max);
                    handler(new_value, window, cx);
                });
            }
        } else {
            dec_button = dec_button.cursor_not_allowed();
        }

        input_row = input_row.child(dec_button);

        // Value display / edit field
        // Visual selection highlight: when text_selected is true, show accent background
        let (value_bg, value_text_color) = if editing && text_selected {
            (Some(theme.button_active), rgba(0xffffffff))
        } else {
            (None, text_color)
        };

        // Build display with cursor if editing and not all selected
        let display_element: AnyElement = if editing && !text_selected {
            // Show text with cursor
            let chars: Vec<char> = edit_text.chars().collect();
            let before: String = chars[..cursor_pos].iter().collect();
            let after: String = chars[cursor_pos..].iter().collect();

            div()
                .flex()
                .items_center()
                .child(before)
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(self.size.font_size() + 2.0))
                        .bg(text_color),
                )
                .child(after)
                .into_any_element()
        } else {
            div().child(edit_text.clone()).into_any_element()
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
            .track_focus(&focus_handle)
            .focusable()
            .child(display_element);

        // Apply selection background if selected
        if let Some(bg) = value_bg {
            value_field = value_field.bg(bg);
        }

        // Apply font size
        value_field = value_field.text_size(px(self.size.font_size()));

        if !disabled {
            // Click to start editing / focus
            let edit_state_for_click = edit_state.clone();
            let focus_handle_for_click = focus_handle.clone();
            let formatted_value =
                Self::format_value_str(current_value, decimals, unit_clone.as_ref());

            value_field = value_field.cursor_text().on_mouse_down(
                MouseButton::Left,
                move |event, window, cx| {
                    // Focus the input
                    window.focus(&focus_handle_for_click, cx);

                    let mut state = edit_state_for_click.borrow_mut();

                    // Double-click: select all
                    if event.click_count == 2 {
                        if state.editing {
                            state.select_all();
                        } else {
                            *state = NumberEditState::new(&formatted_value);
                        }
                        drop(state);
                        window.refresh();
                        return;
                    }

                    // Single click: start editing if not already
                    if !state.editing {
                        *state = NumberEditState::new(&formatted_value);
                    } else {
                        // Clear selection on single click while editing
                        state.text_selected = false;
                    }
                },
            );

            // Keyboard handling
            let edit_state_for_key = edit_state.clone();
            let on_change_key = on_change_rc.clone();
            let unit_for_key = unit_clone.clone();

            value_field = value_field.on_key_down(move |event, window, cx| {
                let mut state = edit_state_for_key.borrow_mut();

                if state.editing {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            // Confirm edit - parse and call on_change
                            let parsed =
                                Self::parse_value_str(&state.text, unit_for_key.as_ref(), min, max);
                            state.editing = false;
                            state.text.clear();
                            state.text_selected = false;
                            drop(state);

                            if let Some(ref handler) = on_change_key
                                && let Some(value) = parsed
                            {
                                handler(value, window, cx);
                            }
                            window.refresh();
                        }
                        "escape" => {
                            // Cancel edit - restore original value
                            state.editing = false;
                            state.text.clear();
                            state.text_selected = false;
                            drop(state);
                            window.refresh();
                        }
                        "backspace" => {
                            state.do_backspace();
                            drop(state);
                            window.refresh();
                        }
                        "delete" => {
                            state.do_delete();
                            drop(state);
                            window.refresh();
                        }
                        "left" => {
                            state.move_left();
                            drop(state);
                            window.refresh();
                        }
                        "right" => {
                            state.move_right();
                            drop(state);
                            window.refresh();
                        }
                        "home" => {
                            state.move_to_start();
                            drop(state);
                            window.refresh();
                        }
                        "end" => {
                            state.move_to_end();
                            drop(state);
                            window.refresh();
                        }
                        key if key.len() == 1 => {
                            // Single character input
                            if let Some(ch) = key.chars().next() {
                                state.insert_char(ch);
                                drop(state);
                                window.refresh();
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Non-editing mode - arrow keys adjust value
                    let new_value = match event.keystroke.key.as_str() {
                        "up" | "right" => Some((current_value + step).clamp(min, max)),
                        "down" | "left" => Some((current_value - step).clamp(min, max)),
                        _ => None,
                    };
                    drop(state);

                    if let Some(v) = new_value
                        && let Some(ref handler) = on_change_key
                    {
                        handler(v, window, cx);
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
                inc_button = inc_button.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let new_value = (current_value + step).clamp(min, max);
                    handler(new_value, window, cx);
                });
            }
        } else {
            inc_button = inc_button.cursor_not_allowed();
        }

        input_row = input_row.child(inc_button);

        // Scroll wheel on the whole input
        if !disabled && let Some(ref handler_rc) = on_change_rc {
            let handler_scroll = handler_rc.clone();
            input_row = input_row.on_scroll_wheel(move |event, window, cx| {
                let delta = event.delta.pixel_delta(px(20.0)).y;
                let scroll_up = delta < px(0.0);
                let change = if scroll_up { step } else { -step };
                let new_value = (current_value + change).clamp(min, max);
                handler_scroll(new_value, window, cx);
            });
        }

        container.child(input_row)
    }
}
