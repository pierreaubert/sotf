//! Plugin UI Components
//!
//! Each plugin type has its own rendering component to encapsulate
//! visualization and parameter display logic.
//!
//! Also includes logic modules for plugin-related App methods:
//! - `level_meters`: Level meter group management (mute/solo/dim)
//! - `graph`: 2D canvas-based plugin graph view

pub mod actions;
pub mod common;
pub mod editing;
pub mod level_meters;
pub mod theme;
pub mod ticks;

pub mod ui_auto_layout;
mod ui_ab_compare;
mod ui_band_merge;
mod ui_band_split;
mod ui_binaural;
mod ui_compressor;
mod ui_convolution;
mod ui_crossfeed;
mod ui_denoiser;
mod ui_downmix;
pub mod ui_eq;
mod ui_expander;
mod ui_fletcher_munson;
mod ui_gain;
mod ui_gate;
mod ui_graph;
mod ui_limiter;
mod ui_loudness;
mod ui_matrix;
mod ui_mb_compressor;
mod ui_mb_expander;
mod ui_mono_to_stereo;
mod ui_mute_solo;
pub mod ui_plugin_shell;
mod ui_pnd;
mod ui_rack;
mod ui_simple;
mod ui_spectrum;
mod ui_upmixer;
mod ui_xtc;

pub use common::*;
pub use sotf_audio_player_midi::mapping::MidiOverlay;
pub use editing::get_param_count;
pub use level_meters::{
    LevelMeterElement, MeterColors, db_to_position, render_gr_meter, render_gradient_meter,
    render_lufs_with_true_peak, render_peak_meter,
};
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use ui_auto_layout::{AutoLayoutInput, render_auto_layout, render_plugin_auto};
pub use ui_ab_compare::render_ab_compare_plugin;
pub use ui_band_merge::render_band_merge_plugin;
pub use ui_band_split::render_band_split_plugin;
pub use ui_binaural::render_binaural_plugin;
pub use ui_compressor::render_compressor_plugin;
pub use ui_convolution::render_convolution_plugin;
pub use ui_crossfeed::render_crossfeed_plugin;
pub use ui_denoiser::render_denoiser_plugin;
pub use ui_downmix::render_downmix_plugin;
pub use ui_eq::render_eq_plugin;
pub use ui_expander::render_expander_plugin;
pub use ui_fletcher_munson::render_fletcher_munson_plugin;
pub use ui_gain::render_gain_plugin;
pub use ui_gate::render_gate_plugin;
pub use ui_limiter::render_limiter_plugin;
pub use ui_loudness::{render_loudness_compensation_plugin, render_loudness_monitor_plugin};
pub use ui_matrix::render_matrix_plugin;
pub use ui_mb_compressor::render_mb_compressor_plugin;
pub use ui_mb_expander::render_mb_expander_plugin;
pub use ui_mono_to_stereo::render_mono_to_stereo_plugin;
pub use ui_mute_solo::render_mute_solo_plugin;
pub use ui_pnd::render_pnd_plugin;
pub use ui_rack::PluginDragInfo;
pub use ui_simple::render_simple_plugin_view;
pub use ui_spectrum::{
    MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin,
};
pub use ui_plugin_shell::render_plugin_shell;
pub use ui_upmixer::render_upmixer_plugin;
pub use ui_xtc::render_xtc_plugin;

use crate::app::AppState;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::{PluginChain, PluginSettings};

