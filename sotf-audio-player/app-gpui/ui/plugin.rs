impl PlayerView {
    /// Apply a pending plugin update to the audio engine.
    /// Called from the timer callback when there's a pending update.
    fn apply_plugin_update(state: &mut AppState, update_type: PluginUpdateType) {
        let result = match update_type {
            PluginUpdateType::Parameter {
                plugin_index,
                param_index,
            } => {
                // Zero-dropout individual parameter update
                if let Some(plugin) = state.app.plugin_chain.get_plugin(plugin_index) {
                    // We must map the UI index to the Engine index because the Engine reorders plugins
                    // (analyzers moved to the end) and filters out disabled ones.
                    if let Some(engine_index) =
                        state.app.plugin_chain.get_engine_index(plugin_index)
                    {
                        if let Some((param_id, value)) =
                            param_index_to_engine_param(&plugin.settings, param_index)
                        {
                            state
                                .player
                                .lock()
                                .set_plugin_parameter(engine_index, param_id, value)
                        } else {
                            // Parameter not supported for individual update, fall back to structural
                            let sample_rate = 48000.0;
                            let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                            state.player.lock().update_plugins(plugins)
                        }
                    } else {
                        // Plugin is disabled or not found in engine map - ignore or full update
                        Ok(())
                    }
                } else {
                    Ok(()) // Plugin not found, ignore
                }
            }
            PluginUpdateType::Structural => {
                // Full plugin chain rebuild
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                log::warn!(
                    "[GPUI] Structural update: sending {} plugins to engine (expected output: {} channels)",
                    plugins.len(),
                    state.app.plugin_chain.output_channels()
                );
                state.player.lock().update_plugins(plugins)
            }
        };

        if let Err(e) = result {
            log::warn!("Failed to apply plugin update: {}", e);
        }
    }
    fn move_plugin_up(&mut self, _: &MovePluginUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_up(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn move_plugin_down(&mut self, _: &MovePluginDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_down(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn toggle_plugin(&mut self, _: &TogglePlugin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_plugin(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    // Quick plugin add shortcuts
    fn quick_add_eq(&mut self, _: &QuickAddEQ, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.add_plugin(&sotf_audio_player::PluginType::EQ);
        });
        cx.notify();
    }

    fn quick_add_upmixer(&mut self, _: &QuickAddUpmixer, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Upmixer);
        });
        cx.notify();
    }

    fn quick_add_compressor(
        &mut self,
        _: &QuickAddCompressor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Compressor);
        });
        cx.notify();
    }

    fn quick_add_gate(&mut self, _: &QuickAddGate, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.add_plugin(&sotf_audio_player::PluginType::Gate);
        });
        cx.notify();
    }

    fn quick_add_limiter(&mut self, _: &QuickAddLimiter, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::Limiter);
        });
        cx.notify();
    }

    fn quick_add_loudness(&mut self, _: &QuickAddLoudness, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::LoudnessCompensation);
        });
        cx.notify();
    }

    fn quick_add_binaural(&mut self, _: &QuickAddBinaural, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .add_plugin(&sotf_audio_player::PluginType::BinauralDecoder);
        });
        cx.notify();
    }

    // Level meter actions
    fn select_next_meter_group(
        &mut self,
        _: &SelectNextMeterGroup,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.select_next_level_meter_group();
        });
        cx.notify();
    }

    fn select_prev_meter_group(
        &mut self,
        _: &SelectPrevMeterGroup,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.select_previous_level_meter_group();
        });
        cx.notify();
    }

    fn toggle_meter_mute(&mut self, _: &ToggleMeterMute, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_mute();
        });
        cx.notify();
    }

    fn toggle_meter_solo(&mut self, _: &ToggleMeterSolo, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_solo();
        });
        cx.notify();
    }

    fn toggle_meter_dim(&mut self, _: &ToggleMeterDim, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_level_meter_dim();
        });
        cx.notify();
    }

    fn clear_meter_mutes_solos(
        &mut self,
        _: &ClearMeterMutesSolos,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.clear_level_meter_mutes_and_solos();
        });
        cx.notify();
    }


}
