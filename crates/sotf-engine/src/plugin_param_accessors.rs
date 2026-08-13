//! Generic parameter accessor methods for `PluginSettings`.
//!
//! These methods map between parameter indices (as defined by the `PARAMS` arrays
//! in `param_specs`) and the actual fields of each `PluginSettings` variant.
//! Consumer code (TUI, GPUI, CLI) uses these instead of hardcoding per-plugin
//! field access.
//!
//! The `impl_param_accessors!` macro generates `param_specs()`, `layout()`,
//! `param_value()`, and `set_param_value()` from a single field declaration
//! per plugin, eliminating index ordering mismatches between getter and setter.

use crate::plugins::PluginSettings;
use sotf_plugins::param_specs::{self, ParamSpec};
use sotf_plugins::plugin_layout::PluginLayout;

macro_rules! field_to_f64 {
    ($field:expr, f64) => {
        Some(*$field)
    };
    ($field:expr, bool) => {
        Some(b2f(*$field))
    };
    ($field:expr, usize) => {
        Some(*$field as f64)
    };
    ($field:expr, i32) => {
        Some(*$field as f64)
    };
    ($field:expr, f32) => {
        Some(*$field as f64)
    };
    ($field:expr, skip) => {
        None
    };
    ($field:expr, [enum $to:path, $from:path]) => {
        Some($to($field))
    };
    ($field:expr, [str $to:path, $from:path]) => {
        Some($to($field))
    };
}
macro_rules! f64_to_field {
    ($field:expr, $val:ident, f64) => {
        *$field = $val
    };
    ($field:expr, $val:ident, bool) => {
        *$field = f2b($val)
    };
    ($field:expr, $val:ident, usize) => {
        *$field = $val as usize
    };
    ($field:expr, $val:ident, i32) => {
        *$field = $val as i32
    };
    ($field:expr, $val:ident, f32) => {
        *$field = $val as f32
    };
    ($field:expr, $val:ident, skip) => {};
    ($field:expr, $val:ident, [enum $to:path, $from:path]) => {
        *$field = $from($val)
    };
    ($field:expr, $val:ident, [str $to:path, $from:path]) => {
        *$field = $from($val)
    };
}
macro_rules! impl_param_accessors {
    (
        normal: [
            $($Variant:ident {
                params: $params:expr,
                layout: $layout_val:expr,
                fields: [$($field:ident : $ty:tt),* $(,)?]
            }),* $(,)?
        ];
        manual: [
            $($ManualVariant:ident {
                params: $manual_params:expr,
                layout: $manual_layout_val:expr,
                manual: [$manual_get:ident, $manual_set:ident],
                fields: [$($manual_field:ident : $manual_ty:tt),* $(,)?]
            }),* $(,)?
        ];
        no_params_unit: [$($NoParamUnit:ident),* $(,)?];
        no_params_struct: [$($NoParamStruct:ident),* $(,)?]
    ) => {
        // Compile-time assertion: field count must match PARAMS array length.
        // Catches bugs where someone adds a param to PARAMS but forgets the field (or vice versa).
        $(
            const _: () = assert!(
                impl_param_accessors!(@count $($field)*) == $params.len(),
                concat!("PARAMS length mismatch for ", stringify!($Variant),
                    ": fields and param_specs array must have the same number of entries")
            );
        )*
        $(
            const _: () = assert!(
                impl_param_accessors!(@count $($manual_field)*) == $manual_params.len(),
                concat!("PARAMS length mismatch for ", stringify!($ManualVariant),
                    ": fields and param_specs array must have the same number of entries")
            );
        )*

        impl PluginSettings {
            /// Return the static `PARAMS` array for this plugin variant.
            ///
            /// For dynamic-param plugins (EQ, MultibandCompressor, MultibandExpander),
            /// returns only the global params. Per-band params use the `BAND_TEMPLATE`
            /// arrays and are handled separately by consumer code.
            ///
            /// Returns an empty slice for plugins without user-editable params
            /// (LoudnessMonitor, SpectrumAnalyzer, ChannelMuteSolo, Matrix).
            pub fn param_specs(&self) -> &'static [ParamSpec] {
                match self {
                    $(Self::$Variant { .. } => $params,)*
                    $(Self::$ManualVariant { .. } => $manual_params,)*
                    $(Self::$NoParamUnit => &[],)*
                    $(Self::$NoParamStruct { .. } => &[],)*
                }
            }

            /// Return the declarative `PluginLayout` for this variant, if one is defined.
            ///
            /// Returns `None` for plugins that keep custom renderers (EQ, SpectrumAnalyzer,
            /// Matrix, ChannelMuteSolo, LoudnessMonitor) or dynamic-param plugins that
            /// require band selection UI (MultibandCompressor, MultibandExpander).
            pub fn layout(&self) -> Option<&'static PluginLayout> {
                match self {
                    $(Self::$Variant { .. } => $layout_val,)*
                    $(Self::$ManualVariant { .. } => $manual_layout_val,)*
                    $(Self::$NoParamUnit => None,)*
                    $(Self::$NoParamStruct { .. } => None,)*
                }
            }

            /// Read the current value of parameter at `index` as f64.
            ///
            /// Returns `None` if the index is out of range or the parameter is a FilePath type.
            /// Bool parameters are returned as 1.0 (true) or 0.0 (false).
            /// Choice parameters are returned as their numeric index.
            #[allow(unused_variables)]
            pub fn param_value(&self, index: usize) -> Option<f64> {
                match self {
                    $(
                        Self::$Variant { $($field,)* .. } => {
                            impl_param_accessors!(@get index; 0usize; $($field : $ty,)*)
                        }
                    )*
                    $(
                        Self::$ManualVariant { .. } => self.$manual_get(index),
                    )*
                    $(Self::$NoParamUnit => None,)*
                    $(Self::$NoParamStruct { .. } => None,)*
                }
            }

            /// Set the value of parameter at `index` from an f64 value.
            ///
            /// Does nothing for FilePath params, out-of-range indices, or non-editable plugins.
            /// Bool parameters: values > 0.5 are treated as true.
            /// Choice parameters: value is cast to the appropriate integer/enum type.
            #[allow(unused_variables)]
            pub fn set_param_value(&mut self, index: usize, value: f64) {
                match self {
                    $(
                        Self::$Variant { $($field,)* .. } => {
                            impl_param_accessors!(@set index, value; 0usize; $($field : $ty,)*)
                        }
                    )*
                    $(
                        Self::$ManualVariant { .. } => self.$manual_set(index, value),
                    )*
                    $(Self::$NoParamUnit => {},)*
                    $(Self::$NoParamStruct { .. } => {},)*
                }
            }

            /// Adjust parameter at `index` by `delta`, clamping to spec bounds.
            ///
            /// Returns `true` if the parameter was adjusted, `false` if the index
            /// is out of range or the parameter is not adjustable (e.g., FilePath).
            pub fn adjust_param_value(&mut self, index: usize, delta: f64) -> bool {
                let specs = self.param_specs();
                if let Some(spec) = specs.get(index) {
                    if let Some(current) = self.param_value(index) {
                        let new_val = spec.adjust_f64(current, delta);
                        self.set_param_value(index, new_val);
                        return true;
                    }
                }
                false
            }
        }
    };

    // --- Count: counts the number of field tokens ---
    (@count) => { 0usize };
    (@count $head:ident $($rest:ident)*) => { 1usize + impl_param_accessors!(@count $($rest)*) };

    // --- Recursive get: generates if/else chain returning Option<f64> ---
    (@get $idx:ident; $n:expr; ) => { None };
    (@get $idx:ident; $n:expr; $field:ident : $ty:tt, $($rest:ident : $rest_ty:tt,)*) => {
        if $idx == $n {
            field_to_f64!($field, $ty)
        } else {
            impl_param_accessors!(@get $idx; $n + 1usize; $($rest : $rest_ty,)*)
        }
    };

    // --- Recursive set: generates if/else chain returning () ---
    (@set $idx:ident, $val:ident; $n:expr; ) => { () };
    (@set $idx:ident, $val:ident; $n:expr; $field:ident : $ty:tt, $($rest:ident : $rest_ty:tt,)*) => {
        if $idx == $n {
            f64_to_field!($field, $val, $ty);
        } else {
            impl_param_accessors!(@set $idx, $val; $n + 1usize; $($rest : $rest_ty,)*)
        }
    };
}
impl_param_accessors! {
    normal: [
    Gain {
        params: param_specs::gain::PARAMS,
        layout: Some(&param_specs::gain::LAYOUT),
        fields: [gain_db: f64, smoothing_ms: f64]
    },
    Compressor {
        params: param_specs::compressor::PARAMS,
        layout: Some(&param_specs::compressor::SINGLE_BAND_LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            knee_db: f64, makeup_gain_db: f64, mix: f64,
            auto_makeup: bool, link_channels: bool, sidechain_hpf_hz: f64,
            sidechain_hpf_order: [str hpf_order_to_index, index_to_hpf_order],
            detection_mode: [str detection_mode_to_index, index_to_detection_mode],
            lookahead_ms: f64, program_dependent_release: bool, measured_auto_makeup: bool,
            sidechain_external: bool,
        ]
    },
    Gate {
        params: param_specs::gate::PARAMS,
        layout: Some(&param_specs::gate::LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, hold_ms: f64,
            release_ms: f64, mix: f64, link_channels: bool, sidechain_hpf_hz: f64,
            sidechain_hpf_order: [str hpf_order_to_index, index_to_hpf_order],
            detection_mode: [str detection_mode_to_index, index_to_detection_mode],
            sidechain_external: bool,
            range_db: f64, hysteresis_db: f64, knee_db: f64, lookahead_ms: f64,
        ]
    },
    Expander {
        params: param_specs::expander::PARAMS,
        layout: Some(&param_specs::expander::SINGLE_BAND_LAYOUT),
        fields: [
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            range_db: f64, knee_db: f64, hysteresis_db: f64, hold_ms: f64,
            mix: f64, auto_makeup: bool, link_channels: bool, sidechain_hpf_hz: f64,
            lookahead_ms: f64,
            detection_mode: [str detection_mode_to_index, index_to_detection_mode],
            measured_auto_makeup: bool,
        ]
    },
    Limiter {
        params: param_specs::limiter::PARAMS,
        layout: Some(&param_specs::limiter::LAYOUT),
        fields: [
            threshold_db: f64, release_ms: f64, lookahead_ms: f64, soft: bool,
            true_peak: bool, isp_mode: bool, dual_release: bool, mix: f64,
            link_amount: f64, feed_forward: bool,
        ]
    },
    LoudnessCompensation {
        params: param_specs::loudness_compensation::PARAMS,
        layout: Some(&param_specs::loudness_compensation::LAYOUT),
        fields: [
            low_freq: f64, low_gain: f64, high_freq: f64, high_gain: f64,
            mid_enabled: bool, mid_freq: f64, mid_gain: f64, mid_q: f64,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
            mode: usize, playback_level_db: f64, reference_level_db: f64,
            playback_volume_db: f64,
            auto_gain_position: usize, headroom_normalized: bool, auto_calibrated: bool,
        ]
    },
    Convolution {
        params: param_specs::convolution::PARAMS,
        layout: Some(&param_specs::convolution::LAYOUT),
        fields: [ir_file: skip, mix: f64, gain_db: f64, use_nupc: bool, zero_latency_head: bool, head_taps: usize]
    },
    BinauralDecoder {
        params: param_specs::binaural::PARAMS,
        layout: Some(&param_specs::binaural::LAYOUT),
        fields: [
            sofa_file: skip, input_channels: usize,
            externalization: f64, near_field_strength: f64,
            crossfade_mode: usize,
            late_reverb_enabled: bool, late_reverb_mix: f64, late_reverb_rt60: f64,
            late_reverb_damping: f64,
            crossfade_ms: f64, head_yaw_deg: f64, head_pitch_deg: f64, head_roll_deg: f64,
            hrtf_database_dir: skip, head_width_cm: f64, ear_height_cm: f64,
        ]
    },
    XTC {
        params: param_specs::xtc::PARAMS,
        layout: Some(&param_specs::xtc::LAYOUT),
        fields: [
            distance_m: f64, speaker_angle_deg: f64, head_radius_m: f64,
            head_offset_x: f64, head_offset_z: f64, head_yaw_deg: f64,
            head_tracking_smooth_s: f64,
            beta_base: f64, beta_low_freq_boost: f64, beta_high_freq_boost: f64,
            head_shadow_cutoff_hz: f64, head_shadow_slope_db_per_octave: f64,
            max_gain_db: f64, spectral_normalization: bool,
            pinna_model_enabled: bool, room_reflections_enabled: bool,
            room_ir_file: skip,
            room_width_m: f64, room_depth_m: f64, wall_absorption: f64,
            reflection_beta_boost: f64,
            bypass_xtc_filters: bool, bypass_spectral_normalization: bool,
            bypass_neumann_refinement: bool,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
            head_model: f64,
        ]
    },
    Denoiser {
        params: param_specs::denoiser::PARAMS,
        layout: Some(&param_specs::denoiser::LAYOUT),
        fields: [
            reduction_db: f64, floor_db: f64, smoothing: f64, attack_ms: f64, release_ms: f64,
            low_latency: bool, polyphonic_detection: bool,
            mcra_alpha_s: f64, mcra_alpha_p: f64, mcra_l: usize, mcra_delta: f64,
            transparency: f64, dd_enabled: bool, dd_alpha: f64,
            psychoacoustic_masking: bool,
            spectral_smoothing_enabled: bool, temporal_smoothing_enabled: bool,
            spectral_sub_enabled: bool, spectral_sub_alpha: f64, spectral_sub_beta: f64,
            learn_noise: bool, use_captured_profile: bool, clear_profile: bool,
            formant_preservation: bool, formant_strength: f64, multi_resolution: bool,
            harmonic_percussive: bool, spatial_denoise: bool, spatial_strength: f64,
        ]
    },
    Declick {
        params: param_specs::declick::PARAMS,
        layout: Some(&param_specs::declick::LAYOUT),
        fields: [enabled: bool, sensitivity: f64]
    },
    HissReducer {
        params: param_specs::hiss_reducer::PARAMS,
        layout: Some(&param_specs::hiss_reducer::LAYOUT),
        fields: [
            enabled: bool, threshold_db: f64, frequency_hz: f64,
            strength: f64,
        ]
    },
    SpeechDenoiser {
        params: param_specs::speech_denoiser::PARAMS,
        layout: Some(&param_specs::speech_denoiser::LAYOUT),
        fields: [enabled: bool]
    },
    Pnd {
        params: param_specs::pnd::PARAMS,
        layout: Some(&param_specs::pnd::LAYOUT),
        fields: [
            correction_strength: f64, analysis_window_ms: f64, drift_smoothing: f64,
            multi_channel_analysis: bool, confidence_threshold: f64,
            phase_vocoder: bool,
        ]
    },
    ABCompare {
        params: param_specs::ab_compare::PARAMS,
        layout: Some(&param_specs::ab_compare::LAYOUT),
        fields: [
            mix: f64, mix_mode: i32, selected_path: i32, bypass: bool,
            auto_gain_enabled: bool, loudness_type: i32,
            max_auto_gain_db: f64, gain_smoothing_ms: f64, mix_transition_ms: f64,
            path_a_config: skip, path_b_config: skip,
            phase_invert_a: bool, phase_invert_b: bool, difference_mode: bool,
        ]
    },
    Crossover {
        params: param_specs::crossover::PARAMS,
        layout: Some(&param_specs::crossover::LAYOUT),
        fields: [
            crossover_type: [str crossover_plugin_type_to_index, index_to_crossover_plugin_type],
            frequency: f64,
            output: [str crossover_output_to_index, index_to_crossover_output],
            fir_taps: usize,
        ]
    },
    BandSplit {
        params: param_specs::band_split::PARAMS,
        layout: Some(&param_specs::band_split::LAYOUT),
        fields: [frequency: f64, crossover_type: [str crossover_type_to_index, index_to_crossover_type]]
    },
    BandMerge {
        params: param_specs::band_merge::PARAMS,
        layout: Some(&param_specs::band_merge::LAYOUT),
        fields: [bands: usize]
    },
    Downmix {
        params: param_specs::downmix::PARAMS,
        layout: Some(&param_specs::downmix::LAYOUT),
        fields: [
            center_gain_db: f64, surround_gain_db: f64, height_gain_db: f64, lfe_gain_db: f64,
            phase_coherence: bool, phase_blend_low_hz: f64, phase_blend_high_hz: f64,
            itu_mode: bool,
        ]
    },
    MonoToStereo {
        params: param_specs::mono_to_stereo::PARAMS,
        layout: Some(&param_specs::mono_to_stereo::LAYOUT),
        fields: [
            stereo_width: f64, haas_delay_ms: f64, decor_low_hz: f64,
            decor_high_hz: f64, freq_dependent: bool,
        ]
    },
    Crossfeed {
        params: param_specs::crossfeed::PARAMS,
        layout: Some(&param_specs::crossfeed::LAYOUT),
        fields: [
            mode: [enum crossfeed_mode_to_index, index_to_crossfeed_mode],
            preset: [enum crossfeed_preset_to_index, index_to_crossfeed_preset],
            enabled: bool, mix: f64,
            bauer_fcut_hz: f64, bauer_feed_db: f64, meier_level: f64,
            mb_low_freq_hz: f64, mb_mid_high_freq_hz: f64,
            mb_low_feed_db: f64, mb_mid_feed_db: f64, mb_high_feed_db: f64,
            itd_delay_ms: f64,
            autogain_enabled: bool, autogain_target_lufs: f64,
            autogain_max_gain_db: f64, autogain_smoothing_ms: f64,
        ]
    },
    Delay {
        params: param_specs::delay::PARAMS,
        layout: Some(&param_specs::delay::LAYOUT),
        fields: [delay_ms: f64, feedback: f64, mix: f64, lfo_rate_hz: f64, lfo_depth_ms: f64, allpass_coeff: f64, allpass_feedback: bool]
    },
    Aec {
        params: param_specs::aec::PARAMS,
        layout: Some(&param_specs::aec::LAYOUT),
        fields: [echo_tail_ms: f64, step_size: f64, post_filter_enabled: bool]
    },
    Beamformer {
        params: param_specs::beamformer::PARAMS,
        layout: Some(&param_specs::beamformer::LAYOUT),
        fields: [num_mics: usize, mic_spacing_cm: f64, steer_angle_deg: f64, beamformer_type: usize]
    },
    AmbisonicsDecoder {
        params: param_specs::ambisonics::PARAMS,
        layout: Some(&param_specs::ambisonics::LAYOUT),
        fields: [
            order: usize,
            target_layout: [str ambisonics_layout_to_index, index_to_ambisonics_layout],
            max_re_weighting: bool,
            dual_band: bool,
        ]
    },
    EQ {
        params: param_specs::eq::GLOBAL_PARAMS,
        layout: None,
        fields: [max_filters: usize, tdf2: bool, topology: f64]
    },
    MultibandCompressor {
        params: param_specs::multiband_compressor::GLOBAL_PARAMS,
        layout: Some(&param_specs::multiband_compressor::LAYOUT),
        fields: [
            num_bands: usize, crossover_preset: i32,
            crossover_freq_1: f64, crossover_freq_2: f64,
            crossover_freq_3: f64, crossover_freq_4: f64,
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            knee_db: f64, mix: f64, link_channels: bool,
            per_band_lookahead_ms: f64, ms_mode: bool,
            sidechain_tilt_db: f64, link_amount: f64,
        ]
    },
    MultibandExpander {
        params: param_specs::multiband_expander::GLOBAL_PARAMS,
        layout: Some(&param_specs::multiband_expander::LAYOUT),
        fields: [
            num_bands: usize, crossover_preset: i32,
            crossover_freq_1: f64, crossover_freq_2: f64,
            crossover_freq_3: f64, crossover_freq_4: f64,
            threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64,
            range_db: f64, knee_db: f64, hysteresis_db: f64, hold_ms: f64,
            mix: f64, link_channels: bool,
            detection_mode: [str detection_mode_to_index, index_to_detection_mode],
            lookahead_ms: f64,
        ]
    },
    ChannelMuteSolo {
        params: param_specs::channel_mute_solo::PARAMS,
        layout: None,
        fields: [enabled: bool, dim_gain_db: f64, fade_ms: f64]
    },
    StereoImager {
        params: param_specs::stereo_imager::PARAMS,
        layout: Some(&param_specs::stereo_imager::LAYOUT),
        fields: [
            width: f64, low_mid_freq: f64, mid_high_freq: f64,
            low_width: f64, mid_width: f64, high_width: f64,
            mono_bass: bool, mix: f64,
        ]
    },
    DeEsser {
        params: param_specs::de_esser::PARAMS,
        layout: Some(&param_specs::de_esser::LAYOUT),
        fields: [
            frequency: f64, q: f64, threshold: f64, ratio: f64,
            attack: f64, release: f64,
            mode: [str de_esser_mode_to_index, index_to_de_esser_mode],
            mix: f64,
        ]
    },
    TransientShaper {
        params: param_specs::transient_shaper::PARAMS,
        layout: Some(&param_specs::transient_shaper::LAYOUT),
        fields: [
            attack: f64, sustain: f64, sensitivity_db: f64,
            output_gain_db: f64, mix: f64,
        ]
    },
    Saturation {
        params: param_specs::saturation::PARAMS,
        layout: Some(&param_specs::saturation::LAYOUT),
        fields: [
            mode: f64, drive: f64, tone: f64,
            exciter_freq: f64, oversampling: f64,
            output_gain_db: f64, mix: f64,
            dynamic_amount: f64, dynamic_attack_ms: f64, dynamic_release_ms: f64,
            dc_blocker: bool, use_adaa: bool,
        ]
    },
    DynamicEq {
        params: param_specs::dynamic_eq::PARAMS,
        layout: Some(&param_specs::dynamic_eq::LAYOUT),
        fields: [
            num_bands: f64, threshold: f64, ratio: f64,
            attack: f64, release: f64, knee: f64,
            link_channels: bool, mix: f64,
        ]
    },
    SpectralCompressor {
        params: param_specs::spectral_compressor::PARAMS,
        layout: Some(&param_specs::spectral_compressor::LAYOUT),
        fields: [
            fft_size: usize, threshold: f64, ratio: f64,
            attack: f64, release: f64, knee: f64,
            spectral_smoothing: f64, mix: f64,
            target_mode: f64, delta_listen: bool,
            adaptive_threshold: bool, adaptive_offset_db: f64,
        ]
    },
    LinearPhaseEq {
        params: param_specs::linear_phase_eq::PARAMS,
        layout: Some(&param_specs::linear_phase_eq::LAYOUT),
        fields: [
            num_filters: f64, fir_length: f64,
            phase_mode: f64, auto_gain: bool, mix: f64,
        ]
    },
    AAE {
        params: param_specs::aae::PARAMS,
        layout: Some(&param_specs::aae::LAYOUT),
        fields: [
            speaker_config: [str aae_speaker_config_to_index, index_to_aae_speaker_config],
            room_size: f64, rt60: f64, bass_ratio: f64, treble_ratio: f64,
            pre_delay_ms: f64,
            room_preset: [str aae_room_preset_to_index, index_to_aae_room_preset],
            dry_level: f64, er_level: f64, late_level: f64, lfe_level: f64,
            mod_depth: f64, er_mod_depth: f64, input_diffusion: f64,
            envelopment: f64, height_amount: f64,
            content_aware: bool, dialogue_attenuation_db: f64, safety_limit_db: f64,
            auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
            bypass: bool, solo_early: bool, solo_late: bool,
        ]
    },
    SpectrumAnalyzer {
        params: param_specs::spectrum::PARAMS,
        layout: None,
        fields: [
            num_bins: usize,
            min_freq: f32,
            max_freq: f32,
            smoothing: f32,
            tilt_correction: [enum spectral_tilt_to_index, index_to_spectral_tilt],
            tilt_reference: [enum tilt_reference_to_index, index_to_tilt_reference],
        ]
    }
    ];
    manual: [
        Upmixer {
            params: param_specs::upmixer::PARAMS,
            layout: Some(&param_specs::upmixer::LAYOUT),
            manual: [upmixer_param_value, upmixer_set_param_value],
            fields: [
                speaker_config: [str speaker_config_to_index, index_to_speaker_config],
                gain_front_direct: f64, gain_front_ambient: f64, gain_rear_ambient: f64,
                height_gain: f64, lfe_gain: f64, lfe_cutoff_hz: f64,
                enable_subharmonic_synth: bool, subharmonic_gain: f64, subharmonic_freq_hz: f64,
                subharmonic_attack_ms: f64, subharmonic_release_ms: f64,
                stereo_width: f64, center_spread: f64, bandpass_hz: f64,
                enable_hr_direct: bool, hr_sharpen: f64, ambient_boost: f64,
                decorrelation_mode: usize, decorrelation_lfo_rate_hz: f64,
                velvet_noise_duration_ms: f64, velvet_noise_density: f64,
                height_hf_cap_hz: f64, height_transient_reduction: f64, height_direct_leak: f64,
                surround_direct_bleed: f64, rear_ambient_boost: f64, rear_late_reflection: f64,
                dialogue_weight: f64, voice_freq_min_hz: f64, voice_freq_max_hz: f64,
                dialogue_centroid_weight: f64, dialogue_variance_weight: f64, dialogue_coherence_weight: f64,
                safety_cap_db: f64,
                low_latency: bool, frequency_resolution: usize,
                bypass_decorrelation: bool, bypass_transient_detection: bool,
                bypass_all_processing: bool, enable_ml_detection: bool,
                multi_source_extraction: bool, multi_source_threshold: f64,
                binaural_preview: bool,
                auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
            ]
        },
        FletcherMunson {
            params: param_specs::loudness_compensation::PARAMS,
            layout: Some(&param_specs::loudness_compensation::LAYOUT),
            manual: [fletcher_munson_param_value, fletcher_munson_set_param_value],
            fields: [
                low_freq: f64, low_gain: f64, high_freq: f64, high_gain: f64,
                mid_enabled: bool, mid_freq: f64, mid_gain: f64, mid_q: f64,
                auto_gain_enabled: bool, auto_gain_max_db: f64, auto_gain_smoothing_ms: f64,
                mode: usize, playback_level_db: f64, reference_level_db: f64,
                playback_volume_db: f64,
            ]
        }
    ];
    no_params_unit: [LoudnessMonitor];
    no_params_struct: [Matrix, External]
}

