//! Home-cinema role and layout helpers for RoomEQ.
//!
//! This intentionally mirrors the channel-label vocabulary used by
//! `sotf-host` speaker configurations without making `autoeq` depend on the
//! host crate. RoomEQ needs the same semantic model for target bands, channel
//! matching, and multi-seat diagnostics.

use num_complex::Complex64;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::PI;

use super::types::{
    BassHeadroomModelConfig, CrossoverConfig, MultiMeasurementConfig, MultiMeasurementStrategy,
    MultiSeatConfig, RoleTargetConfig, RoomConfig, SpatialRobustnessSerdeConfig, SpeakerConfig,
    SubwooferStrategy, SystemConfig, SystemModel, TargetResponseConfig, TargetShape,
    UserPreference,
};
use crate::{Curve, MeasurementSource};

mod types;
pub use types::*;

pub fn analyze_layout(config: &RoomConfig) -> HomeCinemaLayoutReport {
    let mut channels = Vec::new();
    let channel_names = logical_channel_names(config);
    for name in channel_names {
        let role = role_for_channel(&name);
        let role_group = role.group();
        channels.push(HomeCinemaChannelReport {
            name: name.clone(),
            role,
            role_group,
            is_bass_managed: role.is_bass_managed_candidate(),
            matching_group: matching_group_key_for_role(role).map(str::to_string),
            target_band_hz: role.default_target_band_hz(),
            target_profile: target_profile_for_role(config, role),
            target_advisory: target_advisory_for_role(config, role),
        });
    }

    let lfe_channels = channels
        .iter()
        .filter(|ch| ch.role == HomeCinemaRole::Lfe)
        .count();
    let subwoofer_channels = channels
        .iter()
        .filter(|ch| ch.role == HomeCinemaRole::Subwoofer)
        .count();
    let height_channels = channels.iter().filter(|ch| ch.role.is_height()).count();
    let bed_channels = channels
        .iter()
        .filter(|ch| ch.role.is_bed_channel())
        .count();
    let layout = detect_layout_name(bed_channels, lfe_channels, height_channels);

    HomeCinemaLayoutReport {
        layout,
        bed_channels,
        lfe_channels,
        height_channels,
        subwoofer_channels,
        channels,
    }
}

pub fn effective_bass_management(config: &RoomConfig) -> Option<EffectiveBassManagement> {
    let system = config.system.as_ref()?;
    let sub_system = system.subwoofers.as_ref()?;
    let bm = system.bass_management.clone().unwrap_or_default();
    if !bm.enabled {
        return None;
    }
    let (crossover_type, crossover_frequency_hz, advisory) =
        resolve_crossover_descriptor(config, sub_system.crossover.as_deref());

    Some(EffectiveBassManagement {
        config: bm,
        crossover_type,
        crossover_frequency_hz,
        advisory,
    })
}

pub fn bass_output_role(_config: &RoomConfig, system: &SystemConfig) -> String {
    if let Some(bm) = system.bass_management.as_ref()
        && system.speakers.contains_key(&bm.lfe_channel)
    {
        return bm.lfe_channel.clone();
    }
    if system.speakers.contains_key("LFE") {
        return "LFE".to_string();
    }
    let mut candidates: Vec<_> = system
        .speakers
        .keys()
        .filter(|role| role_for_channel(role).is_sub_or_lfe())
        .cloned()
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| "LFE".to_string())
}

pub fn bass_management_report(
    config: &RoomConfig,
    applied_sub_gain_db: Option<f64>,
    gain_limited: bool,
) -> Option<BassManagementReport> {
    bass_management_report_with_optimization(config, applied_sub_gain_db, gain_limited, None)
}

pub fn bass_management_report_with_optimization(
    config: &RoomConfig,
    applied_sub_gain_db: Option<f64>,
    gain_limited: bool,
    optimization: Option<BassManagementOptimizationReport>,
) -> Option<BassManagementReport> {
    bass_management_report_with_optimization_and_sample_rate(
        config,
        applied_sub_gain_db,
        gain_limited,
        optimization,
        48_000.0,
    )
}

pub fn bass_management_report_with_optimization_and_sample_rate(
    config: &RoomConfig,
    applied_sub_gain_db: Option<f64>,
    gain_limited: bool,
    optimization: Option<BassManagementOptimizationReport>,
    sample_rate: f64,
) -> Option<BassManagementReport> {
    let effective = effective_bass_management(config)?;
    let routing_graph = bass_management_routing_graph(config, optimization.as_ref());
    let groups = bass_management_groups(config, optimization.as_ref());
    let sub_outputs =
        bass_management_sub_outputs(config, optimization.as_ref(), routing_graph.as_ref());
    let headroom_simulation = simulate_bass_bus_headroom(
        routing_graph.as_ref(),
        &effective.config.headroom_model,
        effective.config.headroom_margin_db,
        sample_rate,
    );
    let physical_sub_output = config
        .system
        .as_ref()
        .map(|system| bass_output_role(config, system))
        .unwrap_or_else(|| effective.config.lfe_channel.clone());
    let signal_flow = bass_management_signal_flow(
        config,
        &effective,
        &physical_sub_output,
        optimization.as_ref(),
    );
    let redirected_bass_channel_count = signal_flow
        .iter()
        .filter(|entry| entry.redirects_bass)
        .count();
    let signal_flow_advisories =
        bass_management_signal_flow_advisories(&effective, redirected_bass_channel_count);
    let mut advisory = effective.advisory;
    if gain_limited {
        advisory = if advisory == "ok" {
            "sub_gain_limited_for_headroom".to_string()
        } else {
            format!("{advisory};sub_gain_limited_for_headroom")
        };
    }
    if effective.config.lfe_playback_gain_db.abs() > 0.01
        && !effective.config.apply_lfe_gain_to_chain
    {
        advisory = if advisory == "ok" {
            "lfe_gain_reported_not_applied_to_physical_sub_chain".to_string()
        } else {
            format!("{advisory};lfe_gain_reported_not_applied_to_physical_sub_chain")
        };
    }

    Some(BassManagementReport {
        enabled: true,
        crossover_type: effective.crossover_type,
        crossover_frequency_hz: effective.crossover_frequency_hz,
        redirected_bass_enabled: effective.config.redirect_bass,
        lfe_channel: effective.config.lfe_channel,
        lfe_playback_gain_db: effective.config.lfe_playback_gain_db,
        lfe_gain_applied_to_chain: effective.config.apply_lfe_gain_to_chain,
        sub_trim_db: effective.config.sub_trim_db,
        max_sub_boost_db: effective.config.max_sub_boost_db,
        headroom_margin_db: effective.config.headroom_margin_db,
        applied_sub_gain_db,
        gain_limited,
        physical_sub_output,
        redirected_bass_channel_count,
        main_high_pass_hz: effective.crossover_frequency_hz,
        sub_low_pass_hz: effective.crossover_frequency_hz,
        lfe_headroom_required_db: effective.config.lfe_playback_gain_db.max(0.0)
            + effective.config.headroom_margin_db,
        signal_flow,
        signal_flow_advisories,
        routing_graph,
        optimization,
        groups,
        sub_outputs,
        headroom_simulation,
        advisory,
    })
}

