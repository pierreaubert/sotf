use super::app_impl::App;
use super::types::{
    ChannelGroup, ChannelInfo, LoudnessControlOperation, PendingLoudnessControl,
    PendingParameterUpdate,
};
use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};

impl App {
    /// Build channel groups from current speaker configuration or channel count
    /// Uses caching to avoid rebuilding every frame
    pub fn update_level_meter_groups(&mut self) {
        let num_channels = self
            .playback
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        if num_channels == 0 {
            return;
        }

        // Get current speaker config
        let current_speaker_config = self.plugin_rack.graph.output_speaker_config();

        // Skip rebuilding if nothing has changed
        if num_channels == self.level_meters.last_channel_count
            && current_speaker_config == self.level_meters.last_speaker_config
            && !self.level_meters.groups.is_empty()
        {
            return;
        }

        // Update cache
        self.level_meters.last_channel_count = num_channels;
        self.level_meters.last_speaker_config = current_speaker_config.clone();

        self.level_meters.groups.clear();

        // Try to get meter groups from the speaker config (via upmixer plugin)
        // This handles collisions like 5.1.4 vs 7.1.2 (both 10 channels)
        let meter_groups: Option<&[MeterGroupSpec]> = current_speaker_config
            .as_deref()
            .and_then(get_meter_groups)
            .or_else(|| get_meter_groups_by_channels(num_channels));

        if let Some(groups) = meter_groups {
            // Convert static specs to runtime groups
            for group_spec in groups {
                self.level_meters.groups.push(ChannelGroup {
                    name: group_spec.name.to_string(),
                    channels: group_spec
                        .channels
                        .iter()
                        .map(|ch| ChannelInfo {
                            index: ch.index,
                            name: ch.label.to_string(),
                            display_name: ch
                                .display_chars
                                .iter()
                                .map(|s| (*s).to_string())
                                .collect(),
                        })
                        .collect(),
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        } else {
            // Fallback for unknown channel counts (mono, quad, or exotic configs)
            match num_channels {
                1 => {
                    // Mono
                    self.level_meters.groups.push(ChannelGroup {
                        name: "Mono".to_string(),
                        channels: vec![ChannelInfo {
                            index: 0,
                            name: "M".to_string(),
                            display_name: vec!["M".to_string()],
                        }],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                4 => {
                    // Quad (FL, FR, SL, SR) - not a standard speaker config
                    self.level_meters.groups.push(ChannelGroup {
                        name: "L/R".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 0,
                                name: "L".to_string(),
                                display_name: vec!["L".to_string()],
                            },
                            ChannelInfo {
                                index: 1,
                                name: "R".to_string(),
                                display_name: vec!["R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                    self.level_meters.groups.push(ChannelGroup {
                        name: "Surrounds".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 2,
                                name: "SL".to_string(),
                                display_name: vec!["S".to_string(), "L".to_string()],
                            },
                            ChannelInfo {
                                index: 3,
                                name: "SR".to_string(),
                                display_name: vec!["S".to_string(), "R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                _ => {
                    // Generic fallback - treat all channels as one group
                    let channels: Vec<ChannelInfo> = (0..num_channels)
                        .map(|i| {
                            let spec = make_fallback_channel(i);
                            ChannelInfo {
                                index: spec.index,
                                name: spec.label.to_string(),
                                display_name: spec
                                    .display_chars
                                    .iter()
                                    .map(|s| (*s).to_string())
                                    .collect(),
                            }
                        })
                        .collect();
                    self.level_meters.groups.push(ChannelGroup {
                        name: "All Channels".to_string(),
                        channels,
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
            }
        }

        // Update Matrix plugin channel states for M/S/D controls
        self.update_matrix_channel_states();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meters.groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_matrix_channel_states();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get_mut(self.level_meters.selected_group)
        {
            group.muted = !group.muted;
            self.update_matrix_channel_states();
        }
    }

    /// Toggle solo for the selected level meter group
    pub fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get_mut(self.level_meters.selected_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meters.groups.iter_mut().enumerate() {
                if idx == self.level_meters.selected_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_matrix_channel_states();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meters
            .groups
            .get_mut(self.level_meters.selected_group)
        {
            group.dimmed = !group.dimmed;
            self.update_matrix_channel_states();
        }
    }

    /// Update the Matrix plugin's channel states based on current level meter group M/S/D
    fn update_matrix_channel_states(&mut self) {
        use sotf_audio_player::PluginSettings;
        use sotf_plugins::ChannelState;

        // Calculate total channel count
        let num_channels = self
            .level_meters
            .groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meters.groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Find and update the permanent Matrix plugin's channel_states in memory
        for i in 0..self.plugin_rack.graph.len() {
            if let Some(plugin) = self.plugin_rack.graph.get_plugin_mut(i)
                && plugin.is_permanent()
                && matches!(&plugin.settings, PluginSettings::Matrix { .. })
            {
                if let PluginSettings::Matrix {
                    channel_states: ref mut cs,
                    ..
                } = plugin.settings
                {
                    *cs = channel_states.clone();
                }
                break;
            }
        }

        // Queue zero-dropout parameter update via matrix_engine_index
        if let Some(engine_index) = self.plugin_rack.graph.matrix_engine_index()
            && let Ok(json) = serde_json::to_string(&channel_states)
        {
            self.plugin_rack.pending_param_update = Some(PendingParameterUpdate {
                plugin_index: engine_index,
                param_id: "channel_states".to_string(),
                value: json,
            });
        }
    }

    /// Navigate to next level meter group
    pub fn select_next_level_meter_group(&mut self) {
        if !self.level_meters.groups.is_empty() {
            self.level_meters.selected_group =
                (self.level_meters.selected_group + 1) % self.level_meters.groups.len();
        }
    }

    /// Navigate to previous level meter group
    pub fn select_previous_level_meter_group(&mut self) {
        if !self.level_meters.groups.is_empty() {
            if self.level_meters.selected_group == 0 {
                self.level_meters.selected_group = self.level_meters.groups.len() - 1;
            } else {
                self.level_meters.selected_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    pub fn select_next_level_meter_control(&mut self) {
        self.level_meters.control_selection = (self.level_meters.control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    pub fn select_previous_level_meter_control(&mut self) {
        self.level_meters.control_selection = if self.level_meters.control_selection == 0 {
            2
        } else {
            self.level_meters.control_selection - 1
        };
    }

    /// Queue a transient command for the exact output monitor whose snapshot
    /// drives the LevelMeters pane.
    pub fn request_loudness_control(&mut self, operation: LoudnessControlOperation) {
        let translations = crate::i18n::TuiTranslations::for_language(self.ui.language);
        let Some(loudness) = self.playback.loudness_info.as_ref() else {
            self.plugin_rack.loudness_control_error =
                Some(translations.ui("Output monitor unavailable").to_string());
            return;
        };
        let runtime_instance_id = loudness.integrated_control_instance_id;
        if runtime_instance_id == 0 {
            self.plugin_rack.loudness_control_error =
                Some(translations.ui("Output monitor unavailable").to_string());
            return;
        }
        let Some(engine_index) = self.plugin_rack.graph.output_monitor_engine_index() else {
            self.plugin_rack.loudness_control_error =
                Some(translations.ui("Output monitor unavailable").to_string());
            return;
        };
        if self.plugin_rack.pending_param_update.is_some() {
            self.plugin_rack.loudness_control_error = Some(
                translations
                    .ui("Another parameter update is pending")
                    .to_string(),
            );
            return;
        }
        let Some(request_id) = self
            .plugin_rack
            .loudness_control_next_request_id
            .checked_add(1)
        else {
            self.plugin_rack.loudness_control_error =
                Some(translations.ui("Control request ID exhausted").to_string());
            return;
        };
        self.plugin_rack.loudness_control_next_request_id = request_id;
        let pending = PendingLoudnessControl {
            runtime_instance_id,
            request_id,
            operation,
        };
        self.plugin_rack.pending_loudness_control = Some(pending);
        self.plugin_rack.retryable_loudness_control = Some(pending);
        self.plugin_rack.loudness_control_error = None;
        self.plugin_rack.pending_param_update = Some(PendingParameterUpdate {
            plugin_index: engine_index,
            param_id: "integrated_control_command".to_string(),
            value: format!(
                "{}:{}:{}",
                runtime_instance_id,
                request_id,
                operation.parameter_name()
            ),
        });
        self.ui.needs_redraw = true;
    }

    /// Retry the most recent explicit action with a new request ID.
    pub fn retry_loudness_control(&mut self) {
        let Some(retryable) = self.plugin_rack.retryable_loudness_control else {
            return;
        };
        let current_runtime = self
            .playback
            .loudness_info
            .as_ref()
            .map(|snapshot| snapshot.integrated_control_instance_id);
        if current_runtime != Some(retryable.runtime_instance_id) {
            let translations = crate::i18n::TuiTranslations::for_language(self.ui.language);
            self.plugin_rack.retryable_loudness_control = None;
            self.plugin_rack.pending_loudness_control = None;
            self.plugin_rack.loudness_control_error = Some(
                translations
                    .ui("Monitor changed; request not confirmed")
                    .to_string(),
            );
            self.ui.needs_redraw = true;
            return;
        }
        self.request_loudness_control(retryable.operation);
    }

    pub fn reconcile_loudness_control_snapshot(
        &mut self,
        snapshot: Option<&sotf_audio::LoudnessData>,
    ) {
        let Some(request) = self.plugin_rack.retryable_loudness_control else {
            return;
        };
        let translations = crate::i18n::TuiTranslations::for_language(self.ui.language);
        let Some(snapshot) = snapshot else {
            self.plugin_rack.pending_loudness_control = None;
            self.plugin_rack.retryable_loudness_control = None;
            self.plugin_rack.loudness_control_error = Some(
                translations
                    .ui("Monitor changed; request not confirmed")
                    .to_string(),
            );
            self.ui.needs_redraw = true;
            return;
        };
        if snapshot.integrated_control_instance_id == 0
            || snapshot.integrated_control_instance_id != request.runtime_instance_id
        {
            self.plugin_rack.pending_loudness_control = None;
            self.plugin_rack.retryable_loudness_control = None;
            self.plugin_rack.loudness_control_error = Some(
                translations
                    .ui("Monitor changed; request not confirmed")
                    .to_string(),
            );
        } else if snapshot.integrated_control_request_id == request.request_id {
            self.plugin_rack.pending_loudness_control = None;
            self.plugin_rack.retryable_loudness_control = None;
            self.plugin_rack.loudness_control_error = None;
        } else if snapshot.integrated_control_request_id > request.request_id {
            self.plugin_rack.pending_loudness_control = None;
            self.plugin_rack.retryable_loudness_control = None;
            self.plugin_rack.loudness_control_error =
                Some(translations.ui("Request superseded").to_string());
        }
        self.ui.needs_redraw = true;
    }

    pub fn report_loudness_control_submission_error(&mut self, error: String) {
        let translations = crate::i18n::TuiTranslations::for_language(self.ui.language);
        self.plugin_rack.pending_loudness_control = None;
        self.plugin_rack.loudness_control_error = Some(format!(
            "{}: {error}",
            translations.ui("Control submission failed")
        ));
        self.ui.needs_redraw = true;
    }
}

#[cfg(test)]
mod integrated_control_tests {
    use super::*;
    use crate::theme::Theme;
    use sotf_audio::LoudnessData;
    use sotf_plugins::{LoudnessMonitorPlugin, ParameterId, ParameterValue, Plugin};
    use std::sync::Arc;

    fn snapshot(instance_id: u64, request_id: u64) -> LoudnessData {
        LoudnessData {
            integrated_control_instance_id: instance_id,
            integrated_control_request_id: request_id,
            integrated_measurement_running: false,
            ..LoudnessData::default()
        }
    }

    #[test]
    fn output_controls_wait_for_the_exact_receipt_and_reset_while_paused() {
        let mut app = App::new(Theme::default(), false);
        let initial = snapshot(41, 0);
        app.playback.loudness_info = Some(initial.clone());
        let output_engine_index = app
            .plugin_rack
            .graph
            .output_monitor_engine_index()
            .expect("default graph includes an output monitor");

        app.request_loudness_control(LoudnessControlOperation::Reset);
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.plugin_index),
            Some(output_engine_index)
        );
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.value.as_str()),
            Some("41:1:reset")
        );
        assert_eq!(
            app.plugin_rack.pending_loudness_control,
            Some(PendingLoudnessControl {
                runtime_instance_id: 41,
                request_id: 1,
                operation: LoudnessControlOperation::Reset,
            })
        );

        // A stale snapshot and a later request's lower predecessor cannot
        // acknowledge the action. The reset remains available while paused.
        app.plugin_rack.pending_param_update = None;
        app.reconcile_loudness_control_snapshot(Some(&initial));
        assert!(app.plugin_rack.pending_loudness_control.is_some());
        let mut first_ack = snapshot(41, 1);
        app.reconcile_loudness_control_snapshot(Some(&first_ack));
        assert!(app.plugin_rack.pending_loudness_control.is_none());

        app.request_loudness_control(LoudnessControlOperation::Reset);
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.value.as_str()),
            Some("41:2:reset")
        );
        app.plugin_rack.pending_param_update = None;
        app.reconcile_loudness_control_snapshot(Some(&first_ack));
        assert_eq!(
            app.plugin_rack
                .pending_loudness_control
                .map(|pending| pending.request_id),
            Some(2),
            "a retained earlier receipt cannot acknowledge the newer reset"
        );
        first_ack.integrated_control_request_id = 2;
        app.reconcile_loudness_control_snapshot(Some(&first_ack));
        assert!(app.plugin_rack.pending_loudness_control.is_none());
        assert!(app.plugin_rack.retryable_loudness_control.is_none());
        app.retry_loudness_control();
        assert!(app.plugin_rack.pending_param_update.is_none());
        assert_eq!(app.plugin_rack.loudness_control_next_request_id, 2);
    }

    #[test]
    fn replacement_monitor_cancels_a_pending_request_with_visible_error() {
        let mut app = App::new(Theme::default(), false);
        app.playback.loudness_info = Some(snapshot(52, 0));
        app.request_loudness_control(LoudnessControlOperation::Start);
        app.plugin_rack.pending_param_update = None;
        app.reconcile_loudness_control_snapshot(Some(&snapshot(53, 0)));
        assert!(app.plugin_rack.pending_loudness_control.is_none());
        assert_eq!(
            app.plugin_rack.loudness_control_error.as_deref(),
            Some("Monitor changed; request not confirmed")
        );
        assert!(app.plugin_rack.retryable_loudness_control.is_none());
        app.retry_loudness_control();
        assert!(app.plugin_rack.pending_param_update.is_none());
        assert_eq!(app.plugin_rack.loudness_control_next_request_id, 1);
    }

    #[test]
    fn missing_or_unidentified_monitor_cancels_pending_retry() {
        let mut app = App::new(Theme::default(), false);
        app.playback.loudness_info = Some(snapshot(54, 0));
        app.request_loudness_control(LoudnessControlOperation::Reset);
        app.plugin_rack.pending_param_update = None;

        app.reconcile_loudness_control_snapshot(None);
        assert!(app.plugin_rack.pending_loudness_control.is_none());
        assert!(app.plugin_rack.retryable_loudness_control.is_none());
        assert_eq!(
            app.plugin_rack.loudness_control_error.as_deref(),
            Some("Monitor changed; request not confirmed")
        );
        app.retry_loudness_control();
        assert!(app.plugin_rack.pending_param_update.is_none());
        assert_eq!(app.plugin_rack.loudness_control_next_request_id, 1);

        app.request_loudness_control(LoudnessControlOperation::Reset);
        app.plugin_rack.pending_param_update = None;
        app.reconcile_loudness_control_snapshot(Some(&snapshot(0, 0)));
        assert!(app.plugin_rack.pending_loudness_control.is_none());
        assert!(app.plugin_rack.retryable_loudness_control.is_none());
        assert_eq!(
            app.plugin_rack.loudness_control_error.as_deref(),
            Some("Monitor changed; request not confirmed")
        );
    }

    #[test]
    fn failed_request_retries_only_on_its_original_monitor_with_a_fresh_id() {
        let mut app = App::new(Theme::default(), false);
        app.playback.loudness_info = Some(snapshot(61, 0));
        app.request_loudness_control(LoudnessControlOperation::Pause);
        app.plugin_rack.pending_param_update = None;
        app.report_loudness_control_submission_error("temporary failure".to_string());
        assert!(app.plugin_rack.pending_loudness_control.is_none());
        assert!(app.plugin_rack.retryable_loudness_control.is_some());

        app.retry_loudness_control();
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.value.as_str()),
            Some("61:2:pause")
        );
        assert_eq!(
            app.plugin_rack
                .pending_loudness_control
                .map(|pending| pending.runtime_instance_id),
            Some(61)
        );
    }

    #[test]
    fn level_meter_keys_dispatch_lifecycle_controls_to_the_output_monitor() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::new(Theme::default(), false);
        app.current_screen = crate::app::Screen::Queue;
        app.input_mode = crate::app::InputMode::LevelMeters;
        app.playback.loudness_info = Some(snapshot(71, 0));
        let output_engine_index = app
            .plugin_rack
            .graph
            .output_monitor_engine_index()
            .expect("default graph includes an output monitor");

        for (key, operation, expected_value) in [
            ('i', LoudnessControlOperation::Start, "71:1:start"),
            ('p', LoudnessControlOperation::Pause, "71:2:pause"),
            ('o', LoudnessControlOperation::Continue, "71:3:continue"),
            ('r', LoudnessControlOperation::Reset, "71:4:reset"),
        ] {
            let result = crate::events::handle_key_event(
                &mut app,
                KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
            );
            assert!(result.is_none());
            assert_eq!(
                app.plugin_rack
                    .pending_param_update
                    .as_ref()
                    .map(|update| update.plugin_index),
                Some(output_engine_index)
            );
            assert_eq!(
                app.plugin_rack
                    .pending_param_update
                    .as_ref()
                    .map(|update| update.value.as_str()),
                Some(expected_value)
            );
            app.plugin_rack.pending_param_update = None;
            app.reconcile_loudness_control_snapshot(Some(&snapshot(
                71,
                request_id(expected_value),
            )));
            assert_eq!(
                app.plugin_rack.retryable_loudness_control, None,
                "acknowledged {operation:?} is no longer retryable"
            );
        }

        app.request_loudness_control(LoudnessControlOperation::Pause);
        app.plugin_rack.pending_param_update = None;
        app.report_loudness_control_submission_error("temporary failure".to_string());
        let retry = crate::events::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );
        assert!(retry.is_none());
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.plugin_index),
            Some(output_engine_index)
        );
        assert_eq!(
            app.plugin_rack
                .pending_param_update
                .as_ref()
                .map(|update| update.value.as_str()),
            Some("71:6:pause")
        );

        app.plugin_rack.pending_param_update = None;
        let before = app.plugin_rack.loudness_control_next_request_id;

        // On the Queue screen the same key is inert when the meter pane is not
        // focused.
        app.input_mode = crate::app::InputMode::Normal;
        let unfocused = crate::events::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        );
        assert!(unfocused.is_none());
        assert_eq!(app.plugin_rack.loudness_control_next_request_id, before);
        assert!(app.plugin_rack.pending_param_update.is_none());

        // Configure owns the keyboard focus on another screen; its key
        // context must not emit a LevelMeters command.
        app.current_screen = crate::app::Screen::Configure;
        app.input_mode = crate::app::InputMode::Configure;
        let wrong_screen = crate::events::handle_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        );
        assert!(wrong_screen.is_none());
        assert_eq!(app.plugin_rack.loudness_control_next_request_id, before);
        assert!(app.plugin_rack.pending_param_update.is_none());
    }

    #[test]
    fn level_meter_key_commands_reach_only_the_output_host_and_acknowledge_real_receipts() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        const RATE: u32 = 48_000;
        let mut input_host = LoudnessMonitorPlugin::new(2).unwrap();
        input_host.initialize(f64::from(RATE)).unwrap();
        let mut output_host = LoudnessMonitorPlugin::new(2).unwrap();
        output_host.initialize(f64::from(RATE)).unwrap();
        let input_id = input_host.integrated_control_instance_id();
        let output_id = output_host.integrated_control_instance_id();
        assert_ne!(input_id, output_id);

        let mut app = App::new(Theme::default(), false);
        app.current_screen = crate::app::Screen::Queue;
        app.input_mode = crate::app::InputMode::LevelMeters;
        let initial_output: Arc<LoudnessData> = output_host.get_data().unwrap().downcast().unwrap();
        app.playback.loudness_info = Some((*initial_output).clone());
        drop(initial_output);

        let output_index = app
            .plugin_rack
            .graph
            .output_monitor_engine_index()
            .expect("default graph includes an output monitor");
        let input_index = app
            .plugin_rack
            .graph
            .input_monitor_engine_index()
            .expect("default graph includes an input monitor");
        assert_ne!(input_index, output_index);
        let mut hosts = MeterHostRoute {
            input: input_host,
            output: output_host,
            input_id,
            output_id,
            input_index,
            output_index,
        };

        for (key, expected_request, expected_running) in [
            ('i', 1, true),  // Start
            ('p', 2, false), // Pause
            ('r', 3, false), // Reset preserves the paused state
            ('o', 4, true),  // Continue
        ] {
            assert!(
                crate::events::handle_key_event(
                    &mut app,
                    KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
                )
                .is_none()
            );
            hosts.apply_pending(&mut app, expected_request, expected_running);
            assert!(app.plugin_rack.retryable_loudness_control.is_none());
        }

        // Exercise the real host rejection path, then retry the same UI action
        // with a new request ID. The host receipt remains at 4 until retry 6.
        assert!(
            crate::events::handle_key_event(
                &mut app,
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
            )
            .is_none()
        );
        let failed = app.plugin_rack.pending_param_update.take().unwrap();
        assert_eq!(failed.plugin_index, output_index);
        let malformed = format!("{output_id}:5:unknown");
        let error = hosts
            .output
            .set_parameter(
                ParameterId::from(failed.param_id),
                ParameterValue::String(malformed),
            )
            .expect_err("the output host must reject an unknown operation");
        app.report_loudness_control_submission_error(error.to_string());
        let before_retry: Arc<LoudnessData> = hosts.output.get_data().unwrap().downcast().unwrap();
        assert_eq!(before_retry.integrated_control_request_id, 4);
        assert!(before_retry.integrated_measurement_running);
        drop(before_retry);

        assert!(
            crate::events::handle_key_event(
                &mut app,
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            )
            .is_none()
        );
        hosts.apply_pending(&mut app, 6, false);
        assert!(app.plugin_rack.retryable_loudness_control.is_none());
    }

    struct MeterHostRoute {
        input: LoudnessMonitorPlugin,
        output: LoudnessMonitorPlugin,
        input_id: u64,
        output_id: u64,
        input_index: usize,
        output_index: usize,
    }

    impl MeterHostRoute {
        fn apply_pending(&mut self, app: &mut App, expected_request: u64, expected_running: bool) {
            let update = app
                .plugin_rack
                .pending_param_update
                .take()
                .expect("the focused key should queue one host command");
            assert_eq!(update.plugin_index, self.output_index);
            assert_ne!(update.plugin_index, self.input_index);
            assert_eq!(update.param_id, "integrated_control_command");
            self.output
                .set_parameter(
                    ParameterId::from(update.param_id),
                    ParameterValue::String(update.value),
                )
                .unwrap();

            let output_data: Arc<LoudnessData> =
                self.output.get_data().unwrap().downcast().unwrap();
            assert_eq!(output_data.integrated_control_instance_id, self.output_id);
            assert_eq!(output_data.integrated_control_request_id, expected_request);
            assert_eq!(output_data.integrated_measurement_running, expected_running);
            app.reconcile_loudness_control_snapshot(Some(&output_data));
            app.playback.loudness_info = Some((*output_data).clone());
            drop(output_data);

            let input_data: Arc<LoudnessData> = self.input.get_data().unwrap().downcast().unwrap();
            assert_eq!(input_data.integrated_control_instance_id, self.input_id);
            assert_eq!(input_data.integrated_control_request_id, 0);
            assert!(input_data.integrated_measurement_running);
        }
    }

    fn request_id(command: &str) -> u64 {
        command
            .split(':')
            .nth(1)
            .expect("command carries a request ID")
            .parse()
            .expect("request ID is numeric")
    }
}
