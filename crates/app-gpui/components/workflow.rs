use crate::app::i18n::{
    EqDiscoveryTranslations, HeadphoneEqTranslations, RecordingTranslations,
    RoomEqWorkflowTranslations, TakeReviewTranslations, WizardNavigationTranslations,
};
use crate::app::types::{
    ChannelRecordingState, HeadphoneEqStep, RecordingStep, RoomEqStep, Screen, SpinoramaStep,
};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, rems};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Select, SelectOption, Text};

fn recording_config_confirmation_labels(
    language: crate::app::i18n::Language,
) -> (&'static str, &'static str, &'static str) {
    use crate::app::i18n::Language;
    match language {
        Language::French => (
            "Les changements de type de signal, plage, fréquence, appareil ou routage effacent les prises existantes. Un changement de niveau ne modifie que les prochaines prises; les niveaux déjà enregistrés restent associés à leurs résultats.",
            "Modifier les réglages",
            "Garder les prises",
        ),
        Language::German => (
            "Änderungen an Signaltyp, Frequenzbereich, Abtastrate, Gerät oder Kanalzuordnung löschen vorhandene Aufnahmen. Eine Pegeländerung gilt nur für neue Aufnahmen; bestehende Pegelnachweise bleiben erhalten.",
            "Einstellungen ändern",
            "Aufnahmen behalten",
        ),
        Language::Spanish => (
            "Cambiar el tipo de señal, el rango, la frecuencia, el dispositivo o el enrutamiento elimina las tomas existentes. Cambiar el nivel solo afecta a las próximas tomas; cada resultado conserva su nivel registrado.",
            "Cambiar ajustes",
            "Conservar tomas",
        ),
        _ => (
            "Changing signal type, range, sample rate, device, or channel routing clears existing takes. A level change affects only new takes; existing results keep their recorded level evidence.",
            "Edit settings",
            "Keep takes",
        ),
    }
}