pub fn bass_management_routing_graph(
    config: &RoomConfig,
    optimization: Option<&BassManagementOptimizationReport>,
) -> Option<BassManagementRoutingGraph> {
    let system = config.system.as_ref()?;
    let effective = effective_bass_management(config)?;
    let bass_role = bass_output_role(config, system);
    let mut channel_order = logical_channel_names(config);
    channel_order.sort_by(|a, b| {
        home_cinema_role_sort_index(role_for_channel(a))
            .cmp(&home_cinema_role_sort_index(role_for_channel(b)))
            .then_with(|| a.cmp(b))
    });
    let sub_outputs = resolved_bass_sub_outputs(&bass_role, optimization);
    for output in &sub_outputs {
        if !channel_order.contains(&output.output_role) {
            channel_order.push(output.output_role.clone());
        }
    }
    let destination_index = channel_order
        .iter()
        .position(|name| name == &bass_role)
        .unwrap_or_else(|| {
            channel_order.push(bass_role.clone());
            channel_order.len() - 1
        });

    let mut routes = Vec::new();
    for (source_index, source_channel) in channel_order.iter().enumerate() {
        let role = role_for_channel(source_channel);
        let is_lfe = role == HomeCinemaRole::Lfe || source_channel == &effective.config.lfe_channel;
        let group_id = group_id_for_role(role);
        let crossover = resolved_group_crossover(config, group_id, &effective, optimization);
        let route_settings = resolved_group_route_settings(group_id, optimization);

        if role.is_bass_managed_candidate() {
            routes.push(BassManagementRoute {
                group_id: Some(group_id.to_string()),
                source_channel: source_channel.clone(),
                source_index,
                destination: source_channel.clone(),
                destination_index: source_index,
                pre_chain_channel: Some(source_channel.clone()),
                post_chain_channel: Some(source_channel.clone()),
                route_kind: "main_highpass_to_self".to_string(),
                crossover_type: crossover.crossover_type.clone(),
                high_pass_hz: crossover.frequency_hz,
                low_pass_hz: None,
                gain_db: 0.0,
                gain_linear: 1.0,
                matrix_gain: 1.0,
                delay_ms: route_settings.main_delay_ms,
                polarity_inverted: false,
            });
        }

        if effective.config.redirect_bass && role.is_bass_managed_candidate() {
            for sub_output in &sub_outputs {
                let destination_index = channel_order
                    .iter()
                    .position(|name| name == &sub_output.output_role)
                    .unwrap_or(destination_index);
                let route_gain_db = route_settings.trim_db + sub_output.gain_db;
                routes.push(BassManagementRoute {
                    group_id: Some(group_id.to_string()),
                    source_channel: source_channel.clone(),
                    source_index,
                    destination: sub_output.output_role.clone(),
                    destination_index,
                    pre_chain_channel: Some(bass_role.clone()),
                    post_chain_channel: Some(sub_output.output_role.clone()),
                    route_kind: "redirected_bass_lowpass_to_sub".to_string(),
                    crossover_type: crossover.crossover_type.clone(),
                    high_pass_hz: None,
                    low_pass_hz: crossover.frequency_hz,
                    gain_db: route_gain_db,
                    gain_linear: 10.0_f64.powf(route_gain_db / 20.0),
                    matrix_gain: 10.0_f64.powf(route_gain_db / 20.0),
                    delay_ms: route_settings.bass_route_delay_ms + sub_output.delay_ms,
                    polarity_inverted: route_settings.polarity_inverted
                        ^ sub_output.polarity_inverted,
                });
            }
        }

        if is_lfe {
            let route_gain_db = if effective.config.apply_lfe_gain_to_chain {
                0.0
            } else {
                effective.config.lfe_playback_gain_db
            };
            let lfe_crossover = resolved_group_crossover(config, "lfe", &effective, optimization);
            let lfe_settings = resolved_group_route_settings("lfe", optimization);
            for sub_output in &sub_outputs {
                let destination_index = channel_order
                    .iter()
                    .position(|name| name == &sub_output.output_role)
                    .unwrap_or(destination_index);
                let output_gain_db = route_gain_db + sub_output.gain_db;
                routes.push(BassManagementRoute {
                    group_id: Some("lfe".to_string()),
                    source_channel: source_channel.clone(),
                    source_index,
                    destination: sub_output.output_role.clone(),
                    destination_index,
                    pre_chain_channel: Some(bass_role.clone()),
                    post_chain_channel: Some(sub_output.output_role.clone()),
                    route_kind: "lfe_lowpass_to_sub".to_string(),
                    crossover_type: lfe_crossover.crossover_type.clone(),
                    high_pass_hz: None,
                    low_pass_hz: lfe_crossover.frequency_hz,
                    gain_db: output_gain_db,
                    gain_linear: 10.0_f64.powf(output_gain_db / 20.0),
                    matrix_gain: 10.0_f64.powf(output_gain_db / 20.0),
                    delay_ms: lfe_settings.bass_route_delay_ms + sub_output.delay_ms,
                    polarity_inverted: lfe_settings.polarity_inverted
                        ^ sub_output.polarity_inverted,
                });
            }
        }
    }

    let bass_routes: Vec<&BassManagementRoute> = routes
        .iter()
        .filter(|route| {
            route.destination == bass_role && route.destination_index == destination_index
        })
        .collect();
    let matrix =
        (sub_outputs.len() == 1 && !bass_routes.is_empty()).then(|| BassManagementMatrix {
            input_channel_map: bass_routes.iter().map(|route| route.source_index).collect(),
            output_channel_map: vec![destination_index],
            matrix: bass_routes
                .iter()
                .map(|route| route.matrix_gain as f32)
                .collect(),
            route_count: bass_routes.len(),
        });

    let mut advisories = Vec::new();
    if effective.config.apply_lfe_gain_to_chain {
        advisories.push("legacy_lfe_gain_applied_to_shared_sub_chain".to_string());
    }
    if effective.config.redirect_bass && matrix.is_none() && sub_outputs.len() > 1 {
        advisories.push("branch_routing_required_for_multiple_sub_outputs".to_string());
    } else if effective.config.redirect_bass && matrix.is_none() {
        advisories.push("redirect_bass_enabled_but_no_matrix_routes".to_string());
    }
    if advisories.is_empty() {
        advisories.push("ok".to_string());
    }

    Some(BassManagementRoutingGraph {
        physical_sub_output: bass_role,
        input_channels: channel_order.clone(),
        output_channels: channel_order,
        routes,
        matrix,
        advisories,
    })
}

pub fn bass_management_matrix_metadata(graph: &BassManagementRoutingGraph) -> serde_json::Value {
    serde_json::json!({
        "purpose": "home_cinema_bass_management",
        "physical_sub_output": graph.physical_sub_output,
        "routes": graph.routes,
        "advisories": graph.advisories,
    })
}

pub fn group_id_for_role(role: HomeCinemaRole) -> &'static str {
    match role {
        HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight | HomeCinemaRole::Center => "lcr",
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight => "surround",
        HomeCinemaRole::WideLeft | HomeCinemaRole::WideRight => "wide",
        role if role.is_height() => "height",
        HomeCinemaRole::Lfe => "lfe",
        HomeCinemaRole::Subwoofer => "sub",
        HomeCinemaRole::Unknown => "unknown",
        _ => "unknown",
    }
}

#[derive(Debug, Clone)]
struct ResolvedGroupCrossover {
    crossover_type: String,
    frequency_hz: Option<f64>,
    configured_hz: Option<f64>,
    frequency_range: Option<(f64, f64)>,
    missing_config_key: Option<String>,
}

fn resolved_group_crossover(
    config: &RoomConfig,
    group_id: &str,
    effective: &EffectiveBassManagement,
    optimization: Option<&BassManagementOptimizationReport>,
) -> ResolvedGroupCrossover {
    if let Some(group) = optimization_group_result(optimization, group_id) {
        return ResolvedGroupCrossover {
            crossover_type: group.crossover_type.clone(),
            frequency_hz: group.selected_crossover_hz,
            configured_hz: group.configured_crossover_hz,
            frequency_range: None,
            missing_config_key: None,
        };
    }

    let fallback_type = optimization
        .map(|o| o.crossover_type.clone())
        .unwrap_or_else(|| effective.crossover_type.clone());
    let fallback_hz = optimization
        .and_then(|o| o.optimized_crossover_hz)
        .or(effective.crossover_frequency_hz);

    let Some(key) = effective.config.group_crossovers.get(group_id) else {
        return ResolvedGroupCrossover {
            crossover_type: fallback_type,
            frequency_hz: fallback_hz,
            configured_hz: effective.crossover_frequency_hz,
            frequency_range: None,
            missing_config_key: None,
        };
    };
    let Some(crossover) = config
        .crossovers
        .as_ref()
        .and_then(|crossovers| crossovers.get(key))
    else {
        return ResolvedGroupCrossover {
            crossover_type: fallback_type,
            frequency_hz: fallback_hz,
            configured_hz: effective.crossover_frequency_hz,
            frequency_range: None,
            missing_config_key: Some(key.clone()),
        };
    };
    let configured_hz = crossover.frequency.or_else(|| {
        crossover
            .frequency_range
            .map(|(min, max)| (min.max(1.0) * max.max(1.0)).sqrt())
    });
    let crossover_type = if crossover.crossover_type.eq_ignore_ascii_case("auto") {
        fallback_type
    } else {
        crossover.crossover_type.clone()
    };
    ResolvedGroupCrossover {
        crossover_type,
        frequency_hz: configured_hz.or(fallback_hz),
        configured_hz,
        frequency_range: crossover.frequency_range,
        missing_config_key: None,
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedGroupRouteSettings {
    main_delay_ms: f64,
    bass_route_delay_ms: f64,
    polarity_inverted: bool,
    trim_db: f64,
}

fn resolved_group_route_settings(
    group_id: &str,
    optimization: Option<&BassManagementOptimizationReport>,
) -> ResolvedGroupRouteSettings {
    if let Some(group) = optimization_group_result(optimization, group_id) {
        return ResolvedGroupRouteSettings {
            main_delay_ms: group.main_delay_ms,
            bass_route_delay_ms: group.bass_route_delay_ms,
            polarity_inverted: group.polarity_inverted,
            trim_db: group.trim_db,
        };
    }

    ResolvedGroupRouteSettings {
        main_delay_ms: optimization.map(|o| o.main_delay_ms).unwrap_or(0.0),
        bass_route_delay_ms: optimization.map(|o| o.sub_delay_ms).unwrap_or(0.0),
        polarity_inverted: optimization
            .map(|o| o.sub_polarity_inverted)
            .unwrap_or(false),
        trim_db: 0.0,
    }
}

fn optimization_group_result<'a>(
    optimization: Option<&'a BassManagementOptimizationReport>,
    group_id: &str,
) -> Option<&'a BassManagementGroupReport> {
    optimization?
        .group_results
        .iter()
        .find(|group| group.group_id == group_id)
}

fn resolved_bass_sub_outputs(
    fallback_role: &str,
    optimization: Option<&BassManagementOptimizationReport>,
) -> Vec<BassManagementSubOutputReport> {
    if let Some(outputs) = optimization
        .map(|opt| opt.sub_output_results.clone())
        .filter(|outputs| !outputs.is_empty())
    {
        return outputs;
    }

    vec![BassManagementSubOutputReport {
        output_role: fallback_role.to_string(),
        gain_db: 0.0,
        delay_ms: 0.0,
        polarity_inverted: false,
        strategy_source: "single".to_string(),
        headroom_contribution_db: 0.0,
    }]
}

