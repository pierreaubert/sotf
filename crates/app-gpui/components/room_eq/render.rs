use crate::app::types::room_eq::RoomEqReviewGraphSettings;
use crate::components::design::Ds;
use crate::components::graphs::common::render_empty_state;
use crate::components::icons::IconName;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::LegendPosition;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};
use sotf_audio::signal_analysis as dsp;
use std::collections::BTreeSet;

const ROOM_EQ_PYTHON_DEFAULT_SMOOTHING_OCTAVES: f64 = 1.0 / 6.0;

/// Window-width threshold above which the per-channel review charts
/// switch from stacked to a 2-column grid.
const ROOM_EQ_REVIEW_WIDE_BREAKPOINT_PX: f32 = 1600.0;
const ROOM_EQ_CHANNEL_COLORS: [u32; 10] = [
    0x1f77b4, // blue
    0xff7f0e, // orange
    0x2ca02c, // green
    0xd62728, // red
    0x9467bd, // purple
    0x8c564b, // brown
    0xe377c2, // pink
    0x7f7f7f, // gray
    0xbcbd22, // olive
    0x17becf, // cyan
];

const ROOM_EQ_DRIVER_OPACITIES: [f32; 4] = [1.0, 0.78, 0.58, 0.42];

/// Whether the Review step should render the per-filter plot for a
/// channel result.
///
/// Pure predicate extracted from [`Self::render_room_eq_review`] so that
/// the rule is testable in isolation. The plot should appear whenever we
/// have frequency-response data **and** at least one filter stage to
/// overlay — either the main IIR set (`has_main`) or the broadband
/// pre-correction (`has_broadband`).
///
/// Regression guard for Issue 6: a previous version gated on `has_main`
/// alone, which meant broadband-only optimizations rendered no plot at
/// all even though filters existed.
pub fn should_render_filter_plot(
    has_response_data: bool,
    has_main: bool,
    has_broadband: bool,
) -> bool {
    has_response_data && (has_main || has_broadband)
}

pub fn room_eq_trend_fit_domain(_channel_name: &str, frequencies: &[f64]) -> Option<(f64, f64)> {
    let (data_min, data_max) = finite_positive_frequency_range(frequencies)?;
    let (fit_min, fit_max) = (100.0_f64, 10_000.0_f64);

    let min_freq = fit_min.max(data_min);
    let max_freq = fit_max.min(data_max);
    (max_freq > min_freq).then_some((min_freq, max_freq))
}

pub fn is_room_eq_sub_or_lfe_channel(channel_name: &str) -> bool {
    autoeq::roomeq::home_cinema::role_for_channel(channel_name).is_sub_or_lfe()
}

pub fn room_eq_passband_trend_fit_domain(freqs: &[f64], values: &[f64]) -> Option<(f64, f64)> {
    const PASSBAND_DROP_DB: f64 = 3.0;
    const LOG_INSET_FRACTION: f64 = 0.10;

    let points: Vec<(f64, f64)> = freqs
        .iter()
        .zip(values.iter())
        .filter_map(|(&f, &v)| (f.is_finite() && f > 0.0 && v.is_finite()).then_some((f, v)))
        .collect();
    if points.len() < 2 {
        return None;
    }

    let max_value = points
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    if !max_value.is_finite() {
        return None;
    }
    let threshold = max_value - PASSBAND_DROP_DB;
    let first_in_band = points.iter().position(|(_, v)| *v >= threshold)?;
    let last_in_band = points.iter().rposition(|(_, v)| *v >= threshold)?;

    let lower_3db = if first_in_band > 0 {
        interpolate_log_frequency_at_db(points[first_in_band - 1], points[first_in_band], threshold)
    } else {
        points[first_in_band].0
    };
    let upper_3db = if last_in_band + 1 < points.len() {
        interpolate_log_frequency_at_db(points[last_in_band], points[last_in_band + 1], threshold)
    } else {
        points[last_in_band].0
    };

    if !lower_3db.is_finite() || !upper_3db.is_finite() || upper_3db <= lower_3db {
        return None;
    }

    let log_lower = lower_3db.ln();
    let log_upper = upper_3db.ln();
    let log_width = log_upper - log_lower;
    if log_width <= 1e-9 {
        return None;
    }

    let reduced = (
        (log_lower + LOG_INSET_FRACTION * log_width).exp(),
        (log_upper - LOG_INSET_FRACTION * log_width).exp(),
    );
    if count_points_in_domain(&points, reduced) >= 2 {
        Some(reduced)
    } else if count_points_in_domain(&points, (lower_3db, upper_3db)) >= 2 {
        Some((lower_3db, upper_3db))
    } else {
        None
    }
}

fn interpolate_log_frequency_at_db(lower: (f64, f64), upper: (f64, f64), target_db: f64) -> f64 {
    let denom = upper.1 - lower.1;
    if denom.abs() < 1e-12 {
        return (lower.0 * upper.0).sqrt();
    }
    let t = ((target_db - lower.1) / denom).clamp(0.0, 1.0);
    (lower.0.ln() + t * (upper.0.ln() - lower.0.ln())).exp()
}

fn count_points_in_domain(points: &[(f64, f64)], domain: (f64, f64)) -> usize {
    points
        .iter()
        .filter(|(f, _)| *f >= domain.0 && *f <= domain.1)
        .count()
}

pub fn calculate_room_eq_log_trend(
    freqs: &[f64],
    values: &[f64],
    domain: (f64, f64),
) -> Option<(f64, f64)> {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    let mut count = 0.0;

    for (i, &f) in freqs.iter().enumerate() {
        if f >= domain.0
            && f <= domain.1
            && f.is_finite()
            && f > 0.0
            && let Some(&y) = values.get(i)
            && y.is_finite()
        {
            let x = f.log10();
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
            count += 1.0;
        }
    }

    if count < 2.0 {
        return None;
    }

    let mean_x = sum_x / count;
    let mean_y = sum_y / count;
    let denominator = sum_xx - count * mean_x * mean_x;
    if denominator.abs() < 1e-10 {
        return None;
    }

    let slope = (sum_xy - count * mean_x * mean_y) / denominator;
    let intercept = mean_y - slope * mean_x;
    Some((slope, intercept))
}

fn finite_positive_frequency_range(frequencies: &[f64]) -> Option<(f64, f64)> {
    let mut min_freq = f64::INFINITY;
    let mut max_freq = f64::NEG_INFINITY;

    for &f in frequencies {
        if f.is_finite() && f > 0.0 {
            min_freq = min_freq.min(f);
            max_freq = max_freq.max(f);
        }
    }

    (min_freq.is_finite() && max_freq.is_finite()).then_some((min_freq, max_freq))
}

