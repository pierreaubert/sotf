//\! Multi-speaker group and advanced processing
//\!
//\! This module handles optimization of speaker groups (multi-driver), multi-subwoofer arrays,
//\! DBA configurations, Cardioid arrays, and mixed-mode frequency-split processing.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read as load;
use crate::response;
use log::{debug, info, warn};
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;
use std::collections::HashMap;
use std::path::Path;

use super::artifacts::{self, ConvolutionArtifactKind};
use super::crossover;
use super::dba;
use super::eq;
use super::fir;
use super::multiseat::{self, MultiSeatMeasurements};
use super::multisub;
use super::output;
use super::types::{
    ChannelDspChain, MixedModeConfig, MultiMeasurementConfig, MultiSeatConfig, MultiSubGroup,
    OptimizerConfig, RoomConfig, SpeakerGroup,
};

// Type aliases from optimize module
pub(super) type MixedModeResult = (
    ChannelDspChain,
    f64,
    f64,
    Curve,
    Curve,
    Vec<Biquad>,
    f64,
    Option<f64>,
    Option<Vec<f64>>,
);

const GLOBAL_EQ_REGRESSION_TOLERANCE: f64 = 1e-6;

// Import helper functions from optimize and speaker_eq modules
use super::optimize::detect_passband_and_mean;
use super::speaker_eq::determine_optimization_bands;

