use super::misc::get_brand_image_path;
use crate::app::i18n::AudioDeviceTranslations;
#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::state::audio_device::{HalConfig, format_buffer_size, format_sample_rate};
#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Select, SelectOption};
use gpui_ui_kit::{HStack, StackAlign, StackSpacing, Text, VStack};

impl PlayerView {
    /// Poll on the UI control cadence; never block rendering on the player actor.
    pub(crate) fn finish_pending_audio_apply(state: &mut crate::app::AppState, cx: &App) -> bool {
        let Some(result) = state
            .app
            .audio_device_state
            .audio_apply
            .pending
            .as_ref()
            .and_then(|pending| pending.receipt.poll())
        else {
            return false;
        };
        let Some(mut pending) = state.app.audio_device_state.audio_apply.pending.take() else {
            return false;
        };
        if let Some(recovery) = pending.recovery.take() {
            use crate::app::state::audio_device::AudioApplyRecovery;
            if state
                .player
                .last_transport_receipt()
                .is_some_and(|latest| !latest.is_same_command(&pending.receipt))
            {
                let message = match recovery {
                    AudioApplyRecovery::Restoring(message)
                    | AudioApplyRecovery::Stopping(message) => message,
                };
                state.app.audio_device_state.output_draft.error = Some(message);
                return true;
            }
            match recovery {
                AudioApplyRecovery::Restoring(message) => {
                    if result.is_ok() {
                        state.app.audio_device_state.output_draft.error = Some(message);
                    } else {
                        Self::stop_failed_audio_recovery(state, pending, message);
                    }
                }
                AudioApplyRecovery::Stopping(message) => {
                    let stopped = result.is_ok();
                    if stopped {
                        state.app.playback.is_playing = false;
                    }
                    let detail =
                        crate::app::i18n::AudioPreferencesPersistenceTranslations::recovery_stopped(
                            state.app.ui_state.language,
                            stopped,
                        );
                    state.app.audio_device_state.output_draft.error =
                        Some(format!("{message}. {detail}"));
                }
            }
            return true;
        }
        if let Err(failure) = result {
            let text =
                crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
            Self::restore_audio_after_failed_apply(
                state,
                pending,
                format!("{}: {}", text.output_failed, failure.error),
            );
            return true;
        }
        let previous = state.app.audio_preferences();
        let previous_index = state.app.audio_device_state.selected_output_device_index;
        if let Err(error) = state.app.restore_audio_preferences(&pending.target) {
            Self::restore_audio_after_failed_apply(state, pending, error.to_string());
            return true;
        }
        state.app.audio_device_state.selected_output_device_index = pending.target_index;
        let saved = state.app.save_config(state.layout.read(cx));
        if let Err(error) = saved {
            let _ = state.app.restore_audio_preferences(&previous);
            state.app.audio_device_state.selected_output_device_index = previous_index;
            let message = crate::app::i18n::AudioPreferencesPersistenceTranslations::save_failed(
                state.app.ui_state.language,
                true,
            );
            log::warn!("Could not save audio preferences: {error}");
            Self::restore_audio_after_failed_apply(state, pending, message.into());
            return true;
        }
        // Preserve any additional edits made while the operation was pending.
        if state.app.audio_device_state.output_draft == pending.draft {
            state.app.audio_device_state.output_draft.discard();
        } else {
            state
                .app
                .audio_device_state
                .output_draft
                .rebase_after_apply(&previous, &pending.target);
        }
        if state.app.settings.navigation.close_pending
            && !state.app.audio_device_state.output_draft.is_dirty()
        {
            Self::finish_preferences_close(&mut state.app);
        }
        true
    }

    fn restore_audio_after_failed_apply(
        state: &mut crate::app::AppState,
        mut pending: crate::app::state::audio_device::PendingAudioApply,
        message: String,
    ) {
        if state
            .player
            .last_transport_receipt()
            .is_some_and(|latest| !latest.is_same_command(&pending.receipt))
        {
            // Recovery must never replace a newer user transport request.
            // Its completion is handled by the normal transport polling path.
            state.app.audio_device_state.output_draft.error = Some(message);
            return;
        }
        state
            .app
            .plugin_state
            .graph
            .set_replay_gain(pending.previous_graph_gain);
        state.app.audio_device_state.audio_apply.last_submission = None;
        match state.app.audio_device_state.playback_source {
            crate::app::types::PlaybackSource::File => {
                if let Some(source) = state.app.queue_state.current_track_source() {
                    let position = state.app.playback.position_secs;
                    Self::play_track_at(state, source, Some(position));
                }
            }
            #[cfg(all(target_os = "macos", feature = "hal"))]
            PlaybackSource::HalDevice => {
                Self::start_hal_playback_state_with_restart(state, true);
            }
        }
        if let Some(receipt) = state
            .app
            .audio_device_state
            .audio_apply
            .last_submission
            .take()
        {
            pending.receipt = receipt;
            pending.recovery =
                Some(crate::app::state::audio_device::AudioApplyRecovery::Restoring(message));
            state.app.audio_device_state.audio_apply.pending = Some(pending);
        } else {
            Self::stop_failed_audio_recovery(state, pending, message);
        }
    }