pub fn bass_management_groups(
    config: &RoomConfig,
    optimization: Option<&BassManagementOptimizationReport>,
) -> Vec<BassManagementGroupReport> {
    let Some(effective) = effective_bass_management(config) else {
        return Vec::new();
    };
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for channel in logical_channel_names(config) {
        let role = role_for_channel(&channel);
        if role.is_bass_managed_candidate() {
            grouped
                .entry(group_id_for_role(role).to_string())
                .or_default()
                .push(channel);
        }
    }

    grouped
        .into_iter()
        .map(|(group_id, roles)| {
            if let Some(group_report) = optimization_group_result(optimization, &group_id) {
                return group_report.clone();
            }
            let crossover = resolved_group_crossover(config, &group_id, &effective, optimization);
            let mut advisories = Vec::new();
            if !effective.config.optimize_groups {
                advisories.push("group_optimization_disabled".to_string());
            }
            if crossover.frequency_range.is_some()
                && optimization
                    .and_then(|o| o.optimized_crossover_hz)
                    .is_none()
            {
                advisories.push("group_crossover_range_not_optimized".to_string());
            }
            if let Some(key) = crossover.missing_config_key.as_ref() {
                advisories.push(format!("group_crossover_config_missing:{key}"));
            }
            if advisories.is_empty() {
                advisories.push("ok".to_string());
            }
            BassManagementGroupReport {
                group_id,
                roles,
                crossover_type: crossover.crossover_type,
                selected_crossover_hz: crossover.frequency_hz,
                configured_crossover_hz: crossover.configured_hz,
                main_delay_ms: optimization.map(|o| o.main_delay_ms).unwrap_or(0.0),
                bass_route_delay_ms: optimization.map(|o| o.sub_delay_ms).unwrap_or(0.0),
                polarity_inverted: optimization
                    .map(|o| o.sub_polarity_inverted)
                    .unwrap_or(false),
                trim_db: optimization.map(|o| o.applied_sub_gain_db).unwrap_or(0.0),
                objective_before: optimization.and_then(|o| o.objective_before),
                objective_after: optimization.and_then(|o| o.objective_after),
                advisories,
            }
        })
        .collect()
}

pub fn bass_management_sub_outputs(
    config: &RoomConfig,
    optimization: Option<&BassManagementOptimizationReport>,
    graph: Option<&BassManagementRoutingGraph>,
) -> Vec<BassManagementSubOutputReport> {
    if let Some(outputs) = optimization
        .map(|opt| opt.sub_output_results.clone())
        .filter(|outputs| !outputs.is_empty())
    {
        return outputs;
    }

    let Some(system) = config.system.as_ref() else {
        return Vec::new();
    };
    let strategy = system
        .subwoofers
        .as_ref()
        .map(|s| match s.config {
            SubwooferStrategy::Single => "single",
            SubwooferStrategy::Mso => "mso",
            SubwooferStrategy::Dba => "dba_front",
        })
        .unwrap_or("single");

    let mut outputs: Vec<String> = graph
        .map(|graph| {
            graph
                .routes
                .iter()
                .filter(|route| {
                    route.route_kind == "redirected_bass_lowpass_to_sub"
                        || route.route_kind == "lfe_lowpass_to_sub"
                })
                .map(|route| route.destination.clone())
                .collect()
        })
        .unwrap_or_default();
    outputs.sort();
    outputs.dedup();
    if outputs.is_empty() {
        outputs.push(bass_output_role(config, system));
    }

    outputs
        .into_iter()
        .map(|output_role| BassManagementSubOutputReport {
            output_role,
            gain_db: optimization.map(|o| o.applied_sub_gain_db).unwrap_or(0.0),
            delay_ms: optimization.map(|o| o.sub_delay_ms).unwrap_or(0.0),
            polarity_inverted: optimization
                .map(|o| o.sub_polarity_inverted)
                .unwrap_or(false),
            strategy_source: strategy.to_string(),
            headroom_contribution_db: optimization
                .and_then(|o| o.estimated_bass_bus_peak_gain_db)
                .unwrap_or(0.0),
        })
        .collect()
}

pub fn simulate_bass_bus_headroom(
    graph: Option<&BassManagementRoutingGraph>,
    model: &BassHeadroomModelConfig,
    headroom_margin_db: f64,
    sample_rate: f64,
) -> Option<BassBusHeadroomSimulationReport> {
    let graph = graph?;
    let mut per_output = Vec::new();
    let mut worst_rms = f64::NEG_INFINITY;
    let mut worst_peak = f64::NEG_INFINITY;
    let mut worst_lfe = f64::NEG_INFINITY;
    let mut worst_frequency = 20.0;
    let mut outputs: Vec<String> = graph
        .routes
        .iter()
        .filter(|route| {
            route.route_kind == "redirected_bass_lowpass_to_sub"
                || route.route_kind == "lfe_lowpass_to_sub"
        })
        .map(|route| route.destination.clone())
        .collect();
    outputs.sort();
    outputs.dedup();

    for output_role in outputs {
        let routes: Vec<_> = graph
            .routes
            .iter()
            .filter(|route| route.destination == output_role)
            .filter(|route| {
                route.route_kind == "redirected_bass_lowpass_to_sub"
                    || route.route_kind == "lfe_lowpass_to_sub"
            })
            .collect();
        let mut output_worst_rms = f64::NEG_INFINITY;
        let mut output_worst_peak = f64::NEG_INFINITY;
        let mut output_worst_lfe = f64::NEG_INFINITY;
        let mut output_worst_freq = 20.0;

        for idx in 0..96 {
            let t = idx as f64 / 95.0;
            let freq = 20.0_f64 * (250.0_f64 / 20.0_f64).powf(t);
            let route_gains: Vec<Complex64> = routes
                .iter()
                .map(|route| bass_route_complex_gain(route, freq, sample_rate))
                .collect();
            let coherent = route_gains.iter().map(|g| g.norm()).sum::<f64>();
            let mut rms_power = 0.0;
            for (i, route_i) in routes.iter().enumerate() {
                for (j, route_j) in routes.iter().enumerate() {
                    let corr = bass_programme_correlation(
                        role_for_channel(&route_i.source_channel),
                        role_for_channel(&route_j.source_channel),
                        model,
                    );
                    rms_power += (route_gains[i] * route_gains[j].conj()).re * corr;
                }
            }
            let rms = rms_power.max(0.0).sqrt();
            let lfe = routes
                .iter()
                .zip(route_gains.iter())
                .filter(|(route, _)| route.route_kind == "lfe_lowpass_to_sub")
                .map(|(_, gain)| gain.norm())
                .sum::<f64>();
            let rms_db = linear_to_db(rms);
            let coherent_db = linear_to_db(coherent);
            let lfe_db = linear_to_db(lfe);
            output_worst_rms = output_worst_rms.max(rms_db);
            output_worst_lfe = output_worst_lfe.max(lfe_db);
            if coherent_db > output_worst_peak {
                output_worst_peak = coherent_db;
                output_worst_freq = freq;
            }
        }

        worst_rms = worst_rms.max(output_worst_rms);
        if output_worst_peak > worst_peak {
            worst_peak = output_worst_peak;
            worst_lfe = output_worst_lfe;
            worst_frequency = output_worst_freq;
        }
        per_output.push(BassBusOutputHeadroomReport {
            output_role,
            rms_bus_gain_db: output_worst_rms,
            coherent_peak_gain_db: output_worst_peak,
            lfe_contribution_db: output_worst_lfe,
            pass: output_worst_peak <= headroom_margin_db,
            margin_db: headroom_margin_db - output_worst_peak,
            worst_frequency_hz: output_worst_freq,
        });
    }

    if per_output.is_empty() {
        return None;
    }

    Some(BassBusHeadroomSimulationReport {
        model: "cinema_correlated".to_string(),
        frequency_range_hz: (20.0, 250.0),
        rms_bus_gain_db: worst_rms,
        coherent_peak_gain_db: worst_peak,
        lfe_contribution_db: worst_lfe,
        headroom_margin_db,
        pass: worst_peak <= headroom_margin_db,
        margin_db: headroom_margin_db - worst_peak,
        worst_frequency_hz: worst_frequency,
        per_output,
    })
}

fn bass_route_complex_gain(route: &BassManagementRoute, freq: f64, sample_rate: f64) -> Complex64 {
    let polarity = if route.polarity_inverted { -1.0 } else { 1.0 };
    let delay_phase = -2.0 * PI * freq * route.delay_ms / 1000.0;
    let mut response =
        Complex64::from_polar(route_effective_gain_linear(route) * polarity, delay_phase);
    if let Some(filter_response) = route_crossover_response(route, freq, sample_rate) {
        response *= filter_response;
    }
    response
}

fn route_effective_gain_linear(route: &BassManagementRoute) -> f64 {
    if route.gain_db.abs() > 0.01 && (route.matrix_gain - 1.0).abs() < 1e-6 {
        route.gain_linear
    } else {
        route.matrix_gain
    }
}

fn route_crossover_response(
    route: &BassManagementRoute,
    freq: f64,
    sample_rate: f64,
) -> Option<Complex64> {
    let is_lowpass = route.low_pass_hz.is_some();
    let crossover_hz = route.low_pass_hz.or(route.high_pass_hz)?;
    let filters = crossover_filters_for_headroom(
        &route.crossover_type,
        crossover_hz,
        is_lowpass,
        sample_rate,
    );
    if filters.is_empty() {
        return None;
    }
    let freqs = ndarray::arr1(&[freq]);
    crate::response::compute_peq_complex_response(&filters, &freqs, sample_rate)
        .into_iter()
        .next()
}

fn crossover_filters_for_headroom(
    crossover_type: &str,
    freq: f64,
    is_lowpass: bool,
    sample_rate: f64,
) -> Vec<math_audio_iir_fir::Biquad> {
    use math_audio_iir_fir::{
        peq_butterworth_highpass, peq_butterworth_lowpass, peq_linkwitzriley_highpass,
        peq_linkwitzriley_lowpass,
    };
    let peq = match crossover_type.to_lowercase().as_str() {
        "lr12" | "lr2" | "linkwitzriley12" | "linkwitzriley2" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(2, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(2, freq, sample_rate)
            }
        }
        "lr48" | "lr8" | "linkwitzriley48" | "linkwitzriley8" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(8, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(8, freq, sample_rate)
            }
        }
        "bw12" | "butterworth12" | "bw2" | "butterworth2" => {
            if is_lowpass {
                peq_butterworth_lowpass(2, freq, sample_rate)
            } else {
                peq_butterworth_highpass(2, freq, sample_rate)
            }
        }
        "bw24" | "butterworth24" | "bw4" | "butterworth4" => {
            if is_lowpass {
                peq_butterworth_lowpass(4, freq, sample_rate)
            } else {
                peq_butterworth_highpass(4, freq, sample_rate)
            }
        }
        "none" => Vec::new(),
        _ => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(4, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(4, freq, sample_rate)
            }
        }
    };
    peq.into_iter().map(|(_, biquad)| biquad).collect()
}