pub(super) fn process_speaker_group(
    channel_name: &str,
    group: &SpeakerGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    // 1. Load all measurements in the group
    let mut driver_curves = Vec::new();
    for (i, source) in group.measurements.iter().enumerate() {
        let curve = load::load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!(
                "Failed to load driver {} measurement for channel {}: {}",
                i, channel_name, e
            ),
        })?;
        driver_curves.push(curve);
    }

    debug!("  Loaded {} driver measurements", driver_curves.len());

    // 2. Sort drivers by mean frequency (Low to High)
    driver_curves.sort_by(|a, b| {
        let get_mean = |c: &Curve| {
            let (passband, _) = detect_passband_and_mean(c);
            let (min_f, max_f) = passband.unwrap_or_else(|| {
                let min_f = c.freq.iter().copied().fold(f64::INFINITY, f64::min);
                let max_f = c.freq.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (min_f, max_f)
            });
            (min_f * max_f).sqrt()
        };
        get_mean(a)
            .partial_cmp(&get_mean(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 3. Get crossover config
    let crossover_config = if let Some(crossover_ref) = &group.crossover {
        room_config
            .crossovers
            .as_ref()
            .and_then(|xovers| xovers.get(crossover_ref))
            .ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: format!("Crossover configuration '{}' not found", crossover_ref),
            })?
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: "Speaker group requires crossover configuration".to_string(),
        });
    };

    // 4. Per-Driver Linearization (Pre-Correction)
    info!("  Linearizing {} drivers...", driver_curves.len());
    let optimization_bands =
        determine_optimization_bands(driver_curves.len(), room_config, crossover_config);
    let mut linearized_drivers = Vec::with_capacity(driver_curves.len());
    let mut per_driver_filters = Vec::with_capacity(driver_curves.len());

    for (i, curve) in driver_curves.iter().enumerate() {
        let (min_f, max_f) = optimization_bands[i];
        info!(
            "    Driver {}: optimizing bandwidth {:.1}-{:.1} Hz",
            i, min_f, max_f
        );

        // Create driver-specific config
        let mut driver_opt_config = room_config.optimizer.clone();
        driver_opt_config.min_freq = min_f;
        driver_opt_config.max_freq = max_f;

        // Optimize EQ for this driver
        let (filters, _) = eq::optimize_channel_eq(
            curve,
            &driver_opt_config,
            room_config.target_curve.as_ref(), // Use global target (usually flat)
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Linearization failed for driver {}: {}", i, e),
        })?;

        // Apply filters to get linearized curve
        let resp = response::compute_peq_complex_response(&filters, &curve.freq, sample_rate);
        let linear_curve = response::apply_complex_response(curve, &resp);

        linearized_drivers.push(linear_curve);
        per_driver_filters.push(filters);
    }

    // 5. Setup Crossover Optimization
    let crossover_type: crate::loss::CrossoverType = crossover_config
        .crossover_type
        .parse()
        .map_err(|e: String| AutoeqError::InvalidConfiguration { message: e })?;

    let fixed_freqs: Option<Vec<f64>> = if let Some(ref freqs) = crossover_config.frequencies {
        Some(freqs.clone())
    } else {
        crossover_config.frequency.map(|freq| vec![freq])
    };

    // 6. Compute pre-score (using linearized drivers)
    let n_drivers = linearized_drivers.len();
    let initial_gains = vec![0.0; n_drivers];
    let mut initial_xover_freqs = Vec::new();
    // Simple geometric mean estimate for initial guess
    for _ in 0..(n_drivers - 1) {
        let (min, max) = match crossover_config.frequency_range {
            Some((a, b)) => (a, b),
            None => (80.0, 3000.0),
        };
        initial_xover_freqs.push((min * max).sqrt());
    }

    let driver_measurements: Vec<crate::loss::DriverMeasurement> = linearized_drivers
        .iter()
        .map(|curve| crate::loss::DriverMeasurement {
            freq: curve.freq.clone(),
            spl: curve.spl.clone(),
            phase: curve.phase.clone(),
        })
        .collect();

    let initial_delays = vec![0.0; n_drivers];

    let drivers_data = crate::loss::DriversLossData::new(driver_measurements, crossover_type);
    let pre_score = crate::loss::drivers_flat_loss(
        &drivers_data,
        &initial_gains,
        &initial_xover_freqs,
        Some(&initial_delays),
        sample_rate,
        room_config.optimizer.min_freq,
        room_config.optimizer.max_freq,
    );

    // 7. Optimize Crossover (using linearized drivers)
    let (gains, delays, crossover_freqs, combined_curve, inversions) =
        crossover::optimize_crossover(
            linearized_drivers.clone(), // Use linearized curves!
            crossover_type,
            sample_rate,
            &room_config.optimizer,
            fixed_freqs,
            crossover_config.frequency_range,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Crossover optimization failed: {}", e),
        })?;

    info!(
        "  Optimized crossover: freqs={:?}, gains={:?}, delays={:?}, inversions={:?}",
        crossover_freqs, gains, delays, inversions
    );

    // 8. Global EQ (Optional Touch-up)
    // Run global EQ on the combined response to fix any remaining issues
    // but constrain it to be gentle if possible, or normal full optimization.
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let pre_global_eq_score = flat_loss_score(&combined_curve, min_freq, max_freq);

    let (global_eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "Global EQ optimization failed for channel {}: {}",
            channel_name, e
        ),
    })?;

    let (global_eq_filters, post_score, final_curve) =
        if eq_score_regressed(pre_global_eq_score, post_score) {
            warn!(
                "  Global EQ rejected for speaker group {}: flat loss {:.6} -> {:.6}",
                channel_name, pre_global_eq_score, post_score
            );
            (Vec::new(), pre_global_eq_score, combined_curve.clone())
        } else {
            info!("  Optimized {} Global EQ filters", global_eq_filters.len());
            info!(
                "  Pre-score: {:.6}, Post-score: {:.6}",
                pre_global_eq_score, post_score
            );
            let global_resp = response::compute_peq_complex_response(
                &global_eq_filters,
                &combined_curve.freq,
                sample_rate,
            );
            let final_curve = response::apply_complex_response(&combined_curve, &global_resp);
            (global_eq_filters, post_score, final_curve)
        };

    // 9. Build Output DSP Chain
    // We now have per-driver filters AND global filters.

    // Prepare display curves (raw drivers extended)
    let driver_curves_for_display: Vec<Curve> = driver_curves
        .iter()
        .map(output::extend_curve_to_full_range)
        .collect();

    let mut chain = output::build_multidriver_dsp_chain_with_curves(
        channel_name,
        &gains,
        &delays,
        Some(&inversions),
        &crossover_freqs,
        crossover_type.to_plugin_string(),
        &global_eq_filters,
        Some(&per_driver_filters), // Pass the per-driver EQ filters here!
        None,
        None,
        Some(&driver_curves_for_display),
    );

    // Detect passband
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Extend curves for display
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp = response::compute_peq_complex_response(
        &global_eq_filters,
        &display_initial.freq,
        sample_rate,
    );
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    // Use global mean for level alignment
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve.clone(),
        final_curve,
        global_eq_filters,
        mean_spl,
        None, // No single WAV for speaker groups
        None, // IIR-only for speaker groups
    ))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::MeasurementSource;
    use ndarray::array;
    use std::collections::HashMap;

    fn flat_curve_without_phase() -> Curve {
        Curve {
            freq: array![40.0, 80.0, 160.0],
            spl: array![80.0, 80.0, 80.0],
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn cardioid_rejects_missing_phase() {
        let cardioid = super::super::types::CardioidConfig {
            name: "card".to_string(),
            speaker_name: None,
            front: MeasurementSource::InMemory(flat_curve_without_phase()),
            rear: MeasurementSource::InMemory(flat_curve_without_phase()),
            separation_meters: 0.5,
        };
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let err =
            process_cardioid("LFE", &cardioid, &room_config, 48000.0, Path::new(".")).unwrap_err();
        assert!(
            err.to_string().contains("requires measured phase"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn flat_loss_score_zero_for_flat_curve() {
        let curve = Curve {
            freq: array![100.0, 200.0, 400.0, 800.0, 1600.0],
            spl: array![80.0, 80.0, 80.0, 80.0, 80.0],
            phase: None,
            ..Default::default()
        };
        let score = flat_loss_score(&curve, 100.0, 1600.0);
        assert!(
            score.abs() < 1e-6,
            "perfectly flat curve should have zero loss, got {score}"
        );
    }

    #[test]
    fn flat_loss_score_positive_for_uneven_curve() {
        let curve = Curve {
            freq: array![100.0, 200.0, 400.0, 800.0, 1600.0],
            spl: array![80.0, 85.0, 80.0, 75.0, 80.0],
            phase: None,
            ..Default::default()
        };
        let score = flat_loss_score(&curve, 100.0, 1600.0);
        assert!(
            score > 0.1,
            "uneven curve should have positive loss, got {score}"
        );
    }

    #[test]
    fn cardioid_flat_response_does_not_regress() {
        // Front and rear are identical flat curves with measured phase.
        // The cardioid sum will be flat-ish; global EQ should not regress it.
        let front = Curve {
            freq: array![100.0, 200.0, 400.0, 800.0],
            spl: array![80.0, 80.0, 80.0, 80.0],
            phase: Some(array![0.0, 0.0, 0.0, 0.0]),
            ..Default::default()
        };
        let rear = Curve {
            freq: array![100.0, 200.0, 400.0, 800.0],
            spl: array![80.0, 80.0, 80.0, 80.0],
            phase: Some(array![0.0, 0.0, 0.0, 0.0]),
            ..Default::default()
        };
        let cardioid = super::super::types::CardioidConfig {
            name: "card".to_string(),
            speaker_name: None,
            front: MeasurementSource::InMemory(front),
            rear: MeasurementSource::InMemory(rear),
            separation_meters: 0.5,
        };
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                min_freq: 100.0,
                max_freq: 800.0,
                num_filters: 1,
                max_iter: 10,
                population: 4,
                seed: Some(42),
                ..Default::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = process_cardioid("LFE", &cardioid, &room_config, 48000.0, Path::new("."));
        assert!(
            result.is_ok(),
            "Cardioid processing should succeed: {:?}",
            result
        );
        let (_chain, _pre, post, _initial, _final, _filters, _mean, _arrival, _fir) =
            result.unwrap();
        assert!(
            post.is_finite(),
            "post_score must be finite after regression guard"
        );
    }

    #[test]
    fn global_eq_regression_guard_rejects_worse_or_nonfinite_scores() {
        assert!(eq_score_regressed(1.0, 1.01));
        assert!(eq_score_regressed(1.0, f64::NAN));
        assert!(!eq_score_regressed(1.0, 1.0));
        assert!(!eq_score_regressed(
            1.0,
            1.0 + GLOBAL_EQ_REGRESSION_TOLERANCE
        ));
    }

    fn phased_sub_curve(spl_offset: f64, phase_offset: f64) -> Curve {
        let freq = array![20.0, 30.0, 45.0, 67.5, 100.0, 120.0];
        let spl = freq.mapv(|f| {
            let mode = if f < 60.0 { 3.0 } else { -1.0 };
            80.0 + spl_offset + mode
        });
        let phase = freq.mapv(|f| -180.0 * f / 100.0 + phase_offset);
        Curve {
            freq,
            spl,
            phase: Some(phase),
            ..Default::default()
        }
    }

    #[test]
    fn multisub_uses_production_multiseat_path_when_subs_have_seat_measurements() {
        let group = MultiSubGroup {
            name: "subs".to_string(),
            speaker_name: None,
            subwoofers: vec![
                MeasurementSource::InMemoryMultiple(vec![
                    phased_sub_curve(0.0, 0.0),
                    phased_sub_curve(2.0, 12.0),
                ]),
                MeasurementSource::InMemoryMultiple(vec![
                    phased_sub_curve(-1.0, 45.0),
                    phased_sub_curve(1.0, 60.0),
                ]),
            ],
            allpass_optimization: false,
        };
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                min_freq: 20.0,
                max_freq: 120.0,
                num_filters: 1,
                max_iter: 3,
                population: 4,
                seed: Some(7),
                refine: false,
                multi_seat: Some(MultiSeatConfig {
                    enabled: true,
                    per_sub_peq: false,
                    global_eq: false,
                    ..Default::default()
                }),
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let (chain, pre_score, post_score, _initial, _final, filters, _mean, _arrival, _fir) =
            process_multisub_group("LFE", &group, &room_config, 48000.0, Path::new("."))
                .expect("multi-seat multi-sub processing should succeed");

        assert!(pre_score.is_finite());
        assert!(post_score.is_finite());
        assert_ne!(
            pre_score, post_score,
            "pre/post scores should include the production MSO stage, not only global EQ"
        );
        assert!(
            filters.is_empty(),
            "global_eq=false should not emit shared EQ"
        );
        assert!(chain.plugins.is_empty());
        let drivers = chain.drivers.expect("multi-sub output should have drivers");
        assert_eq!(drivers.len(), 2);
    }

    #[test]
    fn average_power_curve_preserves_phase_when_all_inputs_have_phase() {
        let c1 = Curve {
            freq: array![100.0, 200.0, 400.0],
            spl: array![80.0, 80.0, 80.0],
            phase: Some(array![0.0, 45.0, 90.0]),
            ..Default::default()
        };
        let c2 = Curve {
            freq: array![100.0, 200.0, 400.0],
            spl: array![80.0, 80.0, 80.0],
            phase: Some(array![0.0, 45.0, 90.0]),
            ..Default::default()
        };
        let avg = average_power_curve(&[c1, c2]).unwrap();
        assert!(
            avg.phase.is_some(),
            "average_power_curve should preserve phase when all inputs have phase"
        );
        let phase = avg.phase.unwrap();
        // Same-phase curves should average to the same phase
        assert!(
            (phase[0]).abs() < 1.0,
            "same 0° phase should average to ~0°, got {}",
            phase[0]
        );
        assert!(
            (phase[1] - 45.0).abs() < 1.0,
            "same 45° phase should average to ~45°, got {}",
            phase[1]
        );
        assert!(
            (phase[2] - 90.0).abs() < 1.0,
            "same 90° phase should average to ~90°, got {}",
            phase[2]
        );
    }

    #[test]
    fn average_power_curve_returns_none_phase_when_any_input_lacks_phase() {
        let c1 = Curve {
            freq: array![100.0, 200.0],
            spl: array![80.0, 80.0],
            phase: Some(array![0.0, 0.0]),
            ..Default::default()
        };
        let c2 = Curve {
            freq: array![100.0, 200.0],
            spl: array![80.0, 80.0],
            phase: None,
            ..Default::default()
        };
        let avg = average_power_curve(&[c1, c2]).unwrap();
        assert!(
            avg.phase.is_none(),
            "average_power_curve should return None phase when any input lacks phase"
        );
    }

    #[test]
    fn average_power_curve_vector_averages_opposing_phases() {
        let c1 = Curve {
            freq: array![100.0],
            spl: array![80.0],
            phase: Some(array![0.0]),
            ..Default::default()
        };
        let c2 = Curve {
            freq: array![100.0],
            spl: array![80.0],
            phase: Some(array![180.0]),
            ..Default::default()
        };
        let avg = average_power_curve(&[c1, c2]).unwrap();
        let phase = avg.phase.expect("phase should be present");
        // 0° and 180° perfectly cancel; atan2(0,0) returns 0.0
        assert!(
            phase[0].abs() < 1.0,
            "opposing phases should have mean angle ~0° (or undefined), got {}",
            phase[0]
        );
        // Magnitude should reflect cancellation: pressure average of 1 and -1 is 0
        let expected_power = 10.0 * ((10.0_f64.powf(8.0) + 10.0_f64.powf(8.0)) / 2.0).log10();
        // With complex averaging, magnitude will be much lower due to cancellation
        // The key thing is phase is preserved, not the exact SPL value
        assert!(
            avg.spl[0] < expected_power,
            "cancelled phases should reduce SPL magnitude"
        );
    }

    #[test]
    fn production_multiseat_path_emits_per_sub_and_global_eq_when_enabled() {
        let group = MultiSubGroup {
            name: "subs".to_string(),
            speaker_name: None,
            subwoofers: vec![
                MeasurementSource::InMemoryMultiple(vec![
                    phased_sub_curve(0.0, 0.0),
                    phased_sub_curve(2.0, 12.0),
                ]),
                MeasurementSource::InMemoryMultiple(vec![
                    phased_sub_curve(-1.0, 45.0),
                    phased_sub_curve(1.0, 60.0),
                ]),
            ],
            allpass_optimization: false,
        };
        let room_config = RoomConfig {
            version: super::super::types::default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                min_freq: 20.0,
                max_freq: 120.0,
                num_filters: 1,
                max_iter: 3,
                population: 4,
                seed: Some(11),
                refine: false,
                multi_seat: Some(MultiSeatConfig {
                    enabled: true,
                    per_sub_peq: true,
                    global_eq: true,
                    ..Default::default()
                }),
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let (chain, pre_score, post_score, _initial, _final, filters, _mean, _arrival, _fir) =
            process_multisub_group("LFE", &group, &room_config, 48000.0, Path::new("."))
                .expect("multi-seat multi-sub processing should succeed");

        assert!(pre_score.is_finite());
        assert!(post_score.is_finite());
        let has_global_eq = chain
            .plugins
            .iter()
            .any(|plugin| plugin.plugin_type == "eq");
        assert_eq!(
            has_global_eq,
            !filters.is_empty(),
            "shared EQ filters and exported channel EQ plugin should stay in sync"
        );
        let drivers = chain.drivers.expect("multi-sub output should have drivers");
        assert_eq!(drivers.len(), 2);
        assert!(
            drivers.iter().all(|driver| driver
                .plugins
                .iter()
                .any(|plugin| plugin.plugin_type == "eq")),
            "per_sub_peq=true should export per-driver EQ plugins"
        );
    }
}

/// Process multi-subwoofer group
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
pub(super) fn process_multisub_group(
    channel_name: &str,
    group: &MultiSubGroup,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    if let Some(multi_seat_config) = room_config
        .optimizer
        .multi_seat
        .as_ref()
        .filter(|config| config.enabled)
    {
        match load_multisub_seat_measurements(group)? {
            Some(seat_measurements) => {
                return process_multisub_group_multiseat(
                    channel_name,
                    group,
                    room_config,
                    multi_seat_config,
                    sample_rate,
                    seat_measurements,
                );
            }
            None => warn!(
                "  Multi-seat optimization is enabled for multi-sub group '{}' but subwoofer sources do not contain at least two seat measurements each; using single-seat multi-sub path",
                group.name
            ),
        }
    }

    let (result, combined_curve, allpass_filters) = if group.allpass_optimization {
        // All-pass enhanced optimization
        info!("  Using all-pass enhanced multi-sub optimization");
        let ap_result = multisub::optimize_multisub_with_allpass(
            &group.subwoofers,
            &room_config.optimizer,
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Multi-sub all-pass optimization failed: {}", e),
        })?;

        for (i, (freq, q)) in ap_result.allpass_filters.iter().enumerate() {
            info!(
                "  Sub {}: gain={:.1} dB, delay={:.1} ms, all-pass: {:.0} Hz Q={:.2}",
                i, ap_result.base.gains[i], ap_result.base.delays[i], freq, q
            );
        }

        (
            ap_result.base,
            ap_result.combined_curve,
            Some(ap_result.allpass_filters),
        )
    } else {
        // Standard gain + delay optimization
        let (result, curve) =
            multisub::optimize_multisub(&group.subwoofers, &room_config.optimizer, sample_rate)
                .map_err(|e| AutoeqError::OptimizationFailed {
                    message: format!("Multi-sub optimization failed: {}", e),
                })?;
        (result, curve, None)
    };

    info!(
        "  Multi-sub optimization: gains={:?}, delays={:?} ms",
        result.gains, result.delays
    );

    let multisub_eq_optimizer = eq::resolve_multi_measurement_auto_optimizer_config(
        std::slice::from_ref(&combined_curve),
        &room_config.optimizer,
        eq::MultiEqAutoOptimizerContext::sub_channel(),
    );
    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &multisub_eq_optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for multi-sub sum: {}", e),
    })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    // Load individual sub curves for per-driver display
    let driver_curves_for_display: Vec<Curve> = group
        .subwoofers
        .iter()
        .filter_map(|source| {
            load::load_source(source)
                .ok()
                .map(|c| output::extend_curve_to_full_range(&c))
        })
        .collect();
    let driver_display_ref = if driver_curves_for_display.len() == group.subwoofers.len() {
        Some(driver_curves_for_display.as_slice())
    } else {
        None
    };

    let mut chain = output::build_multisub_dsp_chain_with_allpass(
        channel_name,
        &group.name,
        group.subwoofers.len(),
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
        driver_display_ref,
        allpass_filters.as_deref(),
        sample_rate,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization (used for display curves)
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Level alignment: use mean SPL within the EQ optimization range
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for multi-sub groups
        None, // IIR-only for multi-sub groups
    ))
}

fn load_multisub_seat_measurements(group: &MultiSubGroup) -> Result<Option<Vec<Vec<Curve>>>> {
    let mut per_sub = Vec::with_capacity(group.subwoofers.len());
    let mut expected_seats = None;
    let mut any_multi_seat = false;

    for (sub_idx, source) in group.subwoofers.iter().enumerate() {
        let curves =
            load::load_source_individual(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: format!(
                    "Failed to load seat measurements for sub {} in group '{}': {}",
                    sub_idx, group.name, e
                ),
            })?;
        if curves.len() > 1 {
            any_multi_seat = true;
        }
        match expected_seats {
            Some(expected) if curves.len() != expected => {
                return Err(AutoeqError::InvalidConfiguration {
                    message: format!(
                        "Multi-seat multi-sub group '{}' has inconsistent seat counts: sub 0 has {}, sub {} has {}",
                        group.name,
                        expected,
                        sub_idx,
                        curves.len()
                    ),
                });
            }
            None => expected_seats = Some(curves.len()),
            _ => {}
        }
        per_sub.push(curves);
    }

    if any_multi_seat && expected_seats.unwrap_or(0) >= 2 {
        Ok(Some(per_sub))
    } else {
        Ok(None)
    }
}