    fn stop_failed_audio_recovery(
        state: &mut crate::app::AppState,
        mut pending: crate::app::state::audio_device::PendingAudioApply,
        message: String,
    ) {
        if state
            .player
            .last_transport_receipt()
            .is_some_and(|latest| !latest.is_same_command(&pending.receipt))
        {
            state.app.audio_device_state.output_draft.error = Some(message);
            return;
        }
        match state.player.stop_with_receipt() {
            Ok(receipt) => {
                pending.receipt = receipt;
                pending.recovery =
                    Some(crate::app::state::audio_device::AudioApplyRecovery::Stopping(message));
                state.app.audio_device_state.audio_apply.pending = Some(pending);
            }
            Err(_) => {
                let detail =
                    crate::app::i18n::AudioPreferencesPersistenceTranslations::recovery_stopped(
                        state.app.ui_state.language,
                        false,
                    );
                state.app.audio_device_state.output_draft.error =
                    Some(format!("{message}. {detail}"));
            }
        }
    }

    pub(crate) fn apply_audio_output_draft(state: &mut crate::app::AppState, cx: &App) -> bool {
        if state.app.audio_device_state.audio_apply.pending.is_some() {
            return false;
        }
        let text = crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        let previous_preferences = state.app.audio_preferences();
        let was_playing = state.app.playback.is_playing;
        let previous_pending_restart = state.app.audio_device_state.pending_output_restart.clone();
        let devices = &mut state.app.audio_device_state;
        if !devices.output_draft.is_dirty() {
            return true;
        }
        devices.output_draft.error = None;
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        if devices.output_draft.systemwide_input == Some(true) {
            devices.output_draft.error = Some(text.output_failed.into());
            return false;
        }
        let restart_required = devices.output_draft.requires_restart(!matches!(
            devices.playback_source,
            crate::app::types::PlaybackSource::File
        ));
        let target_source = devices.playback_source;
        #[cfg(all(target_os = "macos", feature = "hal"))]
        let target_source = devices
            .output_draft
            .systemwide_input
            .map(|enabled| {
                if enabled {
                    PlaybackSource::HalDevice
                } else {
                    PlaybackSource::File
                }
            })
            .unwrap_or(target_source);
        let mut target_format = devices.hal_config.clone();
        target_format.sample_rate = devices
            .output_draft
            .sample_rate_hz
            .unwrap_or(target_format.sample_rate);
        target_format.channel_count = devices
            .output_draft
            .channel_count
            .unwrap_or(target_format.channel_count);
        target_format.buffer_frames = devices
            .output_draft
            .buffer_frames
            .unwrap_or(target_format.buffer_frames);
        if devices
            .output_draft
            .sample_rate_hz
            .is_some_and(|value| !crate::app::state::audio_device::SAMPLE_RATES.contains(&value))
            || devices.output_draft.channel_count.is_some_and(|value| {
                !sotf_audio_player::ui_models::audio_preferences::INPUT_CHANNEL_COUNTS
                    .contains(&value)
            })
            || devices.output_draft.buffer_frames.is_some_and(|value| {
                !crate::app::state::audio_device::BUFFER_SIZES.contains(&value)
            })
            || (restart_required
                && state.app.playback.is_playing
                && matches!(target_source, crate::app::types::PlaybackSource::File)
                && state.app.queue_state.current_track_source().is_none())
        {
            devices.output_draft.error = Some(text.output_failed.into());
            return false;
        }
        let original_source = devices.playback_source;
        let original_format = devices.hal_config.clone();
        let selected = if devices.output_draft.system_default {
            Some((
                devices
                    .output_devices
                    .iter()
                    .position(|device| device.is_default)
                    .unwrap_or(0),
                None,
            ))
        } else if let Some(name) = devices.output_draft.device_name.clone() {
            let Some(index) = devices
                .output_devices
                .iter()
                .position(|device| device.name == name)
            else {
                devices.output_draft.error = Some(text.output_unavailable.into());
                return false;
            };
            Some((index, Some(name)))
        } else {
            None
        };
        let original = (
            devices.selected_output_device_index,
            devices.current_output_device_name.clone(),
            devices.follow_system_default,
        );
        let original_replay_gain = (
            state.app.playback.replay_gain_enabled,
            state.app.playback.replay_gain_mode,
        );
        let original_graph_gain = state.app.plugin_state.graph.replay_gain_db();
        devices.playback_source = target_source;
        devices.hal_config = target_format;
        if let Some((index, name)) = selected {
            devices.selected_output_device_index = index;
            devices.follow_system_default = name.is_none();
            devices.current_output_device_name = name;
        }
        if let Some(enabled) = devices.output_draft.replay_gain_enabled {
            state.app.playback.replay_gain_enabled = enabled;
        }
        if let Some(mode) = devices.output_draft.replay_gain_mode {
            state.app.playback.replay_gain_mode = mode;
        }
        if restart_required
            && !state.app.playback.is_playing
            && state.app.playback.current_queue_index.is_some()
            && matches!(
                state.app.audio_device_state.playback_source,
                crate::app::types::PlaybackSource::File
            )
        {
            state
                .app
                .audio_device_state
                .pending_output_restart
                .get_or_insert_with(|| original.clone());
        }
        if restart_required && state.app.playback.is_playing {
            state.app.audio_device_state.audio_apply.last_submission = None;
            let accepted = match target_source {
                crate::app::types::PlaybackSource::File => {
                    if let Some(source) = state.app.queue_state.current_track_source() {
                        let position = state.app.playback.position_secs;
                        Self::play_track_at(state, source.clone(), Some(position));
                        state.app.playback.is_playing
                            && state
                                .app
                                .audio_device_state
                                .audio_apply
                                .last_submission
                                .is_some()
                    } else {
                        false
                    }
                }
                #[cfg(all(target_os = "macos", feature = "hal"))]
                PlaybackSource::HalDevice => {
                    Self::start_hal_playback_state_with_restart(state, true)
                }
            };
            if !accepted {
                state.app.audio_device_state.selected_output_device_index = original.0;
                state.app.audio_device_state.current_output_device_name = original.1;
                state.app.audio_device_state.follow_system_default =
                    previous_preferences.follow_system_default;
                state.app.audio_device_state.playback_source = original_source;
                #[cfg(all(target_os = "macos", feature = "hal"))]
                if matches!(target_source, PlaybackSource::HalDevice) {
                    Self::apply_hal_config_to_driver(&original_format);
                }
                state.app.audio_device_state.hal_config = original_format;
                state.app.playback.replay_gain_enabled = original_replay_gain.0;
                state.app.playback.replay_gain_mode = original_replay_gain.1;
                state
                    .app
                    .plugin_state
                    .graph
                    .set_replay_gain(original_graph_gain);
                state.app.audio_device_state.output_draft.error = Some(text.output_failed.into());
                return false;
            }
            if let Some(receipt) = state
                .app
                .audio_device_state
                .audio_apply
                .last_submission
                .take()
            {
                let pending = crate::app::state::audio_device::PendingAudioApply {
                    receipt,
                    target: state.app.audio_preferences(),
                    target_index: state.app.audio_device_state.selected_output_device_index,
                    draft: state.app.audio_device_state.output_draft.clone(),
                    previous_graph_gain: original_graph_gain,
                    recovery: None,
                };
                // Committed preferences remain unchanged while the actor works.
                // Other config saves must not persist an unacknowledged draft.
                let _ = state.app.restore_audio_preferences(&previous_preferences);
                state.app.audio_device_state.selected_output_device_index = original.0;
                state.app.audio_device_state.pending_output_restart = previous_pending_restart;
                state.app.audio_device_state.audio_apply.pending = Some(pending);
                return false;
            }
        }
        let save_result = {
            let layout = state.layout.read(cx);
            state.app.save_config(layout)
        };
        if let Err(error) = save_result {
            if !(was_playing && restart_required) {
                // No engine command ran: restore the complete active snapshot.
                let _ = state.app.restore_audio_preferences(&previous_preferences);
                state.app.audio_device_state.selected_output_device_index = original.0;
                state.app.audio_device_state.pending_output_restart = previous_pending_restart;
            }
            let message = crate::app::i18n::AudioPreferencesPersistenceTranslations::save_failed(
                state.app.ui_state.language,
                was_playing && restart_required,
            );
            log::warn!("Could not save audio preferences: {error}");
            state.app.audio_device_state.output_draft.error = Some(message.into());
            return false;
        }
        state.app.audio_device_state.output_draft.discard();
        true
    }