fn bass_programme_correlation(
    a: HomeCinemaRole,
    b: HomeCinemaRole,
    model: &BassHeadroomModelConfig,
) -> f64 {
    if a == b {
        return 1.0;
    }
    if a == HomeCinemaRole::Lfe || b == HomeCinemaRole::Lfe {
        return 0.0;
    }
    let a_group = group_id_for_role(a);
    let b_group = group_id_for_role(b);
    match (a_group, b_group) {
        ("lcr", "lcr") => {
            if matches!(
                (a, b),
                (HomeCinemaRole::FrontLeft, HomeCinemaRole::FrontRight)
                    | (HomeCinemaRole::FrontRight, HomeCinemaRole::FrontLeft)
            ) {
                model.lr_correlation
            } else {
                model.lcr_correlation
            }
        }
        ("surround", "surround")
        | ("height", "height")
        | ("surround", "height")
        | ("height", "surround") => model.surround_height_correlation,
        _ => 0.25,
    }
}

fn linear_to_db(value: f64) -> f64 {
    20.0 * value.max(1e-12).log10()
}

pub fn estimated_bass_bus_peak_gain_db(
    graph: Option<&BassManagementRoutingGraph>,
    applied_sub_gain_db: f64,
) -> Option<f64> {
    let graph = graph?;
    let route_sum: f64 = graph
        .routes
        .iter()
        .filter(|route| route.destination == graph.physical_sub_output)
        .map(|route| route.gain_linear.abs())
        .sum();
    (route_sum > 0.0).then(|| 20.0 * route_sum.log10() + applied_sub_gain_db)
}

pub fn estimated_bass_bus_peak_gain_db_for_config(
    config: &RoomConfig,
    graph: Option<&BassManagementRoutingGraph>,
    applied_sub_gain_db: f64,
    sample_rate: f64,
) -> Option<f64> {
    let effective = effective_bass_management(config)?;
    simulate_bass_bus_headroom(
        graph,
        &effective.config.headroom_model,
        effective.config.headroom_margin_db,
        sample_rate,
    )
    .map(|report| report.coherent_peak_gain_db)
    .or_else(|| estimated_bass_bus_peak_gain_db(graph, applied_sub_gain_db))
}

fn home_cinema_role_sort_index(role: HomeCinemaRole) -> usize {
    match role {
        HomeCinemaRole::FrontLeft => 0,
        HomeCinemaRole::FrontRight => 1,
        HomeCinemaRole::Center => 2,
        HomeCinemaRole::Lfe | HomeCinemaRole::Subwoofer => 3,
        HomeCinemaRole::SideSurroundLeft => 4,
        HomeCinemaRole::SideSurroundRight => 5,
        HomeCinemaRole::RearSurroundLeft => 6,
        HomeCinemaRole::RearSurroundRight => 7,
        HomeCinemaRole::WideLeft => 8,
        HomeCinemaRole::WideRight => 9,
        HomeCinemaRole::TopFrontLeft => 10,
        HomeCinemaRole::TopFrontRight => 11,
        HomeCinemaRole::TopMiddleLeft => 12,
        HomeCinemaRole::TopMiddleRight => 13,
        HomeCinemaRole::TopRearLeft => 14,
        HomeCinemaRole::TopRearRight => 15,
        HomeCinemaRole::Unknown => 99,
    }
}

pub fn limited_sub_gain(
    requested_gain_db: f64,
    bass_management: Option<&EffectiveBassManagement>,
) -> (f64, bool) {
    let Some(bm) = bass_management else {
        return (requested_gain_db, false);
    };
    let with_trim = requested_gain_db + bm.config.sub_trim_db;
    let max_boost = bm.config.max_sub_boost_db.max(0.0);
    if with_trim > max_boost {
        (max_boost, true)
    } else {
        (with_trim, false)
    }
}

pub fn multi_seat_coverage(config: &RoomConfig) -> MultiSeatCoverageReport {
    let mut by_role_group: BTreeMap<String, usize> = BTreeMap::new();
    let mut channels_with_multiple_measurements = 0;
    let mut non_sub_channel_count = 0;
    let mut non_sub_channels_with_multiple_measurements = 0;
    let mut max_seat_count = 0;

    for (channel, speaker) in logical_speaker_configs(config) {
        let role = role_for_channel(&channel);
        let is_non_sub = !role.is_sub_or_lfe();
        if is_non_sub {
            non_sub_channel_count += 1;
        }
        let Some(seat_count) = speaker_measurement_count(&speaker) else {
            continue;
        };
        if seat_count < 2 {
            continue;
        }

        channels_with_multiple_measurements += 1;
        max_seat_count = max_seat_count.max(seat_count);
        if is_non_sub {
            non_sub_channels_with_multiple_measurements += 1;
        }
        *by_role_group
            .entry(role_group_key(role.group()).to_string())
            .or_insert(0) += 1;
    }

    MultiSeatCoverageReport {
        channels_with_multiple_measurements,
        non_sub_channel_count,
        non_sub_channels_with_multiple_measurements,
        max_seat_count,
        by_role_group,
        all_channel_correction_ready: non_sub_channel_count > 0
            && non_sub_channels_with_multiple_measurements == non_sub_channel_count
            && max_seat_count >= 2,
        recommended_scope: multi_seat_recommended_scope(
            channels_with_multiple_measurements,
            non_sub_channel_count,
            non_sub_channels_with_multiple_measurements,
        )
        .to_string(),
        advisories: multi_seat_coverage_advisories(
            channels_with_multiple_measurements,
            non_sub_channel_count,
            non_sub_channels_with_multiple_measurements,
            max_seat_count,
        ),
    }
}

pub fn derive_all_channel_multiseat_config(
    config: &RoomConfig,
    channel_name: &str,
    source: &MeasurementSource,
) -> Option<MultiMeasurementConfig> {
    if config.optimizer.multi_measurement.is_some() || !all_channel_multiseat_enabled(config) {
        return None;
    }
    let role = role_for_channel(channel_name);
    if role.is_sub_or_lfe() || measurement_source_count(source).unwrap_or(0) < 2 {
        return None;
    }
    let curves = crate::read::load_source_individual(source).ok()?;
    if curves.len() < 2 || !curves_share_frequency_grid(&curves) {
        return None;
    }
    let policy = all_channel_multiseat_policy(config);
    if policy.primary_seat >= curves.len() {
        return None;
    }
    let (weights, weight_advisories) = resolve_all_channel_seat_weights(&policy, curves.len());
    if !weight_advisories.is_empty() {
        return None;
    }
    Some(MultiMeasurementConfig {
        strategy: policy.all_channel_strategy,
        weights: Some(weights),
        variance_lambda: 1.0,
        spatial_robustness: Some(default_all_channel_spatial_robustness()),
        bootstrap_uncertainty: None,
    })
}

pub fn all_channel_multiseat_acceptance(
    config: &RoomConfig,
    channel_name: &str,
    source: &MeasurementSource,
    initial_curve: &Curve,
    final_curve: &Curve,
) -> AllChannelMultiSeatAcceptance {
    const TARGET_FIT_COLLAPSE_TOLERANCE_DB: f64 = 0.5;
    const BROADBAND_LEVEL_SHIFT_TOLERANCE_DB: f64 = 3.0;

    let role = role_for_channel(channel_name);
    let policy = all_channel_multiseat_policy(config);
    let mut advisories = Vec::new();

    if role.is_sub_or_lfe() {
        return AllChannelMultiSeatAcceptance {
            accepted: false,
            advisories: vec!["sub_channels_owned_by_bass_management".to_string()],
        };
    }

    let curves = match crate::read::load_source_individual(source) {
        Ok(curves) => curves,
        Err(err) => {
            return AllChannelMultiSeatAcceptance {
                accepted: false,
                advisories: vec![format!("measurement_load_failed: {err}")],
            };
        }
    };
    if curves.len() < 2 {
        return AllChannelMultiSeatAcceptance {
            accepted: false,
            advisories: vec!["single_seat_only".to_string()],
        };
    }
    if !curves_share_frequency_grid(&curves) {
        return AllChannelMultiSeatAcceptance {
            accepted: false,
            advisories: vec!["frequency_grid_mismatch_all_channel_skipped".to_string()],
        };
    }
    if policy.primary_seat >= curves.len() {
        return AllChannelMultiSeatAcceptance {
            accepted: false,
            advisories: vec!["primary_seat_out_of_range".to_string()],
        };
    }

    let (weights, weight_advisories) = resolve_all_channel_seat_weights(&policy, curves.len());
    advisories.extend(weight_advisories);
    let band_hz = role.default_target_band_hz();
    let mut weighted_before_rms = 0.0;
    let mut weighted_after_rms = 0.0;
    let mut weighted_level_shift_abs = 0.0;
    let mut prediction_count = 0usize;
    let mut primary_pass = false;
    let mut non_primary_pass = true;

    for (idx, seat_curve) in curves.iter().enumerate() {
        let Some((before_rms, _, _, before_mean)) = band_metrics(seat_curve, band_hz) else {
            advisories.push("seat_band_metrics_unavailable".to_string());
            return AllChannelMultiSeatAcceptance {
                accepted: false,
                advisories,
            };
        };
        let Some(after) = predicted_seat_report(
            idx,
            seat_curve,
            &super::optimize::ChannelOptimizationResult {
                name: channel_name.to_string(),
                pre_score: 0.0,
                post_score: 0.0,
                initial_curve: initial_curve.clone(),
                final_curve: final_curve.clone(),
                biquads: Vec::new(),
                fir_coeffs: None,
            },
            band_hz,
            policy.primary_seat,
            weights[idx],
            policy.max_deviation_db,
        ) else {
            advisories.push("seat_prediction_failed".to_string());
            return AllChannelMultiSeatAcceptance {
                accepted: false,
                advisories,
            };
        };
        let predicted_curve = apply_result_delta_to_seat(seat_curve, initial_curve, final_curve);
        let Some((_, _, _, after_mean)) = band_metrics(&predicted_curve, band_hz) else {
            advisories.push("seat_prediction_failed".to_string());
            return AllChannelMultiSeatAcceptance {
                accepted: false,
                advisories,
            };
        };
        weighted_before_rms += weights[idx] * before_rms;
        weighted_after_rms += weights[idx] * after.rms_target_error_db;
        weighted_level_shift_abs += weights[idx] * (after_mean - before_mean).abs();
        if idx == policy.primary_seat {
            primary_pass = after.pass;
        } else if !after.pass {
            non_primary_pass = false;
        }
        prediction_count += 1;
    }

    if prediction_count != curves.len() {
        advisories.push("seat_prediction_count_mismatch".to_string());
    }
    if !primary_pass {
        advisories.push("primary_seat_constraint_failed".to_string());
    }
    if !non_primary_pass {
        advisories.push("non_primary_seat_constraint_failed".to_string());
    }
    if weighted_after_rms > weighted_before_rms + TARGET_FIT_COLLAPSE_TOLERANCE_DB {
        advisories.push("weighted_target_fit_collapsed".to_string());
    }
    if weighted_level_shift_abs > BROADBAND_LEVEL_SHIFT_TOLERANCE_DB {
        advisories.push("weighted_target_level_collapsed".to_string());
    }

    let accepted = advisories.is_empty();
    if accepted {
        advisories.push("accepted".to_string());
    }
    AllChannelMultiSeatAcceptance {
        accepted,
        advisories,
    }
}

