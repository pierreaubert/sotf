//! Specific optimization workflows for different system topologies.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read::load_source;
use crate::response;
use log::info;
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use std::collections::HashMap;
use std::path::Path;

use super::crossover;
use super::dba;
use super::eq;
use super::multisub;
use super::optimize::{ChannelOptimizationResult, RoomOptimizationResult};
use super::output;
use super::types::{
    CardioidConfig, ChannelDspChain, DBAConfig, DriverDspChain, MultiSubGroup,
    OptimizationMetadata, RoomConfig, SpeakerConfig, SubwooferStrategy, SystemConfig,
};

/// Align channel levels by normalizing down to the lowest level.
pub fn align_channels_to_lowest(
    channels: &HashMap<String, Curve>,
    ranges: &HashMap<String, (f64, f64)>,
) -> HashMap<String, f64> {
    let mut means = HashMap::new();
    let mut min_mean = f64::INFINITY;

    for (name, curve) in channels {
        let (min_f, max_f) = ranges.get(name).cloned().unwrap_or((100.0, 2000.0));

        let freqs_f32: Vec<f32> = curve.freq.iter().map(|&f| f as f32).collect();
        let spl_f32: Vec<f32> = curve.spl.iter().map(|&s| s as f32).collect();

        let mean =
            compute_average_response(&freqs_f32, &spl_f32, Some((min_f as f32, max_f as f32)))
                as f64;

        means.insert(name.clone(), mean);
        if mean < min_mean {
            min_mean = mean;
        }
    }

    let mut gains = HashMap::new();
    for (name, mean) in means {
        let diff = min_mean - mean;
        gains.insert(name.clone(), diff);
        info!(
            "  Level alignment for '{}': {:.2} dB (mean {:.2} -> {:.2})",
            name, diff, mean, min_mean
        );
    }
    gains
}

/// Compute flat_loss score for a curve within a frequency range.
///
/// Normalizes SPL by subtracting the mean in the given range, then computes
/// the weighted MSE — same metric used in the main optimization path.
fn compute_flat_loss(curve: &Curve, min_freq: f64, max_freq: f64) -> f64 {
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

/// Helper to load curves for all logical channels
fn load_logical_channels(
    config: &RoomConfig,
    sys: &SystemConfig,
) -> Result<HashMap<String, Curve>> {
    let mut curves = HashMap::new();
    for (role, meas_key) in &sys.speakers {
        if let Some(cfg) = config.speakers.get(meas_key) {
            let source = match cfg {
                SpeakerConfig::Single(s) => s,
                _ => {
                    return Err(AutoeqError::InvalidConfiguration {
                        message: format!("Workflow requires Single speaker config for '{}'", role),
                    });
                }
            };
            let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: e.to_string(),
            })?;
            curves.insert(role.clone(), curve);
        }
    }
    Ok(curves)
}

// ============================================================================
// Sub Preprocessing for Stereo Workflows
// ============================================================================

/// Information about an individual subwoofer driver from multi-sub preprocessing
struct SubDriverInfo {
    /// Driver name (e.g., "subs_1", "Front Sub")
    name: String,
    /// Gain in dB from MSO/DBA optimization
    gain: f64,
    /// Delay in ms from MSO/DBA optimization
    delay: f64,
    /// Whether this driver is polarity-inverted
    inverted: bool,
    /// Initial measurement curve for this driver
    initial_curve: Option<Curve>,
}

/// Result of subwoofer preprocessing
struct SubPreprocessResult {
    /// Combined curve (for crossover optimization and shared post-EQ)
    combined_curve: Curve,
    /// Per-driver info (None for single sub)
    drivers: Option<Vec<SubDriverInfo>>,
}

/// Preprocess the LFE channel's SpeakerConfig into a combined curve and per-driver info.
///
/// Dispatches by SpeakerConfig variant:
/// - Single: load curve, no drivers
/// - MultiSub + Mso: run MSO optimization, return combined + per-sub gains/delays
/// - MultiSub + Single: average all subs, return combined + per-sub info (zero gains/delays)
/// - MultiSub + Dba: error (should use SpeakerConfig::Dba)
/// - Cardioid: simulate combined response from front + delayed/inverted rear
/// - Dba: run DBA optimization, return combined + front/rear info
/// - Group: error (handled by generic path)
fn preprocess_sub(
    lfe_config: &SpeakerConfig,
    strategy: &SubwooferStrategy,
    optimizer: &super::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    match lfe_config {
        SpeakerConfig::Single(source) => {
            let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: e.to_string(),
            })?;
            Ok(SubPreprocessResult {
                combined_curve: curve,
                drivers: None,
            })
        }
        SpeakerConfig::MultiSub(ms) => match strategy {
            SubwooferStrategy::Mso => preprocess_multisub_mso(ms, optimizer, sample_rate),
            SubwooferStrategy::Single => preprocess_multisub_independent(ms),
            SubwooferStrategy::Dba => Err(AutoeqError::InvalidConfiguration {
                message: "SubwooferStrategy::Dba requires SpeakerConfig::Dba, not MultiSub"
                    .to_string(),
            }),
        },
        SpeakerConfig::Cardioid(c) => preprocess_cardioid(c),
        SpeakerConfig::Dba(d) => preprocess_dba(d, optimizer, sample_rate),
        SpeakerConfig::Group(_) => Err(AutoeqError::InvalidConfiguration {
            message: "Group speaker config should not reach stereo sub workflow; use generic path"
                .to_string(),
        }),
    }
}