pub fn sum_room_eq_responses_db(
    main: &[(f64, f64)],
    sub: &[(f64, f64)],
    main_phase_deg: Option<&[(f64, f64)]>,
    sub_phase_deg: Option<&[(f64, f64)]>,
) -> Vec<(f64, f64)> {
    let sub_freqs: Vec<f64> = sub.iter().map(|(f, _)| *f).collect();
    let sub_values: Vec<f64> = sub.iter().map(|(_, db)| *db).collect();
    let Some((sub_min, sub_max)) = finite_positive_frequency_range(&sub_freqs) else {
        return main.to_vec();
    };

    let main_phase_freqs: Vec<f64> = main_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    let main_phase_values: Vec<f64> = main_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(_, p)| *p)
        .collect();
    let sub_phase_freqs: Vec<f64> = sub_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    let sub_phase_values: Vec<f64> = sub_phase_deg
        .unwrap_or_default()
        .iter()
        .map(|(_, p)| *p)
        .collect();
    let phase_available = !main_phase_freqs.is_empty() && !sub_phase_freqs.is_empty();

    main.iter()
        .map(|&(f, main_db)| {
            if f < sub_min || f > sub_max || !f.is_finite() || !main_db.is_finite() {
                return (f, main_db);
            }

            let sub_db = interpolate_value_at(&sub_freqs, &sub_values, f);
            if !sub_db.is_finite() {
                return (f, main_db);
            }

            let sum_db = if phase_available {
                let main_phase =
                    interpolate_value_at(&main_phase_freqs, &main_phase_values, f).to_radians();
                let sub_phase =
                    interpolate_value_at(&sub_phase_freqs, &sub_phase_values, f).to_radians();
                let main_amp = 10.0_f64.powf(main_db / 20.0);
                let sub_amp = 10.0_f64.powf(sub_db / 20.0);
                let re = main_amp * main_phase.cos() + sub_amp * sub_phase.cos();
                let im = main_amp * main_phase.sin() + sub_amp * sub_phase.sin();
                20.0 * re.hypot(im).max(1.0e-12).log10()
            } else {
                let main_amp = 10.0_f64.powf(main_db / 20.0);
                let sub_amp = 10.0_f64.powf(sub_db / 20.0);
                20.0 * (main_amp + sub_amp).max(1.0e-12).log10()
            };
            (f, sum_db)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportCurve {
    pub freq: Vec<f64>,
    pub spl: Vec<f64>,
    pub phase: Option<Vec<f64>>,
}

impl RoomEqReportCurve {
    fn is_empty(&self) -> bool {
        self.freq.is_empty() || self.spl.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportIr {
    pub time_ms: Vec<f64>,
    pub amplitude: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportDriverCurve {
    pub driver_name: String,
    pub curve: RoomEqReportCurve,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportFilter {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEqPass {
    pub label: String,
    pub display_name: String,
    pub color: u32,
    pub filters: Vec<RoomEqReportFilter>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEpaScore {
    pub preference: f64,
    pub evaluation: f64,
    pub potency: f64,
    pub activity: f64,
    pub sharpness_acum: f64,
    pub roughness: f64,
    pub total_loudness_sone: f64,
    pub loudness_balance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEpaComparison {
    pub pre: RoomEqReportEpaScore,
    pub post: RoomEqReportEpaScore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassRoute {
    pub source_channel: String,
    pub destination: String,
    pub route_kind: String,
    pub group_id: Option<String>,
    pub crossover_type: String,
    pub high_pass_hz: Option<f64>,
    pub low_pass_hz: Option<f64>,
    pub gain_db: f64,
    pub matrix_gain: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassGroup {
    pub group_id: String,
    pub roles: Vec<String>,
    pub crossover_type: String,
    pub selected_crossover_hz: Option<f64>,
    pub main_delay_ms: f64,
    pub bass_route_delay_ms: f64,
    pub polarity_inverted: bool,
    pub trim_db: f64,
    pub advisories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassSubOutput {
    pub output_role: String,
    pub strategy_source: String,
    pub gain_db: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
    pub headroom_contribution_db: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassHeadroomOutput {
    pub output_role: String,
    pub rms_bus_gain_db: f64,
    pub coherent_peak_gain_db: f64,
    pub lfe_contribution_db: f64,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
    pub pass: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassHeadroom {
    pub model: String,
    pub headroom_margin_db: f64,
    pub pass: bool,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
    pub per_output: Vec<RoomEqReportBassHeadroomOutput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassManagement {
    pub enabled: bool,
    pub crossover_type: String,
    pub crossover_frequency_hz: Option<f64>,
    pub lfe_playback_gain_db: f64,
    pub applied_sub_gain_db: Option<f64>,
    pub input_channels: Vec<String>,
    pub output_channels: Vec<String>,
    pub physical_outputs: Vec<String>,
    pub route_count: usize,
    pub advisory: String,
    pub advisories: Vec<String>,
    pub routes: Vec<RoomEqReportBassRoute>,
    pub groups: Vec<RoomEqReportBassGroup>,
    pub sub_outputs: Vec<RoomEqReportBassSubOutput>,
    pub headroom: Option<RoomEqReportBassHeadroom>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportChannel {
    pub name: String,
    pub initial_curve: Option<RoomEqReportCurve>,
    pub final_curve: Option<RoomEqReportCurve>,
    pub eq_response: Option<RoomEqReportCurve>,
    pub target_curve: Option<RoomEqReportCurve>,
    pub pre_ir: Option<RoomEqReportIr>,
    pub post_ir: Option<RoomEqReportIr>,
    pub driver_initial_curves: Vec<RoomEqReportDriverCurve>,
    pub eq_passes: Vec<RoomEqReportEqPass>,
    pub epa: Option<RoomEqReportEpaComparison>,
}

/// Aggregate FIR temporal-masking metrics derived from
/// `PerceptualMetrics.fir_*`. Lower (more negative) audible dB values mean
/// less audible ringing; lower penalty means a perceptually safer FIR.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportFirMasking {
    pub pre_audible_db: Option<f64>,
    pub post_audible_db: Option<f64>,
    pub penalty: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportData {
    pub version: String,
    pub pre_score: Option<f64>,
    pub post_score: Option<f64>,
    pub algorithm: Option<String>,
    pub loss_type: Option<String>,
    pub iterations: Option<usize>,
    pub timestamp: Option<String>,
    pub epa_preference_avg: Option<(f64, f64)>,
    pub fir_masking: Option<RoomEqReportFirMasking>,
    pub bass_management: Option<RoomEqReportBassManagement>,
    pub channels: Vec<RoomEqReportChannel>,
}

#[derive(Clone)]
struct RoomEqChartSeries {
    channel_name: Option<String>,
    label: String,
    curve: RoomEqReportCurve,
    color: u32,
    stroke_width: f32,
    opacity: f32,
}

pub fn room_eq_python_default_smoothing_octaves() -> f64 {
    ROOM_EQ_PYTHON_DEFAULT_SMOOTHING_OCTAVES
}

pub fn room_eq_channel_sort_key(channel_name: &str) -> (u16, String) {
    const CHANNEL_ORDER: &[(&str, u16)] = &[
        ("L", 10),
        ("LEFT", 10),
        ("R", 20),
        ("RIGHT", 20),
        ("C", 30),
        ("CENTER", 30),
        ("LFE", 40),
        ("SUB", 40),
        ("SUBWOOFER", 40),
        ("LFE1", 41),
        ("LFE2", 42),
        ("SL", 50),
        ("SURROUND LEFT", 50),
        ("LS", 50),
        ("SR", 60),
        ("SURROUND RIGHT", 60),
        ("RS", 60),
        ("SBL", 70),
        ("SURROUND BACK LEFT", 70),
        ("LBS", 70),
        ("LB", 70),
        ("SBR", 80),
        ("SURROUND BACK RIGHT", 80),
        ("RBS", 80),
        ("RB", 80),
        ("FHL", 90),
        ("FRONT HEIGHT LEFT", 90),
        ("FHR", 100),
        ("FRONT HEIGHT RIGHT", 100),
        ("BHL", 110),
        ("BACK HEIGHT LEFT", 110),
        ("BHR", 120),
        ("BACK HEIGHT RIGHT", 120),
    ];

    let upper = channel_name.to_ascii_uppercase();
    if let Some((_, order)) = CHANNEL_ORDER.iter().find(|(key, _)| upper == *key) {
        return (*order, channel_name.to_string());
    }

    for (key, order) in CHANNEL_ORDER {
        if upper.starts_with(key)
            && (upper.len() == key.len()
                || upper[key.len()..]
                    .chars()
                    .next()
                    .is_some_and(|ch| !ch.is_alphanumeric()))
        {
            return (*order, channel_name.to_string());
        }
    }

    (1000, channel_name.to_string())
}

pub fn room_eq_report_data_from_dsp_output(
    output: &autoeq::roomeq::DspChainOutput,
) -> RoomEqReportData {
    let mut channels: Vec<RoomEqReportChannel> = output
        .channels
        .iter()
        .map(|(map_name, chain)| {
            let name = if chain.channel.is_empty() {
                map_name.clone()
            } else {
                chain.channel.clone()
            };
            let driver_initial_curves = chain
                .drivers
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter_map(|driver| {
                    driver.initial_curve.as_ref().and_then(|curve| {
                        room_eq_report_curve_from_curve_data(curve).map(|curve| {
                            RoomEqReportDriverCurve {
                                driver_name: driver.name.clone(),
                                curve,
                            }
                        })
                    })
                })
                .collect();

            let epa = output
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.epa_per_channel.as_ref())
                .and_then(|per_channel| per_channel.get(&name))
                .map(room_eq_report_epa_comparison_from_metrics);

            RoomEqReportChannel {
                name,
                initial_curve: chain
                    .initial_curve
                    .as_ref()
                    .and_then(room_eq_report_curve_from_curve_data),
                final_curve: chain
                    .final_curve
                    .as_ref()
                    .and_then(room_eq_report_curve_from_curve_data),
                eq_response: chain
                    .eq_response
                    .as_ref()
                    .and_then(room_eq_report_curve_from_curve_data),
                target_curve: chain
                    .target_curve
                    .as_ref()
                    .and_then(room_eq_report_curve_from_curve_data),
                pre_ir: chain
                    .pre_ir
                    .as_ref()
                    .and_then(room_eq_report_ir_from_waveform),
                post_ir: chain
                    .post_ir
                    .as_ref()
                    .and_then(room_eq_report_ir_from_waveform),
                driver_initial_curves,
                eq_passes: room_eq_report_eq_passes_from_plugins(&chain.plugins),
                epa,
            }
        })
        .collect();
    channels.sort_by_key(|channel| room_eq_channel_sort_key(&channel.name));

    let metadata = output.metadata.as_ref();
    RoomEqReportData {
        version: output.version.clone(),
        pre_score: metadata.map(|m| m.pre_score),
        post_score: metadata.map(|m| m.post_score),
        algorithm: metadata.map(|m| m.algorithm.clone()),
        loss_type: metadata.and_then(|m| m.loss_type.clone()),
        iterations: metadata.map(|m| m.iterations),
        timestamp: metadata.map(|m| m.timestamp.clone()),
        epa_preference_avg: metadata.and_then(room_eq_report_epa_preference_avg),
        fir_masking: metadata
            .and_then(|m| m.perceptual_metrics.as_ref())
            .map(|pm| RoomEqReportFirMasking {
                pre_audible_db: pm.fir_pre_ringing_audible_db,
                post_audible_db: pm.fir_post_ringing_audible_db,
                penalty: pm.fir_temporal_masking_penalty,
            })
            // Only surface the block when at least one field carries data —
            // otherwise an all-None record would render an empty card.
            .filter(|m| {
                m.pre_audible_db.is_some() || m.post_audible_db.is_some() || m.penalty.is_some()
            }),
        bass_management: metadata
            .and_then(|m| m.bass_management.as_ref())
            .map(room_eq_report_bass_management_from_report),
        channels,
    }
}

pub fn room_eq_report_channel_has_renderable_data(channel: &RoomEqReportChannel) -> bool {
    channel.initial_curve.is_some()
        || channel.final_curve.is_some()
        || channel.eq_response.is_some()
        || channel.pre_ir.is_some()
        || channel.post_ir.is_some()
        || !channel.eq_passes.is_empty()
        || channel.epa.is_some()
}

fn room_eq_report_curve_from_curve_data(
    curve: &autoeq::roomeq::CurveData,
) -> Option<RoomEqReportCurve> {
    let mut points = Vec::<(f64, f64)>::new();
    let mut phase_values = curve.phase.as_ref().map(|_| Vec::<f64>::new());

    for (idx, (&freq, &spl)) in curve.freq.iter().zip(curve.spl.iter()).enumerate() {
        if freq.is_finite() && freq > 0.0 && spl.is_finite() {
            points.push((freq, spl));
            if let (Some(source_phase), Some(out_phase)) =
                (curve.phase.as_ref(), phase_values.as_mut())
            {
                if let Some(&phase) = source_phase.get(idx)
                    && phase.is_finite()
                {
                    out_phase.push(phase);
                } else {
                    phase_values = None;
                }
            }
        }
    }

    let phase = phase_values.filter(|phase| phase.len() == points.len());
    (!points.is_empty()).then(|| RoomEqReportCurve {
        freq: points.iter().map(|(freq, _)| *freq).collect(),
        spl: points.iter().map(|(_, spl)| *spl).collect(),
        phase,
    })
}

fn room_eq_report_ir_from_waveform(ir: &autoeq::roomeq::IrWaveform) -> Option<RoomEqReportIr> {
    let points: Vec<(f64, f64)> = ir
        .time_ms
        .iter()
        .zip(ir.amplitude.iter())
        .filter_map(|(&time_ms, &amplitude)| {
            (time_ms.is_finite() && amplitude.is_finite()).then_some((time_ms, amplitude))
        })
        .collect();
    (!points.is_empty()).then(|| RoomEqReportIr {
        time_ms: points.iter().map(|(time_ms, _)| *time_ms).collect(),
        amplitude: points.iter().map(|(_, amplitude)| *amplitude).collect(),
    })
}

fn room_eq_report_eq_passes_from_plugins(
    plugins: &[autoeq::roomeq::PluginConfigWrapper],
) -> Vec<RoomEqReportEqPass> {
    plugins
        .iter()
        .filter(|plugin| plugin.plugin_type == "eq")
        .filter_map(|plugin| {
            let label = plugin
                .parameters
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let filters = plugin
                .parameters
                .get("filters")
                .or_else(|| plugin.parameters.get("filter"))
                .and_then(|value| value.as_array())
                .map(|filters| {
                    filters
                        .iter()
                        .filter_map(room_eq_report_filter_from_json)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (!filters.is_empty()).then(|| RoomEqReportEqPass {
                display_name: room_eq_report_pass_display_name(&label).to_string(),
                color: room_eq_report_pass_color(&label),
                label,
                filters,
            })
        })
        .collect()
}

fn room_eq_report_filter_from_json(value: &serde_json::Value) -> Option<RoomEqReportFilter> {
    let filter_type = value
        .get("filter_type")
        .or_else(|| value.get("type"))
        .and_then(|value| value.as_str())
        .unwrap_or("peak")
        .to_string();
    let freq = value
        .get("freq")
        .or_else(|| value.get("frequency"))
        .and_then(|value| value.as_f64())?;
    let q = value
        .get("q")
        .and_then(|value| value.as_f64())
        .unwrap_or(1.0);
    let db_gain = value
        .get("db_gain")
        .or_else(|| value.get("gain_db"))
        .or_else(|| value.get("gain"))
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    Some(RoomEqReportFilter {
        filter_type,
        freq,
        q,
        db_gain,
    })
}

fn room_eq_report_pass_display_name(label: &str) -> &'static str {
    match label {
        "cea2034_speaker_correction" => "Pass 1: Speaker Correction (CEA2034)",
        "broadband" => "Broadband Matching",
        "room_eq_correction" => "Pre-EQ: Room Correction",
        "user_preference" => "Pass 3: User Preference",
        "post_eq" => "Post-EQ: Cleanup (post-crossover)",
        _ => "Room EQ",
    }
}

fn room_eq_report_pass_color(label: &str) -> u32 {
    match label {
        "cea2034_speaker_correction" => 0xffa500,
        "broadband" => 0x4fc3f7,
        "room_eq_correction" => 0x6464ff,
        "user_preference" => 0xb464ff,
        "post_eq" => 0x66bb6a,
        _ => 0x6464ff,
    }
}

fn room_eq_report_epa_score_from_score(
    score: &autoeq::loss::epa::score::EpaScore,
) -> RoomEqReportEpaScore {
    RoomEqReportEpaScore {
        preference: score.preference,
        evaluation: score.evaluation,
        potency: score.potency,
        activity: score.activity,
        sharpness_acum: score.sharpness_acum,
        roughness: score.roughness,
        total_loudness_sone: score.total_loudness_sone,
        loudness_balance: score.loudness_balance,
    }
}

fn room_eq_report_epa_comparison_from_metrics(
    metrics: &autoeq::roomeq::EpaChannelMetrics,
) -> RoomEqReportEpaComparison {
    RoomEqReportEpaComparison {
        pre: room_eq_report_epa_score_from_score(&metrics.pre),
        post: room_eq_report_epa_score_from_score(&metrics.post),
    }
}

fn room_eq_report_epa_preference_avg(
    metadata: &autoeq::roomeq::OptimizationMetadata,
) -> Option<(f64, f64)> {
    let per_channel = metadata.epa_per_channel.as_ref()?;
    if per_channel.is_empty() {
        return None;
    }
    let mut pre_sum = 0.0;
    let mut post_sum = 0.0;
    let mut pre_count = 0.0;
    let mut post_count = 0.0;
    for metrics in per_channel.values() {
        pre_sum += metrics.pre.preference;
        pre_count += 1.0;
        post_sum += metrics.post.preference;
        post_count += 1.0;
    }
    (pre_count > 0.0 && post_count > 0.0).then_some((pre_sum / pre_count, post_sum / post_count))
}

fn room_eq_report_bass_management_from_report(
    report: &autoeq::roomeq::BassManagementReport,
) -> RoomEqReportBassManagement {
    let routes = report
        .routing_graph
        .as_ref()
        .map(|graph| {
            graph
                .routes
                .iter()
                .map(|route| RoomEqReportBassRoute {
                    source_channel: route.source_channel.clone(),
                    destination: route.destination.clone(),
                    route_kind: route.route_kind.clone(),
                    group_id: route.group_id.clone(),
                    crossover_type: route.crossover_type.clone(),
                    high_pass_hz: route.high_pass_hz,
                    low_pass_hz: route.low_pass_hz,
                    gain_db: route.gain_db,
                    matrix_gain: route.matrix_gain,
                    delay_ms: route.delay_ms,
                    polarity_inverted: route.polarity_inverted,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut physical_outputs: BTreeSet<String> = routes
        .iter()
        .filter(|route| {
            matches!(
                route.route_kind.as_str(),
                "redirected_bass_lowpass_to_sub" | "lfe_lowpass_to_sub"
            )
        })
        .map(|route| route.destination.clone())
        .collect();
    if physical_outputs.is_empty() && !report.physical_sub_output.is_empty() {
        physical_outputs.insert(report.physical_sub_output.clone());
    }

    let mut advisories: BTreeSet<String> = BTreeSet::new();
    if !report.advisory.is_empty() && report.advisory != "ok" {
        advisories.insert(report.advisory.clone());
    }
    if let Some(graph) = report.routing_graph.as_ref() {
        advisories.extend(
            graph
                .advisories
                .iter()
                .filter(|item| !item.is_empty() && item.as_str() != "ok")
                .cloned(),
        );
    }

    let groups = if report.groups.is_empty() {
        report
            .optimization
            .as_ref()
            .map(|opt| opt.group_results.as_slice())
            .unwrap_or_default()
    } else {
        report.groups.as_slice()
    };
    let sub_outputs = if report.sub_outputs.is_empty() {
        report
            .optimization
            .as_ref()
            .map(|opt| opt.sub_output_results.as_slice())
            .unwrap_or_default()
    } else {
        report.sub_outputs.as_slice()
    };

    RoomEqReportBassManagement {
        enabled: report.enabled,
        crossover_type: report.crossover_type.clone(),
        crossover_frequency_hz: report.crossover_frequency_hz,
        lfe_playback_gain_db: report.lfe_playback_gain_db,
        applied_sub_gain_db: report.applied_sub_gain_db,
        input_channels: report
            .routing_graph
            .as_ref()
            .map(|graph| graph.input_channels.clone())
            .unwrap_or_default(),
        output_channels: report
            .routing_graph
            .as_ref()
            .map(|graph| graph.output_channels.clone())
            .unwrap_or_default(),
        physical_outputs: physical_outputs.into_iter().collect(),
        route_count: routes.len(),
        advisory: report.advisory.clone(),
        advisories: advisories.into_iter().collect(),
        routes,
        groups: groups
            .iter()
            .map(|group| RoomEqReportBassGroup {
                group_id: group.group_id.clone(),
                roles: group.roles.clone(),
                crossover_type: group.crossover_type.clone(),
                selected_crossover_hz: group.selected_crossover_hz,
                main_delay_ms: group.main_delay_ms,
                bass_route_delay_ms: group.bass_route_delay_ms,
                polarity_inverted: group.polarity_inverted,
                trim_db: group.trim_db,
                advisories: group
                    .advisories
                    .iter()
                    .filter(|item| !item.is_empty() && item.as_str() != "ok")
                    .cloned()
                    .collect(),
            })
            .collect(),
        sub_outputs: sub_outputs
            .iter()
            .map(|output| RoomEqReportBassSubOutput {
                output_role: output.output_role.clone(),
                strategy_source: output.strategy_source.clone(),
                gain_db: output.gain_db,
                delay_ms: output.delay_ms,
                polarity_inverted: output.polarity_inverted,
                headroom_contribution_db: output.headroom_contribution_db,
            })
            .collect(),
        headroom: report
            .headroom_simulation
            .as_ref()
            .map(|headroom| RoomEqReportBassHeadroom {
                model: headroom.model.clone(),
                headroom_margin_db: headroom.headroom_margin_db,
                pass: headroom.pass,
                margin_db: headroom.margin_db,
                worst_frequency_hz: headroom.worst_frequency_hz,
                per_output: headroom
                    .per_output
                    .iter()
                    .map(|output| RoomEqReportBassHeadroomOutput {
                        output_role: output.output_role.clone(),
                        rms_bus_gain_db: output.rms_bus_gain_db,
                        coherent_peak_gain_db: output.coherent_peak_gain_db,
                        lfe_contribution_db: output.lfe_contribution_db,
                        margin_db: output.margin_db,
                        worst_frequency_hz: output.worst_frequency_hz,
                        pass: output.pass,
                    })
                    .collect(),
            }),
    }
}

pub fn room_eq_report_y_range<'a>(
    curves: impl IntoIterator<Item = Option<&'a RoomEqReportCurve>>,
) -> (f64, f64) {
    let mut max_spl = f64::NEG_INFINITY;
    for curve in curves.into_iter().flatten() {
        for &spl in &curve.spl {
            if spl.is_finite() {
                max_spl = max_spl.max(spl);
            }
        }
    }

    if !max_spl.is_finite() {
        return (-20.0, 30.0);
    }

    let upper = (max_spl / 5.0).ceil() * 5.0;
    (upper - 50.0, upper)
}

pub fn room_eq_report_eq_y_range<'a>(
    curves: impl IntoIterator<Item = Option<&'a RoomEqReportCurve>>,
) -> (f64, f64) {
    let mut min_spl = f64::INFINITY;
    let mut max_spl = f64::NEG_INFINITY;
    for curve in curves.into_iter().flatten() {
        for &spl in &curve.spl {
            if spl.is_finite() {
                min_spl = min_spl.min(spl);
                max_spl = max_spl.max(spl);
            }
        }
    }

    if !min_spl.is_finite() || !max_spl.is_finite() {
        return (-15.0, 15.0);
    }

    let upper = ((max_spl / 5.0).ceil() * 5.0 + 5.0).min(20.0);
    let lower = ((min_spl / 5.0).floor() * 5.0 - 5.0).max(-20.0);
    if upper <= lower {
        (lower, lower + 1.0)
    } else {
        (lower, upper)
    }
}

pub(crate) fn render_room_eq_report_summary(
    d: Ds,
    report: &RoomEqReportData,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    let improvement = report
        .pre_score
        .zip(report.post_score)
        .map(|(pre, post)| pre - post);

    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new("Optimization Summary")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    div()
                        .grid()
                        .grid_cols(4)
                        .gap(d.gap_md)
                        .child(render_room_eq_stat_item("Version", &report.version, theme))
                        .child(render_room_eq_stat_item(
                            "Algorithm",
                            report.algorithm.as_deref().unwrap_or("N/A"),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Loss function",
                            report.loss_type.as_deref().unwrap_or("N/A"),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Iterations",
                            &report
                                .iterations
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "N/A".to_string()),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Score Before",
                            &fmt_optional_number(report.pre_score, "{:.2}"),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Score After",
                            &fmt_optional_number(report.post_score, "{:.2}"),
                            theme,
                        ))
                        .child(render_room_eq_colored_stat_item(
                            "Improvement",
                            &fmt_optional_number(improvement, "{:.2}"),
                            improvement.map(|v| v >= 0.0).unwrap_or(true),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Timestamp",
                            report.timestamp.as_deref().unwrap_or("N/A"),
                            theme,
                        )),
                )
                .when_some(report.epa_preference_avg, |el, (pre, post)| {
                    let delta = post - pre;
                    el.child(render_room_eq_colored_stat_item(
                        "EPA Preference (avg)",
                        &format!("{pre:.2} -> {post:.2} ({delta:+.2})"),
                        delta >= 0.0,
                        theme,
                    ))
                })
                .when_some(report.fir_masking.as_ref(), |el, fm| {
                    el.child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap(d.gap_md)
                            .child(render_room_eq_stat_item(
                                "FIR pre-ring audible",
                                &fmt_optional_number(fm.pre_audible_db, "{:.1} dB"),
                                theme,
                            ))
                            .child(render_room_eq_stat_item(
                                "FIR post-ring audible",
                                &fmt_optional_number(fm.post_audible_db, "{:.1} dB"),
                                theme,
                            ))
                            .child(render_room_eq_stat_item(
                                "FIR masking penalty",
                                &fmt_optional_number(fm.penalty, "{:.3}"),
                                theme,
                            )),
                    )
                }),
        )
        .into_any_element()
}

pub(crate) fn render_room_eq_bass_management_report(
    d: Ds,
    bass: &RoomEqReportBassManagement,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        // Keep the green-accent nuance from the previous design: bass
        // management succeeded → tint the border with `theme.success`.
        .border(theme.success)
        .header(
            Text::new("Bass Management")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    // The summary grid carries seven short scalar values
                    // — letting it stretch to 100 % of a wide card creates
                    // 200-px-wide cells of mostly whitespace. Cap the
                    // width so the grid sits compactly at the left edge.
                    // intentional: stat-grid width is layout-driven px.
                    div()
                        .flex_none()
                        .max_w(px(720.0))
                        .grid()
                        .grid_cols(4)
                        .gap(d.gap_md)
                        .child(render_room_eq_stat_item(
                            "Enabled",
                            if bass.enabled { "yes" } else { "no" },
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Crossover",
                            &format!(
                                "{} @ {}",
                                bass.crossover_type,
                                fmt_optional_hz(bass.crossover_frequency_hz)
                            ),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "LFE gain",
                            &fmt_db(bass.lfe_playback_gain_db),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Shared sub gain",
                            &fmt_optional_db(bass.applied_sub_gain_db),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Physical bass outputs",
                            &if bass.physical_outputs.is_empty() {
                                "-".to_string()
                            } else {
                                bass.physical_outputs.join(", ")
                            },
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Route count",
                            &bass.route_count.to_string(),
                            theme,
                        ))
                        .child(render_room_eq_stat_item(
                            "Graph mode",
                            if bass.route_count > 0 {
                                "route branches"
                            } else {
                                "linear / none"
                            },
                            theme,
                        )),
                )
                .when(!bass.advisories.is_empty(), |el| {
                    el.child(
                        Text::new(bass.advisories.join("; "))
                            .size(TextSize::Xs)
                            .color(theme.warning),
                    )
                })
                .when(!bass.routes.is_empty() || bass.headroom.is_some(), |el| {
                    // Wrap charts in a flex-row that owns its own
                    // x AND y gap so the headroom plot doesn't overlap
                    // the routing plot when the card narrows enough to
                    // force the second chart onto a new line. `.gap(_)`
                    // in GPUI sets both axes, which is what we want here.
                    el.child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .justify_start()
                            .gap(d.gap_md)
                            .when(!bass.routes.is_empty(), |row| {
                                row.child(render_room_eq_bass_routing_chart(bass, theme))
                            })
                            .when_some(bass.headroom.as_ref(), |row, headroom| {
                                row.child(render_room_eq_bass_headroom_chart(headroom, theme))
                            }),
                    )
                })
                .when(!bass.routes.is_empty(), |el| {
                    el.child(render_room_eq_bass_routes_table(d, bass, theme))
                })
                .when(!bass.groups.is_empty(), |el| {
                    el.child(render_room_eq_bass_groups_table(d, bass, theme))
                })
                .when(!bass.sub_outputs.is_empty(), |el| {
                    el.child(render_room_eq_bass_sub_outputs_table(d, bass, theme))
                }),
        )
        .into_any_element()
}

fn render_room_eq_stat_item(
    label: &str,
    value: &str,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            Text::new(label.to_string())
                .size(TextSize::Xs)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary),
        )
        .child(
            Text::new(value.to_string())
                .size(TextSize::Sm)
                .color(theme.text_primary),
        )
        .into_any_element()
}

fn render_room_eq_colored_stat_item(
    label: &str,
    value: &str,
    positive: bool,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            Text::new(label.to_string())
                .size(TextSize::Xs)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary),
        )
        .child(
            Text::new(value.to_string())
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold)
                .color(if positive { theme.success } else { theme.error }),
        )
        .into_any_element()
}

fn render_room_eq_bass_routing_chart(
    bass: &RoomEqReportBassManagement,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    use d3rs::sankey::{SankeyLayout, SankeyLinkInput};
    use d3rs::shape::path::PathBuilder as D3PathBuilder;

    let mut node_names = Vec::<String>::new();
    let mut add_node = |name: String| {
        if !node_names.iter().any(|existing| existing == &name) {
            node_names.push(name);
        }
    };

    let display_routes = &bass.routes;
    let links: Vec<SankeyLinkInput> = display_routes
        .iter()
        .map(|route| {
            let source = format!("in: {}", route.source_channel);
            let target = format!("out: {}", route.destination);
            add_node(source.clone());
            add_node(target.clone());
            SankeyLinkInput {
                source,
                target,
                value: route.matrix_gain.abs().max(0.05),
            }
        })
        .collect();

    if links.is_empty() {
        return render_empty_state(IconName::AudioWaveform, "No routing graph data", theme);
    }

    let width = 350.0;
    let height = (220.0 + 12.0 * display_routes.len() as f64).clamp(260.0, 350.0);
    let result = SankeyLayout::new()
        .width(width)
        .height(height)
        .margins(12.0, 18.0, 12.0, 18.0)
        .node_width(16.0)
        .node_padding(12.0)
        .compute(&node_names, &links);

    let mut paths = Vec::<d3rs::shape::path::Path>::new();
    let mut colors = Vec::<Hsla>::new();
    for (link, route) in result.links.iter().zip(display_routes.iter()) {
        let source = &result.nodes[link.source];
        let target = &result.nodes[link.target];
        let sx = source.x1;
        let tx = target.x0;
        let cx = (sx + tx) / 2.0;
        let half_width = link.width / 2.0;
        paths.push(
            D3PathBuilder::new()
                .move_to(sx, link.y0 - half_width)
                .cubic_curve_to(
                    cx,
                    link.y0 - half_width,
                    cx,
                    link.y1 - half_width,
                    tx,
                    link.y1 - half_width,
                )
                .line_to(tx, link.y1 + half_width)
                .cubic_curve_to(
                    cx,
                    link.y1 + half_width,
                    cx,
                    link.y0 + half_width,
                    sx,
                    link.y0 + half_width,
                )
                .close_path()
                .build(),
        );
        colors.push(room_eq_route_color(&route.route_kind).opacity(0.52));
    }

    for node in &result.nodes {
        paths.push(
            D3PathBuilder::new()
                .move_to(node.x0, node.y0)
                .line_to(node.x1, node.y0)
                .line_to(node.x1, node.y1)
                .line_to(node.x0, node.y1)
                .close_path()
                .build(),
        );
        colors.push(Hsla::from(theme.text_secondary).opacity(0.86));
    }

    let max_layer = result
        .nodes
        .iter()
        .map(|node| node.layer)
        .max()
        .unwrap_or_default();
    let labels: Vec<(String, f64, f64, bool)> = result
        .nodes
        .iter()
        .filter(|node| node.y1 - node.y0 > 5.0)
        .map(|node| {
            let is_right = node.layer > max_layer / 2;
            let x = if is_right {
                node.x0 - 6.0
            } else {
                node.x1 + 6.0
            };
            let y = (node.y0 + node.y1) / 2.0;
            (node.id.clone(), x, y, is_right)
        })
        .collect();

    let chart = div()
        .relative()
        .w(px(width as f32))
        .h(px(height as f32))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .rounded(px(4.0))
        .overflow_hidden()
        .child(
            canvas(
                move |bounds, _, _| {
                    let bounds_width: f32 = bounds.size.width.into();
                    let bounds_height: f32 = bounds.size.height.into();
                    let scale_x = bounds_width / width as f32;
                    let scale_y = bounds_height / height as f32;
                    paths
                        .iter()
                        .filter_map(|path| {
                            room_eq_d3rs_path_to_gpui(path, bounds, 0.0, 0.0, scale_x, scale_y)
                        })
                        .collect::<Vec<_>>()
                },
                move |_bounds, paths, window, _| {
                    for (idx, path) in paths.into_iter().enumerate() {
                        if let Some(color) = colors.get(idx) {
                            window.paint_path(path, *color);
                        }
                    }
                },
            )
            .size_full(),
        )
        .children(labels.into_iter().map(|(label, x, y, is_right)| {
            let mut label_el = div()
                .absolute()
                .top(px(y as f32 - 6.0))
                .text_size(px(10.0))
                .line_height(px(11.0))
                .text_color(theme.text_primary);
            if is_right {
                label_el = label_el.right(px((width - x) as f32));
            } else {
                label_el = label_el.left(px(x as f32));
            }
            label_el.child(label)
        }))
        .into_any_element();

    div()
        .w(px(width as f32))
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("Bass Management Routing Graph")
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(theme.text_primary),
                )
                .child(chart),
        )
        .into_any_element()
}

fn render_room_eq_bass_headroom_chart(
    headroom: &RoomEqReportBassHeadroom,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    use gpui_px::{BarTheme, LegendPosition, bar};

    let labels: Vec<String> = headroom
        .per_output
        .iter()
        .map(|output| output.output_role.clone())
        .collect();
    let rms: Vec<f64> = headroom
        .per_output
        .iter()
        .map(|output| output.rms_bus_gain_db)
        .collect();
    let peak: Vec<f64> = headroom
        .per_output
        .iter()
        .map(|output| output.coherent_peak_gain_db)
        .collect();
    let lfe: Vec<f64> = headroom
        .per_output
        .iter()
        .map(|output| output.lfe_contribution_db)
        .collect();
    let bar_theme = BarTheme {
        plot_background: theme.surface,
        title_color: theme.text_primary,
        legend_text_color: theme.text_secondary,
    };

    let chart = bar(&labels, &rms)
        .label("RMS programme gain")
        .color(0x4a90d9)
        .add_series(&peak, Some("Coherent peak gain"), 0xe74c3c, 0.78)
        .add_series(&lfe, Some("LFE contribution"), 0xe67e22, 0.72)
        .legend_position(LegendPosition::Bottom)
        .theme(bar_theme)
        .size(350.0, 260.0)
        .build()
        .map(|chart| chart.into_any_element())
        .unwrap_or_else(|e| {
            log::warn!("RoomEQ bass headroom chart build failed: {e:?}");
            render_empty_state(
                IconName::AudioWaveform,
                "Unable to render headroom chart",
                theme,
            )
        });

    div()
        .w(px(350.0))
        .child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new(format!(
                        "Bass Bus Headroom Simulation ({}, {}, margin {}, worst {})",
                        headroom.model,
                        if headroom.pass { "pass" } else { "fail" },
                        fmt_db(headroom.margin_db),
                        fmt_hz(headroom.worst_frequency_hz)
                    ))
                    .weight(TextWeight::Semibold)
                    .size(TextSize::Xs)
                    .color(theme.text_primary),
                )
                .child(chart),
        )
        .into_any_element()
}

fn render_room_eq_bass_routes_table(
    d: Ds,
    bass: &RoomEqReportBassManagement,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    render_room_eq_table(
        d,
        "Bass Routes",
        &[
            "Source",
            "Destination",
            "Kind",
            "XO",
            "Gain",
            "Delay",
            "Polarity",
        ],
        bass.routes.iter().map(|route| {
            vec![
                route.source_channel.clone(),
                route.destination.clone(),
                route_display_name(&route.route_kind).to_string(),
                format!(
                    "{} @ {}",
                    route.crossover_type,
                    fmt_optional_hz(route.high_pass_hz.or(route.low_pass_hz))
                ),
                fmt_db(route.gain_db),
                fmt_ms(route.delay_ms),
                if route.polarity_inverted {
                    "inverted"
                } else {
                    "normal"
                }
                .to_string(),
            ]
        }),
        theme,
    )
}

fn render_room_eq_bass_groups_table(
    d: Ds,
    bass: &RoomEqReportBassManagement,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    render_room_eq_table(
        d,
        "Per-Speaker-Group Crossovers",
        &[
            "Group",
            "Roles",
            "Type",
            "Selected XO",
            "Main delay",
            "Bass delay",
            "Invert",
            "Trim",
            "Advisories",
        ],
        bass.groups.iter().map(|group| {
            vec![
                group.group_id.clone(),
                group.roles.join(", "),
                group.crossover_type.clone(),
                fmt_optional_hz(group.selected_crossover_hz),
                fmt_ms(group.main_delay_ms),
                fmt_ms(group.bass_route_delay_ms),
                if group.polarity_inverted { "yes" } else { "no" }.to_string(),
                fmt_db(group.trim_db),
                if group.advisories.is_empty() {
                    "-".to_string()
                } else {
                    group.advisories.join(", ")
                },
            ]
        }),
        theme,
    )
}

fn render_room_eq_bass_sub_outputs_table(
    d: Ds,
    bass: &RoomEqReportBassManagement,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    render_room_eq_table(
        d,
        "Physical Bass Outputs",
        &["Output", "Strategy", "Gain", "Delay", "Invert", "Headroom"],
        bass.sub_outputs.iter().map(|output| {
            vec![
                output.output_role.clone(),
                output.strategy_source.clone(),
                fmt_db(output.gain_db),
                fmt_ms(output.delay_ms),
                if output.polarity_inverted {
                    "yes"
                } else {
                    "no"
                }
                .to_string(),
                fmt_db(output.headroom_contribution_db),
            ]
        }),
        theme,
    )
}

fn render_room_eq_table(
    d: Ds,
    title: &str,
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    let rows: Vec<Vec<String>> = rows.into_iter().collect();
    let cols = headers.len().min(u16::MAX as usize) as u16;
    // Tables are sized to content with a hard cap so they don't stretch
    // to fill the parent in wide windows. ~720px fits 6–9 columns
    // comfortably.
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(Text::label(title.to_string()))
        .child(
            div()
                .flex_none()
                .max_w(px(720.0)) // intentional: table widths are layout-driven
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme.border)
                .rounded(d.r_md)
                .overflow_hidden()
                .child(
                    div()
                        .grid()
                        .grid_cols(cols)
                        .bg(theme.background_secondary)
                        .children(headers.iter().map(|header| {
                            div()
                                .px(d.pad_x)
                                .py(d.pad_y_half)
                                .child(Text::label((*header).to_string()))
                        })),
                )
                .children(rows.into_iter().map(|row| {
                    div()
                        .grid()
                        .grid_cols(cols)
                        .children(row.into_iter().map(|cell| {
                            div()
                                .px(d.pad_x)
                                .py(d.pad_y_half)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(Text::new(cell).size(TextSize::Xs).color(theme.text_primary))
                        }))
                })),
        )
        .into_any_element()
}

fn route_display_name(kind: &str) -> &'static str {
    match kind {
        "main_highpass_to_self" => "main high-pass",
        "main_highpass" => "main high-pass",
        "redirected_bass_lowpass_to_sub" => "redirected bass",
        "lfe_lowpass_to_sub" => "LFE to sub",
        "full_range" => "full range",
        _ => "route",
    }
}

fn room_eq_route_color(kind: &str) -> Hsla {
    match kind {
        "main_highpass_to_self" | "main_highpass" => Hsla::from(rgba_from_u32(0x4a90d9)),
        "redirected_bass_lowpass_to_sub" => Hsla::from(rgba_from_u32(0x2ecc71)),
        "lfe_lowpass_to_sub" => Hsla::from(rgba_from_u32(0xe67e22)),
        _ => Hsla::from(rgba_from_u32(0x7f8c8d)),
    }
}

fn room_eq_d3rs_path_to_gpui(
    path: &d3rs::shape::path::Path,
    bounds: Bounds<Pixels>,
    offset_x: f32,
    offset_y: f32,
    scale_x: f32,
    scale_y: f32,
) -> Option<Path<Pixels>> {
    use d3rs::shape::path::PathCommand;

    let mut builder = PathBuilder::fill();
    let origin = bounds.origin;
    let mut current_x = 0.0_f32;
    let mut current_y = 0.0_f32;
    let tx = |x: f64| x as f32 * scale_x + offset_x;
    let ty = |y: f64| y as f32 * scale_y + offset_y;

    for command in path.commands() {
        match command {
            PathCommand::MoveTo { x, y } => {
                current_x = tx(*x);
                current_y = ty(*y);
                builder.move_to(origin + point(px(current_x), px(current_y)));
            }
            PathCommand::LineTo { x, y } => {
                current_x = tx(*x);
                current_y = ty(*y);
                builder.line_to(origin + point(px(current_x), px(current_y)));
            }
            PathCommand::HorizontalLineTo { x } => {
                current_x = tx(*x);
                builder.line_to(origin + point(px(current_x), px(current_y)));
            }
            PathCommand::VerticalLineTo { y } => {
                current_y = ty(*y);
                builder.line_to(origin + point(px(current_x), px(current_y)));
            }
            PathCommand::CubicCurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (x1, y1) = (tx(*x1), ty(*y1));
                let (x2, y2) = (tx(*x2), ty(*y2));
                let (x, y) = (tx(*x), ty(*y));
                for step in 1..=16 {
                    let t = step as f32 / 16.0;
                    let u = 1.0 - t;
                    let px_value = u * u * u * current_x
                        + 3.0 * u * u * t * x1
                        + 3.0 * u * t * t * x2
                        + t * t * t * x;
                    let py_value = u * u * u * current_y
                        + 3.0 * u * u * t * y1
                        + 3.0 * u * t * t * y2
                        + t * t * t * y;
                    builder.line_to(origin + point(px(px_value), px(py_value)));
                }
                current_x = x;
                current_y = y;
            }
            PathCommand::ClosePath => {
                builder.close();
            }
            _ => {}
        }
    }

    builder.build().ok()
}

