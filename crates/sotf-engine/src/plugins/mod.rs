//! Plugin type definitions, settings, and utilities

pub use chain::PluginChain;
pub use eq::{EQFilter, EqFilterTopology, KautzSectionConfig};
pub use matrix::{
    apply_matrix_preset, available_matrix_presets, detect_matrix_preset, resize_matrix,
    upmixer_output_channels,
};
use sotf_plugins::param_specs::aae as aae_specs;
use sotf_plugins::param_specs::ab_compare as ab_compare_specs;
use sotf_plugins::param_specs::aec as aec_specs;
use sotf_plugins::param_specs::ambisonics as ambisonics_specs;
use sotf_plugins::param_specs::band_merge as band_merge_specs;
use sotf_plugins::param_specs::band_split as band_split_specs;
use sotf_plugins::param_specs::beamformer as beamformer_specs;
use sotf_plugins::param_specs::binaural as binaural_specs;
use sotf_plugins::param_specs::channel_mute_solo as cms_specs;
use sotf_plugins::param_specs::compressor as compressor_specs;
use sotf_plugins::param_specs::convolution as convolution_specs;
use sotf_plugins::param_specs::crossfeed as crossfeed_specs;
use sotf_plugins::param_specs::de_esser as de_esser_specs;
use sotf_plugins::param_specs::declick as declick_specs;
use sotf_plugins::param_specs::denoiser as denoiser_specs;
use sotf_plugins::param_specs::downmix as downmix_specs;
use sotf_plugins::param_specs::dynamic_eq as dynamic_eq_specs;
use sotf_plugins::param_specs::expander as expander_specs;
use sotf_plugins::param_specs::fir_designer as fir_designer_specs;
use sotf_plugins::param_specs::gain as gain_specs;
use sotf_plugins::param_specs::gate as gate_specs;
use sotf_plugins::param_specs::hiss_reducer as hiss_reducer_specs;
use sotf_plugins::param_specs::limiter as limiter_specs;
use sotf_plugins::param_specs::linear_phase_eq as linear_phase_eq_specs;
use sotf_plugins::param_specs::loudness_compensation as lc_specs;
use sotf_plugins::param_specs::mono_to_stereo as mono_to_stereo_specs;
use sotf_plugins::param_specs::multiband_compressor as mb_compressor_specs;
use sotf_plugins::param_specs::multiband_expander as mb_expander_specs;
use sotf_plugins::param_specs::pnd as pnd_specs;
use sotf_plugins::param_specs::saturation as saturation_specs;
use sotf_plugins::param_specs::spectral_compressor as spectral_compressor_specs;
use sotf_plugins::param_specs::spectrum as spectrum_specs;
use sotf_plugins::param_specs::speech_denoiser as speech_denoiser_specs;
use sotf_plugins::param_specs::stereo_imager as stereo_imager_specs;
use sotf_plugins::param_specs::transient_shaper as transient_shaper_specs;
use sotf_plugins::param_specs::upmixer as upmixer_specs;
use sotf_plugins::param_specs::xtc as xtc_specs;
pub use utility::{
    db_to_linear, get_channel_label, get_channel_label_from_config, linear_to_db_string,
    plugins_to_path_config_json, preset_file_to_path_config_json,
};

