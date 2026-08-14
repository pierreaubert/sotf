//! Converters for dynamics processors (compressors, expanders, gates, etc.).

use super::PluginConfig;
use crate::plugins::PluginSettings;
use serde_json::json;

pub fn convert_compressor(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Compressor {
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
        sidechain_hpf_order,
        detection_mode,
        lookahead_ms,
        program_dependent_release,
        measured_auto_makeup,
        sidechain_external,
    } = settings
    else {
        return None;
    };
    let mut value = json!({
        "threshold_db": threshold_db,
        "ratio": ratio,
        "attack_ms": attack_ms,
        "release_ms": release_ms,
        "knee_db": knee_db,
        "makeup_gain_db": makeup_gain_db,
        "mix": mix,
        "auto_makeup": auto_makeup,
        "link_channels": link_channels,
        "lookahead_ms": lookahead_ms,
        "measured_auto_makeup": measured_auto_makeup,
    });
    let parameters = value.as_object_mut()?;
    // These legacy controls are not implemented by the current DSP. Preserve
    // non-default requests so construction returns the plugin's explicit
    // unsupported-setting error instead of silently dropping user intent.
    if (*sidechain_hpf_hz - 80.0).abs() > f64::EPSILON {
        parameters.insert("sidechain_hpf_hz".into(), json!(sidechain_hpf_hz));
    }
    if !sidechain_hpf_order.eq_ignore_ascii_case("2nd") {
        parameters.insert("sidechain_hpf_order".into(), json!(sidechain_hpf_order));
    }
    if !detection_mode.eq_ignore_ascii_case("Peak") {
        parameters.insert("detection_mode".into(), json!(detection_mode));
    }
    if *program_dependent_release {
        parameters.insert(
            "program_dependent_release".into(),
            json!(program_dependent_release),
        );
    }
    if *sidechain_external {
        parameters.insert("sidechain_external".into(), json!(sidechain_external));
    }
    Some(PluginConfig::new("compressor", value))
}

pub fn convert_limiter(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Limiter {
        threshold_db,
        release_ms,
        lookahead_ms,
        soft,
        true_peak,
        isp_mode,
        dual_release,
        mix,
        ..
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "limiter",
        json!({
            "threshold_db": threshold_db,
            "release_ms": release_ms,
            "lookahead_ms": lookahead_ms,
            "soft": soft,
            "true_peak": true_peak,
            "isp_mode": isp_mode,
            "dual_release": dual_release,
            "mix": mix,
        }),
    ))
}

pub fn convert_gate(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Gate {
        threshold_db,
        ratio,
        attack_ms,
        hold_ms,
        release_ms,
        mix,
        link_channels,
        sidechain_hpf_hz,
        sidechain_hpf_order,
        detection_mode,
        sidechain_external,
        range_db,
        hysteresis_db,
        knee_db,
        lookahead_ms,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "gate",
        json!({
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": attack_ms,
            "hold_ms": hold_ms,
            "release_ms": release_ms,
            "mix": mix,
            "link_channels": link_channels,
            "sidechain_hpf_hz": sidechain_hpf_hz,
            "sidechain_hpf_order": sidechain_hpf_order,
            "detection_mode": detection_mode,
            "sidechain_external": sidechain_external,
            "range_db": range_db,
            "hysteresis_db": hysteresis_db,
            "knee_db": knee_db,
            "lookahead_ms": lookahead_ms,
        }),
    ))
}

pub fn convert_expander(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Expander {
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
        auto_makeup,
        lookahead_ms,
        detection_mode,
        measured_auto_makeup,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "expander",
        json!({
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": attack_ms,
            "release_ms": release_ms,
            "range_db": range_db,
            "knee_db": knee_db,
            "hysteresis_db": hysteresis_db,
            "hold_ms": hold_ms,
            "mix": mix,
            "link_channels": link_channels,
            "sidechain_hpf_hz": sidechain_hpf_hz,
            "auto_makeup": auto_makeup,
            "lookahead_ms": lookahead_ms,
            "detection_mode": detection_mode,
            "measured_auto_makeup": measured_auto_makeup,
        }),
    ))
}