/// MSO: optimize inter-sub gains/delays, return combined curve + per-sub info
fn preprocess_multisub_mso(
    ms: &MultiSubGroup,
    optimizer: &super::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    info!("  MSO optimization for {} subwoofers", ms.subwoofers.len());

    let (result, combined) = multisub::optimize_multisub(&ms.subwoofers, optimizer, sample_rate)
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("MSO optimization failed: {}", e),
        })?;

    info!(
        "  MSO result: gains={:?}, delays={:?}",
        result.gains, result.delays
    );

    // Load individual curves for driver info
    let mut drivers = Vec::new();
    for (i, source) in ms.subwoofers.iter().enumerate() {
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        drivers.push(SubDriverInfo {
            name: format!("{}_{}", ms.name, i + 1),
            gain: result.gains.get(i).copied().unwrap_or(0.0),
            delay: result.delays.get(i).copied().unwrap_or(0.0),
            inverted: false,
            initial_curve: Some(curve),
        });
    }

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// Independent subs: average all sub curves, return combined + per-sub info (zero gains/delays)
fn preprocess_multisub_independent(ms: &MultiSubGroup) -> Result<SubPreprocessResult> {
    info!(
        "  Independent sub averaging for {} subwoofers",
        ms.subwoofers.len()
    );

    let mut curves = Vec::new();
    for source in &ms.subwoofers {
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        curves.push(curve);
    }

    // Power summation on the first sub's frequency grid:
    // Convert dB to linear power, sum, convert back to dB.
    // This correctly represents incoherent summation of multiple subs.
    let ref_freq = curves[0].freq.clone();
    let mut sum_power = ndarray::Array1::<f64>::zeros(ref_freq.len());
    for curve in &curves {
        let interp = crate::read::interpolate_log_space(&ref_freq, curve);
        sum_power += &interp.spl.mapv(|db| 10.0_f64.powf(db / 10.0));
    }
    let avg_spl = sum_power.mapv(|p| 10.0 * p.log10());

    let combined = Curve {
        freq: ref_freq,
        spl: avg_spl,
        phase: None,
    };

    let drivers: Vec<SubDriverInfo> = curves
        .into_iter()
        .enumerate()
        .map(|(i, curve)| SubDriverInfo {
            name: format!("{}_{}", ms.name, i + 1),
            gain: 0.0,
            delay: 0.0,
            inverted: false,
            initial_curve: Some(curve),
        })
        .collect();

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// Cardioid: simulate combined response from front + delayed/inverted rear sub
fn preprocess_cardioid(c: &CardioidConfig) -> Result<SubPreprocessResult> {
    let front_curve = load_source(&c.front).map_err(|e| AutoeqError::InvalidMeasurement {
        message: format!("Cardioid front: {}", e),
    })?;
    let rear_curve = load_source(&c.rear).map_err(|e| AutoeqError::InvalidMeasurement {
        message: format!("Cardioid rear: {}", e),
    })?;

    let delay_ms = c.separation_meters / 343.0 * 1000.0;
    info!(
        "  Cardioid: separation={:.2}m, delay={:.2}ms",
        c.separation_meters, delay_ms
    );

    // Simulate combined response (complex sum of front + delayed/inverted rear)
    use num_complex::Complex;
    let n_points = front_curve.freq.len();
    let mut combined_spl = ndarray::Array1::zeros(n_points);

    let front_phase_zeros = ndarray::Array1::zeros(n_points);
    let rear_phase_zeros = ndarray::Array1::zeros(n_points);
    let front_phase = front_curve.phase.as_ref().unwrap_or(&front_phase_zeros);
    let rear_phase = rear_curve.phase.as_ref().unwrap_or(&rear_phase_zeros);

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
        let delay_s = delay_ms / 1000.0;
        let delay_phi = -omega * delay_s;
        let invert_phi = std::f64::consts::PI;
        let r_phi_total = r_phi_meas + delay_phi + invert_phi;
        let r_c = Complex::from_polar(r_mag, r_phi_total);

        let sum = f_c + r_c;
        combined_spl[i] = 20.0 * sum.norm().log10();
    }

    let combined = Curve {
        freq: front_curve.freq.clone(),
        spl: combined_spl,
        phase: None,
    };

    let drivers = vec![
        SubDriverInfo {
            name: "Front Sub".to_string(),
            gain: 0.0,
            delay: 0.0,
            inverted: false,
            initial_curve: Some(front_curve),
        },
        SubDriverInfo {
            name: "Rear Sub".to_string(),
            gain: 0.0,
            delay: delay_ms,
            inverted: true,
            initial_curve: Some(rear_curve),
        },
    ];

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// DBA: run DBA optimization, return combined curve + front/rear driver info
fn preprocess_dba(
    d: &DBAConfig,
    optimizer: &super::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    info!("  DBA optimization");

    let (result, combined) = dba::optimize_dba(d, optimizer, sample_rate).map_err(|e| {
        AutoeqError::OptimizationFailed {
            message: format!("DBA optimization failed: {}", e),
        }
    })?;

    info!(
        "  DBA result: gains={:?}, delays={:?}",
        result.gains, result.delays
    );

    // Load front and rear array responses for display
    let front_curve =
        dba::sum_array_response(&d.front).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("DBA front array: {}", e),
        })?;
    let rear_curve =
        dba::sum_array_response(&d.rear).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("DBA rear array: {}", e),
        })?;

    let drivers = vec![
        SubDriverInfo {
            name: "Front Array".to_string(),
            gain: result.gains.first().copied().unwrap_or(0.0),
            delay: result.delays.first().copied().unwrap_or(0.0),
            inverted: false,
            initial_curve: Some(front_curve),
        },
        SubDriverInfo {
            name: "Rear Array".to_string(),
            gain: result.gains.get(1).copied().unwrap_or(0.0),
            delay: result.delays.get(1).copied().unwrap_or(0.0),
            inverted: true,
            initial_curve: Some(rear_curve),
        },
    ];

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// Workflow for Stereo 2.0 (No Subwoofer)
pub fn optimize_stereo_2_0(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    info!("Running Stereo 2.0 Optimization Workflow");

    // 1. Load measurements
    let curves = load_logical_channels(config, sys)?;

    // 2. Alignment
    let mut ranges = HashMap::new();
    for role in curves.keys() {
        ranges.insert(role.clone(), (100.0, 2000.0));
    }
    let gains = align_channels_to_lowest(&curves, &ranges);

    // 3. Optimization
    let min_freq = config.optimizer.min_freq;
    let max_freq = config.optimizer.max_freq;
    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for (role, curve) in &curves {
        let gain = *gains.get(role).unwrap_or(&0.0);

        // Apply gain to curve for optimization context
        let mut aligned_curve = curve.clone();
        for s in aligned_curve.spl.iter_mut() {
            *s += gain;
        }

        // Pre-optimization score
        let pre_score = compute_flat_loss(&aligned_curve, min_freq, max_freq);

        info!(
            "  Optimizing '{}' with alignment gain {:.2} dB (pre_score={:.4})",
            role, gain, pre_score
        );

        let (filters, _loss) = super::optimize::optimize_eq_with_optional_schroeder(
            &aligned_curve,
            &config.optimizer,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        // Build Chain
        let mut plugins = Vec::new();
        if gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(gain));
        }
        if !filters.is_empty() {
            plugins.push(output::create_eq_plugin(&filters));
        }

        // Compute final response
        let resp =
            response::compute_peq_complex_response(&filters, &aligned_curve.freq, sample_rate);
        let final_curve_obj = response::apply_complex_response(&aligned_curve, &resp);

        // Post-optimization score
        let post_score = compute_flat_loss(&final_curve_obj, min_freq, max_freq);

        info!("  '{}' post_score={:.4}", role, post_score);

        let initial_data: super::types::CurveData = (&aligned_curve).into();
        let final_data: super::types::CurveData = (&final_curve_obj).into();
        let eq_resp = super::output::compute_eq_response(&initial_data, &final_data);
        let chain = ChannelDspChain {
            channel: role.clone(),
            plugins,
            drivers: None,
            initial_curve: Some(initial_data),
            final_curve: Some(final_data),
            eq_response: Some(eq_resp),
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        };

        channel_chains.insert(role.clone(), chain);
        pre_scores.push(pre_score);
        post_scores.push(post_score);

        channel_results.insert(
            role.clone(),
            ChannelOptimizationResult {
                name: role.clone(),
                pre_score,
                post_score,
                initial_curve: curve.clone(),
                final_curve: final_curve_obj,
                biquads: filters,
                fir_coeffs: None,
            },
        );
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
        },
    })
}

