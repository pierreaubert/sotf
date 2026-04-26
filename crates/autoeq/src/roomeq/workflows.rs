//! Specific optimization workflows for different system topologies.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read::load_source;
use crate::response;
use log::info;
use math_audio_dsp::analysis::compute_average_response;
use math_audio_iir_fir::Biquad;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::crossover;
use super::dba;
use super::eq;
use super::multisub;
use super::optimize::{ChannelOptimizationResult, RoomOptimizationResult};
use super::output;
use super::types::{
    CardioidConfig, ChannelDspChain, CrossoverConfig, DBAConfig, DriverDspChain, MultiSubGroup,
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

/// Runs a single channel through `process_single_speaker` and prepends an
/// alignment-gain plugin to the returned DSP chain.
///
/// This is the Phase 3 feature-parity bridge: the generic per-channel path
/// honours every `OptimizerConfig` feature (excursion protection, target
/// tilt/response, broadband matching, CEA2034 correction). Workflows that
/// used to call `eq::optimize_channel_eq` directly bypassed all of them.
/// By routing each channel through `process_single_speaker` with the
/// original `MeasurementSource` (so `speaker_name` propagates to CEA2034)
/// and a config clone carrying any workflow-specific frequency overrides,
/// the workflow inherits the full feature matrix.
///
/// The alignment gain is not applied to the curve itself — it is added as a
/// plugin at the head of the chain. `process_single_speaker`'s internal
/// decisions (F3 detection, passband estimation, target tilt, etc.) use
/// relative-to-peak thresholds that are gain-invariant, so passing the raw
/// curve is equivalent to passing an aligned one.
///
/// `config_override` lets stereo 2.1 / home-cinema-with-sub clone
/// `config` and narrow `optimizer.min_freq` / `max_freq` to the band of
/// interest (e.g. Pre-EQ at `min_xo`) before the delegation call.
#[allow(clippy::type_complexity)]
fn run_channel_via_generic_path(
    role: &str,
    source: &crate::MeasurementSource,
    config: &RoomConfig,
    alignment_gain_db: f64,
    sample_rate: f64,
    output_dir: &Path,
) -> Result<(
    ChannelDspChain,
    ChannelOptimizationResult,
    f64,
    f64,
    Option<Vec<f64>>,
)> {
    let (
        raw_chain,
        pre_score,
        post_score,
        initial_curve,
        final_curve,
        biquads,
        _mean_spl,
        _arrival_ms,
        fir_coeffs,
    ) = super::speaker_eq::process_single_speaker(
        role,
        source,
        config,
        sample_rate,
        output_dir,
        None,
        None,
        None,
    )?;

    // Prepend the alignment gain plugin without touching the inner chain's
    // existing plugins (excursion HPF, broadband shelf, CEA2034 PEQ, fine EQ).
    let mut plugins: Vec<_> = Vec::with_capacity(raw_chain.plugins.len() + 1);
    if alignment_gain_db.abs() > 0.01 {
        plugins.push(output::create_gain_plugin(alignment_gain_db));
    }
    plugins.extend(raw_chain.plugins);

    let chain = ChannelDspChain {
        channel: role.to_string(),
        plugins,
        drivers: raw_chain.drivers,
        initial_curve: raw_chain.initial_curve,
        final_curve: raw_chain.final_curve,
        eq_response: raw_chain.eq_response,
        pre_ir: raw_chain.pre_ir,
        post_ir: raw_chain.post_ir,
        target_curve: raw_chain.target_curve,
    };

    let channel_result = ChannelOptimizationResult {
        name: role.to_string(),
        pre_score,
        post_score,
        initial_curve,
        final_curve,
        biquads,
        fir_coeffs: fir_coeffs.clone(),
    };

    Ok((chain, channel_result, pre_score, post_score, fir_coeffs))
}

/// Coherent (complex) sum of N main channels, used by the stereo-2.1 and
/// home-cinema-with-sub crossover optimizers.
///
/// The previous per-bin SPL average with a discarded/averaged phase hid
/// inter-channel phase mismatches from the crossover / group-delay loss
/// (B8). Using the complex sum preserves phase coherence the same way
/// `preprocess_cardioid` does for the front/rear sub pair.
///
/// Callers must only use this for curves with measured/current phase. Bass
/// management is phase-critical, so the workflows skip delay/polarity
/// optimization instead of inventing 0 deg phase.
///
/// Expects every input curve to share the same frequency grid. Empty or
/// single-element input panics — callers always supply ≥ 1 main.
fn complex_sum_mains(curves: &[&Curve]) -> Curve {
    use num_complex::Complex;
    assert!(!curves.is_empty(), "complex_sum_mains needs ≥ 1 curve");
    let n = curves.iter().map(|c| c.spl.len()).min().unwrap();
    let freq = curves[0].freq.slice(ndarray::s![..n]).to_owned();
    let divisor = curves.len() as f64;

    let mut spl = ndarray::Array1::<f64>::zeros(n);
    let mut phase = ndarray::Array1::<f64>::zeros(n);
    for i in 0..n {
        let mut sum = Complex::new(0.0_f64, 0.0);
        for c in curves {
            let mag = 10.0_f64.powf(c.spl[i] / 20.0);
            let phi = c.phase.as_ref().expect("phase checked by caller")[i].to_radians();
            sum += Complex::from_polar(mag, phi);
        }
        sum /= divisor;
        spl[i] = 20.0 * sum.norm().max(1e-12).log10();
        phase[i] = sum.arg().to_degrees();
    }

    Curve {
        freq,
        spl,
        phase: Some(phase),
        ..Default::default()
    }
}

fn average_mains_magnitude(curves: &[&Curve]) -> Curve {
    assert!(
        !curves.is_empty(),
        "average_mains_magnitude needs >= 1 curve"
    );
    let ref_freq = curves[0].freq.clone();
    let mut spl = ndarray::Array1::<f64>::zeros(ref_freq.len());

    for curve in curves {
        let interpolated = crate::read::interpolate_log_space(&ref_freq, curve);
        spl += &interpolated.spl;
    }
    spl.mapv_inplace(|v| v / curves.len() as f64);

    Curve {
        freq: ref_freq,
        spl,
        phase: None,
        ..Default::default()
    }
}

fn curve_has_usable_phase(curve: &Curve) -> bool {
    curve
        .phase
        .as_ref()
        .map(|phase| phase.len() >= curve.freq.len() && phase.iter().all(|v| v.is_finite()))
        .unwrap_or(false)
}

fn all_curves_have_usable_phase(curves: &[&Curve]) -> bool {
    curves.iter().all(|curve| curve_has_usable_phase(curve))
}

fn all_curves_share_frequency_grid(curves: &[&Curve]) -> bool {
    let Some(reference) = curves.first() else {
        return false;
    };
    super::frequency_grid::is_valid_frequency_grid(&reference.freq)
        && curves.iter().skip(1).all(|curve| {
            super::frequency_grid::is_valid_frequency_grid(&curve.freq)
                && super::frequency_grid::same_frequency_grid(&reference.freq, &curve.freq)
        })
}

fn normalize_crossover_delays(main_delay_ms: f64, sub_delay_ms: f64) -> (f64, f64) {
    let common_delay_ms = main_delay_ms.min(sub_delay_ms);
    (
        main_delay_ms - common_delay_ms,
        sub_delay_ms - common_delay_ms,
    )
}

fn apply_delay_and_polarity_to_curve(curve: &Curve, delay_ms: f64, invert: bool) -> Curve {
    let mut adjusted = curve.clone();
    let Some(phase) = adjusted.phase.as_mut() else {
        return adjusted;
    };
    let delay_s = delay_ms / 1000.0;
    for (idx, phase_deg) in phase.iter_mut().enumerate() {
        let freq_hz = adjusted.freq[idx];
        *phase_deg -= 360.0 * freq_hz * delay_s;
        if invert {
            *phase_deg += 180.0;
        }
    }
    adjusted
}

#[allow(clippy::too_many_arguments)]
fn predict_bass_management_sum(
    main_curve: &Curve,
    sub_curve: &Curve,
    xover_type: &str,
    xover_freq: f64,
    sample_rate: f64,
    main_gain_db: f64,
    sub_gain_db: f64,
    main_delay_ms: f64,
    sub_delay_ms: f64,
    sub_inverted: bool,
) -> Option<Curve> {
    use num_complex::Complex;

    if !curve_has_usable_phase(main_curve) || !curve_has_usable_phase(sub_curve) {
        return None;
    }

    let sub_on_main_grid = crate::read::interpolate_log_space(&main_curve.freq, sub_curve);
    if !curve_has_usable_phase(&sub_on_main_grid) {
        return None;
    }

    let hp_biquads = create_crossover_filters(xover_type, xover_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type, xover_freq, sample_rate, true);
    let main_resp =
        response::compute_peq_complex_response(&hp_biquads, &main_curve.freq, sample_rate);
    let sub_resp =
        response::compute_peq_complex_response(&lp_biquads, &sub_on_main_grid.freq, sample_rate);

    let mut main_filtered = response::apply_complex_response(main_curve, &main_resp);
    for spl in main_filtered.spl.iter_mut() {
        *spl += main_gain_db;
    }
    let main_filtered = apply_delay_and_polarity_to_curve(&main_filtered, main_delay_ms, false);

    let mut sub_filtered = response::apply_complex_response(&sub_on_main_grid, &sub_resp);
    for spl in sub_filtered.spl.iter_mut() {
        *spl += sub_gain_db;
    }
    let sub_filtered = apply_delay_and_polarity_to_curve(&sub_filtered, sub_delay_ms, sub_inverted);

    let main_phase = main_filtered.phase.as_ref()?;
    let sub_phase = sub_filtered.phase.as_ref()?;
    let mut spl = ndarray::Array1::<f64>::zeros(main_filtered.freq.len());
    let mut phase = ndarray::Array1::<f64>::zeros(main_filtered.freq.len());

    for i in 0..main_filtered.freq.len() {
        let main = Complex::from_polar(
            10.0_f64.powf(main_filtered.spl[i] / 20.0),
            main_phase[i].to_radians(),
        );
        let sub = Complex::from_polar(
            10.0_f64.powf(sub_filtered.spl[i] / 20.0),
            sub_phase[i].to_radians(),
        );
        let sum = main + sub;
        spl[i] = 20.0 * sum.norm().max(1e-12).log10();
        phase[i] = sum.arg().to_degrees();
    }

    Some(Curve {
        freq: main_filtered.freq,
        spl,
        phase: Some(phase),
        ..Default::default()
    })
}

fn bass_management_objective(curve: Option<&Curve>, xover_freq: f64) -> Option<f64> {
    let curve = curve?;
    let min_freq = (xover_freq / 2.0).max(20.0);
    let max_freq = (xover_freq * 2.0).min(2_000.0).max(min_freq + 1.0);
    Some(compute_flat_loss(curve, min_freq, max_freq))
}

fn bass_management_crossover_type_candidates(requested: &str) -> Vec<String> {
    let requested = requested.trim();
    if requested.eq_ignore_ascii_case("auto") || requested.eq_ignore_ascii_case("optimize") {
        vec![
            "LR24".to_string(),
            "LR48".to_string(),
            "BW12".to_string(),
            "BW24".to_string(),
        ]
    } else {
        vec![requested.to_string()]
    }
}

fn select_bass_management_crossover_type(
    requested: &str,
    main_curve: &Curve,
    sub_curve: &Curve,
    xover_freq: f64,
    sample_rate: f64,
) -> String {
    let candidates = bass_management_crossover_type_candidates(requested);
    if candidates.len() == 1 {
        return candidates[0].clone();
    }

    candidates
        .iter()
        .filter(|candidate| candidate.parse::<crate::loss::CrossoverType>().is_ok())
        .filter_map(|candidate| {
            let predicted = predict_bass_management_sum(
                main_curve,
                sub_curve,
                candidate,
                xover_freq,
                sample_rate,
                0.0,
                0.0,
                0.0,
                0.0,
                false,
            );
            bass_management_objective(predicted.as_ref(), xover_freq)
                .map(|objective| (candidate.clone(), objective))
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(candidate, _)| candidate)
        .unwrap_or_else(|| "LR24".to_string())
}

/// Resolve the `MeasurementSource` for a logical role via the SystemConfig →
/// RoomConfig.speakers indirection. Workflows only accept `SpeakerConfig::Single`
/// roles; any other variant is rejected up front so the generic path doesn't
/// see half-processed data.
fn resolve_single_source<'a>(
    role: &str,
    config: &'a RoomConfig,
    sys: &SystemConfig,
) -> Result<&'a crate::MeasurementSource> {
    let meas_key = sys
        .speakers
        .get(role)
        .ok_or_else(|| AutoeqError::InvalidConfiguration {
            message: format!("Missing speaker mapping for '{}'", role),
        })?;
    let cfg = config
        .speakers
        .get(meas_key)
        .ok_or_else(|| AutoeqError::InvalidConfiguration {
            message: format!("Missing speaker config for key '{}'", meas_key),
        })?;
    match cfg {
        SpeakerConfig::Single(s) => Ok(s),
        _ => Err(AutoeqError::InvalidConfiguration {
            message: format!("Workflow requires Single speaker config for '{}'", role),
        }),
    }
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
        ..Default::default()
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
        ..Default::default()
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

#[derive(Debug, Clone)]
struct GroupCrossoverPlan {
    crossover_type: String,
    frequency_hz: f64,
    configured_hz: f64,
    frequency_range: Option<(f64, f64)>,
}

fn grouped_home_cinema_roles(main_roles: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in main_roles {
        let role_id =
            super::home_cinema::group_id_for_role(super::home_cinema::role_for_channel(role));
        groups
            .entry(role_id.to_string())
            .or_default()
            .push(role.clone());
    }
    groups
}

fn group_crossover_plan(
    config: &RoomConfig,
    fallback: &CrossoverConfig,
    group_id: &str,
) -> Result<GroupCrossoverPlan> {
    let selected = config
        .system
        .as_ref()
        .and_then(|system| system.bass_management.as_ref())
        .and_then(|bm| bm.group_crossovers.get(group_id))
        .and_then(|key| {
            config
                .crossovers
                .as_ref()
                .and_then(|crossovers| crossovers.get(key))
        })
        .unwrap_or(fallback);

    let (min_hz, max_hz, configured_hz) = if let Some(freq) = selected.frequency {
        (freq, freq, freq)
    } else if let Some((min, max)) = selected.frequency_range {
        (min, max, (min.max(1.0) * max.max(1.0)).sqrt())
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "Bass-management crossover for group '{group_id}' requires 'frequency' or 'frequency_range'"
            ),
        });
    };

    Ok(GroupCrossoverPlan {
        crossover_type: selected.crossover_type.clone(),
        frequency_hz: configured_hz,
        configured_hz,
        frequency_range: (min_hz != max_hz).then_some((min_hz, max_hz)),
    })
}

