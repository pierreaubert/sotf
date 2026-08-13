use super::default::default_ab_path_config;
use super::default::default_channels;
use super::default::default_de_esser_mode;
use super::default::default_dyneq_bands;
use super::default::default_eq_oversampling;
use super::default::default_fm_auto_gain_max_db;
use super::default::default_fm_auto_gain_smoothing_ms;
use super::default::default_fm_band1_freq;
use super::default::default_fm_band1_max_gain;
use super::default::default_fm_band1_q;
use super::default::default_fm_band1_slope;
use super::default::default_fm_band2_freq;
use super::default::default_fm_band2_max_gain;
use super::default::default_fm_band2_q;
use super::default::default_fm_band2_slope;
use super::default::default_fm_band3_freq;
use super::default::default_fm_band3_max_gain;
use super::default::default_fm_band3_q;
use super::default::default_fm_band3_slope;
use super::default::default_fm_band4_freq;
use super::default::default_fm_band4_max_gain;
use super::default::default_fm_band4_q;
use super::default::default_fm_band4_slope;
use super::default::default_fm_enabled;
use super::default::default_fm_reference_level_db;
use super::default::default_fm_smoothing_ms;
use super::default::default_max_filters;
use super::default::default_spectrum_tilt_correction;
use super::default::default_spectrum_tilt_reference;
use super::default_aae_auto_gain_enabled;
use super::default_aae_auto_gain_max_db;
use super::default_aae_auto_gain_smoothing_ms;
use super::default_aae_bass_ratio;
use super::default_aae_content_aware;
use super::default_aae_dialogue_attenuation_db;
use super::default_aae_dry_level;
use super::default_aae_envelopment;
use super::default_aae_er_level;
use super::default_aae_er_mod_depth;
use super::default_aae_height_amount;
use super::default_aae_input_diffusion;
use super::default_aae_late_level;
use super::default_aae_lfe_level;
use super::default_aae_mod_depth;
use super::default_aae_pre_delay_ms;
use super::default_aae_room_preset;
use super::default_aae_room_size;
use super::default_aae_rt60;
use super::default_aae_safety_limit_db;
use super::default_aae_speaker_config;
use super::default_aae_treble_ratio;
use super::default_ab_auto_gain_enabled;
use super::default_ab_band_mask_high_hz;
use super::default_ab_band_mask_low_hz;
use super::default_ab_gain_smoothing_ms;
use super::default_ab_max_auto_gain_db;
use super::default_ab_mix_transition_ms;
use super::default_aec_echo_tail_ms;
use super::default_aec_post_filter_enabled;
use super::default_aec_step_size;
use super::default_ambisonics_max_re;
use super::default_ambisonics_order;
use super::default_ambisonics_target_layout;
use super::default_auto_gain_max_db;
use super::default_auto_gain_smoothing_ms;
use super::default_band_merge_bands;
use super::default_band_split_crossover_type;
use super::default_band_split_frequency;
use super::default_beamformer_mic_spacing_cm;
use super::default_beamformer_num_mics;
use super::default_beamformer_steer_angle_deg;
use super::default_beamformer_type;
use super::default_binaural_crossfade_ms;
use super::default_binaural_ear_height_cm;
use super::default_binaural_head_width_cm;
use super::default_binaural_late_reverb_damping;
use super::default_binaural_late_reverb_mix;
use super::default_binaural_late_reverb_rt60;
use super::default_cms_dim_gain_db;
use super::default_cms_fade_ms;
use super::default_compressor_detection_mode;
use super::default_compressor_link_channels;
use super::default_compressor_sidechain_hpf_hz;
use super::default_compressor_sidechain_hpf_order;
use super::default_crossfeed_autogain_max_gain_db;
use super::default_crossfeed_autogain_smoothing_ms;
use super::default_crossfeed_autogain_target_lufs;
use super::default_crossfeed_bauer_fcut_hz;
use super::default_crossfeed_bauer_feed_db;
use super::default_crossfeed_enabled;
use super::default_crossfeed_mb_high_feed_db;
use super::default_crossfeed_mb_low_feed_db;
use super::default_crossfeed_mb_low_freq_hz;
use super::default_crossfeed_mb_mid_feed_db;
use super::default_crossfeed_mb_mid_high_freq_hz;
use super::default_crossfeed_meier_level;
use super::default_crossfeed_mix;
use super::default_crossover_fir_taps;
use super::default_crossover_frequency;
use super::default_crossover_output;
use super::default_crossover_type;
use super::default_de_esser_attack;
use super::default_de_esser_frequency;
use super::default_de_esser_mix;
use super::default_de_esser_q;
use super::default_de_esser_ratio;
use super::default_de_esser_release;
use super::default_de_esser_threshold;
use super::default_declick_enabled;
use super::default_declick_link_channels;
use super::default_declick_sensitivity;
use super::default_delay_allpass_coeff;
use super::default_delay_feedback;
use super::default_delay_mix;
use super::default_delay_ms;
use super::default_denoiser_attack_ms;
use super::default_denoiser_dd_alpha;
use super::default_denoiser_dd_enabled;
use super::default_denoiser_floor_db;
use super::default_denoiser_formant_strength;
use super::default_denoiser_low_latency;
use super::default_denoiser_mcra_alpha_p;
use super::default_denoiser_mcra_alpha_s;
use super::default_denoiser_mcra_delta;
use super::default_denoiser_mcra_l;
use super::default_denoiser_polyphonic_detection;
use super::default_denoiser_psychoacoustic_masking;
use super::default_denoiser_reduction_db;
use super::default_denoiser_release_ms;
use super::default_denoiser_smoothing;
use super::default_denoiser_spectral_smoothing_enabled;
use super::default_denoiser_spectral_sub_alpha;
use super::default_denoiser_spectral_sub_beta;
use super::default_denoiser_spectral_sub_enabled;
use super::default_denoiser_temporal_smoothing_enabled;
use super::default_denoiser_transparency;
use super::default_denoiser_use_captured_profile;
use super::default_dither_bit_depth;
use super::default_dither_noise_shaping;
use super::default_dither_type;
use super::default_downmix_center_gain_db;
use super::default_downmix_height_gain_db;
use super::default_downmix_lfe_gain_db;
use super::default_downmix_matrix_ltrt;
use super::default_downmix_phase_blend_high_hz;
use super::default_downmix_phase_blend_low_hz;
use super::default_downmix_phase_coherence;
use super::default_downmix_surround_gain_db;
use super::default_dyneq_attack;
use super::default_dyneq_knee;
use super::default_dyneq_link_channels;
use super::default_dyneq_mix;
use super::default_dyneq_num_bands;
use super::default_dyneq_ratio;
use super::default_dyneq_release;
use super::default_dyneq_threshold;
use super::default_expander_attack_ms;
use super::default_expander_detection_mode;
use super::default_expander_hold_ms;
use super::default_expander_hysteresis_db;
use super::default_expander_knee_db;
use super::default_expander_link_channels;
use super::default_expander_mix;
use super::default_expander_range_db;
use super::default_expander_ratio;
use super::default_expander_release_ms;
use super::default_expander_sidechain_hpf_hz;
use super::default_expander_threshold_db;
use super::default_gain_smoothing_ms;
use super::default_gate_detection_mode;
use super::default_gate_hold_ms;
use super::default_gate_link_channels;
use super::default_gate_mix;
use super::default_gate_range_db;
use super::default_gate_sidechain_hpf_order;
use super::default_head_taps;
use super::default_hiss_reducer_enabled;
use super::default_hiss_reducer_frequency_hz;
use super::default_hiss_reducer_strength;
use super::default_hiss_reducer_threshold_db;
use super::default_lc_mid_enabled;
use super::default_lc_mid_freq;
use super::default_lc_mid_gain;
use super::default_lc_mid_q;
use super::default_lc_mode;
use super::default_lc_playback_level_db;
use super::default_lc_reference_level_db;
use super::default_limiter_link_amount;
use super::default_limiter_lookahead_ms;
use super::default_limiter_mix;
use super::default_limiter_soft;
use super::default_lpeq_fir_length;
use super::default_lpeq_mix;
use super::default_lpeq_num_filters;
use super::default_lpeq_phase_mode;
use super::default_mb_compressor_attack_ms;
use super::default_mb_compressor_crossover_freq_1;
use super::default_mb_compressor_crossover_freq_2;
use super::default_mb_compressor_crossover_freq_3;
use super::default_mb_compressor_crossover_freq_4;
use super::default_mb_compressor_crossover_preset;
use super::default_mb_compressor_knee_db;
use super::default_mb_compressor_link_amount;
use super::default_mb_compressor_link_channels;
use super::default_mb_compressor_mix;
use super::default_mb_compressor_num_bands;
use super::default_mb_compressor_ratio;
use super::default_mb_compressor_release_ms;
use super::default_mb_compressor_threshold_db;
use super::default_mb_expander_attack_ms;
use super::default_mb_expander_crossover_freq_1;
use super::default_mb_expander_crossover_freq_2;
use super::default_mb_expander_crossover_freq_3;
use super::default_mb_expander_crossover_freq_4;
use super::default_mb_expander_crossover_preset;
use super::default_mb_expander_detection_mode;
use super::default_mb_expander_hold_ms;
use super::default_mb_expander_hysteresis_db;
use super::default_mb_expander_knee_db;
use super::default_mb_expander_link_channels;
use super::default_mb_expander_mix;
use super::default_mb_expander_num_bands;
use super::default_mb_expander_range_db;
use super::default_mb_expander_ratio;
use super::default_mb_expander_release_ms;
use super::default_mb_expander_threshold_db;
use super::default_mono_to_stereo_decor_high_hz;
use super::default_mono_to_stereo_decor_low_hz;
use super::default_mono_to_stereo_freq_dependent;
use super::default_mono_to_stereo_haas_delay_ms;
use super::default_mono_to_stereo_width;
use super::default_pnd_analysis_window_ms;
use super::default_pnd_confidence_threshold;
use super::default_pnd_correction_strength;
use super::default_pnd_drift_smoothing;
use super::default_pnd_multi_channel_analysis;
use super::default_sat_dc_blocker;
use super::default_sat_drive;
use super::default_sat_dynamic_attack_ms;
use super::default_sat_dynamic_release_ms;
use super::default_sat_exciter_freq;
use super::default_sat_mix;
use super::default_sat_mode;
use super::default_sat_output_gain;
use super::default_sat_oversampling;
use super::default_sat_tone;
use super::default_sat_use_adaa;
use super::default_sc_attack;
use super::default_sc_fft_size;
use super::default_sc_knee;
use super::default_sc_mix;
use super::default_sc_ratio;
use super::default_sc_release;
use super::default_sc_spectral_smoothing;
use super::default_sc_threshold;
use super::default_si_high_width;
use super::default_si_low_mid_freq;
use super::default_si_low_width;
use super::default_si_mid_high_freq;
use super::default_si_mid_width;
use super::default_si_mix;
use super::default_si_mono_bass;
use super::default_si_width;
use super::default_spatial_strength;
use super::default_spectrum_max_freq;
use super::default_spectrum_min_freq;
use super::default_spectrum_num_bins;
use super::default_spectrum_smoothing;
use super::default_speech_denoiser_enabled;
use super::default_ts_mix;
use super::default_upmixer_ambient_boost;
use super::default_upmixer_auto_gain_enabled;
use super::default_upmixer_auto_gain_max_db;
use super::default_upmixer_auto_gain_smoothing_ms;
use super::default_upmixer_center_spread;
use super::default_upmixer_decorrelation_lfo_rate_hz;
use super::default_upmixer_dialogue_centroid_weight;
use super::default_upmixer_dialogue_coherence_weight;
use super::default_upmixer_dialogue_variance_weight;
use super::default_upmixer_dialogue_weight;
use super::default_upmixer_enable_hr_direct;
use super::default_upmixer_frequency_resolution;
use super::default_upmixer_height_direct_leak;
use super::default_upmixer_height_hf_cap_hz;
use super::default_upmixer_height_transient_reduction;
use super::default_upmixer_hr_sharpen;
use super::default_upmixer_multi_source_threshold;
use super::default_upmixer_rear_ambient_boost;
use super::default_upmixer_rear_late_reflection;
use super::default_upmixer_safety_cap_db;
use super::default_upmixer_subharmonic_attack_ms;
use super::default_upmixer_subharmonic_freq_hz;
use super::default_upmixer_subharmonic_gain;
use super::default_upmixer_subharmonic_release_ms;
use super::default_upmixer_surround_direct_bleed;
use super::default_upmixer_velvet_noise_density;
use super::default_upmixer_velvet_noise_duration_ms;
use super::default_upmixer_voice_freq_max_hz;
use super::default_upmixer_voice_freq_min_hz;
use super::default_use_nupc;
use super::default_xtc_auto_gain_enabled;
use super::default_xtc_auto_gain_max_db;
use super::default_xtc_auto_gain_smoothing_ms;
use super::default_xtc_beta_base;
use super::default_xtc_beta_high_freq_boost;
use super::default_xtc_beta_low_freq_boost;
use super::default_xtc_distance_m;
use super::default_xtc_head_radius_m;
use super::default_xtc_head_shadow_cutoff_hz;
use super::default_xtc_head_shadow_slope;
use super::default_xtc_head_tracking_smooth_s;
use super::default_xtc_max_gain_db;
use super::default_xtc_pinna_model_enabled;
use super::default_xtc_reflection_beta_boost;
use super::default_xtc_room_depth;
use super::default_xtc_room_reflections_enabled;
use super::default_xtc_room_width;
use super::default_xtc_speaker_angle_deg;
use super::default_xtc_spectral_normalization;
use super::default_xtc_wall_absorption;
pub use super::eq::EQFilter;
use super::misc::deserialize_speaker_config;
use super::plugin_type::PluginType;
use crate::engine::PluginConfig;
use math_audio_iir_fir::BiquadFilterType;
use serde::{Deserialize, Serialize};
use sotf_plugins::ExternalPluginState;

