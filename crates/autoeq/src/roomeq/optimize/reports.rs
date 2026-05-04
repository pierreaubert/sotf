use super::*;

pub(super) fn recompute_curve_flatness_score(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
    let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();
    let mean = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;
    let normalized_spl = &curve.spl - mean;
    crate::loss::flat_loss(&curve.freq, &normalized_spl, min_freq, max_freq)
}

pub(super) fn should_apply_spectral_shelves(
    current_curves: &HashMap<String, Curve>,
    channel_name: &str,
    shelf_filters: &[Biquad],
    sample_rate: f64,
    score_min: f64,
    score_max: f64,
) -> bool {
    if shelf_filters.is_empty() {
        return false;
    }

    let Some(curve) = current_curves.get(channel_name) else {
        return false;
    };

    let response =
        crate::response::compute_peq_complex_response(shelf_filters, &curve.freq, sample_rate);
    let corrected = crate::response::apply_complex_response(curve, &response);
    let flatness_before = recompute_curve_flatness_score(curve, score_min, score_max);
    let flatness_after = recompute_curve_flatness_score(&corrected, score_min, score_max);
    let flatness_regression = (flatness_after - flatness_before).max(0.0);

    let icd_before =
        crate::roomeq::spectral_align::compute_inter_channel_deviation(current_curves, score_min);
    if icd_before.deviation_per_freq.is_empty() {
        return false;
    }

    let mut corrected_curves = current_curves.clone();
    corrected_curves.insert(channel_name.to_string(), corrected);
    let icd_after = crate::roomeq::spectral_align::compute_inter_channel_deviation(
        &corrected_curves,
        score_min,
    );
    if icd_after.deviation_per_freq.is_empty() {
        return false;
    }

    let icd_improvement = icd_before.passband_rms_db - icd_after.passband_rms_db;
    icd_improvement > flatness_regression + 1e-6
}

pub(super) fn final_score_band_for_channel(config: &RoomConfig, channel_name: &str) -> (f64, f64) {
    let min_freq = config.optimizer.min_freq;
    let mut max_freq = config.optimizer.max_freq;
    if config.system.is_none() {
        return (min_freq, max_freq.max(min_freq));
    }
    let crossover_max = config.crossovers.as_ref().and_then(|xos| {
        xos.values()
            .filter_map(|xo| xo.frequency)
            .filter(|freq| freq.is_finite() && *freq > 0.0)
            .reduce(f64::max)
    });

    if is_subwoofer_channel(config, channel_name) {
        let crossover_max = crossover_max.unwrap_or(160.0);
        max_freq = max_freq.min((crossover_max * 2.0).clamp(120.0, 250.0));
    } else if config
        .system
        .as_ref()
        .is_some_and(|sys| sys.subwoofers.is_some())
    {
        let crossover_max = crossover_max.unwrap_or(80.0);
        return (
            min_freq.max(crossover_max),
            max_freq.max(min_freq.max(crossover_max)),
        );
    } else {
        let (role_min, role_max) =
            crate::roomeq::home_cinema::role_score_band(config, channel_name);
        return (role_min, role_max.max(role_min));
    }

    (min_freq, max_freq.max(min_freq))
}

