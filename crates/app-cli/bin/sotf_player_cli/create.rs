use super::misc::get_speaker_config_channels;
use super::parse::parse_crossfeed_mode;
use super::parse::parse_crossfeed_preset;
use super::types::ABCompareArgs;
use super::types::AaeArgs;
use super::types::BandMergeArgs;
use super::types::BandSplitArgs;
use super::types::BinauralArgs;
use super::types::CompressorArgs;
use super::types::ConvolutionArgs;
use super::types::CrossfeedArgs;
use super::types::DenoiserArgs;
use super::types::DownmixArgs;
use super::types::ExpanderArgs;
use super::types::FletcherMunsonArgs;
use super::types::GainArgs;
use super::types::GateArgs;
use super::types::LimiterArgs;
use super::types::MatrixArgs;
use super::types::MonoToStereoArgs;
use super::types::MultibandCompressorArgs;
use super::types::MultibandExpanderArgs;
use super::types::PndArgs;
use super::types::SpectrumAnalyzerArgs;
use super::types::UpmixerArgs;
use super::types::XtcArgs;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_audio::LoudnessCompensation;
use sotf_audio::PluginConfig;

pub(super) fn create_upmixer_plugin_config(args: &UpmixerArgs) -> Result<PluginConfig, String> {
    use serde_json::json;

    if !args.fft_size.is_power_of_two() {
        return Err(format!(
            "Upmixer FFT size must be power of 2, got {}",
            args.fft_size
        ));
    }
    let _ = get_speaker_config_channels(&args.config)?;

    let parameters = json!({
        "speaker_config": args.config,
        "fft_size": args.fft_size,
        "gain_front_direct": args.gain_front_direct,
        "gain_front_ambient": args.gain_front_ambient,
        "gain_rear_ambient": args.gain_rear_ambient,
        "lfe_cutoff_hz": args.lfe_cutoff_hz,
        "stereo_width": args.stereo_width,
        "bandpass_hz": args.bandpass_hz,
        "height_gain": args.height_gain,
        "lfe_gain": args.lfe_gain,
        "enable_subharmonic_synth": args.subharmonic,
        "subharmonic_gain": args.subharmonic_gain,
        "enable_hr_direct": args.hr_direct,
        "hr_sharpen": args.hr_sharpen,
        "safety_cap_db": args.safety_cap_db,
        "center_spread": args.center_spread,
        "surround_direct_bleed": args.surround_direct_bleed,
        "rear_late_reflection": args.rear_late_reflection,
        "subharmonic_freq_hz": args.subharmonic_freq_hz,
        "subharmonic_attack_ms": args.subharmonic_attack_ms,
        "subharmonic_release_ms": args.subharmonic_release_ms,
        "decorrelation_mode": args.decorrelation_mode,
        "decorrelation_lfo_rate_hz": args.decorrelation_lfo_rate_hz,
        "velvet_noise_duration_ms": args.velvet_noise_duration_ms,
        "velvet_noise_density": args.velvet_noise_density,
        "height_hf_cap_hz": args.height_hf_cap_hz,
        "height_transient_reduction": args.height_transient_reduction,
        "height_direct_leak": args.height_direct_leak,
        "ambient_boost": args.ambient_boost,
        "rear_ambient_boost": args.rear_ambient_boost,
        "dialogue_weight": args.dialogue_weight,
        "voice_freq_min_hz": args.voice_freq_min_hz,
        "voice_freq_max_hz": args.voice_freq_max_hz,
        "dialogue_centroid_weight": args.dialogue_centroid_weight,
        "dialogue_variance_weight": args.dialogue_variance_weight,
        "dialogue_coherence_weight": args.dialogue_coherence_weight,
        "bypass_decorrelation": args.bypass_decorrelation,
        "bypass_transient_detection": args.bypass_transient_detection,
        "bypass_all_processing": args.bypass_all_processing,
        "enable_ml_detection": args.enable_ml_detection,
        "low_latency": args.low_latency,
    });

    Ok(PluginConfig {
        plugin_type: "upmixer".to_string(),
        parameters,
    })
}