#[allow(clippy::too_many_arguments)]
fn optimize_home_cinema_group_crossovers(
    config: &RoomConfig,
    main_roles: &[String],
    aligned_curves: &HashMap<String, Curve>,
    aligned_pre_eq_curves: &HashMap<String, Curve>,
    sub_role: &str,
    fallback_crossover: &CrossoverConfig,
    sample_rate: f64,
    bass_management: Option<&super::home_cinema::EffectiveBassManagement>,
) -> Result<BTreeMap<String, super::home_cinema::BassManagementGroupReport>> {
    let mut reports = BTreeMap::new();
    let sub_curve = &aligned_pre_eq_curves[sub_role];

    for (group_id, roles) in grouped_home_cinema_roles(main_roles) {
        let mut advisories = Vec::new();
        let plan = group_crossover_plan(config, fallback_crossover, &group_id)?;
        let main_refs: Vec<&Curve> = roles
            .iter()
            .map(|role| &aligned_pre_eq_curves[role])
            .collect();
        let mut measured_refs: Vec<&Curve> =
            roles.iter().map(|role| &aligned_curves[role]).collect();
        measured_refs.push(&aligned_curves[sub_role]);
        let mut phase_refs = main_refs.clone();
        phase_refs.push(sub_curve);
        let measured_phase_available = all_curves_have_usable_phase(&measured_refs);
        let shared_grid_available = all_curves_share_frequency_grid(&measured_refs)
            && all_curves_share_frequency_grid(&phase_refs);
        let phase_available = measured_phase_available && shared_grid_available;

        if !measured_phase_available {
            advisories.push("missing_phase_group_crossover_alignment_skipped".to_string());
        } else if !shared_grid_available {
            advisories
                .push("frequency_grid_mismatch_group_crossover_alignment_skipped".to_string());
        }

        let virtual_main = if phase_available {
            complex_sum_mains(&main_refs)
        } else {
            average_mains_magnitude(&main_refs)
        };
        let selected_type = select_bass_management_crossover_type(
            &plan.crossover_type,
            &virtual_main,
            sub_curve,
            plan.frequency_hz,
            sample_rate,
        );
        let selected_type_str = selected_type.as_str();

        let objective_before_curve = predict_bass_management_sum(
            &virtual_main,
            sub_curve,
            selected_type_str,
            plan.frequency_hz,
            sample_rate,
            0.0,
            0.0,
            0.0,
            0.0,
            false,
        );
        let objective_before =
            bass_management_objective(objective_before_curve.as_ref(), plan.frequency_hz);

        let mut final_freq = plan.frequency_hz;
        let mut main_delay_ms = 0.0;
        let mut bass_delay_ms = 0.0;
        let mut polarity_inverted = false;
        let mut trim_db = 0.0;
        let mut objective_after = objective_before;

        if phase_available {
            let crossover_type_enum: crate::loss::CrossoverType = selected_type_str
                .parse()
                .map_err(|e: String| AutoeqError::InvalidConfiguration { message: e })?;
            let fixed_freqs = plan
                .frequency_range
                .is_none()
                .then_some(vec![plan.frequency_hz]);
            let mut xo_optimizer_config = config.optimizer.clone();
            xo_optimizer_config.min_db = 0.0;
            xo_optimizer_config.max_db = 0.0;
            let (xo_gains, xo_delays, xo_freqs, _, inversions) = crossover::optimize_crossover(
                vec![virtual_main.clone(), sub_curve.clone()],
                crossover_type_enum,
                sample_rate,
                &xo_optimizer_config,
                fixed_freqs,
                plan.frequency_range,
            )
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: e.to_string(),
            })?;

            final_freq = xo_freqs.first().copied().unwrap_or(plan.frequency_hz);
            let (main_delay, bass_delay) = normalize_crossover_delays(
                xo_delays.first().copied().unwrap_or(0.0),
                xo_delays.get(1).copied().unwrap_or(0.0),
            );
            main_delay_ms = main_delay;
            bass_delay_ms = bass_delay;
            polarity_inverted = inversions.get(1).copied().unwrap_or(false);

            let hp = create_crossover_filters(selected_type_str, final_freq, sample_rate, false);
            let lp = create_crossover_filters(selected_type_str, final_freq, sample_rate, true);
            let apply = |curve: &Curve, filters: &[Biquad], gain: f64, delay: f64, invert: bool| {
                let resp =
                    response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
                let mut c = response::apply_complex_response(curve, &resp);
                for spl in c.spl.iter_mut() {
                    *spl += gain;
                }
                apply_delay_and_polarity_to_curve(&c, delay, invert)
            };
            let main_post = apply(
                &virtual_main,
                &hp,
                xo_gains.first().copied().unwrap_or(0.0),
                main_delay_ms,
                false,
            );
            let sub_post = apply(
                sub_curve,
                &lp,
                xo_gains.get(1).copied().unwrap_or(0.0),
                bass_delay_ms,
                polarity_inverted,
            );
            let main_freqs: Vec<f32> = main_post.freq.iter().map(|&f| f as f32).collect();
            let main_spl: Vec<f32> = main_post.spl.iter().map(|&s| s as f32).collect();
            let sub_freqs: Vec<f32> = sub_post.freq.iter().map(|&f| f as f32).collect();
            let sub_spl: Vec<f32> = sub_post.spl.iter().map(|&s| s as f32).collect();
            let main_mean =
                compute_average_response(&main_freqs, &main_spl, Some((final_freq as f32, 2000.0)))
                    as f64;
            let sub_mean =
                compute_average_response(&sub_freqs, &sub_spl, Some((20.0, final_freq as f32)))
                    as f64;
            let requested_trim = xo_gains.get(1).copied().unwrap_or(0.0) + main_mean - sub_mean;
            let (limited_trim, gain_limited) =
                super::home_cinema::limited_sub_gain(requested_trim, bass_management);
            trim_db = limited_trim;
            if gain_limited {
                advisories.push("group_sub_trim_limited_for_headroom".to_string());
            }

            let objective_after_curve = predict_bass_management_sum(
                &virtual_main,
                sub_curve,
                selected_type_str,
                final_freq,
                sample_rate,
                xo_gains.first().copied().unwrap_or(0.0),
                trim_db,
                main_delay_ms,
                bass_delay_ms,
                polarity_inverted,
            );
            objective_after = bass_management_objective(objective_after_curve.as_ref(), final_freq);
            if objective_after >= objective_before {
                advisories.push("group_optimizer_no_improvement".to_string());
                final_freq = plan.frequency_hz;
                main_delay_ms = 0.0;
                bass_delay_ms = 0.0;
                polarity_inverted = false;
                trim_db = 0.0;
                objective_after = objective_before;
            }
        }

        if advisories.is_empty() {
            advisories.push("ok".to_string());
        }
        reports.insert(
            group_id.clone(),
            super::home_cinema::BassManagementGroupReport {
                group_id,
                roles,
                crossover_type: selected_type_str.to_string(),
                selected_crossover_hz: Some(final_freq),
                configured_crossover_hz: Some(plan.configured_hz),
                main_delay_ms,
                bass_route_delay_ms: bass_delay_ms,
                polarity_inverted,
                trim_db,
                objective_before,
                objective_after,
                advisories,
            },
        );
    }

    Ok(reports)
}

