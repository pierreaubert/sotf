//! Multi-subwoofer optimization
//!
//! Supports two modes:
//! - Standard: optimizes gain + delay per subwoofer
//! - All-pass enhanced: optimizes gain + delay + all-pass
//!   biquad filters per subwoofer. The all-pass filters add phase rotation without
//!   changing magnitude, improving destructive interference cancellation.

use crate::Curve;
use crate::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use crate::workflow::DriverOptimizationResult;
use log::{info, warn};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use num_complex::Complex64;
use std::error::Error;
use std::f64::consts::PI;

use super::types::{MeasurementSource, OptimizerConfig};
use crate::read as load;

/// Optimize multi-subwoofer configuration
///
/// # Arguments
/// * `measurements` - List of subwoofer measurements (sources)
/// * `config` - Optimizer configuration
/// * `sample_rate` - Sample rate
///
/// # Returns
/// * Tuple of (DriverOptimizationResult, Combined Curve)
///
/// # Note on Phase Data
/// For accurate optimization, measurements should include phase data.
/// The optimizer uses complex summation to model constructive/destructive
/// interference between subwoofers. Without phase data, the optimizer
/// assumes 0° phase for all measurements, which may result in suboptimal
/// delay settings.
pub fn optimize_multisub(
    measurements: &[MeasurementSource],
    config: &OptimizerConfig,
    sample_rate: f64,
) -> Result<(DriverOptimizationResult, Curve), Box<dyn Error>> {
    // Load all measurements and check for phase data
    let mut driver_measurements = Vec::new();
    let mut missing_phase_count = 0;

    for source in measurements {
        let curve = load::load_source(source)?;
        if curve.phase.is_none() {
            missing_phase_count += 1;
        }
        driver_measurements.push(DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: curve.phase, // Critical: use phase for accurate summation
        });
    }

    // Warn if phase data is missing
    if missing_phase_count > 0 {
        warn!(
            "Multi-sub optimization: {} of {} measurements are missing phase data. \
            This may result in inaccurate delay optimization. \
            For best results, include phase data in your measurements (e.g., export from REW with phase).",
            missing_phase_count,
            measurements.len()
        );
    }

    // Create drivers data with NO crossover filtering
    let drivers_data = DriversLossData::new(driver_measurements, CrossoverType::None);

    let result = crate::workflow::optimize_multisub(
        drivers_data.clone(),
        config.min_freq,
        config.max_freq,
        sample_rate,
        &config.algorithm,
        config.max_iter,
        config.min_db,
        config.max_db,
        config.seed,
    )?;

    // Compute combined response
    let combined_response = crate::loss::compute_drivers_combined_response(
        &drivers_data,
        &result.gains,
        &[], // no crossovers
        Some(&result.delays),
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_response,
        phase: None,
    };

    Ok((result, combined_curve))
}

/// Result of multi-sub optimization with all-pass filters.
#[derive(Debug, Clone)]
pub struct MultiSubAllPassResult {
    /// Standard optimization result (gains, delays)
    pub base: DriverOptimizationResult,
    /// Per-subwoofer all-pass filter parameters: Vec of (frequency_hz, q)
    pub allpass_filters: Vec<(f64, f64)>,
    /// Combined frequency response after optimization
    pub combined_curve: Curve,
}

