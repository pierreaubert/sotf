use super::*;

// ─── GD-Opt v2 integration (Phase GD-5) ──────────────────────────────────────

/// Attempt to run GD-Opt v2 on the channel results.
///
/// Returns `Some(GroupDelayOptSummary)` if GD-Opt was attempted (success or
/// advisory skip), `None` if fewer than 2 channels have phase data.
pub(super) fn try_run_gd_opt(
    config: &RoomConfig,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    sample_rate: f64,
) -> Option<crate::roomeq::gd_opt::GroupDelayOptSummary> {
    use crate::roomeq::gd_opt::*;

    let gd_user_config = config.optimizer.group_delay.as_ref()?;
    if !gd_user_config.enabled {
        return None;
    }

    // Collect channels with phase data from the current post-DSP curves.
    // Sort by name for deterministic ordering (HashMap iteration is arbitrary).
    let mut sorted_channels: Vec<(&String, &ChannelOptimizationResult)> =
        channel_results.iter().collect();
    sorted_channels.sort_by_key(|(name, _)| (*name).clone());

    let mut gd_channels: Vec<ChannelMeasurementInput> = Vec::new();
    let mut gd_channel_names: Vec<String> = Vec::new();
    let mut missing_coherence = false;

    for (name, ch) in &sorted_channels {
        // Curve.phase is in degrees — convert to radians for GD computation
        let phase = match ch.final_curve.phase.as_ref() {
            Some(p) => p.mapv(|deg| deg.to_radians()),
            None => continue, // skip channels without phase
        };

        // Coherence is a measurement-confidence signal, so carry it forward
        // from the measurement when the final curve lost metadata during DSP.
        let coherence = ch
            .final_curve
            .coherence
            .clone()
            .or_else(|| ch.initial_curve.coherence.clone())
            .unwrap_or_else(|| {
                missing_coherence = true;
                ndarray::Array1::from_elem(ch.final_curve.freq.len(), 1.0)
            });

        gd_channels.push(ChannelMeasurementInput {
            freq: ch.final_curve.freq.clone(),
            spl: ch.final_curve.spl.clone(),
            phase,
            coherence,
        });
        gd_channel_names.push((*name).clone());
    }

    // Need at least 2 channels with phase
    if gd_channels.len() < 2 {
        if !channel_results.is_empty() && channel_results.len() >= 2 {
            // Had enough channels but not enough phase data
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::NoPhaseData,
            ));
        }
        return None;
    }

    // Derive band from crossover config or use default (80 Hz XO assumption)
    let crossover_freq = config
        .crossovers
        .as_ref()
        .and_then(|xos| xos.values().filter_map(|xo| xo.frequency).reduce(f64::min))
        .unwrap_or(80.0);

    let band = derive_band(config.optimizer.min_freq, crossover_freq);

    // Validate consistent grid lengths and values across channels
    let n_freq = gd_channels[0].freq.len();
    for ch in &gd_channels[1..] {
        if ch.freq.len() != n_freq {
            info!("GD-Opt: skipped — inconsistent frequency grid lengths across channels");
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::FrequencyGridMismatch,
            ));
        }
        if !crate::roomeq::frequency_grid::same_frequency_grid(&gd_channels[0].freq, &ch.freq) {
            info!("GD-Opt: skipped — inconsistent frequency grid values across channels");
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::FrequencyGridMismatch,
            ));
        }
    }

    // Check that band is non-empty in the data
    let band_count = (0..n_freq)
        .filter(|&i| gd_channels[0].freq[i] >= band.0 && gd_channels[0].freq[i] <= band.1)
        .count();

    if band_count < 3 {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::EmptyBand,
        ));
    }

    // Check mean coherence
    let mean_coh: f64 = gd_channels
        .iter()
        .flat_map(|ch| {
            (0..n_freq)
                .filter(|&i| ch.freq[i] >= band.0 && ch.freq[i] <= band.1)
                .map(|i| ch.coherence[i])
        })
        .sum::<f64>()
        / (gd_channels.len() * band_count) as f64;

    if !missing_coherence && mean_coh < gd_user_config.coherence_threshold {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::CoherenceBelowThreshold {
                mean_coherence: mean_coh,
            },
        ));
    }

    // Configure and run.
    // AP frequency range is clamped to [20, 500] intersected with the band.
    // If the range is degenerate (min >= max), disable AP filters.
    let ap_min_freq = band.0.max(20.0);
    let ap_max_freq = band.1.min(500.0);
    let (mut ap_per_channel, ap_min_freq, ap_max_freq) = if ap_min_freq < ap_max_freq {
        (gd_user_config.ap_per_channel, ap_min_freq, ap_max_freq)
    } else {
        // AP range is empty — run delay-only
        (0, 20.0, 300.0)
    };
    let mut advisory_override: Option<GdOptAdvisory> = None;
    let mut optimize_polarity = gd_user_config.optimize_polarity;

    if missing_coherence {
        ap_per_channel = 0;
        optimize_polarity = false;
        advisory_override = Some(GdOptAdvisory::MissingCoherenceDelayOnly);
    } else if gd_user_config.adaptive_allpass && ap_per_channel > 0 {
        // Production currently has only the averaged measurement, not
        // independent sweep realisations. Avoid single-measurement AP overfit.
        ap_per_channel = 0;
        advisory_override = Some(GdOptAdvisory::AllPassDisabledNoBootstrapRealisations);
    }

    let gd_config = GdOptConfig {
        sample_rate,
        max_delay_ms: gd_user_config.max_delay_ms,
        ap_per_channel,
        ap_min_freq,
        ap_max_freq,
        ap_min_q: gd_user_config.ap_min_q,
        ap_max_q: gd_user_config.ap_max_q,
        optimize_polarity,
        max_iter: gd_user_config.max_iter,
        popsize: gd_user_config.popsize,
        tol: gd_user_config.tol,
        seed: config.optimizer.seed,
    };

    let result = optimize_group_delay_for_mode(
        &gd_channels,
        band,
        &gd_config,
        &config.optimizer.processing_mode,
        config.optimizer.mixed_config.as_ref(),
    );

    match result {
        Ok(gd_result) => {
            if gd_result.improvement_db < gd_user_config.min_improvement_db {
                info!(
                    "GD-Opt: minimal improvement ({:.1} dB), skipping",
                    gd_result.improvement_db
                );
                Some(GroupDelayOptSummary::from_advisory(
                    &GdOptAdvisory::MinimalImprovement {
                        improvement_db: gd_result.improvement_db,
                    },
                ))
            } else {
                let applied = apply_gd_opt_result(
                    &gd_result,
                    &gd_channel_names,
                    channel_results,
                    channel_chains,
                    sample_rate,
                );
                info!(
                    "GD-Opt: improvement {:.1} dB (pre={:.2}ms, post={:.2}ms) in band [{:.0}, {:.0}] Hz; applied={}",
                    gd_result.improvement_db,
                    gd_result.sum_gd_pre_rms_ms,
                    gd_result.sum_gd_post_rms_ms,
                    band.0,
                    band.1,
                    applied,
                );
                let mut summary = GroupDelayOptSummary::from_result_with_names(
                    &gd_result,
                    gd_channel_names.clone(),
                )
                .with_applied(applied);
                if let Some(advisory) = advisory_override {
                    summary.advisory = GroupDelayOptSummary::from_advisory(&advisory).advisory;
                    if matches!(advisory, GdOptAdvisory::MissingCoherenceDelayOnly) {
                        summary.mean_coherence = 0.0;
                    }
                }
                Some(summary)
            }
        }
        Err(e) => {
            info!("GD-Opt: skipped — {}", e);
            // Map known error messages to advisories
            if e.contains("PhaseLinear") {
                Some(GroupDelayOptSummary::from_advisory(
                    &GdOptAdvisory::PhaseLinearNoTarget,
                ))
            } else {
                None
            }
        }
    }
}