fn multiseat_peq_config(policy: &MultiSeatConfig, seat_count: usize) -> MultiMeasurementConfig {
    let mut weights = match policy.seat_weights.as_ref() {
        Some(weights) if weights.len() == seat_count => weights.clone(),
        _ => vec![1.0; seat_count],
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
    let weight_sum: f64 = weights.iter().sum();
    if weight_sum <= f64::EPSILON {
        weights = vec![1.0 / seat_count.max(1) as f64; seat_count];
    } else {
        for weight in &mut weights {
            *weight /= weight_sum;
        }
    }

    MultiMeasurementConfig {
        strategy: policy.all_channel_strategy.clone(),
        weights: Some(weights),
        variance_lambda: 1.0,
        spatial_robustness: Some(super::types::SpatialRobustnessSerdeConfig {
            variance_threshold_db: 3.0,
            transition_width_db: 2.0,
            min_correction_depth: 0.1,
            mask_smoothing_octaves: 1.0 / 6.0,
        }),
        bootstrap_uncertainty: None,
    }
}

fn apply_per_sub_filters(
    seat_measurements: &[Vec<Curve>],
    per_sub_filters: &[Vec<Biquad>],
    sample_rate: f64,
) -> Vec<Vec<Curve>> {
    seat_measurements
        .iter()
        .zip(per_sub_filters.iter())
        .map(|(sub_curves, filters)| {
            sub_curves
                .iter()
                .map(|curve| {
                    if filters.is_empty() {
                        curve.clone()
                    } else {
                        let resp = response::compute_peq_complex_response(
                            filters,
                            &curve.freq,
                            sample_rate,
                        );
                        response::apply_complex_response(curve, &resp)
                    }
                })
                .collect()
        })
        .collect()
}

fn average_power_curve(curves: &[Curve]) -> Result<Curve> {
    let Some(first) = curves.first() else {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Cannot average an empty multi-seat curve set".to_string(),
        });
    };
    let all_have_phase = curves.iter().all(|c| c.phase.is_some());
    for (idx, curve) in curves.iter().enumerate() {
        if curve.freq.len() != first.freq.len()
            || curve
                .freq
                .iter()
                .zip(first.freq.iter())
                .any(|(a, b)| (a - b).abs() > 1e-6 * b.abs().max(1.0))
        {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "Cannot average multi-seat curves because seat {} has a different frequency grid",
                    idx
                ),
            });
        }
    }

    if all_have_phase {
        // Complex vector average: preserves phase when all inputs have it
        use num_complex::Complex64;
        let n = curves.len() as f64;
        let mut sum_re = Array1::<f64>::zeros(first.freq.len());
        let mut sum_im = Array1::<f64>::zeros(first.freq.len());
        for curve in curves {
            let phase = curve.phase.as_ref().unwrap();
            for i in 0..first.freq.len() {
                let pressure = 10.0_f64.powf(curve.spl[i] / 20.0);
                let rad = phase[i].to_radians();
                sum_re[i] += pressure * rad.cos();
                sum_im[i] += pressure * rad.sin();
            }
        }
        let mut spl = Array1::<f64>::zeros(first.freq.len());
        let mut phase_out = Array1::<f64>::zeros(first.freq.len());
        for i in 0..first.freq.len() {
            let z = Complex64::new(sum_re[i] / n, sum_im[i] / n);
            let mag = z.norm();
            spl[i] = 20.0 * mag.max(1e-12).log10();
            phase_out[i] = if mag > 1e-12 {
                z.arg().to_degrees()
            } else {
                0.0
            };
        }
        Ok(Curve {
            freq: first.freq.clone(),
            spl,
            phase: Some(phase_out),
            ..Default::default()
        })
    } else {
        // Power (energy) average when phase is unavailable or mixed
        let mut power_sum = Array1::<f64>::zeros(first.freq.len());
        for curve in curves {
            power_sum = power_sum + curve.spl.mapv(|spl| 10.0_f64.powf(spl / 10.0));
        }
        let avg_power = power_sum / curves.len() as f64;
        Ok(Curve {
            freq: first.freq.clone(),
            spl: avg_power.mapv(|power| 10.0 * power.max(1e-12).log10()),
            phase: None,
            ..Default::default()
        })
    }
}

