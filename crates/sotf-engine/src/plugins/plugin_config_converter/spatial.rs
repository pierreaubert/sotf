//! Converters for spatial, routing, and room plugins.

use super::PluginConfig;
use crate::plugins::PluginSettings;
use serde_json::json;

pub fn convert_beamformer(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Beamformer {
        num_mics,
        mic_spacing_cm,
        steer_angle_deg,
        beamformer_type,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "beamformer",
        json!({
            "num_mics": num_mics,
            "mic_spacing_cm": *mic_spacing_cm as f32,
            "steer_angle_deg": *steer_angle_deg as f32,
            "beamformer_type": beamformer_type,
        }),
    ))
}

pub fn convert_ambisonics_decoder(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::AmbisonicsDecoder {
        order,
        target_layout,
        max_re_weighting,
        dual_band,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "ambisonics_decoder",
        json!({
            "order": order,
            "target_layout": target_layout,
            "max_re_weighting": max_re_weighting,
            "dual_band": dual_band,
        }),
    ))
}

pub fn convert_binaural_decoder(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::BinauralDecoder {
        sofa_file,
        input_channels,
        enable_optimization,
        externalization,
        near_field_strength,
        crossfade_mode,
        late_reverb_enabled,
        late_reverb_mix,
        late_reverb_rt60,
        late_reverb_damping,
        headphone_eq_enabled,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "binaural_decoder",
        json!({
            "sofa_file": sofa_file,
            "input_channels": input_channels,
            "enable_optimization": enable_optimization,
            "externalization": externalization,
            "near_field_strength": near_field_strength,
            "crossfade_mode": crossfade_mode,
            "late_reverb_enabled": late_reverb_enabled,
            "late_reverb_mix": late_reverb_mix,
            "late_reverb_rt60": late_reverb_rt60,
            "late_reverb_damping": late_reverb_damping,
            "headphone_eq_enabled": headphone_eq_enabled,
        }),
    ))
}

