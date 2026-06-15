        // Level meter controls (Shift + arrow keys)
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_previous_level_meter_group();
            Some(None)
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_next_level_meter_group();
            Some(None)
        }
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_previous_level_meter_control();
            Some(None)
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.select_next_level_meter_control();
            Some(None)
        }
        KeyCode::Char('M') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.input_mode = InputMode::LevelMeters;
            Some(None)
        }
        KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.toggle_level_meter_solo();
            Some(None)
        }

        // ReplayGain toggle
        KeyCode::Char('g') => {
            app.playback.replay_gain_enabled = !app.playback.replay_gain_enabled;
            let mode_str = if app.playback.replay_gain_enabled {
                match app.playback.replay_gain_mode {
                    crate::app::ReplayGainMode::Track => "ON (Track mode)",
                    crate::app::ReplayGainMode::Album => "ON (Album mode)",
                }
            } else {
                "OFF"
            };
            app.ui.status_message = Some(format!("ReplayGain: {}", mode_str));
            if app.playback.is_playing {
                app.plugin_rack.needs_update = true;
            }
            Some(None)
        }
        // ReplayGain mode cycle
        KeyCode::Char('G') => {
            use crate::app::ReplayGainMode;
            app.playback.replay_gain_mode = match app.playback.replay_gain_mode {
                ReplayGainMode::Track => ReplayGainMode::Album,
                ReplayGainMode::Album => ReplayGainMode::Track,
            };
            let mode_str = match app.playback.replay_gain_mode {
                ReplayGainMode::Track => "Track",
                ReplayGainMode::Album => "Album",
            };
            app.ui.status_message = Some(format!("ReplayGain mode: {}", mode_str));
            if app.playback.is_playing && app.playback.replay_gain_enabled {
                app.plugin_rack.needs_update = true;
            }
            Some(None)
        }


        // Output device selection with Ctrl+Arrow keys
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_next_output_device();
            Some(
                app.get_selected_output_device()
                    .map(|device| PlayerCommand::SetOutputDevice(device.name.clone())),
            )
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.select_previous_output_device();
            Some(
                app.get_selected_output_device()
                    .map(|device| PlayerCommand::SetOutputDevice(device.name.clone())),
            )
        }