fn flat_loss_score(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
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

fn eq_score_regressed(pre_score: f64, post_score: f64) -> bool {
    !post_score.is_finite()
        || (pre_score.is_finite() && post_score > pre_score + GLOBAL_EQ_REGRESSION_TOLERANCE)
}

fn identity_multiseat_result(
    measurements: &MultiSeatMeasurements,
    policy: &MultiSeatConfig,
) -> multiseat::MultiSeatOptimizationResult {
    multiseat::MultiSeatOptimizationResult {
        gains: vec![0.0; measurements.num_subs],
        delays: vec![0.0; measurements.num_subs],
        polarities: vec![false; measurements.num_subs],
        allpass_filters: vec![Vec::new(); measurements.num_subs],
        strategy: policy.strategy.clone(),
        objective_name: "identity".to_string(),
        objective_before: 0.0,
        objective_after: 0.0,
        objective_improvement_db: 0.0,
        variance_before: 0.0,
        variance_after: 0.0,
        variance_improvement_db: 0.0,
        improvement_db: 0.0,
    }
}

fn process_multisub_group_multiseat(
    channel_name: &str,
    group: &MultiSubGroup,
    room_config: &RoomConfig,
    multi_seat_config: &MultiSeatConfig,
    sample_rate: f64,
    seat_measurements: Vec<Vec<Curve>>,
) -> Result<MixedModeResult> {
    let seat_count = seat_measurements.first().map(Vec::len).unwrap_or_default();
    let peq_config = multiseat_peq_config(multi_seat_config, seat_count);
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;

    let raw_measurements = MultiSeatMeasurements::new(seat_measurements.clone())?;
    let raw_identity = identity_multiseat_result(&raw_measurements, multi_seat_config);
    let raw_seat_curves = multiseat::compute_multiseat_combined_curves(
        &raw_measurements,
        &raw_identity,
        (min_freq, max_freq),
        sample_rate,
    )?;
    let raw_combined_curve = average_power_curve(&raw_seat_curves)?;
    let pre_score = flat_loss_score(&raw_combined_curve, min_freq, max_freq);

    let per_sub_filters = if multi_seat_config.per_sub_peq {
        let mut filters = Vec::with_capacity(seat_measurements.len());
        for (sub_idx, sub_curves) in seat_measurements.iter().enumerate() {
            info!(
                "  Multi-seat per-sub PEQ: optimizing sub {} across {} seats ({:?})",
                sub_idx, seat_count, peq_config.strategy
            );
            let (sub_filters, sub_loss) = eq::optimize_channel_eq_multi_with_auto_optimizer(
                sub_curves,
                &room_config.optimizer,
                &peq_config,
                None,
                sample_rate,
                eq::MultiEqAutoOptimizerContext::sub_channel(),
            )
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: format!("Per-sub multi-seat PEQ failed for sub {}: {}", sub_idx, e),
            })?;
            info!(
                "  Sub {} per-seat PEQ: {} filters, loss={:.6}",
                sub_idx,
                sub_filters.len(),
                sub_loss
            );
            filters.push(sub_filters);
        }
        filters
    } else {
        vec![Vec::new(); seat_measurements.len()]
    };

    let corrected_measurements =
        apply_per_sub_filters(&seat_measurements, &per_sub_filters, sample_rate);
    let measurements = MultiSeatMeasurements::new(corrected_measurements)?;
    let mso_result = multiseat::optimize_multiseat(
        &measurements,
        multi_seat_config,
        (
            room_config.optimizer.min_freq,
            room_config.optimizer.max_freq,
        ),
        sample_rate,
    )?;
    info!(
        "  Multi-seat multi-sub optimization: gains={:?}, delays={:?} ms, polarities={:?}",
        mso_result.gains, mso_result.delays, mso_result.polarities
    );

    for (sub_idx, filters) in mso_result.allpass_filters.iter().enumerate() {
        for (filter_idx, (freq, q)) in filters.iter().enumerate() {
            info!(
                "  Sub {} all-pass {}: {:.0} Hz Q={:.2}",
                sub_idx, filter_idx, freq, q
            );
        }
    }

    let combined_seat_curves = multiseat::compute_multiseat_combined_curves(
        &measurements,
        &mso_result,
        (
            room_config.optimizer.min_freq,
            room_config.optimizer.max_freq,
        ),
        sample_rate,
    )?;
    let combined_curve = average_power_curve(&combined_seat_curves)?;

    let mut eq_filters = if multi_seat_config.global_eq {
        let (filters, loss) = eq::optimize_channel_eq_multi_with_auto_optimizer(
            &combined_seat_curves,
            &room_config.optimizer,
            &peq_config,
            room_config.target_curve.as_ref(),
            sample_rate,
            eq::MultiEqAutoOptimizerContext::sub_channel(),
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("Global multi-seat EQ failed for multi-sub sum: {}", e),
        })?;
        info!(
            "  Global multi-seat EQ: {} filters, score={:.6}",
            filters.len(),
            loss
        );
        filters
    } else {
        Vec::new()
    };

    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    let global_eq_pre_score = flat_loss_score(&combined_curve, min_freq, max_freq);
    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let mut final_curve = response::apply_complex_response(&combined_curve, &iir_resp);
    let mut post_score = flat_loss_score(&final_curve, min_freq, max_freq);
    if multi_seat_config.global_eq && eq_score_regressed(global_eq_pre_score, post_score) {
        warn!(
            "  Global multi-seat EQ rejected for multi-sub sum: flat loss {:.6} -> {:.6}",
            global_eq_pre_score, post_score
        );
        eq_filters.clear();
        final_curve = combined_curve.clone();
        post_score = global_eq_pre_score;
    }
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    let driver_curves_for_display: Vec<Curve> = group
        .subwoofers
        .iter()
        .filter_map(|source| {
            load::load_source(source)
                .ok()
                .map(|c| output::extend_curve_to_full_range(&c))
        })
        .collect();
    let driver_display_ref = if driver_curves_for_display.len() == group.subwoofers.len() {
        Some(driver_curves_for_display.as_slice())
    } else {
        None
    };

    let mut chain = output::build_multisub_dsp_chain_advanced(
        channel_name,
        &group.name,
        group.subwoofers.len(),
        &mso_result.gains,
        &mso_result.delays,
        &eq_filters,
        None,
        None,
        driver_display_ref,
        Some(&per_sub_filters),
        Some(&mso_result.polarities),
        Some(&mso_result.allpass_filters),
        sample_rate,
    );

    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        pre_score,
        post_score,
        raw_combined_curve,
        final_curve,
        eq_filters,
        mean_spl,
        None,
        None,
    ))
}