pub(super) fn create_loudness_compensation_plugin_config(
    lc: &LoudnessCompensation,
    auto_gain_params: (bool, f32, f32),
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let (auto_gain_enabled, auto_gain_max_db, auto_gain_smoothing_ms) = auto_gain_params;
    let parameters = json!({
        "low_freq": 100.0,
        "low_gain": lc.low_boost,
        "high_freq": 10000.0,
        "high_gain": lc.high_boost,
        "auto_gain_enabled": auto_gain_enabled,
        "auto_gain_max_db": auto_gain_max_db,
        "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
    });

    Ok(PluginConfig {
        plugin_type: "loudness_compensation".to_string(),
        parameters,
    })
}

pub(super) fn create_aae_plugin_config(args: &AaeArgs) -> Result<PluginConfig, String> {
    use serde_json::json;

    let _ = get_speaker_config_channels(&args.config)?;

    let parameters = json!({
        "speaker_config": args.config,
        "room_preset": args.room_preset,
        "room_size": args.room_size,
        "rt60": args.rt60,
        "dry_level": args.dry_level,
        "er_level": args.er_level,
        "late_level": args.late_level,
        "pre_delay_ms": args.pre_delay_ms,
        "mod_depth": args.mod_depth,
        "auto_gain_enabled": args.auto_gain_enabled,
        "auto_gain_max_db": args.auto_gain_max_db,
        "auto_gain_smoothing_ms": args.auto_gain_smoothing_ms,
    });

    Ok(PluginConfig {
        plugin_type: "aae".to_string(),
        parameters,
    })
}

pub(super) fn create_binaural_decoder_plugin_config(
    args: &BinauralArgs,
    input_channels: usize,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    if !args.fft_size.is_power_of_two() {
        return Err(format!(
            "Binaural decoder FFT size must be power of 2, got {}",
            args.fft_size
        ));
    }

    let sofa_path = args
        .sofa_file
        .as_ref()
        .ok_or("Binaural decoder requires --sofa-file to be specified")?;

    if !sofa_path.exists() {
        return Err(format!("SOFA file does not exist: {:?}", sofa_path));
    }

    let parameters = json!({
        "sofa_file": sofa_path.to_string_lossy().to_string(),
        "input_channels": input_channels,
        "fft_size": args.fft_size,
        "externalization": args.externalization,
        "near_field_strength": args.near_field,
    });

    Ok(PluginConfig {
        plugin_type: "binaural_decoder".to_string(),
        parameters,
    })
}

pub(super) fn create_loudness_analyzer_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: json!({}),
    })
}

pub(super) fn create_gain_plugin_config(args: &GainArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": args.gain_db,
            "smoothing_ms": args.smoothing_ms,
        }),
    })
}

pub(super) fn create_compressor_plugin_config(
    args: &CompressorArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "compressor".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "knee_db": args.knee_db,
            "makeup_gain_db": args.makeup_gain_db,
            "mix": args.mix,
            "auto_makeup": args.auto_makeup,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
            "detection_mode": args.detection_mode,
            "lookahead_ms": args.lookahead_ms,
            "program_dependent_release": args.program_dependent_release,
            "measured_auto_makeup": args.measured_auto_makeup,
        }),
    })
}

pub(super) fn create_gate_plugin_config(args: &GateArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "gate".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "hold_ms": args.hold_ms,
            "release_ms": args.release_ms,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
            "range_db": args.range_db,
            "hysteresis_db": args.hysteresis_db,
            "knee_db": args.knee_db,
            "lookahead_ms": args.lookahead_ms,
        }),
    })
}

pub(super) fn create_limiter_plugin_config(args: &LimiterArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "limiter".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "release_ms": args.release_ms,
            "lookahead_ms": args.lookahead_ms,
            "soft": args.soft,
            "mix": args.mix,
            "true_peak": args.true_peak,
            "dual_release": args.dual_release,
        }),
    })
}

