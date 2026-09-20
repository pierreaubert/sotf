use super::PlayerView;
use crate::app::Screen;
use crate::app::i18n::{DesktopTranslations, SpectrumTranslations};
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Select, SelectOption, Text};

impl PlayerView {
    pub(super) fn studio_workspace_navigation(
        &self,
        screen: Screen,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let selected = match screen {
            Screen::Studio | Screen::PluginGraph | Screen::Spectrum => 0,
            Screen::Recording => 1,
            Screen::RoomEq | Screen::HeadphoneEq | Screen::Spinorama => 2,
            Screen::ListeningTest => 3,
            _ => return None,
        };
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let labels = DesktopTranslations::for_language(ui.language).studio_sections;
        let d = Ds::from_cx(cx);
        if self.workflow_is_compact(cx) {
            let targets: Vec<_> = [
                (Screen::Studio, 0, ui.translations.screen_studio_rack),
                (Screen::PluginGraph, 0, ui.translations.screen_studio_full),
                (
                    Screen::Spectrum,
                    0,
                    SpectrumTranslations::for_language(ui.language).analyzer,
                ),
                (Screen::Recording, 1, ui.translations.screen_recording),
                (Screen::RoomEq, 2, ui.translations.screen_room_eq),
                (Screen::HeadphoneEq, 2, ui.translations.screen_headphone_eq),
                (Screen::Spinorama, 2, ui.translations.screen_spinorama),
                (
                    Screen::ListeningTest,
                    3,
                    ui.translations.screen_listening_test,
                ),
            ]
            .into_iter()
            .filter(|(target, _, _)| ui.release_channel.allows(target.maturity()))
            .collect();
            let current = targets
                .iter()
                .position(|(target, _, _)| *target == screen)
                .unwrap_or(0);
            let toggle = self.state.downgrade();
            let change = self.state.downgrade();
            let highlight = self.state.downgrade();
            let picker = Select::new("studio-workspace-picker")
                .aria_label(ui.translations.screen_studio)
                .options(
                    targets
                        .iter()
                        .enumerate()
                        .map(|(index, (_, category, label))| {
                            SelectOption::new(
                                index.to_string(),
                                format!("{} · {label}", labels[*category]),
                            )
                        })
                        .collect(),
                )
                .selected(current.to_string())
                .is_open(ui.navigation.studio_picker_open)
                .highlighted_index(ui.navigation.studio_picker_highlight)
                .on_highlight(move |index, _, cx| {
                    if let Some(state) = highlight.upgrade() {
                        state.update(cx, |state, cx| {
                            state.app.ui_state.navigation.studio_picker_highlight = index;
                            cx.notify();
                        });
                    }
                })
                .theme(ui.theme.to_select_theme())
                .on_toggle(move |open, _, cx| {
                    if let Some(state) = toggle.upgrade() {
                        state.update(cx, |state, cx| {
                            state.app.ui_state.navigation.studio_picker_open = open;
                            state.app.ui_state.navigation.studio_picker_highlight =
                                open.then_some(current);
                            cx.notify();
                        });
                    }
                })
                .on_change(move |value: &SharedString, _, cx| {
                    if let Ok(index) = value.parse::<usize>()
                        && let Some((target, _, _)) = targets.get(index)
                        && let Some(state) = change.upgrade()
                    {
                        state.update(cx, |state, cx| {
                            state.app.ui_state.navigation.studio_picker_open = false;
                            state.app.set_screen(*target, "StudioWorkspacePicker");
                            cx.notify();
                        });
                    }
                });
            #[cfg(feature = "dev-api")]
            let picker = {
                use crate::app::dev_api::DevTrackExt;
                picker.dev_track("studio.workspace-picker")
            };
            return Some(
                div()
                    .flex_none()
                    .p(d.pad_y)
                    .child(picker)
                    .into_any_element(),
            );
        }
        let mut row = div()
            .flex()
            .flex_wrap()
            .gap(d.gap)
            .p(d.pad_y)
            .flex_shrink_0();
        for (index, target) in [
            Screen::Studio,
            Screen::Recording,
            Screen::RoomEq,
            Screen::ListeningTest,
        ]
        .into_iter()
        .enumerate()
        {
            let target = ui
                .navigation
                .studio_target(index)
                .filter(|screen| ui.release_channel.allows(screen.maturity()))
                .unwrap_or(target);
            if !ui.release_channel.allows(target.maturity()) {
                continue;
            }
            let state = self.state.downgrade();
            let view = cx.entity().downgrade();
            let button = Button::new(
                SharedString::from(format!("studio-category-{index}")),
                labels[index],
            )
            .size(ButtonSize::Sm)
            .variant(if index == selected {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .theme(ui.theme.to_button_theme())
            .on_click_event(move |_, window, cx| {
                if index == selected {
                    return;
                }
                if let Some(state) = state.upgrade() {
                    state.update(cx, |state, cx| {
                        state.app.set_screen(target, "StudioCategory");
                        cx.notify();
                    });
                }
                if let Some(view) = view.upgrade() {
                    view.update(cx, |view, cx| {
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    });
                }
            });
            #[cfg(feature = "dev-api")]
            let button = {
                use crate::app::dev_api::DevTrackExt;
                button.dev_track(format!("studio.category.{index}"))
            };
            row = row.child(button);
        }
        let tools = match selected {
            0 => vec![
                (Screen::Studio, ui.translations.screen_studio_rack),
                (Screen::PluginGraph, ui.translations.screen_studio_full),
                (
                    Screen::Spectrum,
                    SpectrumTranslations::for_language(ui.language).analyzer,
                ),
            ],
            2 => vec![
                (Screen::RoomEq, ui.translations.screen_room_eq),
                (Screen::HeadphoneEq, ui.translations.screen_headphone_eq),
                (Screen::Spinorama, ui.translations.screen_spinorama),
            ],
            _ => Vec::new(),
        };
        let mut subtools = div().flex().flex_wrap().gap(d.gap).px(d.pad_y).pb(d.pad_y);
        for (target, label) in tools {
            if !ui.release_channel.allows(target.maturity()) {
                continue;
            }
            let state = self.state.downgrade();
            let view = cx.entity().downgrade();
            let button = Button::new(
                SharedString::from(format!("studio-subtool-{target:?}")),
                label,
            )
            .size(ButtonSize::Sm)
            .variant(if screen == target {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .theme(ui.theme.to_button_theme())
            .on_click_event(move |_, window, cx| {
                if target == screen {
                    return;
                }
                if let Some(state) = state.upgrade() {
                    state.update(cx, |state, cx| {
                        state.app.set_screen(target, "StudioSubtool");
                        cx.notify();
                    });
                }
                if let Some(view) = view.upgrade() {
                    view.update(cx, |view, cx| {
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    });
                }
            });
            #[cfg(feature = "dev-api")]
            let button = {
                use crate::app::dev_api::DevTrackExt;
                button.dev_track(format!("studio.subtool.{target:?}"))
            };
            subtools = subtools.child(button);
        }
        Some(
            div()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .child(row)
                .when(matches!(selected, 0 | 2), |el| el.child(subtools))
                .into_any_element(),
        )
    }

    pub(super) fn desktop_navigation_compact(&self, cx: &Context<Self>) -> bool {
        let ui = &self.state.read(cx).app.ui_state;
        !ui.primary_nav_collapsed
            && super::navigation_is_compact(
                ui.window_width,
                super::compute_combined_scale(
                    ui.window_width,
                    ui.window_height,
                    ui.font_scale,
                    ui.min_font_size_px,
                    ui.max_font_size_px,
                ),
                false,
            )
    }

    fn desktop_destinations(&self, cx: &Context<Self>) -> Vec<(Screen, &'static str)> {
        let state = self.state.read(cx);
        let t = &state.app.ui_state.translations;
        [
            (Screen::Home, t.screen_home),
            (Screen::Library, t.screen_library),
            (Screen::NowPlaying, t.screen_now_playing),
            (Screen::Queue, t.screen_queue),
            (Screen::Playlists, t.screen_playlists),
            (Screen::Streams, t.screen_streams),
            (Screen::StudioHub, t.screen_studio),
        ]
        .into_iter()
        .filter(|(screen, _)| state.app.ui_state.release_channel.allows(screen.maturity()))
        .collect()
    }

    pub(super) fn render_app_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        if state.app.ui_state.primary_nav_collapsed {
            return self.render_legacy_app_sidebar(cx);
        }
        let d = Ds::from_cx(cx);
        let theme = state.app.ui_state.theme.clone();
        let current = state.app.ui_state.current_screen;
        let text = DesktopTranslations::for_language(state.app.ui_state.language);
        let mut sidebar = div()
            .id("desktop-navigation")
            .track_focus(&self.window_state.navigation_focus)
            .w(rems(12.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(d.grid)
            .p(d.pad_y)
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border);
        for (screen, label) in self.desktop_destinations(cx) {
            let entity = self.state.clone();
            let selected =
                current == screen || screen == Screen::StudioHub && current.is_studio_tool();
            let button = Button::new(SharedString::from(format!("desktop-nav-{screen:?}")), label)
                .size(ButtonSize::Sm)
                .variant(if selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if screen == Screen::StudioHub {
                            state.app.enter_studio_mode("DesktopNavigation");
                        } else {
                            state.app.set_screen(screen, "DesktopNavigation");
                        }
                        cx.notify();
                    });
                });
            #[cfg(feature = "dev-api")]
            let button = {
                use crate::app::dev_api::DevTrackExt;
                let button = button.dev_track(format!(
                    "sidebar.nav-{}",
                    format!("{screen:?}").to_lowercase()
                ));
                if screen == Screen::NowPlaying {
                    button.dev_track("sidebar.nav-playing").into_any_element()
                } else {
                    button.into_any_element()
                }
            };
            sidebar = sidebar.child(button);
        }
        let entity = self.state.clone();
        sidebar
            .child(div().flex_1())
            .child(
                Button::new("desktop-preferences", text.preferences)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.set_screen(Screen::Settings, "DesktopPreferences");
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_compact_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let d = Ds::from_cx(cx);
        let text = DesktopTranslations::for_language(ui.language);
        let destinations = self.desktop_destinations(cx);
        let current = if ui.current_screen.is_studio_tool() {
            Screen::StudioHub
        } else {
            ui.current_screen
        };
        let selected = destinations
            .iter()
            .position(|(screen, _)| *screen == current)
            .unwrap_or(0);
        let change = self.state.downgrade();
        let toggle = self.state.downgrade();
        let highlight = self.state.downgrade();
        let settings = self.state.clone();
        div()
            .track_focus(&self.window_state.navigation_focus)
            .flex()
            .flex_wrap()
            .gap(d.gap)
            .p(d.pad_y)
            .child({
                let select = Select::new("desktop-go-to")
                    .label(text.category)
                    .options(
                        destinations
                            .iter()
                            .enumerate()
                            .map(|(index, (_, label))| SelectOption::new(index.to_string(), *label))
                            .collect(),
                    )
                    .selected(selected.to_string())
                    .is_open(ui.show_studio_menu)
                    .highlighted_index(ui.navigation.compact_highlight)
                    .on_highlight(move |index, _, cx| {
                        if let Some(state) = highlight.upgrade() {
                            state.update(cx, |state, cx| {
                                state.app.ui_state.navigation.compact_highlight = index;
                                cx.notify();
                            });
                        }
                    })
                    .theme(ui.theme.to_select_theme())
                    .on_toggle(move |open, _, cx| {
                        let Some(toggle) = toggle.upgrade() else {
                            return;
                        };
                        toggle.update(cx, |state, cx| {
                            state.app.ui_state.show_studio_menu = open;
                            state.app.ui_state.navigation.compact_highlight =
                                open.then_some(selected);
                            cx.notify();
                        });
                    })
                    .on_change(move |value: &SharedString, _, cx| {
                        if let Ok(index) = value.parse::<usize>()
                            && let Some((screen, _)) = destinations.get(index)
                        {
                            let Some(change) = change.upgrade() else {
                                return;
                            };
                            change.update(cx, |state, cx| {
                                if *screen == Screen::StudioHub {
                                    state.app.enter_studio_mode("CompactNavigation");
                                } else {
                                    state.app.set_screen(*screen, "CompactNavigation");
                                }
                                state.app.ui_state.show_studio_menu = false;
                                cx.notify();
                            });
                        }
                    });
                #[cfg(feature = "dev-api")]
                let select = {
                    use crate::app::dev_api::DevTrackExt;
                    select.dev_track("navigation.compact")
                };
                select
            })
            .child(
                Button::new("compact-preferences", text.preferences)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .theme(ui.theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        settings.update(cx, |state, cx| {
                            state.app.set_screen(Screen::Settings, "CompactPreferences");
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_desktop_studio_hub(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let t = &ui.translations;
        let d = Ds::from_cx(cx);
        let labels = DesktopTranslations::for_language(ui.language).studio_sections;
        let groups = [
            vec![
                (Screen::Studio, t.screen_studio_rack),
                (Screen::PluginGraph, t.screen_studio_full),
                (
                    Screen::Spectrum,
                    SpectrumTranslations::for_language(ui.language).analyzer,
                ),
            ],
            vec![(Screen::Recording, t.screen_recording)],
            vec![
                (Screen::RoomEq, t.screen_room_eq),
                (Screen::HeadphoneEq, t.screen_headphone_eq),
                (Screen::Spinorama, t.screen_spinorama),
            ],
            vec![(Screen::ListeningTest, t.screen_listening_test)],
        ];
        let mut body = div()
            .id("desktop-studio-hub")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section);
        for (index, group) in groups.into_iter().enumerate() {
            let mut row = div().flex().flex_wrap().gap(d.gap);
            let mut count = 0;
            for (screen, label) in group {
                if !ui.release_channel.allows(screen.maturity()) {
                    continue;
                }
                count += 1;
                let entity = self.state.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("studio-tool-{screen:?}")), label)
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .theme(ui.theme.to_button_theme())
                        .on_click_event(move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                state.app.set_screen(screen, "StudioTool");
                                cx.notify();
                            });
                        }),
                );
            }
            if count > 0 {
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(d.gap)
                        .child(Text::section_header(labels[index]))
                        .child(row),
                );
            }
        }
        body.into_any_element()
    }
}
