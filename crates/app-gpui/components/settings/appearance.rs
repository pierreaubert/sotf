//! Appearance settings content (Theme and Language)

use crate::components::design::Ds;
use crate::i18n::Language;
use crate::theme::{CommunityThemeId, ThemeAccentPreference, ThemeId};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_themes::{
    AccessibilityPalette, ThemeAppearance, ThemeModePreference, ThemeSchedule, TimeOfDay,
};
use gpui_ui_kit::{
    Button, ButtonSet, ButtonSetOption, ButtonSetSize, ButtonSize, ButtonVariant, NumberInput,
    NumberInputSize, Toggle, ToggleSize, ToggleStyle,
};

#[derive(Clone, Copy)]
enum ScheduleBoundary {
    LightStart,
    DarkStart,
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

fn schedule_from_preference(preference: &ThemeModePreference) -> ThemeSchedule {
    match preference {
        ThemeModePreference::Scheduled { schedule } => *schedule,
        _ => ThemeSchedule::default(),
    }
}

fn clamp_hour(value: f64) -> u8 {
    value.round().clamp(0.0, 23.0) as u8
}

fn clamp_minute(value: f64) -> u8 {
    value.round().clamp(0.0, 59.0) as u8
}

fn set_schedule_boundary(
    mut schedule: ThemeSchedule,
    boundary: ScheduleBoundary,
    time: TimeOfDay,
) -> ThemeSchedule {
    match boundary {
        ScheduleBoundary::LightStart => schedule.light_start = time,
        ScheduleBoundary::DarkStart => schedule.dark_start = time,
    }
    schedule
}

fn accessibility_value(palette: AccessibilityPalette) -> &'static str {
    match palette {
        AccessibilityPalette::Standard => "standard",
        AccessibilityPalette::HighContrast => "high_contrast",
        AccessibilityPalette::Protanopia => "protanopia",
        AccessibilityPalette::Deuteranopia => "deuteranopia",
        AccessibilityPalette::Tritanopia => "tritanopia",
    }
}

fn accessibility_palette_from_value(value: &SharedString) -> Option<AccessibilityPalette> {
    match value.as_ref() {
        "standard" => Some(AccessibilityPalette::Standard),
        "high_contrast" => Some(AccessibilityPalette::HighContrast),
        "protanopia" => Some(AccessibilityPalette::Protanopia),
        "deuteranopia" => Some(AccessibilityPalette::Deuteranopia),
        "tritanopia" => Some(AccessibilityPalette::Tritanopia),
        _ => None,
    }
}

fn theme_appearance_from_window(window: &Window) -> ThemeAppearance {
    match window.appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeAppearance::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeAppearance::Light,
    }
}

