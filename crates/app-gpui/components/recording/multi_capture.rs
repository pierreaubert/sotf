//! Native controls for the shared simultaneous capture workflow.

use crate::app::i18n::MultiCaptureTranslations;
use crate::app::state::measurement::{CaptureAction, CapturePath};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonVariant, Heading, Input, StackSpacing, Text, VStack};
use sotf_audio_player::ui_models::capture::CaptureStage;
use sotf_audio_player::ui_models::room_eq::RoomEqViewEvent;

macro_rules! track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    pub(crate) fn render_multi_capture_launcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let text =
            MultiCaptureTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let busy = self
            .state
            .read(cx)
            .app
            .measurement_state
            .recording_state
            .workflow_is_busy();
        let view = cx.weak_entity();
        track!(
            Button::new("open-multi-capture", text.launcher)
                .aria_label(text.launcher_aria)
                .variant(ButtonVariant::Secondary)
                .disabled(busy)
                .on_click(move |window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            let measurement = &mut state.app.measurement_state;
                            measurement
                                .multi_capture
                                .open(measurement.recording_state.workflow_is_busy());
                        });
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    });
                }),
            "recording.multi_capture.open"
        )
    }

    pub(crate) fn render_multi_capture_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let text = MultiCaptureTranslations::for_language(state.app.ui_state.language);
        let panel = &state.app.measurement_state.multi_capture;
        let workflow = &panel.workflow;
        let busy = workflow.is_busy();
        let view = cx.weak_entity();
        let fields = [
            (
                CapturePath::Plan,
                "capture-plan",
                text.plan,
                panel.plan_path.clone(),
            ),
            (
                CapturePath::Raw,
                "capture-raw",
                text.raw,
                panel.raw_directory.clone(),
            ),
            (
                CapturePath::Processed,
                "capture-processed",
                text.processed,
                panel.processed_directory.clone(),
            ),
        ];
        let inputs = fields
            .into_iter()
            .map(|(field, id, label, value)| {
                let view = view.clone();
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Text::label(label))
                    .child(track!(
                        Input::new(id)
                            .aria_label(label)
                            .value(value)
                            .disabled(busy)
                            .on_text_change(move |value, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        let panel = &mut state.app.measurement_state.multi_capture;
                                        if let Err(error) =
                                            panel.edit_path(field, value.to_string())
                                        {
                                            panel.workflow.message = error;
                                        }
                                    });
                                    cx.notify();
                                });
                            }),
                        id
                    ))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let actions = [
            (
                CaptureAction::LoadPlan,
                "capture-validate",
                text.load,
                !busy && !panel.plan_path.trim().is_empty(),
            ),
            (
                CaptureAction::Record,
                "capture-record",
                text.record,
                workflow.can_record() && !panel.raw_directory.trim().is_empty(),
            ),
            (
                CaptureAction::Cancel,
                "capture-cancel",
                text.cancel,
                workflow.stage == CaptureStage::Recording,
            ),
            (
                CaptureAction::Process,
                "capture-process",
                text.process,
                !busy
                    && !panel.raw_directory.trim().is_empty()
                    && !panel.processed_directory.trim().is_empty(),
            ),
            (
                CaptureAction::Import,
                "capture-import",
                text.import,
                workflow.can_import(),
            ),
        ];
        let buttons = actions
            .into_iter()
            .map(|(action, id, label, enabled)| {
                let view = view.clone();
                track!(
                    Button::new(id, label)
                        .aria_label(label)
                        .disabled(!enabled)
                        .variant(if matches!(action, CaptureAction::Record) {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .on_click(move |_, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let measurement = &mut state.app.measurement_state;
                                    if matches!(action, CaptureAction::Record)
                                        && measurement.recording_state.workflow_is_busy()
                                    {
                                        measurement.multi_capture.workflow.message =
                                            text.legacy_busy.into();
                                        return;
                                    }
                                    match measurement.multi_capture.execute(action) {
                                        Ok(Some(channels)) => {
                                            measurement
                                                .room_eq_state
                                                .model
                                                .apply(RoomEqViewEvent::LoadMeasurements(channels));
                                            measurement.multi_capture.workflow.message =
                                                text.imported.into();
                                        }
                                        Ok(None) => {}
                                        Err(error) => {
                                            measurement.multi_capture.workflow.message = error
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }),
                    id
                )
                .into_any_element()
            })
            .collect::<Vec<_>>();
        let status = if workflow.message.is_empty() {
            text.initial_hint
        } else {
            &workflow.message
        };
        let review = workflow
            .review_lines
            .iter()
            .map(|line| Text::body(line.clone()).into_any_element())
            .collect::<Vec<_>>();
        let qa = workflow
            .qa_lines
            .iter()
            .map(|line| Text::body(line.clone()).into_any_element())
            .collect::<Vec<_>>();
        let manifest = workflow
            .recording_manifest
            .as_ref()
            .map(|path| path.display().to_string());
        let stage = text.stage(workflow.stage);
        let back_view = view;

        div()
            .id("multi-capture-content")
            .size_full()
            .overflow_y_scroll()
            .px(d.pad_x)
            .py(d.pad_y)
            .child(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(d.gap)
                            .child(Heading::h4(text.title))
                            .child(track!(
                                Button::new("capture-back", text.back)
                                    .disabled(busy)
                                    .aria_label(text.back_aria)
                                    .on_click(move |window, cx| {
                                        let _ = back_view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                state.app.measurement_state.multi_capture.close();
                                            });
                                            this.focus_handle.focus(window, cx);
                                            cx.notify();
                                        });
                                    }),
                                "recording.multi_capture.back"
                            )),
                    )
                    .child(Text::caption(text.overview))
                    .children(inputs)
                    .child(Text::caption(text.setup_hint))
                    .child(div().flex().flex_wrap().gap(d.gap).children(buttons))
                    .child(Text::section_header(stage))
                    .child(Text::body(status.to_owned()))
                    .child(Text::caption(text.validation_hint))
                    .when(!review.is_empty(), |content| {
                        content
                            .child(Heading::h4(text.loaded_plan))
                            .children(review)
                    })
                    .when(!qa.is_empty(), |content| {
                        content.child(Heading::h4(text.qa)).children(qa)
                    })
                    .when_some(manifest, |content, path| {
                        content.child(Text::body(format!("{}: {path}", text.manifest)))
                    }),
            )
    }
}
