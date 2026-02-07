//! Plugin management and editing methods.
//!
//! Contains methods for plugin chain management, parameter editing, and presets.

use sotf_audio_player::PluginSettings;
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use super::common::param_index_to_engine_param;
use crate::app::types::PluginUpdateType;
use crate::app::{App, ToastMessage};

pub trait PluginEditingManager {
    fn sync_spectrum_visible(&mut self);
    fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType);
    fn toggle_plugin(&mut self, index: usize);
    fn move_plugin_up(&mut self, index: usize);
    fn move_plugin_down(&mut self, index: usize);
    fn select_next_plugin(&mut self);
    fn select_previous_plugin(&mut self);
    fn remove_plugin(&mut self, index: usize);
    fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin>;
    fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin>;
    fn select_next_param(&mut self);
    fn select_previous_param(&mut self);
    fn adjust_selected_param(&mut self, delta: f64) -> bool;

    // Additional methods
    fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64);
    fn set_plugin_param_string(&mut self, plugin_idx: usize, param_idx: usize, value: String);
    fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        correction: SpectralTiltCorrection,
    );
    fn set_spectrum_tilt_reference(&mut self, plugin_idx: usize, reference: TiltReferenceFreq);
    fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize);
    fn load_apo_file(&mut self) -> Result<(), String>;
    fn load_sofa_file(&mut self) -> Result<(), String>;
    fn add_eq_band(&mut self) -> Result<(), String>;
    fn remove_eq_band(&mut self, band_idx: usize) -> Result<(), String>;
    fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<(), String>;
    fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<(), String>;
    fn set_eq_per_channel_mode(&mut self, plugin_idx: usize, per_channel: bool);
    fn refresh_plugin_presets(&mut self);
    fn save_plugin_chain(&mut self);
    fn save_selected_preset(&mut self);
    fn load_plugin_chain(&mut self);
    fn load_selected_preset(&mut self);
    fn select_next_preset(&mut self);
    fn select_previous_preset(&mut self);
}

impl PluginEditingManager for App {
    /// Sync spectrum_visible flag with the actual plugin chain contents.
    /// Should be called whenever the plugin chain changes structurally.
    /// Sync spectrum_visible flag with the actual plugin chain contents.
    /// Should be called whenever the plugin chain changes structurally.
    fn sync_spectrum_visible(&mut self) {
        self.spectrum_visible = self
            .plugin_state
            .plugin_chain
            .has_enabled_spectrum_analyzer();
    }