/// Render plugin-specific content based on plugin type
/// Uses `Entity<AppState>` for direct state updates
pub fn render_plugin_content(
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
    config_open: bool,
    selected_band_idx: usize,
    loudness: Option<sotf_audio_player::LoudnessData>,
    plugin_data: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    spectrum_tilt_select_open: bool,
    spectrum_reference_select_open: bool,
    plugin_chain: &PluginChain,
    midi_overlay: Option<&MidiOverlay>,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let auto_tab = entity.read(cx).app.plugin_auto_tab.get(&plugin_idx).copied().unwrap_or(0);
    match settings {
        PluginSettings::EQ {
            channels,
            filters,
            channel_filters,
            per_channel_mode,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(filters.len().saturating_sub(1));
            render_eq_plugin(
                entity.clone(),
                plugin_idx,
                ui_eq::EqRenderState {
                    channels: *channels,
                    filters,
                    channel_filters,
                    per_channel_mode: *per_channel_mode,
                    is_editing,
                    selected_param,
                    selected_band_idx,
                    midi_overlay,
                },
                theme,
                cx,
            )
            .into_any_element()
        }
        PluginSettings::Gain { gain_db, .. } => render_gain_plugin(
            entity.clone(),
            plugin_idx,
            ui_gain::GainRenderState {
                gain_db: *gain_db,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
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
        } => render_compressor_plugin(
            entity.clone(),
            plugin_idx,
            ui_compressor::CompressorRenderState {
                threshold_db: *threshold_db,
                ratio: *ratio,
                attack_ms: *attack_ms,
                release_ms: *release_ms,
                knee_db: *knee_db,
                makeup_gain_db: *makeup_gain_db,
                mix: *mix,
                auto_makeup: *auto_makeup,
                link_channels: *link_channels,
                sidechain_hpf_hz: *sidechain_hpf_hz,
                is_editing,
                selected_param,
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
        } => render_limiter_plugin(
            entity.clone(),
            plugin_idx,
            ui_limiter::LimiterRenderState {
                threshold_db: *threshold_db,
                release_ms: *release_ms,
                lookahead_ms: *lookahead_ms,
                soft: *soft,
                mix: *mix,
                is_editing,
                selected_param,
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } => render_gate_plugin(
            entity.clone(),
            plugin_idx,
            ui_gate::GateRenderState {
                threshold_db: *threshold_db,
                ratio: *ratio,
                attack_ms: *attack_ms,
                hold_ms: *hold_ms,
                release_ms: *release_ms,
                mix: *mix,
                link_channels: *link_channels,
                sidechain_hpf_hz: *sidechain_hpf_hz,
                is_editing,
                selected_param,
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Upmixer {
            speaker_config,
            // Gains
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            stereo_width,
            center_spread,
            surround_direct_bleed,
            rear_late_reflection,
            // LFE
            lfe_cutoff_hz,
            lfe_gain,
            bandpass_hz,
            // Sub-harmonic
            enable_subharmonic_synth,
            subharmonic_gain,
            subharmonic_freq_hz,
            subharmonic_attack_ms,
            subharmonic_release_ms,
            // Decorrelation
            decorrelation_mode,
            decorrelation_lfo_rate_hz,
            velvet_noise_duration_ms,
            velvet_noise_density,
            // Height
            enable_hr_direct,
            hr_sharpen,
            height_hf_cap_hz,
            height_transient_reduction,
            height_direct_leak,
            // Ambient
            ambient_boost,
            safety_cap_db,
            rear_ambient_boost,
            // Dialogue
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
            ..
        } => {
            let upmixer_tab = entity.read(cx).app.upmixer_tab;
            render_upmixer_plugin(
            entity,
            plugin_idx,
            ui_upmixer::UpmixerRenderState {
                speaker_config,
                // Gains
                gain_front_direct: *gain_front_direct,
                gain_front_ambient: *gain_front_ambient,
                gain_rear_ambient: *gain_rear_ambient,
                height_gain: *height_gain,
                stereo_width: *stereo_width,
                center_spread: *center_spread,
                surround_direct_bleed: *surround_direct_bleed,
                rear_late_reflection: *rear_late_reflection,
                // LFE
                lfe_cutoff_hz: *lfe_cutoff_hz,
                lfe_gain: *lfe_gain,
                bandpass_hz: *bandpass_hz,
                // Sub-harmonic
                enable_subharmonic_synth: *enable_subharmonic_synth,
                subharmonic_gain: *subharmonic_gain,
                subharmonic_freq_hz: *subharmonic_freq_hz,
                subharmonic_attack_ms: *subharmonic_attack_ms,
                subharmonic_release_ms: *subharmonic_release_ms,
                // Decorrelation
                decorrelation_mode: *decorrelation_mode,
                decorrelation_lfo_rate_hz: *decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms: *velvet_noise_duration_ms,
                velvet_noise_density: *velvet_noise_density,
                // Height
                enable_hr_direct: *enable_hr_direct,
                hr_sharpen: *hr_sharpen,
                height_hf_cap_hz: *height_hf_cap_hz,
                height_transient_reduction: *height_transient_reduction,
                height_direct_leak: *height_direct_leak,
                // Ambient
                ambient_boost: *ambient_boost,
                safety_cap_db: *safety_cap_db,
                rear_ambient_boost: *rear_ambient_boost,
                // Dialogue
                dialogue_weight: *dialogue_weight,
                voice_freq_min_hz: *voice_freq_min_hz,
                voice_freq_max_hz: *voice_freq_max_hz,
                dialogue_centroid_weight: *dialogue_centroid_weight,
                dialogue_variance_weight: *dialogue_variance_weight,
                dialogue_coherence_weight: *dialogue_coherence_weight,
                // Bypasses
                bypass_decorrelation: *bypass_decorrelation,
                bypass_transient_detection: *bypass_transient_detection,
                bypass_all_processing: *bypass_all_processing,
                enable_ml_detection: *enable_ml_detection,
                // UI state
                is_editing,
                selected_param,
                config_open,
                upmixer_tab,
            },
            theme,
        )
        .into_any_element()
        }
        PluginSettings::LoudnessCompensation { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::FletcherMunson { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(loudness, plugin_idx, is_editing, theme)
                .into_any_element()
        }
        PluginSettings::BinauralDecoder { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::Convolution { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            tilt_correction,
            tilt_reference,
        } => render_spectrum_analyzer_plugin(
            entity.clone(),
            plugin_idx,
            ui_spectrum::SpectrumRenderState {
                num_bins: *num_bins,
                min_freq: *min_freq,
                max_freq: *max_freq,
                smoothing: *smoothing,
                tilt_correction: *tilt_correction,
                tilt_reference: *tilt_reference,
                tilt_select_open: spectrum_tilt_select_open,
                reference_select_open: spectrum_reference_select_open,
                is_editing,
                selected_param,
                data: plugin_data.as_ref().and_then(|d| d.downcast_ref()),
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::ChannelMuteSolo {
            enabled,
            channel_states,
        } => render_mute_solo_plugin(
            entity.clone(),
            plugin_idx,
            ui_mute_solo::ChannelMuteSoloRenderState {
                enabled: *enabled,
                channel_states,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Matrix {
            input_channels,
            output_channels,
            matrix,
            channel_states,
        } => {
            let speaker_config = plugin_chain.speaker_config_at_index(plugin_idx);
            render_matrix_plugin(
                entity.clone(),
                plugin_idx,
                ui_matrix::MatrixRenderState {
                    input_channels: *input_channels,
                    output_channels: *output_channels,
                    matrix,
                    channel_states,
                    speaker_config,
                    is_editing,
                    selected_param,
                    selected_cell: None,
                },
                theme,
            )
            .into_any_element()
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
        } => render_expander_plugin(
            entity.clone(),
            plugin_idx,
            ui_expander::ExpanderRenderState {
                threshold_db: *threshold_db,
                ratio: *ratio,
                attack_ms: *attack_ms,
                release_ms: *release_ms,
                range_db: *range_db,
                knee_db: *knee_db,
                hysteresis_db: *hysteresis_db,
                hold_ms: *hold_ms,
                mix: *mix,
                link_channels: *link_channels,
                sidechain_hpf_hz: *sidechain_hpf_hz,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
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
            bands,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(bands.len());

            // If a band is selected (>0), we show its values if they exist, otherwise global
            let (
                disp_threshold,
                disp_ratio,
                disp_attack,
                disp_release,
                disp_knee,
                disp_makeup,
                disp_solo,
                disp_bypass,
            ) = if selected_band_idx > 0 {
                let b = &bands[selected_band_idx - 1];
                (
                    b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                    b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                    b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                    b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                    b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                    b.makeup_gain_db as f64,
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
                    false,
                )
            };

            render_mb_compressor_plugin(
                entity.clone(),
                plugin_idx,
                ui_mb_compressor::MbCompressorRenderState {
                    num_bands: *num_bands,
                    crossover_preset: *crossover_preset,
                    crossover_freq_1: *crossover_freq_1,
                    crossover_freq_2: *crossover_freq_2,
                    crossover_freq_3: *crossover_freq_3,
                    crossover_freq_4: *crossover_freq_4,
                    threshold_db: disp_threshold,
                    ratio: disp_ratio,
                    attack_ms: disp_attack,
                    release_ms: disp_release,
                    knee_db: disp_knee,
                    makeup_gain_db: disp_makeup,
                    solo: disp_solo,
                    bypass: disp_bypass,
                    mix: *mix,
                    link_channels: *link_channels,
                    is_editing,
                    selected_param,
                    selected_band_idx,
                },
                theme,
            )
            .into_any_element()
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
            bands,
            ..
        } => {
            let selected_band_idx = selected_band_idx.min(bands.len());

            // If a band is selected (>0), we show its values if they exist, otherwise global
            let (
                disp_threshold,
                disp_ratio,
                disp_attack,
                disp_release,
                disp_range,
                disp_knee,
                disp_hysteresis,
                disp_hold,
                disp_solo,
                disp_bypass,
            ) = if selected_band_idx > 0 {
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
                    false,
                )
            };

            render_mb_expander_plugin(
                entity.clone(),
                plugin_idx,
                ui_mb_expander::MbExpanderRenderState {
                    num_bands: *num_bands,
                    crossover_preset: *crossover_preset,
                    crossover_freq_1: *crossover_freq_1,
                    crossover_freq_2: *crossover_freq_2,
                    crossover_freq_3: *crossover_freq_3,
                    crossover_freq_4: *crossover_freq_4,
                    threshold_db: disp_threshold,
                    ratio: disp_ratio,
                    attack_ms: disp_attack,
                    release_ms: disp_release,
                    range_db: disp_range,
                    knee_db: disp_knee,
                    hysteresis_db: disp_hysteresis,
                    hold_ms: disp_hold,
                    solo: disp_solo,
                    bypass: disp_bypass,
                    mix: *mix,
                    link_channels: *link_channels,
                    is_editing,
                    selected_param,
                    selected_band_idx,
                },
                theme,
            )
            .into_any_element()
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
            max_gain_db,
            head_offset_x,
            head_offset_z,
            head_yaw_deg,
            head_tracking_smooth_s,
            spectral_normalization,
            room_reflections_enabled,
            room_ir_file: _,
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
        } => render_xtc_plugin(
            entity.clone(),
            plugin_idx,
            ui_xtc::XtcRenderState {
                distance_m: *distance_m,
                speaker_angle_deg: *speaker_angle_deg,
                head_radius_m: *head_radius_m,
                head_offset_x: *head_offset_x,
                head_offset_z: *head_offset_z,
                head_yaw_deg: *head_yaw_deg,
                head_tracking_smooth_s: *head_tracking_smooth_s,
                beta_base: *beta_base,
                beta_low_freq_boost: *beta_low_freq_boost,
                beta_high_freq_boost: *beta_high_freq_boost,
                head_shadow_cutoff_hz: *head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave: *head_shadow_slope_db_per_octave,
                max_gain_db: *max_gain_db,
                spectral_normalization: *spectral_normalization,
                pinna_model_enabled: *pinna_model_enabled,
                room_reflections_enabled: *room_reflections_enabled,
                room_width_m: *room_width_m,
                room_depth_m: *room_depth_m,
                wall_absorption: *wall_absorption,
                reflection_beta_boost: *reflection_beta_boost,
                bypass_xtc_filters: *bypass_xtc_filters,
                bypass_spectral_normalization: *bypass_spectral_normalization,
                bypass_neumann_refinement: *bypass_neumann_refinement,
                auto_gain_enabled: *auto_gain_enabled,
                auto_gain_max_db: *auto_gain_max_db,
                auto_gain_smoothing_ms: *auto_gain_smoothing_ms,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Denoiser { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::Pnd { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
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
            path_a_config,
            path_b_config,
            ..
        } => {
            // Read dropdown states from plugin state
            let app_state = entity.read(cx);
            let ab_dropdowns = app_state.app.plugin_state.ab_compare_dropdowns;
            let _ = app_state;

            render_ab_compare_plugin(
                entity.clone(),
                plugin_idx,
                ui_ab_compare::ABCompareRenderState {
                    mix: *mix,
                    mix_mode: *mix_mode,
                    selected_path: *selected_path,
                    bypass: *bypass,
                    auto_gain_enabled: *auto_gain_enabled,
                    loudness_type: *loudness_type,
                    max_auto_gain_db: *max_auto_gain_db,
                    gain_smoothing_ms: *gain_smoothing_ms,
                    mix_transition_ms: *mix_transition_ms,
                    path_a_config,
                    path_b_config,
                    is_editing,
                    selected_param,
                    path_a_select_open: ab_dropdowns.path_a_open,
                    path_b_select_open: ab_dropdowns.path_b_open,
                },
                theme,
            )
            .into_any_element()
        }
        PluginSettings::BandSplit { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::BandMerge { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::Downmix { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
        PluginSettings::MonoToStereo { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),

        PluginSettings::Crossfeed { .. } => render_plugin_auto(
            entity.clone(),
            plugin_idx,
            settings,
            is_editing,
            selected_param,
            auto_tab,
            theme,
        )
        .into_any_element(),
    }
}
