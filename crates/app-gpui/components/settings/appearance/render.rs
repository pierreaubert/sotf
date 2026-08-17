use super::clamp::clamp_hour;
use super::clamp::clamp_minute;
use super::theme::theme_appearance_from_window;
use super::types::ScheduleBoundary;
use super::types::set_schedule_boundary;
use crate::components::design::Ds;
use crate::theme::ThemeAccentPreference;
use gpui::prelude::*;
use gpui::*;
use gpui_themes::TimeOfDay;
use gpui_ui_kit::{NumberInput, NumberInputSize};

pub(super) fn render_settings_heading(
    d: Ds,
    theme: crate::theme::Theme,
    label: impl Into<SharedString>,
) -> impl IntoElement {
    div()
        .text_size(d.text_base)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_primary)
        .child(label.into())
}

pub(super) fn render_schedule_time_row(
    d: Ds,
    theme: crate::theme::Theme,
    state_entity: Entity<crate::app::AppState>,
    id_prefix: &'static str,
    label: &'static str,
    boundary: ScheduleBoundary,
    time: TimeOfDay,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.gap)
        .child(
            div()
                .min_w(rems(6.5))
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(label),
        )
        .child({
            let state_entity = state_entity.clone();
            NumberInput::new(SharedString::from(format!("{id_prefix}-hour")))
                .value(time.hour as f64)
                .range(0.0, 23.0)
                .step(1.0)
                .decimals(0)
                .unit("h")
                .size(NumberInputSize::Sm)
                .width(76.0)
                .aria_label(format!("{label} hour"))
                .on_change(move |value, window, cx| {
                    let system_appearance = theme_appearance_from_window(window);
                    state_entity.update(cx, |state, _cx| {
                        let schedule = state.app.theme_schedule();
                        let current = match boundary {
                            ScheduleBoundary::LightStart => schedule.light_start,
                            ScheduleBoundary::DarkStart => schedule.dark_start,
                        };
                        let next = TimeOfDay::new(clamp_hour(value), current.minute);
                        state.app.set_theme_schedule_with_system(
                            set_schedule_boundary(schedule, boundary, next),
                            system_appearance,
                        );
                    });
                })
        })
        .child({
            let state_entity = state_entity.clone();
            NumberInput::new(SharedString::from(format!("{id_prefix}-minute")))
                .value(time.minute as f64)
                .range(0.0, 59.0)
                .step(15.0)
                .decimals(0)
                .unit("min")
                .size(NumberInputSize::Sm)
                .width(92.0)
                .aria_label(format!("{label} minute"))
                .on_change(move |value, window, cx| {
                    let system_appearance = theme_appearance_from_window(window);
                    state_entity.update(cx, |state, _cx| {
                        let schedule = state.app.theme_schedule();
                        let current = match boundary {
                            ScheduleBoundary::LightStart => schedule.light_start,
                            ScheduleBoundary::DarkStart => schedule.dark_start,
                        };
                        let next = TimeOfDay::new(current.hour, clamp_minute(value));
                        state.app.set_theme_schedule_with_system(
                            set_schedule_boundary(schedule, boundary, next),
                            system_appearance,
                        );
                    });
                })
        })
}

pub(super) fn render_accent_swatch(
    d: Ds,
    theme: crate::theme::Theme,
    state_entity: Entity<crate::app::AppState>,
    preference: ThemeAccentPreference,
    selected: bool,
    fallback_accent: Rgba,
) -> impl IntoElement {
    let preview_color = preference.preview_color(fallback_accent);
    let keyboard_state_entity = state_entity.clone();
    let focus_background = theme.surface_hover;
    let focus_border = theme.border_focused;
    let preference_name = preference.name();
    div()
        .id(SharedString::from(format!(
            "theme-accent-{}",
            preference.value()
        )))
        .flex()
        .items_center()
        .gap(d.gap)
        .px(d.pad_x)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .border_1()
        .border_color(if selected { theme.accent } else { theme.border })
        .bg(if selected {
            theme.surface_selected
        } else {
            theme.surface
        })
        .cursor_pointer()
        .focusable()
        .focus_visible(move |style| style.border_color(focus_border).bg(focus_background))
        .aria_label(format!("Accent {preference_name}"))
        .child(
            div()
                .w(rems(1.0))
                .h(rems(1.0))
                .rounded_full()
                .border_1()
                .border_color(theme.border)
                .bg(preview_color),
        )
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(if selected {
                    theme.text_primary
                } else {
                    theme.text_secondary
                })
                .child(preference.name()),
        )
        .on_click(move |_, _, cx| {
            state_entity.update(cx, |state, _cx| {
                state.app.set_theme_accent_preference(preference);
            });
        })
        .on_key_down(move |event, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                keyboard_state_entity.update(cx, |state, _cx| {
                    state.app.set_theme_accent_preference(preference);
                });
                cx.stop_propagation();
            }
        })
}