fn fmt_optional_number(value: Option<f64>, _fmt: &str) -> String {
    value
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string())
}

fn fmt_db(value: f64) -> String {
    format!("{value:+.2} dB")
}

fn fmt_optional_db(value: Option<f64>) -> String {
    value.map(fmt_db).unwrap_or_else(|| "-".to_string())
}

fn fmt_hz(value: f64) -> String {
    format!("{value:.1} Hz")
}

fn fmt_optional_hz(value: Option<f64>) -> String {
    value.map(fmt_hz).unwrap_or_else(|| "-".to_string())
}

fn fmt_ms(value: f64) -> String {
    format!("{value:.3} ms")
}

pub(crate) fn render_room_eq_report_overview(
    d: Ds,
    report: &RoomEqReportData,
    theme: &crate::theme::Theme,
    original_settings: RoomEqReviewGraphSettings,
    eq_settings: RoomEqReviewGraphSettings,
    corrected_settings: RoomEqReviewGraphSettings,
    original_controls: Option<gpui::AnyElement>,
    eq_controls: Option<gpui::AnyElement>,
    corrected_controls: Option<gpui::AnyElement>,
    window_width: f32,
) -> impl IntoElement {
    let mut original_series = Vec::new();
    let mut corrected_series = Vec::new();
    let mut eq_series = Vec::new();

    for (idx, channel) in report.channels.iter().enumerate() {
        let color = ROOM_EQ_CHANNEL_COLORS[idx % ROOM_EQ_CHANNEL_COLORS.len()];
        if channel.driver_initial_curves.is_empty() {
            if let Some(curve) = channel.initial_curve.clone() {
                original_series.push(RoomEqChartSeries {
                    channel_name: Some(channel.name.clone()),
                    label: format!("Original: {}", channel.name),
                    curve,
                    color,
                    stroke_width: 2.0,
                    opacity: 1.0,
                });
            }
        } else {
            for (driver_idx, driver) in channel.driver_initial_curves.iter().enumerate() {
                original_series.push(RoomEqChartSeries {
                    channel_name: Some(channel.name.clone()),
                    label: format!("Original: {}/{}", channel.name, driver.driver_name),
                    curve: driver.curve.clone(),
                    color,
                    stroke_width: 2.0,
                    opacity: ROOM_EQ_DRIVER_OPACITIES[driver_idx % ROOM_EQ_DRIVER_OPACITIES.len()],
                });
            }
        }

        if let Some(curve) = channel.eq_response.clone() {
            eq_series.push(RoomEqChartSeries {
                channel_name: Some(channel.name.clone()),
                label: format!("EQ: {}", channel.name),
                curve,
                color,
                stroke_width: 2.0,
                opacity: 1.0,
            });
        }

        if let Some(curve) = channel.final_curve.clone() {
            corrected_series.push(RoomEqChartSeries {
                channel_name: Some(channel.name.clone()),
                label: format!("Corrected: {}", channel.name),
                curve,
                color,
                stroke_width: 2.0,
                opacity: 1.0,
            });
        }
    }
    add_room_eq_corrected_lfe_sums(report, &mut corrected_series);

    // Always render the three overview plots in a 3-column grid so each
    // plot occupies 1/3 of the card width. A Right legend would steal
    // horizontal room inside a 1/3 column, so use the Bottom legend
    // position unconditionally.
    //
    // gpui-px charts draw onto a fixed-size canvas (no flex-fill) so we
    // size them from the actual window width at render time. Chrome
    // budget = page side padding (≈48) + card horizontal padding (≈32)
    // + 2 × grid gap (≈24). Floor at 280 px so charts stay legible on
    // narrow windows.
    // intentional-file: chart canvas dimensions are layout-driven px.
    let overview_chart_width = ((window_width - 104.0) / 3.0).clamp(280.0, 900.0);
    let overview_chart_height = (overview_chart_width * 0.62).clamp(220.0, 520.0);
    let chart_size = (overview_chart_width, overview_chart_height);
    let original_chart = render_room_eq_curve_chart(
        "All Original Curves",
        original_series,
        theme,
        original_settings,
        (20.0, 20000.0),
        (-40.0, 10.0),
        "SPL (dB)",
        "room-eq-overview-original",
        None,
        original_controls,
        chart_size,
        LegendPosition::Bottom,
    );
    let eq_chart = render_room_eq_curve_chart(
        "All EQ Responses",
        eq_series,
        theme,
        eq_settings,
        (20.0, 20000.0),
        (-15.0, 15.0),
        "EQ (dB)",
        "room-eq-overview-eq",
        None,
        eq_controls,
        chart_size,
        LegendPosition::Bottom,
    );
    let corrected_chart = render_room_eq_curve_chart(
        "All Corrected Curves",
        corrected_series,
        theme,
        corrected_settings,
        (20.0, 20000.0),
        (-40.0, 10.0),
        "SPL (dB)",
        "room-eq-overview-corrected",
        None,
        corrected_controls,
        chart_size,
        LegendPosition::Bottom,
    );
    let charts = div()
        .grid()
        .grid_cols(3)
        .gap(d.gap_md)
        .child(original_chart)
        .child(eq_chart)
        .child(corrected_chart)
        .into_any_element();

    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new("All Channels Overview")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(VStack::new().spacing(StackSpacing::Md).child(charts))
        .into_any_element()
}

