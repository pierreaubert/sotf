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

mod ui_ab_compare;
mod ui_band_merge;
mod ui_band_split;
mod ui_binaural;
mod ui_compressor;
mod ui_convolution;
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
mod ui_pnd;
mod ui_rack;
mod ui_spectrum;
mod ui_upmixer;
mod ui_crossfeed;
mod ui_simple;
mod ui_xtc;

pub use common::*;
pub use editing::get_param_count;
pub use level_meters::{
    LevelMeterElement, MeterColors, db_to_position, render_gr_meter, render_gradient_meter,
    render_lufs_with_true_peak, render_peak_meter,
};
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};

pub use ui_ab_compare::render_ab_compare_plugin;
pub use ui_band_merge::render_band_merge_plugin;
pub use ui_band_split::render_band_split_plugin;
pub use ui_binaural::render_binaural_plugin;
pub use ui_compressor::render_compressor_plugin;
pub use ui_convolution::render_convolution_plugin;
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
pub use ui_spectrum::{
    MeterData, SpectrumColors, SpectrumElement, render_spectrum_analyzer_plugin,
};
pub use ui_upmixer::render_upmixer_plugin;
pub use ui_crossfeed::render_crossfeed_plugin;
pub use ui_simple::render_simple_plugin_view;
pub use ui_xtc::render_xtc_plugin;

use crate::app::AppState;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::*;
use sotf_audio_player::{PluginChain, PluginSettings};