/// Process DBA configuration
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
pub(super) fn process_dba(
    channel_name: &str,
    dba_config: &super::types::DBAConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    let (result, combined_curve) =
        dba::optimize_dba(dba_config, &room_config.optimizer, sample_rate).map_err(|e| {
            AutoeqError::OptimizationFailed {
                message: format!("DBA optimization failed: {}", e),
            }
        })?;

    info!(
        "  DBA Optimization: Front Gain={:.2}dB, Rear Gain={:.2}dB, Rear Delay={:.2}ms",
        result.gains[0], result.gains[1], result.delays[1]
    );

    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for DBA sum: {}", e),
    })?;

    info!(
        "  Global EQ: {} filters, score={:.6}",
        eq_filters.len(),
        post_score
    );

    // Load front/rear array curves for per-driver display
    // DBA has 2 "drivers": front aggregate and rear aggregate
    let driver_display_ref = match (
        dba::sum_array_response(&dba_config.front),
        dba::sum_array_response(&dba_config.rear),
    ) {
        (Ok(front), Ok(rear)) => Some(vec![
            output::extend_curve_to_full_range(&front),
            output::extend_curve_to_full_range(&rear),
        ]),
        _ => None,
    };
    let driver_display_slice = driver_display_ref.as_deref();

    let mut chain = output::build_dba_dsp_chain_with_curves(
        channel_name,
        &result.gains,
        &result.delays,
        &eq_filters,
        None,
        None,
        driver_display_slice,
    );

    let iir_resp =
        response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
    let final_curve = response::apply_complex_response(&combined_curve, &iir_resp);

    // Detect passband for normalization (used for display curves)
    let (norm_range, _passband_mean) = detect_passband_and_mean(&combined_curve);

    // Level alignment: use mean SPL within the EQ optimization range
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        result.pre_objective,
        post_score,
        combined_curve.clone(),
        final_curve,
        eq_filters,
        mean_spl,
        None, // No single WAV for DBA
        None, // IIR-only for DBA
    ))
}