fn add_room_eq_corrected_lfe_sums(
    report: &RoomEqReportData,
    corrected_series: &mut Vec<RoomEqChartSeries>,
) {
    let Some(lfe_channel) = report.channels.iter().find(|channel| {
        is_room_eq_sub_or_lfe_channel(&channel.name) && channel.final_curve.is_some()
    }) else {
        return;
    };
    let Some(lfe_curve) = lfe_channel.final_curve.as_ref() else {
        return;
    };

    for target in ["L", "R", "C"] {
        let Some((idx, channel)) = report
            .channels
            .iter()
            .enumerate()
            .find(|(_, channel)| room_eq_is_named_main_channel(&channel.name, target))
        else {
            continue;
        };
        let Some(main_curve) = channel.final_curve.as_ref() else {
            continue;
        };
        let summed = sum_room_eq_report_curves_db(main_curve, lfe_curve);
        if summed.is_empty() {
            continue;
        }
        corrected_series.push(RoomEqChartSeries {
            channel_name: Some(channel.name.clone()),
            label: format!("Corrected: {}+{}", channel.name, lfe_channel.name),
            curve: summed,
            color: ROOM_EQ_CHANNEL_COLORS[idx % ROOM_EQ_CHANNEL_COLORS.len()],
            stroke_width: 2.6,
            opacity: 0.72,
        });
    }
}

fn room_eq_is_named_main_channel(channel_name: &str, target: &str) -> bool {
    let normalized = channel_name.trim().to_ascii_uppercase();
    match target {
        "L" => matches!(normalized.as_str(), "L" | "LEFT"),
        "R" => matches!(normalized.as_str(), "R" | "RIGHT"),
        "C" => matches!(normalized.as_str(), "C" | "CENTER" | "CENTRE"),
        _ => false,
    }
}

fn sum_room_eq_report_curves_db(
    main: &RoomEqReportCurve,
    sub: &RoomEqReportCurve,
) -> RoomEqReportCurve {
    let main_points: Vec<(f64, f64)> = main
        .freq
        .iter()
        .zip(main.spl.iter())
        .map(|(&freq, &spl)| (freq, spl))
        .collect();
    let sub_points: Vec<(f64, f64)> = sub
        .freq
        .iter()
        .zip(sub.spl.iter())
        .map(|(&freq, &spl)| (freq, spl))
        .collect();
    let main_phase = room_eq_phase_points(main);
    let sub_phase = room_eq_phase_points(sub);
    let points = sum_room_eq_responses_db(
        &main_points,
        &sub_points,
        main_phase.as_deref(),
        sub_phase.as_deref(),
    );
    RoomEqReportCurve {
        freq: points.iter().map(|(freq, _)| *freq).collect(),
        spl: points.iter().map(|(_, spl)| *spl).collect(),
        phase: None,
    }
}

fn room_eq_phase_points(curve: &RoomEqReportCurve) -> Option<Vec<(f64, f64)>> {
    let phase = curve.phase.as_ref()?;
    (phase.len() == curve.freq.len()).then(|| {
        curve
            .freq
            .iter()
            .zip(phase.iter())
            .map(|(&freq, &phase)| (freq, phase))
            .collect()
    })
}

