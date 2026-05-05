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

    let mut adaptive_realisations: Option<Vec<Vec<ChannelMeasurementInput>>> = None;

    if missing_coherence {
        ap_per_channel = 0;
        optimize_polarity = false;
        advisory_override = Some(GdOptAdvisory::MissingCoherenceDelayOnly);
    } else if gd_user_config.adaptive_allpass && ap_per_channel > 0 {
        adaptive_realisations =
            build_gd_sweep_realisations(config, channel_results, &gd_channel_names);
        if adaptive_realisations.is_none() {
            // Keep the existing safety gate when production has only an
            // averaged measurement. Independent sweeps are required before
            // accepting all-pass filters from the adaptive bootstrap.
            ap_per_channel = 0;
            advisory_override = Some(GdOptAdvisory::AllPassDisabledNoBootstrapRealisations);
        }
    }

    if matches!(config.optimizer.processing_mode, ProcessingMode::MixedPhase) {
        ap_per_channel = ap_per_channel.min(1);
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

    let result = if let Some(realisations) = adaptive_realisations.as_deref() {
        if let ProcessingMode::Hybrid = config.optimizer.processing_mode {
            let xo_freq = config
                .optimizer
                .mixed_config
                .as_ref()
                .map(|m| m.crossover_freq)
                .unwrap_or(300.0);
            if band.1 > xo_freq {
                Err(format!(
                    "Hybrid mode: GD-Opt band_hi ({:.1} Hz) exceeds mixed_config crossover \
                     ({:.1} Hz). AP filters must stay in the IIR band.",
                    band.1, xo_freq,
                ))
            } else {
                optimize_group_delay_adaptive(&gd_channels, realisations, band, &gd_config)
            }
        } else {
            info!(
                "GD-Opt: adaptive all-pass bootstrap using {} sweep realisations",
                realisations.len()
            );
            optimize_group_delay_adaptive(&gd_channels, realisations, band, &gd_config)
        }
    } else {
        optimize_group_delay_for_mode(
            &gd_channels,
            band,
            &gd_config,
            &config.optimizer.processing_mode,
            config.optimizer.mixed_config.as_ref(),
        )
    };

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

pub(super) fn try_run_phase_linear_fir_gd(
    config: &RoomConfig,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Option<crate::roomeq::gd_opt::GroupDelayOptSummary> {
    use crate::roomeq::gd_opt::*;

    let gd_user_config = config.optimizer.group_delay.as_ref()?;
    if !gd_user_config.enabled {
        return None;
    }

    let mut sorted_channels: Vec<(&String, &ChannelOptimizationResult)> =
        channel_results.iter().collect();
    sorted_channels.sort_by_key(|(name, _)| (*name).clone());

    let mut gd_channels: Vec<ChannelMeasurementInput> = Vec::new();
    let mut gd_channel_names: Vec<String> = Vec::new();
    let mut missing_coherence = false;

    for (name, ch) in &sorted_channels {
        let phase = match ch.final_curve.phase.as_ref() {
            Some(p) => p.mapv(|deg| deg.to_radians()),
            None => continue,
        };
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

    if gd_channels.len() < 2 {
        if !channel_results.is_empty() && channel_results.len() >= 2 {
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::NoPhaseData,
            ));
        }
        return None;
    }

    let crossover_freq = config
        .crossovers
        .as_ref()
        .and_then(|xos| xos.values().filter_map(|xo| xo.frequency).reduce(f64::min))
        .unwrap_or(80.0);
    let band = derive_band(config.optimizer.min_freq, crossover_freq);

    let n_freq = gd_channels[0].freq.len();
    for ch in &gd_channels[1..] {
        if ch.freq.len() != n_freq
            || !crate::roomeq::frequency_grid::same_frequency_grid(&gd_channels[0].freq, &ch.freq)
        {
            return Some(GroupDelayOptSummary::from_advisory(
                &GdOptAdvisory::FrequencyGridMismatch,
            ));
        }
    }

    let band_count = (0..n_freq)
        .filter(|&i| gd_channels[0].freq[i] >= band.0 && gd_channels[0].freq[i] <= band.1)
        .count();
    if band_count < 3 {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::EmptyBand,
        ));
    }

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

    let gd_config = GdOptConfig {
        sample_rate,
        max_delay_ms: gd_user_config.max_delay_ms,
        ap_per_channel: 0,
        ap_min_freq: 20.0,
        ap_max_freq: 300.0,
        ap_min_q: gd_user_config.ap_min_q,
        ap_max_q: gd_user_config.ap_max_q,
        optimize_polarity: false,
        max_iter: gd_user_config.max_iter,
        popsize: gd_user_config.popsize,
        tol: gd_user_config.tol,
        seed: config.optimizer.seed,
    };

    let gd_result = match optimize_group_delay(&gd_channels, band, &gd_config) {
        Ok(result) => result,
        Err(e) => {
            info!("GD-Opt FIR target: skipped — {}", e);
            return None;
        }
    };

    if gd_result.improvement_db < gd_user_config.min_improvement_db {
        return Some(GroupDelayOptSummary::from_advisory(
            &GdOptAdvisory::MinimalImprovement {
                improvement_db: gd_result.improvement_db,
            },
        ));
    }

    let target = build_gd_alignment_target(&gd_channels, &gd_result, &gd_config);
    let mut applied = false;
    let mut phase_updates: Vec<(String, f64)> = Vec::new();
    let out_dir = output_dir.unwrap_or(Path::new("."));

    for (channel_index, name) in gd_channel_names.iter().enumerate() {
        let delay_ms = target
            .per_channel_delay_ms
            .get(channel_index)
            .copied()
            .unwrap_or(0.0);
        if delay_ms.abs() <= 0.01 {
            continue;
        }

        let Some(ch) = channel_results.get_mut(name.as_str()) else {
            continue;
        };

        let updated_coeffs = if let Some(existing) = ch.fir_coeffs.as_deref() {
            fir::apply_gd_delay_to_fir_coefficients(existing, delay_ms, sample_rate)
        } else {
            match fir::generate_fir_correction_with_gd_target(
                &ch.initial_curve,
                &config.optimizer,
                config.target_curve.as_ref(),
                sample_rate,
                Some(&target),
                channel_index,
            ) {
                Ok(coeffs) => coeffs,
                Err(e) => {
                    warn!(
                        "GD-Opt FIR target: failed to regenerate FIR for '{}': {}",
                        name, e
                    );
                    continue;
                }
            }
        };

        ch.fir_coeffs = Some(updated_coeffs.clone());

        let filename = format!("{}_fir.wav", name);
        let wav_path = out_dir.join(&filename);
        if let Err(e) = crate::fir::save_fir_to_wav(&updated_coeffs, sample_rate as u32, &wav_path)
        {
            warn!(
                "GD-Opt FIR target: failed to save FIR WAV for '{}': {}",
                name, e
            );
        }

        if let Some(chain) = channel_chains.get_mut(name.as_str()) {
            let has_convolution = chain.plugins.iter().any(|plugin| {
                plugin.plugin_type == "convolution"
                    && plugin
                        .parameters
                        .get("ir_file")
                        .and_then(|value| value.as_str())
                        == Some(filename.as_str())
            });
            if !has_convolution {
                chain
                    .plugins
                    .push(crate::roomeq::output::create_convolution_plugin(&filename));
            }
        }

        phase_updates.push((name.clone(), delay_ms));
        applied = true;
    }

    for (name, delay_ms) in phase_updates {
        sync_reported_phase_adjustment(&name, channel_results, channel_chains, delay_ms, false);
    }

    let mut summary = GroupDelayOptSummary::from_result_with_names(&gd_result, gd_channel_names)
        .with_applied(applied);
    if missing_coherence {
        summary.advisory =
            GroupDelayOptSummary::from_advisory(&GdOptAdvisory::MissingCoherenceDelayOnly).advisory;
        summary.mean_coherence = 0.0;
    }
    Some(summary)
}