pub fn convert_multiband_compressor(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::MultibandCompressor {
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
        per_band_lookahead_ms,
        ms_mode,
        bands,
        ..
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "multiband_compressor",
        json!({
            "num_bands": num_bands,
            "crossover_preset": crossover_preset,
            "crossover_frequencies": [crossover_freq_1, crossover_freq_2, crossover_freq_3, crossover_freq_4],
            "crossover_freq_1": crossover_freq_1,
            "crossover_freq_2": crossover_freq_2,
            "crossover_freq_3": crossover_freq_3,
            "crossover_freq_4": crossover_freq_4,
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": attack_ms,
            "release_ms": release_ms,
            "knee_db": knee_db,
            "mix": mix,
            "link_channels": link_channels,
            "per_band_lookahead_ms": per_band_lookahead_ms,
            "ms_mode": ms_mode,
            "bands": bands,
        }),
    ))
}

pub fn convert_multiband_expander(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::MultibandExpander {
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
        detection_mode,
        lookahead_ms,
        bands,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "multiband_expander",
        json!({
            "num_bands": num_bands,
            "crossover_preset": crossover_preset,
            "crossover_frequencies": [crossover_freq_1, crossover_freq_2, crossover_freq_3, crossover_freq_4],
            "crossover_freq_1": crossover_freq_1,
            "crossover_freq_2": crossover_freq_2,
            "crossover_freq_3": crossover_freq_3,
            "crossover_freq_4": crossover_freq_4,
            "threshold_db": threshold_db,
            "ratio": ratio,
            "attack_ms": attack_ms,
            "release_ms": release_ms,
            "range_db": range_db,
            "knee_db": knee_db,
            "hysteresis_db": hysteresis_db,
            "hold_ms": hold_ms,
            "mix": mix,
            "link_channels": link_channels,
            "detection_mode": detection_mode,
            "lookahead_ms": lookahead_ms,
            "bands": bands,
        }),
    ))
}

pub fn convert_de_esser(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::DeEsser {
        frequency,
        q,
        threshold,
        ratio,
        attack,
        release,
        mode,
        mix,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "de_esser",
        json!({
            "frequency": *frequency as f32,
            "q": *q as f32,
            "threshold": *threshold as f32,
            "ratio": *ratio as f32,
            "attack_ms": *attack as f32,
            "release_ms": *release as f32,
            "mode": mode,
            "mix": *mix as f32,
        }),
    ))
}

pub fn convert_transient_shaper(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::TransientShaper {
        attack,
        sustain,
        sensitivity_db,
        output_gain_db,
        mix,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "transient_shaper",
        json!({
            "attack": *attack as f32,
            "sustain": *sustain as f32,
            "sensitivity_db": *sensitivity_db as f32,
            "output_gain_db": *output_gain_db as f32,
            "mix": *mix as f32,
        }),
    ))
}

pub fn convert_dynamic_eq(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::DynamicEq {
        num_bands,
        threshold,
        ratio,
        attack,
        release,
        knee,
        link_channels,
        mix,
        bands,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "dynamic_eq",
        json!({
            "num_bands": *num_bands as usize,
            "threshold": *threshold as f32,
            "ratio": *ratio as f32,
            "attack_ms": *attack as f32,
            "release_ms": *release as f32,
            "knee": *knee as f32,
            "link_channels": link_channels,
            "mix": *mix as f32,
            "bands": bands,
        }),
    ))
}

pub fn convert_spectral_compressor(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::SpectralCompressor {
        fft_size,
        threshold,
        ratio,
        attack,
        release,
        knee,
        spectral_smoothing,
        mix,
        target_mode,
        delta_listen,
        adaptive_threshold,
        adaptive_offset_db,
        channel_link,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "spectral_compressor",
        json!({
            "fft_size_index": *fft_size,
            "threshold_db": *threshold as f32,
            "ratio": *ratio as f32,
            "attack_ms": *attack as f32,
            "release_ms": *release as f32,
            "knee_db": *knee as f32,
            "spectral_smoothing": *spectral_smoothing as f32,
            "mix": *mix as f32,
            "target_mode": *target_mode as usize,
            "delta_listen": delta_listen,
            "adaptive_threshold": adaptive_threshold,
            "adaptive_offset_db": adaptive_offset_db,
            "channel_link": channel_link,
        }),
    ))
}