pub(super) fn refresh_final_reports(
    result: &mut RoomOptimizationResult,
    config: &RoomConfig,
    sample_rate: f64,
) {
    for ch_result in result.channel_results.values_mut() {
        let (score_min_freq, score_max_freq) =
            final_score_band_for_channel(config, &ch_result.name);
        ch_result.post_score =
            recompute_curve_flatness_score(&ch_result.final_curve, score_min_freq, score_max_freq);
        if let Some(chain) = result.channels.get_mut(&ch_result.name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    }

    let count = result.channel_results.len().max(1) as f64;
    let avg_pre = result
        .channel_results
        .values()
        .map(|ch| ch.pre_score)
        .sum::<f64>()
        / count;
    let avg_post = result
        .channel_results
        .values()
        .map(|ch| ch.post_score)
        .sum::<f64>()
        / count;
    result.combined_pre_score = avg_pre;
    result.combined_post_score = avg_post;
    result.metadata.pre_score = avg_pre;
    result.metadata.post_score = avg_post;
    result.metadata.home_cinema_layout = Some(crate::roomeq::home_cinema::analyze_layout(config));
    result.metadata.multi_seat_coverage =
        Some(crate::roomeq::home_cinema::multi_seat_coverage(config));
    let existing_bass_management = result.metadata.bass_management.clone();
    result.metadata.bass_management = if let Some(existing) = existing_bass_management {
        crate::roomeq::home_cinema::bass_management_report_with_optimization_and_sample_rate(
            config,
            existing.applied_sub_gain_db,
            existing.gain_limited,
            existing.optimization,
            sample_rate,
        )
    } else {
        crate::roomeq::home_cinema::bass_management_report(config, None, false)
    };

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    result.metadata.epa_per_channel =
        crate::roomeq::output::compute_epa_per_channel(&result.channels, &epa_cfg);
    update_perceptual_metrics(&mut result.metadata, Some(&result.channels), Some(config));

    let ir_inputs: Vec<_> = result
        .channel_results
        .iter()
        .map(|(name, ch)| {
            let delay_ms = result
                .channels
                .get(name)
                .map(total_chain_delay_ms)
                .unwrap_or(0.0);
            (
                name.clone(),
                ch.initial_curve.clone(),
                ch.biquads.clone(),
                ch.fir_coeffs.clone(),
                delay_ms,
            )
        })
        .collect();

    for (channel_name, initial_curve, biquads, fir_coeffs, delay_ms) in ir_inputs {
        if let Some((pre_ir, post_ir)) = crate::roomeq::ir_waveform::compute_channel_ir_waveforms(
            &initial_curve,
            &biquads,
            fir_coeffs.as_deref(),
            delay_ms,
            sample_rate,
        ) && let Some(chain) = result.channels.get_mut(&channel_name)
        {
            chain.pre_ir = Some(pre_ir);
            chain.post_ir = Some(post_ir);
        }
    }
}

pub(super) fn build_timing_diagnostics(
    config: &RoomConfig,
    arrivals_ms: &HashMap<String, f64>,
    chains: &HashMap<String, ChannelDspChain>,
) -> Option<crate::roomeq::home_cinema::TimingDiagnosticsReport> {
    if arrivals_ms.is_empty() {
        return None;
    }

    let mut channels = Vec::new();
    for (name, arrival_ms) in arrivals_ms {
        let applied_delay_ms = chains.get(name).map(total_chain_delay_ms).unwrap_or(0.0);
        let final_arrival_ms = arrival_ms + applied_delay_ms;
        channels.push(crate::roomeq::home_cinema::ChannelTimingReport {
            name: name.clone(),
            role: crate::roomeq::home_cinema::role_for_channel(name),
            measured_arrival_ms: *arrival_ms,
            acoustic_distance_m: arrival_ms * 0.343,
            applied_delay_ms,
            final_arrival_ms,
            final_offset_from_reference_ms: 0.0,
        });
    }
    channels.sort_by(|a, b| a.name.cmp(&b.name));

    let before_values: Vec<f64> = channels
        .iter()
        .map(|channel| channel.measured_arrival_ms)
        .collect();
    let after_values: Vec<f64> = channels
        .iter()
        .map(|channel| channel.final_arrival_ms)
        .collect();
    let arrival_spread_before_ms = spread(&before_values).unwrap_or(0.0);
    let arrival_spread_after_ms = spread(&after_values).unwrap_or(0.0);
    let reference_arrival_ms = after_values.iter().copied().reduce(f64::max);
    let reference_channel = reference_arrival_ms.and_then(|reference| {
        channels
            .iter()
            .find(|channel| (channel.final_arrival_ms - reference).abs() < 1e-6)
            .map(|channel| channel.name.clone())
    });
    if let Some(reference) = reference_arrival_ms {
        for channel in &mut channels {
            channel.final_offset_from_reference_ms = channel.final_arrival_ms - reference;
        }
    }

    let mut advisories = Vec::new();
    if arrival_spread_before_ms > ARRIVAL_TIME_WARNING_THRESHOLD_MS {
        advisories.push("large_measured_arrival_spread".to_string());
    }
    if arrival_spread_after_ms > 0.5 {
        advisories.push("post_dsp_arrivals_not_aligned".to_string());
    }
    if let Some(lcr_advisory) = lcr_timing_advisory(&channels) {
        advisories.push(lcr_advisory);
    }
    if surround_or_height_precedence_risk(&channels) {
        advisories.push("surround_or_height_precedence_risk".to_string());
    }
    if advisories.is_empty() {
        advisories.push("ok".to_string());
    }

    let _ = config;
    Some(crate::roomeq::home_cinema::TimingDiagnosticsReport {
        reference_channel,
        reference_arrival_ms,
        arrival_spread_before_ms,
        arrival_spread_after_ms,
        channels,
        advisories,
    })
}

pub(super) fn lcr_timing_advisory(
    channels: &[crate::roomeq::home_cinema::ChannelTimingReport],
) -> Option<String> {
    let front_or_center: Vec<_> = channels
        .iter()
        .filter(|channel| {
            matches!(
                channel.role,
                crate::roomeq::home_cinema::HomeCinemaRole::FrontLeft
                    | crate::roomeq::home_cinema::HomeCinemaRole::FrontRight
                    | crate::roomeq::home_cinema::HomeCinemaRole::Center
            )
        })
        .collect();
    if front_or_center.len() < 2 {
        return None;
    }
    let values: Vec<f64> = front_or_center
        .iter()
        .map(|channel| channel.final_arrival_ms)
        .collect();
    if spread(&values).unwrap_or(0.0) > 0.5 {
        Some("lcr_imaging_timing_spread".to_string())
    } else {
        None
    }
}

pub(super) fn surround_or_height_precedence_risk(
    channels: &[crate::roomeq::home_cinema::ChannelTimingReport],
) -> bool {
    let front_reference = channels
        .iter()
        .filter(|channel| {
            matches!(
                channel.role,
                crate::roomeq::home_cinema::HomeCinemaRole::FrontLeft
                    | crate::roomeq::home_cinema::HomeCinemaRole::FrontRight
                    | crate::roomeq::home_cinema::HomeCinemaRole::Center
            )
        })
        .map(|channel| channel.final_arrival_ms)
        .reduce(f64::min);
    let Some(front_reference) = front_reference else {
        return false;
    };
    channels.iter().any(|channel| {
        let surround_or_height = matches!(
            channel.role,
            crate::roomeq::home_cinema::HomeCinemaRole::SideSurroundLeft
                | crate::roomeq::home_cinema::HomeCinemaRole::SideSurroundRight
                | crate::roomeq::home_cinema::HomeCinemaRole::RearSurroundLeft
                | crate::roomeq::home_cinema::HomeCinemaRole::RearSurroundRight
                | crate::roomeq::home_cinema::HomeCinemaRole::WideLeft
                | crate::roomeq::home_cinema::HomeCinemaRole::WideRight
        ) || channel.role.is_height();
        surround_or_height && channel.final_arrival_ms + 0.5 < front_reference
    })
}

pub(super) fn spread(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().copied().reduce(f64::min)?;
    let max = values.iter().copied().reduce(f64::max)?;
    Some(max - min)
}

pub(super) fn update_perceptual_metrics(
    metadata: &mut OptimizationMetadata,
    channels: Option<&HashMap<String, ChannelDspChain>>,
    config: Option<&RoomConfig>,
) {
    let Some(epa_per_channel) = metadata.epa_per_channel.as_ref() else {
        metadata.perceptual_metrics = None;
        return;
    };
    if epa_per_channel.is_empty() {
        metadata.perceptual_metrics = None;
        return;
    }

    let count = epa_per_channel.len() as f64;
    let epa_preference_pre = epa_per_channel
        .values()
        .map(|metrics| metrics.pre.preference)
        .sum::<f64>()
        / count;
    let epa_preference_post = epa_per_channel
        .values()
        .map(|metrics| metrics.post.preference)
        .sum::<f64>()
        / count;
    let channel_matching_midrange_rms_db = metadata
        .inter_channel_deviation
        .as_ref()
        .map(|icd| icd.midrange_rms_db);
    let role_channel_matching_rms_db = channels.and_then(role_channel_matching_rms_db);
    let bass_consistency_rms_db = channels.and_then(bass_consistency_rms_db);
    let dialog_band_roughness_rms_db = channels.and_then(dialog_band_roughness_rms_db);
    let headroom_peak_boost_db = channels.and_then(headroom_peak_boost_db);
    let headroom_risk = headroom_peak_boost_db.map(|peak_boost| {
        let margin_db = config
            .and_then(|cfg| cfg.system.as_ref())
            .and_then(|system| system.bass_management.as_ref())
            .map(|bm| bm.headroom_margin_db)
            .unwrap_or(6.0);
        if peak_boost > margin_db {
            "high_boost_exceeds_headroom_margin".to_string()
        } else if peak_boost > margin_db * 0.5 {
            "moderate_boost_uses_headroom".to_string()
        } else {
            "ok".to_string()
        }
    });
    let timing_confidence = metadata.group_delay.as_ref().map(|gd| {
        if gd.applied {
            "gd_applied".to_string()
        } else if gd.advisory == "success" {
            "gd_success_not_applied".to_string()
        } else {
            format!("gd_{}", gd.advisory)
        }
    });

    metadata.perceptual_metrics = Some(PerceptualMetrics {
        epa_preference_pre,
        epa_preference_post,
        epa_preference_delta: epa_preference_post - epa_preference_pre,
        channel_matching_midrange_rms_db,
        role_channel_matching_rms_db,
        bass_consistency_rms_db,
        dialog_band_roughness_rms_db,
        headroom_peak_boost_db,
        headroom_risk,
        timing_confidence,
    });
}

pub(super) fn role_channel_matching_rms_db(
    channels: &HashMap<String, ChannelDspChain>,
) -> Option<f64> {
    let mut grouped: HashMap<&'static str, Vec<&ChannelDspChain>> = HashMap::new();
    for (name, chain) in channels {
        if let Some(key) = channel_matching_role_key(name) {
            grouped.entry(key).or_default().push(chain);
        }
    }

    let mut group_rms = Vec::new();
    for group in grouped.values() {
        if group.len() < 2 {
            continue;
        }
        if let Some(rms) = group_mean_deviation_rms_db(group, (300.0, 4_000.0)) {
            group_rms.push(rms);
        }
    }
    mean(&group_rms)
}

pub(super) fn bass_consistency_rms_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let bass_channels: Vec<&ChannelDspChain> = channels
        .iter()
        .filter_map(|(name, chain)| {
            if crate::roomeq::home_cinema::role_for_channel(name).is_sub_or_lfe() {
                Some(chain)
            } else {
                None
            }
        })
        .collect();
    if bass_channels.len() < 2 {
        return None;
    }
    group_mean_deviation_rms_db(&bass_channels, (20.0, 160.0))
}