pub fn multi_seat_correction_report(
    config: &RoomConfig,
    channel_results: &HashMap<String, super::optimize::ChannelOptimizationResult>,
    rejected_channels: Option<&HashMap<String, Vec<String>>>,
) -> MultiSeatCorrectionReport {
    let policy = all_channel_multiseat_policy(config);
    let enabled = all_channel_multiseat_enabled(config);
    let mut channels = Vec::new();
    let mut max_seat_count = 0usize;
    let mut report_weights = Vec::new();
    let mut advisories = Vec::new();

    for (channel, speaker) in logical_speaker_configs(config) {
        let role = role_for_channel(&channel);
        if role.is_sub_or_lfe() {
            continue;
        }
        let role_group = role.group();
        let target_band_hz = role.default_target_band_hz();
        let source = single_measurement_source(&speaker);
        let seat_count = source.and_then(measurement_source_count).unwrap_or(0);
        max_seat_count = max_seat_count.max(seat_count);
        let (weights, weight_advisories) = resolve_all_channel_seat_weights(&policy, seat_count);
        if report_weights.is_empty() && !weights.is_empty() {
            report_weights = weights.clone();
        }

        let mut channel_advisories = weight_advisories;
        let status: String;
        let mut seats = Vec::new();
        let mut spatial_variance_peak_db = None;
        let mut min_correction_depth = None;

        if !enabled {
            status = "disabled".to_string();
        } else if source.is_none() {
            status = "unsupported_speaker_topology".to_string();
        } else if seat_count < 2 {
            status = "single_seat_only".to_string();
        } else if !channel_advisories.is_empty() {
            status = "invalid_policy_skipped".to_string();
        } else if policy.primary_seat >= seat_count {
            status = "invalid_policy_skipped".to_string();
            channel_advisories.push("primary_seat_out_of_range".to_string());
        } else if let Some(rejection_advisories) =
            rejected_channels.and_then(|rejections| rejections.get(&channel))
        {
            status = "rejected_guardrails".to_string();
            channel_advisories.extend(rejection_advisories.clone());
        } else if let Some(result) = channel_results.get(&channel) {
            match crate::read::load_source_individual(source.unwrap()) {
                Ok(curves) if curves.len() == seat_count => {
                    let same_grid = curves_share_frequency_grid(&curves);
                    if !same_grid {
                        status = "frequency_grid_mismatch_skipped".to_string();
                        channel_advisories
                            .push("frequency_grid_mismatch_all_channel_skipped".to_string());
                    } else {
                        let sr_config = spatial_robustness_config_from(
                            &default_all_channel_spatial_robustness(),
                        );
                        match super::spatial_robustness::analyze_spatial_robustness_weighted(
                            &curves,
                            &sr_config,
                            Some(&weights),
                        ) {
                            Ok(analysis) => {
                                spatial_variance_peak_db =
                                    analysis.spatial_variance.iter().cloned().reduce(f64::max);
                                min_correction_depth =
                                    analysis.correction_depth.iter().cloned().reduce(f64::min);

                                seats = curves
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(idx, seat_curve)| {
                                        predicted_seat_report(
                                            idx,
                                            seat_curve,
                                            result,
                                            target_band_hz,
                                            policy.primary_seat,
                                            *weights.get(idx).unwrap_or(&0.0),
                                            policy.max_deviation_db,
                                        )
                                    })
                                    .collect();
                                if seats.len() == seat_count {
                                    let primary_pass = seats
                                        .iter()
                                        .find(|seat| seat.is_primary)
                                        .is_some_and(|seat| seat.pass);
                                    let non_primary_pass = seats
                                        .iter()
                                        .filter(|seat| !seat.is_primary)
                                        .all(|seat| seat.pass);
                                    if primary_pass && non_primary_pass {
                                        status = "applied".to_string();
                                    } else {
                                        status = "failed_constraints".to_string();
                                        if !primary_pass {
                                            channel_advisories
                                                .push("primary_seat_constraint_failed".to_string());
                                        }
                                        if !non_primary_pass {
                                            channel_advisories.push(
                                                "non_primary_seat_constraint_failed".to_string(),
                                            );
                                        }
                                    }
                                } else {
                                    status = "prediction_failed".to_string();
                                }
                            }
                            Err(e) => {
                                status = "spatial_robustness_invalid_skipped".to_string();
                                channel_advisories
                                    .push(format!("spatial_robustness_invalid_skipped: {e}"));
                            }
                        }
                    }
                    if seats.iter().any(|seat| seat.null_risk) {
                        channel_advisories.push("seat_specific_null_not_corrected".to_string());
                    }
                }
                Ok(_) => {
                    status = "seat_count_mismatch".to_string();
                }
                Err(err) => {
                    status = "measurement_load_failed".to_string();
                    channel_advisories.push(err.to_string());
                }
            }
        } else {
            status = "not_optimized".to_string();
        }

        let rms_target_error_db = optional_max(seats.iter().map(|seat| seat.rms_target_error_db));
        let max_abs_deviation_db = optional_max(seats.iter().map(|seat| seat.max_abs_deviation_db));
        let primary_pass = seats
            .iter()
            .find(|seat| seat.is_primary)
            .map(|seat| seat.pass);
        let non_primary: Vec<_> = seats.iter().filter(|seat| !seat.is_primary).collect();
        let non_primary_pass = (!non_primary.is_empty()).then(|| {
            non_primary
                .iter()
                .all(|seat| seat.max_abs_deviation_db <= policy.max_deviation_db)
        });

        channels.push(MultiSeatChannelCorrectionReport {
            channel,
            role,
            role_group,
            status,
            seat_count,
            target_band_hz,
            rms_target_error_db,
            max_abs_deviation_db,
            primary_pass,
            non_primary_pass,
            spatial_variance_peak_db,
            min_correction_depth,
            seats,
            advisories: channel_advisories,
        });
    }

    let applied = channels.iter().any(|channel| channel.status == "applied");
    if !enabled {
        advisories.push("all_channel_multiseat_disabled".to_string());
    }
    if channels.is_empty() {
        advisories.push("no_non_sub_channels".to_string());
    }
    if !applied && enabled {
        advisories.push("no_all_channel_multiseat_correction_applied".to_string());
    }
    if channels.iter().any(|channel| {
        channel
            .advisories
            .iter()
            .any(|a| a == "seat_specific_null_not_corrected")
    }) {
        advisories.push("seat_specific_nulls_were_not_overcorrected".to_string());
    }
    if channels
        .iter()
        .any(|channel| channel.status == "rejected_guardrails")
    {
        advisories.push("all_channel_corrections_rejected_by_guardrails".to_string());
    }
    if advisories.is_empty() {
        advisories.push("ok".to_string());
    }

    MultiSeatCorrectionReport {
        enabled,
        applied,
        strategy: multi_measurement_strategy_name(
            config
                .optimizer
                .multi_measurement
                .as_ref()
                .map(|mc| &mc.strategy)
                .unwrap_or(&policy.all_channel_strategy),
        )
        .to_string(),
        seat_count: max_seat_count,
        primary_seat: policy.primary_seat,
        seat_weights: report_weights,
        role_groups: multi_seat_role_group_reports(&channels),
        channels,
        advisories,
    }
}

fn bass_management_signal_flow(
    config: &RoomConfig,
    effective: &EffectiveBassManagement,
    physical_sub_output: &str,
    optimization: Option<&BassManagementOptimizationReport>,
) -> Vec<BassManagementSignalFlowEntry> {
    logical_channel_names(config)
        .into_iter()
        .map(|source_channel| {
            let role = role_for_channel(&source_channel);
            let is_lfe =
                role == HomeCinemaRole::Lfe || source_channel == effective.config.lfe_channel;
            let redirects_bass = effective.config.redirect_bass && role.is_bass_managed_candidate();
            let crossover =
                resolved_group_crossover(config, group_id_for_role(role), effective, optimization);
            BassManagementSignalFlowEntry {
                source_channel,
                role,
                destination: if is_lfe || redirects_bass {
                    physical_sub_output.to_string()
                } else {
                    "self".to_string()
                },
                high_pass_hz: role
                    .is_bass_managed_candidate()
                    .then_some(crossover.frequency_hz)
                    .flatten(),
                low_pass_hz: (is_lfe || redirects_bass)
                    .then_some(crossover.frequency_hz)
                    .flatten(),
                lfe_gain_db: if is_lfe {
                    effective.config.lfe_playback_gain_db
                } else {
                    0.0
                },
                redirects_bass,
            }
        })
        .collect()
}

