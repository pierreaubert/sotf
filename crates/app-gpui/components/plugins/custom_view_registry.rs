//! Custom View Registry
//!
//! Maps plugin type keys to custom render functions, replacing the
//! match-arm dispatch in `render_plugin_content()`. Plugins without
//! a registered custom view fall through to the generic layout renderer.

use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::plugins::theme::PluginTheme;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::{PluginGraph, PluginSettings};
use sotf_audio_player_midi::mapping::MidiOverlay;
use std::collections::HashMap;
use std::sync::Arc;

/// Shared context passed to every custom view render function.
pub struct CustomViewRenderContext<'a> {
    pub entity: Entity<AppState>,
    pub plugin_idx: usize,
    pub settings: &'a PluginSettings,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
    pub theme: &'a Theme,
    /// Resolved plugin chassis theme — cascade of rack default + per-plugin
    /// override. Renderers that have adopted the chassis theme system read
    /// from this; renderers still on the global app theme can ignore it.
    pub plugin_theme: &'a PluginTheme,
    pub loudness: Option<sotf_audio_player::LoudnessData>,
    pub plugin_data: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub spectrum_tilt_select_open: bool,
    pub spectrum_reference_select_open: bool,
    pub plugin_graph: &'a PluginGraph,
    pub midi_overlay: Option<&'a MidiOverlay>,
}

/// Function signature for custom view renderers.
pub type CustomViewRenderFn =
    fn(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement;

/// Extract a type key string from a PluginSettings variant.
pub fn plugin_type_key(settings: &PluginSettings) -> &'static str {
    match settings {
        PluginSettings::EQ { .. } => "eq",
        PluginSettings::Gain { .. } => "gain",
        PluginSettings::AAE { .. } => "aae",
        PluginSettings::Upmixer { .. } => "upmixer",
        PluginSettings::Compressor { .. } => "compressor",
        PluginSettings::Limiter { .. } => "limiter",
        PluginSettings::Gate { .. } => "gate",
        PluginSettings::Expander { .. } => "expander",
        PluginSettings::MultibandCompressor { .. } => "multiband_compressor",
        PluginSettings::MultibandExpander { .. } => "multiband_expander",
        PluginSettings::LoudnessCompensation { .. } => "loudness_compensation",
        PluginSettings::FletcherMunson { .. } => "fletcher_munson",
        PluginSettings::BinauralDecoder { .. } => "binaural",
        PluginSettings::Convolution { .. } => "convolution",
        PluginSettings::LoudnessMonitor => "loudness_monitor",
        PluginSettings::SpectrumAnalyzer { .. } => "spectrum_analyzer",
        PluginSettings::ChannelMuteSolo { .. } => "channel_mute_solo",
        PluginSettings::Matrix { .. } => "matrix",
        PluginSettings::XTC { .. } => "xtc",
        PluginSettings::Denoiser { .. } => "denoiser",
        PluginSettings::Declick { .. } => "declick",
        PluginSettings::HissReducer { .. } => "hiss_reducer",
        PluginSettings::SpeechDenoiser { .. } => "speech_denoiser",
        PluginSettings::Pnd { .. } => "pnd",
        PluginSettings::Crossfeed { .. } => "crossfeed",
        PluginSettings::Delay { .. } => "delay",
        PluginSettings::Downmix { .. } => "downmix",
        PluginSettings::MonoToStereo { .. } => "mono_to_stereo",
        PluginSettings::ABCompare { .. } => "ab_compare",
        PluginSettings::AmbisonicsDecoder { .. } => "ambisonics",
        PluginSettings::Beamformer { .. } => "beamformer",
        PluginSettings::Aec { .. } => "aec",
        PluginSettings::BandSplit { .. } => "band_split",
        PluginSettings::BandMerge { .. } => "band_merge",
        PluginSettings::StereoImager { .. } => "stereo_imager",
        PluginSettings::DeEsser { .. } => "de_esser",
        PluginSettings::TransientShaper { .. } => "transient_shaper",
        PluginSettings::Saturation { .. } => "saturation",
        PluginSettings::DynamicEq { .. } => "dynamic_eq",
        PluginSettings::FirDesigner { .. } => "fir_designer",
        PluginSettings::LinearPhaseEq { .. } => "linear_phase_eq",
        PluginSettings::SpectralCompressor { .. } => "spectral_compressor",
    }
}