pub(super) fn dialog_band_roughness_rms_db(
    channels: &HashMap<String, ChannelDspChain>,
) -> Option<f64> {
    let center = channels.iter().find_map(|(name, chain)| {
        if crate::roomeq::home_cinema::role_for_channel(name)
            == crate::roomeq::home_cinema::HomeCinemaRole::Center
        {
            Some(chain)
        } else {
            None
        }
    })?;
    curve_roughness_rms_db(center.final_curve.as_ref()?, (300.0, 4_000.0))
}

pub(super) fn headroom_peak_boost_db(channels: &HashMap<String, ChannelDspChain>) -> Option<f64> {
    let mut peak = 0.0_f64;
    let mut saw_plugin = false;
    for chain in channels.values() {
        for plugin in &chain.plugins {
            if plugin.plugin_type == "gain" {
                if let Some(gain_db) = plugin.parameters.get("gain_db").and_then(|v| v.as_f64()) {
                    peak = peak.max(gain_db);
                    saw_plugin = true;
                }
            } else if plugin.plugin_type == "eq"
                && let Some(filters) = plugin.parameters.get("filters").and_then(|v| v.as_array())
            {
                for filter in filters {
                    if let Some(gain_db) = filter.get("db_gain").and_then(|v| v.as_f64()) {
                        peak = peak.max(gain_db);
                        saw_plugin = true;
                    }
                }
            }
        }
    }
    if saw_plugin { Some(peak) } else { None }
}

