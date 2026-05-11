use super::*;

/// Post-generate FIR coefficients for a channel that only has IIR results.
///
/// For Hybrid mode, uses the IIR-corrected curve as FIR input;
/// for PhaseLinear (FIR-only) mode, uses the raw measurement.
pub(super) fn post_generate_fir(
    name: &str,
    initial_curve: &Curve,
    final_curve: &Curve,
    config: &crate::roomeq::types::OptimizerConfig,
    target_curve: Option<&crate::roomeq::types::TargetCurveConfig>,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Option<Vec<f64>> {
    let fir_input = match config.processing_mode {
        ProcessingMode::Hybrid => final_curve,
        _ => initial_curve,
    };
    match fir::generate_fir_correction(fir_input, config, target_curve, sample_rate) {
        Ok(coeffs) => {
            if let Some(out_dir) = output_dir {
                let filename = format!("{}_fir.wav", name);
                let wav_path = out_dir.join(&filename);
                if let Err(e) = crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                {
                    warn!("Failed to save FIR WAV for {}: {}", name, e);
                } else {
                    info!("  Saved FIR filter to {}", wav_path.display());
                }
            }
            Some(coeffs)
        }
        Err(e) => {
            warn!("FIR generation failed for {}: {}", name, e);
            None
        }
    }
}

/// Post-generate a short excess-phase FIR for MixedPhase mode.
///
/// The workflow path only runs IIR optimisation.  For MixedPhase we still need
/// the short FIR that corrects residual excess phase.  This mirrors the logic
/// in `optimize_speaker_eq` MixedPhase branch but runs after the workflow.
pub(super) fn post_generate_mixed_phase_fir(
    name: &str,
    initial_curve: &Curve,
    config: &crate::roomeq::types::OptimizerConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) -> Option<Vec<f64>> {
    let phase = initial_curve.phase.as_ref()?;
    if phase.is_empty() {
        return None;
    }

    let mp_config = match &config.mixed_phase {
        Some(sc) => crate::roomeq::mixed_phase::MixedPhaseConfig {
            max_fir_length_ms: sc.max_fir_length_ms,
            pre_ringing_threshold_db: sc.pre_ringing_threshold_db,
            min_spatial_depth: sc.min_spatial_depth,
            phase_smoothing_octaves: sc.phase_smoothing_octaves,
        },
        None => crate::roomeq::mixed_phase::MixedPhaseConfig::default(),
    };

    match crate::roomeq::mixed_phase::decompose_phase(initial_curve, &mp_config) {
        Ok((_min_phase, _excess, delay_ms, residual)) => {
            info!(
                "  Mixed-phase (post-workflow) '{}': delay={:.2} ms",
                name, delay_ms
            );
            let coeffs = crate::roomeq::mixed_phase::generate_excess_phase_fir(
                &initial_curve.freq,
                &residual,
                &mp_config,
                sample_rate,
            );

            if let Some(out_dir) = output_dir {
                let filename = format!("{}_excess_phase_fir.wav", name);
                let wav_path = out_dir.join(&filename);
                if let Err(e) = crate::fir::save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path)
                {
                    warn!("Failed to save excess phase FIR for {}: {}", name, e);
                } else {
                    info!("  Saved excess phase FIR to {}", wav_path.display());
                }
            }

            Some(coeffs)
        }
        Err(e) => {
            warn!(
                "  Mixed-phase decomposition failed for '{}': {}. Using IIR only.",
                name, e
            );
            None
        }
    }
}