pub(super) fn create_expander_plugin_config(args: &ExpanderArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "expander".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "range_db": args.range_db,
            "knee_db": args.knee_db,
            "hysteresis_db": args.hysteresis_db,
            "hold_ms": args.hold_ms,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
            "lookahead_ms": args.lookahead_ms,
            "detection_mode": args.detection_mode,
            "measured_auto_makeup": args.measured_auto_makeup,
        }),
    })
}

pub(super) fn create_multiband_compressor_plugin_config(
    args: &MultibandCompressorArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "multiband_compressor".to_string(),
        parameters: json!({
            "num_bands": args.num_bands,
            "crossover_preset": args.crossover_preset,
            "crossover_freq_1": args.crossover_freq_1,
            "crossover_freq_2": args.crossover_freq_2,
            "crossover_freq_3": args.crossover_freq_3,
            "crossover_freq_4": args.crossover_freq_4,
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "knee_db": args.knee_db,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
        }),
    })
}

pub(super) fn create_multiband_expander_plugin_config(
    args: &MultibandExpanderArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "multiband_expander".to_string(),
        parameters: json!({
            "num_bands": args.num_bands,
            "crossover_preset": args.crossover_preset,
            "crossover_freq_1": args.crossover_freq_1,
            "crossover_freq_2": args.crossover_freq_2,
            "crossover_freq_3": args.crossover_freq_3,
            "crossover_freq_4": args.crossover_freq_4,
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "range_db": args.range_db,
            "knee_db": args.knee_db,
            "hysteresis_db": args.hysteresis_db,
            "hold_ms": args.hold_ms,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
        }),
    })
}

pub(super) fn create_xtc_plugin_config(args: &XtcArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "xtc".to_string(),
        parameters: json!({
            "distance_m": args.distance_m,
            "speaker_angle_deg": args.speaker_angle_deg,
            "head_radius_m": args.head_radius_m,
            "beta_base": args.beta_base,
            "beta_low_freq_boost": args.beta_low_freq_boost,
            "beta_high_freq_boost": args.beta_high_freq_boost,
            "head_shadow_cutoff_hz": args.head_shadow_cutoff_hz,
            "head_shadow_slope_db_per_octave": args.head_shadow_slope,
            "max_gain_db": args.max_gain_db,
            "head_offset_x": args.head_offset_x,
            "head_offset_z": args.head_offset_z,
            "head_yaw_deg": args.head_yaw_deg,
            "head_tracking_smooth_s": args.head_tracking_smooth_s,
            "spectral_normalization": args.spectral_normalization,
            "room_reflections_enabled": args.room_reflections,
            "room_ir_file": args.room_ir_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "room_width_m": args.room_width_m,
            "room_depth_m": args.room_depth_m,
            "wall_absorption": args.wall_absorption,
            "reflection_beta_boost": args.reflection_beta_boost,
            "bypass_xtc_filters": args.bypass_filters,
            "bypass_spectral_normalization": args.bypass_spectral_normalization,
            "bypass_neumann_refinement": args.bypass_neumann_refinement,
            "auto_gain_enabled": args.auto_gain,
            "auto_gain_max_db": args.auto_gain_max_db,
            "auto_gain_smoothing_ms": args.auto_gain_smoothing_ms,
            "pinna_model_enabled": args.pinna_model,
        }),
    })
}

pub(super) fn create_denoiser_plugin_config(args: &DenoiserArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "denoiser".to_string(),
        parameters: json!({
            "reduction_db": args.reduction_db,
            "floor_db": args.floor_db,
            "smoothing": args.smoothing,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "low_latency": args.low_latency,
            "polyphonic_detection": args.polyphonic_detection,
            "mcra_alpha_s": args.mcra_alpha_s,
            "mcra_alpha_p": args.mcra_alpha_p,
            "mcra_l": args.mcra_l,
            "mcra_delta": args.mcra_delta,
            "transparency": args.transparency,
            "dd_enabled": args.dd_enabled,
            "dd_alpha": args.dd_alpha,
            "psychoacoustic_masking": args.psychoacoustic_masking,
            "spectral_smoothing_enabled": args.spectral_smoothing_enabled,
            "temporal_smoothing_enabled": args.temporal_smoothing_enabled,
            "spectral_sub_enabled": args.spectral_sub_enabled,
            "spectral_sub_alpha": args.spectral_sub_alpha,
            "spectral_sub_beta": args.spectral_sub_beta,
            "learn_noise": args.learn_noise,
            "use_captured_profile": args.use_captured_profile,
            "clear_profile": args.clear_profile,
        }),
    })
}