fn source_for_output_channel<'a>(
    config: &'a RoomConfig,
    channel_name: &str,
) -> Option<&'a MeasurementSource> {
    let speaker_config = if let Some(system) = &config.system {
        let measurement_key = system.speakers.get(channel_name)?;
        config.speakers.get(measurement_key)?
    } else {
        config.speakers.get(channel_name)?
    };

    match speaker_config {
        SpeakerConfig::Single(source) => Some(source),
        _ => None,
    }
}

fn interpolate_optional_array_log(
    freq_out: &ndarray::Array1<f64>,
    freq_in: &ndarray::Array1<f64>,
    values: &ndarray::Array1<f64>,
) -> ndarray::Array1<f64> {
    let curve = Curve {
        freq: freq_in.clone(),
        spl: values.clone(),
        phase: None,
        ..Default::default()
    };
    crate::read::interpolate_log_space(freq_out, &curve).spl
}

fn corrected_realisation_to_gd_input(
    raw_curve: &Curve,
    initial_curve: &Curve,
    final_curve: &Curve,
) -> Option<crate::roomeq::gd_opt::ChannelMeasurementInput> {
    let raw_on_grid = crate::read::interpolate_log_space(&final_curve.freq, raw_curve);
    let initial_on_grid = crate::read::interpolate_log_space(&final_curve.freq, initial_curve);

    let raw_phase = raw_on_grid.phase.as_ref()?;
    let initial_phase = initial_on_grid.phase.as_ref()?;
    let final_phase = final_curve.phase.as_ref()?;
    let raw_coherence = raw_curve.coherence.as_ref()?;

    let coherence =
        interpolate_optional_array_log(&final_curve.freq, &raw_curve.freq, raw_coherence);

    let spl = &raw_on_grid.spl + &(&final_curve.spl - &initial_on_grid.spl);
    let phase_delta = final_phase - initial_phase;
    let phase = (raw_phase + &phase_delta).mapv(|deg| deg.to_radians());

    Some(crate::roomeq::gd_opt::ChannelMeasurementInput {
        freq: final_curve.freq.clone(),
        spl,
        phase,
        coherence,
    })
}