pub fn convert_upmixer(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Upmixer {
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
    } = settings
    else {
        return None;
    };
    let crate::plugins::UpmixerGainSettings {
        gain_front_direct,
        gain_front_ambient,
        gain_rear_ambient,
        height_gain,
        stereo_width,
        center_spread,
        surround_direct_bleed,
        rear_late_reflection,
        ambient_boost,
        rear_ambient_boost,
    } = gains;
    let crate::plugins::UpmixerLfeSettings {
        lfe_cutoff_hz,
        lfe_gain,
        bandpass_hz,
    } = lfe;
    let crate::plugins::UpmixerSubharmonicSettings {
        enable_subharmonic_synth,
        subharmonic_gain,
        subharmonic_freq_hz,
        subharmonic_attack_ms,
        subharmonic_release_ms,
    } = subharmonic;
    let crate::plugins::UpmixerDecorrelationSettings {
        decorrelation_mode,
        decorrelation_lfo_rate_hz,
        velvet_noise_duration_ms,
        velvet_noise_density,
    } = decorrelation;
    let crate::plugins::UpmixerHeightSettings {
        enable_hr_direct,
        hr_sharpen,
        height_hf_cap_hz,
        height_transient_reduction,
        height_direct_leak,
    } = height;
    let crate::plugins::UpmixerAmbientAnalysisSettings {
        low_latency,
        frequency_resolution,
        safety_cap_db,
    } = ambient_analysis;
    let crate::plugins::UpmixerDialogueSettings {
        dialogue_weight,
        voice_freq_min_hz,
        voice_freq_max_hz,
        dialogue_centroid_weight,
        dialogue_variance_weight,
        dialogue_coherence_weight,
    } = dialogue;
    let crate::plugins::UpmixerBypassSettings {
        bypass_decorrelation,
        bypass_transient_detection,
        bypass_all_processing,
    } = bypass;
    let crate::plugins::UpmixerOutputSettings {
        enable_ml_detection,
        multi_source_extraction,
        multi_source_threshold,
        binaural_preview,
        auto_gain_enabled,
        auto_gain_max_db,
        auto_gain_smoothing_ms,
    } = output;
    Some(PluginConfig::new(
        "upmixer",
        json!({
            "speaker_config": speaker_config,
            "gain_front_direct": gain_front_direct,
            "gain_front_ambient": gain_front_ambient,
            "gain_rear_ambient": gain_rear_ambient,
            "height_gain": height_gain,
            "stereo_width": stereo_width,
            "center_spread": center_spread,
            "surround_direct_bleed": surround_direct_bleed,
            "rear_late_reflection": rear_late_reflection,
            "lfe_cutoff_hz": lfe_cutoff_hz,
            "lfe_gain": lfe_gain,
            "bandpass_hz": bandpass_hz,
            "enable_subharmonic_synth": enable_subharmonic_synth,
            "subharmonic_gain": subharmonic_gain,
            "subharmonic_freq_hz": subharmonic_freq_hz,
            "subharmonic_attack_ms": subharmonic_attack_ms,
            "subharmonic_release_ms": subharmonic_release_ms,
            "decorrelation_mode": decorrelation_mode,
            "decorrelation_lfo_rate_hz": decorrelation_lfo_rate_hz,
            "velvet_noise_duration_ms": velvet_noise_duration_ms,
            "velvet_noise_density": velvet_noise_density,
            "enable_hr_direct": enable_hr_direct,
            "hr_sharpen": hr_sharpen,
            "height_hf_cap_hz": height_hf_cap_hz,
            "height_transient_reduction": height_transient_reduction,
            "height_direct_leak": height_direct_leak,
            "ambient_boost": ambient_boost,
            "safety_cap_db": safety_cap_db,
            "low_latency": low_latency,
            "frequency_resolution": crate::plugins::misc::upmixer_frequency_resolution_label(*frequency_resolution),
            "rear_ambient_boost": rear_ambient_boost,
            "dialogue_weight": dialogue_weight,
            "voice_freq_min_hz": voice_freq_min_hz,
            "voice_freq_max_hz": voice_freq_max_hz,
            "dialogue_centroid_weight": dialogue_centroid_weight,
            "dialogue_variance_weight": dialogue_variance_weight,
            "dialogue_coherence_weight": dialogue_coherence_weight,
            "bypass_decorrelation": bypass_decorrelation,
            "bypass_transient_detection": bypass_transient_detection,
            "bypass_all_processing": bypass_all_processing,
            "enable_ml_detection": enable_ml_detection,
            "multi_source_extraction": multi_source_extraction,
            "multi_source_threshold": multi_source_threshold,
            "binaural_preview": binaural_preview,
            "auto_gain_enabled": auto_gain_enabled,
            "auto_gain_max_db": auto_gain_max_db,
            "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
        }),
    ))
}

pub fn convert_xtc(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::XTC {
        distance_m,
        speaker_angle_deg,
        head_radius_m,
        beta_base,
        beta_low_freq_boost,
        beta_high_freq_boost,
        head_shadow_cutoff_hz,
        head_shadow_slope_db_per_octave,
        max_gain_db,
        head_offset_x,
        head_offset_z,
        head_yaw_deg,
        head_tracking_smooth_s,
        spectral_normalization,
        room_reflections_enabled,
        room_ir_file,
        room_width_m,
        room_depth_m,
        wall_absorption,
        reflection_beta_boost,
        bypass_xtc_filters,
        bypass_spectral_normalization,
        bypass_neumann_refinement,
        auto_gain_enabled,
        auto_gain_max_db,
        auto_gain_smoothing_ms,
        pinna_model_enabled,
        head_model,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "xtc",
        json!({
            "distance_m": distance_m,
            "speaker_angle_deg": speaker_angle_deg,
            "head_radius_m": head_radius_m,
            "beta_base": beta_base,
            "beta_low_freq_boost": beta_low_freq_boost,
            "beta_high_freq_boost": beta_high_freq_boost,
            "head_shadow_cutoff_hz": head_shadow_cutoff_hz,
            "head_shadow_slope_db_per_octave": head_shadow_slope_db_per_octave,
            "max_gain_db": max_gain_db,
            "head_offset_x": head_offset_x,
            "head_offset_z": head_offset_z,
            "head_yaw_deg": head_yaw_deg,
            "head_tracking_smooth_s": head_tracking_smooth_s,
            "spectral_normalization": spectral_normalization,
            "room_reflections_enabled": room_reflections_enabled,
            "room_ir_file": room_ir_file,
            "room_width_m": room_width_m,
            "room_depth_m": room_depth_m,
            "wall_absorption": wall_absorption,
            "reflection_beta_boost": reflection_beta_boost,
            "bypass_xtc_filters": bypass_xtc_filters,
            "bypass_spectral_normalization": bypass_spectral_normalization,
            "bypass_neumann_refinement": bypass_neumann_refinement,
            "auto_gain_enabled": auto_gain_enabled,
            "auto_gain_max_db": auto_gain_max_db,
            "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
            "pinna_model_enabled": pinna_model_enabled,
            "head_model": *head_model as usize,
        }),
    ))
}