mod aae;
// Manual param accessors for Upmixer, whose variant fields are split into
// serde-flattened sub-structs. Keeping these in one place preserves the
// index↔field mapping from the old monolithic variant.
impl PluginSettings {
    fn fletcher_munson_param_value(&self, index: usize) -> Option<f64> {
        let Self::FletcherMunson {
            playback_volume_db,
            reference_level_db,
            enabled,
            band1_freq,
            band1_max_gain,
            band2_q,
            band2_max_gain: _,
            band3_freq,
            band3_q,
            band3_max_gain,
            band4_freq,
            band4_max_gain,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            iso_226,
            ..
        } = self
        else {
            return None;
        };
        match index {
            0 => Some(*band1_freq),
            1 => Some(*band1_max_gain),
            2 => Some(*band4_freq),
            3 => Some(*band4_max_gain),
            4 => Some(b2f(*enabled)),
            5 => Some(*band3_freq),
            6 => Some(*band3_max_gain),
            7 => Some((*band3_q).max(*band2_q)),
            8 => Some(b2f(*auto_gain_enabled)),
            9 => Some(*auto_gain_max_db),
            10 => Some(*auto_gain_smoothing_ms),
            11 => Some(if *auto_gain_enabled {
                2.0
            } else if *iso_226 {
                1.0
            } else {
                0.0
            }),
            12 => Some(70.0),
            13 => Some(*reference_level_db),
            14 => Some(*playback_volume_db),
            _ => None,
        }
    }

