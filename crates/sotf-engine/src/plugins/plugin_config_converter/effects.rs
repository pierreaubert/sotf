//! Converters for effect, utility, and monitoring plugins.

use super::PluginConfig;
use crate::plugins::PluginSettings;
use serde_json::json;

pub fn convert_aec(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Aec {
        echo_tail_ms,
        step_size,
        post_filter_enabled,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "aec",
        json!({
            "echo_tail_ms": *echo_tail_ms as f32,
            "step_size": *step_size as f32,
            "post_filter_enabled": post_filter_enabled,
        }),
    ))
}

pub fn convert_stereo_imager(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::StereoImager {
        width,
        low_mid_freq,
        mid_high_freq,
        low_width,
        mid_width,
        high_width,
        mono_bass,
        mix,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "stereo_imager",
        json!({
            "width": *width as f32,
            "low_mid_freq": *low_mid_freq as f32,
            "mid_high_freq": *mid_high_freq as f32,
            "low_width": *low_width as f32,
            "mid_width": *mid_width as f32,
            "high_width": *high_width as f32,
            "mono_bass": mono_bass,
            "mix": *mix as f32,
        }),
    ))
}

pub fn convert_saturation(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Saturation {
        mode,
        drive,
        tone,
        exciter_freq,
        oversampling,
        output_gain_db,
        mix,
        ..
    } = settings
    else {
        return None;
    };
    let mode_str = sotf_plugins::param_specs::saturation::MODES
        .get(*mode as usize)
        .unwrap_or(&"Soft Clip");
    let os_str = sotf_plugins::param_specs::saturation::OVERSAMPLING_OPTIONS
        .get(*oversampling as usize)
        .unwrap_or(&"Off");
    Some(PluginConfig::new(
        "saturation",
        json!({
            "mode": mode_str,
            "drive": *drive as f32,
            "tone": *tone as f32,
            "exciter_freq": *exciter_freq as f32,
            "oversampling": os_str,
            "output_gain_db": *output_gain_db as f32,
            "mix": *mix as f32,
        }),
    ))
}

pub fn convert_loudness_compensation(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::LoudnessCompensation {
        low_freq,
        low_gain,
        high_freq,
        high_gain,
        mid_enabled,
        mid_freq,
        mid_gain,
        mid_q,
        auto_gain_enabled,
        auto_gain_max_db,
        auto_gain_smoothing_ms,
        mode,
        playback_level_db,
        reference_level_db,
        playback_volume_db,
        auto_gain_position,
        headroom_normalized,
        auto_calibrated,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "loudness_compensation",
        json!({
            "low_freq": low_freq,
            "low_gain": low_gain,
            "high_freq": high_freq,
            "high_gain": high_gain,
            "mid_enabled": mid_enabled,
            "mid_freq": mid_freq,
            "mid_gain": mid_gain,
            "mid_q": mid_q,
            "auto_gain_enabled": auto_gain_enabled,
            "auto_gain_max_db": auto_gain_max_db,
            "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
            "mode": mode,
            "playback_level_db": playback_level_db,
            "reference_level_db": reference_level_db,
            "playback_volume_db": playback_volume_db,
            "auto_gain_position": match auto_gain_position { 1 => "pre", 2 => "post", _ => "disabled" },
            "headroom_normalized": headroom_normalized,
            "auto_calibrated": auto_calibrated,
        }),
    ))
}

pub fn convert_fletcher_munson(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::FletcherMunson {
        playback_volume_db,
        reference_level_db,
        ..
    } = settings
    else {
        return None;
    };
    // Backward compat: emit as loudness_compensation with mode=2 (Auto)
    Some(PluginConfig::new(
        "loudness_compensation",
        json!({
            "low_freq": sotf_plugins::param_specs::find_by_key(sotf_plugins::param_specs::loudness_compensation::PARAMS, "low_freq").default_f64(),
            "low_gain": sotf_plugins::param_specs::find_by_key(sotf_plugins::param_specs::loudness_compensation::PARAMS, "low_gain").default_f64(),
            "high_freq": sotf_plugins::param_specs::find_by_key(sotf_plugins::param_specs::loudness_compensation::PARAMS, "high_freq").default_f64(),
            "high_gain": sotf_plugins::param_specs::find_by_key(sotf_plugins::param_specs::loudness_compensation::PARAMS, "high_gain").default_f64(),
            "mode": 2,
            "playback_volume_db": playback_volume_db,
            "reference_level_db": 83.0 + reference_level_db,
            "playback_level_db": sotf_plugins::param_specs::find_by_key(sotf_plugins::param_specs::loudness_compensation::PARAMS, "playback_level_db").default_f64(),
        }),
    ))
}

