//! Reusable circular potentiometer component with fill indicator

use gpui::prelude::*;
use gpui::*;

/// Render a circular potentiometer knob with value display
///
/// # Parameters
/// - `value`: Current value (0.0 to 1.0)
/// - `label`: Optional text to display (e.g., volume percentage)
/// - `size`: Diameter in pixels
/// - `muted`: Whether the control is muted/disabled
/// - `accent_color`: Color for the filled portion
/// - `muted_color`: Color when muted
/// - `bg_color`: Background circle color
/// - `text_color`: Text color in center
pub fn render_potentiometer(
    value: f32,
    label: String,
    size: f32,
    muted: bool,
    accent_color: gpui::Hsla,
    muted_color: gpui::Hsla,
    bg_color: gpui::Hsla,
    text_color: gpui::Hsla,
) -> impl IntoElement {
    let display_value = if muted { 0.0 } else { value.clamp(0.0, 1.0) };

    let ring_color = if muted { muted_color } else { accent_color };
    let text_color_final = if muted { muted_color } else { text_color };

    // Make fill color slightly lighter than the background
    let fill_color = if muted {
        muted_color
    } else {
        // Lighten the background color by increasing lightness
        let mut lighter = bg_color;
        lighter.l = (lighter.l + 0.15).min(1.0);
        lighter
    };

    // Calculate the vertical offset for the fill circle
    // At 0%, the circle is completely below the visible area
    // At 100%, the circle is fully visible
    let fill_offset = size * (1.0 - display_value);

    div()
        .relative()
        .w(px(size))
        .h(px(size))
        .cursor_pointer()
        // Background circle
        .child(div().absolute().inset_0().rounded_full().bg(bg_color))
        // Filled portion (circular, slides up from bottom)
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(-fill_offset))
                        .w(px(size))
                        .h(px(size))
                        .rounded_full()
                        .bg(fill_color),
                ),
        )
        // Border ring
        .child(
            div()
                .absolute()
                .inset(px(2.0))
                .rounded_full()
                .border_2()
                .border_color(ring_color.opacity(0.3)),
        )
        // Label text in center
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color_final)
                .child(label),
        )
}