    fn fletcher_munson_set_param_value(&mut self, index: usize, value: f64) {
        let Self::FletcherMunson {
            reference_level_db,
            enabled,
            band1_freq,
            band1_max_gain,
            band3_freq,
            band3_q,
            band3_max_gain,
            band4_freq,
            band4_max_gain,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            iso_226,
            ..
        } = self
        else {
            return;
        };
        let specs = param_specs::loudness_compensation::PARAMS;
        match index {
            0 => *band1_freq = specs[0].clamp_f64(value),
            1 => *band1_max_gain = specs[1].clamp_f64(value),
            2 => *band4_freq = specs[2].clamp_f64(value),
            3 => *band4_max_gain = specs[3].clamp_f64(value),
            4 => *enabled = f2b(value),
            5 => *band3_freq = specs[5].clamp_f64(value),
            6 => *band3_max_gain = specs[6].clamp_f64(value),
            7 => *band3_q = specs[7].clamp_f64(value),
            8 => *auto_gain_enabled = f2b(value),
            9 => *auto_gain_max_db = specs[9].clamp_f64(value),
            10 => *auto_gain_smoothing_ms = specs[10].clamp_f64(value),
            11 => {
                let mode = value as usize;
                *iso_226 = mode == 1;
                *auto_gain_enabled = mode == 2;
            }
            13 => *reference_level_db = specs[13].clamp_f64(value),
            _ => {}
        }
    }