fn bass_management_sub_output_results(
    fallback_role: &str,
    drivers: Option<&[SubDriverInfo]>,
    shared_gain_db: f64,
    strategy: &SubwooferStrategy,
) -> Vec<super::home_cinema::BassManagementSubOutputReport> {
    let strategy_source = match strategy {
        SubwooferStrategy::Single => "single",
        SubwooferStrategy::Mso => "mso",
        SubwooferStrategy::Dba => "dba",
    };
    let Some(drivers) = drivers else {
        return vec![super::home_cinema::BassManagementSubOutputReport {
            output_role: fallback_role.to_string(),
            gain_db: 0.0,
            delay_ms: 0.0,
            polarity_inverted: false,
            strategy_source: strategy_source.to_string(),
            headroom_contribution_db: shared_gain_db,
        }];
    };

    drivers
        .iter()
        .map(|driver| super::home_cinema::BassManagementSubOutputReport {
            output_role: driver.name.clone(),
            gain_db: driver.gain,
            delay_ms: driver.delay,
            polarity_inverted: driver.inverted,
            strategy_source: if matches!(strategy, SubwooferStrategy::Dba) {
                if driver.inverted {
                    "dba_rear".to_string()
                } else {
                    "dba_front".to_string()
                }
            } else {
                strategy_source.to_string()
            },
            headroom_contribution_db: driver.gain + shared_gain_db,
        })
        .collect()
}

/// Workflow for Stereo 2.0 (No Subwoofer)
///
/// Per-channel EQ is delegated to `process_single_speaker` so that
/// `excursion_protection`, `target_response`, and `cea2034_correction`
/// all apply inside the workflow. An alignment-gain plugin is prepended
/// to the returned DSP chain without affecting feature decisions
/// (F3 detection, passband estimation, and target shaping all use
/// relative-to-peak thresholds that are gain-invariant).
pub fn optimize_stereo_2_0(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    output_dir: &Path,
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

    // 3. Optimization — delegate each channel to the generic path so features apply.
    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in curves.keys() {
        let gain = *gains.get(role).unwrap_or(&0.0);
        let source = resolve_single_source(role, config, sys)?;

        info!("  Optimizing '{}' with alignment gain {:.2} dB", role, gain);

        let (chain, ch_result, pre_score, post_score, _fir) =
            run_channel_via_generic_path(role, source, config, gain, sample_rate, output_dir)?;

        info!(
            "  '{}' pre_score={:.4} post_score={:.4}",
            role, pre_score, post_score
        );

        channel_chains.insert(role.clone(), chain);
        channel_results.insert(role.clone(), ch_result);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    let epa_per_channel = crate::roomeq::output::compute_epa_per_channel(&channel_chains, &epa_cfg);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            loss_type: Some(config.optimizer.loss_type.clone()),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
            epa_per_channel,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            bass_management: None,
            timing_diagnostics: None,
        },
    })
}

