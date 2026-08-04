use super::consts::ROOM_EQ_PYTHON_DEFAULT_SMOOTHING_OCTAVES;
use super::interpolate::interpolate_log_frequency_at_db;
use super::misc::calculate_room_eq_log_trend;
use super::misc::count_points_in_domain;
use super::misc::finite_positive_frequency_range;
use super::misc::is_room_eq_sub_or_lfe_channel;
use super::misc::rgba_from_u32;
use super::room_eq_report_curve::RoomEqReportCurve;
use super::types::RoomEqChartSeries;
use super::types::RoomEqReportBassGroup;
use super::types::RoomEqReportBassHeadroom;
use super::types::RoomEqReportBassHeadroomOutput;
use super::types::RoomEqReportBassManagement;
use super::types::RoomEqReportBassRoute;
use super::types::RoomEqReportBassSubOutput;
use super::types::RoomEqReportChannel;
use super::types::RoomEqReportData;
use super::types::RoomEqReportDriverCurve;
use super::types::RoomEqReportEpaComparison;
use super::types::RoomEqReportEpaScore;
use super::types::RoomEqReportEqPass;
use super::types::RoomEqReportFilter;
use super::types::RoomEqReportFirMasking;
use super::types::RoomEqReportIr;
use super::types::RoomEqTrendSeries;
use gpui::*;
use sotf_audio::signal_analysis as dsp;
use std::collections::BTreeSet;

pub fn room_eq_trend_fit_domain(_channel_name: &str, frequencies: &[f64]) -> Option<(f64, f64)> {
    let (data_min, data_max) = finite_positive_frequency_range(frequencies)?;
    let (fit_min, fit_max) = (100.0_f64, 10_000.0_f64);

    let min_freq = fit_min.max(data_min);
    let max_freq = fit_max.min(data_max);
    (max_freq > min_freq).then_some((min_freq, max_freq))
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
    score: &autoeq::roomeq_model::EpaScore,
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

pub(super) fn room_eq_route_color(kind: &str) -> Hsla {
    match kind {
        "main_highpass_to_self" | "main_highpass" => Hsla::from(rgba_from_u32(0x4a90d9)),
        "redirected_bass_lowpass_to_sub" => Hsla::from(rgba_from_u32(0x2ecc71)),
        "lfe_lowpass_to_sub" => Hsla::from(rgba_from_u32(0xe67e22)),
        _ => Hsla::from(rgba_from_u32(0x7f8c8d)),
    }
}

pub(super) fn room_eq_d3rs_path_to_gpui(
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

pub(super) fn room_eq_is_named_main_channel(channel_name: &str, target: &str) -> bool {
    let normalized = channel_name.trim().to_ascii_uppercase();
    match target {
        "L" => matches!(normalized.as_str(), "L" | "LEFT"),
        "R" => matches!(normalized.as_str(), "R" | "RIGHT"),
        "C" => matches!(normalized.as_str(), "C" | "CENTER" | "CENTRE"),
        _ => false,
    }
}

pub(super) fn room_eq_phase_points(curve: &RoomEqReportCurve) -> Option<Vec<(f64, f64)>> {
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

pub(super) fn room_eq_trend_for_curve(
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

pub(super) fn room_eq_trend_coefficients(
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

pub(super) fn room_eq_average_value_in_domain(
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

pub(super) fn room_eq_chart_y_range<'a>(
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

pub(super) fn room_eq_smoothed_spl(curve: &RoomEqReportCurve, smoothing_octaves: f64) -> Vec<f64> {
    if smoothing_octaves <= 0.0 {
        return curve.spl.clone();
    }
    dsp::smooth_response_f64(&curve.freq, &curve.spl, smoothing_octaves)
        .into_iter()
        .map(|value| if value.is_finite() { value } else { 0.0 })
        .collect()
}

pub(super) fn room_eq_average_spl_in_range(
    curve: &RoomEqReportCurve,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
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