pub(super) fn apply_gd_opt_result(
    result: &crate::roomeq::gd_opt::GroupDelayOptResult,
    channel_names: &[String],
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    sample_rate: f64,
) -> bool {
    let mut applied_any = false;

    for (name, ch_result) in channel_names.iter().zip(result.per_channel.iter()) {
        let mut inserted_for_channel = false;
        if let Some(chain) = channel_chains.get_mut(name.as_str()) {
            if ch_result.polarity_inverted {
                chain
                    .plugins
                    .push(output::create_gain_plugin_with_invert(0.0, true));
                inserted_for_channel = true;
            }
            if ch_result.delay_ms > 0.01 {
                chain
                    .plugins
                    .push(output::create_delay_plugin(ch_result.delay_ms));
                inserted_for_channel = true;
            }
            if !ch_result.ap_filters.is_empty() {
                chain
                    .plugins
                    .push(output::create_eq_plugin(&ch_result.ap_filters));
                inserted_for_channel = true;
            }
        }

        if let Some(ch) = channel_results.get_mut(name.as_str()) {
            let response = gd_phase_response_for_curve(
                &ch.final_curve.freq,
                ch_result.delay_ms,
                ch_result.polarity_inverted,
                &ch_result.ap_filters,
                sample_rate,
            );
            ch.final_curve = crate::response::apply_complex_response(&ch.final_curve, &response);
            if let Some(chain) = channel_chains.get_mut(name.as_str()) {
                chain.final_curve = Some((&ch.final_curve).into());
            }
        }

        applied_any |= inserted_for_channel;
    }

    applied_any
}

pub(super) fn gd_phase_response_for_curve(
    freqs: &ndarray::Array1<f64>,
    delay_ms: f64,
    polarity_inverted: bool,
    ap_filters: &[Biquad],
    sample_rate: f64,
) -> Vec<Complex64> {
    freqs
        .iter()
        .map(|&f| {
            let mut h = Complex64::new(1.0, 0.0);
            if delay_ms.abs() > 1e-12 {
                h *= Complex64::from_polar(1.0, -2.0 * PI * f * delay_ms * 1e-3);
            }
            for ap in ap_filters {
                // Rebuild with the active sample rate so persisted filter
                // metadata and phase-curve reporting stay aligned.
                let ap = Biquad::new(ap.filter_type, ap.freq, sample_rate, ap.q, ap.db_gain);
                h *= ap.complex_response(f);
            }
            if polarity_inverted {
                h = -h;
            }
            h
        })
        .collect()
}
