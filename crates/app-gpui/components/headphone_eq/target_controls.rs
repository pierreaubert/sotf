use crate::app::i18n::{
    EqDiscoveryTranslations, HeadphoneEqTranslations, HeadphoneIdentityTranslations,
};
use crate::components::autoeq::HEADPHONE_TARGET_CURVE_OPTIONS;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Select, SelectOption, Text};

impl PlayerView {
    pub(crate) fn render_headphone_target_controls(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let headphone = &state.app.measurement_state.headphone_eq_state;
        let ui = &state.app.ui_state;
        let text = HeadphoneIdentityTranslations::for_language(ui.language);
        let d = Ds::from_cx(cx);
        let selected = HEADPHONE_TARGET_CURVE_OPTIONS
            .iter()
            .position(|(value, _)| *value == headphone.target_preset)
            .unwrap_or(0);
        let toggle = self.state.downgrade();
        let highlight = self.state.downgrade();
        let change = self.state.downgrade();
        let picker = Select::new("headphone-target")
            .label(text.target)
            .options(
                HEADPHONE_TARGET_CURVE_OPTIONS
                    .iter()
                    .map(|(value, label)| {
                        SelectOption::new(
                            *value,
                            HeadphoneIdentityTranslations::target_label(ui.language, value, label),
                        )
                    })
                    .collect(),
            )
            .selected(headphone.target_preset.clone())
            .is_open(headphone.dropdowns.target_open)
            .highlighted_index(headphone.dropdowns.target_highlight)
            .theme(ui.theme.to_select_theme())
            .on_toggle(move |open, _, cx| {
                if let Some(state) = toggle.upgrade() {
                    state.update(cx, |state, cx| {
                        let dropdowns =
                            &mut state.app.measurement_state.headphone_eq_state.dropdowns;
                        dropdowns.target_open = open;
                        dropdowns.target_highlight = open.then_some(selected);
                        cx.notify();
                    });
                }
            })
            .on_highlight(move |index, _, cx| {
                if let Some(state) = highlight.upgrade() {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .dropdowns
                            .target_highlight = index;
                        cx.notify();
                    });
                }
            })
            .on_change(move |value: &SharedString, _, cx| {
                if HEADPHONE_TARGET_CURVE_OPTIONS
                    .iter()
                    .any(|(target, _)| *target == value.as_ref())
                    && let Some(state) = change.upgrade()
                {
                    state.update(cx, |state, cx| {
                        let headphone = &mut state.app.measurement_state.headphone_eq_state;
                        headphone.model.target_preset = value.to_string();
                        headphone.dropdowns.target_open = false;
                        headphone.dropdowns.target_highlight = None;
                        cx.notify();
                    });
                }
            });
        #[cfg(feature = "dev-api")]
        let picker = {
            use crate::app::dev_api::DevTrackExt;
            picker.dev_track("headphone.target")
        };
        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .min_w_0()
            .child(picker)
            .when(headphone.model.requires_custom_target_path(), |content| {
                content.child(self.render_headphone_custom_target_controls(cx))
            })
    }

    pub(crate) fn render_headphone_custom_target_controls(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let headphone = &state.app.measurement_state.headphone_eq_state;
        let text = HeadphoneEqTranslations::for_language(ui.language);
        let discovery = EqDiscoveryTranslations::for_language(ui.language);
        let d = Ds::from_cx(cx);
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .gap(d.gap)
            .child(Text::label(text.custom_target_curve))
            .child(Text::caption(discovery.custom_target_help))
            .child(Text::body(if headphone.model.has_custom_target_path() {
                headphone.custom_target_path.clone()
            } else {
                text.no_target_curve.to_string()
            }))
            .child(
                Button::new("browse-custom-target", discovery.browse)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(ui.theme.to_button_theme())
                    .on_click_event(
                        cx.listener(|view, _, _, cx| view.browse_headphone_eq_target(cx)),
                    ),
            )
    }
}