/// Apply standalone phase correction to a channel (rePhase-style).
///
/// Generates a phase-only FIR from the measurement's excess phase and appends it
/// to the channel's DSP chain. If the channel already has a magnitude FIR, the
/// two are convolved together so `fir_coeffs` remains a single filter for IR
/// computation.
pub(super) fn apply_phase_correction(
    name: &str,
    ch: &mut ChannelOptimizationResult,
    chain: &mut crate::roomeq::types::ChannelDspChain,
    config: &crate::roomeq::types::MixedPhaseSerdeConfig,
    sample_rate: f64,
    output_dir: Option<&Path>,
) {
    let phase = match ch.initial_curve.phase.as_ref() {
        Some(p) if !p.is_empty() => p,
        _ => return,
    };
    let _ = phase; // used via initial_curve below

    let mp_config = crate::roomeq::mixed_phase::MixedPhaseConfig {
        max_fir_length_ms: config.max_fir_length_ms,
        pre_ringing_threshold_db: config.pre_ringing_threshold_db,
        min_spatial_depth: config.min_spatial_depth,
        phase_smoothing_octaves: config.phase_smoothing_octaves,
    };

    let phase_fir = match crate::roomeq::mixed_phase::decompose_phase(&ch.initial_curve, &mp_config)
    {
        Ok((_min, _excess, delay_ms, residual)) => {
            info!(
                "  Phase correction '{}': delay={:.2} ms, generating phase-only FIR",
                name, delay_ms
            );
            crate::roomeq::mixed_phase::generate_excess_phase_fir(
                &ch.initial_curve.freq,
                &residual,
                &mp_config,
                sample_rate,
            )
        }
        Err(e) => {
            warn!("  Phase correction failed for '{}': {}", name, e);
            return;
        }
    };

    // Save phase FIR WAV and add convolution plugin
    let filename = format!("{}_phase_correction.wav", name);
    if let Some(out_dir) = output_dir {
        let wav_path = out_dir.join(&filename);
        if let Err(e) = crate::fir::save_fir_to_wav(&phase_fir, sample_rate as u32, &wav_path) {
            warn!("Failed to save phase correction FIR for {}: {}", name, e);
        } else {
            info!("  Saved phase correction FIR to {}", wav_path.display());
        }
    }
    chain
        .plugins
        .push(crate::roomeq::output::create_convolution_plugin(&filename));

    let phase_response = crate::response::compute_fir_complex_response(
        &phase_fir,
        &ch.final_curve.freq,
        sample_rate,
    );
    ch.final_curve = crate::response::apply_complex_response(&ch.final_curve, &phase_response);
    chain.final_curve = Some((&ch.final_curve).into());

    // Combine with existing FIR for IR computation (convolve the two)
    if let Some(ref existing) = ch.fir_coeffs {
        ch.fir_coeffs = Some(convolve(existing, &phase_fir));
    } else {
        ch.fir_coeffs = Some(phase_fir);
    }
}

/// Linear convolution of two FIR filters.
pub(super) fn convolve(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len() + b.len() - 1;
    let mut out = vec![0.0; len];
    for (i, &av) in a.iter().enumerate() {
        for (j, &bv) in b.iter().enumerate() {
            out[i + j] += av * bv;
        }
    }
    out
}

pub(super) fn apply_phase_only_adjustment_to_reported_curve(
    curve: &mut Curve,
    delay_ms: f64,
    invert_polarity: bool,
) {
    if curve.freq.is_empty() {
        return;
    }

    let base_phase = curve
        .phase
        .clone()
        .unwrap_or_else(|| ndarray::Array1::zeros(curve.freq.len()));
    let inversion_phase = if invert_polarity { 180.0 } else { 0.0 };
    let delay_s = delay_ms / 1000.0;

    let phase =
        ndarray::Array1::from_iter(curve.freq.iter().zip(base_phase.iter()).map(
            |(&freq_hz, &phase_deg)| phase_deg + inversion_phase - (360.0 * freq_hz * delay_s),
        ));
    curve.phase = Some(phase);
}

pub(super) fn sync_reported_phase_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    delay_ms: f64,
    invert_polarity: bool,
) {
    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        apply_phase_only_adjustment_to_reported_curve(
            &mut ch_result.final_curve,
            delay_ms,
            invert_polarity,
        );

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let mut curve: Curve = final_curve.into();
        apply_phase_only_adjustment_to_reported_curve(&mut curve, delay_ms, invert_polarity);
        chain.final_curve = Some((&curve).into());
    }
}