/// Registry mapping plugin type keys to custom render functions.
pub struct GpuiViewRegistry {
    views: HashMap<&'static str, CustomViewRenderFn>,
}

impl GpuiViewRegistry {
    /// Create a new registry with all known custom views registered.
    pub fn new() -> Self {
        let mut views: HashMap<&'static str, CustomViewRenderFn> = HashMap::new();

        views.insert("eq", render_eq);
        views.insert("dynamic_eq", render_dynamic_eq);
        views.insert("fir_designer", render_fir_designer);
        views.insert("linear_phase_eq", render_linear_phase_eq);
        views.insert("spectrum_analyzer", render_spectrum);
        views.insert("channel_mute_solo", render_mute_solo);
        views.insert("matrix", render_matrix);
        views.insert("loudness_monitor", render_loudness);
        views.insert("multiband_compressor", render_mb_compressor);
        views.insert("multiband_expander", render_mb_expander);
        views.insert("ab_compare", render_ab_compare);
        views.insert("upmixer", render_upmixer);

        Self { views }
    }

    /// Look up a custom render function for a plugin type.
    pub fn get(&self, plugin_type_key: &str) -> Option<CustomViewRenderFn> {
        self.views.get(plugin_type_key).copied()
    }
}

impl Default for GpuiViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Render function wrappers
// ============================================================================
// Each wrapper extracts the needed fields from PluginSettings and delegates
// to the existing render_*_plugin functions.