pub fn convert_convolution(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Convolution {
        ir_file,
        mix,
        gain_db,
        use_nupc,
        zero_latency_head,
        head_taps,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "convolution",
        json!({
            "ir_file": ir_file,
            "mix": mix,
            "gain_db": gain_db,
            "use_nupc": use_nupc,
            "zero_latency_head": zero_latency_head,
            "head_taps": head_taps,
        }),
    ))
}

pub fn convert_loudness_monitor(
    _settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    Some(PluginConfig::new("loudness_monitor", json!({})))
}

pub fn convert_spectrum_analyzer(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::SpectrumAnalyzer {
        num_bins,
        min_freq,
        max_freq,
        smoothing,
        tilt_correction,
        tilt_reference,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "spectrum_analyzer",
        json!({
            "num_bins": num_bins,
            "min_freq": min_freq,
            "max_freq": max_freq,
            "smoothing": smoothing,
            "tilt_correction": tilt_correction,
            "tilt_reference": tilt_reference,
        }),
    ))
}

pub fn convert_channel_mute_solo(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::ChannelMuteSolo {
        enabled,
        dim_gain_db,
        fade_ms,
        channel_states,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "channel_mute_solo",
        json!({
            "enabled": enabled,
            "dim_gain_db": dim_gain_db,
            "fade_ms": fade_ms,
            "channel_states": channel_states,
        }),
    ))
}

pub fn convert_matrix(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        channel_states,
    } = settings
    else {
        return None;
    };
    let off_diag_count = matrix
        .iter()
        .enumerate()
        .filter(|(idx, v)| {
            let row = idx / input_channels;
            let col = idx % input_channels;
            row != col && v.abs() > 1e-6
        })
        .count();
    if off_diag_count > 0 {
        let off_diag_entries: Vec<_> = matrix
            .iter()
            .enumerate()
            .filter(|(idx, v)| {
                let row = idx / input_channels;
                let col = idx % input_channels;
                row != col && v.abs() > 1e-6
            })
            .map(|(idx, v)| {
                let row = idx / input_channels;
                let col = idx % input_channels;
                format!("in{}→out{}={:.3}", col, row, v)
            })
            .collect();
        log::debug!(
            "[Matrix::to_plugin_config] {}x{} with {} off-diagonal: [{}]",
            input_channels,
            output_channels,
            off_diag_count,
            off_diag_entries.join(", "),
        );
    }
    Some(PluginConfig::new(
        "matrix",
        json!({
            "input_channels": input_channels,
            "output_channels": output_channels,
            "matrix": matrix,
            "channel_states": channel_states,
        }),
    ))
}

pub fn convert_denoiser(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Denoiser {
        reduction_db,
        floor_db,
        smoothing,
        attack_ms,
        release_ms,
        low_latency,
        polyphonic_detection,
        mcra_alpha_s,
        mcra_alpha_p,
        mcra_l,
        mcra_delta,
        transparency,
        dd_enabled,
        dd_alpha,
        psychoacoustic_masking,
        spectral_smoothing_enabled,
        temporal_smoothing_enabled,
        spectral_sub_enabled,
        spectral_sub_alpha,
        spectral_sub_beta,
        learn_noise,
        use_captured_profile,
        clear_profile,
        formant_preservation,
        formant_strength,
        multi_resolution,
        harmonic_percussive,
        spatial_denoise,
        spatial_strength,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "denoiser",
        json!({
            "reduction_db": reduction_db,
            "floor_db": floor_db,
            "smoothing": smoothing,
            "attack_ms": attack_ms,
            "release_ms": release_ms,
            "low_latency": low_latency,
            "polyphonic_detection": polyphonic_detection,
            "mcra_alpha_s": mcra_alpha_s,
            "mcra_alpha_p": mcra_alpha_p,
            "mcra_l": mcra_l,
            "mcra_delta": mcra_delta,
            "transparency": transparency,
            "dd_enabled": dd_enabled,
            "dd_alpha": dd_alpha,
            "psychoacoustic_masking": psychoacoustic_masking,
            "spectral_smoothing_enabled": spectral_smoothing_enabled,
            "temporal_smoothing_enabled": temporal_smoothing_enabled,
            "spectral_sub_enabled": spectral_sub_enabled,
            "spectral_sub_alpha": spectral_sub_alpha,
            "spectral_sub_beta": spectral_sub_beta,
            "learn_noise": learn_noise,
            "use_captured_profile": use_captured_profile,
            "clear_profile": clear_profile,
            "formant_preservation": formant_preservation,
            "formant_strength": formant_strength,
            "multi_resolution": multi_resolution,
            "harmonic_percussive": harmonic_percussive,
            "spatial_denoise": spatial_denoise,
            "spatial_strength": spatial_strength,
        }),
    ))
}

