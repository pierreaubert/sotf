//! Vertical Slider component for audio plugin parameters
//!
//! A vertical slider with:
//! - Selection highlighting for plugin parameter editing
//! - Drag support with vertical mouse movement
//! - Scroll wheel adjustment (Shift for fine control)
//! - Double-click to reset to default
//! - Keyboard navigation when selected:
//!   - Arrow Up/Right: increase value by 5%
//!   - Arrow Down/Left: decrease value by 5%
//!   - Home: set to minimum
//!   - End: set to maximum
//!   - Escape: reset to default
//! - Value display with units
//! - Keyboard shortcut hints
//! - Linear or logarithmic scale

use crate::scale::Scale;
use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Scale type for vertical slider value mapping
/// Re-exported from scale module for API consistency
pub type VerticalSliderScale = Scale;

/// Theme colors for vertical slider styling
#[derive(Debug, Clone)]
pub struct VerticalSliderTheme {
    /// Background color of the slider container
    pub surface: Rgba,
    /// Surface hover color
    pub surface_hover: Rgba,
    /// Track background color
    pub track_bg: Rgba,
    /// Fill color (accent)
    pub accent: Rgba,
    /// Accent muted (for selection background)
    pub accent_muted: Rgba,
    /// Border color
    pub border: Rgba,
    /// Label text color
    pub text_secondary: Rgba,
    /// Value text color
    pub text_primary: Rgba,
    /// Muted text color (for scale markers)
    pub text_muted: Rgba,
    /// Text on accent background
    pub text_on_accent: Rgba,
    /// Background secondary (for value badge)
    pub background_secondary: Rgba,
}

impl Default for VerticalSliderTheme {
    fn default() -> Self {
        Self {
            surface: rgba(0x2a2a2aff),
            surface_hover: rgba(0x3a3a3aff),
            track_bg: rgba(0x1a1a1aff),
            accent: rgba(0x007accff),
            accent_muted: rgba(0x007acc33),
            border: rgba(0x3a3a3aff),
            text_secondary: rgba(0xaaaaaaff),
            text_primary: rgba(0xffffffff),
            text_muted: rgba(0x888888ff),
            text_on_accent: rgba(0xffffffff),
            background_secondary: rgba(0x2a2a2aff),
        }
    }
}

impl From<&Theme> for VerticalSliderTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            surface: theme.surface,
            surface_hover: theme.surface_hover,
            track_bg: theme.muted,
            accent: theme.accent,
            accent_muted: Rgba {
                r: theme.accent.r,
                g: theme.accent.g,
                b: theme.accent.b,
                a: 0.2,
            },
            border: theme.border,
            text_secondary: theme.text_secondary,
            text_primary: theme.text_primary,
            text_muted: theme.text_muted,
            text_on_accent: theme.text_primary,
            background_secondary: theme.surface,
        }
    }
}

/// Vertical slider size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalSliderSize {
    /// Compact size
    Sm,
    /// Default size
    #[default]
    Md,
    /// Large size
    Lg,
}

impl VerticalSliderSize {
    fn track_width(&self) -> f32 {
        match self {
            Self::Sm => 14.0,
            Self::Md => 18.0,
            Self::Lg => 24.0,
        }
    }

    fn track_height(&self) -> f32 {
        match self {
            Self::Sm => 80.0,
            Self::Md => 120.0,
            Self::Lg => 160.0,
        }
    }

    fn min_width(&self) -> f32 {
        match self {
            Self::Sm => 50.0,
            Self::Md => 70.0,
            Self::Lg => 90.0,
        }
    }
}

/// Information about a tick mark
#[derive(Debug, Clone)]
struct TickMark {
    /// The actual value at this tick (stored for potential debugging/future use)
    #[allow(dead_code)]
    value: f64,
    /// Normalized position (0.0 = bottom/min, 1.0 = top/max)
    normalized_pos: f64,
    /// Whether this is a major tick (gets a label)
    is_major: bool,
    /// Optional label text
    label: Option<String>,
}