/// Render plugin-specific content based on plugin type
/// Uses Entity<AppState> for direct state updates
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
    cx: &mut Context<PlayerView>,
) -> AnyElement {
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
            ..
        } => render_upmixer_plugin(
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
                // UI state
                is_editing,
                selected_param,
                config_open,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
        } => render_loudness_compensation_plugin(
            entity.clone(),
            plugin_idx,
            ui_loudness::LoudnessCompensationRenderState {
                low_freq: *low_freq,
                low_gain: *low_gain,
                high_freq: *high_freq,
                high_gain: *high_gain,
                auto_gain_enabled: *auto_gain_enabled,
                auto_gain_max_db: *auto_gain_max_db,
                auto_gain_smoothing_ms: *auto_gain_smoothing_ms,
                auto_gain_current_db: 0.0, // TODO: Get from plugin state when available
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::FletcherMunson {
            playback_volume_db,
            reference_level_db,
            enabled: _, // Ignored as it's handled by rack wrapper
            band1_freq,
            band1_q,
            band1_max_gain,
            band1_slope,
            band2_freq,
            band2_q,
            band2_max_gain,
            band2_slope,
            band3_freq,
            band3_q,
            band3_max_gain,
            band3_slope,
            band4_freq,
            band4_q,
            band4_max_gain,
            band4_slope,
            smoothing_ms,
            auto_gain_enabled,
            auto_gain_max_db,
            auto_gain_smoothing_ms,
            auto_gain_loudness_type,
        } => render_fletcher_munson_plugin(
            entity.clone(),
            plugin_idx,
            ui_fletcher_munson::FletcherMunsonRenderState {
                playback_volume_db: *playback_volume_db,
                reference_level_db: *reference_level_db,
                band1_freq: *band1_freq,
                band1_q: *band1_q,
                band1_max_gain: *band1_max_gain,
                band1_slope: *band1_slope,
                band2_freq: *band2_freq,
                band2_q: *band2_q,
                band2_max_gain: *band2_max_gain,
                band2_slope: *band2_slope,
                band3_freq: *band3_freq,
                band3_q: *band3_q,
                band3_max_gain: *band3_max_gain,
                band3_slope: *band3_slope,
                band4_freq: *band4_freq,
                band4_q: *band4_q,
                band4_max_gain: *band4_max_gain,
                band4_slope: *band4_slope,
                smoothing_ms: *smoothing_ms,
                auto_gain_enabled: *auto_gain_enabled,
                auto_gain_max_db: *auto_gain_max_db,
                auto_gain_smoothing_ms: *auto_gain_smoothing_ms,
                auto_gain_loudness_type: *auto_gain_loudness_type,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::LoudnessMonitor => {
            render_loudness_monitor_plugin(loudness, plugin_idx, is_editing, theme)
                .into_any_element()
        }
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } => render_binaural_plugin(
            entity.clone(),
            plugin_idx,
            ui_binaural::BinauralRenderState {
                sofa_file,
                input_channels: *input_channels,
                enable_optimization: *enable_optimization,
                externalization: *externalization,
                near_field_strength: *near_field_strength,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Convolution {
            ir_file,
            mix,
            gain_db,
        } => render_convolution_plugin(
            entity.clone(),
            plugin_idx,
            ui_convolution::ConvolutionRenderState {
                ir_file,
                mix: *mix,
                gain_db: *gain_db,
                is_editing,
                selected_param,
            },
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
                tilt_correction: tilt_correction.clone(),
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
            let selected_band_idx = selected_band_idx.min(*num_bands);
            
            // If a band is selected (>0), we show its values if they exist, otherwise global
            let (disp_threshold, disp_ratio, disp_attack, disp_release, disp_knee, disp_makeup, disp_solo, disp_bypass) = 
                if selected_band_idx > 0 {
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
                    (*threshold_db, *ratio, *attack_ms, *release_ms, *knee_db, 0.0, false, false)
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
            let selected_band_idx = selected_band_idx.min(*num_bands);

            // If a band is selected (>0), we show its values if they exist, otherwise global
            let (disp_threshold, disp_ratio, disp_attack, disp_release, disp_range, disp_knee, disp_hysteresis, disp_hold, disp_solo, disp_bypass) = 
                if selected_band_idx > 0 {
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
                    (*threshold_db, *ratio, *attack_ms, *release_ms, *range_db, *knee_db, *hysteresis_db, *hold_ms, false, false)
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
            head_tracking_smooth_s: _,
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
        PluginSettings::Denoiser {
            reduction_db,
            floor_db,
            smoothing,
            attack_ms,
            release_ms,
            low_latency,
            polyphonic_detection,
            crack_sensitivity,
            mcra_alpha_s,
            mcra_alpha_p,
            mcra_l,
            mcra_delta,
            transparency,
            dd_enabled,
            dd_alpha,
            psychoacoustic_masking,
            learn_noise,
            use_captured_profile,
            clear_profile,
        } => render_denoiser_plugin(
            entity.clone(),
            plugin_idx,
            ui_denoiser::DenoiserRenderState {
                reduction_db: *reduction_db,
                floor_db: *floor_db,
                smoothing: *smoothing,
                attack_ms: *attack_ms,
                release_ms: *release_ms,
                low_latency: *low_latency,
                polyphonic_detection: *polyphonic_detection,
                crack_sensitivity: *crack_sensitivity,
                mcra_alpha_s: *mcra_alpha_s,
                mcra_alpha_p: *mcra_alpha_p,
                mcra_l: *mcra_l,
                mcra_delta: *mcra_delta,
                transparency: *transparency,
                dd_enabled: *dd_enabled,
                dd_alpha: *dd_alpha,
                psychoacoustic_masking: *psychoacoustic_masking,
                learn_noise: *learn_noise,
                use_captured_profile: *use_captured_profile,
                clear_profile: *clear_profile,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Pnd {
            correction_strength,
            analysis_window_ms,
            drift_smoothing,
        } => render_pnd_plugin(
            entity.clone(),
            plugin_idx,
            ui_pnd::PndRenderState {
                correction_strength: *correction_strength,
                analysis_window_ms: *analysis_window_ms,
                drift_smoothing: *drift_smoothing,
                is_editing,
                selected_param,
            },
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
        PluginSettings::BandSplit {
            frequency,
            crossover_type,
            ..
        } => render_band_split_plugin(
            entity.clone(),
            plugin_idx,
            ui_band_split::BandSplitRenderState {
                frequency: *frequency,
                crossover_type: crossover_type.clone(),
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::BandMerge { bands, .. } => render_band_merge_plugin(
            entity.clone(),
            plugin_idx,
            ui_band_merge::BandMergeRenderState {
                bands: *bands,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::Downmix {
            center_gain_db,
            surround_gain_db,
            height_gain_db,
            lfe_gain_db,
            phase_coherence,
            phase_blend_low_hz,
            phase_blend_high_hz,
            ..
        } => render_downmix_plugin(
            entity.clone(),
            plugin_idx,
            ui_downmix::DownmixRenderState {
                center_gain_db: *center_gain_db,
                surround_gain_db: *surround_gain_db,
                height_gain_db: *height_gain_db,
                lfe_gain_db: *lfe_gain_db,
                phase_coherence: *phase_coherence,
                phase_blend_low_hz: *phase_blend_low_hz,
                phase_blend_high_hz: *phase_blend_high_hz,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),
        PluginSettings::MonoToStereo {
            stereo_width,
            haas_delay_ms,
            enable_comp_eq,
            comp_eq_depth_db,
            decor_low_hz,
            decor_high_hz,
        } => render_mono_to_stereo_plugin(
            entity.clone(),
            plugin_idx,
            ui_mono_to_stereo::MonoToStereoRenderState {
                stereo_width: *stereo_width,
                haas_delay_ms: *haas_delay_ms,
                enable_comp_eq: *enable_comp_eq,
                comp_eq_depth_db: *comp_eq_depth_db,
                decor_low_hz: *decor_low_hz,
                decor_high_hz: *decor_high_hz,
                is_editing,
                selected_param,
            },
            theme,
        )
        .into_any_element(),

        PluginSettings::Crossfeed {
            mode,
            preset,
            enabled,
            mix,
            bauer_fcut_hz,
            bauer_feed_db,
            meier_level,
            mb_low_freq_hz,
            mb_mid_high_freq_hz,
            mb_low_feed_db,
            mb_mid_feed_db,
            mb_high_feed_db,
            autogain_enabled,
            autogain_target_lufs,
            autogain_max_gain_db,
            autogain_smoothing_ms,
        } => {
            let preset_select_open = entity.read(cx).app.crossfeed_preset_select_open;
            render_crossfeed_plugin(
                entity.clone(),
                plugin_idx,
                ui_crossfeed::CrossfeedRenderState {
                    mode: *mode,
                    preset: *preset,
                    enabled: *enabled,
                    mix: *mix,
                    bauer_fcut_hz: *bauer_fcut_hz,
                    bauer_feed_db: *bauer_feed_db,
                    meier_level: *meier_level,
                    mb_low_freq_hz: *mb_low_freq_hz,
                    mb_mid_high_freq_hz: *mb_mid_high_freq_hz,
                    mb_low_feed_db: *mb_low_feed_db,
                    mb_mid_feed_db: *mb_mid_feed_db,
                    mb_high_feed_db: *mb_high_feed_db,
                    autogain_enabled: *autogain_enabled,
                    autogain_target_lufs: *autogain_target_lufs,
                    autogain_max_gain_db: *autogain_max_gain_db,
                    autogain_smoothing_ms: *autogain_smoothing_ms,
                    is_editing,
                    selected_param,
                    preset_select_open,
                },
                theme,
            )
            .into_any_element()
        }
    }
}