    pub(crate) fn render_audio_output_draft(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let devices = &state.app.audio_device_state;
        let theme = state.app.ui_state.theme.clone();
        let text = crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        if state.app.settings.navigation.close_pending
            || (!devices.output_draft.is_dirty() && devices.audio_apply.pending.is_none())
        {
            return div().into_any_element();
        }
        let name = if devices.output_draft.system_default {
            crate::app::i18n::AudioPreferencesPersistenceTranslations::system_default(
                state.app.ui_state.language,
            )
            .into()
        } else {
            devices.output_draft.device_name.clone().unwrap_or_else(|| {
                if devices.output_draft.replay_gain_enabled.is_some()
                    || devices.output_draft.replay_gain_mode.is_some()
                {
                    state.app.ui_state.translations.settings_replaygain.into()
                } else {
                    AudioDeviceTranslations::for_language(state.app.ui_state.language)
                        .audio_source
                        .into()
                }
            })
        };
        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .py(d.card)
            .child(Text::label(name))
            .when(devices.audio_apply.pending.is_some(), |element| {
                element.child(Text::body(
                    crate::app::i18n::AudioPreferencesPersistenceTranslations::applying(
                        state.app.ui_state.language,
                    ),
                ))
            })
            .when_some(devices.output_draft.error.clone(), |element, error| {
                element.child(Text::body(error).color(theme.error))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap)
                    .child(
                        Button::new("audio-output-discard", text.discard_routing)
                            .disabled(devices.audio_apply.pending.is_some())
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if state.app.audio_device_state.audio_apply.pending.is_some() {
                                        return;
                                    }
                                    state.app.audio_device_state.output_draft.discard();
                                });
                                cx.notify();
                            }))
                            .map(|button| {
                                #[cfg(feature = "dev-api")]
                                {
                                    use crate::app::dev_api::DevTrackExt;
                                    button.dev_track("settings.audio-discard")
                                }
                                #[cfg(not(feature = "dev-api"))]
                                {
                                    button
                                }
                            }),
                    )
                    .child(
                        Button::new("audio-output-apply", text.apply_audio)
                            .disabled(devices.audio_apply.pending.is_some())
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Primary)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, cx| {
                                    Self::apply_audio_output_draft(state, cx);
                                });
                                cx.notify();
                            }))
                            .map(|button| {
                                #[cfg(feature = "dev-api")]
                                {
                                    use crate::app::dev_api::DevTrackExt;
                                    button.dev_track("settings.audio-apply")
                                }
                                #[cfg(not(feature = "dev-api"))]
                                {
                                    button
                                }
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn render_audio_device_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let _text = AudioDeviceTranslations::for_language(state.app.ui_state.language);
        let _playback_source = state.app.audio_device_state.playback_source;
        let unnamed_device =
            crate::app::i18n::AudioPreferencesPersistenceTranslations::unnamed_device(
                state.app.ui_state.language,
            );

        let devices = &state.app.audio_device_state;
        let selected_name = devices
            .output_draft
            .selected_name(devices.current_output_device_name.as_deref());
        let selected = match selected_name {
            Some(name) => devices
                .output_devices
                .iter()
                .position(|device| device.name == name)
                .map(|index| format!("device:{index}"))
                .unwrap_or_else(|| "unavailable".into()),
            None => "system-default".into(),
        };
        let mut options = vec![SelectOption::new(
            "system-default",
            crate::app::i18n::AudioPreferencesPersistenceTranslations::system_default(
                state.app.ui_state.language,
            ),
        )];
        options.extend(
            devices
                .output_devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    SelectOption::new(
                        format!("device:{index}"),
                        if device.name.trim().is_empty() {
                            unnamed_device.to_owned()
                        } else {
                            device.name.clone()
                        },
                    )
                })
                .collect::<Vec<_>>(),
        );
        if selected == "unavailable"
            && let Some(name) = selected_name
        {
            options.push(SelectOption::new("unavailable", name.to_owned()));
        }
        let state_change = self.state.clone();
        // Select retains its blur callback beyond the rendered element.
        // That callback must not keep a closed window's AppState alive.
        let state_toggle = self.state.downgrade();
        let state_highlight = self.state.clone();
        let view_change = cx.entity().downgrade();
        let view_toggle = cx.entity().downgrade();
        let view_highlight = cx.entity().downgrade();
        let selector = Select::new("audio-output-device")
            .label(translations.devices_title)
            .options(options)
            .selected(selected)
            .is_open(devices.output_ui.open)
            .highlighted_index(devices.output_ui.highlight)
            .theme(theme.to_select_theme())
            .on_change(move |value: &SharedString, _, cx| {
                state_change.update(cx, |state, _| {
                    let devices = &mut state.app.audio_device_state;
                    if value.as_ref() == "system-default" {
                        devices
                            .output_draft
                            .select_system_default(devices.follow_system_default);
                    } else if let Some(device) = value
                        .strip_prefix("device:")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| devices.output_devices.get(index))
                    {
                        devices.output_draft.select(
                            device.name.clone(),
                            devices.current_output_device_name.as_deref(),
                        );
                    }
                    devices.output_ui.open = false;
                });
                let _ = view_change.update(cx, |_, cx| cx.notify());
            })
            .on_toggle(move |open, _, cx| {
                let _ = state_toggle.update(cx, |state, _| {
                    state.app.audio_device_state.output_ui.open = open;
                    if open {
                        state.app.audio_device_state.hal_dropdowns = Default::default();
                    }
                    state.app.audio_device_state.output_ui.highlight = None;
                });
                let _ = view_toggle.update(cx, |_, cx| cx.notify());
            })
            .on_highlight(move |highlight, _, cx| {
                state_highlight.update(cx, |state, _| {
                    state.app.audio_device_state.output_ui.highlight = highlight
                });
                let _ = view_highlight.update(cx, |_, cx| cx.notify());
            });
        #[cfg(feature = "dev-api")]
        let selector = {
            use crate::app::dev_api::DevTrackExt;
            selector.dev_track("settings.output-device")
        };
        let mut content = VStack::new().spacing(StackSpacing::Sm).child(
            self.preference_control(
                crate::app::types::PreferencesSetting::OutputDevice,
                div()
                    .id("audio-output-search-anchor")
                    .w(rems(24.0))
                    .max_w_full()
                    .child(selector)
                    .when_some(
                        self.preference_focus_handle(
                            crate::app::types::PreferencesSetting::OutputDevice,
                            cx,
                        ),
                        |element, focus| {
                            element
                                .track_focus(&focus)
                                .track_focus_element(&focus)
                                .focusable()
                        },
                    ),
                cx,
            ),
        );

        // HAL Input Source section (macOS only with hal feature)
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            let is_hal_mode = state
                .app
                .audio_device_state
                .output_draft
                .systemwide_input
                .unwrap_or(matches!(_playback_source, PlaybackSource::HalDevice));
            let state_entity = self.state.clone();
            let view_entity = cx.entity().downgrade();
            content = content
                .child(Text::label(_text.audio_source))
                .child({
                    let mut choices = div().flex().flex_wrap().gap(d.gap);
                    for (systemwide, key, label) in [
                        (false, "file", _text.file_player),
                        (true, "hal", _text.hal_device),
                    ] {
                        let button = Button::new(format!("audio-source-{key}"), label)
                            .size(ButtonSize::Sm)
                            .variant(if is_hal_mode == systemwide {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .theme(theme.to_button_theme());
                        let change_source =
                            move |view: &mut Self, _window: &mut Window, cx: &mut Context<Self>| {
                                view.state.update(cx, |state, _cx| {
                                    let devices = &mut state.app.audio_device_state;
                                    devices.output_draft.set_systemwide_input(
                                        systemwide,
                                        matches!(
                                            devices.playback_source,
                                            PlaybackSource::HalDevice
                                        ),
                                    );
                                    devices.close_hal_dropdowns();
                                });
                                cx.notify();
                            };
                        let button = if systemwide {
                            self.preference_action(
                                crate::app::types::PreferencesSetting::AudioSource,
                                button,
                                false,
                                change_source,
                                cx,
                            )
                        } else {
                            button
                                .build()
                                .text_size(d.text_sm)
                                .px(d.pad_x)
                                .py(d.pad_y_half)
                                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                                    change_source(view, window, cx);
                                }))
                                .into_any_element()
                        };
                        #[cfg(feature = "dev-api")]
                        let button = {
                            use crate::app::dev_api::DevTrackExt;
                            button.dev_track(format!("settings.audio-source.{key}"))
                        };
                        choices = choices.child(button);
                    }
                    choices
                })
                .child(div().h(d.gap));
            if is_hal_mode
                || state
                    .app
                    .settings
                    .navigation
                    .setting
                    .is_some_and(crate::app::types::PreferencesSetting::is_systemwide_format)
            {
                let devices = &state.app.audio_device_state;
                let mut hal_config = devices.hal_config.clone();
                hal_config.sample_rate = devices
                    .output_draft
                    .sample_rate_hz
                    .unwrap_or(hal_config.sample_rate);
                hal_config.channel_count = devices
                    .output_draft
                    .channel_count
                    .unwrap_or(hal_config.channel_count);
                hal_config.buffer_frames = devices
                    .output_draft
                    .buffer_frames
                    .unwrap_or(hal_config.buffer_frames);
                let hal_dropdowns = devices.hal_dropdowns.clone();
                let sample_rate_options: Vec<SelectOption> = HalConfig::available_sample_rates()
                    .iter()
                    .map(|&rate| SelectOption::new(rate.to_string(), format_sample_rate(rate)))
                    .collect();
                let channel_options: Vec<SelectOption> = vec![
                    SelectOption::new("2", "2 ch (Stereo)"),
                    SelectOption::new("4", "4 ch (Quad)"),
                    SelectOption::new("6", "6 ch (5.1)"),
                    SelectOption::new("8", "8 ch (7.1)"),
                ];
                let buffer_options: Vec<SelectOption> = HalConfig::available_buffer_sizes()
                    .iter()
                    .map(|&size| {
                        SelectOption::new(
                            size.to_string(),
                            format_buffer_size(size, hal_config.sample_rate),
                        )
                    })
                    .collect();

                if !is_hal_mode {
                    content = content.child(Text::caption(
                        crate::app::i18n::AudioPreferencesPersistenceTranslations::inactive_systemwide_format(state.app.ui_state.language)
                    ).color(theme.text_secondary));
                }
                content = content
                    .child(Text::label(_text.hal_configuration))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.gap_md)
                            .child({
                                // Sample Rate selector
                                let state_for_change = state_entity.clone();
                                let view_for_change = view_entity.clone();
                                let state_for_toggle = state_entity.downgrade();
                                let view_for_toggle = view_entity.clone();
                                let state_for_highlight = state_entity.clone();
                                let view_for_highlight = view_entity.clone();
                                Select::new("hal-sample-rate")
                                    .label(_text.sample_rate)
                                    .options(sample_rate_options)
                                    .selected(hal_config.sample_rate.to_string())
                                    .is_open(hal_dropdowns.sample_rate_open)
                                    .theme(theme.to_select_theme())
                                    .highlighted_index(hal_dropdowns.highlights[0])
                                    .on_highlight(move |highlight, _window, cx| {
                                        state_for_highlight.update(cx, |state, _cx| {
                                            state.app.audio_device_state.hal_dropdowns.highlights
                                                [0] = highlight;
                                        });
                                        let _ = view_for_highlight.update(cx, |_, cx| cx.notify());
                                    })
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(rate) = value.parse::<u32>() {
                                            Self::update_hal_sample_rate(
                                                &state_for_change,
                                                rate,
                                                cx,
                                            );
                                            let _ = view_for_change.update(cx, |_, cx| cx.notify());
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        let _ = state_for_toggle.update(cx, |state, cx| {
                                            state.app.audio_device_state.output_ui.open = false;
                                            state.app.audio_device_state.hal_dropdowns.highlights =
                                                [None; 3];
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = open;
                                            cx.notify();
                                        });
                                        let _ = view_for_toggle.update(cx, |_, cx| cx.notify());
                                    })
                                    .map(|select| {
                                        self.preference_select(
                                        crate::app::types::PreferencesSetting::SystemwideSampleRate,
                                        select, "settings.hal-sample-rate", cx,
                                    )
                                    })
                            })
                            .child({
                                // Channel Count selector
                                let state_for_change = state_entity.clone();
                                let view_for_change = view_entity.clone();
                                let state_for_toggle = state_entity.downgrade();
                                let view_for_toggle = view_entity.clone();
                                let state_for_highlight = state_entity.clone();
                                let view_for_highlight = view_entity.clone();
                                Select::new("hal-channel-count")
                                    .label(_text.channels)
                                    .options(channel_options)
                                    .selected(hal_config.channel_count.to_string())
                                    .is_open(hal_dropdowns.channel_count_open)
                                    .theme(theme.to_select_theme())
                                    .highlighted_index(hal_dropdowns.highlights[1])
                                    .on_highlight(move |highlight, _window, cx| {
                                        state_for_highlight.update(cx, |state, _cx| {
                                            state.app.audio_device_state.hal_dropdowns.highlights
                                                [1] = highlight;
                                        });
                                        let _ = view_for_highlight.update(cx, |_, cx| cx.notify());
                                    })
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(channels) = value.parse::<u32>() {
                                            Self::update_hal_channel_count(
                                                &state_for_change,
                                                channels,
                                                cx,
                                            );
                                            let _ = view_for_change.update(cx, |_, cx| cx.notify());
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        let _ = state_for_toggle.update(cx, |state, cx| {
                                            state.app.audio_device_state.output_ui.open = false;
                                            state.app.audio_device_state.hal_dropdowns.highlights =
                                                [None; 3];
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = open;
                                            cx.notify();
                                        });
                                        let _ = view_for_toggle.update(cx, |_, cx| cx.notify());
                                    })
                                    .map(|select| {
                                        self.preference_select(
                                        crate::app::types::PreferencesSetting::SystemwideChannels,
                                        select, "settings.hal-channel-count", cx,
                                    )
                                    })
                            })
                            .child({
                                // Buffer Size selector
                                let state_for_change = state_entity.clone();
                                let view_for_change = view_entity.clone();
                                let state_for_toggle = state_entity.downgrade();
                                let view_for_toggle = view_entity.clone();
                                let state_for_highlight = state_entity.clone();
                                let view_for_highlight = view_entity.clone();
                                Select::new("hal-buffer-size")
                                    .label(_text.buffer_size)
                                    .options(buffer_options)
                                    .selected(hal_config.buffer_frames.to_string())
                                    .is_open(hal_dropdowns.buffer_size_open)
                                    .theme(theme.to_select_theme())
                                    .highlighted_index(hal_dropdowns.highlights[2])
                                    .on_highlight(move |highlight, _window, cx| {
                                        state_for_highlight.update(cx, |state, _cx| {
                                            state.app.audio_device_state.hal_dropdowns.highlights
                                                [2] = highlight;
                                        });
                                        let _ = view_for_highlight.update(cx, |_, cx| cx.notify());
                                    })
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(size) = value.parse::<u32>() {
                                            Self::update_hal_buffer_size(
                                                &state_for_change,
                                                size,
                                                cx,
                                            );
                                            let _ = view_for_change.update(cx, |_, cx| cx.notify());
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        let _ = state_for_toggle.update(cx, |state, cx| {
                                            state.app.audio_device_state.output_ui.open = false;
                                            state.app.audio_device_state.hal_dropdowns.highlights =
                                                [None; 3];
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = open;
                                            cx.notify();
                                        });
                                        let _ = view_for_toggle.update(cx, |_, cx| cx.notify());
                                    })
                                    .map(|select| {
                                        self.preference_select(
                                            crate::app::types::PreferencesSetting::SystemwideBuffer,
                                            select,
                                            "settings.hal-buffer-size",
                                            cx,
                                        )
                                    })
                            }),
                    )
                    .child(div().h(d.gap)); // Spacer
            }
        }

        #[cfg(target_os = "ios")]
        {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(Text::label(_text.wireless_devices))
                    .child(
                        Button::new("show-airplay-route-picker", "AirPlay")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click_event(|_, _window, _cx| {
                                unsafe extern "C" {
                                    fn sotf_ios_show_route_picker();
                                }
                                unsafe { sotf_ios_show_route_picker() };
                            }),
                    ),
            );
        }

        let details_expanded = state.app.audio_device_state.output_ui.details_expanded;
        let sizing = crate::ui::resolve_sizing_context(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let detail_columns = if sizing.window_width_rems < 72.0 {
            1
        } else {
            2
        };
        content = content.child(
            Button::new(
                "audio-device-details",
                crate::app::i18n::AudioPreferencesPersistenceTranslations::device_details(
                    state.app.ui_state.language,
                ),
            )
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Sm)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(|view, _, _, cx| {
                view.state.update(cx, |state, _| {
                    let expanded = &mut state.app.audio_device_state.output_ui.details_expanded;
                    *expanded = !*expanded;
                });
                cx.notify();
            }))
            .map(|button| {
                #[cfg(feature = "dev-api")]
                {
                    use crate::app::dev_api::DevTrackExt;
                    button.dev_track("settings.device-details")
                }
                #[cfg(not(feature = "dev-api"))]
                {
                    button
                }
            }),
        );
        if !details_expanded {
            return content;
        }

        content.child(
            // Keep device details readable as the window narrows or text grows.
            div()
                .grid()
                .grid_cols(detail_columns)
                .gap(d.gap_md)
                .w_full()
                .children(
                    state
                        .app
                        .audio_device_state
                        .output_devices
                        .iter()
                        .enumerate()
                        .map(|(idx, device)| {
                            let is_selected =
                                state.app.audio_device_state.output_draft.selected_name(
                                    state
                                        .app
                                        .audio_device_state
                                        .current_output_device_name
                                        .as_deref(),
                                ) == Some(device.name.as_str());
                            let sample_rate = device
                                .default_config
                                .as_ref()
                                .map(|c| c.sample_rate)
                                .unwrap_or(0);
                            let channels = device
                                .default_config
                                .as_ref()
                                .map(|c| c.channels)
                                .unwrap_or(0);
                            let theme = theme.clone();
                            let device_name = device.name.clone();
                            let is_default = device.is_default;

                            // Try to find a brand image
                            let brand_image = get_brand_image_path(&device_name);

                            div()
                                .w_full()
                                .p(d.pad_x)
                                .rounded(d.r_md)
                                .cursor_pointer()
                                .border_1()
                                .when(is_selected, |el| {
                                    el.bg(theme.surface_selected).border_color(theme.accent)
                                })
                                .when(!is_selected, |el| {
                                    el.bg(theme.surface)
                                        .border_color(theme.border)
                                        .hover(|s| s.bg(theme.surface_hover))
                                })
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .align(StackAlign::Center)
                                        .when_some(brand_image, |stack, image_path| {
                                            stack.child(
                                                div()
                                                    .w(rems(3.75))
                                                    .h(rems(3.75))
                                                    .rounded(d.r_md)
                                                    .bg(theme.background)
                                                    .overflow_hidden()
                                                    .child(
                                                        img(image_path)
                                                            .w_full()
                                                            .h_full()
                                                            .object_fit(ObjectFit::Contain), // Contain to show full brand
                                                    ),
                                            )
                                        })
                                        .child(
                                            VStack::new()
                                                .spacing(StackSpacing::Xs)
                                                .child(
                                                    Text::label(if device_name.trim().is_empty() {
                                                        unnamed_device.to_owned()
                                                    } else {
                                                        device_name
                                                    })
                                                    .color(theme.text_primary),
                                                )
                                                .child(
                                                    HStack::new()
                                                        .spacing(StackSpacing::Sm)
                                                        .child(device_info_pill(
                                                            format!("{} ch", channels),
                                                            &theme,
                                                            d,
                                                        ))
                                                        .child(device_info_pill(
                                                            if sample_rate >= 1000 {
                                                                format!(
                                                                    "{} kHz",
                                                                    sample_rate / 1000
                                                                )
                                                            } else {
                                                                format!("{} Hz", sample_rate)
                                                            },
                                                            &theme,
                                                            d,
                                                        )),
                                                )
                                                .when(is_default, |stack| {
                                                    stack.child(device_success_pill(
                                                        format!(
                                                            "✓ {}",
                                                            translations.settings_default_badge
                                                        ),
                                                        &theme,
                                                        d,
                                                    ))
                                                }),
                                        ),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            let devices = &mut state.app.audio_device_state;
                                            if let Some(device) = devices.output_devices.get(idx) {
                                                devices.output_draft.select(
                                                    device.name.clone(),
                                                    devices.current_output_device_name.as_deref(),
                                                );
                                            }
                                        });
                                        cx.notify();
                                    }),
                                )
                        }),
                ),
        )
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(crate) fn start_hal_playback_state(state: &mut crate::app::AppState) -> bool {
        Self::start_hal_playback_state_with_restart(state, false)
    }

    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn start_hal_playback_state_with_restart(
        state: &mut crate::app::AppState,
        restart: bool,
    ) -> bool {
        use sotf_audio::engine::PluginConfig;

        // Systemwide input has no track or album loudness metadata.
        state.app.plugin_state.graph.set_replay_gain(None);

        // Get HAL configuration from state
        let hal_config = &state.app.audio_device_state.hal_config;
        let sample_rate = hal_config.sample_rate;
        let channels = hal_config.channel_count;

        // Build plugin chain with hal_input as first plugin
        let mut plugins: Vec<PluginConfig> = Vec::new();

        // Add hal_input plugin as the source with configured settings
        plugins.push(PluginConfig {
            plugin_type: "hal_input".to_string(),
            parameters: serde_json::json!({
                "channels": channels,
            }),
        });

        // Add plugins from the current plugin chain using configured sample rate
        for plugin_config in state
            .app
            .plugin_state
            .graph
            .to_plugin_configs(sample_rate as f64)
        {
            plugins.push(plugin_config);
        }

        // Get output device
        let output_device = state
            .app
            .audio_device_state
            .current_output_device_name
            .clone();

        // Determine output channels from plugin chain
        let output_channels = state.app.plugin_state.graph.output_channels();

        // Update driver-hal with the new configuration
        Self::apply_hal_config_to_driver(hal_config);

        // Start HAL playback with configured sample rate
        let submitted = if restart {
            state.player.restart_hal_playback_with_config_receipt(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            )
        } else {
            state.player.start_hal_playback_with_config_receipt(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            )
        };
        match submitted {
            Ok(receipt) => {
                state.app.audio_device_state.audio_apply.last_submission = Some(receipt);
                state.app.audio_device_state.playback_source = PlaybackSource::HalDevice;
                state.app.playback.is_playing = true;
                log::info!(
                    "HAL playback started: {}Hz, {} channels",
                    sample_rate,
                    channels
                );
                true
            }
            Err(e) => {
                log::error!("Failed to start HAL playback: {}", e);
                state.app.playback.is_playing = false;
                state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::error(
                    format!("Failed to start HAL: {}", e),
                ));
                false
            }
        }
    }

    /// Stage the HAL sample rate without changing playback
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_sample_rate(
        state_entity: &Entity<crate::app::AppState>,
        sample_rate: u32,
        cx: &mut App,
    ) {
        let entity_id = state_entity.entity_id();
        state_entity.update(cx, |state, _cx| {
            let devices = &mut state.app.audio_device_state;
            devices
                .output_draft
                .set_sample_rate_hz(sample_rate, devices.hal_config.sample_rate);
            devices.close_hal_dropdowns();
        });
        cx.notify(entity_id);
    }

    /// Stage the HAL channel count without changing playback
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_channel_count(
        state_entity: &Entity<crate::app::AppState>,
        channel_count: u32,
        cx: &mut App,
    ) {
        let entity_id = state_entity.entity_id();
        state_entity.update(cx, |state, _cx| {
            let devices = &mut state.app.audio_device_state;
            devices
                .output_draft
                .set_channel_count(channel_count, devices.hal_config.channel_count);
            devices.close_hal_dropdowns();
        });
        cx.notify(entity_id);
    }

    /// Stage the HAL buffer size without changing playback
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_buffer_size(
        state_entity: &Entity<crate::app::AppState>,
        buffer_frames: u32,
        cx: &mut App,
    ) {
        let entity_id = state_entity.entity_id();
        state_entity.update(cx, |state, _cx| {
            let devices = &mut state.app.audio_device_state;
            devices
                .output_draft
                .set_buffer_frames(buffer_frames, devices.hal_config.buffer_frames);
            devices.close_hal_dropdowns();
        });
        cx.notify(entity_id);
    }

    /// Apply HAL configuration to the driver-hal shared memory
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn apply_hal_config_to_driver(config: &HalConfig) {
        use driver_hal::HalOutputWriter;

        if let Some(mut writer) = HalOutputWriter::new() {
            writer.set_sample_rate(config.sample_rate);
            writer.set_channel_count(config.channel_count);
            writer.set_buffer_frames(config.buffer_frames);
            log::debug!(
                "Applied HAL config to driver: {}Hz, {} ch, {} frames",
                config.sample_rate,
                config.channel_count,
                config.buffer_frames
            );
        } else {
            log::warn!("Could not connect to HAL driver to apply configuration");
        }
    }
}

fn device_info_pill(
    label: impl Into<SharedString>,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    device_pill(
        label,
        crate::theme::Theme::with_opacity(theme.info, 0.16),
        theme.info,
        d,
    )
}

fn device_success_pill(
    label: impl Into<SharedString>,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    device_pill(
        label,
        crate::theme::Theme::with_opacity(theme.success, 0.16),
        theme.success,
        d,
    )
}

fn device_pill(
    label: impl Into<SharedString>,
    bg: Rgba,
    text_color: Rgba,
    d: Ds,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .px(d.pad_y)
        .py(d.grid)
        .rounded(d.r_sm)
        .bg(bg)
        .text_size(d.text_xs)
        .font_weight(FontWeight::MEDIUM)
        .text_color(text_color)
        .child(label.into())
}