pub(crate) fn render_room_eq_report_channel(
    d: Ds,
    channel: &RoomEqReportChannel,
    theme: &crate::theme::Theme,
    full_settings: RoomEqReviewGraphSettings,
    zoom_settings: RoomEqReviewGraphSettings,
    eq_settings: RoomEqReviewGraphSettings,
    full_controls: Option<gpui::AnyElement>,
    zoom_controls: Option<gpui::AnyElement>,
    eq_controls: Option<gpui::AnyElement>,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    window_width: f32,
) -> impl IntoElement {
    let reference = channel
        .final_curve
        .as_ref()
        .or(channel.initial_curve.as_ref());
    let zoom_center = reference
        .map(|curve| room_eq_average_spl_in_range(curve, 20.0, 1200.0))
        .unwrap_or(0.0);

    let mut full_series = Vec::new();
    if let Some(curve) = channel.initial_curve.clone() {
        full_series.push(RoomEqChartSeries {
            channel_name: Some(channel.name.clone()),
            label: "Before EQ".to_string(),
            curve,
            color: 0xff6464,
            stroke_width: 2.0,
            opacity: 0.8,
        });
    }
    if let Some(curve) = channel.final_curve.clone() {
        full_series.push(RoomEqChartSeries {
            channel_name: Some(channel.name.clone()),
            label: "After EQ".to_string(),
            curve,
            color: 0x64c864,
            stroke_width: 2.0,
            opacity: 0.9,
        });
    }

    let mut eq_series = Vec::new();
    if let Some(curve) = channel.eq_response.clone() {
        eq_series.push(RoomEqChartSeries {
            channel_name: Some(channel.name.clone()),
            label: format!("EQ: {}", channel.name),
            curve,
            color: 0x1f77b4,
            stroke_width: 2.0,
            opacity: 1.0,
        });
    }

    // Decide per-channel layout from the actual viewport width: when
    // the inner column is wide enough to host two charts side by side
    // we use a 2-column grid, otherwise we stack to keep each chart
    // legible. gpui-px charts are fixed-canvas so we size them from
    // window_width with sensible floors/ceilings.
    // intentional-file: chart canvas dimensions are layout-driven px.
    let two_col = window_width >= ROOM_EQ_REVIEW_WIDE_BREAKPOINT_PX;
    let curve_chart_width = if two_col {
        ((window_width - 120.0) / 2.0).clamp(420.0, 1100.0)
    } else {
        (window_width - 120.0).clamp(640.0, 1400.0)
    };
    let curve_chart_height = (curve_chart_width * 0.5).clamp(260.0, 540.0);
    let curve_chart_size = (curve_chart_width, curve_chart_height);
    let ir_chart_size = (curve_chart_width, (curve_chart_height * 0.85).max(220.0));
    let full_chart = render_room_eq_curve_chart(
        "Full Range",
        full_series.clone(),
        theme,
        full_settings,
        (20.0, 20000.0),
        (-40.0, 10.0),
        "SPL (dB)",
        "room-eq-channel-full",
        interactive_state,
        full_controls,
        curve_chart_size,
        LegendPosition::Right,
    );
    let zoom_chart = render_room_eq_curve_chart(
        "Zoomed 20-1200 Hz",
        full_series,
        theme,
        zoom_settings,
        (20.0, 1200.0),
        (zoom_center - 10.0, zoom_center + 10.0),
        "SPL (dB)",
        "room-eq-channel-zoom",
        None,
        zoom_controls,
        curve_chart_size,
        LegendPosition::Right,
    );
    let eq_chart = render_room_eq_curve_chart(
        "EQ Response",
        eq_series,
        theme,
        eq_settings,
        (20.0, 20000.0),
        (-15.0, 15.0),
        "EQ (dB)",
        "room-eq-channel-eq",
        None,
        eq_controls,
        curve_chart_size,
        LegendPosition::Right,
    );
    let has_ir = channel.pre_ir.is_some() || channel.post_ir.is_some();
    let channel_charts = if two_col {
        div()
            .grid()
            .grid_cols(2)
            .gap(d.section)
            .child(full_chart)
            .child(zoom_chart)
            .child(eq_chart)
            .when(has_ir, |el| {
                el.child(render_room_eq_ir_chart(channel, theme, ir_chart_size))
            })
            .into_any_element()
    } else {
        VStack::new()
            .spacing(StackSpacing::Md)
            .child(full_chart)
            .child(zoom_chart)
            .child(eq_chart)
            .when(has_ir, |el| {
                el.child(render_room_eq_ir_chart(channel, theme, ir_chart_size))
            })
            .into_any_element()
    };

    // EPA scores and EQ filter passes are intentionally NOT rendered
    // here: both have been lifted to their own top-level cards in
    // `render_room_eq_review` so the user can scan EPA per channel and
    // copy filter values for every channel without having to switch the
    // channel tab. Charts remain per-tab because they are heavy and
    // user-driven.
    div()
        .w_full()
        .child(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new(format!("Channel: {}", channel.name))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                .child(channel_charts),
        )
        .into_any_element()
}

fn render_room_eq_epa_table(
    d: Ds,
    epa: &RoomEqReportEpaComparison,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    let rows = [
        epa_metric_row(
            "preference",
            "Preference",
            epa.pre.preference,
            epa.post.preference,
            "{:+.2}",
        ),
        epa_metric_row(
            "evaluation",
            "Evaluation",
            epa.pre.evaluation,
            epa.post.evaluation,
            "{:+.2}",
        ),
        epa_metric_row(
            "potency",
            "Potency",
            epa.pre.potency,
            epa.post.potency,
            "{:+.2}",
        ),
        epa_metric_row(
            "activity",
            "Activity",
            epa.pre.activity,
            epa.post.activity,
            "{:+.2}",
        ),
        epa_metric_row(
            "sharpness_acum",
            "Sharpness (acum)",
            epa.pre.sharpness_acum,
            epa.post.sharpness_acum,
            "{:+.2}",
        ),
        epa_metric_row(
            "roughness",
            "Roughness",
            epa.pre.roughness,
            epa.post.roughness,
            "{:+.3}",
        ),
        epa_metric_row(
            "total_loudness_sone",
            "Total loudness (sone)",
            epa.pre.total_loudness_sone,
            epa.post.total_loudness_sone,
            "{:+.2}",
        ),
        epa_metric_row(
            "loudness_balance",
            "Loudness balance",
            epa.pre.loudness_balance,
            epa.post.loudness_balance,
            "{:+.3}",
        ),
    ];

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(render_room_eq_epa_metric_table(d, &rows, theme))
        .child(
            Text::new(
                "Higher is better for Preference / Evaluation / Total loudness / Loudness balance; lower is better for Activity / Sharpness deviation / Roughness.",
            )
            .size(TextSize::Xs)
            .color(theme.text_secondary),
        )
        .into_any_element()
}

/// Specialized EPA metric table that paints the "After EQ" and "Delta"
/// cells green when the metric improved and red when it regressed.
/// Whether higher is better varies per metric (see the legend below the
/// table), so a single ✓/✗ glyph in the After EQ and Delta columns
/// short-circuits the cognitive overhead of remembering the rule for
/// every row.
fn render_room_eq_epa_metric_table(
    d: Ds,
    rows: &[(bool, String, String, String, String, bool)],
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    let headers = ["Metric", "Before EQ", "After EQ", "Delta"];
    let cols = headers.len() as u16;
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("EPA Psychoacoustic Scores")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .child(
            div()
                .flex_none()
                .max_w(px(720.0)) // intentional: epa table compact width.
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme.border)
                .rounded(d.r_md)
                .overflow_hidden()
                .child(
                    div()
                        .grid()
                        .grid_cols(cols)
                        .bg(theme.background_secondary)
                        .children(headers.iter().map(|header| {
                            div().p(d.pad_y_half).px(d.pad_x).child(
                                Text::new((*header).to_string())
                                    .size(TextSize::Xs)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_secondary),
                            )
                        })),
                )
                .children(rows.iter().map(|(_, label, pre, post, delta, improved)| {
                    let verdict_color = if *improved {
                        theme.success
                    } else {
                        theme.error
                    };
                    let mark = if *improved { "✓ " } else { "✗ " };
                    div()
                        .grid()
                        .grid_cols(cols)
                        .child(
                            div()
                                .p(d.pad_y_half)
                                .px(d.pad_x)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(
                                    Text::new(label.clone())
                                        .size(TextSize::Xs)
                                        .color(theme.text_primary),
                                ),
                        )
                        .child(
                            div()
                                .p(d.pad_y_half)
                                .px(d.pad_x)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(
                                    Text::new(pre.clone())
                                        .size(TextSize::Xs)
                                        .color(theme.text_primary),
                                ),
                        )
                        .child(
                            div()
                                .p(d.pad_y_half)
                                .px(d.pad_x)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(
                                    Text::new(format!("{mark}{post}"))
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(verdict_color),
                                ),
                        )
                        .child(
                            div()
                                .p(d.pad_y_half)
                                .px(d.pad_x)
                                .border_t_1()
                                .border_color(theme.border)
                                .child(
                                    Text::new(format!("{mark}{delta}"))
                                        .size(TextSize::Xs)
                                        .weight(TextWeight::Semibold)
                                        .color(verdict_color),
                                ),
                        )
                })),
        )
        .into_any_element()
}

fn epa_metric_row(
    field: &str,
    label: &str,
    pre: f64,
    post: f64,
    fmt: &str,
) -> (bool, String, String, String, String, bool) {
    let delta = post - pre;
    let higher_is_better = matches!(
        field,
        "preference" | "evaluation" | "total_loudness_sone" | "loudness_balance"
    );
    let improved = delta.abs() < 1.0e-9 || (delta > 0.0) == higher_is_better;
    (
        improved,
        label.to_string(),
        fmt_signed(pre, fmt),
        fmt_signed(post, fmt),
        if delta.abs() < 1.0e-9 {
            "=".to_string()
        } else {
            fmt_signed(delta, "{:+.3}")
        },
        improved,
    )
}

fn render_room_eq_filter_details(
    d: Ds,
    passes: &[RoomEqReportEqPass],
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    // The wrapping card already carries the "EQ Filters" title, so we
    // skip a redundant sub-header here. Each pass renders with its
    // colored `display_name` as the per-section legend, which is the
    // real signal of what each filter group is for.
    VStack::new()
        .spacing(StackSpacing::Sm)
        .children(passes.iter().map(|pass| {
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new(pass.display_name.clone())
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(rgba_from_u32(pass.color)),
                )
                .child(render_room_eq_table(
                    d,
                    "",
                    &["#", "Type", "Frequency", "Q", "Gain"],
                    pass.filters.iter().enumerate().map(|(idx, filter)| {
                        vec![
                            (idx + 1).to_string(),
                            filter.filter_type.to_ascii_uppercase(),
                            fmt_hz(filter.freq),
                            format!("{:.2}", filter.q),
                            fmt_db(filter.db_gain),
                        ]
                    }),
                    theme,
                ))
        }))
        .into_any_element()
}

/// Top-level card listing EPA Psychoacoustic Scores for every channel.
///
/// Lifted out of the per-channel "Selected channel result" panel so the
/// user can compare metrics across channels without switching tabs.
/// Each channel renders its own sub-block with a heading carrying the
/// channel name, and only channels that have EPA data populated are
/// included; if no channel has EPA, the function returns an empty stub
/// so callers can chain it unconditionally.
pub(crate) fn render_room_eq_epa_card(
    d: Ds,
    report: &RoomEqReportData,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    let channels_with_epa: Vec<&RoomEqReportChannel> = report
        .channels
        .iter()
        .filter(|channel| channel.epa.is_some())
        .collect();
    if channels_with_epa.is_empty() {
        return div().into_any_element();
    }

    let mut content = VStack::new().spacing(StackSpacing::Md);
    for channel in channels_with_epa {
        let epa = channel.epa.as_ref().unwrap();
        content = content.child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new(format!("Channel: {}", channel.name))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                .child(render_room_eq_epa_table(d, epa, theme)),
        );
    }

    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new("EPA Psychoacoustic Scores")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(content)
        .into_any_element()
}

/// Top-level card listing the EQ filter pipeline for every channel.
///
/// Each channel appears with its name as a sub-heading and its filter
/// passes rendered as compact tables. This replaces the previous design
/// where filter details only showed for the currently-selected channel
/// tab — copying or auditing filters across channels required tab
/// switching, which made it impossible to compare passes side-by-side.
pub(crate) fn render_room_eq_filters_card(
    d: Ds,
    report: &RoomEqReportData,
    theme: &crate::theme::Theme,
) -> gpui::AnyElement {
    let channels_with_filters: Vec<&RoomEqReportChannel> = report
        .channels
        .iter()
        .filter(|channel| !channel.eq_passes.is_empty())
        .collect();
    if channels_with_filters.is_empty() {
        return div().into_any_element();
    }

    let mut content = VStack::new().spacing(StackSpacing::Md);
    for channel in channels_with_filters {
        content = content.child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new(format!("Channel: {}", channel.name))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                )
                .child(render_room_eq_filter_details(d, &channel.eq_passes, theme)),
        );
    }

    Card::new()
        .background(theme.surface)
        .header_background(theme.background_secondary)
        .border(theme.border)
        .header(
            Text::new("EQ Filters")
                .color(theme.text_primary)
                .weight(TextWeight::Semibold),
        )
        .content(content)
        .into_any_element()
}

fn rgba_from_u32(color: u32) -> Rgba {
    let r = ((color >> 16) & 0xff) as f32 / 255.0;
    let g = ((color >> 8) & 0xff) as f32 / 255.0;
    let b = (color & 0xff) as f32 / 255.0;
    Rgba { r, g, b, a: 1.0 }
}

fn fmt_signed(value: f64, fmt: &str) -> String {
    match fmt {
        "{:+.3}" => format!("{value:+.3}"),
        _ => format!("{value:+.2}"),
    }
}

fn render_room_eq_curve_chart(
    title: &str,
    mut series: Vec<RoomEqChartSeries>,
    theme: &crate::theme::Theme,
    settings: RoomEqReviewGraphSettings,
    x_range: (f64, f64),
    y_range: (f64, f64),
    y_label: &str,
    interactive_id: &'static str,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    controls: Option<gpui::AnyElement>,
    chart_size: (f32, f32),
    legend_position: LegendPosition,
) -> gpui::AnyElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{ScaleType, line};

    series.retain(|series| !series.curve.is_empty());
    let Some(_) = series.first() else {
        return VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::new(title.to_string())
                    .weight(TextWeight::Semibold)
                    .size(TextSize::Xs)
                    .color(theme.text_primary),
            )
            .child(render_empty_state(
                IconName::AudioWaveform,
                "No data",
                theme,
            ))
            .into_any_element();
    };

    let prepared = prepare_room_eq_chart_series(&series, settings, x_range, y_label);
    let Some((first, first_y, first_trend)) = prepared.first() else {
        return div().into_any_element();
    };
    let auto_y_range =
        room_eq_chart_y_range(prepared.iter().map(|(_, y, _)| y.as_slice()), y_label)
            .unwrap_or(y_range);
    let chart_theme = theme_to_chart_theme(theme);
    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or(x_range);
    let (y_min, y_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.y_domain())
        .unwrap_or(if settings.y_axis_auto {
            auto_y_range
        } else {
            y_range
        });

    let mut chart = line(&first.curve.freq, first_y)
        .x_scale(ScaleType::Log)
        .x_range(x_min, x_max)
        .y_range(y_min, y_max)
        .y_label(y_label)
        .label(&first.label)
        .legend_position(legend_position)
        .color(first.color)
        .stroke_width(first.stroke_width)
        .opacity(first.opacity)
        .theme(chart_theme)
        .size(chart_size.0, chart_size.1);

    if let Some(trend) = first_trend {
        chart = chart.add_series_with_x(
            &trend.freq,
            &trend.spl,
            Some(&trend.label),
            trend.color,
            1.0,
            0.45,
        );
    }

    for (series, y, trend) in prepared.iter().skip(1) {
        chart = chart.add_series_with_x(
            &series.curve.freq,
            y,
            Some(&series.label),
            series.color,
            series.stroke_width,
            series.opacity,
        );
        if let Some(trend) = trend {
            chart = chart.add_series_with_x(
                &trend.freq,
                &trend.spl,
                Some(&trend.label),
                trend.color,
                1.0,
                0.45,
            );
        }
    }

    let ref_curve = &first.curve;
    if y_label == "SPL (dB)"
        && let (Some(&x0), Some(&x1)) = (ref_curve.freq.first(), ref_curve.freq.last())
    {
        chart = chart.add_series_with_x(
            &[x0, x1],
            &[0.0, 0.0],
            Some("Target (0 dB)"),
            0x999999,
            1.0,
            0.5,
        );
    } else if y_label == "EQ (dB)" {
        chart = chart.add_series_with_x(
            &[20.0, 20000.0],
            &[0.0, 0.0],
            Some("0 dB"),
            0x999999,
            1.0,
            0.5,
        );
    }

    let chart_element = match chart.build() {
        Ok(chart) => {
            if let Some(state) = interactive_state {
                gpui_px::interaction::interactive(interactive_id, chart, state.clone())
                    .build()
                    .into_any_element()
            } else {
                chart.into_any_element()
            }
        }
        Err(e) => {
            log::warn!("RoomEQ report chart build failed for {title}: {e:?}");
            render_empty_state(IconName::AudioWaveform, "Unable to render chart", theme)
        }
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new(title.to_string())
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(theme.text_primary),
                )
                .when_some(controls, |el, controls| el.child(controls)),
        )
        .child(chart_element)
        .into_any_element()
}

