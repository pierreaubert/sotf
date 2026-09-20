#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::HeadphoneIdentityTranslations;
use crate::app::types::headphone_eq::HeadphoneMeasurementSource;
use crate::components::autoeq::HEADPHONE_TARGET_CURVE_OPTIONS;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonVariant, Text};

impl PlayerView {
    pub(super) fn render_headphone_measurement_identity(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let text = HeadphoneIdentityTranslations::for_language(state.app.ui_state.language);
        let headphone = &state.app.measurement_state.headphone_eq_state;
        let expanded = headphone
            .expanded_sections
            .iter()
            .any(|section| section == "measurement-identity");
        let button = Button::new("headphone-measurement-identity", text.title)
            .icon_left(if expanded { "▾" } else { "▸" })
            .selected(expanded)
            .variant(ButtonVariant::Ghost)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(|view, _, _, cx| {
                view.state.update(cx, |state, _| {
                    let sections = &mut state
                        .app
                        .measurement_state
                        .headphone_eq_state
                        .expanded_sections;
                    if sections
                        .iter()
                        .any(|section| section == "measurement-identity")
                    {
                        sections.retain(|section| section != "measurement-identity");
                    } else {
                        sections.push("measurement-identity".into());
                    }
                });
                cx.notify();
            }));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track("headphone.measurement-identity");
        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .w_full()
            .min_w_0()
            .child(button)
            .when(expanded, |content| {
                let preview = headphone.active_file_preview();
                let identity = |key| preview.and_then(|preview| preview.identity_field(key));
                let model = if headphone.measurement_source == HeadphoneMeasurementSource::Spinorama
                {
                    headphone
                        .selected_headphone
                        .as_deref()
                        .unwrap_or(text.unknown)
                } else {
                    identity("model").unwrap_or(text.unknown)
                };
                let rig = [identity("rig"), identity("sample")]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let rig = if rig.is_empty() {
                    text.unknown.to_string()
                } else {
                    rig
                };
                let source = if headphone.measurement_path.is_empty() {
                    text.unknown
                } else {
                    &headphone.measurement_path
                };
                let target = if headphone.target_preset == "custom" {
                    if headphone.custom_target_path.is_empty() {
                        text.unknown
                    } else {
                        &headphone.custom_target_path
                    }
                } else {
                    HEADPHONE_TARGET_CURVE_OPTIONS
                        .iter()
                        .find(|(value, _)| *value == headphone.target_preset)
                        .map(|(value, label)| {
                            HeadphoneIdentityTranslations::target_label(
                                state.app.ui_state.language,
                                value,
                                label,
                            )
                        })
                        .unwrap_or(&headphone.target_preset)
                };
                let bounds = headphone
                    .measurement_frequency_bounds()
                    .map(|(low, high)| format!("{low:.1}–{high:.1} Hz"))
                    .unwrap_or_else(|| text.unknown.to_string());
                content
                    .children(
                        [
                            ("model", text.model, model.to_string()),
                            ("source", text.source, source.to_string()),
                            ("bounds", text.bounds, bounds),
                            ("target", text.target, target.to_string()),
                            ("rig", text.rig, rig),
                            (
                                "compensation",
                                text.compensation,
                                identity("compensation").unwrap_or(text.unknown).to_string(),
                            ),
                        ]
                        .into_iter()
                        .map(|(_key, label, value)| {
                            let row = div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(Text::label(label))
                                .child(Text::body(value));
                            #[cfg(feature = "dev-api")]
                            let row = row.dev_track(format!("headphone.identity.{_key}"));
                            row
                        }),
                    )
                    .child(Text::caption(text.compatibility).color(theme.text_secondary))
            })
    }
}