pub fn convert_mono_to_stereo(
    settings: &PluginSettings,
    _sample_rate: f64,
) -> Option<PluginConfig> {
    let PluginSettings::MonoToStereo {
        stereo_width,
        haas_delay_ms,
        decor_low_hz,
        decor_high_hz,
        freq_dependent,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "mono_to_stereo",
        json!({
            "stereo_width": stereo_width,
            "haas_delay_ms": haas_delay_ms,
            "decor_low_hz": decor_low_hz,
            "decor_high_hz": decor_high_hz,
            "freq_dependent": freq_dependent,
        }),
    ))
}

pub fn convert_downmix(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::Downmix {
        input_channels,
        center_gain_db,
        surround_gain_db,
        height_gain_db,
        lfe_gain_db,
        phase_coherence,
        phase_blend_low_hz,
        phase_blend_high_hz,
        itu_mode,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "downmix",
        json!({
            "input_channels": input_channels,
            "center_gain_db": center_gain_db,
            "surround_gain_db": surround_gain_db,
            "height_gain_db": height_gain_db,
            "lfe_gain_db": lfe_gain_db,
            "phase_coherence": phase_coherence,
            "phase_blend_low_hz": phase_blend_low_hz,
            "phase_blend_high_hz": phase_blend_high_hz,
            "itu_mode": itu_mode,
        }),
    ))
}

pub fn convert_band_split(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::BandSplit {
        channels,
        frequency,
        crossover_type,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "band_split",
        json!({
            "channels": channels,
            "frequency": frequency,
            "type": crossover_type,
        }),
    ))
}

pub fn convert_band_merge(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::BandMerge { channels, bands } = settings else {
        return None;
    };
    Some(PluginConfig::new(
        "band_merge",
        json!({
            "channels": channels,
            "bands": bands,
        }),
    ))
}

pub fn convert_aae(settings: &PluginSettings, _sample_rate: f64) -> Option<PluginConfig> {
    let PluginSettings::AAE {
        speaker_config,
        room_size,
        rt60,
        bass_ratio,
        treble_ratio,
        pre_delay_ms,
        room_preset,
        dry_level,
        er_level,
        late_level,
        lfe_level,
        mod_depth,
        er_mod_depth,
        input_diffusion,
        envelopment,
        height_amount,
        content_aware,
        dialogue_attenuation_db,
        safety_limit_db,
        auto_gain_enabled,
        auto_gain_max_db,
        auto_gain_smoothing_ms,
        bypass,
        solo_early,
        solo_late,
    } = settings
    else {
        return None;
    };
    Some(PluginConfig::new(
        "aae",
        json!({
            "speaker_config": speaker_config,
            "room_size": room_size,
            "rt60": rt60,
            "bass_ratio": bass_ratio,
            "treble_ratio": treble_ratio,
            "pre_delay_ms": pre_delay_ms,
            "room_preset": room_preset,
            "dry_level": dry_level,
            "er_level": er_level,
            "late_level": late_level,
            "lfe_level": lfe_level,
            "mod_depth": mod_depth,
            "er_mod_depth": er_mod_depth,
            "input_diffusion": input_diffusion,
            "envelopment": envelopment,
            "height_amount": height_amount,
            "content_aware": content_aware,
            "dialogue_attenuation_db": dialogue_attenuation_db,
            "safety_limit_db": safety_limit_db,
            "auto_gain_enabled": auto_gain_enabled,
            "auto_gain_max_db": auto_gain_max_db,
            "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
            "bypass": bypass,
            "solo_early": solo_early,
            "solo_late": solo_late,
        }),
    ))
}