/// Format a value with abbreviated suffix (1k, 10k, etc.)
fn format_value_abbrev(value: f64) -> String {
    let abs_value = value.abs();
    let sign = if value < 0.0 { "-" } else { "" };

    if abs_value >= 10000.0 {
        // 10000 -> 10k, 20000 -> 20k
        format!("{}{}k", sign, (abs_value / 1000.0).round() as i32)
    } else if abs_value >= 1000.0 {
        // 1000 -> 1k, 2500 -> 2.5k
        let k_value = abs_value / 1000.0;
        if (k_value.round() - k_value).abs() < 0.01 {
            format!("{}{}k", sign, k_value.round() as i32)
        } else {
            format!("{}{:.1}k", sign, k_value)
        }
    } else if abs_value >= 10.0 {
        // For values >= 10, show as integer
        format!("{}{}", sign, abs_value.round() as i32)
    } else if abs_value >= 1.0 {
        // Show one decimal if needed
        if (abs_value.round() - abs_value).abs() < 0.01 {
            format!("{}{}", sign, abs_value.round() as i32)
        } else {
            format!("{}{:.1}", sign, abs_value)
        }
    } else if abs_value >= 0.1 {
        format!("{}{:.1}", sign, abs_value)
    } else if abs_value > 0.0 {
        format!("{}{:.2}", sign, abs_value)
    } else {
        "0".to_string()
    }
}

/// Find a nice step size for linear scale
fn find_nice_step(range: f64, target_ticks: usize) -> f64 {
    if range <= 0.0 || target_ticks < 2 {
        return range;
    }

    let rough_step = range / (target_ticks - 1) as f64;
    let magnitude = 10_f64.powf(rough_step.log10().floor());

    // Try nice multiples: 1, 2, 2.5, 5, 10
    let normalized = rough_step / magnitude;
    let nice_normalized = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 2.5 {
        2.5
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };

    nice_normalized * magnitude
}

/// Calculate tick marks for linear scale
fn calculate_linear_ticks(min: f64, max: f64, track_height: f32) -> Vec<TickMark> {
    let range = max - min;
    if range <= 0.0 {
        return vec![
            TickMark {
                value: min,
                normalized_pos: 0.0,
                is_major: true,
                label: Some(format_value_abbrev(min)),
            },
            TickMark {
                value: max,
                normalized_pos: 1.0,
                is_major: true,
                label: Some(format_value_abbrev(max)),
            },
        ];
    }

    // Determine target labeled tick count based on height
    // Minimum 2 labels (min/max), up to 6 for very tall sliders
    let target_labels = ((track_height / 40.0) as usize).clamp(2, 6);

    // Find nice step for labels
    let label_step = find_nice_step(range, target_labels);

    // Minor ticks: more frequent, about twice as many
    let minor_step = label_step / 2.0;

    let mut ticks = Vec::new();

    // Always add min as major tick with label
    ticks.push(TickMark {
        value: min,
        normalized_pos: 0.0,
        is_major: true,
        label: Some(format_value_abbrev(min)),
    });

    // Add intermediate ticks
    let first_label_tick = (min / label_step).ceil() * label_step;
    let first_minor_tick = (min / minor_step).ceil() * minor_step;

    // Collect all tick positions
    let mut tick_value = first_minor_tick;
    while tick_value < max - minor_step * 0.1 {
        if (tick_value - min).abs() > minor_step * 0.1 {
            let normalized = (tick_value - min) / range;

            // Check if this is a label tick (on label_step boundary)
            let is_label_tick = ((tick_value - first_label_tick) / label_step).round().abs()
                * label_step
                + first_label_tick;
            let is_labeled = (tick_value - is_label_tick).abs() < label_step * 0.01;

            ticks.push(TickMark {
                value: tick_value,
                normalized_pos: normalized,
                is_major: is_labeled,
                label: if is_labeled {
                    Some(format_value_abbrev(tick_value))
                } else {
                    None
                },
            });
        }
        tick_value += minor_step;
    }

    // Always add max as major tick with label
    ticks.push(TickMark {
        value: max,
        normalized_pos: 1.0,
        is_major: true,
        label: Some(format_value_abbrev(max)),
    });

    ticks
}