fn render_eq(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    use super::ui_eq;
    if let PluginSettings::EQ {
        channels,
        filters,
        channel_filters,
        per_channel_mode,
        ..
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(filters.len().saturating_sub(1));
        super::render_eq_plugin(
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_eq::EqRenderState {
                channels: *channels,
                filters,
                channel_filters,
                per_channel_mode: *per_channel_mode,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
                midi_overlay: ctx.midi_overlay,
                mode: ui_eq::EqViewMode::Standard,
            },
            ctx.theme,
            cx,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_dynamic_eq(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    super::render_dynamic_eq_plugin(
        ctx.entity.clone(),
        ctx.plugin_idx,
        ctx.settings,
        ctx.is_editing,
        ctx.selected_param,
        ctx.selected_band_idx,
        ctx.plugin_data.clone(),
        ctx.theme,
        cx,
    )
    .into_any_element()
}

fn render_linear_phase_eq(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    use super::ui_eq;
    if let PluginSettings::LinearPhaseEq {
        fir_length,
        auto_gain,
        filters,
        ..
    } = ctx.settings
    {
        let fir_len_samples: usize = match *fir_length as usize {
            0 => 1024,
            1 => 2048,
            2 => 4096,
            3 => 8192,
            _ => 2048,
        };
        let latency_samples = fir_len_samples.saturating_sub(1) / 2;
        let sample_rate = ctx
            .entity
            .read(cx)
            .app
            .audio_device_state
            .hal_config
            .sample_rate
            .max(1);
        let latency_ms = (latency_samples as f32) * 1000.0 / (sample_rate as f32);

        let selected_band_idx = ctx.selected_band_idx.min(filters.len().saturating_sub(1));
        super::render_eq_plugin(
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_eq::EqRenderState {
                channels: 2,
                filters,
                channel_filters: &None,
                per_channel_mode: false,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
                midi_overlay: ctx.midi_overlay,
                mode: ui_eq::EqViewMode::LinearPhase {
                    latency_samples,
                    latency_ms,
                    fir_length: fir_len_samples,
                    auto_gain: *auto_gain,
                },
            },
            ctx.theme,
            cx,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_fir_designer(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    use super::ui_eq;
    if let PluginSettings::FirDesigner {
        fir_length,
        phase_mode,
        auto_gain,
        filters,
        ..
    } = ctx.settings
    {
        let fir_len_samples: usize = match *fir_length as usize {
            0 => 1024,
            1 => 2048,
            2 => 4096,
            3 => 8192,
            _ => 2048,
        };
        let phase_mode_label = match *phase_mode as usize {
            1 => "Minimum",
            _ => "Linear",
        };
        let latency_samples = if phase_mode_label == "Linear" {
            fir_len_samples.saturating_sub(1) / 2
        } else {
            0
        };
        let sample_rate = ctx
            .entity
            .read(cx)
            .app
            .audio_device_state
            .hal_config
            .sample_rate
            .max(1);
        let latency_ms = (latency_samples as f32) * 1000.0 / (sample_rate as f32);

        let selected_band_idx = ctx.selected_band_idx.min(filters.len().saturating_sub(1));
        super::render_eq_plugin(
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_eq::EqRenderState {
                channels: 2,
                filters,
                channel_filters: &None,
                per_channel_mode: false,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
                midi_overlay: ctx.midi_overlay,
                mode: ui_eq::EqViewMode::FirDesigner {
                    latency_samples,
                    latency_ms,
                    fir_length: fir_len_samples,
                    phase_mode: phase_mode_label,
                    auto_gain: *auto_gain,
                },
            },
            ctx.theme,
            cx,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_spectrum(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    use super::ui_spectrum;
    let d = Ds::from_cx(cx);
    if let PluginSettings::SpectrumAnalyzer {
        num_bins,
        min_freq,
        max_freq,
        smoothing,
        tilt_correction,
        tilt_reference,
    } = ctx.settings
    {
        super::render_spectrum_analyzer_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_spectrum::SpectrumRenderState {
                num_bins: *num_bins,
                min_freq: *min_freq,
                max_freq: *max_freq,
                smoothing: *smoothing,
                tilt_correction: *tilt_correction,
                tilt_reference: *tilt_reference,
                tilt_select_open: ctx.spectrum_tilt_select_open,
                reference_select_open: ctx.spectrum_reference_select_open,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                data: ctx.plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_upmixer(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    use super::ui_upmixer;
    if let PluginSettings::Upmixer {
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
        low_latency,
        frequency_resolution,
        rear_ambient_boost,
        dialogue_weight,
        voice_freq_min_hz,
        voice_freq_max_hz,
        dialogue_centroid_weight,
        dialogue_variance_weight,
        dialogue_coherence_weight,
        bypass_decorrelation,
        bypass_transient_detection,
        bypass_all_processing,
        enable_ml_detection,
        multi_source_extraction,
        multi_source_threshold,
        binaural_preview,
        auto_gain_enabled,
        auto_gain_max_db,
        auto_gain_smoothing_ms,
    } = ctx.settings
    {
        let (upmixer_tab, loudness_info, spatial_spider) = {
            let app = &ctx.entity.read(cx).app;
            (
                app.upmixer_tab,
                app.playback.loudness_info.clone(),
                app.spatial_spider.clone(),
            )
        };
        let d = Ds::from_cx(cx);
        super::render_upmixer_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_upmixer::UpmixerRenderState {
                speaker_config,
                gain_front_direct: *gain_front_direct,
                gain_front_ambient: *gain_front_ambient,
                gain_rear_ambient: *gain_rear_ambient,
                height_gain: *height_gain,
                stereo_width: *stereo_width,
                center_spread: *center_spread,
                surround_direct_bleed: *surround_direct_bleed,
                rear_late_reflection: *rear_late_reflection,
                lfe_cutoff_hz: *lfe_cutoff_hz,
                lfe_gain: *lfe_gain,
                bandpass_hz: *bandpass_hz,
                enable_subharmonic_synth: *enable_subharmonic_synth,
                subharmonic_gain: *subharmonic_gain,
                subharmonic_freq_hz: *subharmonic_freq_hz,
                subharmonic_attack_ms: *subharmonic_attack_ms,
                subharmonic_release_ms: *subharmonic_release_ms,
                decorrelation_mode: *decorrelation_mode,
                decorrelation_lfo_rate_hz: *decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms: *velvet_noise_duration_ms,
                velvet_noise_density: *velvet_noise_density,
                enable_hr_direct: *enable_hr_direct,
                hr_sharpen: *hr_sharpen,
                height_hf_cap_hz: *height_hf_cap_hz,
                height_transient_reduction: *height_transient_reduction,
                height_direct_leak: *height_direct_leak,
                ambient_boost: *ambient_boost,
                safety_cap_db: *safety_cap_db,
                rear_ambient_boost: *rear_ambient_boost,
                dialogue_weight: *dialogue_weight,
                voice_freq_min_hz: *voice_freq_min_hz,
                voice_freq_max_hz: *voice_freq_max_hz,
                dialogue_centroid_weight: *dialogue_centroid_weight,
                dialogue_variance_weight: *dialogue_variance_weight,
                dialogue_coherence_weight: *dialogue_coherence_weight,
                bypass_decorrelation: *bypass_decorrelation,
                bypass_transient_detection: *bypass_transient_detection,
                bypass_all_processing: *bypass_all_processing,
                enable_ml_detection: *enable_ml_detection,
                low_latency: *low_latency,
                frequency_resolution: *frequency_resolution,
                multi_source_extraction: *multi_source_extraction,
                multi_source_threshold: *multi_source_threshold,
                binaural_preview: *binaural_preview,
                auto_gain_enabled: *auto_gain_enabled,
                auto_gain_max_db: *auto_gain_max_db,
                auto_gain_smoothing_ms: *auto_gain_smoothing_ms,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                config_open: false,
                upmixer_tab,
                loudness_info,
                spatial_spider,
            },
            ctx.theme,
            ctx.plugin_theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_mute_solo(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::ui_mute_solo;
    if let PluginSettings::ChannelMuteSolo {
        enabled,
        channel_states,
        ..
    } = ctx.settings
    {
        super::render_mute_solo_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mute_solo::ChannelMuteSoloRenderState {
                enabled: *enabled,
                channel_states,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_matrix(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::ui_matrix;
    if let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        channel_states,
    } = ctx.settings
    {
        let speaker_config = ctx.plugin_graph.speaker_config_at_index(ctx.plugin_idx);
        super::render_matrix_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_matrix::MatrixRenderState {
                input_channels: *input_channels,
                output_channels: *output_channels,
                matrix,
                channel_states,
                speaker_config,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_cell: None,
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_loudness(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    let d = Ds::from_cx(cx);
    super::render_loudness_monitor_plugin(
        &d,
        ctx.loudness.clone(),
        ctx.plugin_idx,
        ctx.is_editing,
        ctx.theme,
    )
    .into_any_element()
}

fn render_mb_compressor(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::ui_mb_compressor;
    if let PluginSettings::MultibandCompressor {
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
        bands,
        ..
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(bands.len());
        let (dt, dr, da, drl, dk, dm, dam, dact, ds, db) = if selected_band_idx > 0 {
            let b = &bands[selected_band_idx - 1];
            (
                b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                b.makeup_gain_db as f64,
                b.auto_makeup,
                b.active,
                b.solo,
                b.bypass,
            )
        } else {
            (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                0.0,
                false,
                true,
                false,
                false,
            )
        };
        super::render_mb_compressor_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mb_compressor::MbCompressorRenderState {
                num_bands: *num_bands,
                crossover_preset: *crossover_preset,
                crossover_freq_1: *crossover_freq_1,
                crossover_freq_2: *crossover_freq_2,
                crossover_freq_3: *crossover_freq_3,
                crossover_freq_4: *crossover_freq_4,
                threshold_db: dt,
                ratio: dr,
                attack_ms: da,
                release_ms: drl,
                knee_db: dk,
                makeup_gain_db: dm,
                auto_makeup: dam,
                active: dact,
                solo: ds,
                bypass: db,
                mix: *mix,
                link_channels: *link_channels,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_mb_expander(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::ui_mb_expander;
    if let PluginSettings::MultibandExpander {
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
        bands,
        ..
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(bands.len());
        let (dt, dr, da, drl, drng, dk, dh, dhold, dam, dact, ds, db) = if selected_band_idx > 0 {
            let b = &bands[selected_band_idx - 1];
            (
                b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                b.range_db.map(|v| v as f64).unwrap_or(*range_db),
                b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                b.hysteresis_db.map(|v| v as f64).unwrap_or(*hysteresis_db),
                b.hold_ms.map(|v| v as f64).unwrap_or(*hold_ms),
                b.auto_makeup,
                b.active,
                b.solo,
                b.bypass,
            )
        } else {
            (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *range_db,
                *knee_db,
                *hysteresis_db,
                *hold_ms,
                false,
                true,
                false,
                false,
            )
        };
        super::render_mb_expander_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mb_expander::MbExpanderRenderState {
                num_bands: *num_bands,
                crossover_preset: *crossover_preset,
                crossover_freq_1: *crossover_freq_1,
                crossover_freq_2: *crossover_freq_2,
                crossover_freq_3: *crossover_freq_3,
                crossover_freq_4: *crossover_freq_4,
                threshold_db: dt,
                ratio: dr,
                attack_ms: da,
                release_ms: drl,
                range_db: drng,
                knee_db: dk,
                hysteresis_db: dh,
                hold_ms: dhold,
                auto_makeup: dam,
                active: dact,
                solo: ds,
                bypass: db,
                mix: *mix,
                link_channels: *link_channels,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

fn render_ab_compare(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    super::ui_ab_compare::render_ab_compare(ctx, cx)
}