#[derive(Clone)]
struct RoomEqTrendSeries {
    label: String,
    freq: Vec<f64>,
    spl: Vec<f64>,
    color: u32,
}

fn prepare_room_eq_chart_series(
    series: &[RoomEqChartSeries],
    settings: RoomEqReviewGraphSettings,
    _x_range: (f64, f64),
    y_label: &str,
) -> Vec<(RoomEqChartSeries, Vec<f64>, Option<RoomEqTrendSeries>)> {
    series
        .iter()
        .cloned()
        .map(|series| {
            let mut y = room_eq_smoothed_spl(&series.curve, settings.smoothing_octaves);
            let trend = if y_label == "SPL (dB)" {
                room_eq_trend_for_curve(&series, &y, settings.normalize_to_trend)
            } else {
                None
            };
            if settings.normalize_to_trend
                && let Some((slope, intercept, _domain)) = room_eq_trend_coefficients(&series, &y)
            {
                for (&freq, value) in series.curve.freq.iter().zip(y.iter_mut()) {
                    if freq.is_finite() && freq > 0.0 {
                        *value -= slope * freq.log10() + intercept;
                    }
                }
            }
            (series, y, if settings.show_trend { trend } else { None })
        })
        .collect()
}

fn room_eq_trend_for_curve(
    series: &RoomEqChartSeries,
    y: &[f64],
    normalized: bool,
) -> Option<RoomEqTrendSeries> {
    let (slope, intercept, domain) = room_eq_trend_coefficients(series, y)?;
    let values = if normalized {
        vec![0.0, 0.0]
    } else {
        vec![
            slope * domain.0.log10() + intercept,
            slope * domain.1.log10() + intercept,
        ]
    };
    Some(RoomEqTrendSeries {
        label: format!("Trend {}", series.label),
        freq: vec![domain.0, domain.1],
        spl: values,
        color: series.color,
    })
}

fn room_eq_trend_coefficients(
    series: &RoomEqChartSeries,
    values: &[f64],
) -> Option<(f64, f64, (f64, f64))> {
    let channel_name = series.channel_name.as_deref().unwrap_or(&series.label);
    if is_room_eq_sub_or_lfe_channel(channel_name) {
        let domain = room_eq_passband_trend_fit_domain(&series.curve.freq, values)?;
        let average = room_eq_average_value_in_domain(&series.curve.freq, values, domain)?;
        Some((0.0, average, domain))
    } else {
        let domain = room_eq_trend_fit_domain(channel_name, &series.curve.freq)?;
        let (slope, intercept) = calculate_room_eq_log_trend(&series.curve.freq, values, domain)?;
        Some((slope, intercept, domain))
    }
}

fn room_eq_average_value_in_domain(
    freqs: &[f64],
    values: &[f64],
    domain: (f64, f64),
) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (&freq, &value) in freqs.iter().zip(values.iter()) {
        if freq.is_finite() && freq >= domain.0 && freq <= domain.1 && value.is_finite() {
            sum += value;
            count += 1.0;
        }
    }
    (count > 0.0).then_some(sum / count)
}

fn room_eq_chart_y_range<'a>(
    values: impl IntoIterator<Item = &'a [f64]>,
    y_label: &str,
) -> Option<(f64, f64)> {
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;
    for value in values.into_iter().flatten() {
        if value.is_finite() {
            min_value = min_value.min(*value);
            max_value = max_value.max(*value);
        }
    }
    if !min_value.is_finite() || !max_value.is_finite() {
        return None;
    }
    if y_label == "EQ (dB)" {
        let lower = ((min_value / 5.0).floor() * 5.0 - 5.0).max(-30.0);
        let upper = ((max_value / 5.0).ceil() * 5.0 + 5.0).min(30.0);
        return Some(if upper <= lower {
            (lower, lower + 1.0)
        } else {
            (lower, upper)
        });
    }
    let upper = ((max_value / 5.0).ceil() * 5.0).max(5.0);
    let lower = (min_value / 5.0).floor() * 5.0;
    Some(if upper - lower < 20.0 {
        (lower - 5.0, upper + 5.0)
    } else {
        (lower, upper)
    })
}

fn render_room_eq_ir_chart(
    channel: &RoomEqReportChannel,
    theme: &crate::theme::Theme,
    chart_size: (f32, f32),
) -> gpui::AnyElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    let primary = channel.pre_ir.as_ref().or(channel.post_ir.as_ref());
    let Some(primary) = primary else {
        return div().into_any_element();
    };

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for ir in [&channel.pre_ir, &channel.post_ir].into_iter().flatten() {
        for &amp in &ir.amplitude {
            y_min = y_min.min(amp);
            y_max = y_max.max(amp);
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() || y_max <= y_min {
        y_min = -1.0;
        y_max = 1.0;
    }

    let (primary_label, primary_color) = if channel.pre_ir.is_some() {
        ("Before", 0xff6464)
    } else {
        ("After", 0x64c864)
    };
    let mut chart = line(&primary.time_ms, &primary.amplitude)
        .x_scale(ScaleType::Linear)
        .x_range(
            *primary.time_ms.first().unwrap_or(&0.0),
            *primary.time_ms.last().unwrap_or(&1.0),
        )
        .y_range(y_min, y_max)
        .y_label("Amplitude")
        .label(primary_label)
        .legend_position(LegendPosition::Right)
        .color(primary_color)
        .stroke_width(1.5)
        .opacity(0.9)
        .theme(theme_to_chart_theme(theme))
        .size(chart_size.0, chart_size.1);

    if channel.pre_ir.is_some()
        && let Some(post_ir) = channel.post_ir.as_ref()
    {
        chart = chart.add_series_with_x(
            &post_ir.time_ms,
            &post_ir.amplitude,
            Some("After"),
            0x64c864,
            1.5,
            0.9,
        );
    }

    let chart_element = chart
        .build()
        .map(|chart| chart.into_any_element())
        .unwrap_or_else(|e| {
            log::warn!("RoomEQ IR chart build failed: {e:?}");
            render_empty_state(
                IconName::AudioWaveform,
                "Unable to render impulse response",
                theme,
            )
        });

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Impulse Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .child(chart_element)
        .into_any_element()
}

fn room_eq_smoothed_spl(curve: &RoomEqReportCurve, smoothing_octaves: f64) -> Vec<f64> {
    if smoothing_octaves <= 0.0 {
        return curve.spl.clone();
    }
    dsp::smooth_response_f64(&curve.freq, &curve.spl, smoothing_octaves)
        .into_iter()
        .map(|value| if value.is_finite() { value } else { 0.0 })
        .collect()
}

fn room_eq_average_spl_in_range(curve: &RoomEqReportCurve, min_freq: f64, max_freq: f64) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (&freq, &spl) in curve.freq.iter().zip(curve.spl.iter()) {
        if freq >= min_freq && freq <= max_freq && spl.is_finite() {
            sum += spl;
            count += 1.0;
        }
    }
    if count > 0.0 { sum / count } else { 0.0 }
}

/// Interpolate a sampled curve at a single frequency using log-frequency linear interpolation.
fn interpolate_value_at(frequencies: &[f64], values: &[f64], target_freq: f64) -> f64 {
    if frequencies.is_empty() || values.is_empty() {
        return 0.0;
    }
    if target_freq <= frequencies[0] {
        return values[0];
    }
    if target_freq >= *frequencies.last().unwrap() {
        return *values.last().unwrap();
    }
    for i in 0..frequencies.len() - 1 {
        if target_freq >= frequencies[i] && target_freq <= frequencies[i + 1] {
            let denom = frequencies[i + 1].ln() - frequencies[i].ln();
            if denom.abs() < 1e-12 {
                return values[i];
            }
            let t = (target_freq.ln() - frequencies[i].ln()) / denom;
            return values[i] + t * (values[i + 1] - values[i]);
        }
    }
    *values.last().unwrap()
}

// === Free functions for channel configuration UI ===

/// Render a single channel configuration row.
///
/// `sample_rate_hz` drives the FIR-crossover latency readout. Pass the
/// optimizer's active sample rate so latency reflects the actual project
/// (`(taps - 1) / 2 / sample_rate_hz * 1000` ms).
pub(crate) fn render_channel_config_row(
    idx: usize,
    config: &crate::app::types::RoomEqSpeakerConfig,
    theme: &crate::theme::Theme,
    view: &Entity<PlayerView>,
    d: Ds,
    sample_rate_hz: f64,
) -> impl IntoElement {
    use crate::app::types::SpeakerConfigType;

    use crate::app::types::CrossoverType;

    let channel_name = config.channel_name.clone();
    let is_multi = config.config_type == SpeakerConfigType::MultiDriver;
    let crossover_type = config.crossover_type;
    let fir_taps = config.linear_phase_fir_taps;
    let is_linear_phase = matches!(crossover_type, CrossoverType::LinearPhase);

    div()
        .flex()
        .gap(d.section)
        .items_center()
        .w_full()
        .p(d.pad_x)
        .bg(theme.surface)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        // Channel name
        .child(
            div().w(rems(5.0)).child(
                Text::new(channel_name)
                    .weight(TextWeight::Bold)
                    .color(theme.text_primary),
            ),
        )
        // Speaker type toggle
        .child(
            div()
                .flex()
                .gap(d.gap)
                .items_center()
                .child(
                    Text::new("Type:")
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(SharedString::from(format!("single-{}", idx)), "Single")
                        .variant(if !is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) = state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .speaker_configs
                                            .get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::Single;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("multi-{}", idx)), "Multi-Driver")
                        .variant(if is_multi {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        if let Some(cfg) = state
                                            .app
                                            .measurement_state
                                            .room_eq_state
                                            .speaker_configs
                                            .get_mut(idx)
                                        {
                                            cfg.config_type = SpeakerConfigType::MultiDriver;
                                        }
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        // Crossover type selector (only shown for multi-driver)
        .when(is_multi, |el| {
            el.child(
                div()
                    .flex()
                    .gap(d.gap)
                    .items_center()
                    .child(
                        Text::new("Crossover:")
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(render_crossover_dropdown(idx, crossover_type, view, theme))
                    // Linear-phase FIR taps + latency readout only when
                    // the LinearPhase variant is currently selected. The
                    // slider cycles power-of-two values so the FIR length
                    // stays FFT-friendly and matches the plugin's defaults.
                    .when(is_linear_phase, |el| {
                        el.child(render_linear_phase_taps_controls(
                            idx,
                            fir_taps,
                            view,
                            theme,
                            sample_rate_hz,
                        ))
                    }),
            )
        })
}

/// Render the FIR-taps +/- buttons + latency readout for linear-phase
/// crossovers. Taps step through `[1024, 2048, 4096, 8192, 16384, 32768]`
/// with clamping at both ends, so dialing latency down from the default
/// doesn't wrap around through 32768. Latency is `(taps - 1) / 2` samples
/// at the project's active sample rate.
fn render_linear_phase_taps_controls(
    channel_idx: usize,
    fir_taps: usize,
    view: &Entity<PlayerView>,
    theme: &crate::theme::Theme,
    sample_rate_hz: f64,
) -> impl IntoElement {
    const TAP_CHOICES: &[usize] = &[1024, 2048, 4096, 8192, 16384, 32768];

    let display_taps = fir_taps.max(1);
    let latency_samples = display_taps.saturating_sub(1) / 2;
    let latency_ms = (latency_samples as f64) / sample_rate_hz.max(1.0) * 1000.0;
    let readout = format!("FIR {display_taps} • {latency_ms:.1} ms");

    let step_button = |label: &'static str, id: SharedString, delta: i32| {
        let view = view.clone();
        let theme = theme.clone();
        Button::new(id, label)
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Xs)
            .theme(theme.to_button_theme())
            .on_click(move |_, cx| {
                view.update(cx, |this, cx| {
                    this.state.update(cx, |state, _| {
                        if let Some(cfg) = state
                            .app
                            .measurement_state
                            .room_eq_state
                            .speaker_configs
                            .get_mut(channel_idx)
                        {
                            let current = cfg.linear_phase_fir_taps;
                            // Snap onto the preset table, then step. Clamps
                            // at both ends prevent wrap-around so dialing
                            // down from default doesn't suddenly jump to
                            // the longest FIR.
                            let current_idx = TAP_CHOICES
                                .iter()
                                .position(|&t| t >= current)
                                .unwrap_or(TAP_CHOICES.len() - 1);
                            let next_idx = (current_idx as i32 + delta)
                                .clamp(0, TAP_CHOICES.len() as i32 - 1)
                                as usize;
                            cfg.linear_phase_fir_taps = TAP_CHOICES[next_idx];
                        }
                    });
                    cx.notify();
                });
            })
            .build()
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(step_button(
            "-",
            SharedString::from(format!("fir-taps-dec-{channel_idx}")),
            -1,
        ))
        .child(
            div().min_w(px(120.0)).child(
                Text::new(readout)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            ),
        )
        .child(step_button(
            "+",
            SharedString::from(format!("fir-taps-inc-{channel_idx}")),
            1,
        ))
}

/// Render crossover type dropdown as a cycling button
fn render_crossover_dropdown(
    channel_idx: usize,
    current: crate::app::types::CrossoverType,
    view: &Entity<PlayerView>,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::app::types::CrossoverType;

    let crossover_types = CrossoverType::all();
    let current_label = current.as_str();

    Button::new(
        SharedString::from(format!("xover-{}", channel_idx)),
        current_label,
    )
    .variant(ButtonVariant::Secondary)
    .size(ButtonSize::Xs)
    .theme(theme.to_button_theme())
    .on_click({
        let view = view.clone();
        let crossover_types = crossover_types.to_vec();
        move |_, cx| {
            view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    if let Some(cfg) = state
                        .app
                        .measurement_state
                        .room_eq_state
                        .speaker_configs
                        .get_mut(channel_idx)
                    {
                        // Find current index and cycle to next
                        let current_idx = crossover_types
                            .iter()
                            .position(|&ct| ct == cfg.crossover_type)
                            .unwrap_or(0);
                        let next_idx = (current_idx + 1) % crossover_types.len();
                        cfg.crossover_type = crossover_types[next_idx];
                    }
                });
                cx.notify();
            });
        }
    })
}

