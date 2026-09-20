use super::search::{focus_settings_choice_relative, settings_choice_focus_handle};
#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::i18n::{
    DialogTranslations, FeatureAvailabilityTranslations, SettingsSurfaceTranslations,
};
use crate::app::types::{PreferencesSetting, Screen};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AccessibilityExt, AccessibilityNode, AriaProps, AriaRole, AriaState, Button, ButtonSize,
    ButtonVariant, Text,
};
use sotf_audio_player::{PluginType, ReleaseChannel};

struct FeatureRow {
    id: String,
    name: &'static str,
    maturity: ReleaseChannel,
}

impl PlayerView {
    fn select_feature_availability(
        &mut self,
        channel: ReleaseChannel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            state.app.set_release_channel(channel);
            let layout = state.layout.read(cx);
            if let Err(error) = state.app.save_config(layout) {
                let copy =
                    FeatureAvailabilityTranslations::for_language(state.app.ui_state.language);
                state.app.ui_state.toast_message =
                    Some(crate::app::ToastMessage::error(copy.save_error(error)));
            }
        });
        if let Some(focus) =
            self.preference_focus_handle(PreferencesSetting::FeatureAvailability, cx)
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn render_release_channel_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let current = state.app.ui_state.release_channel;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let language = state.app.ui_state.language;
        let copy = FeatureAvailabilityTranslations::for_language(language);
        let text = SettingsSurfaceTranslations::for_language(language);
        let compact = crate::ui::resolve_sizing_context(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        )
        .window_width_rems
            < 52.0;
        let choices = [
            (ReleaseChannel::Prod, text.stable),
            (ReleaseChannel::Beta, text.beta),
            (ReleaseChannel::Alpha, text.alpha),
        ];
        let controls = choices
            .into_iter()
            .map(|(channel, label)| {
                let selected = current == channel;
                let id = ElementId::from(SharedString::from(format!(
                    "select-channel-{}",
                    channel.name()
                )));
                let focus = if selected {
                    self.preference_focus_handle(PreferencesSetting::FeatureAvailability, cx)
                        .unwrap_or_else(|| settings_choice_focus_handle(&id, cx))
                } else {
                    settings_choice_focus_handle(&id, cx)
                };
                let accessible_label = format!("{label} {}", copy.channel);
                cx.register_accessible(AccessibilityNode {
                    element_id: id.clone(),
                    label: accessible_label.clone().into(),
                    props: AriaProps::with_role(AriaRole::Button)
                        .maybe_state(selected, AriaState::Pressed(true)),
                });
                let button = Button::new(id, label)
                    .aria_label(accessible_label)
                    .selected(selected)
                    .variant(if selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .build()
                    .text_size(d.text_sm)
                    .px(d.pad_x)
                    .py(d.pad_y_half)
                    .track_focus(&focus)
                    .track_focus_element(&focus)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_feature_availability(channel, window, cx)
                    }))
                    .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            view.select_feature_availability(channel, window, cx);
                            cx.stop_propagation();
                        }
                    }));
                #[cfg(feature = "dev-api")]
                let button = button.dev_track_with_state(
                    format!("settings.release-channel.{}", channel.name()),
                    DevElementState::default().selected(selected),
                );
                (button.into_any_element(), focus)
            })
            .collect::<Vec<_>>();
        let handles = controls
            .iter()
            .map(|(_, focus)| focus.clone())
            .collect::<Vec<_>>();
        let controls = div()
            .flex()
            .flex_wrap()
            .gap(d.gap)
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
            .children(controls.into_iter().map(|(button, _)| button));
        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .w_full()
            .min_w_0()
            .child(Text::section_header(
                translations.settings_release_channel_title,
            ))
            .child(Text::body(
                translations.settings_release_channel_description,
            ))
            .child(self.preference_control(PreferencesSetting::FeatureAvailability, controls, cx))
            .child(Text::section_header(copy.preview))
            .child(Text::body(copy.summary(current)))
            .child(self.render_feature_table(language, copy, text, &theme, d, compact))
    }

    fn render_feature_table(
        &self,
        language: crate::app::i18n::Language,
        copy: FeatureAvailabilityTranslations,
        text: SettingsSurfaceTranslations,
        theme: &crate::theme::Theme,
        d: Ds,
        compact: bool,
    ) -> impl IntoElement {
        let features = Screen::all()
            .iter()
            .copied()
            .filter(|screen| {
                !matches!(
                    screen,
                    Screen::HomeShelf | Screen::SettingsDetail | Screen::StudioHub
                )
            })
            .map(|screen| FeatureRow {
                id: format!("screen.{screen:?}"),
                name: DialogTranslations::for_language(language).screen_name(screen),
                maturity: screen.maturity(),
            })
            .collect::<Vec<_>>();
        let plugins = PluginType::all()
            .into_iter()
            .map(|plugin| FeatureRow {
                id: format!("plugin.{plugin:?}"),
                name: plugin.name(),
                maturity: plugin.maturity(),
            })
            .collect::<Vec<_>>();
        let channels = [
            (ReleaseChannel::Prod, text.stable),
            (ReleaseChannel::Beta, text.beta),
            (ReleaseChannel::Alpha, text.alpha),
        ];
        let mut table = div().flex().flex_col().w_full().min_w_0().gap(d.gap);
        if !compact {
            table = table.child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(theme.border)
                    .pb(d.gap)
                    .child(div().flex_1().min_w_0())
                    .child(div().flex().w(rems(15.0)).children(channels.iter().map(
                        |(_, label)| div().flex_1().text_center().child(Text::label(*label)),
                    ))),
            );
        }
        for (label, rows) in [(copy.screens, features), (copy.plugins, plugins)] {
            table = table.child(Text::section_header(label));
            for row in rows {
                let mut cells = div()
                    .flex()
                    .min_w_0()
                    .when(compact, |cells| cells.w_full())
                    .when(!compact, |cells| cells.w(rems(15.0)).flex_shrink_0());
                for (channel, label) in channels {
                    let available = channel.allows(row.maturity);
                    let selector =
                        format!("preferences.availability.{}.{}", row.id, channel.name());
                    let cell = div()
                        .id(SharedString::from(selector.clone()))
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .text_size(d.text_sm)
                        .text_color(if available {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                        .aria_label(format!(
                            "{} · {label}: {}",
                            row.name,
                            if available {
                                copy.available
                            } else {
                                copy.unavailable
                            }
                        ))
                        .when(compact, |cell| cell.child(Text::caption(label)))
                        .child(if available { "✓" } else { "—" });
                    cells = cells.child(track_availability(cell, selector, available));
                }
                table = table.child(
                    div()
                        .flex()
                        .gap(d.grid)
                        .py(d.half_grid)
                        .min_w_0()
                        .when(compact, |row| row.flex_col())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Text::label(row.name).color(theme.text_primary)),
                        )
                        .child(cells),
                );
            }
        }
        table
    }
}

fn track_availability(cell: Stateful<Div>, selector: String, available: bool) -> AnyElement {
    #[cfg(feature = "dev-api")]
    {
        cell.dev_track_with_state(selector, DevElementState::default().enabled(available))
            .into_any_element()
    }
    #[cfg(not(feature = "dev-api"))]
    {
        let _ = (selector, available);
        cell.into_any_element()
    }
}