fn bass_management_signal_flow_advisories(
    effective: &EffectiveBassManagement,
    redirected_bass_channel_count: usize,
) -> Vec<String> {
    let mut advisories = Vec::new();
    if effective.crossover_frequency_hz.is_none() {
        advisories.push("missing_crossover_frequency".to_string());
    }
    if effective.config.redirect_bass && redirected_bass_channel_count == 0 {
        advisories.push("redirect_bass_enabled_but_no_eligible_mains".to_string());
    }
    if !effective.config.redirect_bass && effective.crossover_frequency_hz.is_some() {
        advisories.push("main_highpass_without_redirected_bass".to_string());
    }
    if effective.config.lfe_playback_gain_db > effective.config.headroom_margin_db {
        advisories.push("lfe_gain_exceeds_headroom_margin".to_string());
    }
    if advisories.is_empty() {
        advisories.push("ok".to_string());
    }
    advisories
}

fn multi_seat_recommended_scope(
    channels_with_multiple_measurements: usize,
    non_sub_channel_count: usize,
    non_sub_channels_with_multiple_measurements: usize,
) -> &'static str {
    if non_sub_channel_count > 0
        && non_sub_channels_with_multiple_measurements == non_sub_channel_count
    {
        "all_channel_reporting_ready"
    } else if non_sub_channels_with_multiple_measurements > 0 {
        "partial_non_sub_reporting_only"
    } else if channels_with_multiple_measurements > 0 {
        "sub_or_partial_only"
    } else {
        "single_seat_only"
    }
}

fn multi_seat_coverage_advisories(
    channels_with_multiple_measurements: usize,
    non_sub_channel_count: usize,
    non_sub_channels_with_multiple_measurements: usize,
    max_seat_count: usize,
) -> Vec<String> {
    let mut advisories = Vec::new();
    if channels_with_multiple_measurements == 0 {
        advisories.push("no_multi_seat_measurements".to_string());
    }
    if max_seat_count < 2 {
        advisories.push("insufficient_seats".to_string());
    }
    if non_sub_channels_with_multiple_measurements == 0 && channels_with_multiple_measurements > 0 {
        advisories.push("multi_seat_sub_only".to_string());
    }
    if non_sub_channel_count > 1 && non_sub_channels_with_multiple_measurements == 1 {
        advisories.push("only_one_non_sub_channel_has_multi_seat_data".to_string());
    }
    if non_sub_channel_count > 0
        && non_sub_channels_with_multiple_measurements > 0
        && non_sub_channels_with_multiple_measurements < non_sub_channel_count
    {
        advisories.push("partial_non_sub_multi_seat_coverage".to_string());
    }
    if advisories.is_empty() {
        advisories.push("all_channel_multi_seat_reporting_ready".to_string());
    }
    advisories
}

fn all_channel_multiseat_enabled(config: &RoomConfig) -> bool {
    let is_home_cinema = config
        .system
        .as_ref()
        .is_some_and(|system| system.model == SystemModel::HomeCinema);
    is_home_cinema && all_channel_multiseat_policy(config).all_channel_enabled
}

fn all_channel_multiseat_policy(config: &RoomConfig) -> MultiSeatConfig {
    config.optimizer.multi_seat.clone().unwrap_or_default()
}

fn resolve_all_channel_seat_weights(
    policy: &MultiSeatConfig,
    seat_count: usize,
) -> (Vec<f64>, Vec<String>) {
    if seat_count == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut advisories = Vec::new();
    let mut weights = match &policy.seat_weights {
        Some(raw) if raw.len() == seat_count => raw.clone(),
        Some(_) => {
            advisories.push("seat_weights_length_mismatch_equal_weights_used".to_string());
            vec![1.0; seat_count]
        }
        None => vec![1.0; seat_count],
    };
    for weight in &mut weights {
        if !weight.is_finite() || *weight < 0.0 {
            *weight = 0.0;
        }
    }
    if policy.strategy == super::types::MultiSeatStrategy::PrimaryWithConstraints
        && policy.primary_seat < weights.len()
    {
        weights[policy.primary_seat] *= policy.primary_seat_weight.max(1.0);
    }
    let sum: f64 = weights.iter().sum();
    if sum <= f64::EPSILON {
        advisories.push("seat_weights_zero_equal_weights_used".to_string());
        weights = vec![1.0 / seat_count as f64; seat_count];
    } else {
        for weight in &mut weights {
            *weight /= sum;
        }
    }
    (weights, advisories)
}

fn default_all_channel_spatial_robustness() -> SpatialRobustnessSerdeConfig {
    SpatialRobustnessSerdeConfig {
        variance_threshold_db: 3.0,
        transition_width_db: 2.0,
        min_correction_depth: 0.1,
        mask_smoothing_octaves: 1.0 / 6.0,
    }
}

fn spatial_robustness_config_from(
    config: &SpatialRobustnessSerdeConfig,
) -> super::spatial_robustness::SpatialRobustnessConfig {
    super::spatial_robustness::SpatialRobustnessConfig {
        variance_threshold_db: config.variance_threshold_db,
        transition_width_db: config.transition_width_db,
        min_correction_depth: config.min_correction_depth,
        mask_smoothing_octaves: config.mask_smoothing_octaves,
    }
}

fn single_measurement_source(speaker: &SpeakerConfig) -> Option<&MeasurementSource> {
    match speaker {
        SpeakerConfig::Single(source) => Some(source),
        _ => None,
    }
}

fn curves_share_frequency_grid(curves: &[Curve]) -> bool {
    let Some(first) = curves.first() else {
        return true;
    };
    curves.iter().all(|curve| {
        curve.freq.len() == first.freq.len()
            && curve
                .freq
                .iter()
                .zip(first.freq.iter())
                .all(|(a, b)| (a - b).abs() <= (1e-6 * b.abs().max(1.0)))
    })
}

fn predicted_seat_report(
    seat_index: usize,
    seat_curve: &Curve,
    result: &super::optimize::ChannelOptimizationResult,
    band_hz: (f64, f64),
    primary_seat: usize,
    weight: f64,
    max_deviation_db: f64,
) -> Option<MultiSeatPredictionReport> {
    let predicted =
        apply_result_delta_to_seat(seat_curve, &result.initial_curve, &result.final_curve);
    let (rms, max_abs, min_dev, _) = band_metrics(&predicted, band_hz)?;
    Some(MultiSeatPredictionReport {
        seat_index,
        weight,
        is_primary: seat_index == primary_seat,
        rms_target_error_db: rms,
        max_abs_deviation_db: max_abs,
        pass: max_abs <= max_deviation_db,
        null_risk: min_dev < -max_deviation_db,
    })
}

fn apply_result_delta_to_seat(seat_curve: &Curve, initial: &Curve, final_curve: &Curve) -> Curve {
    let initial_on_seat = crate::read::interpolate_log_space(&seat_curve.freq, initial);
    let final_on_seat = crate::read::interpolate_log_space(&seat_curve.freq, final_curve);
    Curve {
        freq: seat_curve.freq.clone(),
        spl: &seat_curve.spl + &(&final_on_seat.spl - &initial_on_seat.spl),
        phase: seat_curve.phase.clone(),
        ..Default::default()
    }
}

fn band_metrics(curve: &Curve, band_hz: (f64, f64)) -> Option<(f64, f64, f64, f64)> {
    let values: Vec<f64> = curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter_map(|(freq, spl)| {
            (*freq >= band_hz.0 && *freq <= band_hz.1 && spl.is_finite()).then_some(*spl)
        })
        .collect();
    if values.is_empty() {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let deviations: Vec<f64> = values.iter().map(|v| v - mean).collect();
    let rms = (deviations.iter().map(|v| v * v).sum::<f64>() / deviations.len() as f64).sqrt();
    let max_abs = deviations.iter().map(|v| v.abs()).fold(0.0, f64::max);
    let min_dev = deviations.iter().cloned().fold(f64::INFINITY, f64::min);
    Some((rms, max_abs, min_dev, mean))
}

fn optional_max(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.reduce(f64::max)
}

fn multi_seat_role_group_reports(
    channels: &[MultiSeatChannelCorrectionReport],
) -> Vec<MultiSeatRoleGroupCorrectionReport> {
    let mut groups: BTreeMap<HomeCinemaRoleGroup, Vec<&MultiSeatChannelCorrectionReport>> =
        BTreeMap::new();
    for channel in channels {
        groups.entry(channel.role_group).or_default().push(channel);
    }
    groups
        .into_iter()
        .map(|(role_group, channels)| {
            let applied: Vec<_> = channels
                .iter()
                .copied()
                .filter(|channel| channel.status == "applied")
                .collect();
            let pass = !applied.is_empty()
                && applied.iter().all(|channel| {
                    channel.primary_pass.unwrap_or(false)
                        && channel.non_primary_pass.unwrap_or(true)
                });
            let mut advisories = Vec::new();
            if applied.is_empty() {
                advisories.push("no_applied_channels".to_string());
            }
            if !pass && !applied.is_empty() {
                advisories.push("seat_constraint_failed".to_string());
            }
            if advisories.is_empty() {
                advisories.push("ok".to_string());
            }
            MultiSeatRoleGroupCorrectionReport {
                role_group,
                channel_count: channels.len(),
                applied_channel_count: applied.len(),
                pass,
                worst_rms_target_error_db: optional_max(
                    applied.iter().filter_map(|c| c.rms_target_error_db),
                ),
                worst_max_abs_deviation_db: optional_max(
                    applied.iter().filter_map(|c| c.max_abs_deviation_db),
                ),
                advisories,
            }
        })
        .collect()
}

fn multi_measurement_strategy_name(strategy: &MultiMeasurementStrategy) -> &'static str {
    match strategy {
        MultiMeasurementStrategy::Average => "average",
        MultiMeasurementStrategy::WeightedSum => "weighted_sum",
        MultiMeasurementStrategy::Minimax => "minimax",
        MultiMeasurementStrategy::VariancePenalized => "variance_penalized",
        MultiMeasurementStrategy::SpatialRobustness => "spatial_robustness",
        MultiMeasurementStrategy::MinimaxUncertainty => "minimax_uncertainty",
    }
}