/// Workflow for Stereo 2.1 (With Subwoofer)
pub fn optimize_stereo_2_1(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    info!("Running Stereo 2.1 Optimization Workflow");

    let sub_role = "LFE";

    // Load L and R (must be Single speaker configs)
    let mut curves = HashMap::new();
    for role in ["L", "R"] {
        let meas_key = sys
            .speakers
            .get(role)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker mapping for '{}'", role),
            })?;
        let cfg = config
            .speakers
            .get(meas_key)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker config for key '{}'", meas_key),
            })?;
        let source = match cfg {
            SpeakerConfig::Single(s) => s,
            _ => {
                return Err(AutoeqError::InvalidConfiguration {
                    message: format!("'{}' must be a Single speaker config", role),
                });
            }
        };
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        curves.insert(role.to_string(), curve);
    }

    // Preprocess LFE (handles Single, MultiSub/MSO, Cardioid, DBA)
    let sub_sys = sys
        .subwoofers
        .as_ref()
        .ok_or(AutoeqError::InvalidConfiguration {
            message: "Missing subwoofers configuration".to_string(),
        })?;

    let lfe_meas_key = sys
        .speakers
        .get(sub_role)
        .ok_or(AutoeqError::InvalidConfiguration {
            message: "Missing speaker mapping for 'LFE'".to_string(),
        })?;
    let lfe_speaker_config =
        config
            .speakers
            .get(lfe_meas_key)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker config for key '{}'", lfe_meas_key),
            })?;

    let sub_preprocess = preprocess_sub(
        lfe_speaker_config,
        &sub_sys.config,
        &config.optimizer,
        sample_rate,
    )?;
    curves.insert(sub_role.to_string(), sub_preprocess.combined_curve.clone());

    let xover_key = sub_sys
        .crossover
        .as_deref()
        .ok_or(AutoeqError::InvalidConfiguration {
            message: "Subwoofer config requires 'crossover' reference".to_string(),
        })?;

    let xover_config = config
        .crossovers
        .as_ref()
        .and_then(|m| m.get(xover_key))
        .ok_or(AutoeqError::InvalidConfiguration {
            message: format!("Crossover '{}' not found in crossovers section", xover_key),
        })?;

    let xover_type_str = &xover_config.crossover_type;

    // Handle fixed frequency vs range
    let (min_xo, max_xo, est_xo) = if let Some(f) = xover_config.frequency {
        (f, f, f)
    } else if let Some((min, max)) = xover_config.frequency_range {
        (min, max, (min * max).sqrt())
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: "Subwoofer crossover requires 'frequency' or 'frequency_range'".to_string(),
        });
    };

    // 1. Level Measurement & Alignment
    // Use max_xo for boundary to ensure we measure Sub fully and Mains safely.
    // Align sub over its full passband (down to optimizer min_freq) to prevent
    // the crossover optimizer from seeing a level mismatch in the deep bass.
    let mut ranges = HashMap::new();
    ranges.insert("L".to_string(), (max_xo, 2000.0));
    ranges.insert("R".to_string(), (max_xo, 2000.0));
    let sub_min_align = config.optimizer.min_freq.max(20.0);
    ranges.insert(sub_role.to_string(), (sub_min_align, max_xo));

    let gains = align_channels_to_lowest(&curves, &ranges);

    // Apply gains
    let mut aligned_curves = HashMap::new();
    for (role, curve) in &curves {
        let mut c = curve.clone();
        let g = *gains.get(role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() {
            *s += g;
        }
        aligned_curves.insert(role.clone(), c);
    }

    // 3. Pre-EQ (Linearization) for L and R
    // Min freq = min_xo to ensure coverage even if crossover drops to min
    let mut pre_eq_filters = HashMap::new();
    let mut linearized_curves = aligned_curves.clone();

    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = min_xo;

        info!(
            "  Pre-EQ Linearization for '{}' (min {:.1} Hz)",
            role, min_xo
        );
        let (filters, _) = eq::optimize_channel_eq(
            &aligned_curves[role],
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        // Apply filters
        let resp = response::compute_peq_complex_response(
            &filters,
            &aligned_curves[role].freq,
            sample_rate,
        );
        let linear = response::apply_complex_response(&aligned_curves[role], &resp);

        pre_eq_filters.insert(role.to_string(), filters);
        linearized_curves.insert(role.to_string(), linear);
    }

    // 4. Crossover Optimization
    // Virtual Main = Avg(L, R)
    // We average the LINEARIZED curves
    let l_curve = &linearized_curves["L"];
    let r_curve = &linearized_curves["R"];
    let sub_curve = &linearized_curves[sub_role]; // Sub is not linearized in step 3? Spec says "Optimal EQ for L and R".

    // Average L and R
    // Average magnitude (SPL)
    // Note: geometric average of magnitude? Or average of dB?
    // compute_average_response does average of SPL values (dB).
    // Let's simple average dB for Virtual Main
    let mut virtual_main = l_curve.clone();
    for i in 0..virtual_main.spl.len() {
        virtual_main.spl[i] = (l_curve.spl[i] + r_curve.spl[i]) / 2.0;
        // Phase averaging is tricky. Use L phase?
        // For crossover optimization, we need phase.
        // Assuming L and R are phase-coherent (level aligned).
        // Let's use L phase.
    }

    // Optimize Crossover between Virtual Main and Sub
    // We reuse crossover::optimize_crossover. It expects a list of drivers.
    // [VirtualMain, Sub]

    // We need to parse crossover type for the optimizer
    let crossover_type_enum = crossover::parse_crossover_type(xover_type_str).map_err(|e| {
        AutoeqError::InvalidConfiguration {
            message: e.to_string(),
        }
    })?;

    // Determine fixed freqs vs range for optimizer
    let (fixed_freqs, range_opt) = if xover_config.frequency.is_some() {
        (Some(vec![est_xo]), None)
    } else {
        (None, Some((min_xo, max_xo)))
    };

    // The crossover optimizer should only optimize delay and polarity, not gains.
    // Level matching is handled by alignment (step 1) and re-alignment (step 4).
    // Using gain bounds allows the optimizer to shift levels, undoing alignment.
    let mut xo_optimizer_config = config.optimizer.clone();
    xo_optimizer_config.min_db = 0.0;
    xo_optimizer_config.max_db = 0.0;

    // Optimize
    let (xo_gains, xo_delays, xo_freqs, _, inversions) = crossover::optimize_crossover(
        vec![virtual_main.clone(), sub_curve.clone()],
        crossover_type_enum,
        sample_rate,
        &xo_optimizer_config,
        fixed_freqs,
        range_opt,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: e.to_string(),
    })?;

    // Results: index 0 = Mains, index 1 = Sub
    let main_gain_post = xo_gains[0];
    let main_delay_post = xo_delays[0];
    let sub_gain_post = xo_gains[1];
    let sub_delay_post = xo_delays[1];
    let sub_inverted = inversions[1];
    let final_xo_freq = xo_freqs[0];

    info!(
        "  Crossover Optimized: Freq={:.1} Hz, Main Gain={:.2}, Sub Gain={:.2}, Main Delay={:.2}, Sub Delay={:.2}",
        final_xo_freq, main_gain_post, sub_gain_post, main_delay_post, sub_delay_post
    );

    // 6. Apply Crossover (Filters + Gain/Delay)
    // We calculate the post-crossover curves for Post-EQ using FINAL frequency

    let hp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, true);

    let apply_chain =
        |curve: &Curve, filters: &[Biquad], gain: f64, _delay: f64, _invert: bool| -> Curve {
            let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
            let mut c = response::apply_complex_response(curve, &resp);
            // Apply gain
            for s in c.spl.iter_mut() {
                *s += gain;
            }
            // Apply delay/invert (affects phase)
            // ... phase update logic ...
            // For Post-EQ magnitude, phase doesn't matter much unless we do more summing.
            c
        };

    // Note: Applying to ALIGNED curves (not linearized), and ignoring optimized delay per user request.
    let l_post = apply_chain(
        &aligned_curves["L"],
        &hp_biquads,
        main_gain_post,
        0.0,
        false,
    );
    let r_post = apply_chain(
        &aligned_curves["R"],
        &hp_biquads,
        main_gain_post,
        0.0,
        false,
    );
    let sub_post_initial = apply_chain(
        &aligned_curves[sub_role],
        &lp_biquads,
        sub_gain_post,
        0.0,
        sub_inverted,
    );

    // Re-align Subwoofer level after crossover application
    // Calculate mean SPL of filtered curves to ensure levels match at crossover.
    // Each curve has its own frequency grid, so use the correct one for each.
    let main_freqs_f32: Vec<f32> = l_post.freq.iter().map(|&f| f as f32).collect();
    let main_spl_f32: Vec<f32> = l_post.spl.iter().map(|&s| s as f32).collect();
    let sub_freqs_f32: Vec<f32> = sub_post_initial.freq.iter().map(|&f| f as f32).collect();
    let sub_spl_f32: Vec<f32> = sub_post_initial.spl.iter().map(|&s| s as f32).collect();

    // Mains: measure above crossover
    let main_mean = compute_average_response(
        &main_freqs_f32,
        &main_spl_f32,
        Some((final_xo_freq as f32, 2000.0)),
    ) as f64;

    // Sub: measure below crossover (full passband)
    let sub_mean = compute_average_response(
        &sub_freqs_f32,
        &sub_spl_f32,
        Some((20.0, final_xo_freq as f32)),
    ) as f64;

    let sub_correction = main_mean - sub_mean;
    info!(
        "  Re-aligning Subwoofer: Main={:.2} dB, Sub={:.2} dB, Correction={:+.2} dB",
        main_mean, sub_mean, sub_correction
    );

    // Apply correction
    let mut sub_post = sub_post_initial.clone();
    for s in sub_post.spl.iter_mut() {
        *s += sub_correction;
    }

    let sub_gain_post = sub_gain_post + sub_correction;

    // 7. Post-EQ (Global)
    // L/R: min_freq = xover + 20
    // Sub: max_freq = xover - 20
    let mut post_eq_filters = HashMap::new();

    for role in ["L", "R"] {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = final_xo_freq + 20.0;

        let post_curve = if role == "L" { &l_post } else { &r_post };
        let (filters, _) = eq::optimize_channel_eq(
            post_curve,
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;
        post_eq_filters.insert(role.to_string(), filters);
    }

    // Sub Post-EQ
    {
        let mut opt_config = config.optimizer.clone();
        opt_config.max_freq = final_xo_freq - 20.0;
        let sub_min_score = config.optimizer.min_freq.max(20.0);
        let (filters, _) = eq::optimize_channel_eq(
            &sub_post,
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        // "Do no harm" guard: discard Post-EQ if it makes the sub worse
        // (e.g., cardioid subs with steep low-frequency rolloff)
        let pre = compute_flat_loss(&sub_post, sub_min_score, final_xo_freq);
        let eq_resp = response::compute_peq_complex_response(&filters, &sub_post.freq, sample_rate);
        let sub_after_eq = response::apply_complex_response(&sub_post, &eq_resp);
        let post = compute_flat_loss(&sub_after_eq, sub_min_score, final_xo_freq);
        if post < pre {
            post_eq_filters.insert(sub_role.to_string(), filters);
        } else {
            log::warn!(
                "  Sub Post-EQ discarded: score regressed from {:.4} to {:.4}",
                pre,
                post
            );
        }
    }

    // 8. Construct Output Chains
    let mut channel_chains = HashMap::new();

    // L/R Chain: AlignGain -> Crossover(HP) -> MainGain -> Delay -> PostEQ
    for role in ["L", "R"] {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(align_gain));
        }

        // Pre-EQ removed per user request (optimization relies on Post-EQ)

        // Crossover HP
        plugins.push(output::create_crossover_plugin(
            xover_type_str,
            final_xo_freq,
            "high",
        ));

        // Main Post Gain
        if main_gain_post.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(main_gain_post));
        }

        // Main delay from crossover optimizer (sub-main time alignment)
        if main_delay_post.abs() > 0.01 {
            plugins.push(output::create_delay_plugin(main_delay_post));
        }

        let eqs = post_eq_filters.get(role);
        if let Some(e) = eqs {
            plugins.push(output::create_eq_plugin(e));
        }

        // Compute final curve
        let intermediate = if role == "L" { &l_post } else { &r_post };
        let final_curve_obj = if let Some(e) = eqs {
            let resp = response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
            response::apply_complex_response(intermediate, &resp)
        } else {
            intermediate.clone()
        };

        let initial_data: super::types::CurveData = (&aligned_curves[role]).into();
        let final_data: super::types::CurveData = (&final_curve_obj).into();
        let eq_resp = super::output::compute_eq_response(&initial_data, &final_data);
        let chain = ChannelDspChain {
            channel: role.to_string(),
            plugins,
            drivers: None,
            initial_curve: Some(initial_data),
            final_curve: Some(final_data),
            eq_response: Some(eq_resp),
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        };
        channel_chains.insert(role.to_string(), chain);
    }

    // Sub Chain: AlignGain -> Crossover(LP) -> SubGain(Invert) -> PostEQ
    // With optional per-driver chains for multi-sub configurations
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin(sub_align_gain));
    }

    sub_plugins.push(output::create_crossover_plugin(
        xover_type_str,
        final_xo_freq,
        "low",
    ));

    // Sub Gain + Invert
    if sub_inverted || sub_gain_post.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin_with_invert(
            sub_gain_post,
            sub_inverted,
        ));
    }

    // Sub delay from crossover optimizer (sub-main time alignment)
    if sub_delay_post.abs() > 0.01 {
        sub_plugins.push(output::create_delay_plugin(sub_delay_post));
    }

    let sub_eqs = post_eq_filters.get(sub_role);
    if let Some(e) = sub_eqs {
        sub_plugins.push(output::create_eq_plugin(e));
    }

    // Compute final curve
    let final_sub_curve = if let Some(e) = sub_eqs {
        let resp = response::compute_peq_complex_response(e, &sub_post.freq, sample_rate);
        response::apply_complex_response(&sub_post, &resp)
    } else {
        sub_post.clone()
    };

    // Build per-driver chains if multi-sub
    let driver_chains = sub_preprocess.drivers.as_ref().map(|drivers| {
        drivers
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let mut driver_plugins = Vec::new();
                if d.inverted || d.gain.abs() > 0.01 {
                    if d.inverted {
                        driver_plugins.push(output::create_gain_plugin_with_invert(d.gain, true));
                    } else {
                        driver_plugins.push(output::create_gain_plugin(d.gain));
                    }
                }
                if d.delay.abs() > 0.001 {
                    driver_plugins.push(output::create_delay_plugin(d.delay));
                }
                let driver_curve = d
                    .initial_curve
                    .as_ref()
                    .map(output::extend_curve_to_full_range)
                    .map(|c| (&c).into());
                DriverDspChain {
                    name: d.name.clone(),
                    index: i,
                    plugins: driver_plugins,
                    initial_curve: driver_curve,
                }
            })
            .collect()
    });

    let sub_initial_data: super::types::CurveData = (&aligned_curves[sub_role]).into();
    let sub_final_data: super::types::CurveData = (&final_sub_curve).into();
    let sub_eq_resp = super::output::compute_eq_response(&sub_initial_data, &sub_final_data);
    let sub_chain = ChannelDspChain {
        channel: sub_role.to_string(),
        plugins: sub_plugins,
        drivers: driver_chains,
        initial_curve: Some(sub_initial_data),
        final_curve: Some(sub_final_data),
        eq_response: Some(sub_eq_resp),
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    };
    channel_chains.insert(sub_role.to_string(), sub_chain);

    // Compute scores per channel
    // Each channel is scored in its operating range:
    //   L/R: above crossover frequency (HP-filtered by crossover)
    //   Sub: below crossover frequency (LP-filtered by crossover)
    // Pre-score baseline uses the post-crossover curve (before post-EQ),
    // so pre vs post measures the improvement from post-EQ alone.
    let max_freq = config.optimizer.max_freq;
    let sub_min_score = config.optimizer.min_freq.max(20.0);
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in ["L", "R"] {
        let intermediate = if role == "L" { &l_post } else { &r_post };
        let pre_score = compute_flat_loss(intermediate, final_xo_freq, max_freq);
        let final_curve_obj = if let Some(e) = post_eq_filters.get(role) {
            let resp = response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
            response::apply_complex_response(intermediate, &resp)
        } else {
            intermediate.clone()
        };
        let post_score = compute_flat_loss(&final_curve_obj, final_xo_freq, max_freq);

        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(
            role.to_string(),
            ChannelOptimizationResult {
                name: role.to_string(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[role].clone(),
                final_curve: final_curve_obj,
                biquads: post_eq_filters.get(role).cloned().unwrap_or_default(),
                fir_coeffs: None,
            },
        );
    }

    // Sub channel
    {
        let pre_score = compute_flat_loss(&sub_post, sub_min_score, final_xo_freq);
        let post_score = compute_flat_loss(&final_sub_curve, sub_min_score, final_xo_freq);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(
            sub_role.to_string(),
            ChannelOptimizationResult {
                name: sub_role.to_string(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[sub_role].clone(),
                final_curve: final_sub_curve.clone(),
                biquads: post_eq_filters.get(sub_role).cloned().unwrap_or_default(),
                fir_coeffs: None,
            },
        );
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
        },
    })
}