// === Review Step UI Free Functions ===

/// Render a single channel result card with plots and filter details
/// If interactive_state is provided, the chart will support pan/zoom interactions
pub(crate) fn render_channel_result_card(
    d: Ds,
    result: crate::app::types::ChannelOptResult,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    y_axis_auto: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
    has_fir: bool,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    let channel_name = result.channel_name.clone();
    let score_improvement = result.pre_score - result.post_score;
    let corrected_response = result
        .normalized_response
        .as_ref()
        .or(result.corrected_response.as_ref());
    let has_response_pair = result.original_response.is_some() && corrected_response.is_some();
    let has_corrected_response = corrected_response.is_some();

    div()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .p(d.card)
        .w_full()
        .bg(theme.surface)
        .rounded(d.r_lg)
        .border_1()
        .border_color(theme.border)
        // Header with channel name and scores
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    Text::new(channel_name)
                        .weight(TextWeight::Bold)
                        .size(TextSize::Md)
                        .color(theme.text_primary),
                )
                .child(
                    div()
                        .flex()
                        .gap(d.section)
                        .child(
                            Text::new(format!("Before: {:.2}", result.pre_score))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("After: {:.2}", result.post_score))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(format!("{:+.2}", score_improvement))
                                .weight(TextWeight::Bold)
                                .color(if score_improvement > 0.0 {
                                    theme.success
                                } else {
                                    theme.error
                                }),
                        ),
                ),
        )
        // Filter plot: each filter and the sum (if available).
        // Render if there are ANY filters to show (main IIR or broadband
        // pre-correction). Previously this gated on `eq_filters` only, so
        // broadband-only optimizations silently dropped the plot.
        .when(
            should_render_filter_plot(
                has_response_pair,
                !result.eq_filters.is_empty(),
                !result.broadband_filters.is_empty(),
            ),
            |div| {
                let (Some(original), Some(normalized)) = (
                    result.original_response.as_ref(),
                    result.normalized_response.as_ref(),
                ) else {
                    return div;
                };
                div.child(render_filter_plot(
                    original,
                    normalized,
                    &result.eq_filters,
                    &result.broadband_filters,
                    result.preamp_gain_db,
                    has_fir,
                    theme,
                    smoothing_octaves,
                    y_axis_auto,
                    interactive_state,
                ))
            },
        )
        // Original vs corrected: a thin viewer over the precomputed JSON
        // curves. If a JSON channel only has final_curve, render that alone.
        .when(has_corrected_response, |div| {
            let Some(corrected) = result
                .normalized_response
                .as_ref()
                .or(result.corrected_response.as_ref())
            else {
                return div;
            };
            let empty_original: Vec<(f64, f64)> = Vec::new();
            let original = result
                .original_response
                .as_deref()
                .unwrap_or(empty_original.as_slice());
            div.child(render_response_comparison_graph(
                &result.channel_name,
                original,
                corrected,
                result.preamp_gain_db,
                theme,
                smoothing_octaves,
                interactive_state,
            ))
        })
        // Histogram (if trend data available)
        .when(
            (result.group_delay_before.is_some() || result.group_delay_after.is_some())
                && has_response_pair,
            |div| {
                let (Some(original), Some(normalized)) = (
                    result.original_response.as_ref(),
                    result.normalized_response.as_ref(),
                ) else {
                    return div;
                };
                div.child(render_tonal_histogram(
                    &result.channel_name,
                    original,
                    normalized,
                    result.preamp_gain_db,
                    theme,
                    smoothing_octaves,
                ))
            },
        )
        // Phase response plot (if phase data available)
        .when(
            result.phase_response_before.is_some() || result.phase_response_after.is_some(),
            |div| {
                div.child(render_phase_graph(
                    result.phase_response_before.as_deref(),
                    result.phase_response_after.as_deref(),
                    theme,
                ))
            },
        )
        // Group delay graph (if phase data available)
        .when(
            result.group_delay_before.is_some() || result.group_delay_after.is_some(),
            |div| {
                div.child(render_group_delay_graph(
                    result.group_delay_before.as_deref(),
                    result.group_delay_after.as_deref(),
                    theme,
                ))
            },
        )
        // Impulse response plot (if IR data available)
        .when_some(result.impulse_response.as_ref(), |div, ir| {
            div.child(render_impulse_response_graph(ir, theme))
        })
        // EQ Filter details — main (IIR room correction) and broadband
        // pre-correction are shown as separate tables so users can tell them
        // apart. The `Applied to Rack` action creates one named plugin per
        // section, so the grouping here mirrors what lands in the rack.
        .when(!result.eq_filters.is_empty(), |el| {
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Room EQ Filters")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(render_filter_table(d, &result.eq_filters, theme)),
            )
        })
        .when(!result.broadband_filters.is_empty(), |el| {
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Broadband Pre-correction Filters")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(render_filter_table(d, &result.broadband_filters, theme)),
            )
        })
        // Crossover info (if multi-driver)
        .when_some(result.crossover_freqs.as_ref(), |el, xover_freqs| {
            el.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new("Crossover Frequencies")
                            .weight(TextWeight::Semibold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .gap(d.gap)
                            .children(xover_freqs.iter().map(|f| {
                                gpui::div()
                                    .px(d.pad_y)
                                    .py(d.pad_y_half)
                                    .bg(theme.background_secondary)
                                    .rounded(d.r_md)
                                    .child(
                                        Text::new(format_frequency(*f))
                                            .size(TextSize::Xs)
                                            .color(theme.text_primary),
                                    )
                            })),
                    ),
            )
        })
}

/// Render the frequency response comparison graph: Original vs Corrected with trendlines
/// If interactive_state is provided, the chart will support pan/zoom interactions
fn render_response_comparison_graph(
    channel_name: &str,
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    _preamp_gain_db: f64,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 400.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;
    const TARGET_GREY: u32 = 0x999999;

    let original_frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let corrected_frequencies: Vec<f64> = corrected.iter().map(|(f, _)| *f).collect();
    let original_values: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    let corrected_values: Vec<f64> = corrected.iter().map(|(_, db)| *db).collect();

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };

    let original_smooth = sanitize(&dsp::smooth_response_f64(
        &original_frequencies,
        &original_values,
        smoothing_octaves,
    ));
    let corrected_smooth = sanitize(&dsp::smooth_response_f64(
        &corrected_frequencies,
        &corrected_values,
        smoothing_octaves,
    ));

    if corrected_frequencies.is_empty() {
        return render_empty_state(IconName::AudioWaveform, "No data available", theme);
    }

    let (y_min_auto, y_max_auto) = {
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        for &v in original_smooth.iter().chain(corrected_smooth.iter()) {
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }
        let max = if max_val.is_finite() {
            ((max_val / 5.0).ceil() * 5.0).max(5.0)
        } else {
            5.0
        };
        let min = if min_val.is_finite() {
            (min_val / 5.0).floor() * 5.0
        } else {
            -15.0
        };
        (min, max)
    };

    let chart_theme = theme_to_chart_theme(theme);
    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or((20.0, 20000.0));
    let (y_min_domain, y_max_domain) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.y_domain())
        .unwrap_or((y_min_auto, y_max_auto));

    let y_min_domain = if y_min_domain.is_finite() {
        y_min_domain
    } else {
        -15.0
    };
    let y_max_domain = if y_max_domain.is_finite() {
        y_max_domain
    } else {
        5.0
    };
    let y_max_domain = if y_max_domain <= y_min_domain {
        y_min_domain + 1.0
    } else {
        y_max_domain
    };

    let original_label = format!("{} Original", channel_name);
    let corrected_label = format!("{} Corrected", channel_name);

    let base_x = if !original_frequencies.is_empty() {
        &original_frequencies
    } else {
        &corrected_frequencies
    };
    let base_y = if !original_smooth.is_empty() {
        &original_smooth
    } else {
        &corrected_smooth
    };

    let mut chart_builder = line(base_x, base_y)
        .x_scale(ScaleType::Log)
        .x_range(x_min, x_max)
        .y_range(y_min_domain, y_max_domain)
        .y_label("SPL (dB)")
        .label(if !original_smooth.is_empty() {
            original_label
        } else {
            corrected_label.clone()
        })
        .legend_position(LegendPosition::Right)
        .color(BLUE)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    if !original_smooth.is_empty() {
        chart_builder = chart_builder.add_series_with_x(
            &corrected_frequencies,
            &corrected_smooth,
            Some(&corrected_label),
            ORANGE,
            2.0,
            1.0,
        );
    }

    if let (Some(&x0), Some(&x1)) = (base_x.first(), base_x.last()) {
        chart_builder = chart_builder.add_series_with_x(
            &[x0, x1],
            &[0.0, 0.0],
            Some("Target (0 dB)"),
            TARGET_GREY,
            1.0,
            0.5,
        );
    }

    let line_chart = chart_builder.build();

    let chart_element: Option<gpui::AnyElement> = line_chart.ok().map(|chart| {
        if let Some(state) = interactive_state {
            gpui_px::interaction::interactive("room-eq-response-chart", chart, state.clone())
                .build()
                .into_any_element()
        } else {
            chart.into_any_element()
        }
    });

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Original vs Corrected")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |el, c| el.child(c))
        .into_any_element()
}

/// Render the filter plot showing each individual filter and their combined response.
///
/// `has_fir` tells the plot whether the channel's DSP chain contains a
/// convolution/FIR block. We can't decompose FIR magnitude into parametric
/// bands, but at minimum the user deserves to know the chain includes an
/// FIR correction they won't see as individual lines.
fn render_filter_plot(
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    eq_filters: &[crate::app::types::EqFilterConfig],
    broadband_filters: &[crate::app::types::EqFilterConfig],
    preamp_gain_db: f64,
    has_fir: bool,
    theme: &crate::theme::Theme,
    _smoothing_octaves: f64,
    y_axis_auto: bool,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, StrokeDashArray, line};
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 400.0;
    const SAMPLE_RATE: f64 = sotf_plugins::DEFAULT_PREVIEW_SAMPLE_RATE;

    const BLUE: u32 = 0x1f77b4;
    const GREEN: u32 = 0x2ca02c;
    const PURPLE: u32 = 0x9467bd;
    const CYAN: u32 = 0x17becf;
    const MAGENTA: u32 = 0xd62728;

    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    let corrected_values_raw: Vec<f64> = corrected.iter().map(|(_, db)| *db).collect();

    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let _corrected_normalized: Vec<f64> =
        corrected_values_raw.iter().map(|&db| db - offset).collect();

    if frequencies.is_empty() || (eq_filters.is_empty() && broadband_filters.is_empty()) {
        return div()
            .child(render_empty_state(
                IconName::AudioWaveform,
                "No filter data available",
                theme,
            ))
            .into_any_element();
    }

    let chart_theme = theme_to_chart_theme(theme);

    let filter_colors = [
        BLUE,
        GREEN,
        PURPLE,
        CYAN,
        MAGENTA,
        0x8c564bu32,
        0xe377c2u32,
        0x7f7f7fu32,
        0xbcbd22u32,
        0x1f77b4u32,
    ];

    let parse_type = |s: &str| -> BiquadFilterType {
        match s {
            "peak" | "pk" | "Peak" => BiquadFilterType::Peak,
            "lowshelf" | "ls" | "Lowshelf" => BiquadFilterType::Lowshelf,
            "highshelf" | "hs" | "Highshelf" => BiquadFilterType::Highshelf,
            "lowpass" | "lp" | "Lowpass" => BiquadFilterType::Lowpass,
            "highpass" | "hp" | "Highpass" => BiquadFilterType::Highpass,
            _ => BiquadFilterType::Peak,
        }
    };

    let filter_response_at = |f: &crate::app::types::EqFilterConfig, freq: f64| -> f64 {
        let biquad = Biquad::new(
            parse_type(&f.filter_type),
            f.frequency,
            SAMPLE_RATE,
            f.q,
            f.gain_db,
        );
        biquad.log_result(freq)
    };

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };

    // Compute combined sum of main EQ + broadband, plus any flat preamp gain
    // applied by post-optimization stages (spectral-alignment, VoG). Without
    // including `preamp_gain_db` here, the Sum line drifts from the actual
    // correction visible in the "Original vs Corrected" plot whenever a
    // flat-gain plugin is present in the channel chain.
    let all_filters: Vec<&crate::app::types::EqFilterConfig> =
        eq_filters.iter().chain(broadband_filters.iter()).collect();
    let eq_response: Vec<f64> = sanitize(
        &frequencies
            .iter()
            .map(|&freq| {
                all_filters
                    .iter()
                    .map(|f| filter_response_at(f, freq))
                    .sum::<f64>()
                    + preamp_gain_db
            })
            .collect::<Vec<_>>(),
    );

    let mut chart_builder = line(&frequencies, &vec![0.0; frequencies.len()])
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(-12.0, 6.0)
        .y_label("EQ (dB)")
        .label("Sum")
        .legend_position(LegendPosition::Right)
        .color(GREEN)
        .stroke_width(2.0)
        .opacity(1.0)
        .theme(chart_theme.clone())
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    chart_builder = chart_builder.add_series(&eq_response, Some("Sum"), GREEN, 2.0, 1.0);

    // Main EQ filters (parametric IIR biquads from the room optimizer).
    for (i, filter) in eq_filters.iter().enumerate() {
        let resp = sanitize(
            &frequencies
                .iter()
                .map(|&f| filter_response_at(filter, f))
                .collect::<Vec<_>>(),
        );
        let color = filter_colors[i % filter_colors.len()];
        let label = format!(
            "IIR {} {} {:.0}Hz",
            i + 1,
            filter.filter_type,
            filter.frequency
        );
        chart_builder = chart_builder.add_series(&resp, Some(&label), color, 1.5, 0.7);
    }

    // Broadband pre-correction filters — same palette but drawn dashed so
    // the user can visually separate "room IIR correction" from "driver
    // tonal pre-tilt" without having to hunt the color legend.
    const BB_COLOR: u32 = 0x8B4513; // saddle brown — distinct from PK palette
    for (i, filter) in broadband_filters.iter().enumerate() {
        let resp = sanitize(
            &frequencies
                .iter()
                .map(|&f| filter_response_at(filter, f))
                .collect::<Vec<_>>(),
        );
        let label = format!(
            "Broadband {} {} {:.0}Hz",
            i + 1,
            filter.filter_type,
            filter.frequency
        );
        chart_builder = chart_builder
            .add_series(&resp, Some(&label), BB_COLOR, 1.5, 0.7)
            .series_dash_array(StrokeDashArray::Dashed);
    }

    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or((20.0, 20000.0));
    let (y_min, y_max) = if y_axis_auto {
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        for &v in eq_response.iter() {
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }
        for filter in eq_filters.iter() {
            let _ = filter;
        }
        let max = if max_val.is_finite() {
            ((max_val / 2.0).ceil() * 2.0).max(6.0)
        } else {
            6.0
        };
        let min = if min_val.is_finite() {
            (min_val / 2.0).floor() * 2.0
        } else {
            -12.0
        };
        (min, max)
    } else {
        (-12.0, 6.0)
    };

    let line_chart = chart_builder
        .x_range(x_min, x_max)
        .y_range(y_min, y_max)
        .build();

    let chart_element: Option<gpui::AnyElement> = line_chart.ok().map(|chart| {
        if let Some(state) = interactive_state {
            gpui_px::interaction::interactive("room-eq-filter-chart", chart, state.clone())
                .build()
                .into_any_element()
        } else {
            chart.into_any_element()
        }
    });

    let iir_count = eq_filters.len();
    let bb_count = broadband_filters.len();
    // Tell the user exactly what each line represents. This header line
    // doubles as a legend key so they can parse the chart without hunting
    // through the color-coded entries on the side.
    let mut subtitle_parts: Vec<String> = Vec::new();
    if iir_count > 0 {
        subtitle_parts.push(format!("{} IIR peak filters", iir_count));
    }
    if bb_count > 0 {
        subtitle_parts.push(format!("{} broadband pre-corrections (dashed)", bb_count));
    }
    if preamp_gain_db.abs() >= 0.05 {
        subtitle_parts.push(format!("preamp {:+.1} dB", preamp_gain_db));
    }
    if has_fir {
        subtitle_parts
            .push("FIR correction applied (magnitude included in Corrected curve)".to_string());
    }
    let subtitle = if subtitle_parts.is_empty() {
        None
    } else {
        Some(subtitle_parts.join(" + "))
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("EQ Filters")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(subtitle, |el, s| {
            el.child(Text::new(s).size(TextSize::Xs).color(theme.text_secondary))
        })
        .when_some(chart_element, |el, c| el.child(c))
        .into_any_element()
}