    // Plugin management methods
    // Plugin management methods
    fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType) {
        // Insert user plugins before the Matrix (between input monitor and matrix)
        let insert_idx = self.plugin_state.plugin_chain.user_plugin_insert_index();
        self.plugin_state
            .plugin_chain
            .insert_plugin(insert_idx, plugin_type);
        self.plugin_state.selected_plugin_index = insert_idx;
        self.plugin_state
            .plugin_chain
            .update_channel_dependent_plugins();
        self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        self.sync_spectrum_visible();
    }

    fn toggle_plugin(&mut self, index: usize) {
        self.plugin_state.plugin_chain.toggle_plugin(index);
        // Update BinauralDecoder input channels after toggle
        self.plugin_state
            .plugin_chain
            .update_channel_dependent_plugins();
        self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        self.sync_spectrum_visible();
    }

    fn move_plugin_up(&mut self, index: usize) {
        if index > 0 {
            self.plugin_state.plugin_chain.move_plugin(index, index - 1);
            self.plugin_state.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_state
                .plugin_chain
                .update_channel_dependent_plugins();
            self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        }
    }

    fn move_plugin_down(&mut self, index: usize) {
        if index < self.plugin_state.plugin_chain.len() - 1 {
            self.plugin_state.plugin_chain.move_plugin(index, index + 1);
            self.plugin_state.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_state
                .plugin_chain
                .update_channel_dependent_plugins();
            self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        }
    }

    fn select_next_plugin(&mut self) {
        if !self.plugin_state.plugin_chain.is_empty() {
            self.plugin_state.selected_plugin_index = (self.plugin_state.selected_plugin_index + 1)
                % self.plugin_state.plugin_chain.len();
        }
    }

    fn select_previous_plugin(&mut self) {
        if !self.plugin_state.plugin_chain.is_empty() {
            if self.plugin_state.selected_plugin_index == 0 {
                self.plugin_state.selected_plugin_index = self.plugin_state.plugin_chain.len() - 1;
            } else {
                self.plugin_state.selected_plugin_index -= 1;
            }
        }
    }

    fn remove_plugin(&mut self, index: usize) {
        if index < self.plugin_state.plugin_chain.len() {
            self.plugin_state.plugin_chain.remove_plugin(index);
            // Update BinauralDecoder input channels after removal
            self.plugin_state
                .plugin_chain
                .update_channel_dependent_plugins();
            self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
            self.sync_spectrum_visible();
            // Adjust selection
            if self.plugin_state.selected_plugin_index >= self.plugin_state.plugin_chain.len()
                && self.plugin_state.selected_plugin_index > 0
            {
                self.plugin_state.selected_plugin_index = self.plugin_state.plugin_chain.len() - 1;
            }
        }
    }

    // Plugin editing methods
    // Plugin editing methods
    fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.plugin_state
            .editing_plugin_index
            .and_then(|idx| self.plugin_state.plugin_chain.get_plugin(idx))
    }

    fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.plugin_state
            .editing_plugin_index
            .and_then(|idx| self.plugin_state.plugin_chain.get_plugin_mut(idx))
    }

    fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_state.plugin_param_selection =
                    (self.plugin_state.plugin_param_selection + 1) % param_count;
            }
        }
    }

    fn select_previous_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                if self.plugin_state.plugin_param_selection == 0 {
                    self.plugin_state.plugin_param_selection = param_count - 1;
                } else {
                    self.plugin_state.plugin_param_selection -= 1;
                }
            }
        }
    }

    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    fn adjust_selected_param(&mut self, delta: f64) -> bool {
        let param_idx = self.plugin_state.plugin_param_selection;
        let mut channel_count_changed = false;

        let result = if let Some(plugin) = self.get_editing_plugin_mut() {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    speaker_config,
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    height_gain,
                    stereo_width,
                    center_spread,
                    surround_direct_bleed,
                    rear_late_reflection,
                    lfe_cutoff_hz,
                    lfe_gain,
                    bandpass_hz,
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    subharmonic_freq_hz,
                    subharmonic_attack_ms,
                    subharmonic_release_ms,
                    decorrelation_mode,
                    decorrelation_lfo_rate_hz,
                    velvet_noise_duration_ms,
                    velvet_noise_density,
                    enable_hr_direct,
                    hr_sharpen,
                    height_hf_cap_hz,
                    height_transient_reduction,
                    height_direct_leak,
                    ambient_boost,
                    safety_cap_db,
                    rear_ambient_boost,
                    dialogue_weight,
                    voice_freq_min_hz,
                    voice_freq_max_hz,
                    ..
                } => {
                    use sotf_audio_player::param_specs::upmixer::*;
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
                            } else if current_idx == 0 {
                                configs.len() - 1
                            } else {
                                current_idx - 1
                            };
                            *speaker_config = configs[new_idx].to_string();
                            channel_count_changed = true;
                            true
                        }
                        1 => {
                            *gain_front_direct = (*gain_front_direct + delta * 0.05)
                                .clamp(GAIN_FRONT_DIRECT_MIN as f64, GAIN_FRONT_DIRECT_MAX as f64);
                            true
                        }
                        2 => {
                            *gain_front_ambient = (*gain_front_ambient + delta * 0.05).clamp(
                                GAIN_FRONT_AMBIENT_MIN as f64,
                                GAIN_FRONT_AMBIENT_MAX as f64,
                            );
                            true
                        }
                        3 => {
                            *gain_rear_ambient = (*gain_rear_ambient + delta * 0.05)
                                .clamp(GAIN_REAR_AMBIENT_MIN as f64, GAIN_REAR_AMBIENT_MAX as f64);
                            true
                        }
                        4 => {
                            *height_gain = (*height_gain + delta * 0.05)
                                .clamp(GAIN_HEIGHT_MIN as f64, GAIN_HEIGHT_MAX as f64);
                            true
                        }
                        5 => {
                            *lfe_gain = (*lfe_gain + delta * 0.05)
                                .clamp(LFE_GAIN_MIN as f64, LFE_GAIN_MAX as f64);
                            true
                        }
                        6 => {
                            *lfe_cutoff_hz = (*lfe_cutoff_hz + delta * 5.0)
                                .clamp(LFE_CUTOFF_HZ_MIN as f64, LFE_CUTOFF_HZ_MAX as f64);
                            true
                        }
                        7 => {
                            *stereo_width = (*stereo_width + delta * 0.05)
                                .clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64);
                            true
                        }
                        8 => {
                            *center_spread = (*center_spread + delta * 0.05)
                                .clamp(CENTER_SPREAD_MIN as f64, CENTER_SPREAD_MAX as f64);
                            true
                        }
                        9 => {
                            *bandpass_hz = (*bandpass_hz + delta * 5.0)
                                .clamp(BANDPASS_HZ_MIN as f64, BANDPASS_HZ_MAX as f64);
                            true
                        }
                        10 => {
                            *enable_subharmonic_synth = !*enable_subharmonic_synth;
                            true
                        }
                        11 => {
                            *subharmonic_gain = (*subharmonic_gain + delta * 0.05)
                                .clamp(SUBHARMONIC_GAIN_MIN as f64, SUBHARMONIC_GAIN_MAX as f64);
                            true
                        }
                        12 => {
                            *enable_hr_direct = !*enable_hr_direct;
                            true
                        }
                        13 => {
                            *hr_sharpen = (*hr_sharpen + delta * 0.05)
                                .clamp(HR_SHARPEN_MIN as f64, HR_SHARPEN_MAX as f64);
                            true
                        }
                        14 => {
                            *safety_cap_db = (*safety_cap_db + delta * 0.1)
                                .clamp(SAFETY_CAP_DB_MIN as f64, SAFETY_CAP_DB_MAX as f64);
                            true
                        }
                        15 => {
                            // Toggle decorrelation mode (0 or 1)
                            if delta.abs() > 0.1 {
                                *decorrelation_mode = if *decorrelation_mode == 0 { 1 } else { 0 };
                            }
                            true
                        }
                        16 => {
                            *subharmonic_freq_hz = (*subharmonic_freq_hz + delta * 2.0).clamp(
                                SUBHARMONIC_FREQ_HZ_MIN as f64,
                                SUBHARMONIC_FREQ_HZ_MAX as f64,
                            );
                            true
                        }
                        17 => {
                            *subharmonic_attack_ms = (*subharmonic_attack_ms + delta * 2.0).clamp(
                                SUBHARMONIC_ATTACK_MS_MIN as f64,
                                SUBHARMONIC_ATTACK_MS_MAX as f64,
                            );
                            true
                        }
                        18 => {
                            *subharmonic_release_ms = (*subharmonic_release_ms + delta * 10.0)
                                .clamp(
                                    SUBHARMONIC_RELEASE_MS_MIN as f64,
                                    SUBHARMONIC_RELEASE_MS_MAX as f64,
                                );
                            true
                        }
                        19 => {
                            *decorrelation_lfo_rate_hz =
                                (*decorrelation_lfo_rate_hz + delta * 0.02).clamp(
                                    DECORRELATION_LFO_RATE_HZ_MIN as f64,
                                    DECORRELATION_LFO_RATE_HZ_MAX as f64,
                                );
                            true
                        }
                        20 => {
                            *velvet_noise_duration_ms = (*velvet_noise_duration_ms + delta * 2.0)
                                .clamp(
                                    VELVET_NOISE_DURATION_MS_MIN as f64,
                                    VELVET_NOISE_DURATION_MS_MAX as f64,
                                );
                            true
                        }
                        21 => {
                            *velvet_noise_density = (*velvet_noise_density + delta * 100.0).clamp(
                                VELVET_NOISE_DENSITY_MIN as f64,
                                VELVET_NOISE_DENSITY_MAX as f64,
                            );
                            true
                        }
                        22 => {
                            *height_hf_cap_hz = (*height_hf_cap_hz + delta * 200.0)
                                .clamp(HEIGHT_HF_CAP_HZ_MIN as f64, HEIGHT_HF_CAP_HZ_MAX as f64);
                            true
                        }
                        23 => {
                            *height_transient_reduction =
                                (*height_transient_reduction + delta * 0.05).clamp(
                                    HEIGHT_TRANSIENT_REDUCTION_MIN as f64,
                                    HEIGHT_TRANSIENT_REDUCTION_MAX as f64,
                                );
                            true
                        }
                        24 => {
                            *height_direct_leak = (*height_direct_leak + delta * 0.02).clamp(
                                HEIGHT_DIRECT_LEAK_MIN as f64,
                                HEIGHT_DIRECT_LEAK_MAX as f64,
                            );
                            true
                        }
                        25 => {
                            *surround_direct_bleed = (*surround_direct_bleed + delta * 0.05).clamp(
                                SURROUND_DIRECT_BLEED_MIN as f64,
                                SURROUND_DIRECT_BLEED_MAX as f64,
                            );
                            true
                        }
                        26 => {
                            *rear_ambient_boost = (*rear_ambient_boost + delta * 0.05).clamp(
                                REAR_AMBIENT_BOOST_MIN as f64,
                                REAR_AMBIENT_BOOST_MAX as f64,
                            );
                            true
                        }
                        27 => {
                            *rear_late_reflection = (*rear_late_reflection + delta * 0.02).clamp(
                                REAR_LATE_REFLECTION_MIN as f64,
                                REAR_LATE_REFLECTION_MAX as f64,
                            );
                            true
                        }
                        28 => {
                            *ambient_boost = (*ambient_boost + delta * 0.05)
                                .clamp(AMBIENT_BOOST_MIN as f64, AMBIENT_BOOST_MAX as f64);
                            true
                        }
                        29 => {
                            *dialogue_weight = (*dialogue_weight + delta * 0.05)
                                .clamp(DIALOGUE_WEIGHT_MIN as f64, DIALOGUE_WEIGHT_MAX as f64);
                            true
                        }
                        30 => {
                            *voice_freq_min_hz = (*voice_freq_min_hz + delta * 20.0)
                                .clamp(VOICE_FREQ_MIN_HZ_MIN as f64, VOICE_FREQ_MIN_HZ_MAX as f64);
                            true
                        }
                        31 => {
                            *voice_freq_max_hz = (*voice_freq_max_hz + delta * 100.0)
                                .clamp(VOICE_FREQ_MAX_HZ_MIN as f64, VOICE_FREQ_MAX_HZ_MAX as f64);
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
                    lookahead_ms,
                    soft,
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
                        *lookahead_ms = (*lookahead_ms + delta as f64 * 0.1).max(0.0).min(20.0);
                        true
                    }
                    3 => {
                        *soft = !*soft;
                        true
                    }
                    4 => {
                        *mix = (*mix + delta as f64 * 0.05).max(0.0).min(1.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    hold_ms,
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
                        *hold_ms = (*hold_ms + delta as f64 * 5.0).max(0.0).min(1000.0);
                        true
                    }
                    4 => {
                        *release_ms = (*release_ms + delta as f64).max(1.0).min(1000.0);
                        true
                    }
                    5 => {
                        *mix = (*mix + delta as f64 * 0.05).max(0.0).min(1.0);
                        true
                    }
                    6 => {
                        *link_channels = !*link_channels;
                        true
                    }
                    7 => {
                        *sidechain_hpf_hz =
                            (*sidechain_hpf_hz + delta as f64 * 5.0).max(0.0).min(200.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::LoudnessCompensation {
                    low_freq,
                    low_gain,
                    high_freq,
                    high_gain,
                    auto_gain_enabled,
                    auto_gain_max_db,
                    auto_gain_smoothing_ms,
                } => match param_idx {
                    0 => {
                        *low_freq = (*low_freq + delta as f64).max(20.0).min(500.0);
                        true
                    }
                    1 => {
                        *low_gain = (*low_gain + delta as f64).max(-20.0).min(20.0);
                        true
                    }
                    2 => {
                        *high_freq = (*high_freq + delta as f64 * 100.0).max(2000.0).min(20000.0);
                        true
                    }
                    3 => {
                        *high_gain = (*high_gain + delta as f64).max(-20.0).min(20.0);
                        true
                    }
                    4 => {
                        // Toggle auto_gain_enabled
                        *auto_gain_enabled = !*auto_gain_enabled;
                        true
                    }
                    5 => {
                        *auto_gain_max_db = (*auto_gain_max_db + delta as f64).max(0.0).min(24.0);
                        true
                    }
                    6 => {
                        *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta as f64 * 10.0)
                            .max(1.0)
                            .min(1000.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::EQ { filters, .. } => {
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
                PluginSettings::Convolution { mix, gain_db, .. } => {
                    use sotf_audio_player::param_specs::convolution::*;
                    match param_idx {
                        0 => {
                            *mix = (*mix + delta * 0.05).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            true
                        }
                        1 => {
                            *gain_db = (*gain_db + delta * 0.5)
                                .clamp(GAIN_DB_MIN as f64, GAIN_DB_MAX as f64);
                            true
                        }
                        _ => false,
                    }
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
                    ..
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
                PluginSettings::Gain { gain_db, .. } => match param_idx {
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
                PluginSettings::Matrix { .. } => {
                    // Matrix is edited via grid UI, not adjustable parameters
                    false
                }
                PluginSettings::Expander {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    range_db,
                    knee_db,
                    hysteresis_db,
                    hold_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => {
                    use sotf_audio_player::param_specs::expander::*;
                    match param_idx {
                        0 => {
                            *threshold_db = (*threshold_db + delta)
                                .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            true
                        }
                        1 => {
                            *ratio =
                                (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            true
                        }
                        2 => {
                            *attack_ms = (*attack_ms + delta * 0.1)
                                .clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            true
                        }
                        3 => {
                            *release_ms = (*release_ms + delta * 10.0)
                                .clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            true
                        }
                        4 => {
                            *range_db =
                                (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64);
                            true
                        }
                        5 => {
                            *knee_db =
                                (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            true
                        }
                        6 => {
                            *hysteresis_db = (*hysteresis_db + delta * 0.1)
                                .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64);
                            true
                        }
                        7 => {
                            *hold_ms =
                                (*hold_ms + delta * 5.0).clamp(HOLD_MIN as f64, HOLD_MAX as f64);
                            true
                        }
                        8 => {
                            *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            true
                        }
                        9 => {
                            *link_channels = !*link_channels;
                            true
                        }
                        10 => {
                            *sidechain_hpf_hz = (*sidechain_hpf_hz + delta * 5.0)
                                .clamp(SIDECHAIN_HPF_HZ_MIN as f64, SIDECHAIN_HPF_HZ_MAX as f64);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::MultibandCompressor {
                    num_bands,
                    crossover_preset,
                    crossover_freq_1,
                    crossover_freq_2,
                    crossover_freq_3,
                    crossover_freq_4,
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    knee_db,
                    mix,
                    link_channels,
                    ..
                } => {
                    use sotf_audio_player::param_specs::multiband_compressor::*;
                    match param_idx {
                        0 => {
                            *num_bands = ((*num_bands as i64) + delta as i64)
                                .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                                as usize;
                            true
                        }
                        1 => {
                            *crossover_preset = ((*crossover_preset as i64) + delta as i64)
                                .clamp(CROSSOVER_PRESET_MIN as i64, CROSSOVER_PRESET_MAX as i64)
                                as i32;
                            true
                        }
                        2 => {
                            *crossover_freq_1 = (*crossover_freq_1 + delta * 10.0)
                                .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64);
                            true
                        }
                        3 => {
                            *crossover_freq_2 = (*crossover_freq_2 + delta * 50.0)
                                .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64);
                            true
                        }
                        4 => {
                            *crossover_freq_3 = (*crossover_freq_3 + delta * 100.0)
                                .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64);
                            true
                        }
                        5 => {
                            *crossover_freq_4 = (*crossover_freq_4 + delta * 100.0)
                                .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64);
                            true
                        }
                        6 => {
                            *threshold_db = (*threshold_db + delta)
                                .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            true
                        }
                        7 => {
                            *ratio =
                                (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            true
                        }
                        8 => {
                            *attack_ms = (*attack_ms + delta * 0.5)
                                .clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            true
                        }
                        9 => {
                            *release_ms = (*release_ms + delta * 5.0)
                                .clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            true
                        }
                        10 => {
                            *knee_db =
                                (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            true
                        }
                        11 => {
                            *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            true
                        }
                        12 => {
                            *link_channels = !*link_channels;
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::MultibandExpander {
                    num_bands,
                    crossover_preset,
                    crossover_freq_1,
                    crossover_freq_2,
                    crossover_freq_3,
                    crossover_freq_4,
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    range_db,
                    knee_db,
                    hysteresis_db,
                    hold_ms,
                    mix,
                    link_channels,
                    ..
                } => {
                    use sotf_audio_player::param_specs::multiband_expander::*;
                    match param_idx {
                        0 => {
                            *num_bands = ((*num_bands as i64) + delta as i64)
                                .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                                as usize;
                            true
                        }
                        1 => {
                            *crossover_preset = ((*crossover_preset as i64) + delta as i64)
                                .clamp(CROSSOVER_PRESET_MIN as i64, CROSSOVER_PRESET_MAX as i64)
                                as i32;
                            true
                        }
                        2 => {
                            *crossover_freq_1 = (*crossover_freq_1 + delta * 10.0)
                                .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64);
                            true
                        }
                        3 => {
                            *crossover_freq_2 = (*crossover_freq_2 + delta * 50.0)
                                .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64);
                            true
                        }
                        4 => {
                            *crossover_freq_3 = (*crossover_freq_3 + delta * 100.0)
                                .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64);
                            true
                        }
                        5 => {
                            *crossover_freq_4 = (*crossover_freq_4 + delta * 100.0)
                                .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64);
                            true
                        }
                        6 => {
                            *threshold_db = (*threshold_db + delta)
                                .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            true
                        }
                        7 => {
                            *ratio =
                                (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            true
                        }
                        8 => {
                            *attack_ms = (*attack_ms + delta * 0.1)
                                .clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            true
                        }
                        9 => {
                            *release_ms = (*release_ms + delta * 10.0)
                                .clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            true
                        }
                        10 => {
                            *range_db =
                                (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64);
                            true
                        }
                        11 => {
                            *knee_db =
                                (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            true
                        }
                        12 => {
                            *hysteresis_db = (*hysteresis_db + delta * 0.1)
                                .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64);
                            true
                        }
                        13 => {
                            *hold_ms =
                                (*hold_ms + delta * 5.0).clamp(HOLD_MIN as f64, HOLD_MAX as f64);
                            true
                        }
                        14 => {
                            *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            true
                        }
                        15 => {
                            *link_channels = !*link_channels;
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::XTC {
                    distance_m,
                    speaker_angle_deg,
                    head_radius_m,
                    beta_base,
                    beta_low_freq_boost,
                    beta_high_freq_boost,
                    head_shadow_cutoff_hz,
                    head_shadow_slope_db_per_octave,
                } => {
                    match param_idx {
                        0 => {
                            *distance_m = (*distance_m + delta * 0.1).clamp(0.5, 5.0);
                            true
                        }
                        1 => {
                            *speaker_angle_deg = (*speaker_angle_deg + delta).clamp(10.0, 60.0);
                            true
                        }
                        2 => {
                            // head_radius stored in meters, display in cm, so /100 back
                            *head_radius_m = (*head_radius_m + delta * 0.001).clamp(0.05, 0.15);
                            true
                        }
                        3 => {
                            *beta_base = (*beta_base + delta * 0.001).clamp(0.0001, 0.1);
                            true
                        }
                        4 => {
                            *beta_low_freq_boost =
                                (*beta_low_freq_boost + delta * 1.0).clamp(1.0, 100.0);
                            true
                        }
                        5 => {
                            *beta_high_freq_boost =
                                (*beta_high_freq_boost + delta * 1.0).clamp(1.0, 100.0);
                            true
                        }
                        6 => {
                            *head_shadow_cutoff_hz =
                                (*head_shadow_cutoff_hz + delta * 100.0).clamp(1000.0, 10000.0);
                            true
                        }
                        7 => {
                            *head_shadow_slope_db_per_octave =
                                (*head_shadow_slope_db_per_octave + delta * 0.1).clamp(0.0, 12.0);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::Denoiser {
                    reduction_db,
                    floor_db,
                    smoothing,
                    attack_ms,
                    release_ms,
                    low_latency,
                    polyphonic_detection,
                    dd_enabled,
                    dd_alpha,
                    psychoacoustic_masking,
                    use_captured_profile,
                } => {
                    use sotf_audio_player::param_specs::denoiser::*;
                    match param_idx {
                        0 => {
                            *reduction_db = (*reduction_db + delta)
                                .clamp(REDUCTION_DB_MIN as f64, REDUCTION_DB_MAX as f64);
                            true
                        }
                        1 => {
                            *floor_db =
                                (*floor_db + delta).clamp(FLOOR_DB_MIN as f64, FLOOR_DB_MAX as f64);
                            true
                        }
                        2 => {
                            *smoothing = (*smoothing + delta * 0.05)
                                .clamp(SMOOTHING_MIN as f64, SMOOTHING_MAX as f64);
                            true
                        }
                        3 => {
                            *attack_ms = (*attack_ms + delta)
                                .clamp(ATTACK_MS_MIN as f64, ATTACK_MS_MAX as f64);
                            true
                        }
                        4 => {
                            *release_ms = (*release_ms + delta * 5.0)
                                .clamp(RELEASE_MS_MIN as f64, RELEASE_MS_MAX as f64);
                            true
                        }
                        5 => {
                            *low_latency = !*low_latency;
                            true
                        }
                        6 => {
                            *polyphonic_detection = !*polyphonic_detection;
                            true
                        }
                        7 => {
                            *dd_enabled = !*dd_enabled;
                            true
                        }
                        8 => {
                            *dd_alpha = (*dd_alpha + delta * 0.01)
                                .clamp(DD_ALPHA_MIN as f64, DD_ALPHA_MAX as f64);
                            true
                        }
                        9 => {
                            *psychoacoustic_masking = !*psychoacoustic_masking;
                            true
                        }
                        10 => true, // learn_noise trigger — handled by set_parameter_value
                        11 => {
                            *use_captured_profile = !*use_captured_profile;
                            true
                        }
                        12 => true, // clear_profile trigger — handled by set_parameter_value
                        _ => false,
                    }
                }
                PluginSettings::Pnd {
                    correction_strength,
                    analysis_window_ms,
                    drift_smoothing,
                } => {
                    use sotf_audio_player::param_specs::pnd::*;
                    match param_idx {
                        0 => {
                            *correction_strength = (*correction_strength + delta * 0.1).clamp(
                                CORRECTION_STRENGTH_MIN as f64,
                                CORRECTION_STRENGTH_MAX as f64,
                            );
                            true
                        }
                        1 => {
                            *analysis_window_ms = (*analysis_window_ms + delta * 10.0).clamp(
                                ANALYSIS_WINDOW_MS_MIN as f64,
                                ANALYSIS_WINDOW_MS_MAX as f64,
                            );
                            true
                        }
                        2 => {
                            *drift_smoothing = (*drift_smoothing + delta * 0.01)
                                .clamp(DRIFT_SMOOTHING_MIN as f64, DRIFT_SMOOTHING_MAX as f64);
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::ABCompare {
                    mix,
                    mix_mode,
                    selected_path,
                    bypass,
                    auto_gain_enabled,
                    loudness_type,
                    max_auto_gain_db,
                    gain_smoothing_ms,
                    mix_transition_ms,
                    ..
                } => match param_idx {
                    0 => {
                        *mix = (*mix + delta * 0.1).clamp(-1.0, 1.0);
                        true
                    }
                    1 => {
                        *mix_mode = if *mix_mode == 0 { 1 } else { 0 };
                        true
                    }
                    2 => {
                        *selected_path = if *selected_path == 0 { 1 } else { 0 };
                        true
                    }
                    3 => {
                        *bypass = !*bypass;
                        true
                    }
                    4 => {
                        *auto_gain_enabled = !*auto_gain_enabled;
                        true
                    }
                    5 => {
                        *loudness_type = if *loudness_type == 0 { 1 } else { 0 };
                        true
                    }
                    6 => {
                        *max_auto_gain_db = (*max_auto_gain_db + delta).clamp(0.0, 24.0);
                        true
                    }
                    7 => {
                        *gain_smoothing_ms = (*gain_smoothing_ms + delta * 10.0).clamp(10.0, 500.0);
                        true
                    }
                    8 => {
                        *mix_transition_ms = (*mix_transition_ms + delta * 5.0).clamp(5.0, 500.0);
                        true
                    }
                    _ => false,
                },
                PluginSettings::FletcherMunson {
                    reference_level_db,
                    smoothing_ms,
                    band1_freq,
                    band1_q,
                    band1_max_gain,
                    band1_slope,
                    band2_freq,
                    band2_q,
                    band2_max_gain,
                    band2_slope,
                    band3_freq,
                    band3_q,
                    band3_max_gain,
                    band3_slope,
                    band4_freq,
                    band4_q,
                    band4_max_gain,
                    band4_slope,
                    auto_gain_enabled,
                    auto_gain_max_db,
                    auto_gain_smoothing_ms,
                    auto_gain_loudness_type,
                    ..
                } => match param_idx {
                    0 => {
                        *reference_level_db = (*reference_level_db + delta).clamp(-40.0, 0.0);
                        true
                    }
                    1 => {
                        *smoothing_ms = (*smoothing_ms + delta * 5.0).clamp(1.0, 200.0);
                        true
                    }
                    // Band 1 parameters (offset 2)
                    2 => {
                        *band1_freq = (*band1_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                        true
                    }
                    3 => {
                        *band1_q = (*band1_q + delta * 0.1).clamp(0.1, 10.0);
                        true
                    }
                    4 => {
                        *band1_max_gain = (*band1_max_gain + delta).clamp(0.0, 24.0);
                        true
                    }
                    5 => {
                        *band1_slope = (*band1_slope + delta * 0.05).clamp(0.0, 1.0);
                        true
                    }
                    // Band 2 parameters (offset 6)
                    6 => {
                        *band2_freq = (*band2_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                        true
                    }
                    7 => {
                        *band2_q = (*band2_q + delta * 0.1).clamp(0.1, 10.0);
                        true
                    }
                    8 => {
                        *band2_max_gain = (*band2_max_gain + delta).clamp(0.0, 24.0);
                        true
                    }
                    9 => {
                        *band2_slope = (*band2_slope + delta * 0.05).clamp(0.0, 1.0);
                        true
                    }
                    // Band 3 parameters (offset 10)
                    10 => {
                        *band3_freq = (*band3_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                        true
                    }
                    11 => {
                        *band3_q = (*band3_q + delta * 0.1).clamp(0.1, 10.0);
                        true
                    }
                    12 => {
                        *band3_max_gain = (*band3_max_gain + delta).clamp(0.0, 24.0);
                        true
                    }
                    13 => {
                        *band3_slope = (*band3_slope + delta * 0.05).clamp(0.0, 1.0);
                        true
                    }
                    // Band 4 parameters (offset 14)
                    14 => {
                        *band4_freq = (*band4_freq * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                        true
                    }
                    15 => {
                        *band4_q = (*band4_q + delta * 0.1).clamp(0.1, 10.0);
                        true
                    }
                    16 => {
                        *band4_max_gain = (*band4_max_gain + delta).clamp(0.0, 24.0);
                        true
                    }
                    17 => {
                        *band4_slope = (*band4_slope + delta * 0.05).clamp(0.0, 1.0);
                        true
                    }
                    // Auto-gain parameters (offset 18)
                    18 => {
                        *auto_gain_enabled = !*auto_gain_enabled;
                        true
                    }
                    19 => {
                        *auto_gain_max_db = (*auto_gain_max_db + delta).clamp(0.0, 24.0);
                        true
                    }
                    20 => {
                        *auto_gain_smoothing_ms =
                            (*auto_gain_smoothing_ms + delta * 10.0).clamp(10.0, 500.0);
                        true
                    }
                    21 => {
                        *auto_gain_loudness_type =
                            if *auto_gain_loudness_type == 0 { 1 } else { 0 };
                        true
                    }
                    _ => false,
                },
                PluginSettings::BandSplit {
                    frequency,
                    crossover_type,
                    ..
                } => match param_idx {
                    0 => {
                        *frequency = (*frequency * (1.0 + delta * 0.05)).clamp(20.0, 20000.0);
                        true
                    }
                    1 => {
                        // Toggle between LR24 and LR48
                        *crossover_type = if crossover_type == "LR24" {
                            "LR48".to_string()
                        } else {
                            "LR24".to_string()
                        };
                        true
                    }
                    _ => false,
                },
                PluginSettings::BandMerge { bands, .. } => match param_idx {
                    0 => {
                        *bands = ((*bands as i64) + delta as i64).clamp(2, 8) as usize;
                        true
                    }
                    _ => false,
                },
                PluginSettings::Downmix {
                    center_gain_db,
                    surround_gain_db,
                    height_gain_db,
                    lfe_gain_db,
                    phase_coherence,
                    phase_blend_low_hz,
                    phase_blend_high_hz,
                    ..
                } => {
                    use sotf_plugins::param_specs::downmix::*;
                    match param_idx {
                        0 => {
                            *center_gain_db = (*center_gain_db + delta * 0.5)
                                .clamp(CENTER_GAIN_DB_MIN as f64, CENTER_GAIN_DB_MAX as f64);
                            true
                        }
                        1 => {
                            *surround_gain_db = (*surround_gain_db + delta * 0.5)
                                .clamp(SURROUND_GAIN_DB_MIN as f64, SURROUND_GAIN_DB_MAX as f64);
                            true
                        }
                        2 => {
                            *height_gain_db = (*height_gain_db + delta * 0.5)
                                .clamp(HEIGHT_GAIN_DB_MIN as f64, HEIGHT_GAIN_DB_MAX as f64);
                            true
                        }
                        3 => {
                            *lfe_gain_db = (*lfe_gain_db + delta * 0.5)
                                .clamp(LFE_GAIN_DB_MIN as f64, LFE_GAIN_DB_MAX as f64);
                            true
                        }
                        4 => {
                            *phase_coherence = !*phase_coherence;
                            true
                        }
                        5 => {
                            *phase_blend_low_hz = (*phase_blend_low_hz + delta * 10.0)
                                .clamp(PHASE_BLEND_LOW_HZ_MIN as f64, PHASE_BLEND_LOW_HZ_MAX as f64);
                            true
                        }
                        6 => {
                            *phase_blend_high_hz = (*phase_blend_high_hz + delta * 10.0).clamp(
                                PHASE_BLEND_HIGH_HZ_MIN as f64,
                                PHASE_BLEND_HIGH_HZ_MAX as f64,
                            );
                            true
                        }
                        _ => false,
                    }
                }
                PluginSettings::MonoToStereo {
                    stereo_width,
                    haas_delay_ms,
                    enable_comp_eq,
                    comp_eq_depth_db,
                    decor_low_hz,
                    decor_high_hz,
                } => {
                    use sotf_plugins::param_specs::mono_to_stereo::*;
                    match param_idx {
                        0 => {
                            *stereo_width = (*stereo_width + delta * 0.05)
                                .clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64);
                            true
                        }
                        1 => {
                            *haas_delay_ms = (*haas_delay_ms + delta * 0.1)
                                .clamp(HAAS_DELAY_MS_MIN as f64, HAAS_DELAY_MS_MAX as f64);
                            true
                        }
                        2 => {
                            *enable_comp_eq = !*enable_comp_eq;
                            true
                        }
                        3 => {
                            *comp_eq_depth_db = (*comp_eq_depth_db + delta * 0.1)
                                .clamp(COMP_EQ_DEPTH_DB_MIN as f64, COMP_EQ_DEPTH_DB_MAX as f64);
                            true
                        }
                        4 => {
                            *decor_low_hz = (*decor_low_hz + delta * 10.0)
                                .clamp(DECOR_LOW_HZ_MIN as f64, DECOR_LOW_HZ_MAX as f64);
                            true
                        }
                        5 => {
                            *decor_high_hz = (*decor_high_hz + delta * 10.0).clamp(
                                DECOR_HIGH_HZ_MIN as f64,
                                DECOR_HIGH_HZ_MAX as f64,
                            );
                            true
                        }
                        _ => false,
                    }
                }
            }
        } else {
            false
        };

        if result && channel_count_changed {
            self.plugin_state
                .plugin_chain
                .update_channel_dependent_plugins();
        }

        if result {
            // Determine update type based on whether this parameter supports individual updates
            let update_type = if channel_count_changed {
                // Channel count changes always require structural update
                PluginUpdateType::Structural
            } else if let Some(plugin_idx) = self.plugin_state.editing_plugin_index {
                if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin(plugin_idx) {
                    if param_index_to_engine_param(&plugin.settings, param_idx).is_some() {
                        PluginUpdateType::Parameter {
                            plugin_index: plugin_idx,
                            param_index: param_idx,
                        }
                    } else {
                        PluginUpdateType::Structural
                    }
                } else {
                    PluginUpdateType::Structural
                }
            } else {
                PluginUpdateType::Structural
            };
            self.plugin_state.pending_plugin_update = Some(update_type);
        }

        result
    }

    /// Set a specific parameter value for a plugin
    fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64) {
        let mut channel_count_changed = false;
        let mut update_needed = false;

        if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(plugin_idx) {
            match &mut plugin.settings {
                PluginSettings::Upmixer {
                    speaker_config,
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    height_gain,
                    stereo_width,
                    center_spread,
                    surround_direct_bleed,
                    rear_late_reflection,
                    lfe_cutoff_hz,
                    lfe_gain,
                    bandpass_hz,
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    subharmonic_freq_hz,
                    subharmonic_attack_ms,
                    subharmonic_release_ms,
                    decorrelation_mode,
                    decorrelation_lfo_rate_hz,
                    velvet_noise_duration_ms,
                    velvet_noise_density,
                    enable_hr_direct,
                    hr_sharpen,
                    height_hf_cap_hz,
                    height_transient_reduction,
                    height_direct_leak,
                    ambient_boost,
                    safety_cap_db,
                    rear_ambient_boost,
                    dialogue_weight,
                    voice_freq_min_hz,
                    voice_freq_max_hz,
                    ..
                } => {
                    use sotf_audio_player::param_specs::upmixer::*;
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
                            *gain_front_direct = value
                                .clamp(GAIN_FRONT_DIRECT_MIN as f64, GAIN_FRONT_DIRECT_MAX as f64);
                            update_needed = true;
                        }
                        2 => {
                            *gain_front_ambient = value.clamp(
                                GAIN_FRONT_AMBIENT_MIN as f64,
                                GAIN_FRONT_AMBIENT_MAX as f64,
                            );
                            update_needed = true;
                        }
                        3 => {
                            *gain_rear_ambient = value
                                .clamp(GAIN_REAR_AMBIENT_MIN as f64, GAIN_REAR_AMBIENT_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *height_gain =
                                value.clamp(GAIN_HEIGHT_MIN as f64, GAIN_HEIGHT_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *lfe_gain = value.clamp(LFE_GAIN_MIN as f64, LFE_GAIN_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *lfe_cutoff_hz =
                                value.clamp(LFE_CUTOFF_HZ_MIN as f64, LFE_CUTOFF_HZ_MAX as f64);
                            update_needed = true;
                        }
                        7 => {
                            *stereo_width =
                                value.clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64);
                            update_needed = true;
                        }
                        8 => {
                            *center_spread =
                                value.clamp(CENTER_SPREAD_MIN as f64, CENTER_SPREAD_MAX as f64);
                            update_needed = true;
                        }
                        9 => {
                            *bandpass_hz =
                                value.clamp(BANDPASS_HZ_MIN as f64, BANDPASS_HZ_MAX as f64);
                            update_needed = true;
                        }
                        10 => {
                            *enable_subharmonic_synth = value > 0.5;
                            update_needed = true;
                        }
                        11 => {
                            *subharmonic_gain = value
                                .clamp(SUBHARMONIC_GAIN_MIN as f64, SUBHARMONIC_GAIN_MAX as f64);
                            update_needed = true;
                        }
                        12 => {
                            *enable_hr_direct = value > 0.5;
                            update_needed = true;
                        }
                        13 => {
                            *hr_sharpen = value.clamp(HR_SHARPEN_MIN as f64, HR_SHARPEN_MAX as f64);
                            update_needed = true;
                        }
                        14 => {
                            *safety_cap_db =
                                value.clamp(SAFETY_CAP_DB_MIN as f64, SAFETY_CAP_DB_MAX as f64);
                            update_needed = true;
                        }
                        15 => {
                            *decorrelation_mode = if value > 0.5 { 1 } else { 0 };
                            update_needed = true;
                        }
                        16 => {
                            *subharmonic_freq_hz = value.clamp(
                                SUBHARMONIC_FREQ_HZ_MIN as f64,
                                SUBHARMONIC_FREQ_HZ_MAX as f64,
                            );
                            update_needed = true;
                        }
                        17 => {
                            *subharmonic_attack_ms = value.clamp(
                                SUBHARMONIC_ATTACK_MS_MIN as f64,
                                SUBHARMONIC_ATTACK_MS_MAX as f64,
                            );
                            update_needed = true;
                        }
                        18 => {
                            *subharmonic_release_ms = value.clamp(
                                SUBHARMONIC_RELEASE_MS_MIN as f64,
                                SUBHARMONIC_RELEASE_MS_MAX as f64,
                            );
                            update_needed = true;
                        }
                        19 => {
                            *decorrelation_lfo_rate_hz = value.clamp(
                                DECORRELATION_LFO_RATE_HZ_MIN as f64,
                                DECORRELATION_LFO_RATE_HZ_MAX as f64,
                            );
                            update_needed = true;
                        }
                        20 => {
                            *velvet_noise_duration_ms = value.clamp(
                                VELVET_NOISE_DURATION_MS_MIN as f64,
                                VELVET_NOISE_DURATION_MS_MAX as f64,
                            );
                            update_needed = true;
                        }
                        21 => {
                            *velvet_noise_density = value.clamp(
                                VELVET_NOISE_DENSITY_MIN as f64,
                                VELVET_NOISE_DENSITY_MAX as f64,
                            );
                            update_needed = true;
                        }
                        22 => {
                            *height_hf_cap_hz = value
                                .clamp(HEIGHT_HF_CAP_HZ_MIN as f64, HEIGHT_HF_CAP_HZ_MAX as f64);
                            update_needed = true;
                        }
                        23 => {
                            *height_transient_reduction = value.clamp(
                                HEIGHT_TRANSIENT_REDUCTION_MIN as f64,
                                HEIGHT_TRANSIENT_REDUCTION_MAX as f64,
                            );
                            update_needed = true;
                        }
                        24 => {
                            *height_direct_leak = value.clamp(
                                HEIGHT_DIRECT_LEAK_MIN as f64,
                                HEIGHT_DIRECT_LEAK_MAX as f64,
                            );
                            update_needed = true;
                        }
                        25 => {
                            *surround_direct_bleed = value.clamp(
                                SURROUND_DIRECT_BLEED_MIN as f64,
                                SURROUND_DIRECT_BLEED_MAX as f64,
                            );
                            update_needed = true;
                        }
                        26 => {
                            *rear_ambient_boost = value.clamp(
                                REAR_AMBIENT_BOOST_MIN as f64,
                                REAR_AMBIENT_BOOST_MAX as f64,
                            );
                            update_needed = true;
                        }
                        27 => {
                            *rear_late_reflection = value.clamp(
                                REAR_LATE_REFLECTION_MIN as f64,
                                REAR_LATE_REFLECTION_MAX as f64,
                            );
                            update_needed = true;
                        }
                        28 => {
                            *ambient_boost =
                                value.clamp(AMBIENT_BOOST_MIN as f64, AMBIENT_BOOST_MAX as f64);
                            update_needed = true;
                        }
                        29 => {
                            *dialogue_weight =
                                value.clamp(DIALOGUE_WEIGHT_MIN as f64, DIALOGUE_WEIGHT_MAX as f64);
                            update_needed = true;
                        }
                        30 => {
                            *voice_freq_min_hz = value
                                .clamp(VOICE_FREQ_MIN_HZ_MIN as f64, VOICE_FREQ_MIN_HZ_MAX as f64);
                            update_needed = true;
                        }
                        31 => {
                            *voice_freq_max_hz = value
                                .clamp(VOICE_FREQ_MAX_HZ_MIN as f64, VOICE_FREQ_MAX_HZ_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                    if param_idx == 0 {
                        channel_count_changed = true;
                    }
                }
                PluginSettings::EQ { filters, .. } => {
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
                PluginSettings::Gain { gain_db, .. } => {
                    if param_idx == 0 {
                        *gain_db = value.clamp(-60.0, 12.0);
                        update_needed = true;
                    }
                }
                PluginSettings::LoudnessCompensation {
                    low_freq,
                    low_gain,
                    high_freq,
                    high_gain,
                    auto_gain_enabled,
                    auto_gain_max_db,
                    auto_gain_smoothing_ms,
                } => match param_idx {
                    0 => {
                        *low_freq = value.clamp(20.0, 500.0);
                        update_needed = true;
                    }
                    1 => {
                        *low_gain = value.clamp(-20.0, 20.0);
                        update_needed = true;
                    }
                    2 => {
                        *high_freq = value.clamp(2000.0, 20000.0);
                        update_needed = true;
                    }
                    3 => {
                        *high_gain = value.clamp(-20.0, 20.0);
                        update_needed = true;
                    }
                    4 => {
                        *auto_gain_enabled = value > 0.5;
                        update_needed = true;
                    }
                    5 => {
                        *auto_gain_max_db = value.clamp(0.0, 24.0);
                        update_needed = true;
                    }
                    6 => {
                        *auto_gain_smoothing_ms = value.clamp(1.0, 1000.0);
                        update_needed = true;
                    }
                    _ => {}
                },
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
                        *threshold_db = value.clamp(-60.0, 0.0);
                        update_needed = true;
                    }
                    1 => {
                        *ratio = value.clamp(1.0, 20.0);
                        update_needed = true;
                    }
                    2 => {
                        *attack_ms = value.clamp(0.1, 200.0);
                        update_needed = true;
                    }
                    3 => {
                        *release_ms = value.clamp(10.0, 2000.0);
                        update_needed = true;
                    }
                    4 => {
                        *knee_db = value.clamp(0.0, 24.0);
                        update_needed = true;
                    }
                    5 => {
                        *makeup_gain_db = value.clamp(-12.0, 24.0);
                        update_needed = true;
                    }
                    6 => {
                        *mix = (value / 100.0).clamp(0.0, 1.0); // Convert from 0-100% to 0-1
                        update_needed = true;
                    }
                    7 => {
                        *auto_makeup = value > 0.5;
                        update_needed = true;
                    }
                    8 => {
                        *link_channels = value > 0.5;
                        update_needed = true;
                    }
                    9 => {
                        *sidechain_hpf_hz = value.clamp(20.0, 500.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Limiter {
                    threshold_db,
                    release_ms,
                    lookahead_ms,
                    soft,
                    mix,
                } => match param_idx {
                    0 => {
                        *threshold_db = value.clamp(-30.0, 0.0);
                        update_needed = true;
                    }
                    1 => {
                        *release_ms = value.clamp(10.0, 1000.0);
                        update_needed = true;
                    }
                    2 => {
                        *lookahead_ms = value.clamp(0.0, 20.0);
                        update_needed = true;
                    }
                    3 => {
                        *soft = value > 0.5;
                        update_needed = true;
                    }
                    4 => {
                        *mix = (value / 100.0).clamp(0.0, 1.0); // Convert from 0-100% to 0-1
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Gate {
                    threshold_db,
                    ratio,
                    attack_ms,
                    hold_ms,
                    release_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => match param_idx {
                    0 => {
                        *threshold_db = value.clamp(-80.0, 0.0);
                        update_needed = true;
                    }
                    1 => {
                        *ratio = value.clamp(1.0, 100.0);
                        update_needed = true;
                    }
                    2 => {
                        *attack_ms = value.clamp(0.1, 100.0);
                        update_needed = true;
                    }
                    3 => {
                        *hold_ms = value.clamp(0.0, 1000.0);
                        update_needed = true;
                    }
                    4 => {
                        *release_ms = value.clamp(1.0, 1000.0);
                        update_needed = true;
                    }
                    5 => {
                        *mix = (value / 100.0).clamp(0.0, 1.0); // Convert from 0-100% to 0-1
                        update_needed = true;
                    }
                    6 => {
                        *link_channels = value > 0.5;
                        update_needed = true;
                    }
                    7 => {
                        *sidechain_hpf_hz = value.clamp(0.0, 200.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::BinauralDecoder {
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    ..
                } => match param_idx {
                    2 => {
                        *enable_optimization = value > 0.5;
                        update_needed = true;
                    }
                    3 => {
                        *externalization = value.clamp(0.0, 1.0);
                        update_needed = true;
                    }
                    4 => {
                        *near_field_strength = value.clamp(0.0, 1.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Convolution { mix, gain_db, .. } => {
                    use sotf_audio_player::param_specs::convolution::*;
                    match param_idx {
                        0 => {
                            *mix = value.clamp(MIX_MIN as f64, MIX_MAX as f64);
                            update_needed = true;
                        }
                        1 => {
                            *gain_db = value.clamp(GAIN_DB_MIN as f64, GAIN_DB_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::SpectrumAnalyzer {
                    num_bins,
                    min_freq,
                    max_freq,
                    smoothing,
                    ..
                } => match param_idx {
                    0 => {
                        *num_bins = (value as usize).clamp(10, 256);
                        update_needed = true;
                    }
                    1 => {
                        *min_freq = (value as f32).clamp(20.0, 20000.0);
                        update_needed = true;
                    }
                    2 => {
                        *max_freq = (value as f32).clamp(20.0, 20000.0);
                        update_needed = true;
                    }
                    3 => {
                        *smoothing = (value as f32).clamp(0.0, 1.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::ChannelMuteSolo { enabled, .. } => match param_idx {
                    0 => {
                        *enabled = value > 0.5;
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Expander {
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    range_db,
                    knee_db,
                    hysteresis_db,
                    hold_ms,
                    mix,
                    link_channels,
                    sidechain_hpf_hz,
                } => {
                    use sotf_audio_player::param_specs::expander::*;
                    match param_idx {
                        0 => {
                            *threshold_db = value.clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            update_needed = true;
                        }
                        1 => {
                            *ratio = value.clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            update_needed = true;
                        }
                        2 => {
                            *attack_ms = value.clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            update_needed = true;
                        }
                        3 => {
                            *release_ms = value.clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *range_db = value.clamp(RANGE_MIN as f64, RANGE_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *knee_db = value.clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *hysteresis_db =
                                value.clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64);
                            update_needed = true;
                        }
                        7 => {
                            *hold_ms = value.clamp(HOLD_MIN as f64, HOLD_MAX as f64);
                            update_needed = true;
                        }
                        8 => {
                            *mix = (value / 100.0).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            update_needed = true;
                        }
                        9 => {
                            *link_channels = value > 0.5;
                            update_needed = true;
                        }
                        10 => {
                            *sidechain_hpf_hz = value
                                .clamp(SIDECHAIN_HPF_HZ_MIN as f64, SIDECHAIN_HPF_HZ_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::MultibandCompressor {
                    num_bands,
                    crossover_preset,
                    crossover_freq_1,
                    crossover_freq_2,
                    crossover_freq_3,
                    crossover_freq_4,
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    knee_db,
                    mix,
                    link_channels,
                    ..
                } => {
                    use sotf_audio_player::param_specs::multiband_compressor::*;
                    match param_idx {
                        0 => {
                            *num_bands = (value as usize).clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
                            update_needed = true;
                        }
                        1 => {
                            *crossover_preset =
                                (value as i32).clamp(CROSSOVER_PRESET_MIN, CROSSOVER_PRESET_MAX);
                            update_needed = true;
                        }
                        2 => {
                            *crossover_freq_1 = value
                                .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64);
                            update_needed = true;
                        }
                        3 => {
                            *crossover_freq_2 = value
                                .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *crossover_freq_3 = value
                                .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *crossover_freq_4 = value
                                .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *threshold_db = value.clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            update_needed = true;
                        }
                        7 => {
                            *ratio = value.clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            update_needed = true;
                        }
                        8 => {
                            *attack_ms = value.clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            update_needed = true;
                        }
                        9 => {
                            *release_ms = value.clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            update_needed = true;
                        }
                        10 => {
                            *knee_db = value.clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            update_needed = true;
                        }
                        11 => {
                            *mix = (value / 100.0).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            update_needed = true;
                        }
                        12 => {
                            *link_channels = value > 0.5;
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::MultibandExpander {
                    num_bands,
                    crossover_preset,
                    crossover_freq_1,
                    crossover_freq_2,
                    crossover_freq_3,
                    crossover_freq_4,
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    range_db,
                    knee_db,
                    hysteresis_db,
                    hold_ms,
                    mix,
                    link_channels,
                    ..
                } => {
                    use sotf_audio_player::param_specs::multiband_expander::*;
                    match param_idx {
                        0 => {
                            *num_bands = (value as usize).clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
                            update_needed = true;
                        }
                        1 => {
                            *crossover_preset =
                                (value as i32).clamp(CROSSOVER_PRESET_MIN, CROSSOVER_PRESET_MAX);
                            update_needed = true;
                        }
                        2 => {
                            *crossover_freq_1 = value
                                .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64);
                            update_needed = true;
                        }
                        3 => {
                            *crossover_freq_2 = value
                                .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *crossover_freq_3 = value
                                .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *crossover_freq_4 = value
                                .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *threshold_db = value.clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64);
                            update_needed = true;
                        }
                        7 => {
                            *ratio = value.clamp(RATIO_MIN as f64, RATIO_MAX as f64);
                            update_needed = true;
                        }
                        8 => {
                            *attack_ms = value.clamp(ATTACK_MIN as f64, ATTACK_MAX as f64);
                            update_needed = true;
                        }
                        9 => {
                            *release_ms = value.clamp(RELEASE_MIN as f64, RELEASE_MAX as f64);
                            update_needed = true;
                        }
                        10 => {
                            *range_db = value.clamp(RANGE_MIN as f64, RANGE_MAX as f64);
                            update_needed = true;
                        }
                        11 => {
                            *knee_db = value.clamp(KNEE_MIN as f64, KNEE_MAX as f64);
                            update_needed = true;
                        }
                        12 => {
                            *hysteresis_db =
                                value.clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64);
                            update_needed = true;
                        }
                        13 => {
                            *hold_ms = value.clamp(HOLD_MIN as f64, HOLD_MAX as f64);
                            update_needed = true;
                        }
                        14 => {
                            *mix = (value / 100.0).clamp(MIX_MIN as f64, MIX_MAX as f64);
                            update_needed = true;
                        }
                        15 => {
                            *link_channels = value > 0.5;
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::XTC {
                    distance_m,
                    speaker_angle_deg,
                    head_radius_m,
                    beta_base,
                    beta_low_freq_boost,
                    beta_high_freq_boost,
                    head_shadow_cutoff_hz,
                    head_shadow_slope_db_per_octave,
                } => match param_idx {
                    0 => {
                        *distance_m = value.clamp(0.5, 5.0);
                        update_needed = true;
                    }
                    1 => {
                        *speaker_angle_deg = value.clamp(10.0, 60.0);
                        update_needed = true;
                    }
                    2 => {
                        // Value comes as cm, convert to meters
                        *head_radius_m = (value / 100.0).clamp(0.05, 0.15);
                        update_needed = true;
                    }
                    3 => {
                        // Value comes as scaled (×1000), convert back
                        *beta_base = (value / 1000.0).clamp(0.0001, 0.1);
                        update_needed = true;
                    }
                    4 => {
                        *beta_low_freq_boost = value.clamp(1.0, 100.0);
                        update_needed = true;
                    }
                    5 => {
                        *beta_high_freq_boost = value.clamp(1.0, 100.0);
                        update_needed = true;
                    }
                    6 => {
                        *head_shadow_cutoff_hz = value.clamp(1000.0, 10000.0);
                        update_needed = true;
                    }
                    7 => {
                        *head_shadow_slope_db_per_octave = value.clamp(0.0, 12.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Denoiser {
                    reduction_db,
                    floor_db,
                    smoothing,
                    attack_ms,
                    release_ms,
                    low_latency,
                    polyphonic_detection,
                    dd_enabled,
                    dd_alpha,
                    psychoacoustic_masking,
                    use_captured_profile,
                } => {
                    use sotf_audio_player::param_specs::denoiser::*;
                    match param_idx {
                        0 => {
                            *reduction_db =
                                value.clamp(REDUCTION_DB_MIN as f64, REDUCTION_DB_MAX as f64);
                            update_needed = true;
                        }
                        1 => {
                            *floor_db = value.clamp(FLOOR_DB_MIN as f64, FLOOR_DB_MAX as f64);
                            update_needed = true;
                        }
                        2 => {
                            // Value comes as percentage, convert to 0-0.99
                            *smoothing =
                                (value / 100.0).clamp(SMOOTHING_MIN as f64, SMOOTHING_MAX as f64);
                            update_needed = true;
                        }
                        3 => {
                            *attack_ms = value.clamp(ATTACK_MS_MIN as f64, ATTACK_MS_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *release_ms = value.clamp(RELEASE_MS_MIN as f64, RELEASE_MS_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *low_latency = value > 0.5;
                            update_needed = true;
                        }
                        6 => {
                            *polyphonic_detection = value > 0.5;
                            update_needed = true;
                        }
                        7 => {
                            *dd_enabled = value > 0.5;
                            update_needed = true;
                        }
                        8 => {
                            *dd_alpha =
                                value.clamp(DD_ALPHA_MIN as f64, DD_ALPHA_MAX as f64);
                            update_needed = true;
                        }
                        9 => {
                            *psychoacoustic_masking = value > 0.5;
                            update_needed = true;
                        }
                        10 => {
                            // learn_noise trigger: value > 0.5 starts learning
                            update_needed = true;
                        }
                        11 => {
                            *use_captured_profile = value > 0.5;
                            update_needed = true;
                        }
                        12 => {
                            // clear_profile trigger: value > 0.5 clears
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::Pnd {
                    correction_strength,
                    analysis_window_ms,
                    drift_smoothing,
                } => {
                    use sotf_audio_player::param_specs::pnd::*;
                    match param_idx {
                        0 => {
                            // Value comes as percentage, convert to 0-2.0
                            *correction_strength = (value / 100.0).clamp(
                                CORRECTION_STRENGTH_MIN as f64,
                                CORRECTION_STRENGTH_MAX as f64,
                            );
                            update_needed = true;
                        }
                        1 => {
                            *analysis_window_ms = value.clamp(
                                ANALYSIS_WINDOW_MS_MIN as f64,
                                ANALYSIS_WINDOW_MS_MAX as f64,
                            );
                            update_needed = true;
                        }
                        2 => {
                            // Value comes as ×1000, convert back
                            *drift_smoothing = (value / 1000.0)
                                .clamp(DRIFT_SMOOTHING_MIN as f64, DRIFT_SMOOTHING_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::ABCompare {
                    mix,
                    mix_mode,
                    selected_path,
                    bypass,
                    auto_gain_enabled,
                    loudness_type,
                    max_auto_gain_db,
                    gain_smoothing_ms,
                    mix_transition_ms,
                    ..
                } => match param_idx {
                    0 => {
                        // Value comes as percentage, convert to -1.0 to 1.0
                        *mix = (value / 100.0).clamp(-1.0, 1.0);
                        update_needed = true;
                    }
                    1 => {
                        *mix_mode = if value > 0.5 { 1 } else { 0 };
                        update_needed = true;
                    }
                    2 => {
                        *selected_path = if value > 0.5 { 1 } else { 0 };
                        update_needed = true;
                    }
                    3 => {
                        *bypass = value > 0.5;
                        update_needed = true;
                    }
                    4 => {
                        *auto_gain_enabled = value > 0.5;
                        update_needed = true;
                    }
                    5 => {
                        *loudness_type = if value > 0.5 { 1 } else { 0 };
                        update_needed = true;
                    }
                    6 => {
                        *max_auto_gain_db = value.clamp(0.0, 24.0);
                        update_needed = true;
                    }
                    7 => {
                        *gain_smoothing_ms = value.clamp(10.0, 500.0);
                        update_needed = true;
                    }
                    8 => {
                        *mix_transition_ms = value.clamp(5.0, 500.0);
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::FletcherMunson {
                    playback_volume_db,
                    reference_level_db,
                    enabled,
                    smoothing_ms,
                    auto_gain_enabled,
                    auto_gain_max_db,
                    auto_gain_smoothing_ms,
                    auto_gain_loudness_type,
                    band1_freq,
                    band1_q,
                    band1_max_gain,
                    band1_slope,
                    band2_freq,
                    band2_q,
                    band2_max_gain,
                    band2_slope,
                    band3_freq,
                    band3_q,
                    band3_max_gain,
                    band3_slope,
                    band4_freq,
                    band4_q,
                    band4_max_gain,
                    band4_slope,
                } => {
                    use sotf_audio_player::param_specs::fletcher_munson::*;
                    match param_idx {
                        0 => {
                            *playback_volume_db = value.clamp(
                                PLAYBACK_VOLUME_DB_MIN as f64,
                                PLAYBACK_VOLUME_DB_MAX as f64,
                            );
                            update_needed = true;
                        }
                        1 => {
                            *reference_level_db = value.clamp(
                                REFERENCE_LEVEL_DB_MIN as f64,
                                REFERENCE_LEVEL_DB_MAX as f64,
                            );
                            update_needed = true;
                        }
                        2 => {
                            *enabled = value > 0.5;
                            update_needed = true;
                        }
                        3 => {
                            *smoothing_ms =
                                value.clamp(SMOOTHING_MS_MIN as f64, SMOOTHING_MS_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *auto_gain_enabled = value > 0.5;
                            update_needed = true;
                        }
                        5 => {
                            *auto_gain_max_db = value
                                .clamp(AUTO_GAIN_MAX_DB_MIN as f64, AUTO_GAIN_MAX_DB_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *auto_gain_smoothing_ms = value.clamp(
                                AUTO_GAIN_SMOOTHING_MS_MIN as f64,
                                AUTO_GAIN_SMOOTHING_MS_MAX as f64,
                            );
                            update_needed = true;
                        }
                        7 => {
                            *auto_gain_loudness_type = (value as i32).clamp(0, 1);
                            update_needed = true;
                        }
                        _ => {
                            if param_idx >= 8 && param_idx < 24 {
                                let rel_idx = param_idx - 8;
                                let band_idx = (rel_idx / 4) + 1;
                                let field_idx = rel_idx % 4;

                                let (freq, q, max_gain, slope) = match band_idx {
                                    1 => (band1_freq, band1_q, band1_max_gain, band1_slope),
                                    2 => (band2_freq, band2_q, band2_max_gain, band2_slope),
                                    3 => (band3_freq, band3_q, band3_max_gain, band3_slope),
                                    4 => (band4_freq, band4_q, band4_max_gain, band4_slope),
                                    _ => return,
                                };

                                match field_idx {
                                    0 => {
                                        *freq =
                                            value.clamp(BAND_FREQ_MIN as f64, BAND_FREQ_MAX as f64);
                                        update_needed = true;
                                    }
                                    1 => {
                                        *q = value.clamp(BAND_Q_MIN as f64, BAND_Q_MAX as f64);
                                        update_needed = true;
                                    }
                                    2 => {
                                        *max_gain = value.clamp(
                                            BAND_MAX_GAIN_MIN as f64,
                                            BAND_MAX_GAIN_MAX as f64,
                                        );
                                        update_needed = true;
                                    }
                                    3 => {
                                        *slope = value
                                            .clamp(BAND_SLOPE_MIN as f64, BAND_SLOPE_MAX as f64);
                                        update_needed = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                PluginSettings::Downmix {
                    center_gain_db,
                    surround_gain_db,
                    height_gain_db,
                    lfe_gain_db,
                    phase_coherence,
                    phase_blend_low_hz,
                    phase_blend_high_hz,
                    ..
                } => {
                    use sotf_plugins::param_specs::downmix::*;
                    match param_idx {
                        0 => {
                            *center_gain_db = value.clamp(CENTER_GAIN_DB_MIN as f64, CENTER_GAIN_DB_MAX as f64);
                            update_needed = true;
                        }
                        1 => {
                            *surround_gain_db = value.clamp(SURROUND_GAIN_DB_MIN as f64, SURROUND_GAIN_DB_MAX as f64);
                            update_needed = true;
                        }
                        2 => {
                            *height_gain_db = value.clamp(HEIGHT_GAIN_DB_MIN as f64, HEIGHT_GAIN_DB_MAX as f64);
                            update_needed = true;
                        }
                        3 => {
                            *lfe_gain_db = value.clamp(LFE_GAIN_DB_MIN as f64, LFE_GAIN_DB_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *phase_coherence = value > 0.5;
                            update_needed = true;
                        }
                        5 => {
                            *phase_blend_low_hz = value.clamp(PHASE_BLEND_LOW_HZ_MIN as f64, PHASE_BLEND_LOW_HZ_MAX as f64);
                            update_needed = true;
                        }
                        6 => {
                            *phase_blend_high_hz = value.clamp(PHASE_BLEND_HIGH_HZ_MIN as f64, PHASE_BLEND_HIGH_HZ_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::MonoToStereo {
                    stereo_width,
                    haas_delay_ms,
                    enable_comp_eq,
                    comp_eq_depth_db,
                    decor_low_hz,
                    decor_high_hz,
                } => {
                    use sotf_plugins::param_specs::mono_to_stereo::*;
                    match param_idx {
                        0 => {
                            *stereo_width = value.clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64);
                            update_needed = true;
                        }
                        1 => {
                            *haas_delay_ms = value.clamp(HAAS_DELAY_MS_MIN as f64, HAAS_DELAY_MS_MAX as f64);
                            update_needed = true;
                        }
                        2 => {
                            *enable_comp_eq = value > 0.5;
                            update_needed = true;
                        }
                        3 => {
                            *comp_eq_depth_db = value.clamp(COMP_EQ_DEPTH_DB_MIN as f64, COMP_EQ_DEPTH_DB_MAX as f64);
                            update_needed = true;
                        }
                        4 => {
                            *decor_low_hz = value.clamp(DECOR_LOW_HZ_MIN as f64, DECOR_LOW_HZ_MAX as f64);
                            update_needed = true;
                        }
                        5 => {
                            *decor_high_hz = value.clamp(DECOR_HIGH_HZ_MIN as f64, DECOR_HIGH_HZ_MAX as f64);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::BandSplit {
                    frequency,
                    crossover_type,
                    ..
                } => {
                    use sotf_plugins::param_specs::band_split::*;
                    match param_idx {
                        0 => {
                            *frequency = value.clamp(FREQUENCY_MIN, FREQUENCY_MAX);
                            update_needed = true;
                        }
                        1 => {
                            *crossover_type = if value > 0.5 {
                                "LR48".to_string()
                            } else {
                                "LR24".to_string()
                            };
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                PluginSettings::BandMerge { bands, .. } => {
                    use sotf_plugins::param_specs::band_merge::*;
                    match param_idx {
                        0 => {
                            *bands = (value as usize).clamp(BANDS_MIN, BANDS_MAX);
                            update_needed = true;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if channel_count_changed {
            self.plugin_state
                .plugin_chain
                .update_channel_dependent_plugins();
        }

        if update_needed {
            // Determine update type based on whether this parameter supports individual updates
            let update_type = if channel_count_changed {
                // Channel count changes always require structural update
                PluginUpdateType::Structural
            } else if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin(plugin_idx) {
                if param_index_to_engine_param(&plugin.settings, param_idx).is_some() {
                    PluginUpdateType::Parameter {
                        plugin_index: plugin_idx,
                        param_index: param_idx,
                    }
                } else {
                    PluginUpdateType::Structural
                }
            } else {
                PluginUpdateType::Structural
            };
            self.plugin_state.pending_plugin_update = Some(update_type);
        }
    }

    /// Set a string parameter value for a plugin (e.g., path configs, file paths)
    fn set_plugin_param_string(&mut self, plugin_idx: usize, param_idx: usize, value: String) {
        let mut update_needed = false;

        if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(plugin_idx) {
            match &mut plugin.settings {
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => match param_idx {
                    9 => {
                        *path_a_config = value;
                        update_needed = true;
                    }
                    10 => {
                        *path_b_config = value;
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Convolution { ir_file, .. } => {
                    if param_idx == 0 {
                        *ir_file = value;
                        update_needed = true;
                    }
                }
                _ => {}
            }
        }

        if update_needed {
            // String parameters always require structural update
            self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        }
    }

    /// Set spectrum analyzer tilt correction mode
    fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        tilt: sotf_plugins::SpectralTiltCorrection,
    ) {
        if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer {
                tilt_correction, ..
            } = &mut plugin.settings
            {
                *tilt_correction = tilt;
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
            }
        }
    }

    /// Set spectrum analyzer tilt reference frequency
    fn set_spectrum_tilt_reference(
        &mut self,
        plugin_idx: usize,
        reference: sotf_plugins::TiltReferenceFreq,
    ) {
        if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer { tilt_reference, .. } = &mut plugin.settings {
                *tilt_reference = reference;
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
            }
        }
    }

    /// Reset a specific parameter to its default value
    fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize) {
        let plugin_type =
            if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin(plugin_idx) {
                plugin.plugin_type()
            } else {
                return;
            };

        // Create default settings for this plugin type
        let default_settings = PluginSettings::default_for(&plugin_type);

        // Get the default value for the parameter
        let default_value = match &default_settings {
            PluginSettings::Upmixer {
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                ..
            } => {
                match param_idx {
                    // 0: speaker_config - no default reset (keep current)
                    1 => *gain_front_direct,
                    2 => *gain_front_ambient,
                    3 => *gain_rear_ambient,
                    4 => *height_gain,
                    5 => *lfe_gain,
                    6 => *lfe_cutoff_hz,
                    7 => *stereo_width,
                    8 => *center_spread,
                    9 => *bandpass_hz,
                    10 => {
                        if *enable_subharmonic_synth {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    11 => *subharmonic_gain,
                    12 => {
                        if *enable_hr_direct {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    13 => *hr_sharpen,
                    14 => *safety_cap_db,
                    15 => *decorrelation_mode as f64,
                    16 => *subharmonic_freq_hz,
                    17 => *subharmonic_attack_ms,
                    18 => *subharmonic_release_ms,
                    19 => *decorrelation_lfo_rate_hz,
                    20 => *velvet_noise_duration_ms,
                    21 => *velvet_noise_density,
                    22 => *height_hf_cap_hz,
                    23 => *height_transient_reduction,
                    24 => *height_direct_leak,
                    25 => *surround_direct_bleed,
                    26 => *rear_ambient_boost,
                    27 => *rear_late_reflection,
                    28 => *ambient_boost,
                    29 => *dialogue_weight,
                    30 => *voice_freq_min_hz,
                    31 => *voice_freq_max_hz,
                    _ => return, // No reset for others or unknown
                }
            }
            PluginSettings::Gain { gain_db, .. } => {
                if param_idx == 0 {
                    *gain_db
                } else {
                    return;
                }
            }
            PluginSettings::Convolution { mix, gain_db, .. } => match param_idx {
                0 => *mix,
                1 => *gain_db,
                _ => return,
            },
            PluginSettings::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                use_captured_profile,
            } => match param_idx {
                0 => *reduction_db,
                1 => *floor_db,
                2 => *smoothing * 100.0, // Convert to percentage for UI
                3 => *attack_ms,
                4 => *release_ms,
                5 => {
                    if *low_latency {
                        1.0
                    } else {
                        0.0
                    }
                }
                6 => {
                    if *polyphonic_detection {
                        1.0
                    } else {
                        0.0
                    }
                }
                7 => {
                    if *dd_enabled {
                        1.0
                    } else {
                        0.0
                    }
                }
                8 => *dd_alpha,
                9 => {
                    if *psychoacoustic_masking {
                        1.0
                    } else {
                        0.0
                    }
                }
                10 => 0.0, // learn_noise trigger — always reads as 0
                11 => {
                    if *use_captured_profile {
                        1.0
                    } else {
                        0.0
                    }
                }
                12 => 0.0, // clear_profile trigger — always reads as 0
                _ => return,
            },
            PluginSettings::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => match param_idx {
                0 => *correction_strength * 100.0, // Convert to percentage for UI
                1 => *analysis_window_ms,
                2 => *drift_smoothing * 1000.0, // Convert to ×1000 for UI
                _ => return,
            },
            PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                ..
            } => match param_idx {
                0 => *mix * 100.0, // Convert to percentage for UI
                1 => *mix_mode as f64,
                2 => *selected_path as f64,
                3 => {
                    if *bypass {
                        1.0
                    } else {
                        0.0
                    }
                }
                4 => {
                    if *auto_gain_enabled {
                        1.0
                    } else {
                        0.0
                    }
                }
                5 => *loudness_type as f64,
                6 => *max_auto_gain_db,
                7 => *gain_smoothing_ms,
                8 => *mix_transition_ms,
                _ => return,
            },
            PluginSettings::FletcherMunson {
                playback_volume_db,
                reference_level_db,
                enabled,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                auto_gain_loudness_type,
                band1_freq,
                band1_q,
                band1_max_gain,
                band1_slope,
                band2_freq,
                band2_q,
                band2_max_gain,
                band2_slope,
                band3_freq,
                band3_q,
                band3_max_gain,
                band3_slope,
                band4_freq,
                band4_q,
                band4_max_gain,
                band4_slope,
            } => match param_idx {
                0 => *playback_volume_db as f64,
                1 => *reference_level_db as f64,
                2 => {
                    if *enabled {
                        1.0
                    } else {
                        0.0
                    }
                }
                3 => *smoothing_ms as f64,
                4 => {
                    if *auto_gain_enabled {
                        1.0
                    } else {
                        0.0
                    }
                }
                5 => *auto_gain_max_db as f64,
                6 => *auto_gain_smoothing_ms as f64,
                7 => *auto_gain_loudness_type as f64,
                _ => {
                    if param_idx >= 8 && param_idx < 24 {
                        let rel_idx = param_idx - 8;
                        let band_idx = (rel_idx / 4) + 1;
                        let field_idx = rel_idx % 4;

                        let (freq, q, max_gain, slope) = match band_idx {
                            1 => (band1_freq, band1_q, band1_max_gain, band1_slope),
                            2 => (band2_freq, band2_q, band2_max_gain, band2_slope),
                            3 => (band3_freq, band3_q, band3_max_gain, band3_slope),
                            4 => (band4_freq, band4_q, band4_max_gain, band4_slope),
                            _ => return,
                        };

                        match field_idx {
                            0 => *freq,
                            1 => *q,
                            2 => *max_gain,
                            3 => *slope,
                            _ => return,
                        }
                    } else {
                        return;
                    }
                }
            },
            PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => match param_idx {
                0 => *center_gain_db,
                1 => *surround_gain_db,
                2 => *height_gain_db,
                3 => *lfe_gain_db,
                4 => {
                    if *phase_coherence {
                        1.0
                    } else {
                        0.0
                    }
                }
                5 => *phase_blend_low_hz,
                6 => *phase_blend_high_hz,
                _ => return,
            },
            PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => match param_idx {
                0 => *stereo_width,
                1 => *haas_delay_ms,
                2 => {
                    if *enable_comp_eq {
                        1.0
                    } else {
                        0.0
                    }
                }
                3 => *comp_eq_depth_db,
                4 => *decor_low_hz,
                5 => *decor_high_hz,
                _ => return,
            },
            PluginSettings::BandSplit {
                frequency,
                crossover_type,
                ..
            } => match param_idx {
                0 => *frequency,
                1 => {
                    if crossover_type == "LR48" {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => return,
            },
            PluginSettings::BandMerge { bands, .. } => match param_idx {
                0 => *bands as f64,
                _ => return,
            },
            _ => return,
        };

        // Apply the value
        self.set_plugin_param(plugin_idx, param_idx, default_value);
    }

    /// Load EQ filters from APO file
    fn load_apo_file(&mut self) -> Result<(), String> {
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

        let path = Path::new(&self.input_state.apo_file_input);

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                channel_filters,
                per_channel_mode,
                ..
            } = &plugin.settings
            {
                let channels = *channels;
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                };
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Update SOFA file path for the currently editing binaural decoder plugin
    fn load_sofa_file(&mut self) -> Result<(), String> {
        // Check plugin state before loading file (or rather, setting path)
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                return Err("Selected plugin is not a Binaural Decoder".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        // Clone the sofa_file_input before borrowing plugin mutably
        let sofa_file_path = self.input_state.sofa_file_input.clone();

        // Update the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = sofa_file_path;
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Add a new EQ band to the currently editing EQ plugin
    /// Returns Ok(()) if successful, Err if no EQ plugin is being edited
    fn add_eq_band(&mut self) -> Result<(), String> {
        use math_audio_iir_fir::BiquadFilterType;
        use sotf_audio_player::EQFilter;

        // Check plugin state before adding band
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        // Add a new filter to the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
            } = &mut plugin.settings
            {
                // Create a new default peak filter at 1kHz
                let new_filter = EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0);
                filters.push(new_filter);

                // Clone values for reassignment (borrow checker)
                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                };

                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Remove an EQ band from the currently editing EQ plugin
    /// Returns Ok(()) if successful, Err if no EQ plugin is being edited or invalid index
    fn remove_eq_band(&mut self, band_idx: usize) -> Result<(), String> {
        // Check plugin state before removing band
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        // Remove the filter from the currently editing plugin
        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters.remove(band_idx);

                // Clone values for reassignment (borrow checker)
                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                };

                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Toggle mute state for an EQ band
    fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<(), String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters[band_idx].muted = !filters[band_idx].muted;

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                };

                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Toggle solo state for an EQ band
    /// When any band is soloed, only soloed bands are active
    fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<(), String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::EQ { .. }) {
                return Err("Selected plugin is not an EQ".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
            } = &mut plugin.settings
            {
                if band_idx >= filters.len() {
                    return Err("Invalid band index".to_string());
                }

                filters[band_idx].solo = !filters[band_idx].solo;

                let channels = *channels;
                let filters = filters.clone();
                let channel_filters = channel_filters.clone();
                let per_channel_mode = *per_channel_mode;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                };

                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Set the EQ plugin to per-channel mode or global mode
    fn set_eq_per_channel_mode(&mut self, plugin_idx: usize, per_channel: bool) {
        if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(plugin_idx) {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
            } = &mut plugin.settings
            {
                // When switching to per-channel mode, initialize channel_filters if needed
                if per_channel && channel_filters.is_none() {
                    // Initialize with copies of the global filters for each channel
                    let num_channels = *channels;
                    let mut ch_filters = Vec::with_capacity(num_channels);
                    for _ in 0..num_channels {
                        ch_filters.push(filters.clone());
                    }
                    *channel_filters = Some(ch_filters);
                }

                *per_channel_mode = per_channel;
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
            }
        }
    }

    // Plugin preset save/load methods

    /// Refresh the list of available plugin presets from the config directory
    fn refresh_plugin_presets(&mut self) {
        self.plugin_state.available_plugin_presets.clear();
        self.plugin_state.selected_preset_index = 0;

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
                    self.plugin_state
                        .available_plugin_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            // Sort presets alphabetically
            self.plugin_state.available_plugin_presets.sort();
        }

        log::info!(
            "Found {} plugin presets",
            self.plugin_state.available_plugin_presets.len()
        );
    }

    /// Save plugin chain to file
    fn save_plugin_chain(&mut self) {
        if self.input_state.plugin_file_input.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        // Check if file exists and show warning if overwriting
        let filename_with_ext = if self.input_state.plugin_file_input.ends_with(".json") {
            self.input_state.plugin_file_input.clone()
        } else {
            format!("{}.json", self.input_state.plugin_file_input)
        };

        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };
        let full_path = presets_dir.join(&filename_with_ext);
        if full_path.exists() {
            log::warn!("Overwriting existing preset: {}", filename_with_ext);
        }

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        match self
            .plugin_state
            .plugin_chain
            .save_to_file(&presets_dir, &self.input_state.plugin_file_input)
        {
            Ok(_) => {
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Saved preset: {}",
                    filename_with_ext
                )));
                self.plugin_state.last_loaded_preset = Some(filename_with_ext);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.ui_state.toast_message =
                    Some(ToastMessage::error(format!("Error saving: {}", e)));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    /// Save plugin chain to selected preset file (overwrite)
    fn save_selected_preset(&mut self) {
        if self.plugin_state.available_plugin_presets.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No presets available".to_string()));
            return;
        }

        if let Some(preset_filename) = self
            .plugin_state
            .available_plugin_presets
            .get(self.plugin_state.selected_preset_index)
            .cloned()
        {
            // Save using the plugin chain's own save method
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.ui_state.toast_message = Some(ToastMessage::error(
                    "Could not find presets directory".to_string(),
                ));
                return;
            };
            match self
                .plugin_state
                .plugin_chain
                .save_to_file(&presets_dir, &preset_filename)
            {
                Ok(_) => {
                    self.ui_state.toast_message = Some(ToastMessage::success(format!(
                        "Overwritten preset: {}",
                        preset_filename
                    )));
                    self.plugin_state.last_loaded_preset = Some(preset_filename);
                    // Refresh presets list
                    self.refresh_plugin_presets();
                }
                Err(e) => {
                    self.ui_state.toast_message =
                        Some(ToastMessage::error(format!("Error saving: {}", e)));
                    log::error!("Failed to save plugin chain: {}", e);
                }
            }
        }
    }

    /// Load plugin chain from file
    fn load_plugin_chain(&mut self) {
        if self.input_state.plugin_file_input.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        // Load using the plugin chain's own load method (handles path, extension, etc.)
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };
        match self
            .plugin_state
            .plugin_chain
            .load_from_file(&presets_dir, &self.input_state.plugin_file_input)
        {
            Ok(_) => {
                // Update BinauralDecoder input channels after loading
                self.plugin_state
                    .plugin_chain
                    .update_channel_dependent_plugins();

                // Get the final filename (with .json appended if needed)
                let filename = if self.input_state.plugin_file_input.ends_with(".json") {
                    self.input_state.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.input_state.plugin_file_input)
                };

                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Loaded preset: {}",
                    filename
                )));
                self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                self.sync_spectrum_visible();
                self.plugin_state.last_loaded_preset = Some(filename);
            }
            Err(e) => {
                self.ui_state.toast_message =
                    Some(ToastMessage::error(format!("Error loading: {}", e)));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    /// Load the selected preset from the available presets list
    fn load_selected_preset(&mut self) {
        if self.plugin_state.available_plugin_presets.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No presets available".to_string()));
            return;
        }

        if let Some(preset_filename) = self
            .plugin_state
            .available_plugin_presets
            .get(self.plugin_state.selected_preset_index)
            .cloned()
        {
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.ui_state.toast_message = Some(ToastMessage::error(
                    "Could not find presets directory".to_string(),
                ));
                return;
            };
            match self
                .plugin_state
                .plugin_chain
                .load_from_file(&presets_dir, &preset_filename)
            {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_state
                        .plugin_chain
                        .update_channel_dependent_plugins();

                    self.ui_state.toast_message = Some(ToastMessage::success(format!(
                        "Loaded preset: {} ({} plugins)",
                        preset_filename,
                        self.plugin_state.plugin_chain.len()
                    )));
                    self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                    self.sync_spectrum_visible();
                    self.plugin_state.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.ui_state.toast_message =
                        Some(ToastMessage::error(format!("Error loading preset: {}", e)));
                    log::error!("Failed to load plugin chain: {}", e);
                }
            }
        }
    }

    /// Select the next preset in the list
    fn select_next_preset(&mut self) {
        if !self.plugin_state.available_plugin_presets.is_empty() {
            self.plugin_state.selected_preset_index = (self.plugin_state.selected_preset_index + 1)
                % self.plugin_state.available_plugin_presets.len();
        }
    }

    /// Select the previous preset in the list
    fn select_previous_preset(&mut self) {
        if !self.plugin_state.available_plugin_presets.is_empty() {
            if self.plugin_state.selected_preset_index == 0 {
                self.plugin_state.selected_preset_index =
                    self.plugin_state.available_plugin_presets.len() - 1;
            } else {
                self.plugin_state.selected_preset_index -= 1;
            }
        }
    }
}

// Helper function to get parameter count for a plugin
pub fn get_param_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::Upmixer { .. } => 32, // All 32 upmixer parameters
        PluginSettings::EQ { filters, .. } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Gate { .. } => 8, // threshold, ratio, attack, hold, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 5, // threshold, release, lookahead, soft, mix
        PluginSettings::LoudnessCompensation { .. } => 7, // low_freq, low_gain, high_freq, high_gain, auto_gain_enabled, auto_gain_max_db, auto_gain_smoothing_ms
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Convolution { .. } => 2,     // mix, gain_db
        PluginSettings::LoudnessMonitor => 0,        // No parameters
        PluginSettings::SpectrumAnalyzer { .. } => 4, // num_bins, min_freq, max_freq, smoothing
        PluginSettings::Gain { .. } => 1,            // gain_db
        PluginSettings::ChannelMuteSolo { .. } => 0, // No editable params (toggles only)
        PluginSettings::Matrix { .. } => 0,          // Matrix is edited via grid UI, not params
        PluginSettings::Expander { .. } => 11, // threshold, ratio, attack, release, range, knee, hysteresis, hold, mix, link_channels, sidechain_hpf_hz
        PluginSettings::MultibandCompressor { .. } => 13, // num_bands, crossover_preset, crossover_freq_1-4, threshold, ratio, attack, release, knee, mix, link_channels
        PluginSettings::MultibandExpander { .. } => 16, // num_bands, crossover_preset, crossover_freq_1-4, threshold, ratio, attack, release, range, knee, hysteresis, hold, mix, link_channels
        PluginSettings::XTC { .. } => 8, // distance, angle, head_radius, beta_base, beta_low_freq_boost, beta_high_freq_boost, head_shadow_cutoff, head_shadow_slope
        PluginSettings::Denoiser { .. } => 13, // reduction_db, floor_db, smoothing, attack_ms, release_ms, low_latency, polyphonic_detection, dd_enabled, dd_alpha, psychoacoustic_masking, learn_noise, use_captured_profile, clear_profile
        PluginSettings::Pnd { .. } => 3, // correction_strength, analysis_window_ms, drift_smoothing
        PluginSettings::ABCompare { .. } => 9, // mix, mix_mode, selected_path, bypass, auto_gain_enabled, loudness_type, max_auto_gain_db, gain_smoothing_ms, mix_transition_ms
        PluginSettings::FletcherMunson { .. } => 22, // reference_level, smoothing, 4 bands x 4 params each, + 4 auto-gain params
        PluginSettings::BandSplit { .. } => 2, // frequency, crossover_type
        PluginSettings::BandMerge { .. } => 1, // bands
        PluginSettings::Downmix { .. } => 7, // center, surround, height, lfe, phase_coherence, blend_low, blend_high
        PluginSettings::MonoToStereo { .. } => 6, // width, haas, comp_eq, depth, decor_low, decor_high
    }
}