// ============================================================================
// Frequency-Based Mixed Mode Processing
// ============================================================================

/// Process mixed mode with frequency-based crossover
///
/// This mode applies FIR correction to one frequency band (default: low frequencies)
/// and IIR correction to the other band (default: high frequencies), separated by
/// a configurable crossover frequency.
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
#[allow(clippy::too_many_arguments)]
pub(super) fn process_mixed_mode_crossover(
    channel_name: &str,
    curve: &Curve,
    room_config: &RoomConfig,
    mixed_config: &MixedModeConfig,
    sample_rate: f64,
    output_dir: &Path,
    min_freq: f64,
    max_freq: f64,
    mean: f64,
    pre_score: f64,
    arrival_time_ms: Option<f64>,
    callback: Option<crate::optim::OptimProgressCallback>,
) -> Result<MixedModeResult> {
    let crossover_freq = mixed_config.crossover_freq;
    let fir_uses_low = mixed_config.fir_band.to_lowercase() == "low";

    info!(
        "  Mixed mode crossover at {} Hz (FIR on {} band, IIR on {} band)",
        crossover_freq,
        if fir_uses_low { "low" } else { "high" },
        if fir_uses_low { "high" } else { "low" }
    );

    // Split the curve at crossover frequency
    let (low_curve, high_curve) = split_curve_at_frequency(curve, crossover_freq);

    // Determine which curve gets FIR and which gets IIR
    let (fir_curve, iir_curve) = if fir_uses_low {
        (&low_curve, &high_curve)
    } else {
        (&high_curve, &low_curve)
    };

    // Create band-specific optimizer configs with appropriate frequency ranges
    let fir_min_freq = fir_curve.freq.first().copied().unwrap_or(min_freq);
    let fir_max_freq = fir_curve.freq.last().copied().unwrap_or(crossover_freq);
    let iir_min_freq = iir_curve.freq.first().copied().unwrap_or(crossover_freq);
    let iir_max_freq = iir_curve.freq.last().copied().unwrap_or(max_freq);

    info!(
        "  FIR band: {:.1}-{:.1} Hz, IIR band: {:.1}-{:.1} Hz",
        fir_min_freq, fir_max_freq, iir_min_freq, iir_max_freq
    );

    // Optimize IIR on designated band
    let iir_config = OptimizerConfig {
        min_freq: iir_min_freq,
        max_freq: iir_max_freq,
        ..room_config.optimizer.clone()
    };

    let (eq_filters, _) = if let Some(cb) = callback {
        eq::optimize_channel_eq_with_callback(
            iir_curve,
            &iir_config,
            room_config.target_curve.as_ref(),
            sample_rate,
            cb,
        )
    } else {
        eq::optimize_channel_eq(
            iir_curve,
            &iir_config,
            room_config.target_curve.as_ref(),
            sample_rate,
        )
    }
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "IIR optimization failed for {} band: {}",
            if fir_uses_low { "high" } else { "low" },
            e
        ),
    })?;

    info!(
        "  IIR stage: {} filters for {} band",
        eq_filters.len(),
        if fir_uses_low { "high" } else { "low" }
    );

    // Optimize FIR on designated band
    let fir_config = OptimizerConfig {
        min_freq: fir_min_freq,
        max_freq: fir_max_freq,
        ..room_config.optimizer.clone()
    };

    let fir_coeffs = fir::generate_fir_correction(
        fir_curve,
        &fir_config,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!(
            "FIR generation failed for {} band: {}",
            if fir_uses_low { "low" } else { "high" },
            e
        ),
    })?;

    // Save FIR to WAV
    let (fir_filename, wav_path) = artifacts::reserve_convolution_artifact_path(
        output_dir,
        channel_name,
        ConvolutionArtifactKind::BandFir,
        sample_rate,
    );
    crate::fir::save_fir_to_wav(&fir_coeffs, sample_rate as u32, &wav_path).map_err(|e| {
        AutoeqError::OptimizationFailed {
            message: format!("Failed to save FIR WAV: {}", e),
        }
    })?;

    info!("  Saved FIR filter to {}", wav_path.display());

    // Build DSP chain with band split/merge
    let mut chain = output::build_mixed_mode_crossover_chain(
        channel_name,
        mixed_config,
        &eq_filters,
        &fir_filename,
        fir_uses_low,
        None,
    );

    // Compute combined response for scoring
    // For proper scoring, we need to simulate what the full chain does:
    // - Split into bands at crossover
    // - Apply FIR to one band, IIR to the other
    // - Sum bands back together
    let iir_resp = response::compute_peq_complex_response(&eq_filters, &curve.freq, sample_rate);
    let fir_resp = response::compute_fir_complex_response(&fir_coeffs, &curve.freq, sample_rate);

    // Compute crossover filter responses (LR24 = 4th order Butterworth)
    let (lp_resp, hp_resp) =
        compute_lr24_crossover_responses(&curve.freq, crossover_freq, sample_rate);

    // Combine responses: low_band * fir_or_iir + high_band * iir_or_fir
    let combined_resp: Vec<num_complex::Complex<f64>> = curve
        .freq
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if fir_uses_low {
                lp_resp[i] * fir_resp[i] + hp_resp[i] * iir_resp[i]
            } else {
                lp_resp[i] * iir_resp[i] + hp_resp[i] * fir_resp[i]
            }
        })
        .collect();

    let final_curve = response::apply_complex_response(curve, &combined_resp);

    // Detect passband for normalization
    let (norm_range, mean_final) = detect_passband_and_mean(&final_curve);

    // Compute post-score
    let normalized_final_spl = &final_curve.spl - mean_final;
    let post_score =
        crate::loss::flat_loss(&final_curve.freq, &normalized_final_spl, min_freq, max_freq);

    info!(
        "  Pre-score: {:.6}, Post-score: {:.6}",
        pre_score, post_score
    );

    // Extend curves to 20 Hz – 20 kHz for display output
    let display_initial = output::extend_curve_to_full_range(curve);
    let display_iir_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_fir_resp =
        response::compute_fir_complex_response(&fir_coeffs, &display_initial.freq, sample_rate);
    let (display_lp, display_hp) =
        compute_lr24_crossover_responses(&display_initial.freq, crossover_freq, sample_rate);
    let display_combined: Vec<num_complex::Complex<f64>> = display_initial
        .freq
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if fir_uses_low {
                display_lp[i] * display_fir_resp[i] + display_hp[i] * display_iir_resp[i]
            } else {
                display_lp[i] * display_iir_resp[i] + display_hp[i] * display_fir_resp[i]
            }
        })
        .collect();
    let display_final = response::apply_complex_response(&display_initial, &display_combined);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    Ok((
        chain,
        pre_score,
        post_score,
        curve.clone(),
        final_curve,
        eq_filters,
        mean,
        arrival_time_ms,
        Some(fir_coeffs),
    ))
}

