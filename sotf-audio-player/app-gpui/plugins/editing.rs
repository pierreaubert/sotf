//! Plugin management and editing methods.
//!
//! Contains methods for plugin chain management, parameter editing, and presets.

use sotf_audio_player::PluginSettings;

use crate::app::{App, InputMode, ToastMessage};

impl App {
    // Plugin management methods
    pub fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType) {
        let new_index = self.plugin_chain.add_plugin(plugin_type);
        self.selected_plugin_index = new_index;
        self.plugin_chain.update_binaural_decoder_channels();
        self.needs_plugin_update = true;
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_chain.toggle_plugin(index);
        // Update BinauralDecoder input channels after toggle
        self.plugin_chain.update_binaural_decoder_channels();
        self.needs_plugin_update = true;
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if index > 0 {
            self.plugin_chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if index < self.plugin_chain.len() - 1 {
            self.plugin_chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
        }
    }

    pub fn select_next_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.plugin_chain.len();
        }
    }

    pub fn select_previous_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.plugin_chain.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    pub fn remove_plugin(&mut self, index: usize) {
        if index < self.plugin_chain.len() {
            self.plugin_chain.remove_plugin(index);
            // Update BinauralDecoder input channels after removal
            self.plugin_chain.update_binaural_decoder_channels();
            self.needs_plugin_update = true;
            // Adjust selection
            if self.selected_plugin_index >= self.plugin_chain.len()
                && self.selected_plugin_index > 0
            {
                self.selected_plugin_index = self.plugin_chain.len() - 1;
            }
        }
    }

    // Plugin editing methods
    pub fn enter_plugin_edit_mode(&mut self) {
        if self.selected_plugin_index < self.plugin_chain.len() {
            self.editing_plugin_index = Some(self.selected_plugin_index);
            self.plugin_param_selection = 0;
            self.input_mode = InputMode::EditPlugin;
        }
    }

    pub fn exit_plugin_edit_mode(&mut self) {
        self.editing_plugin_index = None;
        self.plugin_param_selection = 0;
        self.input_mode = InputMode::Normal;
    }

    pub fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin(idx))
    }

    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin_mut(idx))
    }

    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

    pub fn select_previous_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                if self.plugin_param_selection == 0 {
                    self.plugin_param_selection = param_count - 1;
                } else {
                    self.plugin_param_selection -= 1;
                }
            }
        }
    }

    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    pub fn adjust_selected_param(&mut self, delta: f64) -> bool {
        let param_idx = self.plugin_param_selection;
        let mut channel_count_changed = false;

        let result = if let Some(plugin) = self.get_editing_plugin_mut() {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    speaker_config,
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    lfe_cutoff_hz,
                    stereo_width,
                    bandpass_hz,
                    height_gain,
                    lfe_gain,
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    enable_hr_direct,
                    hr_sharpen,
                    safety_cap_db,
                    decorrelation_mode,
                } => {
                    match param_idx {
                        0 => {
                            // speaker_config: cycle through available configs
                            let configs = [
                                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                "9.1.4", "9.1.6",
                            ];
                            let current_idx = configs
                                .iter()
                                .position(|&c| c == speaker_config.as_str())
                                .unwrap_or(0);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % configs.len()
                            } else {
                                if current_idx == 0 {
                                    configs.len() - 1
                                } else {
                                    current_idx - 1
                                }
                            };
                            *speaker_config = configs[new_idx].to_string();
                            channel_count_changed = true;
                            true
                        }
                        1 => {
                            *gain_front_direct =
                                (*gain_front_direct + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        2 => {
                            *gain_front_ambient =
                                (*gain_front_ambient + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        3 => {
                            *gain_rear_ambient =
                                (*gain_rear_ambient + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        4 => {
                            *lfe_cutoff_hz =
                                (*lfe_cutoff_hz + delta as f64 * 10.0).max(20.0).min(200.0);
                            true
                        }
                        5 => {
                            *stereo_width = (*stereo_width + delta as f64 * 0.1).max(0.0).min(2.0);
                            true
                        }
                        6 => {
                            *bandpass_hz =
                                (*bandpass_hz + delta as f64 * 50.0).max(100.0).min(1000.0);
                            true
                        }
                        7 => {
                            *height_gain = (*height_gain + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        8 => {
                            *lfe_gain = (*lfe_gain + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        9 => {
                            *enable_subharmonic_synth = !*enable_subharmonic_synth;
                            true
                        }
                        10 => {
                            *subharmonic_gain =
                                (*subharmonic_gain + delta as f64).max(-30.0).min(30.0);
                            true
                        }
                        11 => {
                            *enable_hr_direct = !*enable_hr_direct;
                            true
                        }
                        12 => {
                            *hr_sharpen = (*hr_sharpen + delta as f64 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        13 => {
                            *safety_cap_db =
                                (*safety_cap_db + delta as f64 * 0.5).max(0.0).min(12.0);
                            true
                        }
                        14 => {
                            // Toggle decorrelation mode (0 or 1)
                            if delta.abs() > 0.1 {
                                *decorrelation_mode = if *decorrelation_mode == 0 { 1 } else { 0 };
                            }
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Compressor {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    knee_db,
                    makeup_gain_db,
                    mix,
                    auto_makeup,
                    link_channels,
                    sidechain_hpf_hz,
                } => match param_idx {
                    0 => {
                        *threshold_db = (*threshold_db + delta as f64).max(-60.0).min(0.0);
                        true
                    }
                    1 => {
                        *ratio = (*ratio + delta as f64 * 0.1).max(1.0).min(20.0);
                        true
                    }
                    2 => {
                        *attack_ms = (*attack_ms + delta as f64 * 0.1).max(0.1).min(100.0);
                        true
                    }
                    3 => {
                        *release_ms = (*release_ms + delta as f64).max(1.0).min(1000.0);
                        true
                    }
                    4 => {
                        *knee_db = (*knee_db + delta as f64 * 0.1).max(0.0).min(12.0);
                        true
                    }
                    5 => {
                        *makeup_gain_db =
                            (*makeup_gain_db + delta as f64 * 0.1).max(-20.0).min(20.0);
                        true
                    }
                    6 => {
                        *mix = (*mix + delta as f64 * 0.01).max(0.0).min(1.0);
                        true
                    }
                    7 => {
                        *auto_makeup = !*auto_makeup;
                        true
                    }
                    8 => {
                        *link_channels = !*link_channels;
                        true
                    }
                    9 => {
                        *sidechain_hpf_hz = (*sidechain_hpf_hz + delta as f64).max(20.0).min(500.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::Limiter {
                    threshold_db,
                    release_ms,
                    mix,
                } => match param_idx {
                    0 => {
                        *threshold_db = (*threshold_db + delta as f64 * 0.1).max(-20.0).min(0.0);
                        true
                    }
                    1 => {
                        *release_ms = (*release_ms + delta as f64).max(1.0).min(500.0);
                        true
                    }
                    2 => {
                        *mix = (*mix + delta as f64 * 0.05).max(0.0).min(1.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => match param_idx {
                    0 => {
                        *threshold_db = (*threshold_db + delta as f64).max(-80.0).min(0.0);
                        true
                    }
                    1 => {
                        *ratio = (*ratio + delta as f64 * 0.1).max(1.0).min(100.0);
                        true
                    }
                    2 => {
                        *attack_ms = (*attack_ms + delta as f64 * 0.1).max(0.1).min(100.0);
                        true
                    }
                    3 => {
                        *release_ms = (*release_ms + delta as f64).max(1.0).min(1000.0);
                        true
                    }
                    4 => {
                        *mix = (*mix + delta as f64 * 0.05).max(0.0).min(1.0);
                        true
                    }
                    5 => {
                        *link_channels = !*link_channels;
                        true
                    }
                    6 => {
                        *sidechain_hpf_hz =
                            (*sidechain_hpf_hz + delta as f64 * 5.0).max(0.0).min(200.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::LoudnessCompensation {
                    target_lufs,
                    min_gain_db,
                    max_gain_db,
                } => match param_idx {
                    0 => {
                        *target_lufs = (*target_lufs + delta as f64).max(-40.0).min(0.0);
                        true
                    }
                    1 => {
                        *min_gain_db = (*min_gain_db + delta as f64).max(-20.0).min(0.0);
                        true
                    }
                    2 => {
                        *max_gain_db = (*max_gain_db + delta as f64).max(0.0).min(20.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::EQ { filters } => {
                    if filters.is_empty() {
                        return false;
                    }

                    let total_params = filters.len() * 4;
                    if param_idx >= total_params {
                        return false;
                    }

                    let filter_idx = param_idx / 4;
                    let field_idx = param_idx % 4;

                    if let Some(filter) = filters.get_mut(filter_idx) {
                        match field_idx {
                            0 => {
                                // Frequency
                                filter.frequency =
                                    (filter.frequency + delta * 10.0).max(20.0).min(20_000.0);
                                true
                            }
                            1 => {
                                // Q
                                filter.q = (filter.q + delta * 0.1).max(0.1).min(10.0);
                                true
                            }
                            2 => {
                                // Gain
                                filter.gain_db =
                                    (filter.gain_db + delta * 0.5).max(-24.0).min(24.0);
                                true
                            }
                            3 => {
                                // Filter type
                                use sotf_audio_player::BiquadFilterType;

                                let types = [
                                    BiquadFilterType::Peak,
                                    BiquadFilterType::Lowshelf,
                                    BiquadFilterType::Highshelf,
                                    BiquadFilterType::Lowpass,
                                    BiquadFilterType::Highpass,
                                    BiquadFilterType::Bandpass,
                                    BiquadFilterType::Notch,
                                ];

                                let current_idx = types
                                    .iter()
                                    .position(|t| *t == filter.filter_type)
                                    .unwrap_or(0);
                                let new_idx = if delta > 0.0 {
                                    (current_idx + 1) % types.len()
                                } else {
                                    if current_idx == 0 {
                                        types.len() - 1
                                    } else {
                                        current_idx - 1
                                    }
                                };
                                filter.filter_type = types[new_idx];
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                PluginSettings::BinauralDecoder {
                    input_channels,
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    ..
                } => {
                    // sofa_file (param 0) is set via file browser - not adjustable here
                    match param_idx {
                        1 => {
                            *input_channels =
                                ((*input_channels as i64) + delta as i64).max(2).min(16) as usize;
                            true
                        }
                        2 => {
                            *enable_optimization = !*enable_optimization;
                            true
                        }
                        3 => {
                            *externalization =
                                (*externalization + delta as f64 * 0.05).max(0.0).min(1.0);
                            true
                        }
                        4 => {
                            *near_field_strength = (*near_field_strength + delta as f64 * 0.05)
                                .max(0.0)
                                .min(1.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Convolution { .. } => {
                    // Convolution has no adjustable parameters for now
                    false
                }
                PluginSettings::LoudnessMonitor => {
                    // Analyzer plugin - no parameters to adjust
                    false
                }
                PluginSettings::SpectrumAnalyzer {
                    num_bins,
                    min_freq,
                    max_freq,
                    smoothing,
                } => match param_idx {
                    0 => {
                        *num_bins = (*num_bins as i64 + delta as i64).max(10).min(100) as usize;
                        true
                    }
                    1 => {
                        *min_freq = (*min_freq + delta as f32).max(10.0).min(100.0);
                        true
                    }
                    2 => {
                        *max_freq = (*max_freq + delta as f32 * 100.0).max(1000.0).min(24000.0);
                        true
                    }
                    3 => {
                        *smoothing = (*smoothing + delta as f32 * 0.01).max(0.0).min(1.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::Gain { gain_db } => match param_idx {
                    0 => {
                        *gain_db = (*gain_db + delta).clamp(-24.0, 24.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::ChannelMuteSolo { .. } => {
                    // No adjustable parameters (mute/solo are toggles, not adjustable)
                    false
                }
            }
        } else {
            false
        };

        if result && channel_count_changed {
            self.plugin_chain.update_binaural_decoder_channels();
        }

        if result {
            self.needs_plugin_update = true;
        }

        result
    }

    /// Set a specific parameter value for a plugin
    pub fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64) {
        let mut channel_count_changed = false;
        let mut update_needed = false;

        if let Some(plugin) = self.plugin_chain.get_plugin_mut(plugin_idx) {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    speaker_config,
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    lfe_cutoff_hz,
                    stereo_width,
                    bandpass_hz,
                    height_gain,
                    lfe_gain,
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    enable_hr_direct,
                    hr_sharpen,
                    safety_cap_db,
                    decorrelation_mode,
                } => {
                    match param_idx {
                        0 => {
                            // speaker_config: map value (index) to string
                            let configs = [
                                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                "9.1.4", "9.1.6",
                            ];
                            let idx = (value as usize).clamp(0, configs.len() - 1);
                            *speaker_config = configs[idx].to_string();
                            channel_count_changed = true;
                            update_needed = true;
                        }
                        1 => {
                            // gain_front_direct (dB)
                            *gain_front_direct = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        2 => {
                            // gain_front_ambient (Center Gain, dB)
                            *gain_front_ambient = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        3 => {
                            // gain_rear_ambient (Surround Gain, dB)
                            *gain_rear_ambient = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        4 => {
                            // lfe_cutoff_hz
                            *lfe_cutoff_hz = value.clamp(20.0, 200.0);
                            update_needed = true;
                        }
                        5 => {
                            // stereo_width
                            *stereo_width = value.clamp(0.0, 2.0);
                            update_needed = true;
                        }
                        6 => {
                            // bandpass_hz
                            *bandpass_hz = value.clamp(100.0, 1000.0);
                            update_needed = true;
                        }
                        7 => {
                            // height_gain (Top Gain, dB)
                            *height_gain = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        8 => {
                            // lfe_gain (LFE Gain, dB)
                            *lfe_gain = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        9 => {
                            // enable_subharmonic_synth (toggle)
                            *enable_subharmonic_synth = value > 0.5;
                            update_needed = true;
                        }
                        10 => {
                            // subharmonic_gain (dB)
                            *subharmonic_gain = value.clamp(-30.0, 30.0);
                            update_needed = true;
                        }
                        11 => {
                            // enable_hr_direct (toggle)
                            *enable_hr_direct = value > 0.5;
                            update_needed = true;
                        }
                        12 => {
                            // hr_sharpen
                            *hr_sharpen = value.clamp(0.0, 1.0);
                            update_needed = true;
                        }
                        13 => {
                            // safety_cap_db
                            *safety_cap_db = value.clamp(0.0, 12.0);
                            update_needed = true;
                        }
                        14 => {
                            // decorrelation_mode (0=Velvet/1=LFO)
                            *decorrelation_mode = if value > 0.5 { 1 } else { 0 };
                            update_needed = true;
                        }
                        _ => {}
                    }
                    if param_idx == 0 {
                        channel_count_changed = true;
                    }
                }
                PluginSettings::EQ { filters } => {
                    let filter_idx = param_idx / 4;
                    let field_idx = param_idx % 4;

                    if let Some(filter) = filters.get_mut(filter_idx) {
                        match field_idx {
                            0 => {
                                // Frequency
                                filter.frequency = value.clamp(20.0, 20_000.0);
                                update_needed = true;
                            }
                            1 => {
                                // Q
                                filter.q = value.clamp(0.1, 10.0);
                                update_needed = true;
                            }
                            2 => {
                                // Gain
                                filter.gain_db = value.clamp(-24.0, 24.0);
                                update_needed = true;
                            }
                            3 => {
                                // Filter type
                                // Map float value to enum index
                                use sotf_audio_player::BiquadFilterType;
                                let types = [
                                    BiquadFilterType::Peak,
                                    BiquadFilterType::Lowshelf,
                                    BiquadFilterType::Highshelf,
                                    BiquadFilterType::Lowpass,
                                    BiquadFilterType::Highpass,
                                    BiquadFilterType::Bandpass,
                                    BiquadFilterType::Notch,
                                ];
                                let type_idx = (value as usize).clamp(0, types.len() - 1);
                                filter.filter_type = types[type_idx];
                                update_needed = true;
                            }
                            _ => {}
                        }
                    }
                }
                // Implement other plugins as needed, Upmixer is priority
                PluginSettings::Gain { gain_db } => {
                    if param_idx == 0 {
                        *gain_db = value.clamp(-60.0, 12.0);
                        update_needed = true;
                    }
                }
                _ => {}
            }
        }

        if channel_count_changed {
            self.plugin_chain.update_binaural_decoder_channels();
        }

        if update_needed {
            self.needs_plugin_update = true;
        }
    }

    /// Reset a specific parameter to its default value
    pub fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize) {
        let plugin_type = if let Some(plugin) = self.plugin_chain.get_plugin(plugin_idx) {
            plugin.plugin_type()
        } else {
            return;
        };

        // Create default settings for this plugin type
        let default_settings = PluginSettings::default_for(&plugin_type);

        // Get the default value for the parameter
        let default_value = match &default_settings {
            PluginSettings::Upmixer {
                speaker_config: _,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                lfe_cutoff_hz,
                stereo_width,
                bandpass_hz,
                height_gain,
                lfe_gain,
                enable_subharmonic_synth,
                subharmonic_gain,
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db,
                decorrelation_mode,
            } => {
                match param_idx {
                    1 => *gain_front_direct,
                    2 => *gain_front_ambient,
                    3 => *gain_rear_ambient,
                    4 => *lfe_cutoff_hz,
                    5 => *stereo_width,
                    6 => *bandpass_hz,
                    7 => *height_gain,
                    8 => *lfe_gain,
                    9 => {
                        if *enable_subharmonic_synth {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    10 => *subharmonic_gain,
                    11 => {
                        if *enable_hr_direct {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    12 => *hr_sharpen,
                    13 => *safety_cap_db,
                    14 => *decorrelation_mode as f64,
                    _ => return, // No reset for others or unknown
                }
            }
            PluginSettings::Gain { gain_db } => {
                if param_idx == 0 {
                    *gain_db
                } else {
                    return;
                }
            }
            _ => return,
        };

        // Apply the value
        self.set_plugin_param(plugin_idx, param_idx, default_value);
    }

    /// Load EQ filters from APO file
    pub fn load_apo_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::EQFilter;
        use std::path::Path;

        // Check plugin state before loading file
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        let path = Path::new(&self.apo_file_input);

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if matches!(plugin.settings, PluginSettings::EQ { .. }) {
                plugin.settings = PluginSettings::EQ { filters };
                self.needs_plugin_update = true;
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Update SOFA file path for the currently editing binaural decoder plugin
    pub fn load_sofa_file(&mut self) -> Result<(), String> {
        // Check plugin state before loading file (or rather, setting path)
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                return Err("Selected plugin is not a Binaural Decoder".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        // Clone the sofa_file_input before borrowing plugin mutably
        let sofa_file_path = self.sofa_file_input.clone();

        // Update the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = sofa_file_path;
                self.needs_plugin_update = true;
                Ok(())
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    // Plugin preset save/load methods

    /// Refresh the list of available plugin presets from the config directory
    pub fn refresh_plugin_presets(&mut self) {
        self.available_plugin_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_plugin_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            // Sort presets alphabetically
            self.available_plugin_presets.sort();
        }

        log::info!(
            "Found {} plugin presets",
            self.available_plugin_presets.len()
        );
    }

    /// Save plugin chain to file
    pub fn save_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.toast_message = Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        // Check if file exists and show warning if overwriting
        let filename_with_ext = if self.plugin_file_input.ends_with(".json") {
            self.plugin_file_input.clone()
        } else {
            format!("{}.json", self.plugin_file_input)
        };

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
            let full_path = presets_dir.join(&filename_with_ext);
            if full_path.exists() {
                log::warn!("Overwriting existing preset: {}", filename_with_ext);
            }
        }

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        match self.plugin_chain.save_to_file(&self.plugin_file_input) {
            Ok(_) => {
                self.toast_message = Some(ToastMessage::success(format!(
                    "Saved preset: {}",
                    filename_with_ext
                )));
                self.last_loaded_preset = Some(filename_with_ext);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!("Error saving: {}", e)));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    /// Save plugin chain to selected preset file (overwrite)
    pub fn save_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.toast_message = Some(ToastMessage::error("No presets available".to_string()));
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            // Save using the plugin chain's own save method
            match self.plugin_chain.save_to_file(&preset_filename) {
                Ok(_) => {
                    self.toast_message = Some(ToastMessage::success(format!(
                        "Overwritten preset: {}",
                        preset_filename
                    )));
                    self.last_loaded_preset = Some(preset_filename);
                    // Refresh presets list
                    self.refresh_plugin_presets();
                }
                Err(e) => {
                    self.toast_message = Some(ToastMessage::error(format!("Error saving: {}", e)));
                    log::error!("Failed to save plugin chain: {}", e);
                }
            }
        }
    }

    /// Load plugin chain from file
    pub fn load_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.toast_message = Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        // Load using the plugin chain's own load method (handles path, extension, etc.)
        match self.plugin_chain.load_from_file(&self.plugin_file_input) {
            Ok(_) => {
                // Update BinauralDecoder input channels after loading
                self.plugin_chain.update_binaural_decoder_channels();

                // Get the final filename (with .json appended if needed)
                let filename = if self.plugin_file_input.ends_with(".json") {
                    self.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.plugin_file_input)
                };

                self.toast_message = Some(ToastMessage::success(format!(
                    "Loaded preset: {}",
                    filename
                )));
                self.needs_plugin_update = true;
                self.last_loaded_preset = Some(filename);
            }
            Err(e) => {
                self.toast_message = Some(ToastMessage::error(format!("Error loading: {}", e)));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    /// Load the selected preset from the available presets list
    pub fn load_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.toast_message = Some(ToastMessage::error("No presets available".to_string()));
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            match self.plugin_chain.load_from_file(&preset_filename) {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_chain.update_binaural_decoder_channels();

                    self.toast_message = Some(ToastMessage::success(format!(
                        "Loaded preset: {} ({} plugins)",
                        preset_filename,
                        self.plugin_chain.len()
                    )));
                    self.needs_plugin_update = true;
                    self.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.toast_message =
                        Some(ToastMessage::error(format!("Error loading preset: {}", e)));
                    log::error!("Failed to load plugin chain: {}", e);
                }
            }
        }
    }

    /// Select the next preset in the list
    pub fn select_next_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_plugin_presets.len();
        }
    }

    /// Select the previous preset in the list
    pub fn select_previous_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_plugin_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }
}

// Helper function to get parameter count for a plugin
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::Upmixer { .. } => 15,
        PluginSettings::EQ { filters } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Gate { .. } => 7, // threshold, ratio, attack, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 3, // threshold, release, mix
        PluginSettings::LoudnessCompensation { .. } => 3, // target_lufs, min_gain, max_gain
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Convolution { .. } => 0,     // No adjustable params for now
        PluginSettings::LoudnessMonitor => 0,        // No parameters
        PluginSettings::SpectrumAnalyzer { .. } => 4, // num_bins, min_freq, max_freq, smoothing
        PluginSettings::Gain { .. } => 1,            // gain_db
        PluginSettings::ChannelMuteSolo { .. } => 0, // No editable params (toggles only)
    }
}