impl PlayerView {
    pub(crate) fn render_workflow_result_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let m = &state.app.measurement_state;
        let text = crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        let message = match state.app.ui_state.current_screen {
            Screen::HeadphoneEq
                if m.headphone_eq_state.result.is_some()
                    && !m.headphone_eq_state.result_is_current() =>
            {
                Some(text.stale_result)
            }
            Screen::Spinorama
                if m.spinorama_eq_state.result.is_some()
                    && !m.spinorama_eq_state.result_is_current() =>
            {
                Some(text.stale_result)
            }
            Screen::RoomEq
                if m.room_eq_state.dsp_output.is_some() && !m.room_eq_state.result_is_current() =>
            {
                Some(text.stale_result)
            }
            Screen::Recording if !m.recording_state.capture_inputs_are_current() => {
                Some(text.recording_incomplete)
            }
            _ => None,
        };
        let delivery = match state.app.ui_state.current_screen {
            Screen::HeadphoneEq if m.headphone_eq_state.result.is_some() => Some((
                &m.headphone_eq_state.delivery,
                m.headphone_eq_state.result_is_current(),
            )),
            Screen::Spinorama if m.spinorama_eq_state.result.is_some() => Some((
                &m.spinorama_eq_state.delivery,
                m.spinorama_eq_state.result_is_current(),
            )),
            Screen::RoomEq if m.room_eq_state.dsp_output.is_some() => Some((
                &m.room_eq_state.delivery,
                m.room_eq_state.result_is_current(),
            )),
            _ => None,
        };
        let d = Ds::from_cx(cx);
        div()
            .when_some(delivery, |element, (delivery, current)| {
                use sotf_audio_player::ui_models::correction_delivery::CorrectionApplicationStatus;
                let language = state.app.ui_state.language;
                let export =
                    crate::app::i18n::CorrectionDeliveryTranslations::for_language(language);
                let application =
                    crate::app::i18n::CorrectionApplicationTranslations::for_language(language);
                let application_label =
                    match state.app.correction_application_status(delivery, current) {
                        CorrectionApplicationStatus::NotApplied => application.not_applied,
                        CorrectionApplicationStatus::Pending => application.pending,
                        CorrectionApplicationStatus::Applied => application.applied,
                        CorrectionApplicationStatus::PreviousResult => application.previous,
                        CorrectionApplicationStatus::ChangedGraph => application.changed,
                        CorrectionApplicationStatus::Failed => application.failed,
                    };
                let export_label = match delivery.last_export() {
                    None => export.not_exported,
                    Some(_) if delivery.current_result_exported(current) => export.current_export,
                    Some(_) => export.previous_export,
                };
                let status = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p(d.pad_x)
                    .when(current, |element| {
                        element.child(Text::body(export.calculated))
                    })
                    .child(Text::body(application_label))
                    .child(Text::body(export_label));
                #[cfg(feature = "dev-api")]
                let status = {
                    use crate::app::dev_api::DevTrackExt;
                    status.dev_track("workflow.result_status")
                };
                element.child(status)
            })
            .when_some(message, |element, message| {
                element
                    .p(d.pad_x)
                    .child(Text::body(message).color(state.app.ui_state.theme.warning))
            })
            .into_any_element()
    }

    pub(crate) fn workflow_is_compact(&self, cx: &Context<Self>) -> bool {
        let ui = &self.state.read(cx).app.ui_state;
        crate::ui::resolve_sizing_context(
            ui.window_width,
            ui.window_height,
            ui.font_scale,
            ui.min_font_size_px,
            ui.max_font_size_px,
        )
        .desktop_content_width_rems(ui.primary_nav_collapsed)
            < 64.0
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep the declarative layout and its interaction handlers together"
    )]
    pub(crate) fn render_workflow_shell(
        &self,
        content_id: &'static str,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let nav = WizardNavigationTranslations::for_language(ui.language);
        let m = &state.app.measurement_state;
        let (title, labels, current, busy): (&str, Vec<&str>, usize, bool) = match ui.current_screen
        {
            Screen::Recording => {
                let t = RecordingTranslations::for_language(ui.language);
                let steps = RecordingStep::all();
                (
                    ui.translations.screen_recording,
                    steps.iter().map(|step| t.step_label(*step)).collect(),
                    steps
                        .iter()
                        .position(|step| *step == m.recording_state.step)
                        .unwrap_or(0),
                    m.recording_state.workflow_is_busy(),
                )
            }
            Screen::RoomEq => {
                let t = RoomEqWorkflowTranslations::for_language(ui.language);
                (
                    ui.translations.screen_room_eq,
                    RoomEqStep::all()
                        .iter()
                        .map(|step| t.step_label(*step))
                        .collect(),
                    m.room_eq_state.step.index(),
                    m.room_eq_state.is_optimizing(),
                )
            }
            Screen::HeadphoneEq => {
                let t = HeadphoneEqTranslations::for_language(ui.language);
                let current = match m.headphone_eq_state.step {
                    HeadphoneEqStep::MeasurementTarget => 0,
                    HeadphoneEqStep::Optimization => 1,
                    HeadphoneEqStep::Listen => 2,
                    HeadphoneEqStep::Export => 3,
                };
                (
                    t.title,
                    vec![
                        t.measurement_step,
                        t.optimization_step,
                        t.listen_step,
                        t.export,
                    ],
                    current,
                    m.headphone_eq_state.is_optimizing(),
                )
            }
            Screen::Spinorama => {
                let t = EqDiscoveryTranslations::for_language(ui.language);
                (
                    ui.translations.screen_spinorama,
                    [
                        SpinoramaStep::SelectSpeaker,
                        SpinoramaStep::Configure,
                        SpinoramaStep::Review,
                        SpinoramaStep::Export,
                    ]
                    .into_iter()
                    .map(|step| t.spinorama_step_label(step))
                    .collect(),
                    m.spinorama_eq_state.step.index(),
                    m.spinorama_eq_state.is_optimizing(),
                )
            }
            _ => return div().into_any_element(),
        };
        let can_next = state.app.can_advance_workflow_step();
        let theme = ui.theme.clone();
        let open = m.step_menu_open;
        let compact = self.workflow_is_compact(cx);
        let screen = ui.current_screen;
        let recording_action = (screen == Screen::Recording).then(|| {
            let recording = &m.recording_state;
            match recording.step {
                RecordingStep::Capture => {
                    let target = recording
                        .channel_recordings
                        .iter()
                        .position(|take| take.state == ChannelRecordingState::ReviewNeeded)
                        .or_else(|| {
                            recording
                                .channel_recordings
                                .iter()
                                .position(|take| take.state == ChannelRecordingState::Done)
                        });
                    target.map(|index| (RecordingStep::Capture, Some(index)))
                }
                RecordingStep::Saving => Some((RecordingStep::Saving, None)),
                _ => None,
            }
        });
        let recording_action = recording_action.flatten();
        let can_retake = recording_action.is_some_and(|(step, index)| {
            step == RecordingStep::Capture
                && index.is_some()
                && !busy
                && m.recording_state.capture_inputs_are_current()
        });
        let review_target = (screen == Screen::Recording
            && m.recording_state.step == RecordingStep::Capture)
            .then(|| {
                m.recording_state
                    .channel_recordings
                    .iter()
                    .position(|take| take.state == ChannelRecordingState::ReviewNeeded)
            })
            .flatten();
        let can_accept_review =
            review_target.is_some() && !busy && m.recording_state.capture_inputs_are_current();
        let can_save = recording_action.is_some_and(|(step, _)| {
            step == RecordingStep::Saving
                && m.recording_state
                    .channel_recordings
                    .iter()
                    .any(|take| take.state == ChannelRecordingState::Done)
                && (m.recording_state.recording_directory.is_some()
                    || m.recording_state.named_recording_directory().is_some())
                && !busy
                && m.recording_state.session_metadata_is_valid()
        });
        let config_change_confirmation =
            screen == Screen::Recording && m.recording_state.config_change_confirmation_pending;
        let (config_warning, config_confirm_label, config_cancel_label) =
            recording_config_confirmation_labels(ui.language);
        let take_review_translations = TakeReviewTranslations::for_language(ui.language);
        let save_label = ui.translations.recording_save_all;
        let change = self.state.downgrade();
        let toggle = self.state.downgrade();
        let back = self.state.clone();
        let next = self.state.clone();
        let retake_view = cx.entity().clone();
        let accept_view = cx.entity().clone();
        let save_view = cx.entity().clone();
        let config_confirm = self.state.clone();
        let config_cancel = self.state.clone();
        let next_label = if compact && current + 1 < labels.len() {
            crate::app::i18n::PhoneTranslations::for_language(ui.language)
                .next
                .to_string()
        } else {
            crate::components::wizard_continue_label(ui.language, labels.get(current + 1).copied())
        };
        let count = labels.len();
        let selector = Select::new("workflow-step")
            .label(format!("{title} · {} / {count}", current + 1))
            .options(
                labels
                    .iter()
                    .enumerate()
                    .map(|(index, label)| {
                        SelectOption::new(index.to_string(), format!("{}. {label}", index + 1))
                            .disabled(index > current && (index != current + 1 || !can_next))
                    })
                    .collect(),
            )
            .selected(current.to_string())
            .is_open(open)
            .disabled(busy)
            .theme(theme.to_select_theme())
            .on_toggle(move |open, _, cx| {
                if let Some(toggle) = toggle.upgrade() {
                    toggle.update(cx, |state, cx| {
                        state.app.measurement_state.step_menu_open = open;
                        cx.notify();
                    });
                }
            })
            .on_change(move |value: &SharedString, _, cx| {
                if let Ok(target) = value.parse::<usize>()
                    && let Some(change) = change.upgrade()
                {
                    change.update(cx, |state, cx| {
                        if state.app.ui_state.current_screen == screen {
                            state.app.select_workflow_step(target);
                            state.app.measurement_state.step_menu_open = false;
                        }
                        cx.notify();
                    });
                }
            });
        let rail = div()
            .id("workflow-step-rail")
            .flex_none()
            .w(rems(15.0))
            .min_h_0()
            .overflow_y_scroll()
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.gap)
            .border_r_1()
            .border_color(theme.border)
            .child(Text::section_header(title.to_string()))
            .children(labels.iter().enumerate().map(|(index, label)| {
                let target = self.state.clone();
                Button::new(
                    SharedString::from(format!("workflow-step-{index}")),
                    format!("{}. {label}", index + 1),
                )
                .full_width(true)
                .size(ButtonSize::Sm)
                .variant(if index == current {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .disabled(busy || (index > current && (index != current + 1 || !can_next)))
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    target.update(cx, |state, cx| {
                        if state.app.ui_state.current_screen == screen {
                            state.app.select_workflow_step(index);
                        }
                        cx.notify();
                    });
                })
            }));
        let body = div()
            .id(content_id)
            .track_scroll(match screen {
                Screen::Recording => &self.scroll.recording,
                Screen::RoomEq => &self.scroll.room_eq,
                Screen::HeadphoneEq => &self.scroll.headphone_eq,
                _ => &self.scroll.spinorama_eq,
            })
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .p(d.card)
            // Leave enough scrollable clearance for the final wrapped content
            // row to move above the persistent footer on short windows.
            .pb(d.card * 5.0)
            .child(self.render_workflow_result_status(cx))
            .child(content);
        #[cfg(feature = "dev-api")]
        let body = {
            use crate::app::dev_api::DevTrackExt;
            body.dev_track(match screen {
                Screen::Recording => "recording.content",
                Screen::RoomEq => "roomeq.content",
                Screen::HeadphoneEq => "headphone.content",
                _ => "spinorama.content",
            })
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(theme.background)
            .when(compact, |root| {
                root.child(div().flex_none().p(d.card).child(selector))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .when(!compact, |row| row.child(rail))
                    .child(body),
            )
            .child(
                div()
                    .id("workflow-footer")
                    .flex_none()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(d.gap)
                    .p(d.card)
                    .border_t_1()
                    .border_color(theme.border)
                    .child({
                        let button = Button::new(
                            "workflow-back",
                            if current == 0 { nav.close } else { nav.back },
                        )
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .disabled(busy || config_change_confirmation)
                        .theme(theme.to_button_theme())
                        .on_click_event(move |_, _, cx| {
                            back.update(cx, |state, cx| {
                                if state.app.ui_state.current_screen == screen {
                                    state.app.move_workflow_step(false);
                                }
                                cx.notify();
                            });
                        });
                        #[cfg(feature = "dev-api")]
                        let button = {
                            use crate::app::dev_api::DevTrackExt;
                            button.dev_track(match screen {
                                Screen::Recording => "recording.back",
                                Screen::RoomEq => "roomeq.back",
                                Screen::HeadphoneEq => "headphone.back",
                                _ => "spinorama.back",
                            })
                        };
                        button
                    })
                    .when(config_change_confirmation, |footer| {
                        let confirm =
                            Button::new("recording-config-edit-confirm", config_confirm_label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Primary)
                                .theme(theme.to_button_theme())
                                .on_click_event(move |_, _, cx| {
                                    config_confirm.update(cx, |state, cx| {
                                        state.app.confirm_recording_settings_edit();
                                        cx.notify();
                                    });
                                });
                        #[cfg(feature = "dev-api")]
                        let confirm = {
                            use crate::app::dev_api::DevTrackExt;
                            confirm.dev_track("recording.config_edit_confirm")
                        };
                        let cancel =
                            Button::new("recording-config-edit-cancel", config_cancel_label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .on_click_event(move |_, _, cx| {
                                    config_cancel.update(cx, |state, cx| {
                                        state.app.cancel_recording_settings_edit();
                                        cx.notify();
                                    });
                                });
                        #[cfg(feature = "dev-api")]
                        let cancel = {
                            use crate::app::dev_api::DevTrackExt;
                            cancel.dev_track("recording.config_edit_cancel")
                        };
                        footer
                            .child(Text::caption(config_warning))
                            .child(confirm)
                            .child(cancel)
                    })
                    .when(
                        recording_action.is_some_and(|(step, _)| step == RecordingStep::Capture),
                        |footer| {
                            let accept_label = take_review_translations.accept;
                            let accepted_label = take_review_translations.accepted;
                            let accept_button = Button::new(
                                "recording-footer-accept",
                                accept_label,
                            )
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .disabled(!can_accept_review)
                            .theme(theme.to_button_theme())
                            .on_click_event(move |_, _, cx| {
                                if let Some(target) = review_target {
                                    accept_view.update(cx, |view, cx| {
                                        let (accepted, mic_position) = view.state.update(
                                            cx,
                                            |state, _| {
                                                let recording = &mut state
                                                    .app
                                                    .measurement_state
                                                    .recording_state;
                                                let mic_position = recording
                                                    .channel_recordings
                                                    .get(target)
                                                    .map(|take| take.mic_position_index);
                                                let accepted = recording.accept_take(target);
                                                if accepted {
                                                    recording.status_message =
                                                        accepted_label.to_string();
                                                    recording.status_severity = crate::app::types::recording::RecordingStatusSeverity::Success;
                                                }
                                                (usize::from(accepted), mic_position)
                                            },
                                        );
                                        if accepted > 0
                                            && let Some(mic_position) = mic_position
                                        {
                                            view.resume_batch_after_review_accept(
                                                mic_position,
                                                cx,
                                            );
                                        }
                                        cx.notify();
                                    });
                                }
                            });
                            #[cfg(feature = "dev-api")]
                            let accept_button = {
                                use crate::app::dev_api::DevTrackExt;
                                accept_button.dev_track_with_state(
                                    "recording.accept_footer",
                                    crate::app::dev_api::DevElementState::default()
                                        .enabled(can_accept_review),
                                )
                            };
                            let label = take_review_translations.retake;
                            let button = Button::new("recording-footer-retake", label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .disabled(!can_retake)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    retake_view.update(cx, |view, cx| {
                                        let target = {
                                            let state = view.state.read(cx);
                                            let recording =
                                                &state.app.measurement_state.recording_state;
                                            recording
                                                .channel_recordings
                                                .iter()
                                                .position(|take| {
                                                    take.state
                                                        == ChannelRecordingState::ReviewNeeded
                                                })
                                                .or_else(|| {
                                                    recording.channel_recordings.iter().position(
                                                        |take| {
                                                            take.state
                                                                == ChannelRecordingState::Done
                                                        },
                                                    )
                                                })
                                        };
                                        if let Some(target) = target {
                                            view.start_recording_channel(target, cx);
                                        }
                                    });
                                });
                            #[cfg(feature = "dev-api")]
                            let button = {
                                use crate::app::dev_api::DevTrackExt;
                                button.dev_track_with_state(
                                    "recording.retake_footer",
                                    crate::app::dev_api::DevElementState::default()
                                        .enabled(can_retake),
                                )
                            };
                            footer.child(accept_button).child(button)
                        },
                    )
                    .when(
                        recording_action.is_some_and(|(step, _)| step == RecordingStep::Saving),
                        |footer| {
                            let button = Button::new("recording-footer-save", save_label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Primary)
                                .disabled(!can_save)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    save_view.update(cx, crate::ui::PlayerView::save_recordings);
                                });
                            #[cfg(feature = "dev-api")]
                            let button = {
                                use crate::app::dev_api::DevTrackExt;
                                button.dev_track_with_state(
                                    "recording.save_footer",
                                    crate::app::dev_api::DevElementState::default()
                                        .enabled(can_save),
                                )
                            };
                            footer.child(button)
                        },
                    )
                    // The compact step selector already identifies the step.
                    // Repeating it here wraps the footer and starves the body.
                    .when(!compact, |footer| {
                        footer.child(Text::caption(format!(
                            "{} / {count} · {}",
                            current + 1,
                            labels[current]
                        )))
                    })
                    .child({
                        let button = Button::new("workflow-next", next_label)
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Primary)
                            .disabled(!can_next || busy || config_change_confirmation)
                            .theme(theme.to_button_theme())
                            .on_click_event(move |_, _, cx| {
                                next.update(cx, |state, cx| {
                                    if state.app.ui_state.current_screen == screen {
                                        state.app.move_workflow_step(true);
                                    }
                                    cx.notify();
                                });
                            });
                        #[cfg(feature = "dev-api")]
                        let button = {
                            use crate::app::dev_api::DevTrackExt;
                            button.dev_track_with_state(
                                match screen {
                                    Screen::Recording => "recording.next",
                                    Screen::RoomEq => "roomeq.next",
                                    Screen::HeadphoneEq => "headphone.next",
                                    _ => "spinorama.next",
                                },
                                crate::app::dev_api::DevElementState::default()
                                    .enabled(can_next && !busy),
                            )
                        };
                        button
                    }),
            )
            .into_any_element()
    }
}