/// Optimize multi-subwoofer configuration with per-sub all-pass filters.
///
/// Extends the standard gain+delay optimization with one all-pass biquad filter
/// per subwoofer. The all-pass filter adds phase rotation without changing magnitude,
/// enabling better cancellation of room modes through improved phase alignment.
///
/// Parameter vector layout: [gains(N), delays(N), ap_freq(N), ap_q(N)]
///
/// Inspired by Brännmark, Rosencratz & Andersson.
pub fn optimize_multisub_with_allpass(
    measurements: &[MeasurementSource],
    config: &OptimizerConfig,
    sample_rate: f64,
) -> Result<MultiSubAllPassResult, Box<dyn Error>> {
    // Load measurements
    let mut driver_measurements = Vec::new();
    let mut missing_phase_count = 0;

    for source in measurements {
        let curve = load::load_source(source)?;
        if curve.phase.is_none() {
            missing_phase_count += 1;
        }
        driver_measurements.push(DriverMeasurement {
            freq: curve.freq,
            spl: curve.spl,
            phase: curve.phase,
        });
    }

    if missing_phase_count > 0 {
        warn!(
            "Multi-sub all-pass optimization: {} of {} measurements are missing phase data.",
            missing_phase_count,
            measurements.len()
        );
    }

    let drivers_data = DriversLossData::new(driver_measurements, CrossoverType::None);
    let n_drivers = drivers_data.drivers.len();

    // Parameter vector: [gains(N), delays(N), ap_freq(N), ap_q(N)]
    let n_params = n_drivers * 4;

    // Bounds
    let mut lower_bounds = Vec::with_capacity(n_params);
    let mut upper_bounds = Vec::with_capacity(n_params);

    // Gains: [-max_db, max_db]
    for _ in 0..n_drivers {
        lower_bounds.push(-config.max_db);
        upper_bounds.push(config.max_db);
    }
    // Delays: [0, 20] ms
    for _ in 0..n_drivers {
        lower_bounds.push(0.0);
        upper_bounds.push(20.0);
    }
    // All-pass frequencies: [min_freq, max_freq]
    for _ in 0..n_drivers {
        lower_bounds.push(config.min_freq.max(20.0));
        upper_bounds.push(config.max_freq.min(200.0)); // sub range
    }
    // All-pass Q: [0.3, 5.0]
    for _ in 0..n_drivers {
        lower_bounds.push(0.3);
        upper_bounds.push(5.0);
    }

    // Initial guess: zeros for gains, zeros for delays, 60 Hz + Q=1 for allpass
    let mut x = vec![0.0; n_params];
    for i in 0..n_drivers {
        x[2 * n_drivers + i] = 60.0; // initial AP frequency
        x[3 * n_drivers + i] = 1.0; // initial AP Q
    }

    // Pre-objective
    let pre_obj = multisub_allpass_loss(
        &drivers_data,
        &x,
        sample_rate,
        config.min_freq,
        config.max_freq,
    );

    // Use DE optimizer (global search needed for all-pass parameters)
    let drivers_data_clone = drivers_data.clone();
    let min_freq = config.min_freq;
    let max_freq = config.max_freq;

    let objective_fn = move |params: &Array1<f64>| -> f64 {
        multisub_allpass_loss(
            &drivers_data_clone,
            params.as_slice().unwrap(),
            sample_rate,
            min_freq,
            max_freq,
        )
    };

    // Build bounds as (lower, upper) pairs
    let bounds: Vec<(f64, f64)> = lower_bounds
        .iter()
        .zip(upper_bounds.iter())
        .map(|(&l, &u)| (l, u))
        .collect();

    let de_config = crate::de::DEConfigBuilder::default()
        .maxiter(config.max_iter)
        .seed(config.seed.unwrap_or(42))
        .build()
        .expect("DEConfig build should not fail");

    let de_result = crate::de::differential_evolution(&objective_fn, &bounds, de_config)
        .map_err(|e| format!("DE optimization failed: {:?}", e))?;

    x = de_result.x.to_vec();
    let post_obj = de_result.fun;

    info!(
        "Multi-sub all-pass optimization: pre={:.4}, post={:.4}, improvement={:.2} dB",
        pre_obj,
        post_obj,
        pre_obj - post_obj
    );

    // Extract results
    let gains = x[0..n_drivers].to_vec();
    let delays = x[n_drivers..2 * n_drivers].to_vec();
    let mut allpass_filters = Vec::with_capacity(n_drivers);
    for i in 0..n_drivers {
        let freq = x[2 * n_drivers + i];
        let q = x[3 * n_drivers + i];
        allpass_filters.push((freq, q));
        info!(
            "  Sub {}: gain={:.1} dB, delay={:.1} ms, AP: {:.0} Hz Q={:.2}",
            i, gains[i], delays[i], freq, q
        );
    }

    // Compute combined response with all-pass filters applied
    let combined_spl = compute_combined_with_allpass(
        &drivers_data,
        &gains,
        &delays,
        &allpass_filters,
        sample_rate,
    );

    let combined_curve = Curve {
        freq: drivers_data.freq_grid.clone(),
        spl: combined_spl,
        phase: None,
    };

    Ok(MultiSubAllPassResult {
        base: DriverOptimizationResult {
            gains,
            delays,
            crossover_freqs: vec![],
            pre_objective: pre_obj,
            post_objective: post_obj,
            converged: true,
        },
        allpass_filters,
        combined_curve,
    })
}