sotf_plugins::serde_param_default! {
    upmixer_specs::PARAMS;
    fn default_upmixer_subharmonic_gain() -> f64 = "subharmonic_gain";
    fn default_upmixer_hr_sharpen() -> f64 = "hr_sharpen";
    fn default_upmixer_safety_cap_db() -> f64 = "safety_cap_db";
    fn default_upmixer_center_spread() -> f64 = "center_spread";
    fn default_upmixer_surround_direct_bleed() -> f64 = "surround_direct_bleed";
    fn default_upmixer_rear_late_reflection() -> f64 = "rear_late_reflection";
    fn default_upmixer_subharmonic_freq_hz() -> f64 = "subharmonic_freq_hz";
    fn default_upmixer_subharmonic_attack_ms() -> f64 = "subharmonic_attack_ms";
    fn default_upmixer_subharmonic_release_ms() -> f64 = "subharmonic_release_ms";
    fn default_upmixer_decorrelation_lfo_rate_hz() -> f64 = "decorrelation_lfo_rate_hz";
    fn default_upmixer_velvet_noise_duration_ms() -> f64 = "velvet_noise_duration_ms";
    fn default_upmixer_velvet_noise_density() -> f64 = "velvet_noise_density";
    fn default_upmixer_height_hf_cap_hz() -> f64 = "height_hf_cap_hz";
    fn default_upmixer_height_transient_reduction() -> f64 = "height_transient_reduction";
    fn default_upmixer_height_direct_leak() -> f64 = "height_direct_leak";
    fn default_upmixer_rear_ambient_boost() -> f64 = "rear_ambient_boost";
    fn default_upmixer_ambient_boost() -> f64 = "ambient_boost";
    fn default_upmixer_dialogue_weight() -> f64 = "dialogue_weight";
    fn default_upmixer_dialogue_centroid_weight() -> f64 = "dialogue_centroid_weight";
    fn default_upmixer_dialogue_variance_weight() -> f64 = "dialogue_variance_weight";
    fn default_upmixer_dialogue_coherence_weight() -> f64 = "dialogue_coherence_weight";
    fn default_upmixer_voice_freq_min_hz() -> f64 = "voice_freq_min_hz";
    fn default_upmixer_voice_freq_max_hz() -> f64 = "voice_freq_max_hz";
    fn default_upmixer_enable_hr_direct() -> bool = "enable_hr_direct";
    fn default_upmixer_multi_source_threshold() -> f64 = "multi_source_threshold";
    fn default_upmixer_frequency_resolution() -> usize = "frequency_resolution";
    fn default_upmixer_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_upmixer_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_upmixer_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
}
sotf_plugins::serde_param_default! {
    aae_specs::PARAMS;
    fn default_aae_speaker_config() -> String = "speaker_config";
    fn default_aae_room_size() -> f64 = "room_size";
    fn default_aae_rt60() -> f64 = "rt60";
    fn default_aae_bass_ratio() -> f64 = "bass_ratio";
    fn default_aae_treble_ratio() -> f64 = "treble_ratio";
    fn default_aae_pre_delay_ms() -> f64 = "pre_delay_ms";
    fn default_aae_room_preset() -> String = "room_preset";
    fn default_aae_dry_level() -> f64 = "dry_level";
    fn default_aae_er_level() -> f64 = "er_level";
    fn default_aae_late_level() -> f64 = "late_level";
    fn default_aae_lfe_level() -> f64 = "lfe_level";
    fn default_aae_mod_depth() -> f64 = "mod_depth";
    fn default_aae_er_mod_depth() -> f64 = "er_mod_depth";
    fn default_aae_input_diffusion() -> f64 = "input_diffusion";
    fn default_aae_envelopment() -> f64 = "envelopment";
    fn default_aae_height_amount() -> f64 = "height_amount";
    fn default_aae_content_aware() -> bool = "content_aware";
    fn default_aae_dialogue_attenuation_db() -> f64 = "dialogue_attenuation_db";
    fn default_aae_safety_limit_db() -> f64 = "safety_limit_db";
    fn default_aae_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_aae_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_aae_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
}
sotf_plugins::serde_param_default! {
    gain_specs::PARAMS;
    fn default_gain_smoothing_ms() -> f64 = "smoothing_ms";
}
sotf_plugins::serde_param_default! {
    compressor_specs::PARAMS;
    fn default_compressor_link_channels() -> bool = "link_channels";
    fn default_compressor_sidechain_hpf_hz() -> f64 = "sidechain_hpf_hz";
    fn default_compressor_sidechain_hpf_order() -> String = "sidechain_hpf_order";
    fn default_compressor_detection_mode() -> String = "detection_mode";
}
sotf_plugins::serde_param_default! {
    gate_specs::PARAMS;
    fn default_gate_sidechain_hpf_order() -> String = "sidechain_hpf_order";
    fn default_gate_detection_mode() -> String = "detection_mode";
}
sotf_plugins::serde_param_default! {
    de_esser_specs::PARAMS;
    fn default_de_esser_frequency() -> f64 = "frequency";
    fn default_de_esser_q() -> f64 = "q";
    fn default_de_esser_threshold() -> f64 = "threshold";
    fn default_de_esser_ratio() -> f64 = "ratio";
    fn default_de_esser_attack() -> f64 = "attack";
    fn default_de_esser_release() -> f64 = "release";
    fn default_de_esser_mix() -> f64 = "mix";
}
sotf_plugins::serde_param_default! {
    binaural_specs::PARAMS;
    fn default_binaural_late_reverb_mix() -> f64 = "late_reverb_mix";
    fn default_binaural_late_reverb_rt60() -> f64 = "late_reverb_rt60";
    fn default_binaural_late_reverb_damping() -> f64 = "late_reverb_damping";
}
sotf_plugins::serde_param_default! {
    lc_specs::PARAMS;
    fn default_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
    fn default_lc_mid_enabled() -> bool = "mid_enabled";
    fn default_lc_mid_freq() -> f64 = "mid_freq";
    fn default_lc_mid_gain() -> f64 = "mid_gain";
    fn default_lc_mid_q() -> f64 = "mid_q";
    fn default_lc_mode() -> usize = "mode";
    fn default_lc_playback_level_db() -> f64 = "playback_level_db";
    fn default_lc_reference_level_db() -> f64 = "reference_level_db";
}
sotf_plugins::serde_param_default! {
    limiter_specs::PARAMS;
    fn default_limiter_lookahead_ms() -> f64 = "lookahead";
    fn default_limiter_soft() -> bool = "soft";
    fn default_limiter_mix() -> f64 = "mix";
    fn default_limiter_link_amount() -> f64 = "link_amount";
}
sotf_plugins::serde_param_default! {
    gate_specs::PARAMS;
    fn default_gate_hold_ms() -> f64 = "hold";
    fn default_gate_mix() -> f64 = "mix";
    fn default_gate_link_channels() -> bool = "link_channels";
    fn default_gate_range_db() -> f64 = "range_db";
}
sotf_plugins::serde_param_default! {
    expander_specs::PARAMS;
    fn default_expander_threshold_db() -> f64 = "threshold";
    fn default_expander_ratio() -> f64 = "ratio";
    fn default_expander_attack_ms() -> f64 = "attack";
    fn default_expander_release_ms() -> f64 = "release";
    fn default_expander_range_db() -> f64 = "range";
    fn default_expander_knee_db() -> f64 = "knee";
    fn default_expander_hysteresis_db() -> f64 = "hysteresis";
    fn default_expander_hold_ms() -> f64 = "hold";
    fn default_expander_mix() -> f64 = "mix";
    fn default_expander_link_channels() -> bool = "link_channels";
    fn default_expander_sidechain_hpf_hz() -> f64 = "sidechain_hpf_hz";
    fn default_expander_detection_mode() -> String = "detection_mode";
}
sotf_plugins::serde_param_default! {
    mb_compressor_specs::GLOBAL_PARAMS;
    fn default_mb_compressor_num_bands() -> usize = "num_bands";
    fn default_mb_compressor_crossover_preset() -> i32 = "crossover_preset";
    fn default_mb_compressor_crossover_freq_1() -> f64 = "crossover_freq_1";
    fn default_mb_compressor_crossover_freq_2() -> f64 = "crossover_freq_2";
    fn default_mb_compressor_crossover_freq_3() -> f64 = "crossover_freq_3";
    fn default_mb_compressor_crossover_freq_4() -> f64 = "crossover_freq_4";
    fn default_mb_compressor_threshold_db() -> f64 = "threshold";
    fn default_mb_compressor_ratio() -> f64 = "ratio";
    fn default_mb_compressor_attack_ms() -> f64 = "attack";
    fn default_mb_compressor_release_ms() -> f64 = "release";
    fn default_mb_compressor_knee_db() -> f64 = "knee";
    fn default_mb_compressor_mix() -> f64 = "mix";
    fn default_mb_compressor_link_channels() -> bool = "link_channels";
    fn default_mb_compressor_link_amount() -> f64 = "link_amount";
}
sotf_plugins::serde_param_default! {
    mb_expander_specs::GLOBAL_PARAMS;
    fn default_mb_expander_num_bands() -> usize = "num_bands";
    fn default_mb_expander_crossover_preset() -> i32 = "crossover_preset";
    fn default_mb_expander_crossover_freq_1() -> f64 = "crossover_freq_1";
    fn default_mb_expander_crossover_freq_2() -> f64 = "crossover_freq_2";
    fn default_mb_expander_crossover_freq_3() -> f64 = "crossover_freq_3";
    fn default_mb_expander_crossover_freq_4() -> f64 = "crossover_freq_4";
    fn default_mb_expander_threshold_db() -> f64 = "threshold";
    fn default_mb_expander_ratio() -> f64 = "ratio";
    fn default_mb_expander_attack_ms() -> f64 = "attack";
    fn default_mb_expander_release_ms() -> f64 = "release";
    fn default_mb_expander_range_db() -> f64 = "range";
    fn default_mb_expander_knee_db() -> f64 = "knee";
    fn default_mb_expander_hysteresis_db() -> f64 = "hysteresis";
    fn default_mb_expander_hold_ms() -> f64 = "hold";
    fn default_mb_expander_mix() -> f64 = "mix";
    fn default_mb_expander_link_channels() -> bool = "link_channels";
    fn default_mb_expander_detection_mode() -> String = "detection_mode";
}
sotf_plugins::serde_param_default! {
    xtc_specs::PARAMS;
    fn default_xtc_distance_m() -> f64 = "distance_m";
    fn default_xtc_speaker_angle_deg() -> f64 = "speaker_angle_deg";
    fn default_xtc_head_radius_m() -> f64 = "head_radius_m";
    fn default_xtc_beta_base() -> f64 = "beta_base";
    fn default_xtc_beta_low_freq_boost() -> f64 = "beta_low_freq_boost";
    fn default_xtc_beta_high_freq_boost() -> f64 = "beta_high_freq_boost";
    fn default_xtc_head_shadow_cutoff_hz() -> f64 = "head_shadow_cutoff_hz";
    fn default_xtc_head_shadow_slope() -> f64 = "head_shadow_slope_db_per_octave";
    fn default_xtc_max_gain_db() -> f64 = "max_gain_db";
    fn default_xtc_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_xtc_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_xtc_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
    fn default_xtc_room_width() -> f64 = "room_width_m";
    fn default_xtc_room_depth() -> f64 = "room_depth_m";
    fn default_xtc_wall_absorption() -> f64 = "wall_absorption";
    fn default_xtc_reflection_beta_boost() -> f64 = "reflection_beta_boost";
    fn default_xtc_spectral_normalization() -> bool = "spectral_normalization";
    fn default_xtc_room_reflections_enabled() -> bool = "room_reflections_enabled";
    fn default_xtc_pinna_model_enabled() -> bool = "pinna_model_enabled";
    fn default_xtc_head_tracking_smooth_s() -> f64 = "head_tracking_smooth_s";
}
sotf_plugins::serde_param_default! {
    denoiser_specs::PARAMS;
    fn default_denoiser_reduction_db() -> f64 = "reduction_db";
    fn default_denoiser_floor_db() -> f64 = "floor_db";
    fn default_denoiser_smoothing() -> f64 = "smoothing";
    fn default_denoiser_attack_ms() -> f64 = "attack_ms";
    fn default_denoiser_release_ms() -> f64 = "release_ms";
    fn default_denoiser_low_latency() -> bool = "low_latency";
    fn default_denoiser_polyphonic_detection() -> bool = "polyphonic_detection";
    fn default_denoiser_psychoacoustic_masking() -> bool = "psychoacoustic_masking";
    fn default_denoiser_use_captured_profile() -> bool = "use_captured_profile";
    fn default_denoiser_transparency() -> f64 = "transparency";
    fn default_denoiser_dd_enabled() -> bool = "dd_enabled";
    fn default_denoiser_dd_alpha() -> f64 = "dd_alpha";
    fn default_denoiser_mcra_alpha_s() -> f64 = "mcra_alpha_s";
    fn default_denoiser_mcra_alpha_p() -> f64 = "mcra_alpha_p";
    fn default_denoiser_mcra_l() -> usize = "mcra_l";
    fn default_denoiser_mcra_delta() -> f64 = "mcra_delta";
    fn default_denoiser_spectral_smoothing_enabled() -> bool = "spectral_smoothing_enabled";
    fn default_denoiser_temporal_smoothing_enabled() -> bool = "temporal_smoothing_enabled";
    fn default_denoiser_spectral_sub_enabled() -> bool = "spectral_sub_enabled";
    fn default_denoiser_spectral_sub_alpha() -> f64 = "spectral_sub_alpha";
    fn default_denoiser_spectral_sub_beta() -> f64 = "spectral_sub_beta";
    fn default_denoiser_formant_strength() -> f64 = "formant_strength";
    fn default_spatial_strength() -> f64 = "spatial_strength";
}
sotf_plugins::serde_param_default! {
    declick_specs::PARAMS;
    fn default_declick_enabled() -> bool = "enabled";
    fn default_declick_sensitivity() -> f64 = "sensitivity";
}
sotf_plugins::serde_param_default! {
    hiss_reducer_specs::PARAMS;
    fn default_hiss_reducer_enabled() -> bool = "enabled";
    fn default_hiss_reducer_threshold_db() -> f64 = "threshold_db";
    fn default_hiss_reducer_frequency_hz() -> f64 = "frequency_hz";
    fn default_hiss_reducer_strength() -> f64 = "strength";
}
sotf_plugins::serde_param_default! {
    speech_denoiser_specs::PARAMS;
    fn default_speech_denoiser_enabled() -> bool = "enabled";
}
sotf_plugins::serde_param_default! {
    convolution_specs::PARAMS;
    fn default_use_nupc() -> bool = "use_nupc";
    fn default_head_taps() -> usize = "head_taps";
}
sotf_plugins::serde_param_default! {
    pnd_specs::PARAMS;
    fn default_pnd_correction_strength() -> f64 = "correction_strength";
    fn default_pnd_analysis_window_ms() -> f64 = "analysis_window_ms";
    fn default_pnd_drift_smoothing() -> f64 = "drift_smoothing";
    fn default_pnd_multi_channel_analysis() -> bool = "multi_channel_analysis";
    fn default_pnd_confidence_threshold() -> f64 = "confidence_threshold";
}
sotf_plugins::serde_param_default! {
    ab_compare_specs::PARAMS;
    fn default_ab_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_ab_max_auto_gain_db() -> f64 = "max_auto_gain_db";
    fn default_ab_gain_smoothing_ms() -> f64 = "gain_smoothing_ms";
    fn default_ab_mix_transition_ms() -> f64 = "mix_transition_ms";
}
sotf_plugins::serde_param_default! {
    band_split_specs::PARAMS;
    fn default_band_split_frequency() -> f64 = "frequency";
}
sotf_plugins::serde_param_default! {
    band_split_specs::PARAMS;
    fn default_band_split_crossover_type() -> String = "crossover_type";
}
sotf_plugins::serde_param_default! {
    band_merge_specs::PARAMS;
    fn default_band_merge_bands() -> usize = "bands";
}
sotf_plugins::serde_param_default! {
    downmix_specs::PARAMS;
    fn default_downmix_center_gain_db() -> f64 = "center_gain_db";
    fn default_downmix_surround_gain_db() -> f64 = "surround_gain_db";
    fn default_downmix_height_gain_db() -> f64 = "height_gain_db";
    fn default_downmix_lfe_gain_db() -> f64 = "lfe_gain_db";
    fn default_downmix_phase_coherence() -> bool = "phase_coherence";
    fn default_downmix_phase_blend_low_hz() -> f64 = "phase_blend_low_hz";
    fn default_downmix_phase_blend_high_hz() -> f64 = "phase_blend_high_hz";
}
sotf_plugins::serde_param_default! {
    mono_to_stereo_specs::PARAMS;
    fn default_mono_to_stereo_width() -> f64 = "stereo_width";
    fn default_mono_to_stereo_haas_delay_ms() -> f64 = "haas_delay_ms";
    fn default_mono_to_stereo_decor_low_hz() -> f64 = "decor_low_hz";
    fn default_mono_to_stereo_decor_high_hz() -> f64 = "decor_high_hz";
    fn default_mono_to_stereo_freq_dependent() -> bool = "freq_dependent";
}
sotf_plugins::serde_param_default! {
    crossfeed_specs::PARAMS;
    fn default_crossfeed_enabled() -> bool = "enabled";
    fn default_crossfeed_bauer_fcut_hz() -> f64 = "bauer_fcut_hz";
    fn default_crossfeed_bauer_feed_db() -> f64 = "bauer_feed_db";
    fn default_crossfeed_meier_level() -> f64 = "meier_level";
    fn default_crossfeed_mb_low_freq_hz() -> f64 = "mb_low_freq_hz";
    fn default_crossfeed_mb_mid_high_freq_hz() -> f64 = "mb_mid_high_freq_hz";
    fn default_crossfeed_mb_low_feed_db() -> f64 = "mb_low_feed_db";
    fn default_crossfeed_mb_mid_feed_db() -> f64 = "mb_mid_feed_db";
    fn default_crossfeed_mb_high_feed_db() -> f64 = "mb_high_feed_db";
    fn default_crossfeed_autogain_target_lufs() -> f64 = "autogain_target_lufs";
    fn default_crossfeed_autogain_max_gain_db() -> f64 = "autogain_max_gain_db";
    fn default_crossfeed_autogain_smoothing_ms() -> f64 = "autogain_smoothing_ms";
    fn default_crossfeed_mix() -> f64 = "mix";
    fn default_delay_ms() -> f64 = "delay_ms";
    fn default_delay_feedback() -> f64 = "feedback";
    fn default_delay_mix() -> f64 = "mix";
    fn default_delay_allpass_coeff() -> f64 = "allpass_coeff";
}
sotf_plugins::serde_param_default! {
    aec_specs::PARAMS;
    fn default_aec_echo_tail_ms() -> f64 = "echo_tail_ms";
    fn default_aec_step_size() -> f64 = "step_size";
    fn default_aec_post_filter_enabled() -> bool = "post_filter_enabled";
}
sotf_plugins::serde_param_default! {
    beamformer_specs::PARAMS;
    fn default_beamformer_num_mics() -> usize = "num_mics";
    fn default_beamformer_mic_spacing_cm() -> f64 = "mic_spacing_cm";
    fn default_beamformer_steer_angle_deg() -> f64 = "steer_angle_deg";
    fn default_beamformer_type() -> usize = "beamformer_type";
}
sotf_plugins::serde_param_default! {
    ambisonics_specs::PARAMS;
    fn default_ambisonics_order() -> usize = "order";
    fn default_ambisonics_target_layout() -> String = "target_layout";
    fn default_ambisonics_max_re() -> bool = "max_re_weighting";
}
sotf_plugins::serde_param_default! {
    cms_specs::PARAMS;
    fn default_cms_dim_gain_db() -> f64 = "dim_gain_db";
    fn default_cms_fade_ms() -> f64 = "fade_ms";
}
sotf_plugins::serde_param_default! {
    spectrum_specs::PARAMS;
    fn default_spectrum_num_bins() -> usize = "num_bins";
    fn default_spectrum_min_freq() -> f32 = "min_freq";
    fn default_spectrum_max_freq() -> f32 = "max_freq";
    fn default_spectrum_smoothing() -> f32 = "smoothing";
}
sotf_plugins::serde_param_default! {
    stereo_imager_specs::PARAMS;
    fn default_si_width() -> f64 = "width";
    fn default_si_low_mid_freq() -> f64 = "low_mid_freq";
    fn default_si_mid_high_freq() -> f64 = "mid_high_freq";
    fn default_si_low_width() -> f64 = "low_width";
    fn default_si_mid_width() -> f64 = "mid_width";
    fn default_si_high_width() -> f64 = "high_width";
    fn default_si_mono_bass() -> bool = "mono_bass";
    fn default_si_mix() -> f64 = "mix";
}
sotf_plugins::serde_param_default! {
    spectral_compressor_specs::PARAMS;
    fn default_sc_threshold() -> f64 = "threshold";
    fn default_sc_ratio() -> f64 = "ratio";
    fn default_sc_attack() -> f64 = "attack";
    fn default_sc_release() -> f64 = "release";
    fn default_sc_knee() -> f64 = "knee";
    fn default_sc_spectral_smoothing() -> f64 = "spectral_smoothing";
    fn default_sc_mix() -> f64 = "mix";
    fn default_sc_fft_size() -> usize = "fft_size";
}
sotf_plugins::serde_param_default! {
    transient_shaper_specs::PARAMS;
    fn default_ts_mix() -> f64 = "mix";
}
sotf_plugins::serde_param_default! {
    saturation_specs::PARAMS;
    fn default_sat_mode() -> f64 = "mode";
    fn default_sat_drive() -> f64 = "drive";
    fn default_sat_tone() -> f64 = "tone";
    fn default_sat_exciter_freq() -> f64 = "exciter_freq";
    fn default_sat_oversampling() -> f64 = "oversampling";
    fn default_sat_output_gain() -> f64 = "output_gain";
    fn default_sat_mix() -> f64 = "mix";
    fn default_sat_dynamic_attack_ms() -> f64 = "dynamic_attack_ms";
    fn default_sat_dynamic_release_ms() -> f64 = "dynamic_release_ms";
    fn default_sat_dc_blocker() -> bool = "dc_blocker";
    fn default_sat_use_adaa() -> bool = "use_adaa";
}
sotf_plugins::serde_param_default! {
    dynamic_eq_specs::PARAMS;
    fn default_dyneq_num_bands() -> f64 = "num_bands";
    fn default_dyneq_threshold() -> f64 = "threshold";
    fn default_dyneq_ratio() -> f64 = "ratio";
    fn default_dyneq_attack() -> f64 = "attack";
    fn default_dyneq_release() -> f64 = "release";
    fn default_dyneq_knee() -> f64 = "knee";
    fn default_dyneq_link_channels() -> bool = "link_channels";
    fn default_dyneq_mix() -> f64 = "mix";
}
sotf_plugins::serde_param_default! {
    linear_phase_eq_specs::PARAMS;
    fn default_lpeq_num_filters() -> f64 = "num_filters";
    fn default_lpeq_fir_length() -> f64 = "fir_length";
    fn default_lpeq_mix() -> f64 = "mix";
}
sotf_plugins::serde_param_default! {
    fir_designer_specs::PARAMS;
    fn default_fird_num_filters() -> f64 = "num_filters";
    fn default_fird_fir_length() -> f64 = "fir_length";
    fn default_fird_phase_mode() -> f64 = "phase_mode";
    fn default_fird_mix() -> f64 = "mix";
}
pub mod chain;
pub mod eq;
pub mod matrix;
pub mod utility;

mod default;
mod misc;
mod plugin;
mod plugin_settings;
mod plugin_type;
mod release_channel;
#[cfg(test)]
mod tests;
mod types;

pub use plugin::*;
pub use plugin_settings::*;
pub use plugin_type::*;
pub use release_channel::*;
pub use types::*;