pub fn convert_declick(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Declick {
        enabled,
        sensitivity,
        link_channels,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "declick",
        json!({
        "enabled": enabled,
        "sensitivity": sensitivity,
        "link_channels": link_channels,
        }),
    ))
}

pub fn convert_hiss_reducer(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::HissReducer {
        enabled,
        threshold_db,
        frequency_hz,
        strength,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "hiss_reducer",
        json!({
            "enabled": enabled,
            "threshold_db": threshold_db,
            "frequency_hz": frequency_hz,
            "strength": strength,
        }),
    ))
}

pub fn convert_speech_denoiser(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::SpeechDenoiser { enabled } = settings else {
        return None;
    };
    Some(PluginConfig::new(
        "speech_denoiser",
        json!({
            "enabled": enabled,
        }),
    ))
}

pub fn convert_pnd(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Pnd {
        correction_strength,
        analysis_window_ms,
        drift_smoothing,
        multi_channel_analysis,
        confidence_threshold,
        reference_frequency_hz,
        phase_vocoder,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "pnd",
        json!({
            "correction_strength": correction_strength,
            "analysis_window_ms": analysis_window_ms,
            "drift_smoothing": drift_smoothing,
            "multi_channel_analysis": multi_channel_analysis,
            "confidence_threshold": confidence_threshold,
            "reference_frequency_hz": reference_frequency_hz,
            "phase_vocoder": phase_vocoder,
        }),
    ))
}

pub fn convert_ab_compare(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::ABCompare {
        mix,
        mix_mode,
        selected_path,
        bypass,
        auto_gain_enabled,
        loudness_type,
        max_auto_gain_db,
        gain_smoothing_ms,
        mix_transition_ms,
        path_a_config,
        path_b_config,
        phase_invert_a,
        phase_invert_b,
        difference_mode,
        band_mask_low_hz,
        band_mask_high_hz,
        ..
    } = settings
    else {
        return None;
    };
    let loudness_type_str = match loudness_type {
        0 => "Momentary",
        _ => "ShortTerm",
    };
    let mix_mode_str = match mix_mode {
        0 => "Potentiometer",
        _ => "Binary",
    };
    let path_a_val: serde_json::Value =
        serde_json::from_str(path_a_config).unwrap_or(json!({"type": "None"}));
    let path_b_val: serde_json::Value =
        serde_json::from_str(path_b_config).unwrap_or(json!({"type": "None"}));
    Some(PluginConfig::new(
        "ab_compare",
        json!({
            "mix": mix,
            "mix_mode": mix_mode_str,
            "selected_path": selected_path,
            "bypass": bypass,
            "auto_gain_enabled": auto_gain_enabled,
            "loudness_type": loudness_type_str,
            "max_auto_gain_db": max_auto_gain_db,
            "gain_smoothing_ms": gain_smoothing_ms,
            "mix_transition_ms": mix_transition_ms,
            "path_a": path_a_val,
            "path_b": path_b_val,
            "phase_invert_a": phase_invert_a,
            "phase_invert_b": phase_invert_b,
            "difference_mode": difference_mode,
            "band_mask_low_hz": band_mask_low_hz,
            "band_mask_high_hz": band_mask_high_hz,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::plugin_type::PluginType;

    #[test]
    fn saturation_appended_mode_reaches_engine_config() {
        let mut settings = PluginSettings::default_for(&PluginType::Saturation).unwrap();
        let PluginSettings::Saturation { mode, .. } = &mut settings else {
            unreachable!()
        };
        *mode = 4.0;

        let config = convert_saturation(&settings, 48_000.0).unwrap();
        assert_eq!(config.parameters["mode"], "Asymmetric");
    }
}