/// Split a frequency response curve at a specified frequency
fn split_curve_at_frequency(curve: &Curve, crossover_freq: f64) -> (Curve, Curve) {
    // Find the index where frequency exceeds crossover
    let split_idx = curve
        .freq
        .iter()
        .position(|&f| f >= crossover_freq)
        .unwrap_or(curve.freq.len());

    // Include some overlap around crossover for better optimization
    let overlap_points = 3; // Include a few points on each side
    let low_end = (split_idx + overlap_points).min(curve.freq.len());
    let high_start = split_idx.saturating_sub(overlap_points);

    let low_curve = Curve {
        freq: curve.freq.slice(ndarray::s![..low_end]).to_owned(),
        spl: curve.spl.slice(ndarray::s![..low_end]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![..low_end]).to_owned()),
        ..Default::default()
    };

    let high_curve = Curve {
        freq: curve.freq.slice(ndarray::s![high_start..]).to_owned(),
        spl: curve.spl.slice(ndarray::s![high_start..]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![high_start..]).to_owned()),
        ..Default::default()
    };

    (low_curve, high_curve)
}

/// Compute Linkwitz-Riley 24dB/oct crossover filter responses
///
/// Returns (lowpass_response, highpass_response) as complex vectors
///
/// LR24 consists of two cascaded 2nd-order Butterworth filters.
/// This implementation computes the actual complex response including phase,
/// which is critical for accurate band summation in hybrid mode.
fn compute_lr24_crossover_responses(
    frequencies: &ndarray::Array1<f64>,
    crossover_freq: f64,
    sample_rate: f64,
) -> (
    Vec<num_complex::Complex<f64>>,
    Vec<num_complex::Complex<f64>>,
) {
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    // LR24 = two cascaded Butterworth LP2 filters (Q = 0.7071 each)
    // For LR24 lowpass: two 2nd-order Butterworth lowpass filters in series
    // For LR24 highpass: two 2nd-order Butterworth highpass filters in series

    let q = std::f64::consts::FRAC_1_SQRT_2; // Q = 0.7071 for Butterworth

    // Create biquad filters for lowpass (2 cascaded)
    let lp1 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let lp2 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    // Create biquad filters for highpass (2 cascaded)
    let hp1 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let hp2 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    let mut lp_resp = Vec::with_capacity(frequencies.len());
    let mut hp_resp = Vec::with_capacity(frequencies.len());

    for &freq in frequencies.iter() {
        // Compute cascaded response: H_lp = H_lp1 * H_lp2
        let lp1_resp = lp1.complex_response(freq);
        let lp2_resp = lp2.complex_response(freq);
        let lp_total = lp1_resp * lp2_resp;

        // Compute cascaded response: H_hp = H_hp1 * H_hp2
        let hp1_resp = hp1.complex_response(freq);
        let hp2_resp = hp2.complex_response(freq);
        let hp_total = hp1_resp * hp2_resp;

        lp_resp.push(lp_total);
        hp_resp.push(hp_total);
    }

    (lp_resp, hp_resp)
}

/// Perform consistency checks between speakers in the same Acoustic Group
#[allow(dead_code)]
pub(super) fn check_group_consistency(
    group_name: &str,
    channels: &[String],
    channel_means: &HashMap<String, f64>,
    curves: &HashMap<String, Curve>,
) {
    if channels.len() < 2 {
        return;
    }

    // 1. Range Difference Check (3 dB threshold)
    let mut means = Vec::new();
    for ch in channels {
        if let Some(&mean) = channel_means.get(ch) {
            means.push((ch, mean));
        }
    }

    for i in 0..means.len() {
        for j in i + 1..means.len() {
            let (ch1, m1) = means[i];
            let (ch2, m2) = means[j];
            let diff = (m1 - m2).abs();
            if diff > 3.0 {
                warn!(
                    "Speaker group '{}' has significant difference: range SPL between '{}' and '{}' is {:.1} dB (> 3.0 dB threshold).",
                    group_name, ch1, ch2, diff
                );
            }
        }
    }

    // 2. Octave-Wise Difference Check (6 dB threshold)
    // Compare all pairs in the group
    for i in 0..channels.len() {
        for j in i + 1..channels.len() {
            let ch1 = &channels[i];
            let ch2 = &channels[j];
            if let (Some(curve1), Some(curve2)) = (curves.get(ch1), curves.get(ch2)) {
                check_octave_consistency(group_name, ch1, ch2, curve1, curve2);
            }
        }
    }
}