    fn upmixer_param_value(&self, index: usize) -> Option<f64> {
        let Self::Upmixer {
            speaker_config,
            gains,
            lfe,
            subharmonic,
            decorrelation,
            height,
            ambient_analysis,
            dialogue,
            bypass,
            output,
            ..
        } = self
        else {
            return None;
        };
        match index {
            0 => Some(speaker_config_to_index(speaker_config)),
            1 => Some(gains.gain_front_direct),
            2 => Some(gains.gain_front_ambient),
            3 => Some(gains.gain_rear_ambient),
            4 => Some(gains.height_gain),
            5 => Some(lfe.lfe_gain),
            6 => Some(lfe.lfe_cutoff_hz),
            7 => Some(b2f(subharmonic.enable_subharmonic_synth)),
            8 => Some(subharmonic.subharmonic_gain),
            9 => Some(subharmonic.subharmonic_freq_hz),
            10 => Some(subharmonic.subharmonic_attack_ms),
            11 => Some(subharmonic.subharmonic_release_ms),
            12 => Some(gains.stereo_width),
            13 => Some(gains.center_spread),
            14 => Some(lfe.bandpass_hz),
            15 => Some(b2f(height.enable_hr_direct)),
            16 => Some(height.hr_sharpen),
            17 => Some(gains.ambient_boost),
            18 => Some(decorrelation.decorrelation_mode as f64),
            19 => Some(decorrelation.decorrelation_lfo_rate_hz),
            20 => Some(decorrelation.velvet_noise_duration_ms),
            21 => Some(decorrelation.velvet_noise_density),
            22 => Some(height.height_hf_cap_hz),
            23 => Some(height.height_transient_reduction),
            24 => Some(height.height_direct_leak),
            25 => Some(gains.surround_direct_bleed),
            26 => Some(gains.rear_ambient_boost),
            27 => Some(gains.rear_late_reflection),
            28 => Some(dialogue.dialogue_weight),
            29 => Some(dialogue.voice_freq_min_hz),
            30 => Some(dialogue.voice_freq_max_hz),
            31 => Some(dialogue.dialogue_centroid_weight),
            32 => Some(dialogue.dialogue_variance_weight),
            33 => Some(dialogue.dialogue_coherence_weight),
            34 => Some(ambient_analysis.safety_cap_db),
            35 => Some(b2f(ambient_analysis.low_latency)),
            36 => Some(ambient_analysis.frequency_resolution as f64),
            37 => Some(b2f(bypass.bypass_decorrelation)),
            38 => Some(b2f(bypass.bypass_transient_detection)),
            39 => Some(b2f(bypass.bypass_all_processing)),
            40 => Some(b2f(output.enable_ml_detection)),
            41 => Some(b2f(output.multi_source_extraction)),
            42 => Some(output.multi_source_threshold),
            43 => Some(b2f(output.binaural_preview)),
            44 => Some(b2f(output.auto_gain_enabled)),
            45 => Some(output.auto_gain_max_db),
            46 => Some(output.auto_gain_smoothing_ms),
            _ => None,
        }
    }