/// Workflow for Home Cinema X.0 / X.1 (any channel count)
///
/// Handles all standard layouts: 5.0, 5.1, 7.1, 9.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6.
/// The workflow is layout-agnostic: channels are classified as "main" (everything except LFE)
/// and "sub" (LFE if present). The specific channel names don't affect the algorithm.
pub fn optimize_home_cinema(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    _output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    let sub_role = "LFE";
    let has_sub = sys.speakers.contains_key(sub_role);

    // Classify channels into main and sub
    let main_roles: Vec<String> = sys
        .speakers
        .keys()
        .filter(|r| *r != sub_role)
        .cloned()
        .collect();

    info!(
        "Running Home Cinema Optimization Workflow ({} mains{})",
        main_roles.len(),
        if has_sub { " + LFE" } else { "" }
    );

    // 1. Load main channel measurements
    let mut curves = HashMap::new();
    for role in &main_roles {
        let meas_key = sys
            .speakers
            .get(role)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker mapping for '{}'", role),
            })?;
        let cfg = config
            .speakers
            .get(meas_key)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker config for key '{}'", meas_key),
            })?;
        let source = match cfg {
            SpeakerConfig::Single(s) => s,
            _ => {
                return Err(AutoeqError::InvalidConfiguration {
                    message: format!(
                        "'{}' must be a Single speaker config in home cinema workflow",
                        role
                    ),
                });
            }
        };
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        curves.insert(role.clone(), curve);
    }

    // Load LFE if present (handles Single, MultiSub/MSO, Cardioid, DBA)
    let sub_preprocess = if has_sub {
        let sub_sys = sys
            .subwoofers
            .as_ref()
            .ok_or(AutoeqError::InvalidConfiguration {
                message: "Missing subwoofers configuration for home cinema with LFE".to_string(),
            })?;
        let lfe_meas_key = sys
            .speakers
            .get(sub_role)
            .ok_or(AutoeqError::InvalidConfiguration {
                message: "Missing speaker mapping for 'LFE'".to_string(),
            })?;
        let lfe_speaker_config =
            config
                .speakers
                .get(lfe_meas_key)
                .ok_or(AutoeqError::InvalidConfiguration {
                    message: format!("Missing speaker config for key '{}'", lfe_meas_key),
                })?;
        let sp = preprocess_sub(
            lfe_speaker_config,
            &sub_sys.config,
            &config.optimizer,
            sample_rate,
        )?;
        curves.insert(sub_role.to_string(), sp.combined_curve.clone());
        Some(sp)
    } else {
        None
    };

    if has_sub {
        optimize_home_cinema_with_sub(
            config,
            sys,
            &main_roles,
            &curves,
            sub_preprocess.unwrap(),
            sample_rate,
        )
    } else {
        optimize_home_cinema_no_sub(config, &main_roles, &curves, sample_rate)
    }
}