/// Calculate tick marks for logarithmic scale
fn calculate_log_ticks(min: f64, max: f64, track_height: f32) -> Vec<TickMark> {
    let min = min.max(1e-10);
    let max = max.max(min + 1e-10);

    let log_min = min.ln();
    let log_max = max.ln();
    let log_range = log_max - log_min;

    if log_range <= 0.0 {
        return vec![
            TickMark {
                value: min,
                normalized_pos: 0.0,
                is_major: true,
                label: Some(format_value_abbrev(min)),
            },
            TickMark {
                value: max,
                normalized_pos: 1.0,
                is_major: true,
                label: Some(format_value_abbrev(max)),
            },
        ];
    }

    let mut ticks = Vec::new();

    // Always add min as major tick with label
    ticks.push(TickMark {
        value: min,
        normalized_pos: 0.0,
        is_major: true,
        label: Some(format_value_abbrev(min)),
    });

    // Calculate decade range
    let min_decade = min.log10().floor() as i32;
    let max_decade = max.log10().ceil() as i32;
    let num_decades = (max_decade - min_decade) as usize;

    // Determine how many labels we can fit based on height
    // About one label per 35-40 pixels, minimum 2
    let max_labels = ((track_height / 35.0) as usize).clamp(2, 8);

    // Decide which decade markers get labels
    // If few decades, label all of them; otherwise label every Nth
    let label_every_n = if num_decades <= max_labels {
        1
    } else {
        (num_decades / max_labels).max(1)
    };

    // Determine detail level based on height
    let include_sub_decades = track_height >= 80.0;

    // Add decade markers and sub-decade markers
    let mut decade_index = 0;
    for decade in min_decade..=max_decade {
        let decade_value = 10_f64.powi(decade);

        // Main decade marker (1, 10, 100, 1k, 10k, etc.)
        if decade_value > min * 1.05 && decade_value < max * 0.95 {
            let normalized = (decade_value.ln() - log_min) / log_range;
            let should_label = decade_index % label_every_n == 0;
            ticks.push(TickMark {
                value: decade_value,
                normalized_pos: normalized,
                is_major: should_label,
                label: if should_label {
                    Some(format_value_abbrev(decade_value))
                } else {
                    None
                },
            });
            decade_index += 1;
        }

        // Sub-decade markers (2, 5) if we have enough space
        if include_sub_decades {
            for multiplier in [2.0, 5.0] {
                let sub_value = decade_value * multiplier;
                if sub_value > min * 1.05 && sub_value < max * 0.95 {
                    let normalized = (sub_value.ln() - log_min) / log_range;
                    ticks.push(TickMark {
                        value: sub_value,
                        normalized_pos: normalized,
                        is_major: false,
                        label: None,
                    });
                }
            }
        }
    }

    // Always add max as major tick with label
    ticks.push(TickMark {
        value: max,
        normalized_pos: 1.0,
        is_major: true,
        label: Some(format_value_abbrev(max)),
    });

    // Sort by normalized position
    ticks.sort_by(|a, b| a.normalized_pos.partial_cmp(&b.normalized_pos).unwrap());

    ticks
}

/// Calculate tick marks based on scale type
fn calculate_ticks(min: f64, max: f64, scale: Scale, track_height: f32) -> Vec<TickMark> {
    match scale {
        Scale::Linear => calculate_linear_ticks(min, max, track_height),
        Scale::Logarithmic => calculate_log_ticks(min, max, track_height),
    }
}

