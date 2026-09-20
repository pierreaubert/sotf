use crate::app::i18n::DesktopTranslations;
use crate::app::types::{PreferencesCategory, PreferencesSetting, Screen, SettingsTab};
use crate::components::{design::Ds, settings_tab_label};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Input, Select, SelectOption, Text};

impl PlayerView {
    pub(crate) fn request_preferences_close(app: &mut crate::app::App) {
        app.audio_device_state.output_ui.open = false;
        app.audio_device_state.output_ui.highlight = None;
        app.audio_device_state.close_hal_dropdowns();
        app.settings.navigation.category_open = false;
        if app.audio_device_state.output_draft.is_dirty()
            || app.audio_device_state.audio_apply.pending.is_some()
        {
            app.settings.navigation.close_pending = true;
        } else {
            Self::finish_preferences_close(app);
        }
    }

    pub(crate) fn finish_preferences_close(app: &mut crate::app::App) {
        app.settings.navigation.close_pending = false;
        let screen = app
            .settings
            .navigation
            .return_screen
            .unwrap_or(Screen::Home);
        app.set_screen(screen, "PreferencesDone");
    }

    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let close_pending = state.app.settings.navigation.close_pending;
        let audio_apply_pending = state.app.audio_device_state.audio_apply.pending.is_some();
        let applying_label = crate::app::i18n::AudioPreferencesPersistenceTranslations::applying(
            state.app.ui_state.language,
        );
        let close_error = state.app.audio_device_state.output_draft.error.clone();
        let close_labels = crate::app::i18n::PreferencesCloseTranslations::for_language(
            state.app.ui_state.language,
        );
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let text = DesktopTranslations::for_language(state.app.ui_state.language);
        let language = state.app.ui_state.language;
        let tabs = SettingsTab::visible_tabs();
        let active = if tabs.contains(&state.app.ui_state.active_settings_tab) {
            state.app.ui_state.active_settings_tab
        } else {
            SettingsTab::fallback_for_platform()
        };
        let category = PreferencesCategory::for_tab(active);
        let query = state.app.settings.navigation.query.clone();
        let open = state.app.settings.navigation.category_open;
        let category_highlight = state.app.settings.navigation.category_highlight;
        let sizing = crate::ui::resolve_sizing_context(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let compact = sizing.window_width_rems < 52.0;
        let entity = self.state.downgrade();
        let categories: Vec<_> = PreferencesCategory::ALL
            .into_iter()
            .filter(|category| !category.visible_tabs(cfg!(target_os = "ios")).is_empty())
            .collect();
        let category_index = categories.iter().position(|group| *group == category);

        let mut navigation = div()
            .flex()
            .flex_col()
            .gap(d.grid)
            .w(rems(16.0))
            .flex_shrink_0();
        for group in &categories {
            let group = *group;
            let entity = self.state.clone();
            navigation = navigation.child(track_settings_route(
                Button::new(
                    SharedString::from(format!("preferences-category-{group:?}")),
                    text.categories[group as usize],
                )
                .size(ButtonSize::Sm)
                .variant(if category == group {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if let Some(tab) = group.visible_tabs(cfg!(target_os = "ios")).first() {
                            state.app.ui_state.active_settings_tab = *tab;
                        }
                        state.app.settings.navigation.query.clear();
                        state.app.settings.navigation.clear_setting_target();
                        cx.notify();
                    });
                }),
                group.tabs()[0],
                category == group,
            ));
        }
        let mut content = div().flex().flex_col().gap(d.section).min_w_0().w_full();
        if query.trim().is_empty() {
            content = content.child(Text::section_header(text.categories[category as usize]));
            let group_tabs = category.visible_tabs(cfg!(target_os = "ios"));
            if group_tabs.len() > 1 {
                let mut sections = div().flex().flex_wrap().gap(d.grid);
                for tab in group_tabs {
                    let entity = self.state.clone();
                    sections = sections.child(track_settings_route(
                        Button::new(
                            SharedString::from(format!("settings-tab-{tab:?}")),
                            settings_tab_label(tab, &translations),
                        )
                        .size(ButtonSize::Sm)
                        .variant(if tab == active {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .theme(theme.to_button_theme())
                        .on_click_event(move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                state.app.ui_state.active_settings_tab = tab;
                                state.app.settings.navigation.clear_setting_target();
                                cx.notify();
                            });
                        }),
                        tab,
                        active == tab,
                    ));
                }
                content = content.child(sections);
            }
            content = content.child(self.render_settings_tab_content(active, cx));
        } else {
            let mut found = false;
            let mut matched_tabs = Vec::new();
            for setting in PreferencesSetting::ALL {
                if !setting.is_available()
                    || !tabs.contains(&setting.tab())
                    || !setting.matches(&query, language)
                {
                    continue;
                }
                found = true;
                matched_tabs.push(setting.tab());
                let entity = self.state.downgrade();
                let result = Button::new(
                    SharedString::from(format!("preferences-setting-{setting:?}")),
                    format!(
                        "{} · {}",
                        text.categories[PreferencesCategory::for_tab(setting.tab()) as usize],
                        setting.label(language)
                    ),
                )
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Secondary)
                .theme(theme.to_button_theme())
                .on_click_event(move |_, window, cx| {
                    let Some(entity) = entity.upgrade() else {
                        return;
                    };
                    let focus = entity.update(cx, |state, cx| {
                        let focus = cx.focus_handle();
                        state.app.ui_state.active_settings_tab = setting.tab();
                        if setting == PreferencesSetting::AudioAnalysis
                            && !state
                                .app
                                .settings
                                .expanded_sections
                                .iter()
                                .any(|id| id == "library-analysis")
                        {
                            state
                                .app
                                .settings
                                .expanded_sections
                                .push("library-analysis".into());
                        }
                        let navigation = &mut state.app.settings.navigation;
                        navigation.query.clear();
                        navigation.category_open = false;
                        navigation.setting = Some(setting);
                        navigation.setting_focus = Some(focus.clone());
                        navigation.reveal_setting = true;
                        cx.notify();
                        focus
                    });
                    focus.focus(window, cx);
                });
                #[cfg(feature = "dev-api")]
                let result = {
                    use crate::app::dev_api::DevTrackExt;
                    result.dev_track(format!("preferences.result.{setting:?}"))
                };
                content = content.child(result);
            }
            for tab in tabs {
                if matched_tabs.contains(&tab) {
                    continue;
                }
                let label = settings_tab_label(tab, &translations);
                if !PreferencesCategory::matches(tab, &query, label) {
                    continue;
                }
                found = true;
                let entity = self.state.clone();
                let group = PreferencesCategory::for_tab(tab);
                content = content.child(
                    Button::new(
                        SharedString::from(format!("preferences-result-{tab:?}")),
                        format!("{} · {label}", text.categories[group as usize]),
                    )
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.ui_state.active_settings_tab = tab;
                            state.app.settings.navigation.clear_setting_target();
                            state.app.settings.navigation.query.clear();
                            state.app.settings.navigation.clear_setting_target();
                            cx.notify();
                        });
                    }),
                );
            }
            if !found {
                content = content.child(Text::body(text.no_results));
            }
        }
        let mut body = div().flex().flex_1().min_h_0().min_w_0().gap(d.section);
        if !compact {
            body = body.child(navigation);
        }
        body = body.child(
            div()
                .id("settings-content-scroll")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .track_scroll(&self.scroll.preferences)
                .child(div().w_full().max_w(rems(64.0)).py(d.gap).child(content))
                .map(|content| track_preferences_control(content, "preferences.content")),
        );

        let mut root = div()
            .id("settings-screen")
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(d.section)
            .p(d.card)
            .bg(theme.background)
            .text_color(theme.text_primary)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(d.gap)
                    .child(Text::section_header(text.preferences))
                    .child(
                        div().flex_1().min_w(rems(12.0)).max_w(rems(32.0)).child(
                            track_preferences_control(
                                Input::new("preferences-search")
                                    .value(query)
                                    .placeholder(if compact {
                                        translations.library_search
                                    } else {
                                        text.search
                                    })
                                    .aria_label(text.search)
                                    .on_text_change(move |value, _, cx| {
                                        let Some(entity) = entity.upgrade() else {
                                            return;
                                        };
                                        entity.update(cx, |state, cx| {
                                            state.app.settings.navigation.query = value.to_string();
                                            state.app.settings.navigation.clear_setting_target();
                                            cx.notify();
                                        });
                                    }),
                                "preferences.search",
                            ),
                        ),
                    )
                    .child(track_preferences_control(
                        Button::new("preferences-done", text.done)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, window, cx| {
                                view.state.update(cx, |state, cx| {
                                    Self::request_preferences_close(&mut state.app);
                                    cx.notify();
                                });
                                view.focus_handle.focus(window, cx);
                                cx.notify();
                            })),
                        "preferences.done",
                    )),
            );
        if close_pending {
            let labels = close_labels;
            let mut actions = div().flex().flex_wrap().gap(d.gap);
            for (action, label) in [
                ("keep", labels[1]),
                ("discard", labels[2]),
                ("apply", labels[3]),
            ] {
                let button = Button::new(format!("preferences-close-{action}"), label)
                    .disabled(action != "keep" && audio_apply_pending)
                    .size(ButtonSize::Sm)
                    .variant(if action == "apply" {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, window, cx| {
                        view.state.update(cx, |state, cx| {
                            match action {
                                "keep" => state.app.settings.navigation.close_pending = false,
                                "discard" => {
                                    if state.app.audio_device_state.audio_apply.pending.is_some() {
                                        return;
                                    }
                                    state.app.audio_device_state.output_draft.discard();
                                    Self::finish_preferences_close(&mut state.app);
                                }
                                "apply" if Self::apply_audio_output_draft(state, cx) => {
                                    Self::finish_preferences_close(&mut state.app);
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    }));
                #[cfg(feature = "dev-api")]
                let button = {
                    use crate::app::dev_api::DevTrackExt;
                    button.dev_track(format!("preferences.close.{action}"))
                };
                actions = actions.child(button);
            }
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(Text::section_header(if audio_apply_pending {
                        applying_label
                    } else {
                        labels[0]
                    }))
                    .when_some(close_error, |el, error| {
                        el.child(Text::body(error).color(theme.error))
                    })
                    .child(actions),
            );
        }
        if compact {
            let change = self.state.downgrade();
            let toggle = self.state.downgrade();
            let highlight = self.state.downgrade();
            root = root.child(track_preferences_control(
                Select::new("preferences-category")
                    .label(text.category)
                    .aria_label(text.category)
                    .options(
                        categories
                            .iter()
                            .map(|group| {
                                SelectOption::new(
                                    (*group as usize).to_string(),
                                    text.categories[*group as usize],
                                )
                            })
                            .collect(),
                    )
                    .selected((category as usize).to_string())
                    .is_open(open)
                    .highlighted_index(category_highlight)
                    .on_highlight(move |index, _, cx| {
                        if let Some(state) = highlight.upgrade() {
                            state.update(cx, |state, cx| {
                                state.app.settings.navigation.category_highlight = index;
                                cx.notify();
                            });
                        }
                    })
                    .theme(theme.to_select_theme())
                    .on_toggle(move |open, _, cx| {
                        let Some(toggle) = toggle.upgrade() else {
                            return;
                        };
                        toggle.update(cx, |state, cx| {
                            state.app.settings.navigation.category_open = open;
                            state.app.settings.navigation.category_highlight =
                                if open { category_index } else { None };
                            cx.notify();
                        });
                    })
                    .on_change(move |value: &SharedString, _, cx| {
                        if let Ok(index) = value.parse::<usize>()
                            && let Some(group) = PreferencesCategory::ALL.get(index)
                        {
                            let Some(change) = change.upgrade() else {
                                return;
                            };
                            change.update(cx, |state, cx| {
                                if let Some(tab) =
                                    group.visible_tabs(cfg!(target_os = "ios")).first()
                                {
                                    state.app.ui_state.active_settings_tab = *tab;
                                }
                                state.app.settings.navigation.category_open = false;
                                state.app.settings.navigation.query.clear();
                                state.app.settings.navigation.clear_setting_target();
                                cx.notify();
                            });
                        }
                    }),
                "preferences.category",
            ));
        }
        root.child(body).into_any_element()
    }
}

fn track_settings_route(
    element: impl IntoElement,
    tab: SettingsTab,
    is_selected: bool,
) -> AnyElement {
    #[cfg(feature = "dev-api")]
    {
        use crate::app::dev_api::DevTrackExt;
        element
            .into_any_element()
            .dev_track_with_state(
                format!("settings.tab.{tab:?}"),
                crate::app::dev_api::DevElementState::default().selected(is_selected),
            )
            .into_any_element()
    }
    #[cfg(not(feature = "dev-api"))]
    {
        let _ = (tab, is_selected);
        element.into_any_element()
    }
}

fn track_preferences_control(element: impl IntoElement, selector: &'static str) -> AnyElement {
    #[cfg(feature = "dev-api")]
    {
        use crate::app::dev_api::DevTrackExt;
        element
            .into_any_element()
            .dev_track(selector)
            .into_any_element()
    }
    #[cfg(not(feature = "dev-api"))]
    {
        let _ = selector;
        element.into_any_element()
    }
}