    fn upmixer_set_param_value(&mut self, index: usize, value: f64) {
        let Self::Upmixer {
            speaker_config,
            gains,
            lfe,
            subharmonic,
            decorrelation,
            height,
            ambient_analysis,
            dialogue,
            bypass,
            output,
            ..
        } = self
        else {
            return;
        };
        match index {
            0 => *speaker_config = index_to_speaker_config(value),
            1 => gains.gain_front_direct = value,
            2 => gains.gain_front_ambient = value,
            3 => gains.gain_rear_ambient = value,
            4 => gains.height_gain = value,
            5 => lfe.lfe_gain = value,
            6 => lfe.lfe_cutoff_hz = value,
            7 => subharmonic.enable_subharmonic_synth = f2b(value),
            8 => subharmonic.subharmonic_gain = value,
            9 => subharmonic.subharmonic_freq_hz = value,
            10 => subharmonic.subharmonic_attack_ms = value,
            11 => subharmonic.subharmonic_release_ms = value,
            12 => gains.stereo_width = value,
            13 => gains.center_spread = value,
            14 => lfe.bandpass_hz = value,
            15 => height.enable_hr_direct = f2b(value),
            16 => height.hr_sharpen = value,
            17 => gains.ambient_boost = value,
            18 => decorrelation.decorrelation_mode = value as usize,
            19 => decorrelation.decorrelation_lfo_rate_hz = value,
            20 => decorrelation.velvet_noise_duration_ms = value,
            21 => decorrelation.velvet_noise_density = value,
            22 => height.height_hf_cap_hz = value,
            23 => height.height_transient_reduction = value,
            24 => height.height_direct_leak = value,
            25 => gains.surround_direct_bleed = value,
            26 => gains.rear_ambient_boost = value,
            27 => gains.rear_late_reflection = value,
            28 => dialogue.dialogue_weight = value,
            29 => dialogue.voice_freq_min_hz = value,
            30 => dialogue.voice_freq_max_hz = value,
            31 => dialogue.dialogue_centroid_weight = value,
            32 => dialogue.dialogue_variance_weight = value,
            33 => dialogue.dialogue_coherence_weight = value,
            34 => ambient_analysis.safety_cap_db = value,
            35 => ambient_analysis.low_latency = f2b(value),
            36 => ambient_analysis.frequency_resolution = value as usize,
            37 => bypass.bypass_decorrelation = f2b(value),
            38 => bypass.bypass_transient_detection = f2b(value),
            39 => bypass.bypass_all_processing = f2b(value),
            40 => output.enable_ml_detection = f2b(value),
            41 => output.multi_source_extraction = f2b(value),
            42 => output.multi_source_threshold = value,
            43 => output.binaural_preview = f2b(value),
            44 => output.auto_gain_enabled = f2b(value),
            45 => output.auto_gain_max_db = value,
            46 => output.auto_gain_smoothing_ms = value,
            _ => {}
        }
    }
}