/// A vertical slider component for audio plugin parameters
#[derive(IntoElement)]
pub struct VerticalSlider {
    id: ElementId,
    value: f64,
    min: f64,
    max: f64,
    unit: SharedString,
    label: Option<SharedString>,
    shortcut_key: Option<char>,
    size: VerticalSliderSize,
    scale: Scale,
    custom_height: Option<f32>,
    show_ticks: bool,
    selected: bool,
    disabled: bool,
    theme: Option<VerticalSliderTheme>,
    on_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_drag_start: Option<Box<dyn Fn(f32, f64, &mut Window, &mut App) + 'static>>,
    on_select: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_reset: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl VerticalSlider {
    /// Create a new vertical slider with the given ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            unit: "".into(),
            label: None,
            shortcut_key: None,
            size: VerticalSliderSize::default(),
            scale: Scale::default(),
            custom_height: None,
            show_ticks: false,
            selected: false,
            disabled: false,
            theme: None,
            on_change: None,
            on_drag_start: None,
            on_select: None,
            on_reset: None,
        }
    }

    /// Convert a value to normalized position [0, 1] based on scale type
    fn value_to_normalized(&self, value: f64) -> f64 {
        self.scale.value_to_normalized(value, self.min, self.max)
    }

    /// Set the current value
    /// Note: The value is stored as-is and clamped at render time
    /// after min/max are known
    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
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

    /// Set the unit label (e.g., "dB", "Hz", "%", ":1")
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Set the display label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the keyboard shortcut key for the label
    pub fn shortcut_key(mut self, key: char) -> Self {
        self.shortcut_key = Some(key);
        self
    }

    /// Set the slider size
    pub fn size(mut self, size: VerticalSliderSize) -> Self {
        self.size = size;
        self
    }

    /// Set the value scale type (linear or logarithmic)
    ///
    /// Use `Logarithmic` for frequency parameters (e.g., 20Hz to 20kHz)
    /// where equal visual distances should represent equal ratios.
    ///
    /// Note: For logarithmic scale, min must be > 0.
    pub fn scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    /// Set a custom track height in pixels (overrides size preset)
    pub fn height(mut self, height: f32) -> Self {
        self.custom_height = Some(height);
        self
    }

    /// Enable tick marks along the track
    pub fn with_ticks(mut self) -> Self {
        self.show_ticks = true;
        self
    }

    /// Set selected state (for plugin parameter editing)
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set theme colors
    pub fn theme(mut self, theme: VerticalSliderTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set value change handler (called on scroll wheel)
    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set drag start handler (called on mouse down with y position and current value)
    pub fn on_drag_start(
        mut self,
        handler: impl Fn(f32, f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drag_start = Some(Box::new(handler));
        self
    }

    /// Set select handler (called on click to select this parameter)
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Set reset handler (called on double-click)
    pub fn on_reset(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(Box::new(handler));
        self
    }

    /// Format the label with keyboard shortcut indicator
    fn format_label(&self) -> String {
        let label = self
            .label
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        match self.shortcut_key {
            Some(key) => {
                let key_lower = key.to_ascii_lowercase();
                let label_lower = label.to_lowercase();
                if let Some(pos) = label_lower.find(key_lower) {
                    format!(
                        "{}[{}]{}",
                        &label[..pos],
                        label.chars().nth(pos).unwrap().to_ascii_uppercase(),
                        &label[pos + 1..]
                    )
                } else {
                    format!("[{}] {}", key.to_ascii_uppercase(), label)
                }
            }
            None => label,
        }
    }

    /// Format the value display
    fn format_value(&self) -> String {
        let unit = self.unit.to_string();
        if unit == ":1" {
            format!("{:.1}{}", self.value, unit)
        } else if unit == "%" {
            format!("{:.0}{}", self.value * 100.0, unit)
        } else if unit.is_empty() {
            format!("{:.1}", self.value)
        } else {
            format!("{:.1} {}", self.value, unit)
        }
    }
}

impl RenderOnce for VerticalSlider {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Clamp value to min/max range now that both are set
        self.value = self.value.clamp(self.min, self.max);

        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| VerticalSliderTheme::from(&global_theme));
        let selected = self.selected;
        let disabled = self.disabled;

        // Use scale-aware normalization for slider position
        let normalized = self.value_to_normalized(self.value) as f32;

        let formatted_label = self.format_label();
        let value_str = self.format_value();

        let track_width = self.size.track_width();
        let track_height = self
            .custom_height
            .unwrap_or_else(|| self.size.track_height());
        let min_width = self.size.min_width();
        let show_ticks = self.show_ticks;

        // Calculate ticks based on scale type and available height
        let ticks = calculate_ticks(self.min, self.max, self.scale, track_height);

        // Colors based on selection state
        let bg_color = if selected {
            theme.accent_muted
        } else {
            theme.surface
        };
        let border_color = if selected { theme.accent } else { theme.border };
        let track_border = if selected { theme.accent } else { theme.border };
        let label_color = if selected {
            theme.accent
        } else {
            theme.text_secondary
        };
        let value_bg = if selected {
            theme.accent
        } else {
            theme.background_secondary
        };
        let value_color = if selected {
            theme.text_on_accent
        } else {
            theme.text_primary
        };
        let track_bg = if selected {
            theme.surface_hover
        } else {
            theme.track_bg
        };
        let thumb_color = if selected {
            theme.text_on_accent
        } else {
            theme.accent
        };
        let thumb_height = if selected { 6.0 } else { 4.0 };
        let scale_color = if selected {
            theme.text_secondary
        } else {
            theme.text_muted
        };

        // Capture values for closures
        let value = self.value;
        let min = self.min;
        let max = self.max;
        let scale = self.scale;

        let mut container = div()
            .id(self.id)
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .p_2()
            .rounded_lg()
            .bg(bg_color)
            .border_2()
            .border_color(border_color)
            .min_w(px(min_width));

        // Add shadow when selected
        if selected {
            container = container.shadow_md();
        }

        // Hover effect
        let hover_border = theme.accent;
        let hover_bg = theme.surface_hover;
        container = container.hover(|s| s.border_color(hover_border).bg(hover_bg));

        // Cursor
        if disabled {
            container = container.cursor_not_allowed().opacity(0.5);
        } else {
            container = container.cursor_ns_resize();
        }

        // Event handlers
        if !disabled {
            // Wrap on_change in Rc for sharing between handlers
            let on_change_rc = self.on_change.map(|h| std::rc::Rc::new(h));
            let on_reset_rc = self.on_reset.map(|h| std::rc::Rc::new(h));

            // Mouse down - start drag and select
            if let Some(on_select) = self.on_select {
                let on_drag_start = self.on_drag_start;
                container = container.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    on_select(window, cx);
                    if let Some(ref handler) = on_drag_start {
                        handler(event.position.y.into(), value, window, cx);
                    }
                });
            } else if let Some(on_drag_start) = self.on_drag_start {
                container = container.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    on_drag_start(event.position.y.into(), value, window, cx);
                });
            } else if let Some(ref handler_rc) = on_change_rc {
                // If no drag handler, use click to step value (scale-aware)
                let handler_click = handler_rc.clone();
                container =
                    container.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        // Use scale-aware stepping (10% in normalized space)
                        let new_value = scale.step_value(value, min, max, 1.0, 0.1);
                        handler_click(new_value, window, cx);
                    });
            }

            // Double-click - reset
            if let Some(ref reset_rc) = on_reset_rc {
                let reset_handler = reset_rc.clone();
                container = container.on_click(move |event, window, cx| {
                    if event.click_count() == 2 {
                        reset_handler(window, cx);
                    }
                });
            }

            // Scroll wheel - adjust value (Shift for fine control)
            if let Some(ref handler_rc) = on_change_rc {
                let handler_scroll = handler_rc.clone();
                container = container.on_scroll_wheel(move |event, window, cx| {
                    // Get scroll delta (works for both pixel and line deltas)
                    // On macOS, shift+scroll converts vertical to horizontal, so check both axes
                    let (delta_x, delta_y): (f32, f32) = match event.delta {
                        gpui::ScrollDelta::Pixels(point) => (point.x.into(), point.y.into()),
                        gpui::ScrollDelta::Lines(point) => (point.x, point.y),
                    };

                    // Use Y delta primarily, but fall back to X delta (for shift+scroll on macOS)
                    let delta = if delta_y.abs() > 0.0001 {
                        delta_y
                    } else if delta_x.abs() > 0.0001 {
                        delta_x
                    } else {
                        return; // No meaningful scroll movement
                    };

                    // Scroll up/left (negative delta) = increase value
                    // Scroll down/right (positive delta) = decrease value
                    let direction = if delta < 0.0 { 1.0 } else { -1.0 };

                    // Check for shift key for fine-grained control
                    let step_size = if event.modifiers.shift { 0.005 } else { 0.05 };

                    // Use scale-aware stepping
                    let new_value = scale.step_value(value, min, max, direction, step_size);
                    handler_scroll(new_value, window, cx);
                });
            }

            // Keyboard navigation when selected
            if selected
                && let Some(ref handler_rc) = on_change_rc {
                    let handler_key = handler_rc.clone();
                    let reset_key = on_reset_rc.clone();
                    container = container.on_key_down(move |event, window, cx| {
                        match event.keystroke.key.as_str() {
                            // Arrow Up or Right - increase value (scale-aware)
                            "up" | "right" => {
                                let new_value = scale.step_value(value, min, max, 1.0, 0.05);
                                handler_key(new_value, window, cx);
                            }
                            // Arrow Down or Left - decrease value (scale-aware)
                            "down" | "left" => {
                                let new_value = scale.step_value(value, min, max, -1.0, 0.05);
                                handler_key(new_value, window, cx);
                            }
                            // Home - set to minimum
                            "home" => {
                                handler_key(min, window, cx);
                            }
                            // End - set to maximum
                            "end" => {
                                handler_key(max, window, cx);
                            }
                            // Escape - reset to default
                            "escape" => {
                                if let Some(ref reset_handler) = reset_key {
                                    reset_handler(window, cx);
                                }
                            }
                            _ => {}
                        }
                    });
                }
        }

        // Label with keyboard shortcut
        container = container.child(
            div()
                .text_xs()
                .font_weight(if selected {
                    FontWeight::BOLD
                } else {
                    FontWeight::SEMIBOLD
                })
                .text_color(label_color)
                .text_center()
                .child(formatted_label),
        );

        // Value badge
        container = container.child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(value_bg)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(value_color)
                .child(value_str),
        );

        // Track with fill and thumb
        let mut track = div()
            .w(px(track_width))
            .h(px(track_height))
            .bg(track_bg)
            .rounded_lg()
            .border_2()
            .border_color(track_border)
            .relative()
            .overflow_hidden();

        if selected {
            track = track.shadow_sm();
        }

        // Filled portion (from bottom)
        track = track.child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(relative(normalized))
                .bg(theme.accent)
                .rounded_b_md(),
        );

        // Thumb indicator
        track = track.child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(relative(normalized))
                .h(px(thumb_height))
                .bg(thumb_color)
                .rounded_sm()
                .when(selected, |d| d.shadow_sm()),
        );

        // Track with optional tick marks
        if show_ticks {
            // Calculate label width for alignment (find widest label)
            let label_width = ticks
                .iter()
                .filter_map(|t| t.label.as_ref())
                .map(|l| l.len())
                .max()
                .unwrap_or(2) as f32
                * 7.0; // Approximate character width

            let tick_mark_width = 6.0_f32; // Major tick width
            let label_tick_gap = 3.0_f32; // Gap between label and tick
            let label_height = 12.0_f32; // Approximate label height for centering

            // Build tick marks container with absolute positioning
            // Height matches track exactly for proper alignment
            let mut ticks_container = div()
                .relative()
                .h(px(track_height))
                .w(px(label_width + label_tick_gap + tick_mark_width));

            for tick in &ticks {
                let pos = tick.normalized_pos as f32;
                let tick_width = if tick.is_major { 6.0 } else { 3.0 };

                // Calculate pixel position from top (inverted: 0=bottom, 1=top)
                // pos=0 should be at bottom (top = track_height - label_height/2)
                // pos=1 should be at top (top = -label_height/2)
                let top_pos = (1.0 - pos) * track_height - label_height / 2.0;

                // Create a tick row positioned from top, centered vertically
                let tick_element = div()
                    .absolute()
                    .top(px(top_pos))
                    .right_0()
                    .h(px(label_height))
                    .flex()
                    .items_center()
                    .gap(px(label_tick_gap))
                    // Add label for major ticks
                    .when(tick.label.is_some(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(scale_color)
                                .min_w(px(label_width))
                                .text_right()
                                .child(tick.label.clone().unwrap_or_default()),
                        )
                    })
                    // Tick mark
                    .child(div().w(px(tick_width)).h(px(1.0)).bg(scale_color));

                ticks_container = ticks_container.child(tick_element);
            }

            // Build right-side tick marks (no labels, just tick marks)
            let mut ticks_right = div().relative().h(px(track_height)).w(px(tick_mark_width));

            for tick in &ticks {
                let pos = tick.normalized_pos as f32;
                let tick_width = if tick.is_major { 6.0 } else { 3.0 };
                let top_pos = (1.0 - pos) * track_height - label_height / 2.0;

                let tick_element = div()
                    .absolute()
                    .top(px(top_pos))
                    .left_0()
                    .h(px(label_height))
                    .flex()
                    .items_center()
                    .child(div().w(px(tick_width)).h(px(1.0)).bg(scale_color));

                ticks_right = ticks_right.child(tick_element);
            }

            // Wrap track and ticks in HStack (left ticks - track - right ticks)
            container = container.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(ticks_container)
                    .child(track)
                    .child(ticks_right),
            );
        } else {
            container = container.child(track);

            // Scale markers (only when not showing ticks) - use abbreviated format
            container = container.child(
                div()
                    .flex()
                    .justify_between()
                    .w_full()
                    .text_xs()
                    .text_color(scale_color)
                    .child(format_value_abbrev(min))
                    .child(format_value_abbrev(max)),
            );
        }

        container
    }
}