pub(super) fn create_pnd_plugin_config(args: &PndArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "pnd".to_string(),
        parameters: json!({
            "correction_strength": args.correction_strength,
            "analysis_window_ms": args.analysis_window_ms,
            "drift_smoothing": args.drift_smoothing,
        }),
    })
}

pub(super) fn create_fletcher_munson_plugin_config(
    args: &FletcherMunsonArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    // Fletcher-Munson merged into LoudnessCompensation Auto mode
    Ok(PluginConfig {
        plugin_type: "loudness_compensation".to_string(),
        parameters: json!({
            "mode": 2,
            "playback_volume_db": 0.0,
            "reference_level_db": 83.0 + args.reference_level_db as f64,
        }),
    })
}

pub(super) fn create_convolution_plugin_config(
    args: &ConvolutionArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let ir_path = args
        .ir_file
        .as_ref()
        .ok_or("Convolution plugin requires --convolution-ir-file to be specified")?;

    if !ir_path.exists() {
        return Err(format!(
            "Impulse response file does not exist: {:?}",
            ir_path
        ));
    }

    Ok(PluginConfig {
        plugin_type: "convolution".to_string(),
        parameters: json!({
            "ir_file": ir_path.to_string_lossy().to_string(),
            "mix": args.mix,
            "gain_db": args.gain_db,
        }),
    })
}

pub(super) fn create_spectrum_analyzer_plugin_config(
    args: &SpectrumAnalyzerArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "spectrum_analyzer".to_string(),
        parameters: json!({
            "num_bins": args.num_bins,
            "min_freq": args.min_freq,
            "max_freq": args.max_freq,
            "smoothing": args.smoothing,
        }),
    })
}

pub(super) fn create_channel_mute_solo_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "channel_mute_solo".to_string(),
        parameters: json!({
            "enabled": true,
        }),
    })
}

pub(super) fn create_ab_compare_plugin_config(
    args: &ABCompareArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "ab_compare".to_string(),
        parameters: json!({
            "auto_gain_enabled": args.auto_gain,
            "bypass": args.bypass,
        }),
    })
}

pub(super) fn create_band_split_plugin_config(
    args: &BandSplitArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "band_split".to_string(),
        parameters: json!({
            "frequency": args.frequency,
            "type": args.crossover_type,
        }),
    })
}

pub(super) fn create_band_merge_plugin_config(
    args: &BandMergeArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "band_merge".to_string(),
        parameters: json!({
            "bands": args.bands,
        }),
    })
}

pub(super) fn create_downmix_plugin_config(
    args: &DownmixArgs,
    input_channels: usize,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "downmix".to_string(),
        parameters: json!({
            "input_channels": input_channels,
            "center_gain_db": args.center_gain_db,
            "surround_gain_db": args.surround_gain_db,
            "height_gain_db": args.height_gain_db,
            "lfe_gain_db": args.lfe_gain_db,
            "phase_coherence": args.phase_coherence,
            "phase_blend_low_hz": args.phase_blend_low_hz,
            "phase_blend_high_hz": args.phase_blend_high_hz,
            "itu_mode": args.itu_mode,
        }),
    })
}

pub(super) fn create_mono_to_stereo_plugin_config(
    args: &MonoToStereoArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "mono_to_stereo".to_string(),
        parameters: json!({
            "stereo_width": args.stereo_width,
            "haas_delay_ms": args.haas_delay_ms,
            "decor_low_hz": args.decor_low_hz,
            "decor_high_hz": args.decor_high_hz,
            "freq_dependent": args.enable_comp_eq,
        }),
    })
}

