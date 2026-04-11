//! Custom View Registry
//!
//! Maps plugin type keys to custom render functions, replacing the
//! match-arm dispatch in `render_plugin_content()`. Plugins without
//! a registered custom view fall through to the generic layout renderer.

use crate::app::AppState;
use crate::components::design::Ds;
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
        views.insert("spectrum_analyzer", render_spectrum);
        views.insert("channel_mute_solo", render_mute_solo);
        views.insert("matrix", render_matrix);
        views.insert("loudness_monitor", render_loudness);
        views.insert("multiband_compressor", render_mb_compressor);
        views.insert("multiband_expander", render_mb_expander);
        views.insert("ab_compare", render_ab_compare);

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