pub(super) fn group_mean_deviation_rms_db(
    channels: &[&ChannelDspChain],
    band: (f64, f64),
) -> Option<f64> {
    let reference = channels.first()?.final_curve.as_ref()?;
    if channels.iter().any(|chain| {
        chain.final_curve.as_ref().is_none_or(|curve| {
            curve.freq.len() != reference.freq.len()
                || curve.freq.iter().zip(reference.freq.iter()).any(|(a, b)| {
                    let scale = a.abs().max(b.abs()).max(1.0);
                    (a - b).abs() > scale * 1e-6
                })
        })
    }) {
        return None;
    }

    let mut deviations = Vec::new();
    for idx in 0..reference.freq.len() {
        let freq = reference.freq[idx];
        if freq < band.0 || freq > band.1 {
            continue;
        }
        let values: Vec<f64> = channels
            .iter()
            .filter_map(|chain| chain.final_curve.as_ref().map(|curve| curve.spl[idx]))
            .collect();
        let Some(avg) = mean(&values) else {
            continue;
        };
        deviations.extend(values.into_iter().map(|value| value - avg));
    }
    rms(&deviations)
}

pub(super) fn curve_roughness_rms_db(
    curve: &crate::roomeq::types::CurveData,
    band: (f64, f64),
) -> Option<f64> {
    let values: Vec<f64> = curve
        .freq
        .iter()
        .zip(curve.spl.iter())
        .filter_map(|(freq, spl)| {
            if *freq >= band.0 && *freq <= band.1 {
                Some(*spl)
            } else {
                None
            }
        })
        .collect();
    let avg = mean(&values)?;
    let deviations: Vec<f64> = values.into_iter().map(|value| value - avg).collect();
    rms(&deviations)
}