mod ambisonics;
mod crossfeed;
mod crossover;
mod de;
mod detection;
mod hpf;
mod index;
mod misc;
mod speaker;
#[cfg(test)]
mod tests;

use aae::aae_room_preset_to_index;
use aae::aae_speaker_config_to_index;
use ambisonics::ambisonics_layout_to_index;
use crossfeed::crossfeed_mode_to_index;
use crossfeed::crossfeed_preset_to_index;
use crossover::crossover_output_to_index;
use crossover::crossover_plugin_type_to_index;
use crossover::crossover_type_to_index;
use crossover::index_to_crossover_plugin_type;
use de::de_esser_mode_to_index;
use detection::detection_mode_to_index;
use hpf::hpf_order_to_index;
use index::index_to_aae_room_preset;
use index::index_to_aae_speaker_config;
use index::index_to_ambisonics_layout;
use index::index_to_crossfeed_mode;
use index::index_to_crossfeed_preset;
use index::index_to_crossover_output;
use index::index_to_crossover_type;
use index::index_to_de_esser_mode;
use index::index_to_detection_mode;
use index::index_to_hpf_order;
use index::index_to_speaker_config;
use index::index_to_spectral_tilt;
use index::index_to_tilt_reference;
use misc::b2f;
use misc::f2b;
use misc::spectral_tilt_to_index;
use misc::tilt_reference_to_index;
use speaker::speaker_config_to_index;