pub fn role_for_channel(channel_name: &str) -> HomeCinemaRole {
    let normalized = normalize_channel_name(channel_name);
    match normalized.as_str() {
        "l" | "fl" | "left" | "frontleft" => HomeCinemaRole::FrontLeft,
        "r" | "fr" | "right" | "frontright" => HomeCinemaRole::FrontRight,
        "c" | "center" | "centre" => HomeCinemaRole::Center,
        "lfe" | "lf" => HomeCinemaRole::Lfe,
        "sub" | "subwoofer" | "sw" | "sub1" | "sub2" => HomeCinemaRole::Subwoofer,
        "sl" | "ls" | "surroundleft" | "sideleft" => HomeCinemaRole::SideSurroundLeft,
        "sr" | "rs" | "surroundright" | "sideright" => HomeCinemaRole::SideSurroundRight,
        "bl" | "rl" | "sbl" | "rearleft" | "backleft" | "surroundbackleft" => {
            HomeCinemaRole::RearSurroundLeft
        }
        "br" | "rr" | "sbr" | "rearright" | "backright" | "surroundbackright" => {
            HomeCinemaRole::RearSurroundRight
        }
        "wl" | "wideleft" | "frontwideleft" => HomeCinemaRole::WideLeft,
        "wr" | "wideright" | "frontwideright" => HomeCinemaRole::WideRight,
        "tfl" | "fhl" | "topfrontleft" | "frontheightleft" => HomeCinemaRole::TopFrontLeft,
        "tfr" | "fhr" | "topfrontright" | "frontheightright" => HomeCinemaRole::TopFrontRight,
        "tml" | "topmiddleleft" => HomeCinemaRole::TopMiddleLeft,
        "tmr" | "topmiddleright" => HomeCinemaRole::TopMiddleRight,
        "tbl" | "trl" | "rhl" | "topbackleft" | "toprearleft" | "rearheightleft" => {
            HomeCinemaRole::TopRearLeft
        }
        "tbr" | "trr" | "rhr" | "topbackright" | "toprearright" | "rearheightright" => {
            HomeCinemaRole::TopRearRight
        }
        _ if normalized.contains("sub") => HomeCinemaRole::Subwoofer,
        _ if normalized.contains("lfe") => HomeCinemaRole::Lfe,
        _ => HomeCinemaRole::Unknown,
    }
}

pub fn matching_group_key(channel_name: &str) -> Option<&'static str> {
    matching_group_key_for_role(role_for_channel(channel_name))
}

pub fn role_score_band(config: &RoomConfig, channel_name: &str) -> (f64, f64) {
    let role = role_for_channel(channel_name);
    let (role_min, role_max) = role.default_target_band_hz();
    let min = config.optimizer.min_freq.max(role_min);
    let max = config.optimizer.max_freq.min(role_max).max(min);
    (min, max)
}

pub fn role_adjusted_target_response(
    channel_name: &str,
    base: &TargetResponseConfig,
) -> TargetResponseConfig {
    let Some(role_targets) = base.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return base.clone();
    };

    let mut adjusted = base.clone();
    apply_role_target_adjustment(role_for_channel(channel_name), role_targets, &mut adjusted);
    adjusted
}

pub fn apply_role_target_curve_shape(
    channel_name: &str,
    target_curve: &mut Curve,
    target: &TargetResponseConfig,
) {
    let Some(role_targets) = target.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return;
    };
    let role = role_for_channel(channel_name);

    if role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001 {
        apply_log_band_emphasis(
            target_curve,
            role_targets.center_dialog_low_hz,
            role_targets.center_dialog_high_hz,
            role_targets.center_dialog_boost_db,
        );
    }

    if role_targets.cinema_x_curve_enabled
        && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001
    {
        apply_high_frequency_slope(
            target_curve,
            role_targets.cinema_x_curve_start_hz,
            role_targets.cinema_x_curve_db_per_octave,
        );
    }

    if let Some(distance_m) = role_targets.listening_distance_m {
        let ref_m = role_targets.cinema_reference_distance_m;
        if distance_m > ref_m
            && ref_m > 0.0
            && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001
        {
            let distance_doublings = (distance_m / ref_m).log2();
            apply_high_frequency_slope(
                target_curve,
                role_targets.cinema_x_curve_start_hz,
                -role_targets.distance_treble_rolloff_db_per_doubling.abs() * distance_doublings,
            );
        }
    }
}

pub fn role_target_curve_shape_active(channel_name: &str, target: &TargetResponseConfig) -> bool {
    let Some(role_targets) = target.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return false;
    };
    let role = role_for_channel(channel_name);
    (role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001)
        || (role_targets.cinema_x_curve_enabled
            && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001)
        || (role_targets.listening_distance_m.is_some()
            && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001)
}

impl HomeCinemaRole {
    pub fn group(self) -> HomeCinemaRoleGroup {
        match self {
            HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => HomeCinemaRoleGroup::FrontLr,
            HomeCinemaRole::Center => HomeCinemaRoleGroup::Center,
            HomeCinemaRole::Lfe => HomeCinemaRoleGroup::Lfe,
            HomeCinemaRole::Subwoofer => HomeCinemaRoleGroup::Subwoofer,
            HomeCinemaRole::SideSurroundLeft | HomeCinemaRole::SideSurroundRight => {
                HomeCinemaRoleGroup::SideSurrounds
            }
            HomeCinemaRole::RearSurroundLeft | HomeCinemaRole::RearSurroundRight => {
                HomeCinemaRoleGroup::RearSurrounds
            }
            HomeCinemaRole::WideLeft | HomeCinemaRole::WideRight => HomeCinemaRoleGroup::Wides,
            HomeCinemaRole::TopFrontLeft | HomeCinemaRole::TopFrontRight => {
                HomeCinemaRoleGroup::TopFront
            }
            HomeCinemaRole::TopMiddleLeft | HomeCinemaRole::TopMiddleRight => {
                HomeCinemaRoleGroup::TopMiddle
            }
            HomeCinemaRole::TopRearLeft | HomeCinemaRole::TopRearRight => {
                HomeCinemaRoleGroup::TopRear
            }
            HomeCinemaRole::Unknown => HomeCinemaRoleGroup::Unknown,
        }
    }

    pub fn is_height(self) -> bool {
        matches!(
            self,
            HomeCinemaRole::TopFrontLeft
                | HomeCinemaRole::TopFrontRight
                | HomeCinemaRole::TopMiddleLeft
                | HomeCinemaRole::TopMiddleRight
                | HomeCinemaRole::TopRearLeft
                | HomeCinemaRole::TopRearRight
        )
    }

    pub fn is_sub_or_lfe(self) -> bool {
        matches!(self, HomeCinemaRole::Subwoofer | HomeCinemaRole::Lfe)
    }

    pub fn is_bed_channel(self) -> bool {
        !self.is_height() && !self.is_sub_or_lfe() && self != HomeCinemaRole::Unknown
    }

    pub fn is_bass_managed_candidate(self) -> bool {
        self.is_bed_channel() || self.is_height()
    }

    pub fn default_target_band_hz(self) -> (f64, f64) {
        match self {
            HomeCinemaRole::Lfe | HomeCinemaRole::Subwoofer => (20.0, 160.0),
            HomeCinemaRole::Center => (80.0, 16_000.0),
            HomeCinemaRole::SideSurroundLeft
            | HomeCinemaRole::SideSurroundRight
            | HomeCinemaRole::RearSurroundLeft
            | HomeCinemaRole::RearSurroundRight
            | HomeCinemaRole::WideLeft
            | HomeCinemaRole::WideRight => (80.0, 12_000.0),
            role if role.is_height() => (120.0, 10_000.0),
            HomeCinemaRole::Unknown => (20.0, 20_000.0),
            _ => (40.0, 18_000.0),
        }
    }
}

fn apply_role_target_adjustment(
    role: HomeCinemaRole,
    role_targets: &RoleTargetConfig,
    target: &mut TargetResponseConfig,
) {
    let slope_offset = role_slope_offset(role, role_targets);
    if slope_offset.abs() > 0.001 {
        add_slope_offset(target, slope_offset);
    }

    match role {
        HomeCinemaRole::Center => {
            target.preference.treble_shelf_db += role_targets.center_treble_shelf_db;
        }
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => {
            target.preference.treble_shelf_db += role_targets.surround_treble_shelf_db;
        }
        role if role.is_height() => {
            target.preference.treble_shelf_db += role_targets.height_treble_shelf_db;
        }
        HomeCinemaRole::Lfe => {
            target.preference.bass_shelf_db += role_targets.lfe_bass_shelf_db;
        }
        HomeCinemaRole::Subwoofer => {
            target.preference.bass_shelf_db += role_targets.subwoofer_bass_shelf_db;
        }
        _ => {}
    }

    if target.preference.treble_shelf_freq <= 0.0 {
        target.preference.treble_shelf_freq = UserPreference::default().treble_shelf_freq;
    }
    if target.preference.bass_shelf_freq <= 0.0 {
        target.preference.bass_shelf_freq = UserPreference::default().bass_shelf_freq;
    }
}