fn render_schedule_time_row(
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
        .items_center()
        .gap(d.gap)
        .child(
            div()
                .w(rems(5.5))
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

fn render_accent_swatch(
    d: Ds,
    theme: crate::theme::Theme,
    state_entity: Entity<crate::app::AppState>,
    preference: ThemeAccentPreference,
    selected: bool,
    fallback_accent: Rgba,
) -> impl IntoElement {
    let preview_color = preference.preview_color(fallback_accent);
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
}

impl PlayerView {
    /// Render theme settings content
    pub(crate) fn render_theme_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme_id = state.app.ui_state.theme_id;
        let theme_mode_preference = state.app.ui_state.theme_mode_preference.clone();
        let schedule = schedule_from_preference(&theme_mode_preference);
        let is_scheduled = matches!(
            &theme_mode_preference,
            ThemeModePreference::Scheduled { .. }
        );
        let accessibility_palette = state.app.ui_state.accessibility_palette;
        let theme_accent_preference = state.app.ui_state.theme_accent_preference;
        let community_theme_id = state.app.ui_state.community_theme_id;
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
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Mode"),
                    )
                    .child({
                        let state_entity = self.state.clone();
                        ButtonSet::new("theme-mode-select")
                            .size(ButtonSetSize::Sm)
                            .options(vec![
                                ButtonSetOption::new("follow_system", "System"),
                                ButtonSetOption::new("light", "Light"),
                                ButtonSetOption::new("dark", "Dark"),
                                ButtonSetOption::new("scheduled", "Scheduled"),
                            ])
                            .selected(theme_mode_value(&theme_mode_preference))
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, window, cx| {
                                if let Some(preference) =
                                    theme_mode_preference_from_value(value, schedule)
                                {
                                    let system_appearance = theme_appearance_from_window(window);
                                    state_entity.update(cx, |state, _cx| {
                                        state.app.set_theme_mode_preference_with_system(
                                            preference,
                                            system_appearance,
                                        );
                                    });
                                }
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
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Accent"),
                    )
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
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Accessibility"),
                    )
                    .child({
                        let state_entity = self.state.clone();
                        ButtonSet::new("theme-accessibility-select")
                            .size(ButtonSetSize::Sm)
                            .options(
                                AccessibilityPalette::all()
                                    .iter()
                                    .map(|palette| {
                                        ButtonSetOption::new(
                                            accessibility_value(*palette),
                                            palette.name(),
                                        )
                                    })
                                    .collect(),
                            )
                            .selected(accessibility_value(accessibility_palette))
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, window, cx| {
                                if let Some(palette) = accessibility_palette_from_value(value) {
                                    let system_appearance = theme_appearance_from_window(window);
                                    state_entity.update(cx, |state, _cx| {
                                        state.app.set_accessibility_palette_with_system(
                                            palette,
                                            system_appearance,
                                        );
                                    });
                                }
                            })
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(d.section)
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .text_color(theme.text_secondary)
                                    .child("Motion"),
                            )
                            .child(
                                Toggle::new("theme-reduce-motion")
                                    .size(ToggleSize::Sm)
                                    .checked(reduce_motion)
                                    .label("Reduce motion")
                                    .style(ToggleStyle::Segmented)
                                    .theme(theme.to_toggle_theme())
                                    .on_change({
                                        let state_entity = self.state.clone();
                                        move |enabled, _window, cx| {
                                            state_entity.update(cx, |state, _cx| {
                                                state.app.set_reduce_motion(enabled);
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Community"),
                    )
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
                                cx,
                            ));
                        }

                        container
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(translations.settings_theme),
                    )
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
                .child({
                    let state_entity = self.state.clone();
                    ButtonSet::new("language-select")
                        .options(
                            Language::all()
                                .iter()
                                .map(|lang| ButtonSetOption::new(lang.name(), lang.name()))
                                .collect(),
                        )
                        .selected(language.name())
                        .theme(theme.to_button_set_theme())
                        .on_change(move |value, _window, cx| {
                            let lang = Language::all()
                                .iter()
                                .find(|l| l.name() == value.as_ref())
                                .copied();
                            if let Some(lang) = lang {
                                state_entity.update(cx, |state, _cx| {
                                    state.app.set_language(lang);
                                });
                            }
                        })
                }),
        )
    }

    /// Render a visual preview card for a theme showing its color scheme
    fn render_theme_preview_card(
        &self,
        theme_id: ThemeId,
        preview_theme: crate::theme::Theme,
        is_selected: bool,
        current_theme: crate::theme::Theme,
        active_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        div()
            .flex()
            .flex_col()
            .w(rems(12.5))
            .rounded(d.r_md)
            .overflow_hidden()
            .cursor_pointer()
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
                                    .child("Buttons"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(d.grid)
                                    .child(
                                        Button::new("preview-primary", "Pri")
                                            .aria_label("Primary variant preview")
                                            .variant(ButtonVariant::Primary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-secondary", "Sec")
                                            .aria_label("Secondary variant preview")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-destructive", "Del")
                                            .aria_label("Destructive variant preview")
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-ghost", "Gho")
                                            .aria_label("Ghost variant preview")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Xs)
                                            .theme(preview_theme.to_button_theme())
                                            .build(),
                                    )
                                    .child(
                                        Button::new("preview-outline", "Out")
                                            .aria_label("Outline variant preview")
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
                                            .label("Off".to_string())
                                            .style(ToggleStyle::Segmented)
                                            .theme(preview_theme.to_toggle_theme()),
                                    )
                                    .child(
                                        Toggle::new("preview-toggle-on")
                                            .checked(true)
                                            .label("On".to_string())
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
            })
    }

    fn render_community_theme_preview_card(
        &self,
        theme_id: CommunityThemeId,
        preview_theme: crate::theme::Theme,
        is_selected: bool,
        current_theme: crate::theme::Theme,
        active_label: &'static str,
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
                                preview_theme.plugin_colors.eq,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Meter",
                                preview_theme.meter_normal,
                                preview_theme.text_primary,
                            ))
                            .child(self.render_color_swatch(
                                &d,
                                "Graph",
                                preview_theme.graph_colors.corrected,
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
                                    if is_selected { active_label } else { "Apply" },
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
    fn render_color_swatch(
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
            .border_color(gpui::Rgba {
                r: text_color.r,
                g: text_color.g,
                b: text_color.b,
                a: 0.2,
            })
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label),
            )
    }
}
