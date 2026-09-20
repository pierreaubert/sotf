use super::accessibility::accessibility_palette_from_value;
use super::accessibility::accessibility_value;
use super::design::design_language_from_value;
use super::design::design_language_value;
use super::misc::schedule_from_preference;
use super::render::render_accent_swatch;
use super::render::render_schedule_time_row;
use super::render::render_settings_heading;
use super::types::ScheduleBoundary;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::types::DensityMode;
use crate::app::types::PreferencesSetting;
use crate::components::design::Ds;
use crate::components::settings::search::{
    focus_settings_choice_relative, settings_choice_focus_handle,
};
use crate::i18n::{AppearanceTranslations, Language};
use crate::theme::{CommunityThemeId, ThemeAccentPreference, ThemeId};
use crate::ui::PlayerView;
use crate::ui::{
    DEFAULT_MAX_FONT_SIZE_PX, DEFAULT_MIN_FONT_SIZE_PX, MIN_CONFIGURABLE_FONT_SIZE_PX,
};
use gpui::prelude::*;
use gpui::*;
use gpui_design::{DesignLanguage, DesignSystem, DesignSystemState};
use gpui_themes::{AccessibilityPalette, ThemeAppearance, ThemeModePreference, ThemeSchedule};
use gpui_ui_kit::accessibility::{
    AccessibilityExt, AccessibilityNode, AriaProps, AriaRole, AriaState,
};
use gpui_ui_kit::{
    Button, ButtonSet, ButtonSetOption, ButtonSetSize, ButtonSize, ButtonVariant, Input, InputSize,
    NumberInput, NumberInputSize, Toggle, ToggleSize, ToggleStyle,
};

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

fn theme_mode_value(preference: &ThemeModePreference) -> &'static str {
    match preference {
        ThemeModePreference::FollowSystem => "follow_system",
        ThemeModePreference::Light => "light",
        ThemeModePreference::Dark => "dark",
        ThemeModePreference::Scheduled { .. } => "scheduled",
    }
}

fn theme_mode_preference_from_value(
    value: &SharedString,
    schedule: ThemeSchedule,
) -> Option<ThemeModePreference> {
    match value.as_ref() {
        "follow_system" => Some(ThemeModePreference::FollowSystem),
        "light" => Some(ThemeModePreference::Light),
        "dark" => Some(ThemeModePreference::Dark),
        "scheduled" => Some(ThemeModePreference::Scheduled { schedule }),
        _ => None,
    }
}

pub(super) fn theme_appearance_from_window(window: &Window) -> ThemeAppearance {
    match window.appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeAppearance::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeAppearance::Light,
    }
}