/// Check if two curves are consistent across all octaves (6 dB threshold)
#[allow(dead_code)]
pub(super) fn check_octave_consistency(
    group_name: &str,
    ch1: &str,
    ch2: &str,
    curve1: &Curve,
    curve2: &Curve,
) {
    // Standard acoustic octaves
    let octave_centers = [
        31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ];

    for &center in &octave_centers {
        let f_min = center / 2.0_f64.sqrt();
        let f_max = center * 2.0_f64.sqrt();

        // Find overlap range
        let start_freq = f_min.max(curve1.freq[0]).max(curve2.freq[0]);
        let end_freq = f_max
            .min(curve1.freq[curve1.freq.len() - 1])
            .min(curve2.freq[curve2.freq.len() - 1]);

        if end_freq <= start_freq * 1.1 {
            continue; // Not enough bandwidth in this octave for comparison
        }

        // Compute average SPL for this octave in both curves
        let freqs1_f32: Vec<f32> = curve1.freq.iter().map(|&f| f as f32).collect();
        let spl1_f32: Vec<f32> = curve1.spl.iter().map(|&s| s as f32).collect();
        let freqs2_f32: Vec<f32> = curve2.freq.iter().map(|&f| f as f32).collect();
        let spl2_f32: Vec<f32> = curve2.spl.iter().map(|&s| s as f32).collect();

        let range = Some((start_freq as f32, end_freq as f32));
        let avg1 = compute_average_response(&freqs1_f32, &spl1_f32, range);
        let avg2 = compute_average_response(&freqs2_f32, &spl2_f32, range);

        let diff = (avg1 - avg2).abs() as f64;
        if diff > 6.0 {
            warn!(
                "Speaker group '{}' has significant difference: octave around {:.0} Hz between '{}' and '{}' differs by {:.1} dB (> 6.0 dB threshold).",
                group_name, center, ch1, ch2, diff
            );
        }
    }
}
/// Process Gradient Cardioid configuration
///
/// Returns: (DSP chain, pre_score, post_score, initial_curve, final_curve, biquads, mean_spl, arrival_time_ms)
pub(super) fn process_cardioid(
    channel_name: &str,
    config: &super::types::CardioidConfig,
    room_config: &RoomConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<MixedModeResult> {
    // 1. Load measurements
    let front_curve =
        load::load_source(&config.front).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("Failed to load Front measurement: {}", e),
        })?;
    let rear_curve =
        load::load_source(&config.rear).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("Failed to load Rear measurement: {}", e),
        })?;
    if front_curve.phase.is_none() || rear_curve.phase.is_none() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Cardioid processing requires measured phase for front and rear drivers"
                .to_string(),
        });
    }
    let rear_curve =
        if super::frequency_grid::same_frequency_grid(&front_curve.freq, &rear_curve.freq) {
            rear_curve
        } else {
            crate::read::interpolate_log_space(&front_curve.freq, &rear_curve)
        };

    // 2. Calculate Delay
    let delay_ms = config.separation_meters / 343.0 * 1000.0;
    info!(
        "  Cardioid: Separation {:.2}m -> Delay {:.2}ms",
        config.separation_meters, delay_ms
    );

    // 3. Simulate Combined Response
    use num_complex::Complex;
    let n_points = front_curve.freq.len();
    let mut combined_complex = Vec::with_capacity(n_points);
    let front_phase =
        front_curve
            .phase
            .as_ref()
            .ok_or_else(|| AutoeqError::InvalidMeasurement {
                message: "Cardioid front phase missing after validation".to_string(),
            })?;
    let rear_phase = rear_curve
        .phase
        .as_ref()
        .ok_or_else(|| AutoeqError::InvalidMeasurement {
            message: "Cardioid rear phase missing after interpolation".to_string(),
        })?;

    for i in 0..n_points {
        let f = front_curve.freq[i];
        let omega = 2.0 * std::f64::consts::PI * f;

        // Front
        let f_mag = 10.0_f64.powf(front_curve.spl[i] / 20.0);
        let f_phi = front_phase[i].to_radians();
        let f_c = Complex::from_polar(f_mag, f_phi);

        // Rear (Inverted + Delayed)
        let r_mag = 10.0_f64.powf(rear_curve.spl[i] / 20.0);
        let r_phi_meas = rear_phase[i].to_radians();

        // Delay phase shift: -omega * delay
        let delay_s = delay_ms / 1000.0;
        let delay_phi = -omega * delay_s;

        // Inversion: +180 deg (PI rad)
        let invert_phi = std::f64::consts::PI;

        let r_phi_total = r_phi_meas + delay_phi + invert_phi;
        let r_c = Complex::from_polar(r_mag, r_phi_total);

        let sum = f_c + r_c;
        combined_complex.push(sum);
    }

    let combined_curve = Curve {
        freq: front_curve.freq.clone(),
        spl: ndarray::Array1::from_iter(
            combined_complex
                .iter()
                .map(|z| 20.0 * z.norm().max(1e-12).log10()),
        ),
        phase: Some(ndarray::Array1::from_iter(
            combined_complex.iter().map(|z| z.arg().to_degrees()),
        )),
        ..Default::default()
    };

    // 4. Optimize EQ
    let min_freq = room_config.optimizer.min_freq;
    let max_freq = room_config.optimizer.max_freq;
    let pre_score = flat_loss_score(&combined_curve, min_freq, max_freq);

    let (eq_filters, post_score) = eq::optimize_channel_eq(
        &combined_curve,
        &room_config.optimizer,
        room_config.target_curve.as_ref(),
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("EQ optimization failed for Cardioid sum: {}", e),
    })?;

    let (eq_filters, post_score, final_curve) = if eq_score_regressed(pre_score, post_score) {
        warn!(
            "  Global EQ rejected for Cardioid sum {}: flat loss {:.6} -> {:.6}",
            channel_name, pre_score, post_score
        );
        (Vec::new(), pre_score, combined_curve.clone())
    } else {
        info!(
            "  Global EQ: {} filters, pre={:.6}, post={:.6}",
            eq_filters.len(),
            pre_score,
            post_score
        );
        let eq_resp =
            response::compute_peq_complex_response(&eq_filters, &combined_curve.freq, sample_rate);
        let final_curve = response::apply_complex_response(&combined_curve, &eq_resp);
        (eq_filters, post_score, final_curve)
    };

    // 5. Build Output Chain
    // Prepare display curves
    let driver_curves_for_display = vec![
        output::extend_curve_to_full_range(&front_curve),
        output::extend_curve_to_full_range(&rear_curve),
    ];

    let mut chain = output::build_cardioid_dsp_chain_with_curves(
        channel_name,
        &[0.0, 0.0],      // Gains (0 for now)
        &[0.0, delay_ms], // Delays
        &eq_filters,
        None,
        None,
        Some(&driver_curves_for_display),
    );

    // Populate initial/final curves in chain
    let (norm_range, _) = detect_passband_and_mean(&combined_curve);
    let display_initial = output::extend_curve_to_full_range(&combined_curve);
    let display_resp =
        response::compute_peq_complex_response(&eq_filters, &display_initial.freq, sample_rate);
    let display_final = response::apply_complex_response(&display_initial, &display_resp);

    let mut initial_data: super::types::CurveData = (&display_initial).into();
    initial_data.norm_range = norm_range;
    let mut final_data: super::types::CurveData = (&display_final).into();
    final_data.norm_range = norm_range;

    chain.initial_curve = Some(initial_data.clone());
    chain.final_curve = Some(final_data.clone());
    chain.eq_response = Some(output::compute_eq_response(&initial_data, &final_data));

    // Mean SPL
    let freqs_f32: Vec<f32> = combined_curve.freq.iter().map(|&f| f as f32).collect();
    let spl_f32: Vec<f32> = combined_curve.spl.iter().map(|&s| s as f32).collect();
    let mean_spl = compute_average_response(
        &freqs_f32,
        &spl_f32,
        Some((min_freq as f32, max_freq as f32)),
    ) as f64;

    Ok((
        chain,
        pre_score,
        post_score,
        combined_curve,
        final_curve,
        eq_filters,
        mean_spl,
        None,
        None, // IIR-only for cardioid
    ))
}