/// Workflow for Stereo 2.1 (With Subwoofer)
///
/// Phase 3b: per-channel features (`excursion_protection`, `target_response`,
/// `cea2034_correction`) are applied via `process_single_speaker` at the
/// Pre-EQ stage and the resulting plugin stack is inserted before the
/// crossover HP/LP in the final DSP chain. Post-EQ remains a plain cleanup
/// pass on the post-crossover curve, with the "do no harm" guard from
/// Phase 3a.
pub fn optimize_stereo_2_1(
    config: &RoomConfig,
    sys: &SystemConfig,
    sample_rate: f64,
    output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    info!("Running Stereo 2.1 Optimization Workflow");

    let sub_role = super::home_cinema::bass_output_role(config, sys);

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

    let lfe_meas_key =
        sys.speakers
            .get(sub_role.as_str())
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!("Missing speaker mapping for '{}'", sub_role),
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
    curves.insert(sub_role.clone(), sub_preprocess.combined_curve.clone());

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
    let bass_management = super::home_cinema::effective_bass_management(config);

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
    ranges.insert(sub_role.clone(), (sub_min_align, max_xo));

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

    // 3. Pre-EQ — route each channel through `process_single_speaker` so
    //    excursion / CEA2034 / broadband / target-response all apply. The
    //    returned plugin stack is kept as the per-channel "feature chain"
    //    that runs before crossover HP/LP in the final DSP assembly. The
    //    returned final_curve is the linearized, feature-corrected curve
    //    used to inform crossover optimization.
    //
    //    Mains: min_freq = min_xo so the optimizer focuses on the post-
    //    crossover band (the F3 clamp inside `process_single_speaker`
    //    still raises this further when the speaker's F3 > min_xo).
    //
    //    Sub: max_freq = max_xo so the optimizer focuses on the pre-
    //    crossover band. The sub is fed as an in-memory source — it
    //    carries no speaker_name, so CEA2034 correction is automatically
    //    skipped (subs aren't spinorama-shaped).
    let mut pre_eq_plugins: HashMap<String, Vec<super::types::PluginConfigWrapper>> =
        HashMap::new();
    let mut linearized_curves: HashMap<String, Curve> = HashMap::new();

    for role in ["L", "R"] {
        let source = resolve_single_source(role, config, sys)?;
        let mut per_config = config.clone();
        per_config.optimizer.min_freq = min_xo;

        info!(
            "  Pre-EQ via generic path for '{}' (min_freq={:.1} Hz)",
            role, min_xo
        );
        let (chain, ch_result, _pre_score, _post_score, _fir) =
            run_channel_via_generic_path(role, source, &per_config, 0.0, sample_rate, output_dir)?;
        pre_eq_plugins.insert(role.to_string(), chain.plugins);
        linearized_curves.insert(role.to_string(), ch_result.final_curve);
    }

    // Sub Pre-EQ: inline source with no speaker_name → CEA2034 skipped.
    {
        let sub_source = crate::MeasurementSource::InMemory(sub_preprocess.combined_curve.clone());
        let mut sub_config = config.clone();
        sub_config.optimizer.max_freq = max_xo;
        info!(
            "  Pre-EQ via generic path for '{}' (max_freq={:.1} Hz)",
            sub_role, max_xo
        );
        let (chain, ch_result, _pre_score, _post_score, _fir) = run_channel_via_generic_path(
            &sub_role,
            &sub_source,
            &sub_config,
            0.0,
            sample_rate,
            output_dir,
        )?;
        pre_eq_plugins.insert(sub_role.clone(), chain.plugins);
        linearized_curves.insert(sub_role.clone(), ch_result.final_curve);
    }

    // Aligned linearized curves: post-feature curves at the listening level
    // that the crossover optimizer operates on, and that `apply_chain`
    // below filters through the crossover HP/LP. Using these (instead of
    // the raw `aligned_curves`) is what makes the Post-EQ step see the
    // same curve the listener will hear after the feature stack.
    let mut aligned_pre_eq_curves: HashMap<String, Curve> = HashMap::new();
    for role in ["L", "R", sub_role.as_str()] {
        let mut c = linearized_curves[role].clone();
        let g = *gains.get(role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() {
            *s += g;
        }
        aligned_pre_eq_curves.insert(role.to_string(), c);
    }

    // 4. Bass-management crossover optimization
    let l_curve = &aligned_pre_eq_curves["L"];
    let r_curve = &aligned_pre_eq_curves["R"];
    let sub_curve = &aligned_pre_eq_curves[&sub_role];
    let measured_phase_inputs = [
        &aligned_curves["L"],
        &aligned_curves["R"],
        &aligned_curves[&sub_role],
    ];
    let phase_inputs = [l_curve, r_curve, sub_curve];
    let measured_phase_available = all_curves_have_usable_phase(&measured_phase_inputs);
    let shared_grid_available = all_curves_share_frequency_grid(&measured_phase_inputs)
        && all_curves_share_frequency_grid(&phase_inputs);
    let phase_available = measured_phase_available && shared_grid_available;
    let mut optimization_advisories = Vec::new();
    if !measured_phase_available {
        optimization_advisories.push("missing_phase_crossover_alignment_skipped".to_string());
    } else if !shared_grid_available {
        optimization_advisories
            .push("frequency_grid_mismatch_crossover_alignment_skipped".to_string());
    }
    let virtual_main = if phase_available {
        // The crossover optimizer needs a coherent summed magnitude+phase for
        // the mains, not separate averages. If phase is missing, we fall back
        // to magnitude-only reporting and keep configured timing/polarity.
        complex_sum_mains(&[l_curve, r_curve])
    } else {
        average_mains_magnitude(&[l_curve, r_curve])
    };

    let final_xover_type = select_bass_management_crossover_type(
        xover_type_str,
        &virtual_main,
        sub_curve,
        est_xo,
        sample_rate,
    );
    let xover_type_str = final_xover_type.as_str();

    // We need to parse crossover type for the optimizer
    let crossover_type_enum: crate::loss::CrossoverType = xover_type_str
        .parse()
        .map_err(|e: String| AutoeqError::InvalidConfiguration { message: e })?;

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

    let objective_before_curve = predict_bass_management_sum(
        &virtual_main,
        sub_curve,
        xover_type_str,
        est_xo,
        sample_rate,
        0.0,
        0.0,
        0.0,
        0.0,
        false,
    );
    let objective_before = bass_management_objective(objective_before_curve.as_ref(), est_xo);

    let (main_gain_post, main_delay_raw, sub_gain_raw, sub_delay_raw, sub_inverted, final_xo_freq) =
        if phase_available {
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

            (
                xo_gains[0],
                xo_delays[0],
                xo_gains[1],
                xo_delays[1],
                inversions[1],
                xo_freqs[0],
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, false, est_xo)
        };
    let (main_delay_post, sub_delay_post) =
        normalize_crossover_delays(main_delay_raw, sub_delay_raw);
    let sub_gain_post = sub_gain_raw;

    info!(
        "  Crossover Optimized: Freq={:.1} Hz, Main Gain={:.2}, Sub Gain={:.2}, Main Delay={:.2}, Sub Delay={:.2}",
        final_xo_freq, main_gain_post, sub_gain_post, main_delay_post, sub_delay_post
    );

    // 6. Apply Crossover (Filters + Gain/Delay)
    // We calculate the post-crossover curves for Post-EQ using FINAL frequency

    let hp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, true);

    let apply_chain =
        |curve: &Curve, filters: &[Biquad], gain: f64, delay: f64, invert: bool| -> Curve {
            let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
            let mut c = response::apply_complex_response(curve, &resp);
            for s in c.spl.iter_mut() {
                *s += gain;
            }
            apply_delay_and_polarity_to_curve(&c, delay, invert)
        };

    // Apply the crossover to the POST-FEATURE curves so Post-EQ sees the
    // same response the listener will hear after Pre-EQ + crossover.
    // Phase 3a used aligned_curves (raw + alignment gain) here, but now
    // that the feature stack lives in the final chain before the crossover,
    // the post-crossover reference must carry the feature correction.
    let l_post = apply_chain(
        &aligned_pre_eq_curves["L"],
        &hp_biquads,
        main_gain_post,
        main_delay_post,
        false,
    );
    let r_post = apply_chain(
        &aligned_pre_eq_curves["R"],
        &hp_biquads,
        main_gain_post,
        main_delay_post,
        false,
    );
    let sub_post_initial = apply_chain(
        &aligned_pre_eq_curves[&sub_role],
        &lp_biquads,
        sub_gain_post,
        sub_delay_post,
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

    let lfe_physical_gain = bass_management
        .as_ref()
        .filter(|bm| bm.config.apply_lfe_gain_to_chain)
        .map(|bm| bm.config.lfe_playback_gain_db)
        .unwrap_or(0.0);
    let requested_sub_gain = sub_gain_post + sub_correction + lfe_physical_gain;
    let (sub_gain_post, sub_gain_limited) =
        super::home_cinema::limited_sub_gain(requested_sub_gain, bass_management.as_ref());
    if sub_gain_limited {
        log::warn!(
            "  Bass management limited sub gain from {:+.2} dB to {:+.2} dB for headroom",
            requested_sub_gain,
            sub_gain_post
        );
        optimization_advisories.push("sub_gain_limited_for_headroom".to_string());
    }
    let mut sub_post = sub_post_initial.clone();
    for s in sub_post.spl.iter_mut() {
        *s += sub_gain_post - sub_gain_raw;
    }
    let objective_after_curve = predict_bass_management_sum(
        &virtual_main,
        sub_curve,
        xover_type_str,
        final_xo_freq,
        sample_rate,
        main_gain_post,
        sub_gain_post,
        main_delay_post,
        sub_delay_post,
        sub_inverted,
    );
    let objective_after = bass_management_objective(objective_after_curve.as_ref(), final_xo_freq);
    if optimization_advisories.is_empty() {
        optimization_advisories.push("ok".to_string());
    }
    let mut bass_management_optimization = super::home_cinema::BassManagementOptimizationReport {
        applied: phase_available,
        phase_required: true,
        phase_available,
        configured_crossover_hz: Some(est_xo),
        optimized_crossover_hz: Some(final_xo_freq),
        crossover_range_hz: xover_config.frequency_range,
        crossover_type: xover_type_str.to_string(),
        main_delay_ms: main_delay_post,
        sub_delay_ms: sub_delay_post,
        relative_sub_delay_ms: sub_delay_post - main_delay_post,
        sub_polarity_inverted: sub_inverted,
        requested_sub_gain_db: requested_sub_gain,
        applied_sub_gain_db: sub_gain_post,
        gain_limited: sub_gain_limited,
        estimated_bass_bus_peak_gain_db: None,
        objective_before,
        objective_after,
        group_results: Vec::new(),
        sub_output_results: Vec::new(),
        advisories: optimization_advisories,
    };
    let bass_routing_graph = super::home_cinema::bass_management_routing_graph(
        config,
        Some(&bass_management_optimization),
    );
    bass_management_optimization.estimated_bass_bus_peak_gain_db =
        super::home_cinema::estimated_bass_bus_peak_gain_db(
            bass_routing_graph.as_ref(),
            sub_gain_post,
        );

    // 7. Post-EQ (Global)
    // L/R: min_freq = xover + 20
    // Sub: max_freq = xover - 20
    let mut post_eq_filters = HashMap::new();

    let main_post_max_freq = config.optimizer.max_freq;
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

        // B7 — "do no harm" guard on the Mains Post-EQ. When Pre-EQ +
        // Crossover already leaves the post-crossover curve flat, a tight
        // Post-EQ can over-fit and worsen it (narrow modes, excursion-
        // constrained bass). The Sub Post-EQ has had this guard for a
        // while; mirror it on L/R.
        let pre = compute_flat_loss(post_curve, opt_config.min_freq, main_post_max_freq);
        let eq_resp =
            response::compute_peq_complex_response(&filters, &post_curve.freq, sample_rate);
        let post_curve_after = response::apply_complex_response(post_curve, &eq_resp);
        let post = compute_flat_loss(&post_curve_after, opt_config.min_freq, main_post_max_freq);
        if post < pre {
            post_eq_filters.insert(role.to_string(), filters);
        } else {
            log::warn!(
                "  {} Post-EQ discarded: score regressed from {:.4} to {:.4}",
                role,
                pre,
                post
            );
            post_eq_filters.insert(role.to_string(), Vec::new());
        }
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
            post_eq_filters.insert(sub_role.clone(), filters);
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

    // L/R Chain: AlignGain -> [Pre-EQ feature stack: excursion, CEA2034,
    //            broadband, main EQ] -> Crossover(HP) -> MainGain -> Delay -> PostEQ
    for role in ["L", "R"] {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(align_gain));
        }

        // Pre-EQ feature stack from `process_single_speaker`: excursion
        // HPF + CEA2034 Pass 1 + broadband shelf+gain + per-channel EQ.
        // Inserted here (before the crossover HP) so the features act on
        // the raw speaker signal and the crossover integration picks up
        // the feature-corrected response.
        if let Some(stack) = pre_eq_plugins.get(role) {
            plugins.extend(stack.clone());
        }

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

    // Sub Chain: AlignGain -> [Pre-EQ feature stack: excursion, broadband,
    //            sub EQ — CEA2034 skipped since no speaker_name on the
    //            inline source] -> Crossover(LP) -> SubGain(Invert) -> PostEQ
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(&sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin(sub_align_gain));
    }

    if let Some(stack) = pre_eq_plugins.get(&sub_role) {
        sub_plugins.extend(stack.clone());
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

    let sub_eqs = post_eq_filters.get(&sub_role);
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

    let sub_initial_data: super::types::CurveData = (&aligned_curves[&sub_role]).into();
    let sub_final_data: super::types::CurveData = (&final_sub_curve).into();
    let sub_eq_resp = super::output::compute_eq_response(&sub_initial_data, &sub_final_data);
    let sub_chain = ChannelDspChain {
        channel: sub_role.clone(),
        plugins: sub_plugins,
        drivers: driver_chains,
        initial_curve: Some(sub_initial_data),
        final_curve: Some(sub_final_data),
        eq_response: Some(sub_eq_resp),
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    };
    channel_chains.insert(sub_role.clone(), sub_chain);

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
            sub_role.clone(),
            ChannelOptimizationResult {
                name: sub_role.clone(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[&sub_role].clone(),
                final_curve: final_sub_curve.clone(),
                biquads: post_eq_filters.get(&sub_role).cloned().unwrap_or_default(),
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

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    let epa_per_channel = crate::roomeq::output::compute_epa_per_channel(&channel_chains, &epa_cfg);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            loss_type: Some(config.optimizer.loss_type.clone()),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
            epa_per_channel,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: Some(super::home_cinema::analyze_layout(config)),
            multi_seat_coverage: Some(super::home_cinema::multi_seat_coverage(config)),
            bass_management: super::home_cinema::bass_management_report_with_optimization(
                config,
                Some(sub_gain_post),
                sub_gain_limited,
                Some(bass_management_optimization),
            ),
            timing_diagnostics: None,
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
    let sub_role = super::home_cinema::bass_output_role(config, sys);
    let has_sub = sys.speakers.contains_key(&sub_role);

    // Classify channels into main and sub
    let main_roles: Vec<String> = sys
        .speakers
        .keys()
        .filter(|r| *r != &sub_role && !super::home_cinema::role_for_channel(r).is_sub_or_lfe())
        .cloned()
        .collect();

    info!(
        "Running Home Cinema Optimization Workflow ({} mains{})",
        main_roles.len(),
        if has_sub { " + bass-managed sub" } else { "" }
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

    // Load bass output if present (handles Single, MultiSub/MSO, Cardioid, DBA)
    let sub_preprocess = if has_sub {
        let sub_sys = sys
            .subwoofers
            .as_ref()
            .ok_or(AutoeqError::InvalidConfiguration {
                message: format!(
                    "Missing subwoofers configuration for home cinema with '{}'",
                    sub_role
                ),
            })?;
        let lfe_meas_key =
            sys.speakers
                .get(&sub_role)
                .ok_or(AutoeqError::InvalidConfiguration {
                    message: format!("Missing speaker mapping for '{}'", sub_role),
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
        curves.insert(sub_role.clone(), sp.combined_curve.clone());
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
            _output_dir,
        )
    } else {
        optimize_home_cinema_no_sub(config, sys, &main_roles, &curves, sample_rate, _output_dir)
    }
}

/// Home Cinema X.0 (no subwoofer): per-channel EQ optimization
///
/// Delegates each channel to `process_single_speaker` so every feature
/// (excursion protection, target response, broadband matching, CEA2034
/// correction) applies uniformly with the generic path.
fn optimize_home_cinema_no_sub(
    config: &RoomConfig,
    sys: &SystemConfig,
    main_roles: &[String],
    curves: &HashMap<String, Curve>,
    sample_rate: f64,
    output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    // Level alignment: mains measured from 100 Hz to 2000 Hz
    let mut ranges = HashMap::new();
    for role in main_roles {
        ranges.insert(role.clone(), (100.0, 2000.0));
    }
    let gains = align_channels_to_lowest(curves, &ranges);

    let mut channel_chains = HashMap::new();
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in main_roles {
        let gain = *gains.get(role).unwrap_or(&0.0);
        let source = resolve_single_source(role, config, sys)?;

        info!("  Optimizing '{}' with alignment gain {:.2} dB", role, gain);

        let (chain, ch_result, pre_score, post_score, _fir) =
            run_channel_via_generic_path(role, source, config, gain, sample_rate, output_dir)?;

        info!(
            "  '{}' pre_score={:.4} post_score={:.4}",
            role, pre_score, post_score
        );

        channel_chains.insert(role.clone(), chain);
        channel_results.insert(role.clone(), ch_result);
        pre_scores.push(pre_score);
        post_scores.push(post_score);
    }

    let avg_pre = pre_scores.iter().sum::<f64>() / pre_scores.len() as f64;
    let avg_post = post_scores.iter().sum::<f64>() / post_scores.len() as f64;

    info!(
        "Average pre-score: {:.4}, post-score: {:.4}",
        avg_pre, avg_post
    );

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    let epa_per_channel = crate::roomeq::output::compute_epa_per_channel(&channel_chains, &epa_cfg);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            loss_type: Some(config.optimizer.loss_type.clone()),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
            epa_per_channel,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: Some(super::home_cinema::analyze_layout(config)),
            multi_seat_coverage: Some(super::home_cinema::multi_seat_coverage(config)),
            bass_management: None,
            timing_diagnostics: None,
        },
    })
}

/// Home Cinema X.1 (with subwoofer): crossover management + per-channel EQ.
///
/// Phase 3b: per-channel features apply via `process_single_speaker` at
/// Pre-EQ, the plugin stack is inserted before the crossover HP/LP in
/// the final chain, and Post-EQ remains a cleanup pass with the B7
/// "do no harm" guard.
fn optimize_home_cinema_with_sub(
    config: &RoomConfig,
    sys: &SystemConfig,
    main_roles: &[String],
    curves: &HashMap<String, Curve>,
    sub_preprocess: SubPreprocessResult,
    sample_rate: f64,
    output_dir: &Path,
) -> Result<RoomOptimizationResult> {
    let sub_role = super::home_cinema::bass_output_role(config, sys);

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
    let bass_management = super::home_cinema::effective_bass_management(config);

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
    ranges.insert(sub_role.clone(), (sub_min_align, max_xo));

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

    // 2. Pre-EQ — route each channel through `process_single_speaker` so
    //    excursion / CEA2034 / broadband / target-response all apply. The
    //    returned plugin stack becomes the "feature chain" that runs
    //    before crossover HP/LP in the final DSP assembly; the returned
    //    final_curve is the linearized curve the crossover optimizer
    //    sees. See `optimize_stereo_2_1` for the shared rationale.
    let mut pre_eq_plugins: HashMap<String, Vec<super::types::PluginConfigWrapper>> =
        HashMap::new();
    let mut linearized_curves: HashMap<String, Curve> = HashMap::new();

    for role in main_roles {
        let source = resolve_single_source(role, config, sys)?;
        let mut per_config = config.clone();
        per_config.optimizer.min_freq = min_xo;
        info!(
            "  Pre-EQ via generic path for '{}' (min_freq={:.1} Hz)",
            role, min_xo
        );
        let (chain, ch_result, _pre, _post, _fir) =
            run_channel_via_generic_path(role, source, &per_config, 0.0, sample_rate, output_dir)?;
        pre_eq_plugins.insert(role.clone(), chain.plugins);
        linearized_curves.insert(role.clone(), ch_result.final_curve);
    }

    // Sub Pre-EQ
    {
        let sub_source = crate::MeasurementSource::InMemory(sub_preprocess.combined_curve.clone());
        let mut sub_config = config.clone();
        sub_config.optimizer.max_freq = max_xo;
        info!(
            "  Pre-EQ via generic path for '{}' (max_freq={:.1} Hz)",
            sub_role, max_xo
        );
        let (chain, ch_result, _pre, _post, _fir) = run_channel_via_generic_path(
            &sub_role,
            &sub_source,
            &sub_config,
            0.0,
            sample_rate,
            output_dir,
        )?;
        pre_eq_plugins.insert(sub_role.clone(), chain.plugins);
        linearized_curves.insert(sub_role.clone(), ch_result.final_curve);
    }

    // Aligned linearized curves (post-feature, at listening level) — used
    // for crossover optimization and for the apply_chain step below.
    let mut aligned_pre_eq_curves: HashMap<String, Curve> = HashMap::new();
    for role in main_roles {
        let mut c = linearized_curves[role].clone();
        let g = *gains.get(role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() {
            *s += g;
        }
        aligned_pre_eq_curves.insert(role.clone(), c);
    }
    {
        let mut c = linearized_curves[&sub_role].clone();
        let g = *gains.get(&sub_role).unwrap_or(&0.0);
        for s in c.spl.iter_mut() {
            *s += g;
        }
        aligned_pre_eq_curves.insert(sub_role.clone(), c);
    }

    // 3. Bass-managed virtual main. Phase is mandatory for optimizing
    // crossover delay/polarity; without it, keep configured timing/polarity.
    let main_refs: Vec<&Curve> = main_roles
        .iter()
        .map(|r| &aligned_pre_eq_curves[r])
        .collect();
    let sub_curve = &aligned_pre_eq_curves[&sub_role];
    let mut measured_phase_check_refs: Vec<&Curve> =
        main_roles.iter().map(|r| &aligned_curves[r]).collect();
    measured_phase_check_refs.push(&aligned_curves[&sub_role]);
    let mut phase_check_refs = main_refs.clone();
    phase_check_refs.push(sub_curve);
    let measured_phase_available = all_curves_have_usable_phase(&measured_phase_check_refs);
    let shared_grid_available = all_curves_share_frequency_grid(&measured_phase_check_refs)
        && all_curves_share_frequency_grid(&phase_check_refs);
    let phase_available = measured_phase_available && shared_grid_available;
    let mut optimization_advisories = Vec::new();
    if !measured_phase_available {
        optimization_advisories.push("missing_phase_crossover_alignment_skipped".to_string());
    } else if !shared_grid_available {
        optimization_advisories
            .push("frequency_grid_mismatch_crossover_alignment_skipped".to_string());
    }
    let virtual_main = if phase_available {
        complex_sum_mains(&main_refs)
    } else {
        average_mains_magnitude(&main_refs)
    };

    // 4. Crossover optimization between virtual main and physical bass output
    let final_xover_type = select_bass_management_crossover_type(
        xover_type_str,
        &virtual_main,
        sub_curve,
        est_xo,
        sample_rate,
    );
    let xover_type_str = final_xover_type.as_str();
    let crossover_type_enum: crate::loss::CrossoverType = xover_type_str
        .parse()
        .map_err(|e: String| AutoeqError::InvalidConfiguration { message: e })?;

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

    let objective_before_curve = predict_bass_management_sum(
        &virtual_main,
        sub_curve,
        xover_type_str,
        est_xo,
        sample_rate,
        0.0,
        0.0,
        0.0,
        0.0,
        false,
    );
    let objective_before = bass_management_objective(objective_before_curve.as_ref(), est_xo);

    let (main_gain_post, main_delay_raw, sub_gain_raw, sub_delay_raw, sub_inverted, final_xo_freq) =
        if phase_available {
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

            (
                xo_gains[0],
                xo_delays[0],
                xo_gains[1],
                xo_delays[1],
                inversions[1],
                xo_freqs[0],
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, false, est_xo)
        };
    let (main_delay_post, sub_delay_post) =
        normalize_crossover_delays(main_delay_raw, sub_delay_raw);
    let sub_gain_post = sub_gain_raw;

    info!(
        "  Crossover Optimized: Freq={:.1} Hz, Main Gain={:.2}, Sub Gain={:.2}, Main Delay={:.2}, Sub Delay={:.2}",
        final_xo_freq, main_gain_post, sub_gain_post, main_delay_post, sub_delay_post
    );

    let group_results_by_id = if bass_management
        .as_ref()
        .map(|bm| bm.config.optimize_groups)
        .unwrap_or(true)
    {
        optimize_home_cinema_group_crossovers(
            config,
            main_roles,
            &aligned_curves,
            &aligned_pre_eq_curves,
            &sub_role,
            xover_config,
            sample_rate,
            bass_management.as_ref(),
        )?
    } else {
        BTreeMap::new()
    };
    // 5. Apply crossover filters
    let _hp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, false);
    let lp_biquads = create_crossover_filters(xover_type_str, final_xo_freq, sample_rate, true);

    let apply_chain =
        |curve: &Curve, filters: &[Biquad], gain: f64, delay: f64, invert: bool| -> Curve {
            let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
            let mut c = response::apply_complex_response(curve, &resp);
            for s in c.spl.iter_mut() {
                *s += gain;
            }
            apply_delay_and_polarity_to_curve(&c, delay, invert)
        };

    // Post-crossover curves for all mains and sub.
    // Using aligned_pre_eq_curves (post-feature, post-align) so Post-EQ
    // sees the real curve the listener will hear after the feature stack
    // and crossover.
    let mut main_post_curves = HashMap::new();
    for role in main_roles {
        let group_id =
            super::home_cinema::group_id_for_role(super::home_cinema::role_for_channel(role));
        let group = group_results_by_id.get(group_id);
        let role_xover_type = group
            .map(|g| g.crossover_type.as_str())
            .unwrap_or(xover_type_str);
        let role_xover_freq = group
            .and_then(|g| g.selected_crossover_hz)
            .unwrap_or(final_xo_freq);
        let role_main_delay = group.map(|g| g.main_delay_ms).unwrap_or(main_delay_post);
        let role_hp_biquads =
            create_crossover_filters(role_xover_type, role_xover_freq, sample_rate, false);
        let post = apply_chain(
            &aligned_pre_eq_curves[role],
            &role_hp_biquads,
            main_gain_post,
            role_main_delay,
            false,
        );
        main_post_curves.insert(role.clone(), post);
    }
    let sub_post_initial = apply_chain(
        &aligned_pre_eq_curves[&sub_role],
        &lp_biquads,
        sub_gain_post,
        sub_delay_post,
        sub_inverted,
    );

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

    let lfe_physical_gain = bass_management
        .as_ref()
        .filter(|bm| bm.config.apply_lfe_gain_to_chain)
        .map(|bm| bm.config.lfe_playback_gain_db)
        .unwrap_or(0.0);
    let requested_sub_gain = sub_gain_post + sub_correction + lfe_physical_gain;
    let (sub_gain_post, sub_gain_limited) =
        super::home_cinema::limited_sub_gain(requested_sub_gain, bass_management.as_ref());
    if sub_gain_limited {
        log::warn!(
            "  Bass management limited sub gain from {:+.2} dB to {:+.2} dB for headroom",
            requested_sub_gain,
            sub_gain_post
        );
        optimization_advisories.push("sub_gain_limited_for_headroom".to_string());
    }
    let mut sub_post = sub_post_initial.clone();
    for s in sub_post.spl.iter_mut() {
        *s += sub_gain_post - sub_gain_raw;
    }
    let objective_after_curve = predict_bass_management_sum(
        &virtual_main,
        sub_curve,
        xover_type_str,
        final_xo_freq,
        sample_rate,
        main_gain_post,
        sub_gain_post,
        main_delay_post,
        sub_delay_post,
        sub_inverted,
    );
    let objective_after = bass_management_objective(objective_after_curve.as_ref(), final_xo_freq);
    if optimization_advisories.is_empty() {
        optimization_advisories.push("ok".to_string());
    }
    let mut bass_management_optimization = super::home_cinema::BassManagementOptimizationReport {
        applied: phase_available,
        phase_required: true,
        phase_available,
        configured_crossover_hz: Some(est_xo),
        optimized_crossover_hz: Some(final_xo_freq),
        crossover_range_hz: xover_config.frequency_range,
        crossover_type: xover_type_str.to_string(),
        main_delay_ms: main_delay_post,
        sub_delay_ms: sub_delay_post,
        relative_sub_delay_ms: sub_delay_post - main_delay_post,
        sub_polarity_inverted: sub_inverted,
        requested_sub_gain_db: requested_sub_gain,
        applied_sub_gain_db: sub_gain_post,
        gain_limited: sub_gain_limited,
        estimated_bass_bus_peak_gain_db: None,
        objective_before,
        objective_after,
        group_results: group_results_by_id.values().cloned().collect(),
        sub_output_results: bass_management_sub_output_results(
            &sub_role,
            sub_preprocess.drivers.as_deref(),
            sub_gain_post,
            &sub_sys.config,
        ),
        advisories: optimization_advisories,
    };
    let bass_routing_graph = super::home_cinema::bass_management_routing_graph(
        config,
        Some(&bass_management_optimization),
    );
    bass_management_optimization.estimated_bass_bus_peak_gain_db =
        super::home_cinema::estimated_bass_bus_peak_gain_db(
            bass_routing_graph.as_ref(),
            sub_gain_post,
        );

    // 6. Post-EQ
    let mut post_eq_filters = HashMap::new();
    let main_post_max_freq = config.optimizer.max_freq;

    for role in main_roles {
        let mut opt_config = config.optimizer.clone();
        let group_id =
            super::home_cinema::group_id_for_role(super::home_cinema::role_for_channel(role));
        let role_xover_freq = group_results_by_id
            .get(group_id)
            .and_then(|g| g.selected_crossover_hz)
            .unwrap_or(final_xo_freq);
        opt_config.min_freq = role_xover_freq + 20.0;

        let post_curve = &main_post_curves[role];
        let (filters, _) = eq::optimize_channel_eq(
            post_curve,
            &opt_config,
            config.target_curve.as_ref(),
            sample_rate,
        )
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: e.to_string(),
        })?;

        // B7 — "do no harm" guard on the Mains Post-EQ, mirroring the
        // long-standing Sub guard below.
        let pre = compute_flat_loss(post_curve, opt_config.min_freq, main_post_max_freq);
        let eq_resp =
            response::compute_peq_complex_response(&filters, &post_curve.freq, sample_rate);
        let post_curve_after = response::apply_complex_response(post_curve, &eq_resp);
        let post = compute_flat_loss(&post_curve_after, opt_config.min_freq, main_post_max_freq);
        if post < pre {
            post_eq_filters.insert(role.clone(), filters);
        } else {
            log::warn!(
                "  {} Post-EQ discarded: score regressed from {:.4} to {:.4}",
                role,
                pre,
                post
            );
            post_eq_filters.insert(role.clone(), Vec::new());
        }
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
            post_eq_filters.insert(sub_role.clone(), filters);
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

    // Main channels: AlignGain -> [Pre-EQ feature stack] -> Crossover(HP)
    //                -> MainGain -> Delay -> PostEQ
    for role in main_roles {
        let mut plugins = Vec::new();
        let align_gain = *gains.get(role).unwrap_or(&0.0);
        if align_gain.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(align_gain));
        }

        if let Some(stack) = pre_eq_plugins.get(role) {
            plugins.extend(stack.clone());
        }

        let group_id =
            super::home_cinema::group_id_for_role(super::home_cinema::role_for_channel(role));
        let group = group_results_by_id.get(group_id);
        let role_xover_type = group
            .map(|g| g.crossover_type.as_str())
            .unwrap_or(xover_type_str);
        let role_xover_freq = group
            .and_then(|g| g.selected_crossover_hz)
            .unwrap_or(final_xo_freq);
        let role_main_delay = group.map(|g| g.main_delay_ms).unwrap_or(main_delay_post);

        plugins.push(output::create_crossover_plugin(
            role_xover_type,
            role_xover_freq,
            "high",
        ));

        if main_gain_post.abs() > 0.01 {
            plugins.push(output::create_gain_plugin(main_gain_post));
        }

        // Main delay from crossover optimizer (sub-main time alignment)
        if role_main_delay.abs() > 0.01 {
            plugins.push(output::create_delay_plugin(role_main_delay));
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

    // Sub chain: AlignGain -> [Pre-EQ feature stack] -> Crossover(LP)
    //            -> SubGain(Invert) -> Delay -> PostEQ
    let mut sub_plugins = Vec::new();
    let sub_align_gain = *gains.get(&sub_role).unwrap_or(&0.0);
    if sub_align_gain.abs() > 0.01 {
        sub_plugins.push(output::create_gain_plugin(sub_align_gain));
    }

    if let Some(stack) = pre_eq_plugins.get(&sub_role) {
        sub_plugins.extend(stack.clone());
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

    let sub_eqs = post_eq_filters.get(&sub_role);
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

    let sub_initial_data: super::types::CurveData = (&aligned_curves[&sub_role]).into();
    let sub_final_data: super::types::CurveData = (&final_sub_curve).into();
    let sub_eq_resp = super::output::compute_eq_response(&sub_initial_data, &sub_final_data);
    let sub_chain = ChannelDspChain {
        channel: sub_role.clone(),
        plugins: sub_plugins,
        drivers: driver_chains,
        initial_curve: Some(sub_initial_data),
        final_curve: Some(sub_final_data),
        eq_response: Some(sub_eq_resp),
        pre_ir: None,
        post_ir: None,
        target_curve: None,
    };
    channel_chains.insert(sub_role.clone(), sub_chain);

    // 8. Compute scores
    let max_freq = config.optimizer.max_freq;
    let sub_min_score = config.optimizer.min_freq.max(20.0);
    let mut channel_results = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for role in main_roles {
        let intermediate = &main_post_curves[role];
        let group_id =
            super::home_cinema::group_id_for_role(super::home_cinema::role_for_channel(role));
        let role_xover_freq = group_results_by_id
            .get(group_id)
            .and_then(|g| g.selected_crossover_hz)
            .unwrap_or(final_xo_freq);
        let pre_score = compute_flat_loss(intermediate, role_xover_freq, max_freq);
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
        let post_score = compute_flat_loss(&final_curve_obj, role_xover_freq, max_freq);

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
            sub_role.clone(),
            ChannelOptimizationResult {
                name: sub_role.clone(),
                pre_score,
                post_score,
                initial_curve: aligned_curves[&sub_role].clone(),
                final_curve: final_sub_curve.clone(),
                biquads: post_eq_filters.get(&sub_role).cloned().unwrap_or_default(),
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

    let epa_cfg = config.optimizer.epa_config.clone().unwrap_or_default();
    let epa_per_channel = crate::roomeq::output::compute_epa_per_channel(&channel_chains, &epa_cfg);

    Ok(RoomOptimizationResult {
        channels: channel_chains,
        channel_results,
        combined_pre_score: avg_pre,
        combined_post_score: avg_post,
        metadata: OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: config.optimizer.algorithm.clone(),
            loss_type: Some(config.optimizer.loss_type.clone()),
            iterations: config.optimizer.max_iter,
            timestamp: chrono::Utc::now().to_rfc3339(),
            inter_channel_deviation: None,
            epa_per_channel,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: Some(super::home_cinema::analyze_layout(config)),
            multi_seat_coverage: Some(super::home_cinema::multi_seat_coverage(config)),
            bass_management: super::home_cinema::bass_management_report_with_optimization(
                config,
                Some(sub_gain_post),
                sub_gain_limited,
                Some(bass_management_optimization),
            ),
            timing_diagnostics: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MeasurementSource;
    use crate::roomeq::optimize::optimize_room;
    use crate::roomeq::types::{
        BassManagementConfig, CrossoverConfig, OptimizerConfig, RoomConfig, SpeakerConfig,
        SubwooferSystemConfig, SystemConfig, SystemModel,
    };
    use ndarray::array;
    use std::collections::HashMap;

    fn phase_curve(phase_deg: f64) -> Curve {
        Curve {
            freq: array![40.0, 80.0, 160.0],
            spl: array![0.0, 0.0, 0.0],
            phase: Some(array![phase_deg, phase_deg, phase_deg]),
            ..Default::default()
        }
    }

    fn bass_management_workflow_curve(
        base_level: f64,
        acoustic_delay_ms: f64,
        with_phase: bool,
    ) -> Curve {
        let n = 96;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / (n - 1) as f64))
            .collect();
        let spl: Vec<f64> = freq
            .iter()
            .map(|&f| {
                let bass_shelf = if f < 80.0 {
                    -3.0 * (80.0 / f).log2().min(2.0)
                } else {
                    0.0
                };
                base_level + bass_shelf
            })
            .collect();
        let phase = with_phase.then(|| {
            ndarray::Array1::from_vec(
                freq.iter()
                    .map(|&f| -360.0 * f * acoustic_delay_ms / 1000.0)
                    .collect(),
            )
        });

        Curve {
            freq: ndarray::Array1::from_vec(freq),
            spl: ndarray::Array1::from_vec(spl),
            phase,
            ..Default::default()
        }
    }

    fn bass_management_workflow_config(with_phase: bool, max_sub_boost_db: f64) -> RoomConfig {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(bass_management_workflow_curve(
                76.0, 0.0, with_phase,
            ))),
        );
        speakers.insert(
            "right".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(bass_management_workflow_curve(
                76.5, 0.1, with_phase,
            ))),
        );
        speakers.insert(
            "sub".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(bass_management_workflow_curve(
                62.0, 3.0, with_phase,
            ))),
        );

        let mut system_speakers = HashMap::new();
        system_speakers.insert("L".to_string(), "left".to_string());
        system_speakers.insert("R".to_string(), "right".to_string());
        system_speakers.insert("LFE".to_string(), "sub".to_string());

        let mut crossovers = HashMap::new();
        crossovers.insert(
            "bass".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        );

        RoomConfig {
            version: "test".to_string(),
            system: Some(SystemConfig {
                model: SystemModel::HomeCinema,
                speakers: system_speakers,
                subwoofers: Some(SubwooferSystemConfig {
                    config: super::SubwooferStrategy::Single,
                    crossover: Some("bass".to_string()),
                    mapping: HashMap::new(),
                }),
                bass_management: Some(BassManagementConfig {
                    enabled: true,
                    redirect_bass: true,
                    max_sub_boost_db,
                    ..BassManagementConfig::default()
                }),
            }),
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer: OptimizerConfig {
                num_filters: 1,
                max_iter: 4,
                population: 4,
                seed: Some(7),
                refine: false,
                allow_delay: Some(false),
                decomposed_correction: None,
                min_freq: 20.0,
                max_freq: 20_000.0,
                ..OptimizerConfig::default()
            },
            recording_config: None,
            cea2034_cache: None,
        }
    }

    fn total_delay_ms(chain: &ChannelDspChain) -> f64 {
        chain
            .plugins
            .iter()
            .filter(|p| p.plugin_type == "delay")
            .map(|p| p.parameters["delay_ms"].as_f64().unwrap_or(0.0))
            .sum()
    }

    fn has_crossover_plugin(chain: &ChannelDspChain, mode: &str) -> bool {
        chain
            .plugins
            .iter()
            .any(|p| p.plugin_type == "crossover" && p.parameters["output"].as_str() == Some(mode))
    }

    #[test]
    fn home_cinema_bass_management_delays_are_normalized() {
        let (main, sub) = normalize_crossover_delays(-1.5, 0.25);
        assert_eq!(main, 0.0);
        assert!((sub - 1.75).abs() < 1e-9);

        let (main, sub) = normalize_crossover_delays(2.0, 5.0);
        assert_eq!(main, 0.0);
        assert_eq!(sub, 3.0);
    }

    #[test]
    fn home_cinema_bass_management_delay_and_polarity_update_phase() {
        let curve = phase_curve(0.0);
        let adjusted = apply_delay_and_polarity_to_curve(&curve, 1.0, true);
        let phase = adjusted.phase.expect("phase");

        assert!((phase[0] - 165.6).abs() < 1e-6);
        assert!((phase[1] - 151.2).abs() < 1e-6);
        assert!((phase[2] - 122.4).abs() < 1e-6);
    }

    #[test]
    fn home_cinema_bass_management_prediction_requires_phase() {
        let main = phase_curve(0.0);
        let mut sub = phase_curve(0.0);
        sub.phase = None;

        let predicted = predict_bass_management_sum(
            &main, &sub, "LR24", 80.0, 48_000.0, 0.0, 0.0, 0.0, 0.0, false,
        );

        assert!(predicted.is_none());
    }

    #[test]
    fn home_cinema_bass_management_alignment_requires_matching_grids() {
        let main = phase_curve(0.0);
        let mut shifted = phase_curve(0.0);
        shifted.freq = array![41.0, 82.0, 164.0];

        assert!(all_curves_have_usable_phase(&[&main, &shifted]));
        assert!(!all_curves_share_frequency_grid(&[&main, &shifted]));
    }

    #[test]
    fn home_cinema_bass_management_prediction_has_phase_when_valid() {
        let main = phase_curve(0.0);
        let sub = phase_curve(0.0);

        let predicted = predict_bass_management_sum(
            &main, &sub, "LR24", 80.0, 48_000.0, 0.0, 0.0, 0.0, 1.0, false,
        )
        .expect("prediction");

        assert_eq!(predicted.freq.len(), main.freq.len());
        assert!(predicted.phase.is_some());
        assert!(bass_management_objective(Some(&predicted), 80.0).is_some());
    }

    #[test]
    fn home_cinema_bass_management_workflow_reports_exported_dsp() {
        let config = bass_management_workflow_config(true, 6.0);
        let result = optimize_room(&config, 48_000.0, None, None).expect("room optimization");
        let report = result
            .metadata
            .bass_management
            .as_ref()
            .expect("bass management report");
        let optimization = report.optimization.as_ref().expect("optimization report");

        assert!(optimization.applied);
        assert!(optimization.phase_available);
        assert_eq!(optimization.advisories, vec!["ok".to_string()]);
        assert!(optimization.objective_before.is_some());
        assert!(optimization.objective_after.is_some());
        assert!(optimization.main_delay_ms >= -1e-9);
        assert!(optimization.sub_delay_ms >= -1e-9);
        assert_eq!(
            report.applied_sub_gain_db,
            Some(optimization.applied_sub_gain_db)
        );
        assert!(optimization.estimated_bass_bus_peak_gain_db.is_some());
        let routing = report.routing_graph.as_ref().expect("routing graph");
        assert_eq!(routing.physical_sub_output, "LFE");
        assert!(routing.routes.iter().any(|route| {
            route.route_kind == "redirected_bass_lowpass_to_sub"
                && route.source_channel == "L"
                && route.destination == "LFE"
        }));
        assert!(routing.routes.iter().any(|route| {
            route.route_kind == "lfe_lowpass_to_sub"
                && route.source_channel == "LFE"
                && (route.gain_db - 10.0).abs() < 1e-9
        }));
        let routing_matrix = routing.matrix.as_ref().expect("routing matrix");
        assert_eq!(routing_matrix.output_channel_map, vec![2]);
        assert_eq!(routing_matrix.route_count, 3);

        for role in ["L", "R"] {
            let chain = result.channels.get(role).expect("main chain");
            assert!(has_crossover_plugin(chain, "high"));
            let group = optimization
                .group_results
                .iter()
                .find(|group| group.group_id == "lcr")
                .expect("lcr group result");
            assert!((total_delay_ms(chain) - group.main_delay_ms).abs() < 1e-6);
            let chain_phase = chain
                .final_curve
                .as_ref()
                .and_then(|c| c.phase.as_ref())
                .expect("chain final phase");
            let result_phase = result.channel_results[role]
                .final_curve
                .phase
                .as_ref()
                .expect("result final phase");
            assert_eq!(chain_phase.len(), result_phase.len());
        }

        let sub_chain = result.channels.get("LFE").expect("sub chain");
        let dsp_output = result.to_dsp_chain_output();
        let matrix_plugin = dsp_output
            .global_plugins
            .iter()
            .find(|plugin| plugin.plugin_type == "matrix")
            .expect("bass-management global matrix plugin");
        assert_eq!(
            matrix_plugin.parameters["label"].as_str(),
            Some("home_cinema_bass_management")
        );
        assert_eq!(
            matrix_plugin.parameters["input_channel_map"]
                .as_array()
                .expect("input map")
                .len(),
            routing_matrix.input_channel_map.len()
        );
        assert!(has_crossover_plugin(sub_chain, "low"));
        assert!((total_delay_ms(sub_chain) - optimization.sub_delay_ms).abs() < 1e-6);
        assert!(
            sub_chain
                .final_curve
                .as_ref()
                .and_then(|c| c.phase.as_ref())
                .is_some()
        );
    }

    #[test]
    fn home_cinema_bass_management_workflow_skips_alignment_without_phase() {
        let config = bass_management_workflow_config(false, 6.0);
        let result = optimize_room(&config, 48_000.0, None, None).expect("room optimization");
        let optimization = result
            .metadata
            .bass_management
            .as_ref()
            .and_then(|r| r.optimization.as_ref())
            .expect("optimization report");

        assert!(!optimization.applied);
        assert!(!optimization.phase_available);
        assert_eq!(optimization.main_delay_ms, 0.0);
        assert_eq!(optimization.sub_delay_ms, 0.0);
        assert!(!optimization.sub_polarity_inverted);
        assert!(optimization.objective_before.is_none());
        assert!(optimization.objective_after.is_none());
        assert!(
            optimization
                .advisories
                .contains(&"missing_phase_crossover_alignment_skipped".to_string())
        );
    }

    #[test]
    fn home_cinema_bass_management_workflow_limits_sub_boost_for_headroom() {
        let config = bass_management_workflow_config(true, 0.0);
        let result = optimize_room(&config, 48_000.0, None, None).expect("room optimization");
        let report = result
            .metadata
            .bass_management
            .as_ref()
            .expect("bass management report");
        let optimization = report.optimization.as_ref().expect("optimization report");

        assert!(optimization.gain_limited);
        assert!(report.gain_limited);
        assert!(optimization.applied_sub_gain_db <= 1e-9);
        assert_eq!(
            report.applied_sub_gain_db,
            Some(optimization.applied_sub_gain_db)
        );
        assert!(
            optimization
                .advisories
                .contains(&"sub_gain_limited_for_headroom".to_string())
        );
    }

    #[test]
    fn home_cinema_bass_management_workflow_selects_auto_crossover_type() {
        let mut config = bass_management_workflow_config(true, 6.0);
        config
            .crossovers
            .as_mut()
            .and_then(|crossovers| crossovers.get_mut("bass"))
            .expect("bass crossover")
            .crossover_type = "auto".to_string();

        let result = optimize_room(&config, 48_000.0, None, None).expect("room optimization");
        let routing = result
            .metadata
            .bass_management
            .as_ref()
            .and_then(|report| report.routing_graph.as_ref())
            .expect("routing graph");
        let optimization = result
            .metadata
            .bass_management
            .as_ref()
            .and_then(|report| report.optimization.as_ref())
            .expect("optimization report");

        assert_ne!(optimization.crossover_type, "auto");
        assert!(["LR24", "LR48", "BW12", "BW24"].contains(&optimization.crossover_type.as_str()));
        assert!(
            routing
                .routes
                .iter()
                .all(|route| route.crossover_type == optimization.crossover_type)
        );
    }
}