/// Home Cinema X.0 (no subwoofer): per-channel EQ optimization
fn optimize_home_cinema_no_sub(
    config: &RoomConfig,
    main_roles: &[String],
    curves: &HashMap<String, Curve>,
    sample_rate: f64,
) -> Result<RoomOptimizationResult> {
    // Level alignment: mains measured from 100 Hz to 2000 Hz
    let mut ranges = HashMap::new();
    for role in main_roles {
        ranges.insert(role.clone(), (100.0, 2000.0));
    }
    let gains = align_channels_to_lowest(curves, &ranges);

    let min_freq = config.optimizer.min_freq;
    let max_freq = config.optimizer.max_freq;
    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in main_roles {
        let curve = &curves[role];
        let gain = *gains.get(role).unwrap_or(&0.0);

        let mut aligned_curve = curve.clone();
        for s in aligned_curve.spl.iter_mut() {
            *s += gain;
        }

        let pre_score = compute_flat_loss(&aligned_curve, min_freq, max_freq);
        info!(
            "  Optimizing '{}' with alignment gain {:.2} dB (pre_score={:.4})",
            role, gain, pre_score
        );

        let (filters, _loss) = super::optimize::optimize_eq_with_optional_schroeder(
            &aligned_curve,
            &config.optimizer,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        let mut plugins = Vec::new();
        if gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(gain));
        }
        if !filters.is_empty() {
            plugins.push(output::create_eq_plugin(&filters));
        }

        let resp =
            response::compute_peq_complex_response(&filters, &aligned_curve.freq, sample_rate);
        let final_curve_obj = response::apply_complex_response(&aligned_curve, &resp);
        let post_score = compute_flat_loss(&final_curve_obj, min_freq, max_freq);

        info!("  '{}' post_score={:.4}", role, post_score);

        let initial_data: super::types::CurveData = (&aligned_curve).into();
        let final_data: super::types::CurveData = (&final_curve_obj).into();
        let eq_resp = super::output::compute_eq_response(&initial_data, &final_data);
        let chain = ChannelDspChain {
            channel: role.clone(),
            plugins,
            drivers: None,
            initial_curve: Some(initial_data),
            final_curve: Some(final_data),
            eq_response: Some(eq_resp),
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        };

        channel_chains.insert(role.clone(), chain);
        pre_scores.push(pre_score);
        post_scores.push(post_score);

        channel_results.insert(
            role.clone(),
            ChannelOptimizationResult {
                name: role.clone(),
                pre_score,
                post_score,
                initial_curve: curve.clone(),
                final_curve: final_curve_obj,
                biquads: filters,
                fir_coeffs: None,
            },
        );
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
        },
    })
}

