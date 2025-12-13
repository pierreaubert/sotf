// Screen rendering modules
//
mod headphone_eq;
pub mod library;
pub mod queue;
pub mod recording;
mod room_eq;
mod speaker_diy;
mod speaker_eq;

mod conf;

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, StackSpacing, Text, TextSize, TextWeight};

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let active_tab = state.app.active_settings_tab;
        let translations = state.app.translations.clone();

        // Content area based on active tab
        let content = match active_tab {
            crate::app::SettingsTab::Library => {
                self.render_library_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Appearance => self
                .render_appearance_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::AudioDevice => self
                .render_audio_device_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Plugins => self.render_plugins_screen(cx).into_any_element(),
            crate::app::SettingsTab::RoomEQ => {
                self.render_roomeq_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Headphone => self
                .render_headphone_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Spinorama => self
                .render_spinorama_settings_content(cx)
                .into_any_element(),
        };

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(
                // Tab Header
                div()
                    .w_full()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .px_4()
                    .pt_2()
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(self.render_settings_tab(
                                translations.settings_tab_library,
                                crate::app::SettingsTab::Library,
                                &theme,
                                cx,
                            ))
                            .child(self.render_settings_tab(
                                translations.settings_tab_appearance,
                                crate::app::SettingsTab::Appearance,
                                &theme,
                                cx,
                            ))
                            .child(self.render_settings_tab(
                                translations.settings_tab_audio_device,
                                crate::app::SettingsTab::AudioDevice,
                                &theme,
                                cx,
                            ))
                            .child(self.render_settings_tab(
                                translations.settings_tab_plugins,
                                crate::app::SettingsTab::Plugins,
                                &theme,
                                cx,
                            ))
                            .child(self.render_recording_tab(
                                translations.settings_tab_recording,
                                &theme,
                                cx,
                            ))
                            .child(self.render_room_eq_tab(
                                translations.settings_tab_room_eq,
                                &theme,
                                cx,
                            ))
                            .child(self.render_settings_tab(
                                translations.settings_tab_headphone,
                                crate::app::SettingsTab::Headphone,
                                &theme,
                                cx,
                            ))
                            .child(self.render_settings_tab(
                                translations.settings_tab_spinorama,
                                crate::app::SettingsTab::Spinorama,
                                &theme,
                                cx,
                            )),
                    ),
            )
            // Content
            .child(
                div()
                    // .overflow_y_scroll()
                    .flex_1()
                    .p_4()
                    .child(content),
            )
    }

    /// Render a special tab that navigates to the Recording screen
    fn render_recording_tab(
        &self,
        label: &'static str,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();

        div()
            .px_3()
            .py_2()
            .cursor_pointer()
            .rounded_t_md()
            .bg(theme.surface)
            .text_color(theme.text_secondary)
            .hover(|s| s.bg(theme.surface_hover).text_color(theme.text_primary))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.last_screen = state.app.current_screen;
                        state.app.current_screen = crate::app::Screen::Recording;
                    });
                    cx.notify();
                }),
            )
            .child(
                Text::new(label)
                    .size(TextSize::Sm)
                    .weight(TextWeight::Normal)
                    .color(theme.text_secondary),
            )
    }

    /// Render a special tab that navigates to the Room EQ screen
    fn render_room_eq_tab(
        &self,
        label: &'static str,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();

        div()
            .px_3()
            .py_2()
            .cursor_pointer()
            .rounded_t_md()
            .bg(theme.surface)
            .text_color(theme.text_secondary)
            .hover(|s| s.bg(theme.surface_hover).text_color(theme.text_primary))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.last_screen = state.app.current_screen;
                        state.app.current_screen = crate::app::Screen::RoomEq;
                    });
                    cx.notify();
                }),
            )
            .child(
                Text::new(label)
                    .size(TextSize::Sm)
                    .weight(TextWeight::Normal)
                    .color(theme.text_secondary),
            )
    }

    fn render_settings_tab(
        &self,
        label: &str,
        tab: crate::app::SettingsTab,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_active = state.app.active_settings_tab == tab;
        let theme = theme.clone();

        div()
            .px_3()
            .py_2()
            .cursor_pointer()
            .rounded_t_md()
            .when(is_active, |style| {
                style
                    .bg(theme.background)
                    .border_t_1()
                    .border_l_1()
                    .border_r_1()
                    .border_color(theme.border)
                    .pb_2() // Overlap bottom border
                    .mb_neg_px() // Shift down to cover border
            })
            .when(!is_active, |style| {
                style
                    .bg(theme.surface)
                    .text_color(theme.text_secondary)
                    .hover(|s| s.bg(theme.surface_hover).text_color(theme.text_primary))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.active_settings_tab = tab;
                    });
                    cx.notify();
                }),
            )
            .child(
                Text::new(label.to_string())
                    .size(TextSize::Sm)
                    .weight(if is_active {
                        TextWeight::Bold
                    } else {
                        TextWeight::Normal
                    })
                    .color(if is_active {
                        theme.accent
                    } else {
                        theme.text_secondary
                    }),
            )
    }
}
