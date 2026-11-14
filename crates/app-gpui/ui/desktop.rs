use super::PlayerView;
use crate::app::Screen;
use crate::app::i18n::{DesktopTranslations, SpectrumTranslations};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Select, SelectOption, TabItem, TabVariant, Tabs, Text,
};

fn studio_tab(id: String, label: &str, icon: IconName, selector: String) -> TabItem {
    TabItem::new(id, label.to_owned()).icon_with_color(move |color| {
        let icon = Icon::new(icon).small().color(color);
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            icon.dev_track(selector.clone()).into_any_element()
        }
        #[cfg(not(feature = "dev-api"))]
        {
            let _ = &selector;
            icon.into_any_element()
        }
    })
}

impl PlayerView {
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the ordered UI workflow and its state updates in one operation"
    )]
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
        let categories: Vec<_> = [
            Screen::Studio,
            Screen::Recording,
            Screen::RoomEq,
            Screen::ListeningTest,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, fallback)| {
            let target = ui
                .navigation
                .studio_target(index)
                .filter(|target| ui.release_channel.allows(target.maturity()))
                .unwrap_or(fallback);
            (index, target)
        })
        .filter(|(_, target)| ui.release_channel.allows(target.maturity()))
        .collect();
        let category_selected = categories
            .iter()
            .position(|(index, _)| *index == selected)
            .unwrap_or(0);
        let state_for_category = self.state.downgrade();
        let row = Tabs::new("studio-categories")
            .tabs(
                categories
                    .iter()
                    .map(|(index, _)| {
                        let icon = [
                            IconName::SlidersHorizontal,
                            IconName::AudioWaveform,
                            IconName::AudioWaveform,
                            IconName::Headphones,
                        ][*index];
                        studio_tab(
                            format!("studio-category-{index}"),
                            labels[*index],
                            icon,
                            format!("studio.category.{index}"),
                        )
                    })
                    .collect(),
            )
            .selected_index(category_selected)
            .variant(TabVariant::Enclosed)
            .theme(gpui_ui_kit::TabsTheme {
                selected_bg: ui.theme.accent,
                ..ui.theme.to_tabs_theme()
            })
            .aria_label(ui.translations.screen_studio)
            .on_change(move |index, _, cx| {
                if let Some((_, target)) = categories.get(index)
                    && let Some(state) = state_for_category.upgrade()
                {
                    state.update(cx, |state, cx| {
                        state.app.set_screen(*target, "StudioCategory");
                        cx.notify();
                    });
                }
            });
        let row = div().flex().justify_center().p(d.pad_y).child(row);
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
        let tools: Vec<_> = tools
            .into_iter()
            .filter(|(target, _)| ui.release_channel.allows(target.maturity()))
            .collect();
        let tool_selected = tools
            .iter()
            .position(|(target, _)| *target == screen)
            .unwrap_or(0);
        let state_for_tool = self.state.downgrade();
        let subtools = Tabs::new("studio-subtools")
            .tabs(
                tools
                    .iter()
                    .map(|(target, label)| {
                        let icon = match target {
                            Screen::Spectrum => IconName::AudioWaveform,
                            Screen::HeadphoneEq => IconName::Headphones,
                            _ => IconName::SlidersHorizontal,
                        };
                        studio_tab(
                            format!("studio-subtool-{target:?}"),
                            label,
                            icon,
                            format!("studio.subtool.{target:?}"),
                        )
                    })
                    .collect(),
            )
            .selected_index(tool_selected)
            .variant(TabVariant::Underline)
            .theme(gpui_ui_kit::TabsTheme {
                text_selected: ui.theme.accent,
                icon_selected: Some(ui.theme.accent),
                ..ui.theme.to_tabs_theme()
            })
            .on_change(move |index, _, cx| {
                if let Some((target, _)) = tools.get(index)
                    && let Some(state) = state_for_tool.upgrade()
                {
                    state.update(cx, |state, cx| {
                        state.app.set_screen(*target, "StudioSubtool");
                        cx.notify();
                    });
                }
            });
        let subtools = div()
            .flex()
            .justify_center()
            .px(d.pad_y)
            .pb(d.pad_y)
            .child(subtools);
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

    fn desktop_destinations(&self, cx: &Context<Self>) -> Vec<(Screen, &'static str, IconName)> {
        let state = self.state.read(cx);
        let t = &state.app.ui_state.translations;
        [
            (Screen::Home, t.screen_home, IconName::Home),
            (Screen::Library, t.screen_library, IconName::Library),
            (Screen::NowPlaying, t.screen_now_playing, IconName::Music),
            (Screen::Queue, t.screen_queue, IconName::ListMusic),
            (Screen::Playlists, t.screen_playlists, IconName::Album),
            (Screen::Streams, t.screen_streams, IconName::Radio),
            (
                Screen::StudioHub,
                t.screen_studio,
                IconName::SlidersHorizontal,
            ),
        ]
        .into_iter()
        .filter(|(screen, _, _)| state.app.ui_state.release_channel.allows(screen.maturity()))
        .collect()
    }

    pub(super) fn render_app_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        self.render_legacy_app_sidebar(cx)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
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
            .position(|(screen, _, _)| *screen == current)
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
                            .map(|(index, (_, label, _))| {
                                SelectOption::new(index.to_string(), *label)
                            })
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
                            && let Some((screen, _, _)) = destinations.get(index)
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
                    })
                    .map(|button| {
                        #[cfg(feature = "dev-api")]
                        {
                            use crate::app::dev_api::DevTrackExt;
                            button.dev_track("navigation.preferences")
                        }
                        #[cfg(not(feature = "dev-api"))]
                        {
                            button
                        }
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