/// Home Cinema X.1 (with subwoofer): crossover management + per-channel EQ
fn optimize_home_cinema_with_sub(
    config: &RoomConfig,
    sys: &SystemConfig,
    main_roles: &[String],
    curves: &HashMap<String, Curve>,
    sub_preprocess: SubPreprocessResult,
    sample_rate: f64,
) -> Result<RoomOptimizationResult> {
    let sub_role = "LFE";

    // Resolve crossover config
    let sub_sys = sys.subwoofers.as_ref().unwrap();
    let xover_key = sub_sys
        .crossover
        .as_deref()
        .ok_or(AutoeqError::InvalidConfiguration {
            message: "Subwoofer config requires 'crossover' reference".to_string(),
        })?;
    let xover_config = config
        .crossovers
        .as_ref()
        .and_then(|m| m.get(xover_key))
        .ok_or(AutoeqError::InvalidConfiguration {
            message: format!("Crossover '{}' not found in crossovers section", xover_key),
        })?;
    let xover_type_str = &xover_config.crossover_type;

    let (min_xo, max_xo, est_xo) = if let Some(f) = xover_config.frequency {
        (f, f, f)
    } else if let Some((min, max)) = xover_config.frequency_range {
        (min, max, (min * max).sqrt())
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: "Subwoofer crossover requires 'frequency' or 'frequency_range'".to_string(),
        });
    };

    // 1. Level alignment
    let mut ranges = HashMap::new();
    for role in main_roles {
        ranges.insert(role.clone(), (max_xo, 2000.0));
    }
    let sub_min_align = config.optimizer.min_freq.max(20.0);
    ranges.insert(sub_role.to_string(), (sub_min_align, max_xo));

    let gains = align_channels_to_lowest(curves, &ranges);

    let mut aligned_curves = HashMap::new();
    for (role, curve) in curves {
        let mut c = curve.clone();
        let g = *gains.get(role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() {
            *s += g;
        }
        aligned_curves.insert(role.clone(), c);
    }

    // 2. Pre-EQ linearization for all main channels
    let mut pre_eq_filters = HashMap::new();
    let mut linearized_curves = aligned_curves.clone();

    for role in main_roles {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = min_xo;

        info!(
            "  Pre-EQ Linearization for '{}' (min {:.1} Hz)",
            role, min_xo
        );
        let (filters, _) = eq::optimize_channel_eq(
            &aligned_curves[role],
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        let resp = response::compute_peq_complex_response(
            &filters,
            &aligned_curves[role].freq,
            sample_rate,
        );
        let linear = response::apply_complex_response(&aligned_curves[role], &resp);

        pre_eq_filters.insert(role.clone(), filters);
        linearized_curves.insert(role.clone(), linear);
    }

    // 3. Virtual Main = dB average of all linearized main channels
    let ref_curve = &linearized_curves[&main_roles[0]];
    let mut virtual_main = ref_curve.clone();
    for i in 0..virtual_main.spl.len() {
        let sum: f64 = main_roles.iter().map(|r| linearized_curves[r].spl[i]).sum();
        virtual_main.spl[i] = sum / main_roles.len() as f64;
    }

    // 4. Crossover optimization between Virtual Main and LFE
    let sub_curve = &linearized_curves[sub_role];

    let crossover_type_enum = crossover::parse_crossover_type(xover_type_str).map_err(|e| {
        AutoeqError::InvalidConfiguration {
            message: e.to_string(),
        }
    })?;

    let (fixed_freqs, range_opt) = if xover_config.frequency.is_some() {
        (Some(vec![est_xo]), None)
    } else {
        (None, Some((min_xo, max_xo)))
    };

    // The crossover optimizer should only optimize delay and polarity, not gains.
    // Level matching is handled by alignment (step 1) and re-alignment (step 4).
    let mut xo_optimizer_config = config.optimizer.clone();
    xo_optimizer_config.min_db = 0.0;
    xo_optimizer_config.max_db = 0.0;

    let (xo_gains, xo_delays, xo_freqs, _, inversions) = crossover::optimize_crossover(
        vec![virtual_main.clone(), sub_curve.clone()],
        crossover_type_enum,
        sample_rate,
        &xo_optimizer_config,
        fixed_freqs,
        range_opt,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: e.to_string(),
    })?;

    let main_gain_post = xo_gains[0];
    let main_delay_post = xo_delays[0];
    let sub_gain_post = xo_gains[1];
    let sub_delay_post = xo_delays[1];
    let sub_inverted = inversions[1];
    let final_xo_freq = xo_freqs[0];

    info!(
        "  Crossover Optimized: Freq={:.1} Hz, Main Gain={:.2}, Sub Gain={:.2}, Main Delay={:.2}, Sub Delay={:.2}",
        final_xo_freq, main_gain_post, sub_gain_post, main_delay_post, sub_delay_post
    );

    // 5. Apply crossover filters
    let hp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, true);

    let apply_chain = |curve: &Curve, filters: &[Biquad], gain: f64| -> Curve {
        let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
        let mut c = response::apply_complex_response(curve, &resp);
        for s in c.spl.iter_mut() {
            *s += gain;
        }
        c
    };

    // Post-crossover curves for all mains and sub
    let mut main_post_curves = HashMap::new();
    for role in main_roles {
        let post = apply_chain(&aligned_curves[role], &hp_biquads, main_gain_post);
        main_post_curves.insert(role.clone(), post);
    }
    let sub_post_initial = apply_chain(&aligned_curves[sub_role], &lp_biquads, sub_gain_post);

    // Re-align sub level post-crossover (use first main as reference)
    // Each curve has its own frequency grid, so use the correct one for each.
    let ref_main_post = &main_post_curves[&main_roles[0]];
    let main_freqs_f32: Vec<f32> = ref_main_post.freq.iter().map(|&f| f as f32).collect();
    let main_spl_f32: Vec<f32> = ref_main_post.spl.iter().map(|&s| s as f32).collect();
    let sub_freqs_f32: Vec<f32> = sub_post_initial.freq.iter().map(|&f| f as f32).collect();
    let sub_spl_f32: Vec<f32> = sub_post_initial.spl.iter().map(|&s| s as f32).collect();

    let main_mean = math_audio_dsp::analysis::compute_average_response(
        &main_freqs_f32,
        &main_spl_f32,
        Some((final_xo_freq as f32, 2000.0)),
    ) as f64;
    let sub_mean = math_audio_dsp::analysis::compute_average_response(
        &sub_freqs_f32,
        &sub_spl_f32,
        Some((20.0, final_xo_freq as f32)),
    ) as f64;

    let sub_correction = main_mean - sub_mean;
    info!(
        "  Re-aligning Subwoofer: Main={:.2} dB, Sub={:.2} dB, Correction={:+.2} dB",
        main_mean, sub_mean, sub_correction
    );

    let mut sub_post = sub_post_initial.clone();
    for s in sub_post.spl.iter_mut() {
        *s += sub_correction;
    }
    let sub_gain_post = sub_gain_post + sub_correction;

    // 6. Post-EQ
    let mut post_eq_filters = HashMap::new();

    for role in main_roles {
        let mut opt_config = config.optimizer.clone();
        opt_config.min_freq = final_xo_freq + 20.0;

        let (filters, _) = eq::optimize_channel_eq(
            &main_post_curves[role],
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;
        post_eq_filters.insert(role.clone(), filters);
    }

    // Sub Post-EQ
    {
        let mut opt_config = config.optimizer.clone();
        opt_config.max_freq = final_xo_freq - 20.0;
        let sub_min_score = config.optimizer.min_freq.max(20.0);
        let (filters, _) = eq::optimize_channel_eq(
            &sub_post,
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        // "Do no harm" guard: discard Post-EQ if it makes the sub worse
        let pre = compute_flat_loss(&sub_post, sub_min_score, final_xo_freq);
        let eq_resp = response::compute_peq_complex_response(&filters, &sub_post.freq, sample_rate);
        let sub_after_eq = response::apply_complex_response(&sub_post, &eq_resp);
        let post = compute_flat_loss(&sub_after_eq, sub_min_score, final_xo_freq);
        if post < pre {
            post_eq_filters.insert(sub_role.to_string(), filters);
        } else {
            log::warn!(
                "  Sub Post-EQ discarded: score regressed from {:.4} to {:.4}",
                pre,
                post
            );
        }
    }

    // 7. Build output chains
    let mut channel_chains = HashMap::new();

    // Main channels: AlignGain -> Crossover(HP) -> MainGain -> Delay -> PostEQ
    for role in main_roles {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(align_gain));
        }

        plugins.push(output::create_crossover_plugin(
            xover_type_str,
            final_xo_freq,
            "high",
        ));

        if main_gain_post.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(main_gain_post));
        }

        // Main delay from crossover optimizer (sub-main time alignment)
        if main_delay_post.abs() > 0.01 {
            plugins.push(output::create_delay_plugin(main_delay_post));
        }

        let eqs = post_eq_filters.get(role);
        if let Some(e) = eqs
            && !e.is_empty()
        {
            plugins.push(output::create_eq_plugin(e));
        }

        let intermediate = &main_post_curves[role];
        let final_curve_obj = if let Some(e) = eqs {
            if !e.is_empty() {
                let resp =
                    response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
                response::apply_complex_response(intermediate, &resp)
            } else {
                intermediate.clone()
            }
        } else {
            intermediate.clone()
        };

        let initial_data: super::types::CurveData = (&aligned_curves[role]).into();
        let final_data: super::types::CurveData = (&final_curve_obj).into();
        let eq_resp = super::output::compute_eq_response(&initial_data, &final_data);
        let chain = ChannelDspChain {
            channel: role.clone(),
            plugins,
            drivers: None,
            initial_curve: Some(initial_data),
            final_curve: Some(final_data),
            eq_response: Some(eq_resp),
            pre_ir: None,
            post_ir: None,
            target_curve: None,
        };
        channel_chains.insert(role.clone(), chain);
    }

    // Sub chain: AlignGain -> Crossover(LP) -> SubGain(Invert) -> Delay -> PostEQ
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin(sub_align_gain));
    }

    sub_plugins.push(output::create_crossover_plugin(
        xover_type_str,
        final_xo_freq,
        "low",
    ));

    if sub_inverted || sub_gain_post.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin_with_invert(
            sub_gain_post,
            sub_inverted,
        ));
    }

    // Sub delay from crossover optimizer (sub-main time alignment)
    if sub_delay_post.abs() > 0.01 {
        sub_plugins.push(output::create_delay_plugin(sub_delay_post));
    }

    let sub_eqs = post_eq_filters.get(sub_role);
    if let Some(e) = sub_eqs
        && !e.is_empty()
    {
        sub_plugins.push(output::create_eq_plugin(e));
    }

    let final_sub_curve = if let Some(e) = sub_eqs {
        if !e.is_empty() {
            let resp = response::compute_peq_complex_response(e, &sub_post.freq, sample_rate);
            response::apply_complex_response(&sub_post, &resp)
        } else {
            sub_post.clone()
        }
    } else {
        sub_post.clone()
    };

    // Build per-driver chains if multi-sub
    let driver_chains = sub_preprocess.drivers.as_ref().map(|drivers| {
        drivers
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let mut driver_plugins = Vec::new();
                if d.inverted || d.gain.abs() > 0.01 {
                    if d.inverted {
                        driver_plugins.push(output::create_gain_plugin_with_invert(d.gain, true));
                    } else {
                        driver_plugins.push(output::create_gain_plugin(d.gain));
                    }
                }
                if d.delay.abs() > 0.001 {
                    driver_plugins.push(output::create_delay_plugin(d.delay));
                }
                let driver_curve = d
                    .initial_curve
                    .as_ref()
                    .map(output::extend_curve_to_full_range)
                    .map(|c| (&c).into());
                DriverDspChain {
                    name: d.name.clone(),
                    index: i,
                    plugins: driver_plugins,
                    initial_curve: driver_curve,
                }
            })
            .collect()
    });

    let sub_initial_data: super::types::CurveData = (&aligned_curves[sub_role]).into();
    let sub_final_data: super::types::CurveData = (&final_sub_curve).into();
    let sub_eq_resp = super::output::compute_eq_response(&sub_initial_data, &sub_final_data);
    let sub_chain = ChannelDspChain {
        channel: sub_role.to_string(),
        plugins: sub_plugins,
        drivers: driver_chains,
        initial_curve: Some(sub_initial_data),
        final_curve: Some(sub_final_data),
        eq_response: Some(sub_eq_resp),
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    };
    channel_chains.insert(sub_role.to_string(), sub_chain);

    // 8. Compute scores
    let max_freq = config.optimizer.max_freq;
    let sub_min_score = config.optimizer.min_freq.max(20.0);
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in main_roles {
        let intermediate = &main_post_curves[role];
        let pre_score = compute_flat_loss(intermediate, final_xo_freq, max_freq);
        let final_curve_obj = if let Some(e) = post_eq_filters.get(role) {
            if !e.is_empty() {
                let resp =
                    response::compute_peq_complex_response(e, &intermediate.freq, sample_rate);
                response::apply_complex_response(intermediate, &resp)
            } else {
                intermediate.clone()
            }
        } else {
            intermediate.clone()
        };
        let post_score = compute_flat_loss(&final_curve_obj, final_xo_freq, max_freq);

        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(
            role.clone(),
            ChannelOptimizationResult {
                name: role.clone(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[role].clone(),
                final_curve: final_curve_obj,
                biquads: post_eq_filters.get(role).cloned().unwrap_or_default(),
                fir_coeffs: None,
            },
        );
    }

    // Sub channel score
    {
        let pre_score = compute_flat_loss(&sub_post, sub_min_score, final_xo_freq);
        let post_score = compute_flat_loss(&final_sub_curve, sub_min_score, final_xo_freq);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
        channel_results.insert(
            sub_role.to_string(),
            ChannelOptimizationResult {
                name: sub_role.to_string(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[sub_role].clone(),
                final_curve: final_sub_curve.clone(),
                biquads: post_eq_filters.get(sub_role).cloned().unwrap_or_default(),
                fir_coeffs: None,
            },
        );
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
        },
    })
}