/// Render the tonal balance histogram
fn render_tonal_histogram(
    channel_name: &str,
    original: &[(f64, f64)],
    corrected: &[(f64, f64)],
    preamp_gain_db: f64,
    theme: &crate::theme::Theme,
    smoothing_octaves: f64,
) -> impl IntoElement {
    use gpui_px::{BarTheme, LegendPosition, bar};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let frequencies: Vec<f64> = original.iter().map(|(f, _)| *f).collect();
    let original_values_raw: Vec<f64> = original.iter().map(|(_, db)| *db).collect();
    // Strip the inter-channel level-match preamp from Corrected so the
    // histogram reflects EQ shape, not the per-channel gain shift.
    let corrected_values_raw: Vec<f64> = corrected
        .iter()
        .map(|(_, db)| *db - preamp_gain_db)
        .collect();

    let offset = crate::app::types::RoomEqState::calculate_normalization_offset(
        &frequencies,
        &original_values_raw,
    );
    let original_values: Vec<f64> = original_values_raw.iter().map(|&db| db - offset).collect();
    let corrected_values: Vec<f64> = corrected_values_raw.iter().map(|&db| db - offset).collect();

    let original_smooth =
        dsp::smooth_response_f64(&frequencies, &original_values, smoothing_octaves);
    let corrected_smooth =
        dsp::smooth_response_f64(&frequencies, &corrected_values, smoothing_octaves);

    let sanitize = |v: &[f64]| -> Vec<f64> {
        v.iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect()
    };
    let original_smooth = sanitize(&original_smooth);
    let corrected_smooth = sanitize(&corrected_smooth);

    let is_sub_or_lfe = is_room_eq_sub_or_lfe_channel(channel_name);
    let orig_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &original_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let corr_trend_domain = if is_sub_or_lfe {
        room_eq_passband_trend_fit_domain(&frequencies, &corrected_smooth)
    } else {
        room_eq_trend_fit_domain(channel_name, &frequencies)
    };
    let orig_trend = orig_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &original_smooth, domain));
    let corr_trend = corr_trend_domain
        .and_then(|domain| calculate_room_eq_log_trend(&frequencies, &corrected_smooth, domain));

    let hist_chart = if let (Some((slope_orig, int_orig)), Some((slope_corr, int_corr))) =
        (orig_trend, corr_trend)
    {
        let calculate_histogram = |freqs: &[f64],
                                   values: &[f64],
                                   domain: Option<(f64, f64)>,
                                   slope: f64,
                                   intercept: f64|
         -> Vec<f64> {
            let mut bins = vec![0.0; 9];

            for (i, &f) in freqs.iter().enumerate() {
                if let Some(domain) = domain
                    && f >= domain.0
                    && f <= domain.1
                    && let Some(&y) = values.get(i)
                {
                    let trend_y = slope * f.log10() + intercept;
                    let deviation = (y - trend_y).abs();

                    let bin_idx = (deviation / 0.5).floor() as usize;
                    if bin_idx < 8 {
                        bins[bin_idx] += 1.0;
                    } else {
                        bins[8] += 1.0;
                    }
                }
            }
            bins
        };

        let hist_orig = calculate_histogram(
            &frequencies,
            &original_smooth,
            orig_trend_domain,
            slope_orig,
            int_orig,
        );
        let hist_corr = calculate_histogram(
            &frequencies,
            &corrected_smooth,
            corr_trend_domain,
            slope_corr,
            int_corr,
        );

        let labels = vec![
            "0-0.5", "0.5-1", "1-1.5", "1.5-2", "2-2.5", "2.5-3", "3-3.5", "3.5-4", ">4",
        ];

        let bar_theme = BarTheme {
            plot_background: theme.surface,
            title_color: theme.text_primary,
            legend_text_color: theme.text_secondary,
        };

        bar(&labels, &hist_orig)
            .color(BLUE)
            .label("Original")
            .theme(bar_theme)
            .size(GRAPH_WIDTH, GRAPH_HEIGHT)
            .bar_gap(4.0)
            .opacity(0.8)
            .legend_position(LegendPosition::Bottom)
            .add_series(&hist_corr, Some("Corrected"), ORANGE, 0.8)
            .build()
            .ok()
    } else {
        None
    };

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Tonal Balance")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(hist_chart, |el, c| el.child(c))
        .into_any_element()
}

/// Render the phase response graph
fn render_phase_graph(
    phase_before: Option<&[(f64, f64)]>,
    phase_after: Option<&[(f64, f64)]>,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let chart_theme = theme_to_chart_theme(theme);

    let Some(reference) = phase_before.or(phase_after) else {
        return div().into_any_element();
    };
    let frequencies: Vec<f64> = reference.iter().map(|(f, _)| *f).collect();

    let in_range = |f: f64| (20.0..=20000.0).contains(&f);

    let before_values: Option<Vec<f64>> = phase_before.map(|b| b.iter().map(|(_, p)| *p).collect());

    let after_values: Option<Vec<f64>> = phase_after.map(|after| {
        frequencies
            .iter()
            .map(|&f| {
                if let Some(pos) = after.windows(2).position(|w| w[0].0 <= f && f <= w[1].0) {
                    let (f0, p0) = after[pos];
                    let (f1, p1) = after[pos + 1];
                    let t = if (f1 - f0).abs() > 1e-12 {
                        (f - f0) / (f1 - f0)
                    } else {
                        0.0
                    };
                    p0 + t * (p1 - p0)
                } else {
                    after.last().map(|(_, p)| *p).unwrap_or(0.0)
                }
            })
            .collect()
    });

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (i, &f) in frequencies.iter().enumerate() {
        if in_range(f) {
            for vals in [&before_values, &after_values].into_iter().flatten() {
                if let Some(&v) = vals.get(i)
                    && v.is_finite()
                {
                    y_min = y_min.min(v);
                    y_max = y_max.max(v);
                }
            }
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -std::f64::consts::PI;
        y_max = std::f64::consts::PI;
    }
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min = (y_min - margin).floor();
    y_max = (y_max + margin).ceil();

    let (primary_values, primary_label, primary_color) = if let Some(ref bv) = before_values {
        (bv.as_slice(), "Before", BLUE)
    } else if let Some(ref av) = after_values {
        (av.as_slice(), "After", ORANGE)
    } else {
        return div().into_any_element();
    };

    let mut chart_builder = line(&frequencies, primary_values)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(y_min, y_max)
        .y_label("Phase (rad)")
        .label(primary_label)
        .legend_position(LegendPosition::Bottom)
        .color(primary_color)
        .stroke_width(1.5)
        .opacity(0.7)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    if before_values.is_some()
        && let Some(ref av) = after_values
    {
        chart_builder = chart_builder.add_series(av, Some("After"), ORANGE, 1.5, 0.9);
    }

    let chart_element: Option<gpui::AnyElement> =
        chart_builder.build().ok().map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Phase Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
        .into_any_element()
}

/// Render the impulse response graph
fn render_impulse_response_graph(
    impulse_response: &[(f64, f64)],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 200.0;

    const BLUE: u32 = 0x1f77b4;

    let chart_theme = theme_to_chart_theme(theme);

    let samples: Vec<f64> = impulse_response.iter().map(|(s, _)| *s).collect();
    let amplitudes: Vec<f64> = impulse_response.iter().map(|(_, a)| *a).collect();

    let sanitize: Vec<f64> = amplitudes
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &v in sanitize.iter() {
        y_min = y_min.min(v);
        y_max = y_max.max(v);
    }
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -1.0;
        y_max = 1.0;
    }
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min -= margin;
    y_max += margin;

    let chart_element: Option<gpui::AnyElement> = line(&samples, &sanitize)
        .x_scale(ScaleType::Linear)
        .x_range(
            samples.first().copied().unwrap_or(0.0),
            samples.last().copied().unwrap_or(1.0),
        )
        .y_range(y_min, y_max)
        .y_label("Amplitude")
        .label("IR")
        .legend_position(LegendPosition::Bottom)
        .color(BLUE)
        .stroke_width(1.5)
        .opacity(1.0)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT)
        .build()
        .ok()
        .map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Impulse Response")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
        .into_any_element()
}

/// Render the EQ filter table
fn render_filter_table(
    d: Ds,
    filters: &[crate::app::types::EqFilterConfig],
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    use crate::components::graphs::format_frequency;

    if filters.is_empty() {
        return div()
            .child(render_empty_state(
                IconName::AudioWaveform,
                "No filters",
                theme,
            ))
            .into_any_element();
    }

    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(d.gap)
        .children(filters.iter().enumerate().map(|(i, f)| {
            let gain_color = if f.gain_db > 0.5 {
                theme.success
            } else if f.gain_db < -0.5 {
                theme.error
            } else {
                theme.text_muted
            };

            div()
                .px(d.pad_x)
                .py(d.pad_y)
                .bg(theme.background_secondary)
                .rounded(d.r_md)
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(d.grid)
                .min_w(rems(5.0))
                // Filter number and type
                .child(
                    div()
                        .flex()
                        .gap(d.grid)
                        .items_center()
                        .child(
                            Text::new(format!("{}", i + 1))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(theme.text_primary),
                        )
                        .child(
                            Text::new(&f.filter_type)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Frequency
                .child(
                    Text::new(format_frequency(f.frequency))
                        .weight(TextWeight::Semibold)
                        .size(TextSize::Xs)
                        .color(theme.text_primary),
                )
                // Gain and Q
                .child(
                    div()
                        .flex()
                        .gap(d.gap)
                        .child(
                            Text::new(format!("{:+.1}dB", f.gain_db))
                                .weight(TextWeight::Bold)
                                .size(TextSize::Xs)
                                .color(gain_color),
                        )
                        .child(Text::caption(format!("Q:{:.1}", f.q))),
                )
        }))
        .into_any_element()
}

/// Render group delay comparison graph (before vs after optimization)
fn render_group_delay_graph(
    gd_before: Option<&[(f64, f64)]>,
    gd_after: Option<&[(f64, f64)]>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    use crate::components::graphs::common::theme_to_chart_theme;
    use gpui_px::{LegendPosition, ScaleType, line};

    const GRAPH_WIDTH: f32 = 800.0;
    const GRAPH_HEIGHT: f32 = 200.0;
    const BLUE: u32 = 0x1f77b4;
    const ORANGE: u32 = 0xff7f0e;

    let chart_theme = theme_to_chart_theme(theme);

    // Use the before curve for the x-axis, or after if before is missing
    let Some(reference) = gd_before.or(gd_after) else {
        return div().into_any_element();
    };
    let frequencies: Vec<f64> = reference.iter().map(|(f, _)| *f).collect();

    // Filter to 20Hz-20kHz and compute y range
    let in_range = |f: f64| (20.0..=20000.0).contains(&f);

    let before_values: Option<Vec<f64>> = gd_before.map(|b| b.iter().map(|(_, d)| *d).collect());

    let after_values: Option<Vec<f64>> = gd_after.map(|after| {
        // Interpolate after to match the reference frequency grid
        frequencies
            .iter()
            .map(|&f| {
                if let Some(pos) = after.windows(2).position(|w| w[0].0 <= f && f <= w[1].0) {
                    let (f0, d0) = after[pos];
                    let (f1, d1) = after[pos + 1];
                    let t = if (f1 - f0).abs() > 1e-12 {
                        (f - f0) / (f1 - f0)
                    } else {
                        0.0
                    };
                    d0 + t * (d1 - d0)
                } else {
                    after.last().map(|(_, d)| *d).unwrap_or(0.0)
                }
            })
            .collect()
    });

    // Compute y range from whichever datasets are present
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (i, &f) in frequencies.iter().enumerate() {
        if in_range(f) {
            for vals in [&before_values, &after_values].into_iter().flatten() {
                if let Some(&v) = vals.get(i)
                    && v.is_finite()
                {
                    y_min = y_min.min(v);
                    y_max = y_max.max(v);
                }
            }
        }
    }
    // Fallback if no valid data found in range
    if !y_min.is_finite() || !y_max.is_finite() || y_min >= y_max {
        y_min = -5.0;
        y_max = 50.0;
    }
    // Round to nice bounds with some padding
    let margin = (y_max - y_min).max(1.0) * 0.1;
    y_min = (y_min - margin).floor();
    y_max = (y_max + margin).ceil();

    // Build chart: use whichever series is available as primary.
    // Only show "Before" when the measurement actually had phase data —
    // don't draw a misleading flat line at 0ms.
    let (primary_values, primary_label, primary_color) = if let Some(ref bv) = before_values {
        (bv.as_slice(), "Before", BLUE)
    } else if let Some(ref av) = after_values {
        (av.as_slice(), "After", ORANGE)
    } else {
        // Should not happen due to .when() guard, but handle gracefully
        return div().into_any_element();
    };

    let mut chart_builder = line(&frequencies, primary_values)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_range(y_min, y_max)
        .y_label("GD (ms)")
        .label(primary_label)
        .legend_position(LegendPosition::Bottom)
        .color(primary_color)
        .stroke_width(1.5)
        .opacity(0.7)
        .theme(chart_theme)
        .size(GRAPH_WIDTH, GRAPH_HEIGHT);

    // Add the secondary series only if it's different from the primary
    if before_values.is_some()
        && let Some(ref av) = after_values
    {
        chart_builder = chart_builder.add_series(av, Some("After"), ORANGE, 1.5, 0.9);
    }

    let chart_element: Option<gpui::AnyElement> =
        chart_builder.build().ok().map(|c| c.into_any_element());

    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new("Group Delay")
                .weight(TextWeight::Semibold)
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .when_some(chart_element, |div, el| div.child(el))
        .into_any_element()
}