use sotf_plugins::param_specs::aae as aae_specs;
use sotf_plugins::param_specs::ab_compare as ab_compare_specs;
use sotf_plugins::param_specs::aec as aec_specs;
use sotf_plugins::param_specs::ambisonics as ambisonics_specs;
use sotf_plugins::param_specs::beamformer as beamformer_specs;
use sotf_plugins::param_specs::binaural as binaural_specs;
use sotf_plugins::param_specs::channel_mute_solo as cms_specs;
use sotf_plugins::param_specs::compressor as compressor_specs;
use sotf_plugins::param_specs::convolution as convolution_specs;
use sotf_plugins::param_specs::crossfeed as crossfeed_specs;
use sotf_plugins::param_specs::crossover as crossover_specs;
use sotf_plugins::param_specs::de_esser as de_esser_specs;
use sotf_plugins::param_specs::declick as declick_specs;
use sotf_plugins::param_specs::delay as delay_specs;
use sotf_plugins::param_specs::denoiser as denoiser_specs;
use sotf_plugins::param_specs::dither as dither_specs;
use sotf_plugins::param_specs::downmix as downmix_specs;
use sotf_plugins::param_specs::dynamic_eq as dynamic_eq_specs;
use sotf_plugins::param_specs::expander as expander_specs;
use sotf_plugins::param_specs::find_by_key as pk;
use sotf_plugins::param_specs::gain as gain_specs;
use sotf_plugins::param_specs::gate as gate_specs;
use sotf_plugins::param_specs::hiss_reducer as hiss_reducer_specs;
use sotf_plugins::param_specs::limiter as limiter_specs;
use sotf_plugins::param_specs::linear_phase_eq as linear_phase_eq_specs;
use sotf_plugins::param_specs::loudness_compensation as lc_specs;
use sotf_plugins::param_specs::matrix as matrix_specs;
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
use sotf_plugins::{
    BandCompressorParams, BandExpanderParams, CrossfeedMode, CrossfeedPreset, DynEqBandParams,
    SpectralTiltCorrection, TiltReferenceFreq,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerGainSettings {
    pub gain_front_direct: f64,
    pub gain_front_ambient: f64,
    pub gain_rear_ambient: f64,
    pub height_gain: f64,
    pub stereo_width: f64,
    #[serde(default = "default_upmixer_center_spread")]
    pub center_spread: f64,
    #[serde(default = "default_upmixer_surround_direct_bleed")]
    pub surround_direct_bleed: f64,
    #[serde(default = "default_upmixer_rear_late_reflection")]
    pub rear_late_reflection: f64,
    #[serde(default = "default_upmixer_ambient_boost")]
    pub ambient_boost: f64,
    #[serde(default = "default_upmixer_rear_ambient_boost")]
    pub rear_ambient_boost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerLfeSettings {
    pub lfe_cutoff_hz: f64,
    pub lfe_gain: f64,
    pub bandpass_hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerSubharmonicSettings {
    #[serde(default)] // false
    pub enable_subharmonic_synth: bool,
    #[serde(default = "default_upmixer_subharmonic_gain")]
    pub subharmonic_gain: f64,
    #[serde(default = "default_upmixer_subharmonic_freq_hz")]
    pub subharmonic_freq_hz: f64,
    #[serde(default = "default_upmixer_subharmonic_attack_ms")]
    pub subharmonic_attack_ms: f64,
    #[serde(default = "default_upmixer_subharmonic_release_ms")]
    pub subharmonic_release_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerDecorrelationSettings {
    #[serde(default)] // 0
    pub decorrelation_mode: usize,
    #[serde(default = "default_upmixer_decorrelation_lfo_rate_hz")]
    pub decorrelation_lfo_rate_hz: f64,
    #[serde(default = "default_upmixer_velvet_noise_duration_ms")]
    pub velvet_noise_duration_ms: f64,
    #[serde(default = "default_upmixer_velvet_noise_density")]
    pub velvet_noise_density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerHeightSettings {
    #[serde(default = "default_upmixer_enable_hr_direct")]
    pub enable_hr_direct: bool,
    #[serde(default = "default_upmixer_hr_sharpen")]
    pub hr_sharpen: f64,
    #[serde(default = "default_upmixer_height_hf_cap_hz")]
    pub height_hf_cap_hz: f64,
    #[serde(default = "default_upmixer_height_transient_reduction")]
    pub height_transient_reduction: f64,
    #[serde(default = "default_upmixer_height_direct_leak")]
    pub height_direct_leak: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerDialogueSettings {
    #[serde(default = "default_upmixer_dialogue_weight")]
    pub dialogue_weight: f64,
    #[serde(default = "default_upmixer_voice_freq_min_hz")]
    pub voice_freq_min_hz: f64,
    #[serde(default = "default_upmixer_voice_freq_max_hz")]
    pub voice_freq_max_hz: f64,
    #[serde(default = "default_upmixer_dialogue_centroid_weight")]
    pub dialogue_centroid_weight: f64,
    #[serde(default = "default_upmixer_dialogue_variance_weight")]
    pub dialogue_variance_weight: f64,
    #[serde(default = "default_upmixer_dialogue_coherence_weight")]
    pub dialogue_coherence_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerBypassSettings {
    #[serde(default)] // false
    pub bypass_decorrelation: bool,
    #[serde(default)] // false
    pub bypass_transient_detection: bool,
    #[serde(default)] // false
    pub bypass_all_processing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerAmbientAnalysisSettings {
    #[serde(default)] // false
    pub low_latency: bool,
    #[serde(default = "default_upmixer_frequency_resolution")]
    pub frequency_resolution: usize,
    #[serde(default = "default_upmixer_safety_cap_db")]
    pub safety_cap_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerOutputSettings {
    #[serde(default)] // false
    pub enable_ml_detection: bool,
    #[serde(default)]
    pub multi_source_extraction: bool,
    #[serde(default = "default_upmixer_multi_source_threshold")]
    pub multi_source_threshold: f64,
    #[serde(default)]
    pub binaural_preview: bool,
    #[serde(default = "default_upmixer_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "default_upmixer_auto_gain_max_db")]
    pub auto_gain_max_db: f64,
    #[serde(default = "default_upmixer_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSettings {
    EQ {
        #[serde(default = "default_channels")]
        channels: usize,
        /// Global filters applied to all channels (used when per_channel_mode is false)
        filters: Vec<EQFilter>,
        /// Per-channel filters (used when per_channel_mode is true)
        /// Index corresponds to channel index
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel_filters: Option<Vec<Vec<EQFilter>>>,
        /// Whether to use per-channel mode (default: false = all channels share same EQ)
        #[serde(default)]
        per_channel_mode: bool,
        /// Maximum number of filters to display/use in the UI
        #[serde(default = "default_max_filters")]
        max_filters: usize,
        /// Use Transposed Direct Form II for biquad filters
        #[serde(default)]
        tdf2: bool,
        /// Filter topology: 0 = Biquad (default), 1 = SVF (zero-delay feedback)
        #[serde(default)]
        topology: f64,
        /// Enable the EQ output auto-gain compensation.
        #[serde(default)]
        auto_gain_enabled: bool,
        /// Internal EQ oversampling factor: 1 (off), 2, or 4.
        #[serde(default = "default_eq_oversampling")]
        oversampling: f64,
    },
    Gain {
        #[serde(default = "default_channels")]
        channels: usize,
        gain_db: f64,
        #[serde(default = "default_gain_smoothing_ms")]
        smoothing_ms: f64,
    },
    Upmixer {
        #[serde(deserialize_with = "deserialize_speaker_config")]
        speaker_config: String,
        #[serde(flatten)]
        gains: UpmixerGainSettings,
        #[serde(flatten)]
        lfe: UpmixerLfeSettings,
        #[serde(flatten)]
        subharmonic: UpmixerSubharmonicSettings,
        #[serde(flatten)]
        decorrelation: UpmixerDecorrelationSettings,
        #[serde(flatten)]
        height: UpmixerHeightSettings,
        #[serde(flatten)]
        ambient_analysis: UpmixerAmbientAnalysisSettings,
        #[serde(flatten)]
        dialogue: UpmixerDialogueSettings,
        #[serde(flatten)]
        bypass: UpmixerBypassSettings,
        #[serde(flatten)]
        output: UpmixerOutputSettings,
    },
    AAE {
        #[serde(default = "default_aae_speaker_config")]
        speaker_config: String,
        #[serde(default = "default_aae_room_size")]
        room_size: f64,
        #[serde(default = "default_aae_rt60")]
        rt60: f64,
        #[serde(default = "default_aae_bass_ratio")]
        bass_ratio: f64,
        #[serde(default = "default_aae_treble_ratio")]
        treble_ratio: f64,
        #[serde(default = "default_aae_pre_delay_ms")]
        pre_delay_ms: f64,
        #[serde(default = "default_aae_room_preset")]
        room_preset: String,
        #[serde(default = "default_aae_dry_level")]
        dry_level: f64,
        #[serde(default = "default_aae_er_level")]
        er_level: f64,
        #[serde(default = "default_aae_late_level")]
        late_level: f64,
        #[serde(default = "default_aae_lfe_level")]
        lfe_level: f64,
        #[serde(default = "default_aae_mod_depth")]
        mod_depth: f64,
        #[serde(default = "default_aae_er_mod_depth")]
        er_mod_depth: f64,
        #[serde(default = "default_aae_input_diffusion")]
        input_diffusion: f64,
        #[serde(default = "default_aae_envelopment")]
        envelopment: f64,
        #[serde(default = "default_aae_height_amount")]
        height_amount: f64,
        #[serde(default = "default_aae_content_aware")]
        content_aware: bool,
        #[serde(default = "default_aae_dialogue_attenuation_db")]
        dialogue_attenuation_db: f64,
        #[serde(default = "default_aae_safety_limit_db")]
        safety_limit_db: f64,
        #[serde(default = "default_aae_auto_gain_enabled")]
        auto_gain_enabled: bool,
        #[serde(default = "default_aae_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_aae_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        #[serde(default)]
        bypass: bool,
        #[serde(default)]
        solo_early: bool,
        #[serde(default)]
        solo_late: bool,
    },
    Compressor {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        knee_db: f64,
        makeup_gain_db: f64,
        mix: f64,
        #[serde(default)] // false (matches plugin default)
        auto_makeup: bool,
        #[serde(default = "default_compressor_link_channels")]
        link_channels: bool,
        #[serde(default = "default_compressor_sidechain_hpf_hz")]
        sidechain_hpf_hz: f64,
        #[serde(default = "default_compressor_sidechain_hpf_order")]
        sidechain_hpf_order: String,
        #[serde(default = "default_compressor_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        lookahead_ms: f64,
        #[serde(default)]
        program_dependent_release: bool,
        #[serde(default)]
        measured_auto_makeup: bool,
        #[serde(default)]
        sidechain_external: bool,
    },
    Limiter {
        threshold_db: f64,
        release_ms: f64,
        #[serde(default = "default_limiter_lookahead_ms")]
        lookahead_ms: f64,
        #[serde(default = "default_limiter_soft")]
        soft: bool,
        #[serde(default)]
        true_peak: bool,
        #[serde(default)]
        isp_mode: bool,
        #[serde(default)]
        dual_release: bool,
        #[serde(default = "default_limiter_mix")]
        mix: f64,
        #[serde(default = "default_limiter_link_amount")]
        link_amount: f64,
        #[serde(default)]
        feed_forward: bool,
    },
    Gate {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        #[serde(default = "default_gate_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_gate_mix")]
        mix: f64,
        #[serde(default = "default_gate_link_channels")]
        link_channels: bool,
        #[serde(default)] // 0.0
        sidechain_hpf_hz: f64,
        #[serde(default = "default_gate_sidechain_hpf_order")]
        sidechain_hpf_order: String,
        #[serde(default = "default_gate_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        sidechain_external: bool,
        #[serde(default = "default_gate_range_db")]
        range_db: f64,
        #[serde(default)]
        hysteresis_db: f64,
        #[serde(default)]
        knee_db: f64,
        #[serde(default)]
        lookahead_ms: f64,
    },
    Expander {
        #[serde(default = "default_expander_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_expander_ratio")]
        ratio: f64,
        #[serde(default = "default_expander_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_expander_release_ms")]
        release_ms: f64,
        #[serde(default = "default_expander_range_db")]
        range_db: f64,
        #[serde(default = "default_expander_knee_db")]
        knee_db: f64,
        #[serde(default = "default_expander_hysteresis_db")]
        hysteresis_db: f64,
        #[serde(default = "default_expander_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_expander_mix")]
        mix: f64,
        #[serde(default = "default_expander_link_channels")]
        link_channels: bool,
        #[serde(default = "default_expander_sidechain_hpf_hz")]
        sidechain_hpf_hz: f64,
        #[serde(default)]
        auto_makeup: bool,
        #[serde(default)]
        lookahead_ms: f64,
        #[serde(default = "default_expander_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        measured_auto_makeup: bool,
    },
    MultibandCompressor {
        #[serde(default = "default_mb_compressor_num_bands")]
        num_bands: usize,
        #[serde(default = "default_mb_compressor_crossover_preset")]
        crossover_preset: i32,
        #[serde(default = "default_mb_compressor_crossover_freq_1")]
        crossover_freq_1: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_2")]
        crossover_freq_2: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_3")]
        crossover_freq_3: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_4")]
        crossover_freq_4: f64,
        #[serde(default = "default_mb_compressor_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_mb_compressor_ratio")]
        ratio: f64,
        #[serde(default = "default_mb_compressor_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_mb_compressor_release_ms")]
        release_ms: f64,
        #[serde(default = "default_mb_compressor_knee_db")]
        knee_db: f64,
        #[serde(default = "default_mb_compressor_mix")]
        mix: f64,
        #[serde(default = "default_mb_compressor_link_channels")]
        link_channels: bool,
        #[serde(default)]
        per_band_lookahead_ms: f64,
        #[serde(default)]
        ms_mode: bool,
        #[serde(default)]
        bands: Vec<BandCompressorParams>,
        #[serde(default)]
        sidechain_tilt_db: f64,
        #[serde(default = "default_mb_compressor_link_amount")]
        link_amount: f64,
    },
    MultibandExpander {
        #[serde(default = "default_mb_expander_num_bands")]
        num_bands: usize,
        #[serde(default = "default_mb_expander_crossover_preset")]
        crossover_preset: i32,
        #[serde(default = "default_mb_expander_crossover_freq_1")]
        crossover_freq_1: f64,
        #[serde(default = "default_mb_expander_crossover_freq_2")]
        crossover_freq_2: f64,
        #[serde(default = "default_mb_expander_crossover_freq_3")]
        crossover_freq_3: f64,
        #[serde(default = "default_mb_expander_crossover_freq_4")]
        crossover_freq_4: f64,
        #[serde(default = "default_mb_expander_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_mb_expander_ratio")]
        ratio: f64,
        #[serde(default = "default_mb_expander_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_mb_expander_release_ms")]
        release_ms: f64,
        #[serde(default = "default_mb_expander_range_db")]
        range_db: f64,
        #[serde(default = "default_mb_expander_knee_db")]
        knee_db: f64,
        #[serde(default = "default_mb_expander_hysteresis_db")]
        hysteresis_db: f64,
        #[serde(default = "default_mb_expander_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_mb_expander_mix")]
        mix: f64,
        #[serde(default = "default_mb_expander_link_channels")]
        link_channels: bool,
        #[serde(default = "default_mb_expander_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        lookahead_ms: f64,
        #[serde(default)]
        bands: Vec<BandExpanderParams>,
    },
    LoudnessCompensation {
        low_freq: f64,
        low_gain: f64,
        high_freq: f64,
        high_gain: f64,
        #[serde(default = "default_lc_mid_enabled")]
        mid_enabled: bool,
        #[serde(default = "default_lc_mid_freq")]
        mid_freq: f64,
        #[serde(default = "default_lc_mid_gain")]
        mid_gain: f64,
        #[serde(default = "default_lc_mid_q")]
        mid_q: f64,
        #[serde(default)]
        auto_gain_enabled: bool,
        #[serde(default = "default_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        /// 0 = Manual, 1 = ISO 226, 2 = Auto
        #[serde(default = "default_lc_mode")]
        mode: usize,
        #[serde(default = "default_lc_playback_level_db")]
        playback_level_db: f64,
        #[serde(default = "default_lc_reference_level_db")]
        reference_level_db: f64,
        /// Engine playback volume in dB (used in Auto mode)
        #[serde(default)]
        playback_volume_db: f64,
        #[serde(default)]
        auto_gain_position: usize,
        #[serde(default)]
        headroom_normalized: bool,
        #[serde(default)]
        auto_calibrated: bool,
    },
    FletcherMunson {
        /// Current playback volume (set by engine/UI)
        #[serde(default)]
        playback_volume_db: f64,
        /// Reference level where response is flat
        #[serde(default = "default_fm_reference_level_db")]
        reference_level_db: f64,
        /// Enabled bypass switch
        #[serde(default = "default_fm_enabled")]
        enabled: bool,
        /// Band 1 (sub-bass) parameters
        #[serde(default = "default_fm_band1_freq")]
        band1_freq: f64,
        #[serde(default = "default_fm_band1_q")]
        band1_q: f64,
        #[serde(default = "default_fm_band1_max_gain")]
        band1_max_gain: f64,
        #[serde(default = "default_fm_band1_slope")]
        band1_slope: f64,
        /// Band 2 (mid-bass) parameters
        #[serde(default = "default_fm_band2_freq")]
        band2_freq: f64,
        #[serde(default = "default_fm_band2_q")]
        band2_q: f64,
        #[serde(default = "default_fm_band2_max_gain")]
        band2_max_gain: f64,
        #[serde(default = "default_fm_band2_slope")]
        band2_slope: f64,
        /// Band 3 (presence) parameters
        #[serde(default = "default_fm_band3_freq")]
        band3_freq: f64,
        #[serde(default = "default_fm_band3_q")]
        band3_q: f64,
        #[serde(default = "default_fm_band3_max_gain")]
        band3_max_gain: f64,
        #[serde(default = "default_fm_band3_slope")]
        band3_slope: f64,
        /// Band 4 (air/brilliance) parameters
        #[serde(default = "default_fm_band4_freq")]
        band4_freq: f64,
        #[serde(default = "default_fm_band4_q")]
        band4_q: f64,
        #[serde(default = "default_fm_band4_max_gain")]
        band4_max_gain: f64,
        #[serde(default = "default_fm_band4_slope")]
        band4_slope: f64,
        /// Smoothing time for gain transitions (ms)
        #[serde(default = "default_fm_smoothing_ms")]
        smoothing_ms: f64,
        /// Auto-gain enabled
        #[serde(default)]
        auto_gain_enabled: bool,
        /// Auto-gain maximum correction in dB
        #[serde(default = "default_fm_auto_gain_max_db")]
        auto_gain_max_db: f64,
        /// Auto-gain smoothing time in ms
        #[serde(default = "default_fm_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        /// Auto-gain loudness type (0 = Momentary, 1 = ShortTerm)
        #[serde(default)]
        auto_gain_loudness_type: i32,
        /// Use ISO 226:2003 equal-loudness contours
        #[serde(default)]
        iso_226: bool,
    },
    BinauralDecoder {
        sofa_file: String,
        input_channels: usize,
        #[serde(default)] // 0.0
        externalization: f64,
        #[serde(default)] // 0.0
        near_field_strength: f64,
        #[serde(default)] // 0 = Linear
        crossfade_mode: usize,
        // Phase 4E: Late reverb
        #[serde(default)]
        late_reverb_enabled: bool,
        #[serde(default = "default_binaural_late_reverb_mix")]
        late_reverb_mix: f64,
        #[serde(default = "default_binaural_late_reverb_rt60")]
        late_reverb_rt60: f64,
        #[serde(default = "default_binaural_late_reverb_damping")]
        late_reverb_damping: f64,
        #[serde(default = "default_binaural_crossfade_ms")]
        crossfade_ms: f64,
        #[serde(default)]
        head_yaw_deg: f64,
        #[serde(default)]
        head_pitch_deg: f64,
        #[serde(default)]
        head_roll_deg: f64,
        #[serde(default)]
        hrtf_database_dir: String,
        #[serde(default = "default_binaural_head_width_cm")]
        head_width_cm: f64,
        #[serde(default = "default_binaural_ear_height_cm")]
        ear_height_cm: f64,
    },
    Convolution {
        ir_file: String,
        mix: f64,
        gain_db: f64,
        #[serde(default = "default_use_nupc")]
        use_nupc: bool,
        #[serde(default)]
        zero_latency_head: bool,
        #[serde(default = "default_head_taps")]
        head_taps: usize,
    },
    LoudnessMonitor,
    SpectrumAnalyzer {
        #[serde(default = "default_spectrum_num_bins")]
        num_bins: usize,
        #[serde(default = "default_spectrum_min_freq")]
        min_freq: f32,
        #[serde(default = "default_spectrum_max_freq")]
        max_freq: f32,
        #[serde(default = "default_spectrum_smoothing")]
        smoothing: f32,
        #[serde(default = "default_spectrum_tilt_correction")]
        tilt_correction: SpectralTiltCorrection,
        #[serde(default = "default_spectrum_tilt_reference")]
        tilt_reference: TiltReferenceFreq,
    },
    ChannelMuteSolo {
        enabled: bool,
        #[serde(default = "default_cms_dim_gain_db")]
        dim_gain_db: f64,
        #[serde(default = "default_cms_fade_ms")]
        fade_ms: f64,
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
    Matrix {
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>, // Row-major: matrix[out * in_count + in] = linear_gain
        #[serde(default)]
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
    XTC {
        #[serde(default = "default_xtc_distance_m")]
        distance_m: f64,
        #[serde(default = "default_xtc_speaker_angle_deg")]
        speaker_angle_deg: f64,
        #[serde(default = "default_xtc_head_radius_m")]
        head_radius_m: f64,
        #[serde(default = "default_xtc_beta_base")]
        beta_base: f64,
        #[serde(default = "default_xtc_beta_low_freq_boost")]
        beta_low_freq_boost: f64,
        #[serde(default = "default_xtc_beta_high_freq_boost")]
        beta_high_freq_boost: f64,
        #[serde(default = "default_xtc_head_shadow_cutoff_hz")]
        head_shadow_cutoff_hz: f64,
        #[serde(default = "default_xtc_head_shadow_slope")]
        head_shadow_slope_db_per_octave: f64,
        #[serde(default = "default_xtc_max_gain_db")]
        max_gain_db: f64,
        #[serde(default)]
        head_offset_x: f64,
        #[serde(default)]
        head_offset_z: f64,
        #[serde(default)]
        head_yaw_deg: f64,
        #[serde(default = "default_xtc_head_tracking_smooth_s")]
        head_tracking_smooth_s: f64,
        #[serde(default = "default_xtc_spectral_normalization")]
        spectral_normalization: bool,
        #[serde(default = "default_xtc_room_reflections_enabled")]
        room_reflections_enabled: bool,
        #[serde(default)]
        room_ir_file: Option<String>,
        #[serde(default = "default_xtc_room_width")]
        room_width_m: f64,
        #[serde(default = "default_xtc_room_depth")]
        room_depth_m: f64,
        #[serde(default = "default_xtc_wall_absorption")]
        wall_absorption: f64,
        #[serde(default = "default_xtc_reflection_beta_boost")]
        reflection_beta_boost: f64,
        #[serde(default)]
        bypass_xtc_filters: bool,
        #[serde(default)]
        bypass_spectral_normalization: bool,
        #[serde(default)]
        bypass_neumann_refinement: bool,
        #[serde(default = "default_xtc_auto_gain_enabled")]
        auto_gain_enabled: bool,
        #[serde(default = "default_xtc_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_xtc_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        #[serde(default = "default_xtc_pinna_model_enabled")]
        pinna_model_enabled: bool,
        #[serde(default)]
        head_model: f64,
    },
    Denoiser {
        #[serde(default = "default_denoiser_reduction_db")]
        reduction_db: f64,
        #[serde(default = "default_denoiser_floor_db")]
        floor_db: f64,
        #[serde(default = "default_denoiser_smoothing")]
        smoothing: f64,
        #[serde(default = "default_denoiser_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_denoiser_release_ms")]
        release_ms: f64,
        #[serde(default = "default_denoiser_low_latency")]
        low_latency: bool,
        #[serde(default = "default_denoiser_polyphonic_detection")]
        polyphonic_detection: bool,
        #[serde(default = "default_denoiser_mcra_alpha_s")]
        mcra_alpha_s: f64,
        #[serde(default = "default_denoiser_mcra_alpha_p")]
        mcra_alpha_p: f64,
        #[serde(default = "default_denoiser_mcra_l")]
        mcra_l: usize,
        #[serde(default = "default_denoiser_mcra_delta")]
        mcra_delta: f64,
        #[serde(default = "default_denoiser_transparency")]
        transparency: f64,
        #[serde(default = "default_denoiser_dd_enabled")]
        dd_enabled: bool,
        #[serde(default = "default_denoiser_dd_alpha")]
        dd_alpha: f64,
        #[serde(default = "default_denoiser_psychoacoustic_masking")]
        psychoacoustic_masking: bool,
        #[serde(default = "default_denoiser_spectral_smoothing_enabled")]
        spectral_smoothing_enabled: bool,
        #[serde(default = "default_denoiser_temporal_smoothing_enabled")]
        temporal_smoothing_enabled: bool,
        #[serde(default = "default_denoiser_spectral_sub_enabled")]
        spectral_sub_enabled: bool,
        #[serde(default = "default_denoiser_spectral_sub_alpha")]
        spectral_sub_alpha: f64,
        #[serde(default = "default_denoiser_spectral_sub_beta")]
        spectral_sub_beta: f64,
        #[serde(default)]
        learn_noise: bool,
        #[serde(default = "default_denoiser_use_captured_profile")]
        use_captured_profile: bool,
        #[serde(default)]
        clear_profile: bool,
        #[serde(default)]
        formant_preservation: bool,
        #[serde(default = "default_denoiser_formant_strength")]
        formant_strength: f64,
        #[serde(default)]
        multi_resolution: bool,
        #[serde(default)]
        harmonic_percussive: bool,
        #[serde(default)]
        spatial_denoise: bool,
        #[serde(default = "default_spatial_strength")]
        spatial_strength: f64,
    },
    Declick {
        #[serde(default = "default_declick_enabled")]
        enabled: bool,
        #[serde(default = "default_declick_sensitivity")]
        sensitivity: f64,
        #[serde(default = "default_declick_link_channels")]
        link_channels: bool,
    },
    HissReducer {
        #[serde(default = "default_hiss_reducer_enabled")]
        enabled: bool,
        #[serde(default = "default_hiss_reducer_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_hiss_reducer_frequency_hz")]
        frequency_hz: f64,
        #[serde(default = "default_hiss_reducer_strength")]
        strength: f64,
    },
    SpeechDenoiser {
        #[serde(default = "default_speech_denoiser_enabled")]
        enabled: bool,
    },
    Pnd {
        #[serde(default = "default_pnd_correction_strength")]
        correction_strength: f64,
        #[serde(default = "default_pnd_analysis_window_ms")]
        analysis_window_ms: f64,
        #[serde(default = "default_pnd_drift_smoothing")]
        drift_smoothing: f64,
        #[serde(default = "default_pnd_multi_channel_analysis")]
        multi_channel_analysis: bool,
        #[serde(default = "default_pnd_confidence_threshold")]
        confidence_threshold: f64,
        #[serde(default)]
        phase_vocoder: bool,
    },
    ABCompare {
        /// A/B mix: -1.0 = A only, 0.0 = 50/50, 1.0 = B only
        #[serde(default)]
        mix: f64,
        /// Mix mode: 0 = potentiometer (continuous), 1 = binary (A or B)
        #[serde(default)]
        mix_mode: i32,
        /// Selected path in binary mode: 0 = A, 1 = B
        #[serde(default)]
        selected_path: i32,
        /// Bypass: output original input
        #[serde(default)]
        bypass: bool,
        /// Enable automatic loudness matching
        #[serde(default = "default_ab_auto_gain_enabled")]
        auto_gain_enabled: bool,
        /// Loudness measurement type: 0 = momentary (400ms), 1 = short-term (3s)
        #[serde(default)]
        loudness_type: i32,
        /// Maximum auto-gain adjustment in dB
        #[serde(default = "default_ab_max_auto_gain_db")]
        max_auto_gain_db: f64,
        /// Gain smoothing time in ms
        #[serde(default = "default_ab_gain_smoothing_ms")]
        gain_smoothing_ms: f64,
        /// Mix transition time in ms
        #[serde(default = "default_ab_mix_transition_ms")]
        mix_transition_ms: f64,
        /// Path A configuration (JSON)
        #[serde(default = "default_ab_path_config")]
        path_a_config: String,
        /// Path B configuration (JSON)
        #[serde(default = "default_ab_path_config")]
        path_b_config: String,
        /// Path A config source file (for display only)
        #[serde(default)]
        path_a_file: String,
        /// Path B config source file (for display only)
        #[serde(default)]
        path_b_file: String,
        #[serde(default)]
        phase_invert_a: bool,
        #[serde(default)]
        phase_invert_b: bool,
        #[serde(default)]
        difference_mode: bool,
        #[serde(default = "default_ab_band_mask_low_hz")]
        band_mask_low_hz: f64,
        #[serde(default = "default_ab_band_mask_high_hz")]
        band_mask_high_hz: f64,
    },
    Crossover {
        /// Crossover type: "LR24" or "LinearPhase"
        #[serde(rename = "type", default = "default_crossover_type")]
        crossover_type: String,
        /// Primary crossover frequency in Hz
        #[serde(default = "default_crossover_frequency")]
        frequency: f64,
        /// Output mode: "lowpass", "highpass", or "both"
        #[serde(default = "default_crossover_output")]
        output: String,
        /// FIR tap count for linear-phase mode
        #[serde(default = "default_crossover_fir_taps")]
        fir_taps: usize,
    },
    BandSplit {
        /// Number of input channels
        #[serde(default = "default_channels")]
        channels: usize,
        /// Crossover frequency in Hz
        #[serde(default = "default_band_split_frequency")]
        frequency: f64,
        /// Crossover type: "LR24" or "LR48"
        #[serde(default = "default_band_split_crossover_type")]
        crossover_type: String,
    },
    BandMerge {
        /// Number of output channels
        #[serde(default = "default_channels")]
        channels: usize,
        /// Number of bands to merge
        #[serde(default = "default_band_merge_bands")]
        bands: usize,
    },
    Downmix {
        #[serde(default = "default_channels")]
        input_channels: usize,
        #[serde(default)]
        input_layout: Option<String>,
        #[serde(default = "default_downmix_center_gain_db")]
        center_gain_db: f64,
        #[serde(default = "default_downmix_surround_gain_db")]
        surround_gain_db: f64,
        #[serde(default = "default_downmix_height_gain_db")]
        height_gain_db: f64,
        #[serde(default = "default_downmix_lfe_gain_db")]
        lfe_gain_db: f64,
        #[serde(default = "default_downmix_phase_coherence")]
        phase_coherence: bool,
        #[serde(default = "default_downmix_phase_blend_low_hz")]
        phase_blend_low_hz: f64,
        #[serde(default = "default_downmix_phase_blend_high_hz")]
        phase_blend_high_hz: f64,
        #[serde(default)]
        itu_mode: bool,
        #[serde(default = "default_downmix_matrix_ltrt")]
        matrix_ltrt: bool,
    },
    MonoToStereo {
        #[serde(default = "default_mono_to_stereo_width")]
        stereo_width: f64,
        #[serde(default = "default_mono_to_stereo_haas_delay_ms")]
        haas_delay_ms: f64,
        #[serde(default = "default_mono_to_stereo_decor_low_hz")]
        decor_low_hz: f64,
        #[serde(default = "default_mono_to_stereo_decor_high_hz")]
        decor_high_hz: f64,
        #[serde(default = "default_mono_to_stereo_freq_dependent")]
        freq_dependent: bool,
    },
    Crossfeed {
        #[serde(default)]
        mode: CrossfeedMode,
        #[serde(default)]
        preset: CrossfeedPreset,
        #[serde(default = "default_crossfeed_enabled")]
        enabled: bool,
        #[serde(default = "default_crossfeed_mix")]
        mix: f64,
        // Bauer
        #[serde(default = "default_crossfeed_bauer_fcut_hz")]
        bauer_fcut_hz: f64,
        #[serde(default = "default_crossfeed_bauer_feed_db")]
        bauer_feed_db: f64,
        // Meier
        #[serde(default = "default_crossfeed_meier_level")]
        meier_level: f64,
        // Multiband
        #[serde(default = "default_crossfeed_mb_low_freq_hz")]
        mb_low_freq_hz: f64,
        #[serde(default = "default_crossfeed_mb_mid_high_freq_hz")]
        mb_mid_high_freq_hz: f64,
        #[serde(default = "default_crossfeed_mb_low_feed_db")]
        mb_low_feed_db: f64,
        #[serde(default = "default_crossfeed_mb_mid_feed_db")]
        mb_mid_feed_db: f64,
        #[serde(default = "default_crossfeed_mb_high_feed_db")]
        mb_high_feed_db: f64,
        // ITD
        #[serde(default)]
        itd_delay_ms: f64,
        // Auto gain
        #[serde(default)]
        autogain_enabled: bool,
        #[serde(default = "default_crossfeed_autogain_target_lufs")]
        autogain_target_lufs: f64,
        #[serde(default = "default_crossfeed_autogain_max_gain_db")]
        autogain_max_gain_db: f64,
        #[serde(default = "default_crossfeed_autogain_smoothing_ms")]
        autogain_smoothing_ms: f64,
    },
    Delay {
        #[serde(default = "default_delay_ms")]
        delay_ms: f64,
        #[serde(default = "default_delay_feedback")]
        feedback: f64,
        #[serde(default = "default_delay_mix")]
        mix: f64,
        #[serde(default)]
        lfo_rate_hz: f64,
        #[serde(default)]
        lfo_depth_ms: f64,
        #[serde(default)]
        allpass_feedback: bool,
        #[serde(default = "default_delay_allpass_coeff")]
        allpass_coeff: f64,
    },
    Dither {
        #[serde(default = "default_dither_bit_depth")]
        bit_depth: usize,
        #[serde(default = "default_dither_noise_shaping")]
        noise_shaping: bool,
        #[serde(default = "default_dither_type")]
        dither_type: usize,
    },
    Aec {
        #[serde(default = "default_aec_echo_tail_ms")]
        echo_tail_ms: f64,
        #[serde(default = "default_aec_step_size")]
        step_size: f64,
        #[serde(default = "default_aec_post_filter_enabled")]
        post_filter_enabled: bool,
    },
    Beamformer {
        #[serde(default = "default_beamformer_num_mics")]
        num_mics: usize,
        #[serde(default = "default_beamformer_mic_spacing_cm")]
        mic_spacing_cm: f64,
        #[serde(default = "default_beamformer_steer_angle_deg")]
        steer_angle_deg: f64,
        #[serde(default = "default_beamformer_type")]
        beamformer_type: usize,
    },
    AmbisonicsDecoder {
        #[serde(default = "default_ambisonics_order")]
        order: usize,
        #[serde(default = "default_ambisonics_target_layout")]
        target_layout: String,
        #[serde(default = "default_ambisonics_max_re")]
        max_re_weighting: bool,
        #[serde(default)]
        dual_band: bool,
    },
    StereoImager {
        #[serde(default = "default_si_width")]
        width: f64,
        #[serde(default = "default_si_low_mid_freq")]
        low_mid_freq: f64,
        #[serde(default = "default_si_mid_high_freq")]
        mid_high_freq: f64,
        #[serde(default = "default_si_low_width")]
        low_width: f64,
        #[serde(default = "default_si_mid_width")]
        mid_width: f64,
        #[serde(default = "default_si_high_width")]
        high_width: f64,
        #[serde(default = "default_si_mono_bass")]
        mono_bass: bool,
        #[serde(default = "default_si_mix")]
        mix: f64,
    },
    DeEsser {
        #[serde(default = "default_de_esser_frequency")]
        frequency: f64,
        #[serde(default = "default_de_esser_q")]
        q: f64,
        #[serde(default = "default_de_esser_threshold")]
        threshold: f64,
        #[serde(default = "default_de_esser_ratio")]
        ratio: f64,
        #[serde(default = "default_de_esser_attack")]
        attack: f64,
        #[serde(default = "default_de_esser_release")]
        release: f64,
        #[serde(default = "default_de_esser_mode")]
        mode: String,
        #[serde(default = "default_de_esser_mix")]
        mix: f64,
    },
    TransientShaper {
        #[serde(default)]
        attack: f64,
        #[serde(default)]
        sustain: f64,
        #[serde(default)]
        sensitivity_db: f64,
        #[serde(default)]
        output_gain_db: f64,
        #[serde(default = "default_ts_mix")]
        mix: f64,
    },
    Saturation {
        #[serde(default = "default_sat_mode")]
        mode: f64,
        #[serde(default = "default_sat_drive")]
        drive: f64,
        #[serde(default = "default_sat_tone")]
        tone: f64,
        #[serde(default = "default_sat_exciter_freq")]
        exciter_freq: f64,
        #[serde(default = "default_sat_oversampling")]
        oversampling: f64,
        #[serde(default = "default_sat_output_gain")]
        output_gain_db: f64,
        #[serde(default = "default_sat_mix")]
        mix: f64,
        #[serde(default)]
        dynamic_amount: f64,
        #[serde(default = "default_sat_dynamic_attack_ms")]
        dynamic_attack_ms: f64,
        #[serde(default = "default_sat_dynamic_release_ms")]
        dynamic_release_ms: f64,
        #[serde(default = "default_sat_dc_blocker")]
        dc_blocker: bool,
        #[serde(default = "default_sat_use_adaa")]
        use_adaa: bool,
    },
    DynamicEq {
        #[serde(default = "default_dyneq_num_bands")]
        num_bands: f64,
        #[serde(default = "default_dyneq_threshold")]
        threshold: f64,
        #[serde(default = "default_dyneq_ratio")]
        ratio: f64,
        #[serde(default = "default_dyneq_attack")]
        attack: f64,
        #[serde(default = "default_dyneq_release")]
        release: f64,
        #[serde(default = "default_dyneq_knee")]
        knee: f64,
        #[serde(default = "default_dyneq_link_channels")]
        link_channels: bool,
        #[serde(default = "default_dyneq_mix")]
        mix: f64,
        #[serde(default = "default_dyneq_bands")]
        bands: Vec<DynEqBandParams>,
    },
    #[serde(alias = "FirDesigner")]
    LinearPhaseEq {
        #[serde(default = "default_lpeq_num_filters")]
        num_filters: f64,
        #[serde(default = "default_lpeq_fir_length")]
        fir_length: f64,
        #[serde(default = "default_lpeq_phase_mode")]
        phase_mode: f64,
        #[serde(default)]
        auto_gain: bool,
        #[serde(default = "default_lpeq_mix")]
        mix: f64,
        #[serde(default)]
        filters: Vec<EQFilter>,
    },
    SpectralCompressor {
        #[serde(default = "default_sc_fft_size")]
        fft_size: usize,
        #[serde(default = "default_sc_threshold")]
        threshold: f64,
        #[serde(default = "default_sc_ratio")]
        ratio: f64,
        #[serde(default = "default_sc_attack")]
        attack: f64,
        #[serde(default = "default_sc_release")]
        release: f64,
        #[serde(default = "default_sc_knee")]
        knee: f64,
        #[serde(default = "default_sc_spectral_smoothing")]
        spectral_smoothing: f64,
        #[serde(default = "default_sc_mix")]
        mix: f64,
        #[serde(default)]
        target_mode: f64,
        #[serde(default)]
        delta_listen: bool,
        // Phase 4A: Adaptive threshold
        #[serde(default)]
        adaptive_threshold: bool,
        #[serde(default)]
        adaptive_offset_db: f64,
    },
    /// Concrete third-party plugin state. Unlike built-in variants, this must
    /// be created from a scanner-provided descriptor and has no default.
    External {
        state: ExternalPluginState,
    },
}

impl PluginSettings {
    /// Mutable access to the global EQ band list shared by EQ and LinearPhaseEq.
    /// Returns `None` for any other plugin variant.
    pub fn eq_global_filters_mut(&mut self) -> Option<&mut Vec<EQFilter>> {
        match self {
            Self::EQ { filters, .. } => Some(filters),
            Self::LinearPhaseEq { filters, .. } => Some(filters),
            _ => None,
        }
    }

    /// Read-only counterpart of [`eq_global_filters_mut`].
    pub fn eq_global_filters(&self) -> Option<&Vec<EQFilter>> {
        match self {
            Self::EQ { filters, .. } => Some(filters),
            Self::LinearPhaseEq { filters, .. } => Some(filters),
            _ => None,
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        match self {
            Self::EQ { .. } => PluginType::EQ,
            Self::Gain { .. } => PluginType::Gain,
            Self::Upmixer { .. } => PluginType::Upmixer,
            Self::Compressor { .. } => PluginType::Compressor,
            Self::Limiter { .. } => PluginType::Limiter,
            Self::Gate { .. } => PluginType::Gate,
            Self::Expander { .. } => PluginType::Expander,
            Self::MultibandCompressor { .. } => PluginType::MultibandCompressor,
            Self::MultibandExpander { .. } => PluginType::MultibandExpander,
            Self::LoudnessCompensation { .. } => PluginType::LoudnessCompensation,
            Self::FletcherMunson { .. } => PluginType::FletcherMunson,
            Self::BinauralDecoder { .. } => PluginType::BinauralDecoder,
            Self::Convolution { .. } => PluginType::Convolution,
            Self::LoudnessMonitor => PluginType::LoudnessMonitor,
            Self::SpectrumAnalyzer { .. } => PluginType::SpectrumAnalyzer,
            Self::ChannelMuteSolo { .. } => PluginType::ChannelMuteSolo,
            Self::Matrix { .. } => PluginType::Matrix,
            Self::XTC { .. } => PluginType::XTC,
            Self::Denoiser { .. } => PluginType::Denoiser,
            Self::Declick { .. } => PluginType::Declick,
            Self::HissReducer { .. } => PluginType::HissReducer,
            Self::SpeechDenoiser { .. } => PluginType::SpeechDenoiser,
            Self::Pnd { .. } => PluginType::Pnd,
            Self::ABCompare { .. } => PluginType::ABCompare,
            Self::Crossover { .. } => PluginType::Crossover,
            Self::BandSplit { .. } => PluginType::BandSplit,
            Self::BandMerge { .. } => PluginType::BandMerge,
            Self::Downmix { .. } => PluginType::Downmix,
            Self::MonoToStereo { .. } => PluginType::MonoToStereo,
            Self::Crossfeed { .. } => PluginType::Crossfeed,
            Self::Delay { .. } => PluginType::Delay,
            Self::Dither { .. } => PluginType::Dither,
            Self::Aec { .. } => PluginType::Aec,
            Self::Beamformer { .. } => PluginType::Beamformer,
            Self::AmbisonicsDecoder { .. } => PluginType::AmbisonicsDecoder,
            Self::StereoImager { .. } => PluginType::StereoImager,
            Self::DeEsser { .. } => PluginType::DeEsser,
            Self::TransientShaper { .. } => PluginType::TransientShaper,
            Self::Saturation { .. } => PluginType::Saturation,
            Self::DynamicEq { .. } => PluginType::DynamicEq,
            Self::LinearPhaseEq { .. } => PluginType::LinearPhaseEq,
            Self::SpectralCompressor { .. } => PluginType::SpectralCompressor,
            Self::External { .. } => PluginType::External,
            Self::AAE { .. } => PluginType::AAE,
        }
    }

    /// Returns the fixed input channel count this plugin requires, or None if it adapts to any.
    pub fn required_input_channels(&self) -> Option<usize> {
        match self {
            Self::Upmixer { .. } => Some(2),
            Self::AAE { .. } => Some(2),
            Self::StereoImager { .. } => Some(2),
            Self::XTC { .. } => Some(2),
            Self::Crossfeed { .. } => Some(2),
            Self::MonoToStereo { .. } => Some(1),
            Self::Aec { .. } => Some(2),
            Self::Beamformer { num_mics, .. } => Some(*num_mics),
            Self::BinauralDecoder { input_channels, .. } => Some(*input_channels),
            Self::AmbisonicsDecoder { order, .. } => {
                let channels_per_axis = order.saturating_add(1);
                Some(channels_per_axis.saturating_mul(channels_per_axis))
            }
            Self::External { state } if state.descriptor.audio_inputs > 0 => {
                Some(state.descriptor.audio_inputs)
            }
            _ => None,
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> PluginConfig {
        let wire_type = self.plugin_type().wire_name();
        if let Some(config) = super::plugin_config_converter::PluginConfigConverterRegistry::global(
        )
        .convert(wire_type, self, sample_rate)
        {
            return config;
        }

        #[allow(clippy::match_single_binding)]
        match self {
            _ => unreachable!(
                "plugin type {} should be handled by the converter registry",
                wire_type
            ),
        }
    }

    /// Create default settings for a plugin type
    pub fn default_for(plugin_type: &PluginType) -> Result<Self, String> {
        use sotf_plugins::param_specs::find_by_key as p;

        let settings = match plugin_type {
            PluginType::EQ => Self::EQ {
                channels: default_channels(),
                filters: vec![
                    // Default: 5-band flat EQ
                    EQFilter::new(BiquadFilterType::Peak, 100.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 300.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 3000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 10000.0, 1.4, 0.0),
                ],
                channel_filters: None,
                per_channel_mode: false,
                max_filters: 5,
                tdf2: false,
                topology: 0.0,
                auto_gain_enabled: false,
                oversampling: default_eq_oversampling(),
            },
            PluginType::Gain => Self::Gain {
                channels: default_channels(),
                gain_db: p(gain_specs::PARAMS, "gain_db").default_f64(),
                smoothing_ms: p(gain_specs::PARAMS, "smoothing_ms").default_f64(),
            },
            PluginType::Upmixer => {
                let u = upmixer_specs::PARAMS;
                Self::Upmixer {
                    speaker_config: "5.1".to_string(),
                    gains: UpmixerGainSettings {
                        gain_front_direct: p(u, "gain_front_direct").default_f64(),
                        gain_front_ambient: p(u, "gain_front_ambient").default_f64(),
                        gain_rear_ambient: p(u, "gain_rear_ambient").default_f64(),
                        height_gain: p(u, "height_gain").default_f64(),
                        stereo_width: p(u, "stereo_width").default_f64(),
                        center_spread: p(u, "center_spread").default_f64(),
                        surround_direct_bleed: p(u, "surround_direct_bleed").default_f64(),
                        rear_late_reflection: p(u, "rear_late_reflection").default_f64(),
                        ambient_boost: p(u, "ambient_boost").default_f64(),
                        rear_ambient_boost: p(u, "rear_ambient_boost").default_f64(),
                    },
                    lfe: UpmixerLfeSettings {
                        lfe_cutoff_hz: p(u, "lfe_cutoff_hz").default_f64(),
                        lfe_gain: p(u, "lfe_gain").default_f64(),
                        bandpass_hz: p(u, "bandpass_hz").default_f64(),
                    },
                    subharmonic: UpmixerSubharmonicSettings {
                        enable_subharmonic_synth: p(u, "enable_subharmonic_synth").default_bool(),
                        subharmonic_gain: p(u, "subharmonic_gain").default_f64(),
                        subharmonic_freq_hz: p(u, "subharmonic_freq_hz").default_f64(),
                        subharmonic_attack_ms: p(u, "subharmonic_attack_ms").default_f64(),
                        subharmonic_release_ms: p(u, "subharmonic_release_ms").default_f64(),
                    },
                    decorrelation: UpmixerDecorrelationSettings {
                        decorrelation_mode: p(u, "decorrelation_mode").default_usize(),
                        decorrelation_lfo_rate_hz: p(u, "decorrelation_lfo_rate_hz").default_f64(),
                        velvet_noise_duration_ms: p(u, "velvet_noise_duration_ms").default_f64(),
                        velvet_noise_density: p(u, "velvet_noise_density").default_f64(),
                    },
                    height: UpmixerHeightSettings {
                        enable_hr_direct: p(u, "enable_hr_direct").default_bool(),
                        hr_sharpen: p(u, "hr_sharpen").default_f64(),
                        height_hf_cap_hz: p(u, "height_hf_cap_hz").default_f64(),
                        height_transient_reduction: p(u, "height_transient_reduction")
                            .default_f64(),
                        height_direct_leak: p(u, "height_direct_leak").default_f64(),
                    },
                    ambient_analysis: UpmixerAmbientAnalysisSettings {
                        low_latency: p(u, "low_latency").default_bool(),
                        frequency_resolution: p(u, "frequency_resolution").default_usize(),
                        safety_cap_db: p(u, "safety_cap_db").default_f64(),
                    },
                    dialogue: UpmixerDialogueSettings {
                        dialogue_weight: p(u, "dialogue_weight").default_f64(),
                        voice_freq_min_hz: p(u, "voice_freq_min_hz").default_f64(),
                        voice_freq_max_hz: p(u, "voice_freq_max_hz").default_f64(),
                        dialogue_centroid_weight: p(u, "dialogue_centroid_weight").default_f64(),
                        dialogue_variance_weight: p(u, "dialogue_variance_weight").default_f64(),
                        dialogue_coherence_weight: p(u, "dialogue_coherence_weight").default_f64(),
                    },
                    bypass: UpmixerBypassSettings {
                        bypass_decorrelation: false,
                        bypass_transient_detection: false,
                        bypass_all_processing: false,
                    },
                    output: UpmixerOutputSettings {
                        enable_ml_detection: p(u, "enable_ml_detection").default_bool(),
                        multi_source_extraction: p(u, "multi_source_extraction").default_bool(),
                        multi_source_threshold: p(u, "multi_source_threshold").default_f64(),
                        binaural_preview: p(u, "binaural_preview").default_bool(),
                        auto_gain_enabled: p(u, "auto_gain_enabled").default_bool(),
                        auto_gain_max_db: p(u, "auto_gain_max_db").default_f64(),
                        auto_gain_smoothing_ms: p(u, "auto_gain_smoothing_ms").default_f64(),
                    },
                }
            }
            PluginType::Compressor => {
                let c = compressor_specs::PARAMS;
                Self::Compressor {
                    threshold_db: p(c, "threshold").default_f64(),
                    ratio: p(c, "ratio").default_f64(),
                    attack_ms: p(c, "attack").default_f64(),
                    release_ms: p(c, "release").default_f64(),
                    knee_db: p(c, "knee").default_f64(),
                    makeup_gain_db: p(c, "makeup_gain").default_f64(),
                    mix: p(c, "mix").default_f64(),
                    auto_makeup: p(c, "auto_makeup").default_bool(),
                    link_channels: p(c, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(c, "sidechain_hpf_hz").default_f64(),
                    sidechain_hpf_order: default_compressor_sidechain_hpf_order(),
                    detection_mode: default_compressor_detection_mode(),
                    lookahead_ms: p(c, "lookahead_ms").default_f64(),
                    program_dependent_release: p(c, "program_dependent_release").default_bool(),
                    measured_auto_makeup: p(c, "measured_auto_makeup").default_bool(),
                    sidechain_external: p(c, "sidechain_external").default_bool(),
                }
            }
            PluginType::Limiter => {
                let l = limiter_specs::PARAMS;
                Self::Limiter {
                    threshold_db: p(l, "threshold").default_f64(),
                    release_ms: p(l, "release").default_f64(),
                    lookahead_ms: p(l, "lookahead").default_f64(),
                    soft: p(l, "soft").default_bool(),
                    true_peak: p(l, "true_peak").default_bool(),
                    isp_mode: p(l, "isp_mode").default_bool(),
                    dual_release: p(l, "dual_release").default_bool(),
                    mix: p(l, "mix").default_f64(),
                    link_amount: p(l, "link_amount").default_f64(),
                    feed_forward: p(l, "feed_forward").default_bool(),
                }
            }
            PluginType::Gate => {
                let g = gate_specs::PARAMS;
                Self::Gate {
                    threshold_db: p(g, "threshold").default_f64(),
                    ratio: p(g, "ratio").default_f64(),
                    attack_ms: p(g, "attack").default_f64(),
                    hold_ms: p(g, "hold").default_f64(),
                    release_ms: p(g, "release").default_f64(),
                    mix: p(g, "mix").default_f64(),
                    link_channels: p(g, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(g, "sidechain_hpf_hz").default_f64(),
                    sidechain_hpf_order: default_gate_sidechain_hpf_order(),
                    detection_mode: default_gate_detection_mode(),
                    sidechain_external: p(g, "sidechain_external").default_bool(),
                    range_db: p(g, "range_db").default_f64(),
                    hysteresis_db: p(g, "hysteresis_db").default_f64(),
                    knee_db: p(g, "knee_db").default_f64(),
                    lookahead_ms: p(g, "lookahead_ms").default_f64(),
                }
            }
            PluginType::Expander => {
                let e = expander_specs::PARAMS;
                Self::Expander {
                    threshold_db: p(e, "threshold").default_f64(),
                    ratio: p(e, "ratio").default_f64(),
                    attack_ms: p(e, "attack").default_f64(),
                    release_ms: p(e, "release").default_f64(),
                    range_db: p(e, "range").default_f64(),
                    knee_db: p(e, "knee").default_f64(),
                    hysteresis_db: p(e, "hysteresis").default_f64(),
                    hold_ms: p(e, "hold").default_f64(),
                    mix: p(e, "mix").default_f64(),
                    auto_makeup: p(e, "auto_makeup").default_bool(),
                    link_channels: p(e, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(e, "sidechain_hpf_hz").default_f64(),
                    lookahead_ms: p(e, "lookahead_ms").default_f64(),
                    detection_mode: default_expander_detection_mode(),
                    measured_auto_makeup: p(e, "measured_auto_makeup").default_bool(),
                }
            }
            PluginType::MultibandCompressor => {
                let mc = mb_compressor_specs::GLOBAL_PARAMS;
                Self::MultibandCompressor {
                    num_bands: p(mc, "num_bands").default_usize(),
                    crossover_preset: p(mc, "crossover_preset").default_i32(),
                    crossover_freq_1: p(mc, "crossover_freq_1").default_f64(),
                    crossover_freq_2: p(mc, "crossover_freq_2").default_f64(),
                    crossover_freq_3: p(mc, "crossover_freq_3").default_f64(),
                    crossover_freq_4: p(mc, "crossover_freq_4").default_f64(),
                    threshold_db: p(mc, "threshold").default_f64(),
                    ratio: p(mc, "ratio").default_f64(),
                    attack_ms: p(mc, "attack").default_f64(),
                    release_ms: p(mc, "release").default_f64(),
                    knee_db: p(mc, "knee").default_f64(),
                    mix: p(mc, "mix").default_f64(),
                    link_channels: p(mc, "link_channels").default_bool(),
                    per_band_lookahead_ms: p(mc, "per_band_lookahead_ms").default_f64(),
                    ms_mode: p(mc, "ms_mode").default_bool(),
                    bands: Vec::new(),
                    sidechain_tilt_db: 0.0,
                    link_amount: p(mc, "link_amount").default_f64(),
                }
            }
            PluginType::MultibandExpander => {
                let me = mb_expander_specs::GLOBAL_PARAMS;
                Self::MultibandExpander {
                    num_bands: p(me, "num_bands").default_usize(),
                    crossover_preset: p(me, "crossover_preset").default_i32(),
                    crossover_freq_1: p(me, "crossover_freq_1").default_f64(),
                    crossover_freq_2: p(me, "crossover_freq_2").default_f64(),
                    crossover_freq_3: p(me, "crossover_freq_3").default_f64(),
                    crossover_freq_4: p(me, "crossover_freq_4").default_f64(),
                    threshold_db: p(me, "threshold").default_f64(),
                    ratio: p(me, "ratio").default_f64(),
                    attack_ms: p(me, "attack").default_f64(),
                    release_ms: p(me, "release").default_f64(),
                    range_db: p(me, "range").default_f64(),
                    knee_db: p(me, "knee").default_f64(),
                    hysteresis_db: p(me, "hysteresis").default_f64(),
                    hold_ms: p(me, "hold").default_f64(),
                    mix: p(me, "mix").default_f64(),
                    link_channels: p(me, "link_channels").default_bool(),
                    detection_mode: default_mb_expander_detection_mode(),
                    lookahead_ms: p(me, "lookahead_ms").default_f64(),
                    bands: Vec::new(),
                }
            }
            PluginType::LoudnessCompensation => {
                let lc = lc_specs::PARAMS;
                Self::LoudnessCompensation {
                    low_freq: p(lc, "low_freq").default_f64(),
                    low_gain: p(lc, "low_gain").default_f64(),
                    high_freq: p(lc, "high_freq").default_f64(),
                    high_gain: p(lc, "high_gain").default_f64(),
                    mid_enabled: p(lc, "mid_enabled").default_bool(),
                    mid_freq: p(lc, "mid_freq").default_f64(),
                    mid_gain: p(lc, "mid_gain").default_f64(),
                    mid_q: p(lc, "mid_q").default_f64(),
                    auto_gain_enabled: p(lc, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(lc, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(lc, "auto_gain_smoothing_ms").default_f64(),
                    mode: p(lc, "mode").default_usize(),
                    playback_level_db: p(lc, "playback_level_db").default_f64(),
                    reference_level_db: p(lc, "reference_level_db").default_f64(),
                    playback_volume_db: 0.0,
                    auto_gain_position: p(lc, "auto_gain_position").default_usize(),
                    headroom_normalized: p(lc, "headroom_normalized").default_bool(),
                    auto_calibrated: p(lc, "auto_calibrated").default_bool(),
                }
            }
            PluginType::FletcherMunson => {
                // Fletcher-Munson merged into LoudnessCompensation with mode=2 (Auto)
                let lc = lc_specs::PARAMS;
                Self::LoudnessCompensation {
                    low_freq: p(lc, "low_freq").default_f64(),
                    low_gain: p(lc, "low_gain").default_f64(),
                    high_freq: p(lc, "high_freq").default_f64(),
                    high_gain: p(lc, "high_gain").default_f64(),
                    mid_enabled: p(lc, "mid_enabled").default_bool(),
                    mid_freq: p(lc, "mid_freq").default_f64(),
                    mid_gain: p(lc, "mid_gain").default_f64(),
                    mid_q: p(lc, "mid_q").default_f64(),
                    auto_gain_enabled: p(lc, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(lc, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(lc, "auto_gain_smoothing_ms").default_f64(),
                    mode: 2, // Auto mode
                    playback_level_db: p(lc, "playback_level_db").default_f64(),
                    reference_level_db: p(lc, "reference_level_db").default_f64(),
                    playback_volume_db: 0.0,
                    auto_gain_position: p(lc, "auto_gain_position").default_usize(),
                    headroom_normalized: p(lc, "headroom_normalized").default_bool(),
                    auto_calibrated: true,
                }
            }
            PluginType::BinauralDecoder => {
                let b = binaural_specs::PARAMS;
                Self::BinauralDecoder {
                    sofa_file: String::new(),
                    input_channels: 6, // Default to 5.1
                    externalization: p(b, "externalization").default_f64(),
                    near_field_strength: p(b, "near_field_strength").default_f64(),
                    crossfade_mode: p(b, "crossfade_mode").default_usize(),
                    late_reverb_enabled: false,
                    late_reverb_mix: p(b, "late_reverb_mix").default_f64(),
                    late_reverb_rt60: p(b, "late_reverb_rt60").default_f64(),
                    late_reverb_damping: p(b, "late_reverb_damping").default_f64(),
                    crossfade_ms: p(b, "crossfade_ms").default_f64(),
                    head_yaw_deg: p(b, "head_yaw_deg").default_f64(),
                    head_pitch_deg: p(b, "head_pitch_deg").default_f64(),
                    head_roll_deg: p(b, "head_roll_deg").default_f64(),
                    hrtf_database_dir: String::new(),
                    head_width_cm: p(b, "head_width_cm").default_f64(),
                    ear_height_cm: p(b, "ear_height_cm").default_f64(),
                }
            }
            PluginType::Convolution => {
                let cv = convolution_specs::PARAMS;
                Self::Convolution {
                    ir_file: String::new(),
                    mix: p(cv, "mix").default_f64(),
                    gain_db: p(cv, "gain_db").default_f64(),
                    use_nupc: p(cv, "use_nupc").default_bool(),
                    zero_latency_head: false,
                    head_taps: p(cv, "head_taps").default_usize(),
                }
            }
            PluginType::LoudnessMonitor => Self::LoudnessMonitor,
            PluginType::SpectrumAnalyzer => Self::SpectrumAnalyzer {
                num_bins: pk(spectrum_specs::PARAMS, "num_bins").default_usize(),
                min_freq: pk(spectrum_specs::PARAMS, "min_freq").default_f64() as f32,
                max_freq: pk(spectrum_specs::PARAMS, "max_freq").default_f64() as f32,
                smoothing: pk(spectrum_specs::PARAMS, "smoothing").default_f64() as f32,
                tilt_correction: SpectralTiltCorrection::None,
                tilt_reference: TiltReferenceFreq::Standard,
            },
            PluginType::ChannelMuteSolo => Self::ChannelMuteSolo {
                enabled: pk(cms_specs::PARAMS, "enabled").default_bool(),
                dim_gain_db: pk(cms_specs::PARAMS, "dim_gain_db").default_f64(),
                fade_ms: pk(cms_specs::PARAMS, "fade_ms").default_f64(),
                channel_states: vec![],
            },
            PluginType::Matrix => Self::Matrix {
                input_channels: 2,
                output_channels: 2,
                matrix: vec![
                    pk(matrix_specs::PARAMS, "gain").max_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").min_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").min_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").max_f64() as f32,
                ], // Identity 2x2
                channel_states: vec![],
            },
            PluginType::XTC => {
                let x = xtc_specs::PARAMS;
                Self::XTC {
                    distance_m: p(x, "distance_m").default_f64(),
                    speaker_angle_deg: p(x, "speaker_angle_deg").default_f64(),
                    head_radius_m: p(x, "head_radius_m").default_f64(),
                    beta_base: p(x, "beta_base").default_f64(),
                    beta_low_freq_boost: p(x, "beta_low_freq_boost").default_f64(),
                    beta_high_freq_boost: p(x, "beta_high_freq_boost").default_f64(),
                    head_shadow_cutoff_hz: p(x, "head_shadow_cutoff_hz").default_f64(),
                    head_shadow_slope_db_per_octave: p(x, "head_shadow_slope_db_per_octave")
                        .default_f64(),
                    max_gain_db: p(x, "max_gain_db").default_f64(),
                    head_offset_x: p(x, "head_offset_x").default_f64(),
                    head_offset_z: p(x, "head_offset_z").default_f64(),
                    head_yaw_deg: p(x, "head_yaw_deg").default_f64(),
                    head_tracking_smooth_s: pk(xtc_specs::PARAMS, "head_tracking_smooth_s")
                        .default_f64(),
                    spectral_normalization: p(x, "spectral_normalization").default_bool(),
                    room_reflections_enabled: p(x, "room_reflections_enabled").default_bool(),
                    room_ir_file: None,
                    room_width_m: p(x, "room_width_m").default_f64(),
                    room_depth_m: p(x, "room_depth_m").default_f64(),
                    wall_absorption: p(x, "wall_absorption").default_f64(),
                    reflection_beta_boost: p(x, "reflection_beta_boost").default_f64(),
                    bypass_xtc_filters: p(x, "bypass_xtc_filters").default_bool(),
                    bypass_spectral_normalization: p(x, "bypass_spectral_normalization")
                        .default_bool(),
                    bypass_neumann_refinement: p(x, "bypass_neumann_refinement").default_bool(),
                    auto_gain_enabled: p(x, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(x, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(x, "auto_gain_smoothing_ms").default_f64(),
                    pinna_model_enabled: p(x, "pinna_model_enabled").default_bool(),
                    head_model: p(x, "head_model").default_f64(),
                }
            }
            PluginType::Denoiser => {
                let d = denoiser_specs::PARAMS;
                Self::Denoiser {
                    reduction_db: p(d, "reduction_db").default_f64(),
                    floor_db: p(d, "floor_db").default_f64(),
                    smoothing: p(d, "smoothing").default_f64(),
                    attack_ms: p(d, "attack_ms").default_f64(),
                    release_ms: p(d, "release_ms").default_f64(),
                    low_latency: p(d, "low_latency").default_bool(),
                    polyphonic_detection: p(d, "polyphonic_detection").default_bool(),
                    mcra_alpha_s: p(d, "mcra_alpha_s").default_f64(),
                    mcra_alpha_p: p(d, "mcra_alpha_p").default_f64(),
                    mcra_l: p(d, "mcra_l").default_usize(),
                    mcra_delta: p(d, "mcra_delta").default_f64(),
                    transparency: p(d, "transparency").default_f64(),
                    dd_enabled: p(d, "dd_enabled").default_bool(),
                    dd_alpha: p(d, "dd_alpha").default_f64(),
                    psychoacoustic_masking: p(d, "psychoacoustic_masking").default_bool(),
                    spectral_smoothing_enabled: p(d, "spectral_smoothing_enabled").default_bool(),
                    temporal_smoothing_enabled: p(d, "temporal_smoothing_enabled").default_bool(),
                    spectral_sub_enabled: p(d, "spectral_sub_enabled").default_bool(),
                    spectral_sub_alpha: p(d, "spectral_sub_alpha").default_f64(),
                    spectral_sub_beta: p(d, "spectral_sub_beta").default_f64(),
                    learn_noise: p(d, "learn_noise").default_bool(),
                    use_captured_profile: p(d, "use_captured_profile").default_bool(),
                    clear_profile: p(d, "clear_profile").default_bool(),
                    formant_preservation: p(d, "formant_preservation").default_bool(),
                    formant_strength: p(d, "formant_strength").default_f64(),
                    multi_resolution: p(d, "multi_resolution").default_bool(),
                    harmonic_percussive: false,
                    spatial_denoise: false,
                    spatial_strength: p(d, "spatial_strength").default_f64(),
                }
            }
            PluginType::Declick => {
                let dc = declick_specs::PARAMS;
                Self::Declick {
                    enabled: p(dc, "enabled").default_bool(),
                    sensitivity: p(dc, "sensitivity").default_f64(),
                    link_channels: p(dc, "link_channels").default_bool(),
                }
            }
            PluginType::HissReducer => {
                let hr = hiss_reducer_specs::PARAMS;
                Self::HissReducer {
                    enabled: p(hr, "enabled").default_bool(),
                    threshold_db: p(hr, "threshold_db").default_f64(),
                    frequency_hz: p(hr, "frequency_hz").default_f64(),
                    strength: p(hr, "strength").default_f64(),
                }
            }
            PluginType::SpeechDenoiser => {
                let sd = speech_denoiser_specs::PARAMS;
                Self::SpeechDenoiser {
                    enabled: p(sd, "enabled").default_bool(),
                }
            }
            PluginType::Pnd => {
                let pn = pnd_specs::PARAMS;
                Self::Pnd {
                    correction_strength: p(pn, "correction_strength").default_f64(),
                    analysis_window_ms: p(pn, "analysis_window_ms").default_f64(),
                    drift_smoothing: p(pn, "drift_smoothing").default_f64(),
                    multi_channel_analysis: p(pn, "multi_channel_analysis").default_bool(),
                    confidence_threshold: p(pn, "confidence_threshold").default_f64(),
                    phase_vocoder: p(pn, "phase_vocoder").default_bool(),
                }
            }
            PluginType::ABCompare => {
                let ab = ab_compare_specs::PARAMS;
                Self::ABCompare {
                    mix: p(ab, "mix").default_f64(),
                    mix_mode: p(ab, "mix_mode").default_i32(),
                    selected_path: p(ab, "selected_path").default_i32(),
                    bypass: p(ab, "bypass").default_bool(),
                    auto_gain_enabled: p(ab, "auto_gain_enabled").default_bool(),
                    loudness_type: p(ab, "loudness_type").default_i32(),
                    max_auto_gain_db: p(ab, "max_auto_gain_db").default_f64(),
                    gain_smoothing_ms: p(ab, "gain_smoothing_ms").default_f64(),
                    mix_transition_ms: p(ab, "mix_transition_ms").default_f64(),
                    path_a_config: default_ab_path_config(),
                    path_b_config: default_ab_path_config(),
                    path_a_file: String::new(),
                    path_b_file: String::new(),
                    phase_invert_a: p(ab, "phase_invert_a").default_bool(),
                    phase_invert_b: p(ab, "phase_invert_b").default_bool(),
                    difference_mode: p(ab, "difference_mode").default_bool(),
                    band_mask_low_hz: p(ab, "band_mask_low_hz").default_f64(),
                    band_mask_high_hz: p(ab, "band_mask_high_hz").default_f64(),
                }
            }
            PluginType::Crossover => {
                let co = crossover_specs::PARAMS;
                Self::Crossover {
                    crossover_type: default_crossover_type(),
                    frequency: p(co, "frequency").default_f64(),
                    output: default_crossover_output(),
                    fir_taps: p(co, "fir_taps").default_usize(),
                }
            }
            PluginType::BandSplit => Self::BandSplit {
                channels: default_channels(),
                frequency: default_band_split_frequency(),
                crossover_type: default_band_split_crossover_type(),
            },
            PluginType::BandMerge => Self::BandMerge {
                channels: default_channels(),
                bands: default_band_merge_bands(),
            },
            PluginType::Downmix => {
                let dw = downmix_specs::PARAMS;
                Self::Downmix {
                    input_channels: 6, // Default to 5.1
                    // Unspecified means "adapt to live engine metadata". Ambiguous
                    // widths still require the chain/stream to supply a layout.
                    input_layout: None,
                    center_gain_db: p(dw, "center_gain_db").default_f64(),
                    surround_gain_db: p(dw, "surround_gain_db").default_f64(),
                    height_gain_db: p(dw, "height_gain_db").default_f64(),
                    lfe_gain_db: p(dw, "lfe_gain_db").default_f64(),
                    phase_coherence: p(dw, "phase_coherence").default_bool(),
                    phase_blend_low_hz: p(dw, "phase_blend_low_hz").default_f64(),
                    phase_blend_high_hz: p(dw, "phase_blend_high_hz").default_f64(),
                    itu_mode: p(dw, "itu_mode").default_bool(),
                    matrix_ltrt: p(dw, "matrix_ltrt").default_bool(),
                }
            }
            PluginType::MonoToStereo => {
                let ms = mono_to_stereo_specs::PARAMS;
                Self::MonoToStereo {
                    stereo_width: p(ms, "stereo_width").default_f64(),
                    haas_delay_ms: p(ms, "haas_delay_ms").default_f64(),
                    decor_low_hz: p(ms, "decor_low_hz").default_f64(),
                    decor_high_hz: p(ms, "decor_high_hz").default_f64(),
                    freq_dependent: p(ms, "freq_dependent").default_bool(),
                }
            }
            PluginType::Crossfeed => {
                let cf = crossfeed_specs::PARAMS;
                Self::Crossfeed {
                    mode: CrossfeedMode::Mb,
                    preset: CrossfeedPreset::Default,
                    enabled: true,
                    mix: p(cf, "mix").default_f64(),
                    bauer_fcut_hz: p(cf, "bauer_fcut_hz").default_f64(),
                    bauer_feed_db: p(cf, "bauer_feed_db").default_f64(),
                    meier_level: p(cf, "meier_level").default_f64(),
                    mb_low_freq_hz: p(cf, "mb_low_freq_hz").default_f64(),
                    mb_mid_high_freq_hz: p(cf, "mb_mid_high_freq_hz").default_f64(),
                    mb_low_feed_db: p(cf, "mb_low_feed_db").default_f64(),
                    mb_mid_feed_db: p(cf, "mb_mid_feed_db").default_f64(),
                    mb_high_feed_db: p(cf, "mb_high_feed_db").default_f64(),
                    itd_delay_ms: p(cf, "itd_delay_ms").default_f64(),
                    autogain_enabled: p(cf, "autogain_enabled").default_bool(),
                    autogain_target_lufs: p(cf, "autogain_target_lufs").default_f64(),
                    autogain_max_gain_db: p(cf, "autogain_max_gain_db").default_f64(),
                    autogain_smoothing_ms: p(cf, "autogain_smoothing_ms").default_f64(),
                }
            }
            PluginType::Delay => {
                let d = delay_specs::PARAMS;
                Self::Delay {
                    delay_ms: p(d, "delay_ms").default_f64(),
                    feedback: p(d, "feedback").default_f64(),
                    mix: p(d, "mix").default_f64(),
                    lfo_rate_hz: p(d, "lfo_rate_hz").default_f64(),
                    lfo_depth_ms: p(d, "lfo_depth_ms").default_f64(),
                    allpass_feedback: p(d, "allpass_feedback").default_bool(),
                    allpass_coeff: p(d, "allpass_coeff").default_f64(),
                }
            }
            PluginType::Dither => {
                let d = dither_specs::PARAMS;
                Self::Dither {
                    bit_depth: p(d, "bit_depth").default_usize(),
                    noise_shaping: p(d, "noise_shaping").default_bool(),
                    dither_type: p(d, "dither_type").default_usize(),
                }
            }
            PluginType::Aec => {
                let a = aec_specs::PARAMS;
                Self::Aec {
                    echo_tail_ms: p(a, "echo_tail_ms").default_f64(),
                    step_size: p(a, "step_size").default_f64(),
                    post_filter_enabled: p(a, "post_filter_enabled").default_bool(),
                }
            }
            PluginType::Beamformer => {
                let b = beamformer_specs::PARAMS;
                Self::Beamformer {
                    num_mics: p(b, "num_mics").default_usize(),
                    mic_spacing_cm: p(b, "mic_spacing_cm").default_f64(),
                    steer_angle_deg: p(b, "steer_angle_deg").default_f64(),
                    beamformer_type: p(b, "beamformer_type").default_usize(),
                }
            }
            PluginType::AmbisonicsDecoder => {
                let a = ambisonics_specs::PARAMS;
                Self::AmbisonicsDecoder {
                    order: p(a, "order").default_usize(),
                    target_layout: default_ambisonics_target_layout(),
                    max_re_weighting: p(a, "max_re_weighting").default_bool(),
                    dual_band: p(a, "dual_band").default_bool(),
                }
            }
            PluginType::StereoImager => {
                let si = stereo_imager_specs::PARAMS;
                Self::StereoImager {
                    width: p(si, "width").default_f64(),
                    low_mid_freq: p(si, "low_mid_freq").default_f64(),
                    mid_high_freq: p(si, "mid_high_freq").default_f64(),
                    low_width: p(si, "low_width").default_f64(),
                    mid_width: p(si, "mid_width").default_f64(),
                    high_width: p(si, "high_width").default_f64(),
                    mono_bass: p(si, "mono_bass").default_bool(),
                    mix: p(si, "mix").default_f64(),
                }
            }
            PluginType::DeEsser => {
                let de = de_esser_specs::PARAMS;
                Self::DeEsser {
                    frequency: p(de, "frequency").default_f64(),
                    q: p(de, "q").default_f64(),
                    threshold: p(de, "threshold").default_f64(),
                    ratio: p(de, "ratio").default_f64(),
                    attack: p(de, "attack").default_f64(),
                    release: p(de, "release").default_f64(),
                    mode: default_de_esser_mode(),
                    mix: p(de, "mix").default_f64(),
                }
            }
            PluginType::TransientShaper => {
                let ts = transient_shaper_specs::PARAMS;
                Self::TransientShaper {
                    attack: p(ts, "attack").default_f64(),
                    sustain: p(ts, "sustain").default_f64(),
                    sensitivity_db: p(ts, "sensitivity").default_f64(),
                    output_gain_db: p(ts, "output_gain").default_f64(),
                    mix: p(ts, "mix").default_f64(),
                }
            }
            PluginType::Saturation => {
                let sat = saturation_specs::PARAMS;
                Self::Saturation {
                    mode: p(sat, "mode").default_f64(),
                    drive: p(sat, "drive").default_f64(),
                    tone: p(sat, "tone").default_f64(),
                    exciter_freq: p(sat, "exciter_freq").default_f64(),
                    oversampling: p(sat, "oversampling").default_f64(),
                    output_gain_db: p(sat, "output_gain").default_f64(),
                    mix: p(sat, "mix").default_f64(),
                    dynamic_amount: p(sat, "dynamic_amount").default_f64(),
                    dynamic_attack_ms: p(sat, "dynamic_attack_ms").default_f64(),
                    dynamic_release_ms: p(sat, "dynamic_release_ms").default_f64(),
                    dc_blocker: p(sat, "dc_blocker").default_bool(),
                    use_adaa: p(sat, "use_adaa").default_bool(),
                }
            }
            PluginType::DynamicEq => {
                let dq = dynamic_eq_specs::PARAMS;
                let num_bands = p(dq, "num_bands").default_f64();
                Self::DynamicEq {
                    num_bands,
                    threshold: p(dq, "threshold").default_f64(),
                    ratio: p(dq, "ratio").default_f64(),
                    attack: p(dq, "attack").default_f64(),
                    release: p(dq, "release").default_f64(),
                    knee: p(dq, "knee").default_f64(),
                    link_channels: p(dq, "link_channels").default_bool(),
                    mix: p(dq, "mix").default_f64(),
                    bands: (0..num_bands as usize)
                        .map(|_| DynEqBandParams::default())
                        .collect(),
                }
            }
            PluginType::LinearPhaseEq => {
                let lp = linear_phase_eq_specs::PARAMS;
                let n = p(lp, "num_filters").default_f64() as usize;
                let filters = (0..n)
                    .map(|_| EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0))
                    .collect();
                Self::LinearPhaseEq {
                    num_filters: p(lp, "num_filters").default_f64(),
                    fir_length: p(lp, "fir_length").default_f64(),
                    phase_mode: p(lp, "phase_mode").default_f64(),
                    auto_gain: p(lp, "auto_gain").default_bool(),
                    mix: p(lp, "mix").default_f64(),
                    filters,
                }
            }
            PluginType::SpectralCompressor => {
                let sc = spectral_compressor_specs::PARAMS;
                Self::SpectralCompressor {
                    fft_size: p(sc, "fft_size").default_f64() as usize,
                    threshold: p(sc, "threshold").default_f64(),
                    ratio: p(sc, "ratio").default_f64(),
                    attack: p(sc, "attack").default_f64(),
                    release: p(sc, "release").default_f64(),
                    knee: p(sc, "knee").default_f64(),
                    spectral_smoothing: p(sc, "spectral_smoothing").default_f64(),
                    mix: p(sc, "mix").default_f64(),
                    target_mode: p(sc, "target_mode").default_f64(),
                    delta_listen: false,
                    adaptive_threshold: false,
                    adaptive_offset_db: 0.0,
                }
            }
            PluginType::AAE => {
                let a = aae_specs::PARAMS;
                Self::AAE {
                    speaker_config: p(a, "speaker_config").default_choice_label(),
                    room_size: p(a, "room_size").default_f64(),
                    rt60: p(a, "rt60").default_f64(),
                    bass_ratio: p(a, "bass_ratio").default_f64(),
                    treble_ratio: p(a, "treble_ratio").default_f64(),
                    pre_delay_ms: p(a, "pre_delay_ms").default_f64(),
                    room_preset: p(a, "room_preset").default_choice_label(),
                    dry_level: p(a, "dry_level").default_f64(),
                    er_level: p(a, "er_level").default_f64(),
                    late_level: p(a, "late_level").default_f64(),
                    lfe_level: p(a, "lfe_level").default_f64(),
                    mod_depth: p(a, "mod_depth").default_f64(),
                    er_mod_depth: p(a, "er_mod_depth").default_f64(),
                    input_diffusion: p(a, "input_diffusion").default_f64(),
                    envelopment: p(a, "envelopment").default_f64(),
                    height_amount: p(a, "height_amount").default_f64(),
                    content_aware: p(a, "content_aware").default_bool(),
                    dialogue_attenuation_db: p(a, "dialogue_attenuation_db").default_f64(),
                    safety_limit_db: p(a, "safety_limit_db").default_f64(),
                    auto_gain_enabled: p(a, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(a, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(a, "auto_gain_smoothing_ms").default_f64(),
                    bypass: false,
                    solo_early: false,
                    solo_late: false,
                }
            }
            PluginType::External => {
                return Err("external plugins require concrete discovered settings".to_string());
            }
        };
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plugin_default_setting_round_trips_through_preset_json() {
        for plugin_type in PluginType::all() {
            let settings = PluginSettings::default_for(&plugin_type).unwrap();
            let json = serde_json::to_value(&settings)
                .unwrap_or_else(|error| panic!("{} serialize failed: {error}", plugin_type.name()));
            let restored: PluginSettings =
                serde_json::from_value(json.clone()).unwrap_or_else(|error| {
                    panic!("{} deserialize failed: {error}", plugin_type.name())
                });
            let restored_json = serde_json::to_value(&restored).unwrap();

            let expected_type = if plugin_type == PluginType::FletcherMunson {
                PluginType::LoudnessCompensation
            } else {
                plugin_type.clone()
            };
            assert_eq!(restored.plugin_type(), expected_type);
            assert_eq!(
                restored_json,
                json,
                "{} default settings changed during preset round-trip",
                plugin_type.name()
            );
        }
    }

    #[test]
    fn fir_designer_preset_migrates_to_linear_phase_eq() {
        let legacy = serde_json::json!({
            "FirDesigner": {
                "num_filters": 1.0,
                "fir_length": 2.0,
                "phase_mode": 1.0,
                "auto_gain": true,
                "mix": 0.75,
                "filters": []
            }
        });

        let settings: PluginSettings = serde_json::from_value(legacy).unwrap();
        assert!(matches!(
            settings,
            PluginSettings::LinearPhaseEq {
                phase_mode: 1.0,
                ..
            }
        ));

        let serialized = serde_json::to_value(settings).unwrap();
        assert!(serialized.get("LinearPhaseEq").is_some());
        assert!(serialized.get("FirDesigner").is_none());
    }

    #[test]
    fn upmixer_deserializes_legacy_flat_json() {
        let legacy = serde_json::json!({
            "Upmixer": {
                "speaker_config": "5.1",
                "gain_front_direct": 1.1,
                "gain_front_ambient": 0.6,
                "gain_rear_ambient": 0.4,
                "height_gain": 0.25,
                "stereo_width": 1.2,
                "center_spread": 0.55,
                "surround_direct_bleed": 0.15,
                "rear_late_reflection": 0.25,
                "ambient_boost": 0.05,
                "rear_ambient_boost": 0.05,
                "lfe_cutoff_hz": 90.0,
                "lfe_gain": 0.5,
                "bandpass_hz": 110.0,
                "enable_subharmonic_synth": true,
                "subharmonic_gain": 0.7,
                "subharmonic_freq_hz": 45.0,
                "subharmonic_attack_ms": 12.0,
                "subharmonic_release_ms": 120.0,
                "decorrelation_mode": 1,
                "decorrelation_lfo_rate_hz": 0.6,
                "velvet_noise_duration_ms": 25.0,
                "velvet_noise_density": 0.6,
                "enable_hr_direct": false,
                "hr_sharpen": 0.4,
                "height_hf_cap_hz": 13000.0,
                "height_transient_reduction": 0.4,
                "height_direct_leak": 0.2,
                "safety_cap_db": -0.2,
                "low_latency": true,
                "frequency_resolution": 1024,
                "dialogue_weight": 0.9,
                "voice_freq_min_hz": 220.0,
                "voice_freq_max_hz": 3800.0,
                "dialogue_centroid_weight": 0.4,
                "dialogue_variance_weight": 0.4,
                "dialogue_coherence_weight": 0.4,
                "bypass_decorrelation": true,
                "bypass_transient_detection": false,
                "bypass_all_processing": false,
                "enable_ml_detection": false,
                "multi_source_extraction": false,
                "multi_source_threshold": 0.25,
                "binaural_preview": false,
                "auto_gain_enabled": true,
                "auto_gain_max_db": 10.0,
                "auto_gain_smoothing_ms": 80.0,
            }
        });

        let settings: PluginSettings = serde_json::from_value(legacy).expect("deserialize legacy");
        match settings {
            PluginSettings::Upmixer {
                speaker_config,
                gains,
                output,
                ..
            } => {
                assert_eq!(speaker_config, "5.1");
                assert_eq!(gains.gain_front_direct, 1.1);
                assert!(!output.binaural_preview);
                assert!(output.auto_gain_enabled);
            }
            _ => panic!("expected Upmixer variant"),
        }
    }

    #[test]
    fn upmixer_serde_roundtrip_preserves_flat_keys() {
        let settings = PluginSettings::Upmixer {
            speaker_config: "7.1.4".to_string(),
            gains: UpmixerGainSettings {
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 0.3,
                height_gain: 0.2,
                stereo_width: 1.0,
                center_spread: 0.5,
                surround_direct_bleed: 0.1,
                rear_late_reflection: 0.2,
                ambient_boost: 0.0,
                rear_ambient_boost: 0.0,
            },
            lfe: UpmixerLfeSettings {
                lfe_cutoff_hz: 80.0,
                lfe_gain: 0.0,
                bandpass_hz: 100.0,
            },
            subharmonic: UpmixerSubharmonicSettings {
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.0,
                subharmonic_freq_hz: 40.0,
                subharmonic_attack_ms: 10.0,
                subharmonic_release_ms: 100.0,
            },
            decorrelation: UpmixerDecorrelationSettings {
                decorrelation_mode: 0,
                decorrelation_lfo_rate_hz: 0.5,
                velvet_noise_duration_ms: 20.0,
                velvet_noise_density: 0.5,
            },
            height: UpmixerHeightSettings {
                enable_hr_direct: true,
                hr_sharpen: 0.5,
                height_hf_cap_hz: 12000.0,
                height_transient_reduction: 0.5,
                height_direct_leak: 0.1,
            },
            ambient_analysis: UpmixerAmbientAnalysisSettings {
                low_latency: false,
                frequency_resolution: 2048,
                safety_cap_db: -0.1,
            },
            dialogue: UpmixerDialogueSettings {
                dialogue_weight: 1.0,
                voice_freq_min_hz: 200.0,
                voice_freq_max_hz: 4000.0,
                dialogue_centroid_weight: 0.5,
                dialogue_variance_weight: 0.5,
                dialogue_coherence_weight: 0.5,
            },
            bypass: UpmixerBypassSettings {
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
            },
            output: UpmixerOutputSettings {
                enable_ml_detection: false,
                multi_source_extraction: false,
                multi_source_threshold: 0.3,
                binaural_preview: true,
                auto_gain_enabled: false,
                auto_gain_max_db: 12.0,
                auto_gain_smoothing_ms: 100.0,
            },
        };

        let json = serde_json::to_value(&settings).expect("serialize");
        // PluginSettings is an externally tagged enum, so the variant is the outer key.
        let inner = json
            .get("Upmixer")
            .expect("externally tagged variant object");
        // Flattened sub-structs must produce keys inside the variant object, not nested objects.
        assert!(inner.get("speaker_config").is_some());
        assert!(inner.get("gain_front_direct").is_some());
        assert!(inner.get("binaural_preview").is_some());
        assert!(inner.get("gains").is_none());

        let roundtripped: PluginSettings = serde_json::from_value(json).expect("deserialize");
        match roundtripped {
            PluginSettings::Upmixer {
                speaker_config,
                output:
                    UpmixerOutputSettings {
                        binaural_preview, ..
                    },
                ..
            } => {
                assert_eq!(speaker_config, "7.1.4");
                assert!(binaural_preview);
            }
            _ => panic!("expected Upmixer variant"),
        }
    }
}