fn role_slope_offset(role: HomeCinemaRole, role_targets: &RoleTargetConfig) -> f64 {
    match role {
        HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => {
            role_targets.front_slope_offset_db_per_octave
        }
        HomeCinemaRole::Center => role_targets.center_slope_offset_db_per_octave,
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => role_targets.surround_slope_offset_db_per_octave,
        HomeCinemaRole::TopFrontLeft
        | HomeCinemaRole::TopFrontRight
        | HomeCinemaRole::TopMiddleLeft
        | HomeCinemaRole::TopMiddleRight
        | HomeCinemaRole::TopRearLeft
        | HomeCinemaRole::TopRearRight => role_targets.height_slope_offset_db_per_octave,
        HomeCinemaRole::Subwoofer => role_targets.subwoofer_slope_offset_db_per_octave,
        HomeCinemaRole::Lfe => role_targets.lfe_slope_offset_db_per_octave,
        HomeCinemaRole::Unknown => 0.0,
    }
}

fn add_slope_offset(target: &mut TargetResponseConfig, slope_offset_db_per_octave: f64) {
    let base_slope = match target.shape {
        TargetShape::Flat => 0.0,
        TargetShape::Harman => -0.8,
        TargetShape::Custom => target.slope_db_per_octave,
        TargetShape::File | TargetShape::FromMeasurement => target.slope_db_per_octave,
    };
    target.shape = TargetShape::Custom;
    target.slope_db_per_octave = base_slope + slope_offset_db_per_octave;
}

fn apply_log_band_emphasis(target_curve: &mut Curve, low_hz: f64, high_hz: f64, gain_db: f64) {
    if !(low_hz > 0.0 && high_hz > low_hz) {
        return;
    }
    let center_hz = (low_hz * high_hz).sqrt();
    let half_width_oct = (high_hz / low_hz).log2() / 2.0;
    if half_width_oct <= 0.0 {
        return;
    }

    for (freq, spl) in target_curve.freq.iter().zip(target_curve.spl.iter_mut()) {
        let distance_oct = (*freq / center_hz).max(1e-9).log2().abs();
        if distance_oct <= half_width_oct {
            let weight = 0.5 * (1.0 + (std::f64::consts::PI * distance_oct / half_width_oct).cos());
            *spl += gain_db * weight;
        }
    }
}

fn apply_high_frequency_slope(target_curve: &mut Curve, start_hz: f64, slope_db_per_octave: f64) {
    if start_hz <= 0.0 {
        return;
    }
    for (freq, spl) in target_curve.freq.iter().zip(target_curve.spl.iter_mut()) {
        if *freq > start_hz {
            *spl += slope_db_per_octave * (*freq / start_hz).log2();
        }
    }
}

fn target_profile_for_role(config: &RoomConfig, role: HomeCinemaRole) -> String {
    let enabled = config
        .optimizer
        .target_response
        .as_ref()
        .and_then(|target| target.role_targets.as_ref())
        .is_some_and(|role_targets| role_targets.enabled);
    let suffix = if enabled { "_role_target" } else { "_default" };
    format!("{}{}", role_profile_base(role), suffix)
}

fn target_advisory_for_role(config: &RoomConfig, role: HomeCinemaRole) -> Option<String> {
    let role_targets = config
        .optimizer
        .target_response
        .as_ref()
        .and_then(|target| target.role_targets.as_ref())
        .filter(|role_targets| role_targets.enabled)?;
    let mut advisories = Vec::new();
    if role_slope_offset(role, role_targets).abs() > 0.001 {
        advisories.push("role_slope_offset");
    }
    if role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001 {
        advisories.push("center_dialog_band");
    }
    if role_targets.cinema_x_curve_enabled
        && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001
    {
        advisories.push("cinema_x_curve");
    }
    if role_targets.listening_distance_m.is_some()
        && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001
    {
        advisories.push("distance_treble_rolloff");
    }
    if advisories.is_empty() {
        Some("role_targets_enabled_neutral".to_string())
    } else {
        Some(advisories.join(";"))
    }
}

fn role_profile_base(role: HomeCinemaRole) -> &'static str {
    match role {
        HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => "front_lr",
        HomeCinemaRole::Center => "center_dialog",
        HomeCinemaRole::Lfe => "lfe",
        HomeCinemaRole::Subwoofer => "subwoofer",
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => "surround",
        HomeCinemaRole::TopFrontLeft
        | HomeCinemaRole::TopFrontRight
        | HomeCinemaRole::TopMiddleLeft
        | HomeCinemaRole::TopMiddleRight
        | HomeCinemaRole::TopRearLeft
        | HomeCinemaRole::TopRearRight => "height",
        HomeCinemaRole::Unknown => "generic",
    }
}

fn matching_group_key_for_role(role: HomeCinemaRole) -> Option<&'static str> {
    match role.group() {
        HomeCinemaRoleGroup::FrontLr => Some("front_lr"),
        HomeCinemaRoleGroup::SideSurrounds => Some("side_surrounds"),
        HomeCinemaRoleGroup::RearSurrounds => Some("rear_surrounds"),
        HomeCinemaRoleGroup::Wides => Some("wides"),
        HomeCinemaRoleGroup::TopFront => Some("top_front"),
        HomeCinemaRoleGroup::TopMiddle => Some("top_middle"),
        HomeCinemaRoleGroup::TopRear => Some("top_rear"),
        HomeCinemaRoleGroup::Unknown => Some("generic"),
        HomeCinemaRoleGroup::Center | HomeCinemaRoleGroup::Lfe | HomeCinemaRoleGroup::Subwoofer => {
            None
        }
    }
}

fn role_group_key(group: HomeCinemaRoleGroup) -> &'static str {
    match group {
        HomeCinemaRoleGroup::FrontLr => "front_lr",
        HomeCinemaRoleGroup::Center => "center",
        HomeCinemaRoleGroup::Lfe => "lfe",
        HomeCinemaRoleGroup::Subwoofer => "subwoofer",
        HomeCinemaRoleGroup::SideSurrounds => "side_surrounds",
        HomeCinemaRoleGroup::RearSurrounds => "rear_surrounds",
        HomeCinemaRoleGroup::Wides => "wides",
        HomeCinemaRoleGroup::TopFront => "top_front",
        HomeCinemaRoleGroup::TopMiddle => "top_middle",
        HomeCinemaRoleGroup::TopRear => "top_rear",
        HomeCinemaRoleGroup::Unknown => "unknown",
    }
}

fn detect_layout_name(bed_channels: usize, lfe_channels: usize, height_channels: usize) -> String {
    if height_channels > 0 {
        format!("{bed_channels}.{lfe_channels}.{height_channels}")
    } else {
        format!("{bed_channels}.{lfe_channels}")
    }
}

fn resolve_crossover_descriptor(
    config: &RoomConfig,
    crossover_key: Option<&str>,
) -> (String, Option<f64>, String) {
    let Some(key) = crossover_key else {
        return (
            "unknown".to_string(),
            None,
            "missing_subwoofer_crossover".to_string(),
        );
    };
    let Some(crossover) = config.crossovers.as_ref().and_then(|xos| xos.get(key)) else {
        return (
            "unknown".to_string(),
            None,
            format!("missing_crossover_config:{key}"),
        );
    };
    let frequency = crossover.frequency.or_else(|| {
        crossover
            .frequency_range
            .map(|(min, max)| (min * max).sqrt())
    });
    (
        crossover.crossover_type.clone(),
        frequency,
        crossover_advisory(crossover),
    )
}

fn crossover_advisory(crossover: &CrossoverConfig) -> String {
    if crossover.frequency.is_none() && crossover.frequency_range.is_none() {
        "missing_crossover_frequency".to_string()
    } else {
        "ok".to_string()
    }
}

fn logical_channel_names(config: &RoomConfig) -> Vec<String> {
    if let Some(system) = config.system.as_ref() {
        let mut pairs: Vec<_> = system.speakers.keys().cloned().collect();
        pairs.sort();
        pairs
    } else if let Some(recording) = config.recording_config.as_ref()
        && let Some(names) = recording.channel_names.as_ref()
        && !names.is_empty()
    {
        names.clone()
    } else {
        let mut names: Vec<_> = config.speakers.keys().cloned().collect();
        names.sort();
        names
    }
}

fn logical_speaker_configs(config: &RoomConfig) -> HashMap<String, SpeakerConfig> {
    if let Some(system) = config.system.as_ref() {
        system
            .speakers
            .iter()
            .filter_map(|(role, key)| {
                config
                    .speakers
                    .get(key)
                    .cloned()
                    .map(|speaker| (role.clone(), speaker))
            })
            .collect()
    } else {
        config.speakers.clone()
    }
}

fn speaker_measurement_count(speaker: &SpeakerConfig) -> Option<usize> {
    match speaker {
        SpeakerConfig::Single(source) => measurement_source_count(source),
        SpeakerConfig::Group(group) => group
            .measurements
            .iter()
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::MultiSub(group) => group
            .subwoofers
            .iter()
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::Dba(config) => config
            .front
            .iter()
            .chain(config.rear.iter())
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::Cardioid(config) => [measurement_source_count(&config.front)]
            .into_iter()
            .chain([measurement_source_count(&config.rear)])
            .flatten()
            .max(),
    }
}

fn measurement_source_count(source: &MeasurementSource) -> Option<usize> {
    match source {
        MeasurementSource::Single(_) | MeasurementSource::InMemory(_) => Some(1),
        MeasurementSource::Multiple(m) => Some(m.measurements.len()),
        MeasurementSource::InMemoryMultiple(curves) => Some(curves.len()),
    }
}

fn normalize_channel_name(channel_name: &str) -> String {
    channel_name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_' | '.'))
        .collect()
}

#[cfg(test)]
mod tests;