pub(super) fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

pub(super) fn rms(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some((values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt())
    }
}

pub(super) fn channel_matching_role_key(channel_name: &str) -> Option<&'static str> {
    crate::roomeq::home_cinema::matching_group_key(channel_name)
}

#[derive(Debug, Clone)]
pub(super) struct RoleChannelMatchingGroup {
    pub(super) role_key: &'static str,
    pub(super) curves: HashMap<String, crate::Curve>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RoleChannelMatchingProfile {
    pub(super) rms_threshold_db: f64,
    pub(super) correction: crate::roomeq::spectral_align::ChannelMatchingCorrectionProfile,
}

pub(super) fn channel_matching_profile_for_role_key(
    role_key: &str,
    base_threshold_db: f64,
) -> RoleChannelMatchingProfile {
    let base_threshold_db = base_threshold_db.max(0.05);
    let (threshold_factor, correction_weight, min_freq_hz, max_freq_hz) = match role_key {
        "front_lr" => (2.0 / 3.0, 1.0, 80.0, 16_000.0),
        "side_surrounds" | "rear_surrounds" | "wides" => (1.0, 0.85, 100.0, 12_000.0),
        "top_front" | "top_middle" | "top_rear" => (4.0 / 3.0, 0.65, 120.0, 10_000.0),
        _ => (1.0, 0.8, 200.0, 10_000.0),
    };
    let rms_threshold_db = base_threshold_db * threshold_factor;

    RoleChannelMatchingProfile {
        rms_threshold_db,
        correction: crate::roomeq::spectral_align::ChannelMatchingCorrectionProfile {
            peak_tolerance_db: rms_threshold_db,
            correction_weight,
            min_freq_hz,
            max_freq_hz,
        },
    }
}

pub(super) fn channel_matching_band_rms_db(
    icd: &crate::roomeq::types::InterChannelDeviation,
    band: (f64, f64),
) -> f64 {
    let mut sum_sq = 0.0;
    let mut count = 0usize;
    for (freq, spread) in &icd.deviation_per_freq {
        if *freq >= band.0 && *freq <= band.1 {
            sum_sq += spread * spread;
            count += 1;
        }
    }

    if count > 0 {
        (sum_sq / count as f64).sqrt()
    } else {
        icd.midrange_rms_db
    }
}

#[cfg(test)]
pub(super) fn role_aware_channel_matching_groups(
    final_curves: &HashMap<String, crate::Curve>,
) -> Vec<HashMap<String, crate::Curve>> {
    role_aware_channel_matching_groups_with_keys(final_curves)
        .into_iter()
        .map(|group| group.curves)
        .collect()
}

pub(super) fn role_aware_channel_matching_groups_with_keys(
    final_curves: &HashMap<String, crate::Curve>,
) -> Vec<RoleChannelMatchingGroup> {
    let mut grouped: HashMap<&'static str, HashMap<String, crate::Curve>> = HashMap::new();

    for (name, curve) in final_curves {
        if let Some(key) = channel_matching_role_key(name) {
            grouped
                .entry(key)
                .or_default()
                .insert(name.clone(), curve.clone());
        }
    }

    let order = [
        "front_lr",
        "side_surrounds",
        "rear_surrounds",
        "wides",
        "top_front",
        "top_middle",
        "top_rear",
        "generic",
    ];

    order
        .iter()
        .filter_map(|key| {
            grouped.remove(key).map(|curves| RoleChannelMatchingGroup {
                role_key: key,
                curves,
            })
        })
        .filter(|group| group.curves.len() > 1)
        .collect()
}

pub(super) fn apply_channel_matching_correction(
    result: &mut RoomOptimizationResult,
    correction: &crate::roomeq::spectral_align::ChannelMatchingResult,
    sample_rate: f64,
) {
    if let Some(plugin) = &correction.plugin {
        info!(
            "  Channel '{}': {} matching filters",
            correction.channel_name,
            correction.filters.len(),
        );
        for f in &correction.filters {
            info!(
                "    PK @ {:.0} Hz, Q={:.2}, gain={:+.1} dB",
                f.freq, f.q, f.db_gain,
            );
        }

        if let Some(chain) = result.channels.get_mut(&correction.channel_name) {
            chain.plugins.push(plugin.clone());
        }

        if let Some(ch_result) = result.channel_results.get_mut(&correction.channel_name) {
            let resp = crate::response::compute_peq_complex_response(
                &correction.filters,
                &ch_result.final_curve.freq,
                sample_rate,
            );
            ch_result.final_curve =
                crate::response::apply_complex_response(&ch_result.final_curve, &resp);

            if let Some(chain) = result.channels.get_mut(&correction.channel_name)
                && let Some(ref display_final) = chain.final_curve
            {
                let display_curve: crate::Curve = display_final.clone().into();
                let display_resp = crate::response::compute_peq_complex_response(
                    &correction.filters,
                    &display_curve.freq,
                    sample_rate,
                );
                let corrected =
                    crate::response::apply_complex_response(&display_curve, &display_resp);
                chain.final_curve = Some((&corrected).into());
            }
        }
    }
}

pub(super) fn channel_matching_worsens_reported_scores(
    result: &RoomOptimizationResult,
    config: &RoomConfig,
    baseline: &HashMap<String, ChannelOptimizationResult>,
) -> Option<(String, f64, f64)> {
    result.channel_results.iter().find_map(|(name, ch)| {
        let before = baseline.get(name)?.post_score;
        let (score_min, score_max) = final_score_band_for_channel(config, name);
        let after = recompute_curve_flatness_score(&ch.final_curve, score_min, score_max);
        if after > before + 1e-6 {
            Some((name.clone(), before, after))
        } else {
            None
        }
    })
}

/// Compute inter-channel deviation and optionally apply correction filters.
///
/// Called after per-channel optimization on any result with >1 channel.
/// If `channel_matching` config is enabled and ICD exceeds threshold, adds
/// targeted PEQ filters to bring channels closer together.
pub(super) fn compute_and_correct_icd(
    result: &mut RoomOptimizationResult,
    config: &RoomConfig,
    sample_rate: f64,
) {
    let final_curves: HashMap<String, crate::Curve> = result
        .channel_results
        .iter()
        .filter(|(name, _)| !is_subwoofer_channel(config, name))
        .map(|(name, ch)| (name.clone(), ch.final_curve.clone()))
        .collect();
    if final_curves.len() <= 1 {
        result.metadata.inter_channel_deviation = None;
        return;
    }

    let f3 = final_curves
        .values()
        .filter_map(|c| {
            crate::roomeq::excursion::detect_f3(c, None)
                .ok()
                .map(|r| r.f3_hz)
        })
        .reduce(f64::min)
        .unwrap_or(50.0);

    let icd = crate::roomeq::spectral_align::compute_inter_channel_deviation(&final_curves, f3);
    info!(
        "Inter-channel deviation: midrange_rms={:.2}dB, peak={:.1}dB @{:.0}Hz, passband_rms={:.2}dB",
        icd.midrange_rms_db, icd.midrange_peak_db, icd.midrange_peak_freq, icd.passband_rms_db,
    );

    // Check if correction is enabled and needed.
    // When the user omits `channel_matching`, fall back to `ChannelMatchingConfig::default()`
    // so the two paths (None vs Some(default)) produce identical behaviour.
    let matching_cfg = config
        .optimizer
        .channel_matching
        .clone()
        .unwrap_or_default();
    let enabled = matching_cfg.enabled;
    let threshold = matching_cfg.threshold_db;
    let max_filters = matching_cfg.max_filters;

    if enabled {
        let matching_groups = role_aware_channel_matching_groups_with_keys(&final_curves);
        let mut applied_any = false;
        let baseline_channel_results = result.channel_results.clone();
        let baseline_channels = result.channels.clone();

        if matching_groups.is_empty() {
            info!("No role-compatible channel matching groups found; skipping ICD correction");
        }

        for group in matching_groups {
            let mut group_names: Vec<_> = group.curves.keys().cloned().collect();
            group_names.sort();
            let profile = channel_matching_profile_for_role_key(group.role_key, threshold);
            let matching_band = profile.correction.matching_band(f3);

            let group_icd =
                crate::roomeq::spectral_align::compute_inter_channel_deviation(&group.curves, f3);
            let group_rms_db = channel_matching_band_rms_db(&group_icd, matching_band);
            if group_rms_db <= profile.rms_threshold_db {
                info!(
                    "ICD group [{}] role={} rms={:.2}dB over {:.0}-{:.0}Hz <= threshold={:.2}dB - no correction needed",
                    group_names.join(", "),
                    group.role_key,
                    group_rms_db,
                    matching_band.0,
                    matching_band.1,
                    profile.rms_threshold_db,
                );
                continue;
            }

            info!(
                "ICD group [{}] role={} rms={:.2}dB over {:.0}-{:.0}Hz > threshold={:.2}dB - applying role-aware channel matching (tolerance {:.2}dB, weight {:.2}, max {} filters/ch)",
                group_names.join(", "),
                group.role_key,
                group_rms_db,
                matching_band.0,
                matching_band.1,
                profile.rms_threshold_db,
                profile.correction.peak_tolerance_db,
                profile.correction.correction_weight,
                max_filters,
            );

            let corrections =
                crate::roomeq::spectral_align::correct_inter_channel_deviation_with_profile(
                    &group.curves,
                    f3,
                    max_filters,
                    sample_rate,
                    profile.correction,
                );

            for correction in &corrections {
                if correction.plugin.is_some() {
                    apply_channel_matching_correction(result, correction, sample_rate);
                    applied_any = true;
                }
            }
        }

        if !applied_any {
            result.metadata.inter_channel_deviation = Some(icd);
            return;
        }

        if let Some((channel_name, before, after)) =
            channel_matching_worsens_reported_scores(result, config, &baseline_channel_results)
        {
            info!(
                "ICD correction discarded: channel '{}' score would regress from {:.4} to {:.4}",
                channel_name, before, after
            );
            result.channel_results = baseline_channel_results;
            result.channels = baseline_channels;
            result.metadata.inter_channel_deviation = Some(icd);
            return;
        }

        // Re-compute global ICD after role-aware correction.
        let corrected_curves: HashMap<String, crate::Curve> = result
            .channel_results
            .iter()
            .filter(|(name, _)| !is_subwoofer_channel(config, name))
            .map(|(name, ch)| (name.clone(), ch.final_curve.clone()))
            .collect();
        let icd_after =
            crate::roomeq::spectral_align::compute_inter_channel_deviation(&corrected_curves, f3);
        info!(
            "ICD after correction: midrange_rms={:.2}dB (was {:.2}dB), peak={:.1}dB @{:.0}Hz",
            icd_after.midrange_rms_db,
            icd.midrange_rms_db,
            icd_after.midrange_peak_db,
            icd_after.midrange_peak_freq,
        );
        result.metadata.inter_channel_deviation = Some(icd_after);
    } else {
        result.metadata.inter_channel_deviation = Some(icd);
    }
}