impl PlayerView {
    fn select_theme_mode(
        &mut self,
        value: &str,
        schedule: ThemeSchedule,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preference) = theme_mode_preference_from_value(&value.into(), schedule) else {
            return;
        };
        self.state.update(cx, |state, _cx| {
            state.app.set_theme_mode_preference_with_system(
                preference,
                theme_appearance_from_window(window),
            );
        });
        // The selected option reuses the search focus handle on the next paint.
        if let Some(focus) = self.preference_focus_handle(PreferencesSetting::ThemeMode, cx) {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    /// Render theme settings content
    pub(crate) fn render_theme_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let text = AppearanceTranslations::for_language(state.app.ui_state.language);
        let theme_id = state.app.ui_state.theme_id;
        let design_language = state.app.ui_state.design_language.clone();
        let density_mode = state.app.ui_state.density_mode;
        let font_scale = state.app.ui_state.font_scale;
        let min_font_px = state.app.ui_state.min_font_size_px;
        let max_font_px = state.app.ui_state.max_font_size_px;
        let theme_mode_preference = state.app.ui_state.theme_mode_preference.clone();
        let schedule = schedule_from_preference(&theme_mode_preference);
        let is_scheduled = matches!(
            &theme_mode_preference,
            ThemeModePreference::Scheduled { .. }
        );
        let accessibility_palette = state.app.ui_state.accessibility_palette;
        let theme_accent_preference = state.app.ui_state.theme_accent_preference;
        let community_theme_id = state.app.ui_state.community_theme_id;
        let community_theme_json_draft = state.app.ui_state.community_theme_json_draft.clone();
        let reduce_motion = state.app.ui_state.reduce_motion;
        let theme = state.app.ui_state.theme.clone();
        let base_theme = crate::theme::Theme::from_id(theme_id);
        let translations = state.app.ui_state.translations.clone();

        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(d, theme.clone(), "Interface"))
                    .child({
                        let state_entity = self.state.clone();
                        ButtonSet::new("design-language-select")
                            .size(ButtonSetSize::Sm)
                            .options(vec![
                                ButtonSetOption::new("system", text.system),
                                ButtonSetOption::new(
                                    DesignLanguage::AppleHig.as_str(),
                                    DesignLanguage::AppleHig.label(),
                                ),
                                ButtonSetOption::new(
                                    DesignLanguage::Material3.as_str(),
                                    DesignLanguage::Material3.label(),
                                ),
                                ButtonSetOption::new(
                                    DesignLanguage::Fluent.as_str(),
                                    DesignLanguage::Fluent.label(),
                                ),
                                ButtonSetOption::new(
                                    DesignLanguage::Neutral.as_str(),
                                    DesignLanguage::Neutral.label(),
                                ),
                            ])
                            .selected(design_language_value(design_language.as_deref()))
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                if let Some(selection) = design_language_from_value(value) {
                                    let system = selection
                                        .map(DesignSystem::for_language)
                                        .unwrap_or_else(DesignSystem::platform_default);
                                    cx.set_global(DesignSystemState::with_system(system));
                                    state_entity.update(cx, |state, cx| {
                                        state.app.ui_state.design_language =
                                            selection.map(|language| language.as_str().to_string());
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                }
                            })
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(d, theme.clone(), "Density"))
                    .child({
                        let state_entity = self.state.clone();
                        ButtonSet::new("density-mode-select")
                            .size(ButtonSetSize::Sm)
                            .options(
                                DensityMode::all()
                                    .iter()
                                    .map(|mode| ButtonSetOption::new(mode.value(), mode.label()))
                                    .collect(),
                            )
                            .selected(density_mode.value())
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                if let Some(mode) = DensityMode::from_value(value.as_ref()) {
                                    state_entity.update(cx, |state, cx| {
                                        state.app.ui_state.density_mode = mode;
                                        state.app.ui_state.layout_mode = mode
                                            .layout_mode_for_window(
                                                state.app.ui_state.window_width,
                                                state.app.ui_state.window_height,
                                            );
                                        let layout = state.layout.read(cx);
                                        if let Err(error) = state.app.save_config(layout) {
                                            log::error!("Failed to save config: {error}");
                                        }
                                    });
                                }
                            })
                    })
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(text.navigation_mode_description),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(d, theme.clone(), "Typography"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(d.section)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.grid)
                                    .flex_1()
                                    .min_w(rems(14.0))
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .child(text.text_size),
                                    )
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .text_color(theme.text_secondary)
                                            .child(text.text_size_description),
                                    ),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                dev_track!(
                                    NumberInput::new("appearance-text-size")
                                        .value((font_scale * 100.0) as f64)
                                        .range(50.0, 200.0)
                                        .step(5.0)
                                        .decimals(0)
                                        .unit("%")
                                        .size(NumberInputSize::Sm)
                                        .width(120.0)
                                        .on_change(move |val, _window, cx| {
                                            let scale = ((val as f32) / 100.0).clamp(0.5, 2.0);
                                            state_entity.update(cx, |state, cx| {
                                                state.app.ui_state.font_scale = scale;
                                                crate::ui::recalculate_pagination_for_state(
                                                    state, true,
                                                );
                                                let layout = state.layout.read(cx);
                                                if let Err(error) = state.app.save_config(layout) {
                                                    log::error!("Failed to save config: {error}");
                                                }
                                            });
                                        })
                                        .map(|input| self.preference_number_input(
                                            PreferencesSetting::TextSize,
                                            input,
                                            cx
                                        )),
                                    "settings.appearance.font-scale"
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(d.section)
                            .child({
                                let state_entity = self.state.clone();
                                let current_max = max_font_px
                                    .unwrap_or(DEFAULT_MAX_FONT_SIZE_PX)
                                    .clamp(MIN_CONFIGURABLE_FONT_SIZE_PX + 1.0, 48.0);
                                NumberInput::new("appearance-min-font-size")
                                    .value(
                                        min_font_px
                                            .unwrap_or(DEFAULT_MIN_FONT_SIZE_PX)
                                            .clamp(MIN_CONFIGURABLE_FONT_SIZE_PX, current_max - 1.0)
                                            as f64,
                                    )
                                    .range(
                                        MIN_CONFIGURABLE_FONT_SIZE_PX as f64,
                                        (current_max - 1.0) as f64,
                                    )
                                    .step(1.0)
                                    .decimals(0)
                                    .unit("px min")
                                    .size(NumberInputSize::Sm)
                                    .width(128.0)
                                    .on_change(move |val, _window, cx| {
                                        let px = (val as f32).clamp(
                                            MIN_CONFIGURABLE_FONT_SIZE_PX,
                                            current_max - 1.0,
                                        );
                                        state_entity.update(cx, |state, cx| {
                                            state.app.ui_state.min_font_size_px = Some(px);
                                            crate::ui::recalculate_pagination_for_state(
                                                state, true,
                                            );
                                            let layout = state.layout.read(cx);
                                            if let Err(error) = state.app.save_config(layout) {
                                                log::error!("Failed to save config: {error}");
                                            }
                                        });
                                    })
                                    .map(|input| {
                                        self.preference_number_input(
                                            PreferencesSetting::MinimumFont,
                                            input,
                                            cx,
                                        )
                                    })
                            })
                            .child({
                                let state_entity = self.state.clone();
                                let current_min = min_font_px
                                    .unwrap_or(DEFAULT_MIN_FONT_SIZE_PX)
                                    .clamp(MIN_CONFIGURABLE_FONT_SIZE_PX, 47.0);
                                NumberInput::new("appearance-max-font-size")
                                    .value(
                                        max_font_px
                                            .unwrap_or(DEFAULT_MAX_FONT_SIZE_PX)
                                            .clamp(current_min + 1.0, 48.0)
                                            as f64,
                                    )
                                    .range((current_min + 1.0) as f64, 48.0)
                                    .step(1.0)
                                    .decimals(0)
                                    .unit("px max")
                                    .size(NumberInputSize::Sm)
                                    .width(128.0)
                                    .on_change(move |val, _window, cx| {
                                        let px = (val as f32).clamp(current_min + 1.0, 48.0);
                                        state_entity.update(cx, |state, cx| {
                                            state.app.ui_state.max_font_size_px = Some(px);
                                            crate::ui::recalculate_pagination_for_state(
                                                state, true,
                                            );
                                            let layout = state.layout.read(cx);
                                            if let Err(error) = state.app.save_config(layout) {
                                                log::error!("Failed to save config: {error}");
                                            }
                                        });
                                    })
                                    .map(|input| {
                                        self.preference_number_input(
                                            PreferencesSetting::MaximumFont,
                                            input,
                                            cx,
                                        )
                                    })
                            }),
                    )
                    .child(
                        div()
                            .p(d.card)
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .border_1()
                            .border_color(theme.border)
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .child(
                                div()
                                    .text_size(d.text_base)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(text.live_preview),
                            )
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .text_color(theme.text_secondary)
                                    .child(text.preview_description),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(
                        d,
                        theme.clone(),
                        translations.settings_mode,
                    ))
                    .child({
                        let current = theme_mode_value(&theme_mode_preference);
                        let controls = [
                            ("follow_system", text.system),
                            ("light", text.light),
                            ("dark", text.dark),
                            ("scheduled", text.scheduled),
                        ]
                        .into_iter()
                        .map(|(value, label)| {
                            let selected = value == current;
                            let id =
                                ElementId::from(SharedString::from(format!("theme-mode-{value}")));
                            let focus = if selected {
                                self.preference_focus_handle(PreferencesSetting::ThemeMode, cx)
                                    .unwrap_or_else(|| settings_choice_focus_handle(&id, cx))
                            } else {
                                settings_choice_focus_handle(&id, cx)
                            };
                            cx.register_accessible(AccessibilityNode {
                                element_id: id.clone(),
                                label: label.into(),
                                props: AriaProps::with_role(AriaRole::Button)
                                    .maybe_state(selected, AriaState::Pressed(true)),
                            });
                            let button = Button::new(id, label)
                                .size(ButtonSize::Sm)
                                .selected(selected)
                                .variant(if selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .theme(theme.to_button_theme())
                                .build()
                                .text_size(d.text_sm)
                                .px(d.pad_x)
                                .py(d.pad_y_half)
                                .track_focus(&focus)
                                .track_focus_element(&focus)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.select_theme_mode(value, schedule, window, cx);
                                }))
                                .on_key_down(cx.listener(
                                    move |view, event: &KeyDownEvent, window, cx| {
                                        if matches!(event.keystroke.key.as_str(), "space" | "enter")
                                        {
                                            view.select_theme_mode(value, schedule, window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                ));
                            let button =
                                dev_track!(button, format!("settings.appearance.mode.{value}"));
                            (button.into_any_element(), focus)
                        })
                        .collect::<Vec<_>>();
                        let handles = controls
                            .iter()
                            .map(|(_, focus)| focus.clone())
                            .collect::<Vec<_>>();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.grid)
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                if event.keystroke.key == "tab"
                                    && focus_settings_choice_relative(
                                        &handles,
                                        window,
                                        cx,
                                        event.keystroke.modifiers.shift,
                                    )
                                {
                                    cx.stop_propagation();
                                }
                            })
                            .children(controls.into_iter().map(|(button, _)| button))
                            .map(|group| {
                                self.preference_control(PreferencesSetting::ThemeMode, group, cx)
                            })
                    })
                    .when(is_scheduled, |section| {
                        section.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(d.section)
                                .child(render_schedule_time_row(
                                    d,
                                    theme.clone(),
                                    self.state.clone(),
                                    "theme-light-start",
                                    "Light starts",
                                    ScheduleBoundary::LightStart,
                                    schedule.light_start,
                                ))
                                .child(render_schedule_time_row(
                                    d,
                                    theme.clone(),
                                    self.state.clone(),
                                    "theme-dark-start",
                                    "Dark starts",
                                    ScheduleBoundary::DarkStart,
                                    schedule.dark_start,
                                )),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(d, theme.clone(), "Accent"))
                    .child({
                        let mut swatches = div().flex().flex_wrap().gap(d.gap);
                        for preference in ThemeAccentPreference::all() {
                            swatches = swatches.child(render_accent_swatch(
                                d,
                                theme.clone(),
                                self.state.clone(),
                                *preference,
                                theme_accent_preference == *preference,
                                base_theme.accent,
                            ));
                        }
                        swatches
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(render_settings_heading(d, theme.clone(), "Accessibility"))
                    .child({
                        div()
                            .id("theme-accessibility-select")
                            .flex()
                            .flex_wrap()
                            .gap(d.grid)
                            .children(AccessibilityPalette::all().iter().map(|palette| {
                                let value = SharedString::from(accessibility_value(*palette));
                                let selected = *palette == accessibility_palette;
                                dev_track!(
                                    Button::new(
                                        format!("theme-accessibility-{value}"),
                                        palette.name(),
                                    )
                                    .size(ButtonSize::Sm)
                                    .selected(selected)
                                    .variant(if selected {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(move |view, _, window, cx| {
                                            if let Some(palette) =
                                                accessibility_palette_from_value(&value)
                                            {
                                                let appearance =
                                                    theme_appearance_from_window(window);
                                                view.state.update(cx, |state, _cx| {
                                                    state
                                                        .app
                                                        .set_accessibility_palette_with_system(
                                                            palette, appearance,
                                                        );
                                                });
                                                cx.notify();
                                            }
                                        })
                                    ),
                                    format!(
                                        "settings.appearance.palette.{}",
                                        accessibility_value(*palette)
                                    )
                                )
                            }))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(d.gap)
                            .child(dev_track!(
                                Toggle::new("theme-reduce-motion")
                                    .size(ToggleSize::Sm)
                                    .checked(reduce_motion)
                                    .label(text.reduce_motion)
                                    .style(ToggleStyle::Segmented)
                                    .theme(theme.to_toggle_theme())
                                    .on_change({
                                        let view = cx.entity().downgrade();
                                        move |enabled, _window, cx| {
                                            let Some(view) = view.upgrade() else {
                                                return;
                                            };
                                            view.update(cx, |view, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.set_reduce_motion(enabled);
                                                });
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .map(|toggle| {
                                        self.preference_toggle(
                                            PreferencesSetting::ReduceMotion,
                                            toggle,
                                            cx,
                                        )
                                    }),
                                "settings.appearance.reduce-motion"
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(render_settings_heading(
                        d,
                        theme.clone(),
                        "Community themes",
                    ))
                    .child({
                        let mut container = div().flex().flex_wrap().gap(d.section);

                        for id in CommunityThemeId::all().iter() {
                            let is_selected = community_theme_id == Some(*id);
                            let preview_theme =
                                id.theme().with_accent_preference(theme_accent_preference);

                            container = container.child(self.render_community_theme_preview_card(
                                *id,
                                preview_theme,
                                is_selected,
                                theme.clone(),
                                translations.settings_active,
                                text.apply,
                                cx,
                            ));
                        }

                        container
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.gap)
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child(text.custom_theme_json),
                            )
                            .child(
                                Input::new("community-theme-json-input")
                                    .value(SharedString::from(community_theme_json_draft))
                                    .placeholder(SharedString::from(
                                        "{\"manifest\": {...}, \"theme\": {...}}",
                                    ))
                                    .size(InputSize::Sm)
                                    .on_text_change({
                                        let state_entity = self.state.clone();
                                        move |value, _window, cx| {
                                            state_entity.update(cx, |state, _cx| {
                                                state.app.set_community_theme_json_draft(value);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(d.grid)
                                    .child(
                                        Button::new("community-theme-import-apply", text.import)
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| match state
                                                        .app
                                                        .apply_community_theme_json_draft()
                                                    {
                                                        Ok(()) => {
                                                            state.app.ui_state.toast_message = Some(
                                                                crate::app::ToastMessage::success(
                                                                    "Imported community theme"
                                                                        .to_string(),
                                                                ),
                                                            );
                                                        }
                                                        Err(error) => {
                                                            state.app.ui_state.toast_message = Some(
                                                                crate::app::ToastMessage::error(
                                                                    error,
                                                                ),
                                                            );
                                                        }
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("community-theme-import-clear", text.clear)
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .set_community_theme_json_draft("");
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(render_settings_heading(
                        d,
                        theme.clone(),
                        translations.settings_theme,
                    ))
                    .child({
                        let mut container = div().flex().flex_wrap().gap(d.section);

                        for id in ThemeId::all().iter() {
                            let is_selected = community_theme_id.is_none() && theme_id == *id;
                            let preview_theme = crate::theme::Theme::from_id(*id);

                            container = container.child(self.render_theme_preview_card(
                                *id,
                                preview_theme,
                                is_selected,
                                theme.clone(),
                                translations.settings_active,
                                cx,
                            ));
                        }

                        container
                    }),
            )
    }

    /// Render language settings content
    pub(crate) fn render_language_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let language = state.app.ui_state.language;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        let language_controls = Language::all()
            .iter()
            .map(|candidate| {
                let candidate = *candidate;
                let selected = candidate == language;
                let label = candidate.name();
                let element_id = ElementId::from(SharedString::from(format!(
                    "settings-language-{}",
                    candidate.code()
                )));
                let focus_handle = if selected {
                    self.preference_focus_handle(PreferencesSetting::Language, cx)
                        .unwrap_or_else(|| settings_choice_focus_handle(&element_id, cx))
                } else {
                    settings_choice_focus_handle(&element_id, cx)
                };
                cx.register_accessible(AccessibilityNode {
                    element_id: element_id.clone(),
                    label: label.into(),
                    props: AriaProps::with_role(AriaRole::Button)
                        .maybe_state(selected, AriaState::Pressed(true)),
                });

                let state_for_click = self.state.clone();
                let state_for_key = self.state.clone();
                let button = Button::new(element_id, label)
                    .variant(if selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .selected(selected)
                    .theme(theme.to_button_theme())
                    .build()
                    .track_focus(&focus_handle)
                    .track_focus_element(&focus_handle)
                    .on_click(move |_: &ClickEvent, _window, cx| {
                        state_for_click.update(cx, |state, cx| {
                            state.app.set_language(candidate);
                            let layout = state.layout.read(cx);
                            if let Err(error) = state.app.save_config(layout) {
                                log::error!("Failed to save config: {error}");
                            }
                            cx.notify();
                        });
                    })
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            state_for_key.update(cx, |state, cx| {
                                state.app.set_language(candidate);
                                let layout = state.layout.read(cx);
                                if let Err(error) = state.app.save_config(layout) {
                                    log::error!("Failed to save config: {error}");
                                }
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }
                    });

                #[cfg(feature = "dev-api")]
                let button = button.dev_track_with_state(
                    format!("settings.language.{}", candidate.code()),
                    DevElementState::default().selected(selected),
                );

                let button = if selected {
                    self.preference_control(PreferencesSetting::Language, button, cx)
                } else {
                    button.into_any_element()
                };
                (button, focus_handle)
            })
            .collect::<Vec<_>>();
        let language_focus_handles = language_controls
            .iter()
            .map(|(_, handle)| handle.clone())
            .collect::<Vec<_>>();
        let language_buttons = language_controls
            .into_iter()
            .map(|(button, _)| button)
            .collect::<Vec<_>>();

        div().flex().flex_col().gap(d.section_lg).child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(translations.settings_language),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap_md)
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "tab"
                                && focus_settings_choice_relative(
                                    &language_focus_handles,
                                    window,
                                    cx,
                                    event.keystroke.modifiers.shift,
                                )
                            {
                                cx.stop_propagation();
                            }
                        })
                        .children(language_buttons),
                ),
        )
    }

    /// Render a visual preview card for a theme showing its color scheme
    pub(super) fn render_theme_preview_card(
        &self,
        theme_id: ThemeId,
        preview_theme: crate::theme::Theme,
        is_selected: bool,
        current_theme: crate::theme::Theme,
        active_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = AppearanceTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let card = div()
            .flex()
            .flex_col()
            .w(rems(12.5))
            .rounded(d.r_md)
            .overflow_hidden()
            .cursor_pointer()
            .id(SharedString::from(format!(
                "theme-preview-{}",
                theme_id.name()
            )))
            .focusable()
            .focus_visible(|s| {
                s.border_color(current_theme.border_focused)
                    .bg(current_theme.surface_hover)
            })
            .border_2()
            .border_color(if is_selected {
                current_theme.accent
            } else {
                current_theme.border
            })
            .bg(preview_theme.surface)
            .shadow_md()
            .hover(|style| {
                style.border_color(if is_selected {
                    current_theme.accent_hover
                } else {
                    current_theme.border_focused
                })
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_theme(theme_id);
                    });
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, _window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_theme(theme_id);
                    });
                    cx.stop_propagation();
                }
            }))
            .child(
                // Theme name header
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(rems(2.5))
                    .bg(preview_theme.background)
                    .border_b_1()
                    .border_color(preview_theme.border)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(preview_theme.text_primary)
                            .child(theme_id.name()),
                    ),
            )
            .child(
                // Color swatches grid
                div()
                    .flex()
                    .flex_col()
                    .p(d.pad_x)
                    .gap(d.gap)
                    .child(
                        // Background colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "BG",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Surf",
                                preview_theme.surface,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Hover",
                                preview_theme.surface_hover,
                                preview_theme.text_primary,
                            )),
                    )
                    .child(
                        // Accent and text colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "Accent",
                                preview_theme.accent,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Text",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Muted",
                                preview_theme.background,
                                preview_theme.text_muted,
                            )),
                    )
                    .child(
                        // Semantic colors row
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "✓",
                                preview_theme.success,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "⚠",
                                preview_theme.warning,
                                preview_theme.text_on_accent,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "✗",
                                preview_theme.error,
                                preview_theme.text_on_accent,
                            )),
                    )
                    .child(
                        // Button variants preview
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .pt(d.pad_y)
                            .border_t_1()
                            .border_color(preview_theme.border)
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(preview_theme.text_muted)
                                    .child(text.buttons),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.grid)
                                    .child(
                                        Button::new("preview-primary", text.primary_abbreviation)
                                            .aria_label(text.primary_preview)
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new(
                                            "preview-secondary",
                                            text.secondary_abbreviation,
                                        )
                                        .aria_label(text.secondary_preview)
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .theme(preview_theme.to_button_theme())
                                        .build(),
                                    )
                                    .child(
                                        Button::new(
                                            "preview-destructive",
                                            text.destructive_abbreviation,
                                        )
                                        .aria_label(text.destructive_preview)
                                        .variant(ButtonVariant::Destructive)
                                        .size(ButtonSize::Xs)
                                        .theme(preview_theme.to_button_theme())
                                        .build(),
                                    )
                                    .child(
                                        Button::new("preview-ghost", text.ghost_abbreviation)
                                            .aria_label(text.ghost_preview)
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-outline", text.outline_abbreviation)
                                            .aria_label(text.outline_preview)
                                            .variant(ButtonVariant::Outline)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(d.grid)
                                    .child(
                                        Toggle::new("preview-toggle-off")
                                            .checked(false)
                                            .label(text.off)
                                            .style(ToggleStyle::Segmented)
                                            .theme(preview_theme.to_toggle_theme()),
                                    )
                                    .child(
                                        Toggle::new("preview-toggle-on")
                                            .checked(true)
                                            .label(text.on)
                                            .style(ToggleStyle::Segmented)
                                            .theme(preview_theme.to_toggle_theme()),
                                    ),
                            ),
                    ),
            )
            .when(is_selected, |this| {
                this.child(
                    // Selected indicator
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(rems(1.875))
                        .bg(current_theme.accent)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(current_theme.text_on_accent)
                                .child(format!("✓ {}", active_label)),
                        ),
                )
            });
        dev_track!(card, format!("settings.appearance.theme.{theme_id:?}"))
    }

    pub(super) fn render_community_theme_preview_card(
        &self,
        theme_id: CommunityThemeId,
        preview_theme: crate::theme::Theme,
        is_selected: bool,
        current_theme: crate::theme::Theme,
        active_label: &'static str,
        apply_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let tags = theme_id.tags().join(" / ");
        div()
            .flex()
            .flex_col()
            .w(rems(14.5))
            .rounded(d.r_md)
            .overflow_hidden()
            .border_2()
            .border_color(if is_selected {
                current_theme.accent
            } else {
                current_theme.border
            })
            .bg(preview_theme.surface)
            .shadow_md()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .p(d.pad_x)
                    .bg(preview_theme.background)
                    .border_b_1()
                    .border_color(preview_theme.border)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(preview_theme.text_primary)
                            .child(theme_id.name()),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(preview_theme.text_muted)
                            .child(theme_id.author()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(d.pad_x)
                    .gap(d.gap)
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "BG",
                                preview_theme.background,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Surf",
                                preview_theme.surface,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Accent",
                                preview_theme.accent,
                                preview_theme.text_on_accent,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(self.render_color_swatch(
                                &d,
                                "EQ",
                                preview_theme.plugin_palette.plugin_colors.eq,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Meter",
                                preview_theme.feedback.meter_normal,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Graph",
                                preview_theme.plugin_palette.graph_colors.corrected,
                                preview_theme.text_primary,
                            )),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(preview_theme.text_muted)
                            .child(tags),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(
                                Button::new(
                                    SharedString::from(format!(
                                        "community-theme-apply-{}",
                                        theme_id.value()
                                    )),
                                    if is_selected {
                                        active_label
                                    } else {
                                        apply_label
                                    },
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Xs)
                                .theme(preview_theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.set_community_theme(theme_id);
                                        });
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!(
                                        "community-theme-json-{}",
                                        theme_id.value()
                                    )),
                                    "JSON",
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .theme(preview_theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        let json = theme_id.to_community_json().unwrap_or_default();
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            json,
                                        ));
                                        view.state.update(cx, |state, _cx| {
                                            state.app.ui_state.toast_message =
                                                Some(crate::app::ToastMessage::success(format!(
                                                    "Copied {} JSON",
                                                    theme_id.name()
                                                )));
                                        });
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
    }

    /// Render a small color swatch with label
    pub(super) fn render_color_swatch(
        &self,
        d: &Ds,
        label: &'static str,
        bg_color: gpui::Rgba,
        text_color: gpui::Rgba,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .h(rems(2.0))
            .rounded(d.r_sm)
            .bg(bg_color)
            .border_1()
            .border_color(crate::theme::Theme::with_opacity(text_color, 0.2))
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label),
            )
    }
}