fn create_crossover_filters(
    type_str: &str,
    freq: f64,
    sample_rate: f64,
    is_lowpass: bool,
) -> Vec<Biquad> {
    use math_audio_iir_fir::*;
    let type_lower = type_str.to_lowercase();
    let peq = match type_lower.as_str() {
        "lr24" | "lr4" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(4, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(4, freq, sample_rate)
            }
        }
        "lr48" | "lr8" => {
            if is_lowpass {
                peq_linkwitzriley_lowpass(8, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(8, freq, sample_rate)
            }
        }
        "bw12" | "butterworth12" => {
            if is_lowpass {
                peq_butterworth_lowpass(2, freq, sample_rate)
            } else {
                peq_butterworth_highpass(2, freq, sample_rate)
            }
        }
        "bw24" | "butterworth24" => {
            if is_lowpass {
                peq_butterworth_lowpass(4, freq, sample_rate)
            } else {
                peq_butterworth_highpass(4, freq, sample_rate)
            }
        }
        _ => {
            log::warn!("Unknown crossover type '{}', defaulting to LR24", type_str);
            if is_lowpass {
                peq_linkwitzriley_lowpass(4, freq, sample_rate)
            } else {
                peq_linkwitzriley_highpass(4, freq, sample_rate)
            }
        }
    };
    peq.into_iter().map(|(_, b)| b).collect()
}
