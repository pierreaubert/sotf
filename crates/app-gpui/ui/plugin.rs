// Macro for quick-add plugin handlers - reduces boilerplate for identical patterns
macro_rules! quick_add_plugin_handler {
    ($fn_name:ident, $action:ty, $plugin_type:expr) => {
        #[allow(dead_code)]
        fn $fn_name(&mut self, _: &$action, _: &mut Window, cx: &mut Context<Self>) {
            self.state.update(cx, |state, _cx| {
                if state.app.is_plugin_available(&$plugin_type) {
                    state.app.add_plugin(&$plugin_type);
                }
            });
            cx.notify();
        }
    };
}

// Macro for simple state method handlers - reduces boilerplate for action→method patterns
macro_rules! state_method_handler {
    ($fn_name:ident, $action:ty, $method:ident) => {
        fn $fn_name(&mut self, _: &$action, _: &mut Window, cx: &mut Context<Self>) {
            self.state.update(cx, |state, _cx| {
                state.app.$method();
            });
            cx.notify();
        }
    };
}

impl PlayerView {
    /// Apply a pending plugin update to the audio engine.
    /// Called from the timer callback when there's a pending update.
    fn apply_plugin_update(state: &mut AppState, update_type: PluginUpdateType) {
        let plugin_state_snapshot = match update_type {
            PluginUpdateType::Structural => Some(state.app.plugin_state.clone()),
            PluginUpdateType::Parameter { .. } | PluginUpdateType::ParameterByNodeId { .. } => {
                None
            }
        };

        let result = match update_type {
            PluginUpdateType::Parameter {
                plugin_index,
                param_index,
            } => {
                // Zero-dropout individual parameter update
                if let Some(plugin) = state.app.plugin_state.graph.get_plugin(plugin_index) {
                    // We must map the UI index to the Engine index because the Engine reorders plugins
                    // (analyzers moved to the end) and filters out disabled ones.
                    if let Some(engine_index) =
                        state.app.plugin_state.graph.get_engine_index_by_linear_position(plugin_index)
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
                            let device_name = state.app.audio_device_state.current_output_device_name.as_deref();
                            let track_sample_rate = state.app.playback.sample_rate.unwrap_or(48000);
                            let sample_rate = sotf_audio::select_output_sample_rate(track_sample_rate, device_name) as f64;
                            let plugins = state.app.plugin_state.graph.to_plugin_configs(sample_rate);
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
            PluginUpdateType::ParameterByNodeId {
                node_id,
                param_index,
            } => {
                // Zero-dropout parameter update via graph node ID (works for non-linear graphs)
                if let Some(node) = state.app.plugin_state.graph.nodes.get(&node_id) {
                    if let Some(engine_index) =
                        state.app.plugin_state.graph.get_engine_index(node_id)
                    {
                        if let Some((param_id, value)) =
                            param_index_to_engine_param(&node.plugin.settings, param_index)
                        {
                            state
                                .player
                                .lock()
                                .set_plugin_parameter(engine_index, param_id, value)
                        } else {
                            // Fall back to structural rebuild
                            let device_name = state.app.audio_device_state.current_output_device_name.as_deref();
                            let track_sample_rate = state.app.playback.sample_rate.unwrap_or(48000);
                            let sample_rate = sotf_audio::select_output_sample_rate(track_sample_rate, device_name) as f64;
                            let plugins = state.app.plugin_state.graph.to_plugin_configs(sample_rate);
                            state.player.lock().update_plugins(plugins)
                        }
                    } else {
                        Ok(()) // Node disabled or not in engine
                    }
                } else {
                    Ok(()) // Node not found
                }
            }
            PluginUpdateType::Structural => {
                // Full plugin chain rebuild
                let device_name = state.app.audio_device_state.current_output_device_name.as_deref();
                let track_sample_rate = state.app.playback.sample_rate.unwrap_or(48000);
                let sample_rate = sotf_audio::select_output_sample_rate(track_sample_rate, device_name) as f64;
                let plugins = state.app.plugin_state.graph.to_plugin_configs(sample_rate);
                log::warn!(
                    "[GPUI] Structural update: sending {} plugins to engine (expected output: {} channels) at {}Hz",
                    plugins.len(),
                    state.app.plugin_state.graph.output_channels(),
                    sample_rate
                );
                // Invalidate the workflow canvas so the graph view rebuilds
                state.app.plugin_state.workflow_canvas = None;
                state.player.lock().update_plugins(plugins)
            }
        };

        if let Err(e) = result {
            log::warn!("Failed to apply plugin update: {}", e);
            if let Some(snapshot) = plugin_state_snapshot {
                state.app.rollback_failed_plugin_update(snapshot, e.to_string());
            } else {
                state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(format!(
                    "Plugin update failed: {}",
                    e
                )));
            }
        }
    }

    fn move_plugin_up(&mut self, _: &MovePluginUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_up(state.app.plugin_state.selected_plugin_index);
        });
        cx.notify();
    }

    fn move_plugin_down(&mut self, _: &MovePluginDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_down(state.app.plugin_state.selected_plugin_index);
        });
        cx.notify();
    }

    fn toggle_plugin(&mut self, _: &TogglePlugin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_plugin(state.app.plugin_state.selected_plugin_index);
        });
        cx.notify();
    }

    // Quick plugin add shortcuts generated by macro
    quick_add_plugin_handler!(quick_add_eq, QuickAddEQ, sotf_audio_player::PluginType::EQ);
    quick_add_plugin_handler!(quick_add_gain, QuickAddGain, sotf_audio_player::PluginType::Gain);
    quick_add_plugin_handler!(quick_add_upmixer, QuickAddUpmixer, sotf_audio_player::PluginType::Upmixer);
    quick_add_plugin_handler!(quick_add_aae, QuickAddAAE, sotf_audio_player::PluginType::AAE);
    quick_add_plugin_handler!(quick_add_compressor, QuickAddCompressor, sotf_audio_player::PluginType::Compressor);
    quick_add_plugin_handler!(quick_add_gate, QuickAddGate, sotf_audio_player::PluginType::Gate);
    quick_add_plugin_handler!(quick_add_limiter, QuickAddLimiter, sotf_audio_player::PluginType::Limiter);
    quick_add_plugin_handler!(quick_add_expander, QuickAddExpander, sotf_audio_player::PluginType::Expander);
    quick_add_plugin_handler!(quick_add_mbcomp, QuickAddMultibandCompressor, sotf_audio_player::PluginType::MultibandCompressor);
    quick_add_plugin_handler!(quick_add_mbexp, QuickAddMultibandExpander, sotf_audio_player::PluginType::MultibandExpander);
    quick_add_plugin_handler!(quick_add_loudness, QuickAddLoudness, sotf_audio_player::PluginType::LoudnessCompensation);
    quick_add_plugin_handler!(quick_add_fletcher, QuickAddFletcherMunson, sotf_audio_player::PluginType::FletcherMunson);
    quick_add_plugin_handler!(quick_add_binaural, QuickAddBinaural, sotf_audio_player::PluginType::BinauralDecoder);
    quick_add_plugin_handler!(quick_add_convolution, QuickAddConvolution, sotf_audio_player::PluginType::Convolution);
    quick_add_plugin_handler!(quick_add_loudness_monitor, QuickAddLoudnessMonitor, sotf_audio_player::PluginType::LoudnessMonitor);
    quick_add_plugin_handler!(quick_add_spectrum, QuickAddSpectrum, sotf_audio_player::PluginType::SpectrumAnalyzer);
    quick_add_plugin_handler!(quick_add_mutesolo, QuickAddMuteSolo, sotf_audio_player::PluginType::ChannelMuteSolo);
    quick_add_plugin_handler!(quick_add_xtc, QuickAddXTC, sotf_audio_player::PluginType::XTC);
    quick_add_plugin_handler!(quick_add_denoiser, QuickAddDenoiser, sotf_audio_player::PluginType::Denoiser);
    quick_add_plugin_handler!(quick_add_pnd, QuickAddPnd, sotf_audio_player::PluginType::Pnd);
    quick_add_plugin_handler!(quick_add_ab_compare, QuickAddABCompare, sotf_audio_player::PluginType::ABCompare);
    quick_add_plugin_handler!(quick_add_downmix, QuickAddDownmix, sotf_audio_player::PluginType::Downmix);
    quick_add_plugin_handler!(quick_add_mono_to_stereo, QuickAddMonoToStereo, sotf_audio_player::PluginType::MonoToStereo);
    quick_add_plugin_handler!(quick_add_band_split, QuickAddBandSplit, sotf_audio_player::PluginType::BandSplit);
    quick_add_plugin_handler!(quick_add_band_merge, QuickAddBandMerge, sotf_audio_player::PluginType::BandMerge);
    quick_add_plugin_handler!(quick_add_crossfeed, QuickAddCrossfeed, sotf_audio_player::PluginType::Crossfeed);
    quick_add_plugin_handler!(quick_add_delay, QuickAddDelay, sotf_audio_player::PluginType::Delay);
    quick_add_plugin_handler!(quick_add_aec, QuickAddAec, sotf_audio_player::PluginType::Aec);
    quick_add_plugin_handler!(quick_add_beamformer, QuickAddBeamformer, sotf_audio_player::PluginType::Beamformer);
    quick_add_plugin_handler!(quick_add_transient_shaper, QuickAddTransientShaper, sotf_audio_player::PluginType::TransientShaper);
    quick_add_plugin_handler!(quick_add_saturation, QuickAddSaturation, sotf_audio_player::PluginType::Saturation);
    quick_add_plugin_handler!(quick_add_dynamic_eq, QuickAddDynamicEq, sotf_audio_player::PluginType::DynamicEq);
    quick_add_plugin_handler!(quick_add_linear_phase_eq, QuickAddLinearPhaseEq, sotf_audio_player::PluginType::LinearPhaseEq);
    quick_add_plugin_handler!(quick_add_spectral_compressor, QuickAddSpectralCompressor, sotf_audio_player::PluginType::SpectralCompressor);

    fn increment_plugin_param(&mut self, _: &IncrementPluginParam, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(1.0);
        });
        cx.notify();
    }

    fn decrement_plugin_param(&mut self, _: &DecrementPluginParam, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(-1.0);
        });
        cx.notify();
    }

    fn increment_plugin_param_large(&mut self, _: &IncrementPluginParamLarge, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(10.0);
        });
        cx.notify();
    }

    fn decrement_plugin_param_large(&mut self, _: &DecrementPluginParamLarge, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(-10.0);
        });
        cx.notify();
    }

    fn increment_plugin_param_small(&mut self, _: &IncrementPluginParamSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(0.1);
        });
        cx.notify();
    }

    fn decrement_plugin_param_small(&mut self, _: &DecrementPluginParamSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.adjust_selected_param(-0.1);
        });
        cx.notify();
    }

    fn select_band_global(&mut self, _: &SelectBandGlobal, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 0; });
        cx.notify();
    }
    fn select_band_1(&mut self, _: &SelectBand1, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 1; });
        cx.notify();
    }
    fn select_band_2(&mut self, _: &SelectBand2, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 2; });
        cx.notify();
    }
    fn select_band_3(&mut self, _: &SelectBand3, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 3; });
        cx.notify();
    }
    fn select_band_4(&mut self, _: &SelectBand4, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 4; });
        cx.notify();
    }
    fn select_band_5(&mut self, _: &SelectBand5, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| { state.app.plugin_state.selected_eq_band = 5; });
        cx.notify();
    }

    fn select_next_eq_band(
        &mut self,
        _: &SelectNextEqBand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.plugin_state.selected_eq_band += 1;
        });
        cx.notify();
    }

    fn select_prev_eq_band(
        &mut self,
        _: &SelectPrevEqBand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.plugin_state.selected_eq_band =
                state.app.plugin_state.selected_eq_band.saturating_sub(1);
        });
        cx.notify();
    }

    fn toggle_simple_view(&mut self, _: &ToggleSimpleView, _: &mut Window, cx: &mut Context<Self>) {
        use crate::app::state::plugin::PluginUiView;
        self.state.update(cx, |state, _cx| {
            state.app.plugin_state.plugin_ui_view =
                if state.app.plugin_state.plugin_ui_view.is_simple() {
                    PluginUiView::UI
                } else {
                    PluginUiView::Simple
                };
        });
        cx.notify();
    }

    // Level meter actions generated by macro
    state_method_handler!(select_next_meter_group, SelectNextMeterGroup, select_next_level_meter_group);
    state_method_handler!(select_prev_meter_group, SelectPrevMeterGroup, select_previous_level_meter_group);
    state_method_handler!(toggle_meter_mute, ToggleMeterMute, toggle_level_meter_mute);
    state_method_handler!(toggle_meter_solo, ToggleMeterSolo, toggle_level_meter_solo);
    state_method_handler!(toggle_meter_dim, ToggleMeterDim, toggle_level_meter_dim);
    state_method_handler!(clear_meter_mutes_solos, ClearMeterMutesSolos, clear_level_meter_mutes_and_solos);
}