pub(super) fn create_crossfeed_plugin_config(args: &CrossfeedArgs) -> Result<PluginConfig, String> {
    use serde_json::json;

    let mode = parse_crossfeed_mode(&args.mode)?;
    let preset = parse_crossfeed_preset(&args.preset)?;

    Ok(PluginConfig {
        plugin_type: "crossfeed".to_string(),
        parameters: json!({
            "mode": mode,
            "preset": preset,
            "enabled": true,
            "mix": args.mix,
            "bauer_fcut_hz": args.bauer_fcut_hz,
            "bauer_feed_db": args.bauer_feed_db,
            "meier_level": args.meier_level,
            "mb_low_freq_hz": args.mb_low_freq_hz,
            "mb_mid_high_freq_hz": args.mb_mid_high_freq_hz,
            "mb_low_feed_db": args.mb_low_feed_db,
            "mb_mid_feed_db": args.mb_mid_feed_db,
            "mb_high_feed_db": args.mb_high_feed_db,
            "autogain_enabled": args.autogain,
            "autogain_target_lufs": args.autogain_target_lufs,
            "autogain_max_gain_db": args.autogain_max_gain_db,
            "autogain_smoothing_ms": args.autogain_smoothing_ms,
            "itd_delay_ms": args.itd_delay_ms,
        }),
    })
}

pub(super) fn create_matrix_standalone_plugin_config(
    args: &MatrixArgs,
    input_channels: usize,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let out_ch = args.output_channels.unwrap_or(input_channels);

    let matrix = if let Some(ref coeffs_str) = args.coefficients {
        // Parse "row1_c1,row1_c2;row2_c1,row2_c2" format
        let mut matrix = Vec::new();
        for (row_idx, row_str) in coeffs_str.split(';').enumerate() {
            let row_values: Vec<f32> = row_str
                .split(',')
                .map(|s| {
                    s.trim().parse::<f32>().map_err(|e| {
                        format!(
                            "Invalid matrix coefficient '{}' in row {}: {}",
                            s.trim(),
                            row_idx,
                            e
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if row_values.len() != input_channels {
                return Err(format!(
                    "Matrix row {} has {} values but expected {} (input channels)",
                    row_idx,
                    row_values.len(),
                    input_channels
                ));
            }
            matrix.extend(row_values);
        }
        let expected_rows = out_ch;
        let actual_rows = matrix.len() / input_channels;
        if actual_rows != expected_rows {
            return Err(format!(
                "Matrix has {} rows but expected {} (output channels)",
                actual_rows, expected_rows
            ));
        }
        matrix
    } else {
        // Identity matrix (or zero-padded identity if in != out)
        let mut matrix = vec![0.0f32; out_ch * input_channels];
        for i in 0..std::cmp::min(input_channels, out_ch) {
            matrix[i * input_channels + i] = 1.0;
        }
        matrix
    };

    Ok(PluginConfig {
        plugin_type: "matrix".to_string(),
        parameters: json!({
            "input_channels": input_channels,
            "output_channels": out_ch,
            "matrix": matrix,
        }),
    })
}

/// Convert Biquad filters to PluginConfig for EQ plugin
pub(super) fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use serde_json::json;

    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            let filter_type = match f.filter_type {
                BiquadFilterType::HighpassVariableQ => "highpass".to_string(),
                _ => f.filter_type.long_name().to_lowercase(),
            };

            Ok(json!({
                "filter_type": filter_type,
                "freq": f.freq,
                "q": f.q,
                "db_gain": f.db_gain,
            }))
        })
        .collect();

    let filter_configs = filter_configs?;

    let parameters = json!({
        "filters": filter_configs,
    });

    Ok(PluginConfig {
        plugin_type: "eq".to_string(),
        parameters,
    })
}

pub(super) fn create_matrix_plugin_config(
    input_channel_map: Vec<usize>,
    output_channel_map: Vec<usize>,
    matrix: Vec<f32>,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "input_channel_map": input_channel_map,
        "output_channel_map": output_channel_map,
        "matrix": matrix,
    });

    Ok(PluginConfig {
        plugin_type: "matrix".to_string(),
        parameters,
    })
}