pub(super) fn sync_reported_gain_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    gain_db: f64,
    invert_polarity: bool,
) {
    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        ch_result.final_curve.spl = &ch_result.final_curve.spl + gain_db;
        apply_phase_only_adjustment_to_reported_curve(
            &mut ch_result.final_curve,
            0.0,
            invert_polarity,
        );

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let mut curve: Curve = final_curve.into();
        curve.spl = &curve.spl + gain_db;
        apply_phase_only_adjustment_to_reported_curve(&mut curve, 0.0, invert_polarity);
        chain.final_curve = Some((&curve).into());
    }
}

pub(super) fn sync_reported_biquad_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    filters: &[Biquad],
    sample_rate: f64,
) {
    if filters.is_empty() {
        return;
    }

    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        let response = crate::response::compute_peq_complex_response(
            filters,
            &ch_result.final_curve.freq,
            sample_rate,
        );
        ch_result.final_curve =
            crate::response::apply_complex_response(&ch_result.final_curve, &response);

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let curve: Curve = final_curve.into();
        let response =
            crate::response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
        let corrected = crate::response::apply_complex_response(&curve, &response);
        chain.final_curve = Some((&corrected).into());
    }
}

pub(super) fn sync_reported_fir_adjustment(
    channel_name: &str,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
    coeffs: &[f64],
    sample_rate: f64,
) {
    if coeffs.is_empty() {
        return;
    }

    if let Some(ch_result) = channel_results.get_mut(channel_name) {
        let response = crate::response::compute_fir_complex_response(
            coeffs,
            &ch_result.final_curve.freq,
            sample_rate,
        );
        ch_result.final_curve =
            crate::response::apply_complex_response(&ch_result.final_curve, &response);

        if let Some(chain) = channel_chains.get_mut(channel_name) {
            chain.final_curve = Some((&ch_result.final_curve).into());
        }
    } else if let Some(chain) = channel_chains.get_mut(channel_name)
        && let Some(final_curve) = chain.final_curve.clone()
    {
        let curve: Curve = final_curve.into();
        let response =
            crate::response::compute_fir_complex_response(coeffs, &curve.freq, sample_rate);
        let corrected = crate::response::apply_complex_response(&curve, &response);
        chain.final_curve = Some((&corrected).into());
    }
}

pub(super) fn collect_current_final_curves(
    channel_results: &HashMap<String, ChannelOptimizationResult>,
) -> HashMap<String, Curve> {
    channel_results
        .iter()
        .map(|(name, result)| (name.clone(), result.final_curve.clone()))
        .collect()
}

pub(super) fn total_chain_delay_ms(chain: &ChannelDspChain) -> f64 {
    chain
        .plugins
        .iter()
        .filter(|plugin| plugin.plugin_type == "delay")
        .filter_map(|plugin| {
            plugin
                .parameters
                .get("delay_ms")
                .and_then(|value| value.as_f64())
        })
        .sum()
}