/// Loss function for multi-sub optimization with all-pass filters.
///
/// Parameter vector layout: [gains(N), delays(N), ap_freq(N), ap_q(N)]
fn multisub_allpass_loss(
    data: &DriversLossData,
    params: &[f64],
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    let n_drivers = data.drivers.len();
    let gains = &params[0..n_drivers];
    let delays = &params[n_drivers..2 * n_drivers];

    let mut allpass_filters = Vec::with_capacity(n_drivers);
    for i in 0..n_drivers {
        let freq = params[2 * n_drivers + i];
        let q = params[3 * n_drivers + i];
        allpass_filters.push((freq, q));
    }

    let combined = compute_combined_with_allpass(data, gains, delays, &allpass_filters, sample_rate);

    // Normalize and compute flatness loss
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..data.freq_grid.len() {
        let freq = data.freq_grid[i];
        if freq >= min_freq && freq <= max_freq {
            sum += combined[i];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized = &combined - mean;

    crate::loss::flat_loss(&data.freq_grid, &normalized, min_freq, max_freq)
}

/// Compute combined multi-sub response with per-sub all-pass filters.
fn compute_combined_with_allpass(
    data: &DriversLossData,
    gains: &[f64],
    delays: &[f64],
    allpass_filters: &[(f64, f64)],
    sample_rate: f64,
) -> Array1<f64> {
    let n_drivers = data.drivers.len();

    // Prepare driver curves on common grid (same approach as loss.rs)
    let driver_curves: Vec<Curve> = data
        .drivers
        .iter()
        .map(|d| {
            crate::read::normalize_and_interpolate_response_with_range(
                &data.freq_grid,
                &Curve {
                    freq: d.freq.clone(),
                    spl: d.spl.clone(),
                    phase: d.phase.clone(),
                },
                20.0,
                20000.0,
            )
        })
        .collect();

    // Sum complex responses
    let mut combined_complex = Array1::<Complex64>::zeros(data.freq_grid.len());

    for i in 0..n_drivers {
        let mag_factor = 10.0_f64.powf(gains[i] / 20.0);
        let delay_s = delays[i] / 1000.0;
        let (ap_freq, ap_q) = allpass_filters[i];
        let ap_biquad = Biquad::new(BiquadFilterType::AllPass, ap_freq, sample_rate, ap_q, 0.0);

        for j in 0..data.freq_grid.len() {
            let f = data.freq_grid[j];
            let spl = driver_curves[i].spl[j];

            // Driver complex response
            let z_driver = if let Some(phase) = &driver_curves[i].phase {
                let phi = phase[j].to_radians();
                let m = 10.0_f64.powf(spl / 20.0);
                Complex64::from_polar(m, phi)
            } else {
                let m = 10.0_f64.powf(spl / 20.0);
                Complex64::new(m, 0.0)
            };

            // Delay phase
            let phi_delay = -2.0 * PI * f * delay_s;
            let z_delay = Complex64::from_polar(1.0, phi_delay);

            // All-pass complex response
            let z_allpass = allpass_complex_response(&ap_biquad, f);

            combined_complex[j] += z_driver * mag_factor * z_delay * z_allpass;
        }
    }

    // Convert to dB SPL
    combined_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10())
}

/// Compute complex frequency response of an all-pass biquad at frequency f.
fn allpass_complex_response(biquad: &Biquad, f: f64) -> Complex64 {
    let (a1, a2, b0, b1, b2) = biquad.constants();
    let omega = 2.0 * PI * f / biquad.srate;
    let z_inv = Complex64::from_polar(1.0, -omega);
    let z_inv2 = z_inv * z_inv;

    let num = b0 + b1 * z_inv + b2 * z_inv2;
    let den = 1.0 + a1 * z_inv + a2 * z_inv2;

    num / den
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn make_driver(freq: Vec<f64>, spl: Vec<f64>, phase: Option<Vec<f64>>) -> DriverMeasurement {
        DriverMeasurement {
            freq: Array1::from_vec(freq),
            spl: Array1::from_vec(spl),
            phase: phase.map(Array1::from_vec),
        }
    }

    #[test]
    fn test_allpass_complex_response_unity_magnitude() {
        // All-pass should have unity magnitude at all frequencies
        let biquad = Biquad::new(BiquadFilterType::AllPass, 80.0, 48000.0, 2.0, 0.0);

        for &f in &[20.0, 50.0, 80.0, 100.0, 200.0] {
            let response = allpass_complex_response(&biquad, f);
            let magnitude = response.norm();
            assert!(
                (magnitude - 1.0).abs() < 0.01,
                "All-pass magnitude at {} Hz should be ~1.0, got {:.4}",
                f,
                magnitude
            );
        }
    }

    #[test]
    fn test_allpass_complex_response_phase_varies() {
        // All-pass should have varying phase across frequencies
        let biquad = Biquad::new(BiquadFilterType::AllPass, 80.0, 48000.0, 2.0, 0.0);

        let phase_20 = allpass_complex_response(&biquad, 20.0).arg();
        let phase_80 = allpass_complex_response(&biquad, 80.0).arg();
        let phase_200 = allpass_complex_response(&biquad, 200.0).arg();

        // Phase should vary significantly across these frequencies
        assert!(
            (phase_20 - phase_200).abs() > 0.1,
            "All-pass phase should vary: 20Hz={:.3}, 200Hz={:.3}",
            phase_20,
            phase_200
        );
        // Phase at center frequency should be between extremes
        assert!(
            phase_80 != phase_20 || phase_80 != phase_200,
            "Phase at center frequency should differ from at least one extreme"
        );
    }

    #[test]
    fn test_multisub_allpass_loss_basic() {
        // Two subs with flat response
        let freqs = vec![20.0, 40.0, 60.0, 80.0, 100.0, 150.0, 200.0];
        let spl = vec![80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0];

        let d1 = make_driver(freqs.clone(), spl.clone(), None);
        let d2 = make_driver(freqs, spl, None);
        let data = DriversLossData::new(vec![d1, d2], CrossoverType::None);

        // Params: [gain1, gain2, delay1, delay2, ap_f1, ap_f2, ap_q1, ap_q2]
        let params = vec![0.0, 0.0, 0.0, 0.0, 60.0, 60.0, 1.0, 1.0];
        let loss = multisub_allpass_loss(&data, &params, 48000.0, 20.0, 200.0);

        assert!(loss.is_finite(), "Loss should be finite");
        assert!(loss >= 0.0, "Loss should be non-negative");
    }

    #[test]
    fn test_multisub_allpass_loss_with_phase_data() {
        // Test with phase data present (complex summation path)
        let freqs = vec![20.0, 40.0, 60.0, 80.0, 100.0, 150.0, 200.0];
        let spl = vec![80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0];
        let phase1 = vec![0.0, -10.0, -20.0, -30.0, -40.0, -50.0, -60.0];
        let phase2 = vec![0.0, -5.0, -10.0, -15.0, -20.0, -25.0, -30.0];

        let d1 = make_driver(freqs.clone(), spl.clone(), Some(phase1));
        let d2 = make_driver(freqs, spl, Some(phase2));
        let data = DriversLossData::new(vec![d1, d2], CrossoverType::None);

        let params = vec![0.0, -3.0, 0.0, 2.0, 50.0, 80.0, 1.5, 2.0];
        let loss = multisub_allpass_loss(&data, &params, 48000.0, 20.0, 200.0);

        assert!(loss.is_finite(), "Loss with phase data should be finite");
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_compute_combined_with_allpass_finite_output() {
        // Two subs with all-pass filters should produce finite output
        let freqs = vec![20.0, 60.0, 100.0, 200.0];
        let spl = vec![80.0, 85.0, 82.0, 78.0];
        let d1 = make_driver(freqs.clone(), spl.clone(), None);
        let d2 = make_driver(freqs, spl, None);
        let data = DriversLossData::new(vec![d1, d2], CrossoverType::None);

        let gains = vec![0.0, 0.0];
        let delays = vec![0.0, 0.0];
        let allpass = vec![(60.0, 1.0), (60.0, 1.0)];

        let combined = compute_combined_with_allpass(&data, &gains, &delays, &allpass, 48000.0);

        for i in 0..combined.len() {
            assert!(
                combined[i].is_finite(),
                "combined[{}] should be finite",
                i
            );
        }
    }

    #[test]
    fn test_allpass_gain_delay_affect_loss() {
        // Changing gain/delay should change the loss
        let freqs = vec![20.0, 40.0, 60.0, 80.0, 100.0, 150.0, 200.0];
        let spl = vec![80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0];

        let d1 = make_driver(freqs.clone(), spl.clone(), None);
        let d2 = make_driver(freqs, spl, None);
        let data = DriversLossData::new(vec![d1, d2], CrossoverType::None);

        let params_zero = vec![0.0, 0.0, 0.0, 0.0, 60.0, 60.0, 1.0, 1.0];
        let params_diff = vec![3.0, -3.0, 0.0, 5.0, 40.0, 100.0, 2.0, 0.5];

        let loss_zero = multisub_allpass_loss(&data, &params_zero, 48000.0, 20.0, 200.0);
        let loss_diff = multisub_allpass_loss(&data, &params_diff, 48000.0, 20.0, 200.0);

        // Different params should produce different loss
        assert!(
            (loss_zero - loss_diff).abs() > 1e-6,
            "different params should produce different loss: {} vs {}",
            loss_zero,
            loss_diff
        );
    }
}