fn build_gd_sweep_realisations(
    config: &RoomConfig,
    channel_results: &HashMap<String, ChannelOptimizationResult>,
    channel_names: &[String],
) -> Option<Vec<Vec<crate::roomeq::gd_opt::ChannelMeasurementInput>>> {
    let mut per_channel: Vec<Vec<crate::roomeq::gd_opt::ChannelMeasurementInput>> = Vec::new();

    for name in channel_names {
        let source = source_for_output_channel(config, name)?;
        let raw_curves = crate::read::load_source_individual(source).ok()?;
        if raw_curves.len() < 2 {
            return None;
        }

        let ch = channel_results.get(name.as_str())?;
        let mut realisations = Vec::with_capacity(raw_curves.len());
        for raw_curve in &raw_curves {
            let input =
                corrected_realisation_to_gd_input(raw_curve, &ch.initial_curve, &ch.final_curve)?;
            realisations.push(input);
        }
        per_channel.push(realisations);
    }

    let n_realisations = per_channel.first()?.len();
    if n_realisations < 2
        || per_channel
            .iter()
            .any(|inputs| inputs.len() != n_realisations)
    {
        return None;
    }

    let mut by_sweep = Vec::with_capacity(n_realisations);
    for sweep_idx in 0..n_realisations {
        by_sweep.push(
            per_channel
                .iter()
                .map(|inputs| inputs[sweep_idx].clone())
                .collect(),
        );
    }

    Some(by_sweep)
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