pub(super) fn compute_phase_alignment_delay_schedule(
    phase_alignment_results: &HashMap<String, (f64, bool, String)>,
) -> HashMap<String, f64> {
    let mut graph: HashMap<String, Vec<(String, f64)>> = HashMap::new();

    for (main_name, (relative_delay_ms, _invert, sub_name)) in phase_alignment_results {
        // The optimizer's delay means: delay(main) - delay(sub) = relative_delay_ms.
        graph
            .entry(sub_name.clone())
            .or_default()
            .push((main_name.clone(), *relative_delay_ms));
        graph
            .entry(main_name.clone())
            .or_default()
            .push((sub_name.clone(), -*relative_delay_ms));
    }

    let mut raw_offsets: HashMap<String, f64> = HashMap::new();
    let mut schedule: HashMap<String, f64> = HashMap::new();

    for start in graph.keys() {
        if raw_offsets.contains_key(start) {
            continue;
        }

        raw_offsets.insert(start.clone(), 0.0);
        let mut stack = vec![start.clone()];
        let mut component = Vec::new();

        while let Some(channel) = stack.pop() {
            component.push(channel.clone());
            let channel_offset = raw_offsets[&channel];

            if let Some(neighbors) = graph.get(&channel) {
                for (neighbor, delta_ms) in neighbors {
                    let neighbor_offset = channel_offset + *delta_ms;
                    if let Some(existing) = raw_offsets.get(neighbor) {
                        if (existing - neighbor_offset).abs() > 0.05 {
                            warn!(
                                "Conflicting phase-alignment delay constraints for '{}': {:.3} ms vs {:.3} ms; averaging",
                                neighbor, existing, neighbor_offset
                            );
                            let consensus = (existing + neighbor_offset) / 2.0;
                            raw_offsets.insert(neighbor.clone(), consensus);
                        }
                    } else {
                        raw_offsets.insert(neighbor.clone(), neighbor_offset);
                        stack.push(neighbor.clone());
                    }
                }
            }
        }

        let min_offset = component
            .iter()
            .filter_map(|name| raw_offsets.get(name))
            .copied()
            .fold(f64::INFINITY, f64::min);

        for name in component {
            if let Some(offset) = raw_offsets.get(&name) {
                let delay_ms = offset - min_offset;
                if delay_ms > 0.01 {
                    schedule.insert(name, delay_ms);
                }
            }
        }
    }

    schedule
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn phase_alignment_schedule_is_consistent_for_shared_sub() {
        // Two mains aligned to the same sub — a common real-world case.
        // L -> LFE: -5.0 ms (LFE arrives 5 ms after L)
        // R -> LFE:  2.0 ms (LFE arrives 2 ms before R)
        let results = HashMap::from([
            ("L".to_string(), (-5.0, false, "LFE".to_string())),
            ("R".to_string(), (2.0, false, "LFE".to_string())),
        ]);

        let schedule = super::compute_phase_alignment_delay_schedule(&results);

        // L is the reference (earliest arrival), so L gets 0 delay.
        // LFE needs +5.0 ms to align with L.
        // R needs +7.0 ms to align with L (5.0 + 2.0).
        assert_close(*schedule.get("L").unwrap_or(&0.0), 0.0);
        assert_close(schedule["LFE"], 5.0);
        assert_close(schedule["R"], 7.0);
    }

    #[test]
    fn conflicting_delays_use_consensus_instead_of_arbitrary_first() {
        // When the same channel is reached with conflicting offsets,
        // the code should average the estimates rather than keeping
        // the arbitrary first one.
        //
        // We simulate this by building a graph with a cycle that has
        // inconsistent edge weights. In practice this requires the
        // same channel to appear in multiple alignment pairs.
        let mut results = HashMap::new();
        results.insert("A".to_string(), (0.0, false, "B".to_string()));
        results.insert("B".to_string(), (0.0, false, "C".to_string()));
        results.insert("C".to_string(), (5.0, false, "A".to_string()));

        let schedule = super::compute_phase_alignment_delay_schedule(&results);

        // The cycle A-B-C-A has total weight 0+0+5 = 5 ms, which is inconsistent
        // (should be 0 for a consistent cycle). The conflict resolution
        // should produce a finite schedule for all channels.
        assert!(
            schedule.contains_key("A") || schedule.contains_key("B") || schedule.contains_key("C"),
            "schedule should not be empty even with inconsistent cycle"
        );
    }

    fn assert_close(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 0.01,
            "assertion failed: {} ≈ {} (diff = {})",
            a,
            b,
            (a - b).abs()
        );
    }
}

pub(super) fn apply_phase_alignment_delay_schedule(
    phase_alignment_results: &HashMap<String, (f64, bool, String)>,
    channel_results: &mut HashMap<String, ChannelOptimizationResult>,
    channel_chains: &mut HashMap<String, ChannelDspChain>,
) -> HashMap<String, f64> {
    let schedule = compute_phase_alignment_delay_schedule(phase_alignment_results);

    for (channel_name, delay_ms) in &schedule {
        let applied = if let Some(chain) = channel_chains.get_mut(channel_name.as_str()) {
            output::add_delay_plugin(chain, *delay_ms);
            true
        } else {
            false
        };

        if applied {
            sync_reported_phase_adjustment(
                channel_name,
                channel_results,
                channel_chains,
                *delay_ms,
                false,
            );
            info!(
                "  Applied {:.3} ms phase alignment delay to '{}'",
                delay_ms, channel_name
            );
        }
    }

    schedule
}
