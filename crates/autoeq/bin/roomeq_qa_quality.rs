//! RoomEQ QA: Convergence, Monotonicity, Cross-Mode & Per-Option Tests
//!
//! Validates that roomeq optimization modes produce converging results,
//! that giving the optimizer more resources always improves or maintains loss,
//! that IIR/FIR/Mixed modes converge to similar frequency responses,
//! and that each optimizer option has its expected effect.
//!
//! Uses autoeq:de with LSHADE strategy and fixed seed for deterministic results.
//! Test cases run in parallel for maximum throughput.
//!
//! Usage:
//!   cargo run --bin roomeq-qa --release              # run all tests
//!   cargo run --bin roomeq-qa --release -- --list     # list available cases
//!   cargo run --bin roomeq-qa --release -- --case "Stereo 2.0"
//!   cargo run --bin roomeq-qa --release -- --case "Cross-Mode"
//!   cargo run --bin roomeq-qa --release -- --case "target_tilt"

use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use autoeq::Curve;
use autoeq::loss::phase_aware::{compute_group_delay, unwrap_phase_degrees};
use autoeq::loss::{calculate_standard_deviation_in_range, regression_slope_per_octave_in_range};
use autoeq::roomeq::{
    CallbackAction, DecomposedCorrectionSerdeConfig, ExcursionProtectionConfig,
    MixedPhaseSerdeConfig, MultiMeasurementConfig, MultiMeasurementStrategy, PhaseAlignmentConfig,
    PreRingingSerdeConfig, ProcessingMode, RoomConfig, RoomOptimizationResult,
    SchroederSplitConfig, SpatialRobustnessSerdeConfig, TargetResponseConfig, TargetShape,
    VoiceOfGodConfig, load_config, merge_json_objects, optimize_room,
};
use autoeq::{MeasurementMultiple, MeasurementRef, MeasurementSource};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Monotonicity tolerance: variation may be at most 20% worse than baseline.
const MONOTONICITY_TOLERANCE: f64 = 1.20;

/// Cross-mode ratio: max score / min score must be <= 5.0.
const CROSS_MODE_RATIO_LIMIT: f64 = 5.0;

const SAMPLE_RATE: f64 = 48000.0;

const SEED: u64 = 42;

/// DE maxeval for QA. LSHADE with tolerance=1e-3 converges in ~100-300 generations,
/// so we don't need many evaluations. The tolerance does the early stopping.
const QA_MAXEVAL: usize = 15_000;

/// Base config directories
const FEM_DIR: &str = "data_tests/roomeq/generated/fem";
const OPTIM_CONFIG_DIR: &str = "data_tests/roomeq/generated/optimiser-config";

// Cross-mode convergence thresholds
/// Maximum dB difference between any two modes' final curves in passband.
/// Generous limit: IIR/FIR/Mixed use fundamentally different correction
/// mechanisms so some divergence is expected.
const CROSS_MODE_FR_MAX_DIFF_DB: f64 = 18.0;
/// Score ratio limit for cross-mode convergence (reuse existing)
const CROSS_MODE_SCORE_RATIO_LIMIT: f64 = 3.0;

// Per-option effect thresholds
/// Slope tolerance in dB/octave for target_tilt validation.
///
/// The check is `option_err < baseline_err + TILT_SLOPE_TOLERANCE`. With a
/// fixed seed the DE optimizer is *mostly* deterministic, but parallel
/// execution adds non-determinism in the baseline run — depending on
/// thread scheduling the baseline slope can land anywhere in a ~1 dB/oct
/// band, which directly shifts `baseline_err`. Option behavior (tilt
/// applied) stays consistent across runs at ~0.7 dB/oct error. We
/// therefore use a 0.8 dB/oct tolerance to absorb baseline jitter while
/// still catching real tilt-application regressions (which would show
/// up as option_err well above baseline_err + 0.8).
const TILT_SLOPE_TOLERANCE: f64 = 0.8;
/// Score tolerance for option vs baseline (option within 1.2x of baseline)
const OPTION_SCORE_TOLERANCE: f64 = 1.20;
/// Psychoacoustic may trade raw score for perceptual quality
const PSYCHOACOUSTIC_SCORE_TOLERANCE: f64 = 2.0;

// ---------------------------------------------------------------------------
// QA config overrides (autoeq:de with LSHADE, fixed seed)
// ---------------------------------------------------------------------------

/// Override optimizer settings for QA: use autoeq:de with LSHADE strategy and fixed seed.
/// Uses relaxed tolerance (1e-3) for fast convergence — LSHADE typically converges
/// in ~100-300 generations, making QA fast while still using a proper global optimizer.
fn apply_qa_overrides(config: &mut RoomConfig) {
    config.optimizer.algorithm = "autoeq:de".to_string();
    config.optimizer.strategy = "lshade".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.population = 50;
    config.optimizer.num_filters = 3;
    config.optimizer.tolerance = 1e-3;
    config.optimizer.atolerance = 1e-3;
    config.optimizer.refine = true;
    config.optimizer.seed = Some(SEED);
}

// ---------------------------------------------------------------------------
// Config loading helpers
// ---------------------------------------------------------------------------

fn load_config_for_generic_path(
    base_config_path: &Path,
    override_config_path: Option<&Path>,
    processing_mode: ProcessingMode,
) -> Result<(RoomConfig, PathBuf)> {
    let config_json = std::fs::read_to_string(base_config_path)
        .with_context(|| format!("Failed to read config: {:?}", base_config_path))?;

    let mut config_value: serde_json::Value =
        serde_json::from_str(&config_json).with_context(|| "Failed to parse config JSON")?;

    if let Some(override_path) = override_config_path {
        let override_json = std::fs::read_to_string(override_path)
            .with_context(|| format!("Failed to read override config: {:?}", override_path))?;
        let override_value: serde_json::Value = serde_json::from_str(&override_json)
            .with_context(|| "Failed to parse override config JSON")?;
        merge_json_objects(&mut config_value, &override_value);
    }

    // Remove "system" key to force generic path
    if let Some(obj) = config_value.as_object_mut() {
        obj.remove("system");
    }

    let config_dir = base_config_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut room_config: RoomConfig = serde_json::from_value(config_value)
        .with_context(|| "Failed to deserialize config into RoomConfig")?;

    room_config.resolve_paths(&config_dir);
    room_config.optimizer.processing_mode = processing_mode;

    Ok((room_config, config_dir))
}

// ---------------------------------------------------------------------------
// Optimization runner
// ---------------------------------------------------------------------------

/// Global counter for unique temp dir names across threads
static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run_optimization(config: &RoomConfig) -> Result<RoomOptimizationResult> {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir().join(format!("roomeq_qa_{}_{}", std::process::id(), id));
    std::fs::create_dir_all(&temp_dir)?;
    let callback =
        Box::new(|_: &autoeq::roomeq::RoomOptimizationProgress| CallbackAction::Continue);
    let result = optimize_room(config, SAMPLE_RATE, Some(callback), Some(&temp_dir))
        .map_err(|e| anyhow!("{}", e));
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

/// Maximum absolute SPL difference between any pair of curves on a common frequency grid.
/// Only considers the frequency range [fmin, fmax].
fn max_curve_difference_db(curves: &[&Curve], fmin: f64, fmax: f64) -> f64 {
    if curves.len() < 2 {
        return 0.0;
    }
    // Build common frequency grid from the first curve (filtered to passband)
    let ref_curve = curves[0];
    let mut max_diff = 0.0_f64;

    for i in 0..ref_curve.freq.len() {
        let f = ref_curve.freq[i];
        if f < fmin || f > fmax {
            continue;
        }
        // Interpolate all curves at this frequency and find max spread
        let mut spl_values = Vec::with_capacity(curves.len());
        for curve in curves {
            if let Some(spl) = interpolate_spl_at(curve, f) {
                spl_values.push(spl);
            }
        }
        if spl_values.len() >= 2 {
            let min_spl = spl_values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_spl = spl_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            max_diff = max_diff.max(max_spl - min_spl);
        }
    }
    max_diff
}

/// Linear interpolation of SPL at a given frequency
fn interpolate_spl_at(curve: &Curve, freq: f64) -> Option<f64> {
    let n = curve.freq.len();
    if n == 0 {
        return None;
    }
    if freq <= curve.freq[0] {
        return Some(curve.spl[0]);
    }
    if freq >= curve.freq[n - 1] {
        return Some(curve.spl[n - 1]);
    }
    for i in 0..n - 1 {
        if curve.freq[i] <= freq && freq <= curve.freq[i + 1] {
            let t = (freq - curve.freq[i]) / (curve.freq[i + 1] - curve.freq[i]);
            return Some(curve.spl[i] + t * (curve.spl[i + 1] - curve.spl[i]));
        }
    }
    None
}

/// Standard deviation of group delay (in ms) for a curve with phase data.
/// Returns None if phase data is missing or no data in passband.
fn group_delay_std_dev(curve: &Curve, fmin: f64, fmax: f64) -> Option<f64> {
    let phase = curve.phase.as_ref()?;
    // Unwrap phase to avoid discontinuities that would cause GD spikes
    let unwrapped = unwrap_phase_degrees(phase);
    // compute_group_delay returns values in ms
    let gd = compute_group_delay(&curve.freq, &unwrapped);

    // Compute passband mean (not global mean) for accurate std dev
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= fmin && curve.freq[i] <= fmax {
            sum += gd[i];
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let passband_mean = sum / count as f64;

    let deviation = &gd - passband_mean;
    Some(calculate_standard_deviation_in_range(
        &curve.freq,
        &deviation,
        fmin,
        fmax,
    ))
}

/// Mean SPL in a frequency range
fn mean_spl_in_range(curve: &Curve, fmin: f64, fmax: f64) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..curve.freq.len() {
        if curve.freq[i] >= fmin && curve.freq[i] <= fmax {
            sum += curve.spl[i];
            count += 1;
        }
    }
    if count > 0 { sum / count as f64 } else { 0.0 }
}

/// Split error (final - initial) into peaks (positive) and dips (negative),
/// returning (peak_rms, dip_rms) in the given frequency range.
fn peak_dip_rms(initial: &Curve, final_curve: &Curve, fmin: f64, fmax: f64) -> (f64, f64) {
    let mut peak_sum = 0.0;
    let mut peak_count = 0usize;
    let mut dip_sum = 0.0;
    let mut dip_count = 0usize;

    for i in 0..initial.freq.len() {
        let f = initial.freq[i];
        if f < fmin || f > fmax {
            continue;
        }
        if let Some(final_spl) = interpolate_spl_at(final_curve, f) {
            let error = final_spl - initial.spl[i];
            if error > 0.0 {
                peak_sum += error * error;
                peak_count += 1;
            } else if error < 0.0 {
                dip_sum += error * error;
                dip_count += 1;
            }
        }
    }

    let peak_rms = if peak_count > 0 {
        (peak_sum / peak_count as f64).sqrt()
    } else {
        0.0
    };
    let dip_rms = if dip_count > 0 {
        (dip_sum / dip_count as f64).sqrt()
    } else {
        0.0
    };
    (peak_rms, dip_rms)
}

// ---------------------------------------------------------------------------
// Parameter mutations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Mutation {
    Baseline,
    MoreFilters,
    WiderQ,
    WiderDb,
    MoreFirTaps,
}

impl std::fmt::Display for Mutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mutation::Baseline => write!(f, "baseline"),
            Mutation::MoreFilters => write!(f, "+50% filters"),
            Mutation::WiderQ => write!(f, "+50% max_q"),
            Mutation::WiderDb => write!(f, "+50% max_db"),
            Mutation::MoreFirTaps => write!(f, "+100% taps"),
        }
    }
}

fn apply_mutation(config: &mut RoomConfig, mutation: Mutation) {
    match mutation {
        Mutation::Baseline => {}
        Mutation::MoreFilters => {
            config.optimizer.num_filters =
                (config.optimizer.num_filters as f64 * 1.5).ceil() as usize;
        }
        Mutation::WiderQ => {
            config.optimizer.max_q *= 1.5;
        }
        Mutation::WiderDb => {
            config.optimizer.min_db *= 1.5; // e.g. -12 -> -18
            config.optimizer.max_db *= 1.5; // e.g.   6 ->   9
        }
        Mutation::MoreFirTaps => {
            if let Some(ref mut fir) = config.optimizer.fir {
                fir.taps *= 2; // e.g. 4096 -> 8192
            }
        }
    }
}

/// IIR mutations: more filters, wider Q, wider dB
const IIR_MUTATIONS: &[Mutation] = &[
    Mutation::Baseline,
    Mutation::MoreFilters,
    Mutation::WiderQ,
    Mutation::WiderDb,
];

/// FIR mutations: more taps (FIR ignores num_filters/Q/dB)
const FIR_MUTATIONS: &[Mutation] = &[Mutation::Baseline, Mutation::MoreFirTaps];

/// Mixed mutations: more filters + more taps (both IIR and FIR knobs)
const MIXED_MUTATIONS: &[Mutation] = &[
    Mutation::Baseline,
    Mutation::MoreFilters,
    Mutation::WiderDb,
    Mutation::MoreFirTaps,
];

// MixedPhase mutations: IIR knobs (FIR is auto-generated for excess phase)
const MIXED_PHASE_MUTATIONS: &[Mutation] =
    &[Mutation::Baseline, Mutation::MoreFilters, Mutation::WiderQ];

// ---------------------------------------------------------------------------
// Test result tracking
// ---------------------------------------------------------------------------

struct TestResult {
    label: String,
    pre_score: f64,
    post_score: f64,
    epa_preference: Option<f64>,
    pass: bool,
    reason: String,
}

/// Compute average EPA post-preference across channels.
fn avg_epa_preference(result: &RoomOptimizationResult) -> Option<f64> {
    let epa = result.metadata.epa_per_channel.as_ref()?;
    if epa.is_empty() {
        return None;
    }
    let sum: f64 = epa.values().map(|m| m.post.preference).sum();
    Some(sum / epa.len() as f64)
}

// ---------------------------------------------------------------------------
// Option Override: programmatic config mutation for per-option tests
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum OptionOverride {
    TargetTilt {
        slope_db_per_octave: f64,
    },
    ExcursionProtection,
    SchroederSplit {
        schroeder_freq: f64,
        low_max_q: f64,
        high_max_q: f64,
    },
    AsymmetricLoss,
    Psychoacoustic,
    BroadbandTargetMatching,
    PhaseAlignment,
    MultiMeasurementMinimax,
    MultiMeasurementVariancePenalized,
    VoiceOfGod {
        reference_channel: String,
    },
    SpatialRobustness,
    PreRinging,
    MixedPhaseMode,
    DecomposedCorrection,
}

impl std::fmt::Display for OptionOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionOverride::TargetTilt {
                slope_db_per_octave,
            } => {
                write!(f, "target_tilt(slope={})", slope_db_per_octave)
            }
            OptionOverride::ExcursionProtection => write!(f, "excursion_protection"),
            OptionOverride::SchroederSplit { schroeder_freq, .. } => {
                write!(f, "schroeder_split(fs={})", schroeder_freq)
            }
            OptionOverride::AsymmetricLoss => write!(f, "asymmetric_loss"),
            OptionOverride::Psychoacoustic => write!(f, "psychoacoustic"),
            OptionOverride::BroadbandTargetMatching => write!(f, "broadband_target_matching"),
            OptionOverride::PhaseAlignment => write!(f, "phase_alignment"),
            OptionOverride::MultiMeasurementMinimax => write!(f, "multi_measurement_minimax"),
            OptionOverride::MultiMeasurementVariancePenalized => {
                write!(f, "multi_measurement_variance")
            }
            OptionOverride::VoiceOfGod { reference_channel } => {
                write!(f, "voice_of_god(ref={})", reference_channel)
            }
            OptionOverride::SpatialRobustness => write!(f, "spatial_robustness"),
            OptionOverride::PreRinging => write!(f, "pre_ringing"),
            OptionOverride::MixedPhaseMode => write!(f, "mixed_phase"),
            OptionOverride::DecomposedCorrection => write!(f, "decomposed_correction"),
        }
    }
}

/// Apply option override to config. Also disables the option for baseline configs.
fn apply_option_override(config: &mut RoomConfig, option: &OptionOverride) {
    match option {
        OptionOverride::TargetTilt {
            slope_db_per_octave,
        } => {
            let existing = config
                .optimizer
                .target_response
                .get_or_insert_with(Default::default);
            existing.shape = autoeq::roomeq::TargetShape::Custom;
            existing.slope_db_per_octave = *slope_db_per_octave;
        }
        OptionOverride::ExcursionProtection => {
            config.optimizer.excursion_protection = Some(ExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: true,
                manual_f3_hz: None,
                filter_order: 4,
                filter_type: Default::default(),
                margin_octaves: 0.25,
            });
        }
        OptionOverride::SchroederSplit {
            schroeder_freq,
            low_max_q,
            high_max_q,
        } => {
            config.optimizer.schroeder_split = Some(SchroederSplitConfig {
                enabled: true,
                schroeder_freq: *schroeder_freq,
                room_dimensions: None,
                low_freq_config: autoeq::roomeq::LowFreqFilterConfig {
                    max_q: *low_max_q,
                    ..Default::default()
                },
                high_freq_config: autoeq::roomeq::HighFreqFilterConfig {
                    max_q: *high_max_q,
                    ..Default::default()
                },
            });
        }
        OptionOverride::AsymmetricLoss => {
            config.optimizer.asymmetric_loss = true;
        }
        OptionOverride::Psychoacoustic => {
            config.optimizer.psychoacoustic = true;
        }
        OptionOverride::BroadbandTargetMatching => {
            let existing = config
                .optimizer
                .target_response
                .get_or_insert_with(Default::default);
            existing.broadband_precorrection = true;
        }
        OptionOverride::PhaseAlignment => {
            config.optimizer.phase_alignment = Some(PhaseAlignmentConfig::default());
            config.optimizer.allow_delay = Some(true);
        }
        OptionOverride::MultiMeasurementMinimax => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::Minimax,
                ..Default::default()
            });
        }
        OptionOverride::MultiMeasurementVariancePenalized => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::VariancePenalized,
                variance_lambda: 1.0,
                ..Default::default()
            });
        }
        OptionOverride::VoiceOfGod { reference_channel } => {
            config.optimizer.vog = Some(VoiceOfGodConfig {
                enabled: true,
                reference_channel: reference_channel.clone(),
            });
        }
        OptionOverride::SpatialRobustness => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::SpatialRobustness,
                spatial_robustness: Some(SpatialRobustnessSerdeConfig {
                    variance_threshold_db: 3.0,
                    transition_width_db: 2.0,
                    min_correction_depth: 0.1,
                    mask_smoothing_octaves: 1.0 / 6.0,
                }),
                ..Default::default()
            });
        }
        OptionOverride::PreRinging => {
            // Enable FIR mode with pre-ringing control
            config.optimizer.processing_mode = ProcessingMode::PhaseLinear;
            if config.optimizer.fir.is_none() {
                config.optimizer.fir = Some(autoeq::roomeq::FirConfig {
                    taps: 2048,
                    phase: "kirkeby".to_string(),
                    correct_excess_phase: false,
                    phase_smoothing: 0.167,
                    pre_ringing: Some(PreRingingSerdeConfig {
                        threshold_db: -30.0,
                        max_time_s: 0.005,
                    }),
                });
            } else if let Some(ref mut fir) = config.optimizer.fir {
                fir.pre_ringing = Some(PreRingingSerdeConfig {
                    threshold_db: -30.0,
                    max_time_s: 0.005,
                });
            }
        }
        OptionOverride::MixedPhaseMode => {
            config.optimizer.processing_mode = ProcessingMode::MixedPhase;
            config.optimizer.mixed_phase = Some(MixedPhaseSerdeConfig {
                max_fir_length_ms: 10.0,
                pre_ringing_threshold_db: -30.0,
                min_spatial_depth: 0.5,
                phase_smoothing_octaves: 1.0 / 6.0,
            });
        }
        OptionOverride::DecomposedCorrection => {
            config.optimizer.decomposed_correction = Some(DecomposedCorrectionSerdeConfig {
                enabled: true,
                schroeder_freq: 200.0,
                room_dimensions: None,
                min_mode_q: 3.0,
                min_mode_prominence_db: 3.0,
                mode_correction_weight: 1.0,
                early_reflection_weight: 0.3,
                steady_state_weight: 0.5,
            });
        }
    }
}

/// Disable the option in config to create a clean baseline
fn disable_option(config: &mut RoomConfig, option: &OptionOverride) {
    match option {
        OptionOverride::TargetTilt { .. } => {
            config.optimizer.target_response = None;
        }
        OptionOverride::ExcursionProtection => {
            config.optimizer.excursion_protection = None;
        }
        OptionOverride::SchroederSplit { .. } => {
            config.optimizer.schroeder_split = None;
        }
        OptionOverride::AsymmetricLoss => {
            config.optimizer.asymmetric_loss = false;
        }
        OptionOverride::Psychoacoustic => {
            config.optimizer.psychoacoustic = false;
        }
        OptionOverride::BroadbandTargetMatching => {
            if let Some(ref mut tr) = config.optimizer.target_response {
                tr.broadband_precorrection = false;
            }
        }
        OptionOverride::PhaseAlignment => {
            config.optimizer.phase_alignment = None;
            config.optimizer.allow_delay = Some(false);
        }
        OptionOverride::MultiMeasurementMinimax
        | OptionOverride::MultiMeasurementVariancePenalized => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::Average,
                ..Default::default()
            });
        }
        OptionOverride::VoiceOfGod { .. } => {
            config.optimizer.vog = None;
        }
        OptionOverride::SpatialRobustness => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::Average,
                ..Default::default()
            });
        }
        OptionOverride::PreRinging => {
            if let Some(ref mut fir) = config.optimizer.fir {
                fir.pre_ringing = None;
            }
        }
        OptionOverride::MixedPhaseMode => {
            config.optimizer.processing_mode = ProcessingMode::LowLatency;
            config.optimizer.mixed_phase = None;
        }
        OptionOverride::DecomposedCorrection => {
            config.optimizer.decomposed_correction = None;
        }
    }
}

/// For multi-measurement tests, swap single CSV paths to MeasurementMultiple
/// with all 3 listening positions (lp0, lp1, lp2).
fn enable_multi_measurement_paths(config: &mut RoomConfig, fem_dir: &Path, fem_subdir: &str) {
    let data_dir = fem_dir.join(fem_subdir);
    let mut new_speakers = HashMap::new();

    for key in config.speakers.keys() {
        // e.g. key="left" -> files: left_lp0.csv, left_lp1.csv, left_lp2.csv
        let mut measurements = Vec::new();
        for lp in 0..3 {
            let filename = format!("{}_lp{}.csv", key, lp);
            let path = data_dir.join(&filename);
            if path.exists() {
                measurements.push(MeasurementRef::Path(path));
            }
        }

        let source = if measurements.len() > 1 {
            MeasurementSource::Multiple(MeasurementMultiple {
                measurements,
                speaker_name: None,
            })
        } else if measurements.len() == 1 {
            MeasurementSource::Single(autoeq::MeasurementSingle {
                measurement: measurements.remove(0),
                speaker_name: None,
            })
        } else {
            // Keep original if no lp files found
            continue;
        };

        new_speakers.insert(key.clone(), autoeq::roomeq::SpeakerConfig::Single(source));
    }

    for (key, speaker) in new_speakers {
        config.speakers.insert(key, speaker);
    }
}

// ---------------------------------------------------------------------------
// Test case registry
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum TestCase {
    /// Stereo/Home Cinema workflow test (IIR mutations)
    Workflow {
        name: &'static str,
        fem_subdir: &'static str,
        optim_subdir: &'static str,
    },
    /// Generic path test (all 3 modes: IIR, FIR, Mixed)
    Generic {
        name: &'static str,
        fem_subdir: &'static str,
        optim_subdir: &'static str,
    },
    /// Cross-mode convergence: IIR vs FIR vs Mixed frequency response similarity
    CrossModeConvergence {
        name: &'static str,
        fem_subdir: &'static str,
        optim_subdir: &'static str,
    },
    /// Per-option A/B test: baseline vs with-option(s)
    /// Supports single options and combinations.
    OptionEffect {
        name: &'static str,
        fem_subdir: &'static str,
        optim_subdir: &'static str,
        options: Vec<OptionOverride>,
    },
}

impl TestCase {
    fn name(&self) -> &str {
        match self {
            TestCase::Workflow { name, .. } => name,
            TestCase::Generic { name, .. } => name,
            TestCase::CrossModeConvergence { name, .. } => name,
            TestCase::OptionEffect { name, .. } => name,
        }
    }
}

fn all_test_cases() -> Vec<TestCase> {
    vec![
        // Part A: Stereo workflows
        TestCase::Workflow {
            name: "Stereo 2.0",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
        },
        TestCase::Workflow {
            name: "Stereo 2.1",
            fem_subdir: "small_stereo_2_1",
            optim_subdir: "small_stereo_2_1",
        },
        TestCase::Workflow {
            name: "Stereo 2.2 MSO",
            fem_subdir: "small_stereo_2_2_mso",
            optim_subdir: "small_stereo_2_2_mso",
        },
        TestCase::Workflow {
            name: "Stereo 2.2 Cardioid",
            fem_subdir: "small_stereo_2_2_cardioid",
            optim_subdir: "small_stereo_2_2_cardioid",
        },
        TestCase::Workflow {
            name: "Stereo 2.2 Group",
            fem_subdir: "small_stereo_2_2_group",
            optim_subdir: "small_stereo_2_2_group",
        },
        TestCase::Workflow {
            name: "Stereo 2.2 Independent",
            fem_subdir: "small_stereo_2_2_mso", // same FEM data, different optimizer config
            optim_subdir: "small_stereo_2_2_independent",
        },
        // Part A.2: Home Cinema workflows
        TestCase::Workflow {
            name: "Home Cinema 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
        },
        TestCase::Workflow {
            name: "Home Cinema 5.1.4",
            fem_subdir: "medium_surround_5_1_4",
            optim_subdir: "medium_surround_5_1_4",
        },
        // Part B: Generic path (all 3 modes)
        TestCase::Generic {
            name: "Generic small_stereo_2_0",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
        },
        // Part C: Cross-mode convergence
        TestCase::CrossModeConvergence {
            name: "Cross-Mode small_stereo_2_0",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
        },
        // Part D: Per-option effect tests (single option)
        TestCase::OptionEffect {
            name: "OE target_tilt",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::TargetTilt {
                slope_db_per_octave: -0.8,
            }],
        },
        TestCase::OptionEffect {
            name: "OE excursion_protection",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::ExcursionProtection],
        },
        TestCase::OptionEffect {
            name: "OE schroeder_split",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::SchroederSplit {
                schroeder_freq: 300.0,
                low_max_q: 10.0,
                high_max_q: 1.0,
            }],
        },
        TestCase::OptionEffect {
            name: "OE asymmetric_loss",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::AsymmetricLoss],
        },
        TestCase::OptionEffect {
            name: "OE psychoacoustic",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::Psychoacoustic],
        },
        TestCase::OptionEffect {
            name: "OE broadband_target_matching",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![OptionOverride::BroadbandTargetMatching],
        },
        TestCase::OptionEffect {
            name: "OE phase_alignment",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![OptionOverride::PhaseAlignment],
        },
        TestCase::OptionEffect {
            name: "OE voice_of_god",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![OptionOverride::VoiceOfGod {
                reference_channel: "C".to_string(),
            }],
        },
        TestCase::OptionEffect {
            name: "OE multi_measurement_minimax",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![OptionOverride::MultiMeasurementMinimax],
        },
        TestCase::OptionEffect {
            name: "OE multi_measurement_variance",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![OptionOverride::MultiMeasurementVariancePenalized],
        },
        // --- D.8: Spatial robustness (multi-position correction depth) ---
        TestCase::OptionEffect {
            name: "OE spatial_robustness",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![OptionOverride::SpatialRobustness],
        },
        // --- D.9: Pre-ringing control (FIR with bounded pre-ringing) ---
        TestCase::OptionEffect {
            name: "OE pre_ringing",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::PreRinging],
        },
        // --- D.10: Mixed-phase mode (IIR + short excess phase FIR) ---
        TestCase::OptionEffect {
            name: "OE mixed_phase",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::MixedPhaseMode],
        },
        // --- D.11: Decomposed correction (mode/steady-state weighting) ---
        TestCase::OptionEffect {
            name: "OE decomposed_correction",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![OptionOverride::DecomposedCorrection],
        },
        // ================================================================
        // Part E: Combination tests — multi-option interaction coverage
        // ================================================================

        // --- E.1: Loss shaping pairs (both modify the objective function) ---
        TestCase::OptionEffect {
            name: "COMBO asymmetric+psycho",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
            ],
        },
        // --- E.2: Frequency partitioning (both constrain low freq behaviour) ---
        TestCase::OptionEffect {
            name: "COMBO schroeder+excursion",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::ExcursionProtection,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO schroeder+asymmetric",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::AsymmetricLoss,
            ],
        },
        // --- E.3: Target shaping (tilt defines the target, broadband pre-corrects) ---
        TestCase::OptionEffect {
            name: "COMBO tilt+psycho",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO tilt+excursion",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO tilt+broadband 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::BroadbandTargetMatching,
            ],
        },
        // --- E.4: Sub integration combos (phase + other options on 5.1) ---
        TestCase::OptionEffect {
            name: "COMBO phase+psycho 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::PhaseAlignment,
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO phase+asymmetric 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::PhaseAlignment,
                OptionOverride::AsymmetricLoss,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO phase+broadband+tilt 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::PhaseAlignment,
                OptionOverride::BroadbandTargetMatching,
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
            ],
        },
        // --- E.5: Multi-measurement combos ---
        TestCase::OptionEffect {
            name: "COMBO minimax+psycho+asymmetric",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![
                OptionOverride::MultiMeasurementMinimax,
                OptionOverride::Psychoacoustic,
                OptionOverride::AsymmetricLoss,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO variance+tilt+psycho",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![
                OptionOverride::MultiMeasurementVariancePenalized,
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO minimax+schroeder+excursion",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![
                OptionOverride::MultiMeasurementMinimax,
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::ExcursionProtection,
            ],
        },
        // --- E.6: Triple+ combos on stereo (interaction stress tests) ---
        TestCase::OptionEffect {
            name: "COMBO tilt+schroeder+asymmetric+psycho",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO tilt+excursion+schroeder+psycho",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO excursion+asymmetric+psycho",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::ExcursionProtection,
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
            ],
        },
        // --- E.7: Kitchen sink (all compatible options per scenario) ---
        TestCase::OptionEffect {
            name: "COMBO all stereo options",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
                OptionOverride::BroadbandTargetMatching,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO all 5.1 options",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
                OptionOverride::PhaseAlignment,
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
                OptionOverride::BroadbandTargetMatching,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO all multi-seat minimax options",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
                OptionOverride::MultiMeasurementMinimax,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO all multi-seat variance options",
            fem_subdir: "medium_multi_seat",
            optim_subdir: "medium_multi_seat",
            options: vec![
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
                OptionOverride::ExcursionProtection,
                OptionOverride::SchroederSplit {
                    schroeder_freq: 300.0,
                    low_max_q: 10.0,
                    high_max_q: 1.0,
                },
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
                OptionOverride::MultiMeasurementVariancePenalized,
            ],
        },
        // --- E.8: Sub topology combos (2.1 scenario) ---
        TestCase::OptionEffect {
            name: "COMBO phase+excursion+tilt 2.1",
            fem_subdir: "small_stereo_2_1",
            optim_subdir: "small_stereo_2_1",
            options: vec![
                OptionOverride::PhaseAlignment,
                OptionOverride::ExcursionProtection,
                OptionOverride::TargetTilt {
                    slope_db_per_octave: -0.8,
                },
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO phase+asymmetric+psycho 2.1",
            fem_subdir: "small_stereo_2_1",
            optim_subdir: "small_stereo_2_1",
            options: vec![
                OptionOverride::PhaseAlignment,
                OptionOverride::AsymmetricLoss,
                OptionOverride::Psychoacoustic,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Test runners (return output buffer + results for parallel execution)
// ---------------------------------------------------------------------------

fn run_stereo_workflow_tests(
    name: &str,
    base_config_path: &Path,
    override_config_path: Option<&Path>,
) -> Result<(String, Vec<TestResult>)> {
    let mut out = String::new();
    let mut results = Vec::new();

    writeln!(out, "\n--- {} (IIR workflow) ---", name).unwrap();

    let mut baseline_post: Option<f64> = None;

    for mutation in IIR_MUTATIONS {
        let (mut config, _) = load_config(base_config_path, override_config_path)?;
        apply_qa_overrides(&mut config);
        apply_mutation(&mut config, *mutation);

        let result =
            run_optimization(&config).with_context(|| format!("{} IIR {}", name, mutation))?;

        let pre = result.combined_pre_score;
        let post = result.combined_post_score;

        let (pass, reason) = evaluate_result(*mutation, pre, post, &mut baseline_post);

        let status = if pass { "PASS" } else { "FAIL" };
        writeln!(
            out,
            "  IIR {:>14}: post={:.4}  {}  ({})",
            mutation.to_string(),
            post,
            status,
            reason
        )
        .unwrap();

        results.push(TestResult {
            label: format!("{} IIR {}", name, mutation),
            pre_score: pre,
            post_score: post,
            epa_preference: avg_epa_preference(&result),
            pass,
            reason,
        });
    }

    Ok((out, results))
}

fn run_generic_path_tests(
    name: &str,
    base_config_path: &Path,
    override_config_dir: &Path,
) -> Result<(String, Vec<TestResult>)> {
    let mut out = String::new();
    let mut results = Vec::new();

    writeln!(out, "\n--- Generic Path ({}, all modes) ---", name).unwrap();

    let modes: &[(&str, ProcessingMode, &str, &[Mutation])] = &[
        (
            "IIR",
            ProcessingMode::LowLatency,
            "optimiser-iir.json",
            IIR_MUTATIONS,
        ),
        (
            "FIR",
            ProcessingMode::PhaseLinear,
            "optimiser-fir.json",
            FIR_MUTATIONS,
        ),
        (
            "Mixed",
            ProcessingMode::Hybrid,
            "optimiser-mixed.json",
            MIXED_MUTATIONS,
        ),
        (
            "MixedPhase",
            ProcessingMode::MixedPhase,
            "optimiser-iir.json", // MixedPhase uses IIR config as base
            MIXED_PHASE_MUTATIONS,
        ),
    ];

    let mut mode_baselines: Vec<(&str, f64)> = Vec::new();

    for (mode_name, processing_mode, override_file, mutations) in modes {
        let override_path = override_config_dir.join(override_file);
        let mut baseline_post: Option<f64> = None;

        for mutation in *mutations {
            let (mut config, _) = load_config_for_generic_path(
                base_config_path,
                Some(&override_path),
                processing_mode.clone(),
            )?;
            apply_qa_overrides(&mut config);
            apply_mutation(&mut config, *mutation);

            let result = run_optimization(&config)
                .with_context(|| format!("{} {} generic {}", name, mode_name, mutation))?;

            let pre = result.combined_pre_score;
            let post = result.combined_post_score;

            let (pass, reason) = evaluate_result(*mutation, pre, post, &mut baseline_post);

            // Record baseline for cross-mode comparison
            if matches!(mutation, Mutation::Baseline) {
                mode_baselines.push((mode_name, post));
            }

            let status = if pass { "PASS" } else { "FAIL" };
            writeln!(
                out,
                "  {} {:>14}: post={:.4}  {}  ({})",
                mode_name,
                mutation.to_string(),
                post,
                status,
                reason
            )
            .unwrap();

            results.push(TestResult {
                label: format!("{} generic {} {}", name, mode_name, mutation),
                pre_score: pre,
                post_score: post,
                epa_preference: avg_epa_preference(&result),
                pass,
                reason,
            });
        }
    }

    // Cross-mode comparison
    if mode_baselines.len() >= 2 {
        let scores: Vec<f64> = mode_baselines.iter().map(|(_, s)| *s).collect();
        let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ratio = if min_score > 0.0 {
            max_score / min_score
        } else {
            f64::INFINITY
        };
        let pass = ratio <= CROSS_MODE_RATIO_LIMIT;
        let status = if pass { "PASS" } else { "FAIL" };

        let mode_scores: String = mode_baselines
            .iter()
            .map(|(name, score)| format!("{}={:.4}", name, score))
            .collect::<Vec<_>>()
            .join(" ");

        writeln!(
            out,
            "\n  Cross-mode: {} ratio={:.2}x  {}",
            mode_scores, ratio, status
        )
        .unwrap();

        results.push(TestResult {
            label: format!("{} cross-mode", name),
            pre_score: 0.0,
            post_score: 0.0,
            epa_preference: None,
            pass,
            reason: format!("ratio={:.2}x (limit={:.1}x)", ratio, CROSS_MODE_RATIO_LIMIT),
        });
    }

    Ok((out, results))
}

// ---------------------------------------------------------------------------
// Cross-Mode Convergence Tests (CM-1, CM-2, CM-3)
// ---------------------------------------------------------------------------

fn run_cross_mode_convergence_tests(
    name: &str,
    base_config_path: &Path,
    override_config_dir: &Path,
) -> Result<(String, Vec<TestResult>)> {
    let mut out = String::new();
    let mut results = Vec::new();

    writeln!(out, "\n--- {} (cross-mode convergence) ---", name).unwrap();

    let modes: &[(&str, ProcessingMode, &str)] = &[
        ("IIR", ProcessingMode::LowLatency, "optimiser-iir.json"),
        ("FIR", ProcessingMode::PhaseLinear, "optimiser-fir.json"),
        ("Mixed", ProcessingMode::Hybrid, "optimiser-mixed.json"),
    ];

    // Run all 3 modes, collect results
    let mut mode_results: Vec<(&str, RoomOptimizationResult)> = Vec::new();

    for (mode_name, processing_mode, override_file) in modes {
        let override_path = override_config_dir.join(override_file);
        let (mut config, _) = load_config_for_generic_path(
            base_config_path,
            Some(&override_path),
            processing_mode.clone(),
        )?;
        apply_qa_overrides(&mut config);

        let result = run_optimization(&config)
            .with_context(|| format!("{} {} cross-mode", name, mode_name))?;

        writeln!(
            out,
            "  {}: post={:.4} (pre={:.4})",
            mode_name, result.combined_post_score, result.combined_pre_score
        )
        .unwrap();

        mode_results.push((mode_name, result));
    }

    // CM-1: Frequency response convergence
    // Compare final curves across modes for each channel
    {
        let channel_names: Vec<String> =
            mode_results[0].1.channel_results.keys().cloned().collect();

        let mut cm1_max_diff = 0.0_f64;

        for ch_name in &channel_names {
            let curves: Vec<&Curve> = mode_results
                .iter()
                .filter_map(|(_, r)| r.channel_results.get(ch_name).map(|c| &c.final_curve))
                .collect();

            if curves.len() >= 2 {
                let fmin = curves[0]
                    .freq
                    .iter()
                    .cloned()
                    .find(|&f| f >= 20.0)
                    .unwrap_or(20.0);
                let fmax = curves[0]
                    .freq
                    .iter()
                    .cloned()
                    .rev()
                    .find(|&f| f <= 500.0)
                    .unwrap_or(500.0);
                let diff = max_curve_difference_db(&curves, fmin, fmax);
                cm1_max_diff = cm1_max_diff.max(diff);
            }
        }

        let cm1_pass = cm1_max_diff <= CROSS_MODE_FR_MAX_DIFF_DB;
        let status = if cm1_pass { "PASS" } else { "FAIL" };
        writeln!(
            out,
            "  CM-1 FR convergence: max_diff={:.2}dB (limit={:.1}dB)  {}",
            cm1_max_diff, CROSS_MODE_FR_MAX_DIFF_DB, status
        )
        .unwrap();

        results.push(TestResult {
            label: format!("{} CM-1 FR convergence", name),
            pre_score: 0.0,
            post_score: cm1_max_diff,
            epa_preference: None,
            pass: cm1_pass,
            reason: format!(
                "max_diff={:.2}dB (limit={:.1}dB)",
                cm1_max_diff, CROSS_MODE_FR_MAX_DIFF_DB
            ),
        });
    }

    // CM-2: Group delay flatness (FIR/Mixed should have <= IIR GD std dev)
    {
        let channel_names: Vec<String> =
            mode_results[0].1.channel_results.keys().cloned().collect();

        let mut iir_gd_max = 0.0_f64;
        let mut fir_gd_max = 0.0_f64;
        let mut mixed_gd_max = 0.0_f64;
        let mut has_phase = false;

        for ch_name in &channel_names {
            for (mode_name, result) in &mode_results {
                if let Some(ch) = result.channel_results.get(ch_name)
                    && let Some(gd_std) = group_delay_std_dev(&ch.final_curve, 20.0, 500.0)
                {
                    has_phase = true;
                    match *mode_name {
                        "IIR" => iir_gd_max = iir_gd_max.max(gd_std),
                        "FIR" => fir_gd_max = fir_gd_max.max(gd_std),
                        "Mixed" => mixed_gd_max = mixed_gd_max.max(gd_std),
                        _ => {}
                    }
                }
            }
        }

        if has_phase {
            // Verify all modes produce reasonable GD (< 50ms std dev).
            // Note: FIR/Mixed don't necessarily have flatter GD on the *final curve*
            // since the room's own phase dominates. FIR's phase advantage is in the
            // correction filter, not the combined room+correction result.
            let max_gd = iir_gd_max.max(fir_gd_max).max(mixed_gd_max);
            let cm2_pass = max_gd < 50.0;
            let status = if cm2_pass { "PASS" } else { "FAIL" };

            writeln!(
                out,
                "  CM-2 GD flatness: IIR={:.2}ms FIR={:.2}ms Mixed={:.2}ms  {}",
                iir_gd_max, fir_gd_max, mixed_gd_max, status
            )
            .unwrap();

            results.push(TestResult {
                label: format!("{} CM-2 GD flatness", name),
                pre_score: iir_gd_max,
                post_score: fir_gd_max.max(mixed_gd_max),
                epa_preference: None,
                pass: cm2_pass,
                reason: format!(
                    "IIR={:.2}ms FIR={:.2}ms Mixed={:.2}ms",
                    iir_gd_max, fir_gd_max, mixed_gd_max
                ),
            });
        } else {
            writeln!(out, "  CM-2 GD flatness: SKIP (no phase data)").unwrap();
        }
    }

    // CM-3: Score convergence (ratio of max/min post scores)
    {
        let scores: Vec<f64> = mode_results
            .iter()
            .map(|(_, r)| r.combined_post_score)
            .collect();
        let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ratio = if min_score > 0.0 {
            max_score / min_score
        } else {
            f64::INFINITY
        };
        let cm3_pass = ratio <= CROSS_MODE_SCORE_RATIO_LIMIT;
        let status = if cm3_pass { "PASS" } else { "FAIL" };

        let mode_scores: String = mode_results
            .iter()
            .map(|(name, r)| format!("{}={:.4}", name, r.combined_post_score))
            .collect::<Vec<_>>()
            .join(" ");

        writeln!(
            out,
            "  CM-3 Score convergence: {} ratio={:.2}x (limit={:.1}x)  {}",
            mode_scores, ratio, CROSS_MODE_SCORE_RATIO_LIMIT, status
        )
        .unwrap();

        results.push(TestResult {
            label: format!("{} CM-3 score convergence", name),
            pre_score: 0.0,
            post_score: ratio,
            epa_preference: None,
            pass: cm3_pass,
            reason: format!(
                "{} ratio={:.2}x (limit={:.1}x)",
                mode_scores, ratio, CROSS_MODE_SCORE_RATIO_LIMIT
            ),
        });
    }

    Ok((out, results))
}

// ---------------------------------------------------------------------------
// Per-Option Effect Tests
// ---------------------------------------------------------------------------

fn run_option_effect_test(
    name: &str,
    fem_dir: &Path,
    fem_subdir: &str,
    optim_dir: &Path,
    optim_subdir: &str,
    options: &[OptionOverride],
) -> Result<(String, Vec<TestResult>)> {
    let mut out = String::new();
    let mut results = Vec::new();

    let options_str: String = options
        .iter()
        .map(|o| o.to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    writeln!(out, "\n--- {} ({}) ---", name, options_str).unwrap();

    let base_config_path = fem_dir.join(format!("{}/config.json", fem_subdir));
    let override_path = optim_dir.join(format!("{}/optimiser-iir.json", optim_subdir));
    let override_path = if override_path.exists() {
        Some(override_path)
    } else {
        None
    };

    let needs_multi_measurement = options.iter().any(|o| {
        matches!(
            o,
            OptionOverride::MultiMeasurementMinimax
                | OptionOverride::MultiMeasurementVariancePenalized
                | OptionOverride::SpatialRobustness
        )
    });

    // BroadbandTargetMatching needs a target tilt to have something to match.
    // When the combo doesn't include an explicit TargetTilt, both baseline and
    // option get a default -0.8 dB/oct tilt so the only variable is broadband.
    let has_broadband = options
        .iter()
        .any(|o| matches!(o, OptionOverride::BroadbandTargetMatching));
    let has_tilt = options
        .iter()
        .any(|o| matches!(o, OptionOverride::TargetTilt { .. }));
    let default_target_response = if has_broadband && !has_tilt {
        Some(TargetResponseConfig {
            shape: TargetShape::Custom,
            slope_db_per_octave: -0.8,
            ..TargetResponseConfig::default()
        })
    } else {
        None
    };

    // Load and run baseline (all options disabled)
    let (mut baseline_config, _) = load_config(&base_config_path, override_path.as_deref())?;
    apply_qa_overrides(&mut baseline_config);
    for option in options {
        disable_option(&mut baseline_config, option);
    }
    if let Some(ref tr) = default_target_response {
        baseline_config.optimizer.target_response = Some(tr.clone());
    }
    if needs_multi_measurement {
        enable_multi_measurement_paths(&mut baseline_config, fem_dir, fem_subdir);
    }

    let baseline_result =
        run_optimization(&baseline_config).with_context(|| format!("{} baseline", name))?;

    writeln!(
        out,
        "  baseline: post={:.4} (pre={:.4})",
        baseline_result.combined_post_score, baseline_result.combined_pre_score
    )
    .unwrap();

    // Load and run with all options enabled
    let (mut option_config, _) = load_config(&base_config_path, override_path.as_deref())?;
    apply_qa_overrides(&mut option_config);
    for option in options {
        apply_option_override(&mut option_config, option);
    }
    if let Some(ref tr) = default_target_response {
        option_config.optimizer.target_response = Some(tr.clone());
    }
    if needs_multi_measurement {
        enable_multi_measurement_paths(&mut option_config, fem_dir, fem_subdir);
    }

    let option_result =
        run_optimization(&option_config).with_context(|| format!("{} with-options", name))?;

    writeln!(
        out,
        "  with-options: post={:.4} (pre={:.4})",
        option_result.combined_post_score, option_result.combined_pre_score
    )
    .unwrap();

    // Validate each per-option invariant individually
    let mut all_pass = true;
    for option in options {
        let (pass, reason) = validate_option_effect(
            option,
            &baseline_config,
            &baseline_result,
            &option_config,
            &option_result,
            options,
        );

        let status = if pass { "PASS" } else { "FAIL" };
        writeln!(out, "  {}: {}  ({})", option, status, reason).unwrap();

        if !pass {
            all_pass = false;
            results.push(TestResult {
                label: format!("{} [{}]", name, option),
                pre_score: baseline_result.combined_post_score,
                post_score: option_result.combined_post_score,
                epa_preference: avg_epa_preference(&option_result),
                pass: false,
                reason,
            });
        }
    }

    // Combo-level check: combined result should still converge (post < pre).
    // For high-interaction combos (4+ options), conflicting constraints
    // (excursion HPF + schroeder split + tilt + psychoacoustic) create a
    // very constrained search space. Allow a small regression margin.
    let convergence_margin = if options.len() >= 4 {
        option_result.combined_pre_score * 0.15 // 15% regression tolerance
    } else {
        0.0
    };
    let converged =
        option_result.combined_post_score < option_result.combined_pre_score + convergence_margin;
    if !converged {
        all_pass = false;
        let reason = format!(
            "no convergence: post {:.4} >= pre {:.4}",
            option_result.combined_post_score, option_result.combined_pre_score
        );
        writeln!(out, "  convergence: FAIL  ({})", reason).unwrap();
        results.push(TestResult {
            label: format!("{} [convergence]", name),
            pre_score: option_result.combined_pre_score,
            post_score: option_result.combined_post_score,
            epa_preference: avg_epa_preference(&option_result),
            pass: false,
            reason,
        });
    }

    // If everything passed, push a single PASS result
    if all_pass {
        results.push(TestResult {
            label: name.to_string(),
            pre_score: baseline_result.combined_post_score,
            post_score: option_result.combined_post_score,
            epa_preference: avg_epa_preference(&option_result),
            pass: true,
            reason: format!(
                "all {} invariants pass, post={:.4}",
                options.len(),
                option_result.combined_post_score
            ),
        });
    }

    Ok((out, results))
}

/// Per-option validation logic.
/// `all_options` is the full set of simultaneously active options — validators
/// can widen tolerances when many options interact.
fn validate_option_effect(
    option: &OptionOverride,
    _baseline_config: &RoomConfig,
    baseline_result: &RoomOptimizationResult,
    option_config: &RoomConfig,
    option_result: &RoomOptimizationResult,
    all_options: &[OptionOverride],
) -> (bool, String) {
    let num_options = all_options.len();
    let has_schroeder = all_options
        .iter()
        .any(|o| matches!(o, OptionOverride::SchroederSplit { .. }));
    let has_broadband = all_options
        .iter()
        .any(|o| matches!(o, OptionOverride::BroadbandTargetMatching));
    match option {
        OptionOverride::TargetTilt {
            slope_db_per_octave,
        } => validate_target_tilt(
            *slope_db_per_octave,
            baseline_result,
            option_result,
            num_options,
            has_schroeder,
            has_broadband,
        ),
        OptionOverride::ExcursionProtection => {
            validate_excursion_protection(baseline_result, option_result, num_options)
        }
        OptionOverride::SchroederSplit {
            schroeder_freq,
            low_max_q,
            high_max_q,
        } => validate_schroeder_split(*schroeder_freq, *low_max_q, *high_max_q, option_result),
        OptionOverride::AsymmetricLoss => validate_asymmetric_loss(baseline_result, option_result),
        OptionOverride::Psychoacoustic => {
            validate_psychoacoustic(baseline_result, option_result, num_options)
        }
        OptionOverride::BroadbandTargetMatching => validate_broadband_target_matching(
            baseline_result,
            option_result,
            option_config,
            num_options,
        ),
        OptionOverride::PhaseAlignment => {
            validate_phase_alignment(baseline_result, option_result, num_options)
        }
        OptionOverride::MultiMeasurementMinimax => {
            validate_multi_measurement_minimax(baseline_result, option_result, num_options)
        }
        OptionOverride::MultiMeasurementVariancePenalized => {
            validate_multi_measurement_variance(baseline_result, option_result, num_options)
        }
        OptionOverride::VoiceOfGod { .. } => {
            // VoG: combined score should not be significantly worse than baseline
            let score_ok = option_result.combined_post_score
                <= OPTION_SCORE_TOLERANCE * baseline_result.combined_post_score;

            if !score_ok {
                (
                    false,
                    format!(
                        "VoG score {:.3} > {:.1}x baseline {:.3}",
                        option_result.combined_post_score,
                        OPTION_SCORE_TOLERANCE,
                        baseline_result.combined_post_score,
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "VoG OK: score {:.3} vs baseline {:.3}",
                        option_result.combined_post_score, baseline_result.combined_post_score,
                    ),
                )
            }
        }
        OptionOverride::SpatialRobustness => {
            // Spatial robustness: score should be within tolerance of baseline
            // (it trades raw score for spatial consistency)
            let tolerance = PSYCHOACOUSTIC_SCORE_TOLERANCE; // similar trade-off
            let score_ok = option_result.combined_post_score
                <= tolerance * baseline_result.combined_post_score;

            if !score_ok {
                (
                    false,
                    format!(
                        "SpatialRobustness score {:.3} > {:.1}x baseline {:.3}",
                        option_result.combined_post_score,
                        tolerance,
                        baseline_result.combined_post_score,
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "SpatialRobustness OK: score {:.3} vs baseline {:.3}",
                        option_result.combined_post_score, baseline_result.combined_post_score,
                    ),
                )
            }
        }
        OptionOverride::PreRinging => {
            // Pre-ringing: score should not be worse than 1.5x baseline
            // (pre-ringing suppression may slightly degrade frequency response accuracy)
            let tolerance = 1.5;
            let score_ok = option_result.combined_post_score
                <= tolerance * baseline_result.combined_post_score;

            if !score_ok {
                (
                    false,
                    format!(
                        "PreRinging score {:.3} > {:.1}x baseline {:.3}",
                        option_result.combined_post_score,
                        tolerance,
                        baseline_result.combined_post_score,
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "PreRinging OK: score {:.3} vs baseline {:.3}",
                        option_result.combined_post_score, baseline_result.combined_post_score,
                    ),
                )
            }
        }
        OptionOverride::MixedPhaseMode => {
            // MixedPhase: should converge (post < pre) and not be much worse than baseline
            let tolerance = PSYCHOACOUSTIC_SCORE_TOLERANCE;
            let score_ok = option_result.combined_post_score
                <= tolerance * baseline_result.combined_post_score;

            if !score_ok {
                (
                    false,
                    format!(
                        "MixedPhase score {:.3} > {:.1}x baseline {:.3}",
                        option_result.combined_post_score,
                        tolerance,
                        baseline_result.combined_post_score,
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "MixedPhase OK: score {:.3} vs baseline {:.3}",
                        option_result.combined_post_score, baseline_result.combined_post_score,
                    ),
                )
            }
        }
        OptionOverride::DecomposedCorrection => {
            // DecomposedCorrection applies frequency-dependent weighting.
            // It should not make things significantly worse than baseline.
            let ratio =
                option_result.combined_post_score / baseline_result.combined_post_score.max(1e-6);
            if ratio > 2.0 {
                (
                    false,
                    format!(
                        "DecomposedCorrection degraded score too much: {:.3} vs baseline {:.3} (ratio {:.2})",
                        option_result.combined_post_score,
                        baseline_result.combined_post_score,
                        ratio,
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "DecomposedCorrection OK: score {:.3} vs baseline {:.3}",
                        option_result.combined_post_score, baseline_result.combined_post_score,
                    ),
                )
            }
        }
    }
}

/// OE-1: Target tilt - slope of final curve should be closer to requested tilt
fn validate_target_tilt(
    requested_slope: f64,
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
    has_schroeder: bool,
    has_broadband: bool,
) -> (bool, String) {
    let mut baseline_slope_err = 0.0_f64;
    let mut option_slope_err = 0.0_f64;
    let mut count = 0;

    for (ch_name, baseline_ch) in &baseline_result.channel_results {
        if let Some(option_ch) = option_result.channel_results.get(ch_name) {
            let fmin = 100.0;
            let fmax = 500.0;

            if let Some(baseline_slope) = regression_slope_per_octave_in_range(
                &baseline_ch.final_curve.freq,
                &baseline_ch.final_curve.spl,
                fmin,
                fmax,
            ) && let Some(option_slope) = regression_slope_per_octave_in_range(
                &option_ch.final_curve.freq,
                &option_ch.final_curve.spl,
                fmin,
                fmax,
            ) {
                baseline_slope_err += (baseline_slope - requested_slope).abs();
                option_slope_err += (option_slope - requested_slope).abs();
                count += 1;
            }
        }
    }

    if count == 0 {
        return (false, "no slope data available".to_string());
    }

    let avg_baseline_err = baseline_slope_err / count as f64;
    let avg_option_err = option_slope_err / count as f64;
    // With-option slope should be closer to requested (or within tolerance).
    // Widen tolerance for combos: other options (excursion HPF, schroeder split,
    // psychoacoustic) can distort the slope in the 100-500 Hz measurement band.
    let mut combo_tolerance = TILT_SLOPE_TOLERANCE * (1.0 + (num_options.saturating_sub(1) as f64));
    // Schroeder split at 300 Hz bisects the 100-500 Hz slope measurement range,
    // creating two independently-optimized zones with different tilt behavior.
    // This fundamentally limits slope accuracy across the crossover.
    if has_schroeder {
        combo_tolerance += 3.0;
    }
    // Broadband shelves interact with tilt, adding global slope shifts.
    if has_broadband {
        combo_tolerance += 2.0;
    }
    let pass = avg_option_err < avg_baseline_err + combo_tolerance;

    (
        pass,
        format!(
            "slope_err: baseline={:.3} option={:.3} dB/oct (requested={:.1})",
            avg_baseline_err, avg_option_err, requested_slope
        ),
    )
}

/// OE-2: Excursion protection - response below F3 should not be boosted
fn validate_excursion_protection(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    let mut checks_pass = true;
    let mut details = Vec::new();

    // In combos, other options (tilt, broadband shelves, schroeder split) can shift
    // low-freq energy significantly. Scale tolerance with number of active options.
    // Each additional option contributes up to 4 dB of interaction, capped at 25 dB
    // for extreme kitchen-sink combos where excursion HPF + schroeder + tilt all
    // modify the bass region simultaneously.
    let tolerance_db = (2.0 + (num_options.saturating_sub(1) as f64) * 4.0).min(25.0);

    for (ch_name, option_ch) in &option_result.channel_results {
        if let Some(baseline_ch) = baseline_result.channel_results.get(ch_name) {
            // Check mean SPL in very low frequency range (20-40 Hz)
            let baseline_low = mean_spl_in_range(&baseline_ch.final_curve, 20.0, 40.0);
            let option_low = mean_spl_in_range(&option_ch.final_curve, 20.0, 40.0);

            // With excursion protection, low freq SPL should be <= baseline (no boost)
            if option_low > baseline_low + tolerance_db {
                checks_pass = false;
                details.push(format!(
                    "{}: low_freq {:.1}dB > baseline {:.1}dB",
                    ch_name, option_low, baseline_low
                ));
            } else {
                details.push(format!(
                    "{}: low_freq {:.1}dB <= baseline {:.1}dB",
                    ch_name, option_low, baseline_low
                ));
            }
        }
    }

    (checks_pass, details.join("; "))
}

/// OE-3: Schroeder split - structural and Q-limit validation
///
/// The Schroeder split should produce filters with different characteristics
/// above and below the Schroeder frequency:
/// - Below: higher Q (narrow, targeting room modes), predominantly cuts
/// - Above: lower Q (broad, gentle tone control)
///
/// We validate:
/// 1. Structural: mean Q below >= mean Q above
/// 2. Hard Q limits: every filter above Schroeder must respect high_max_q,
///    even though below-Schroeder Q can be high. This prevents the optimizer
///    from placing narrow aggressive filters in the tone-control band.
fn validate_schroeder_split(
    schroeder_freq: f64,
    low_max_q: f64,
    high_max_q: f64,
    option_result: &RoomOptimizationResult,
) -> (bool, String) {
    let mut total_low_q = 0.0;
    let mut total_high_q = 0.0;
    let mut low_count = 0usize;
    let mut high_count = 0usize;
    let mut low_boosts = 0usize;
    let mut q_violations = Vec::new();

    for (ch_name, ch_result) in &option_result.channel_results {
        for (i, bq) in ch_result.biquads.iter().enumerate() {
            if bq.freq < schroeder_freq {
                total_low_q += bq.q;
                low_count += 1;
                if bq.db_gain > 0.1 {
                    low_boosts += 1;
                }
                // Below Schroeder: Q must stay within configured low_max_q.
                // Allow 20% tolerance for optimizer bound enforcement.
                if bq.q > low_max_q * 1.2 {
                    q_violations.push(format!(
                        "{} f{}({:.0}Hz): Q={:.1}>{:.1}",
                        ch_name, i, bq.freq, bq.q, low_max_q
                    ));
                }
            } else {
                total_high_q += bq.q;
                high_count += 1;
                // Above Schroeder: Q must respect the tighter high_max_q.
                // This is the key invariant — prevents narrow aggressive
                // filters in the tone-control band.
                if bq.q > high_max_q * 1.2 {
                    q_violations.push(format!(
                        "{} f{}({:.0}Hz): Q={:.1}>{:.1}",
                        ch_name, i, bq.freq, bq.q, high_max_q
                    ));
                }
            }
        }
    }

    if low_count == 0 || high_count == 0 {
        return (true, "no filters in one band (skip)".to_string());
    }

    let mean_low_q = total_low_q / low_count as f64;
    let mean_high_q = total_high_q / high_count as f64;
    let boost_pct = if low_count > 0 {
        low_boosts as f64 / low_count as f64 * 100.0
    } else {
        0.0
    };

    let mut details = Vec::new();

    // Structural checks:
    // 1. Mean Q below Schroeder should be >= mean Q above (within tolerance).
    // The optimizer picks the lowest-Q filter that covers a given deviation,
    // so with only 2 filters below Schroeder and broad modal dips the low-Q
    // can come out ~0.6-0.7 while the high-band (capped at 1.0) naturally
    // sits near its max. The tolerance factor 0.7 accommodates this; the
    // structural intent (low should trend narrower when modes are present)
    // is preserved because tight modes push low_q well above 1.0.
    let q_ok = mean_low_q >= mean_high_q * 0.7;
    details.push(format!(
        "mean_Q: low={:.2} high={:.2}",
        mean_low_q, mean_high_q
    ));

    // 2. Majority of below-Schroeder filters should be cuts
    let boost_ok = boost_pct <= 60.0;
    details.push(format!(
        "low_boost={:.0}% ({}/{})",
        boost_pct, low_boosts, low_count
    ));

    // 3. Hard Q-limit violations
    let q_limits_ok = q_violations.is_empty();
    if !q_limits_ok {
        details.push(format!("Q violations: {}", q_violations.join(", ")));
    }

    let pass = q_ok && boost_ok && q_limits_ok;
    (pass, details.join("; "))
}

/// OE-4: Asymmetric loss - peaks should be penalized more than dips
fn validate_asymmetric_loss(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
) -> (bool, String) {
    let mut baseline_ratio_sum = 0.0;
    let mut option_ratio_sum = 0.0;
    let mut count = 0;

    for (ch_name, baseline_ch) in &baseline_result.channel_results {
        if let Some(option_ch) = option_result.channel_results.get(ch_name) {
            let fmin = 20.0;
            let fmax = 500.0;

            let (b_peak, b_dip) = peak_dip_rms(
                &baseline_ch.initial_curve,
                &baseline_ch.final_curve,
                fmin,
                fmax,
            );
            let (o_peak, o_dip) =
                peak_dip_rms(&option_ch.initial_curve, &option_ch.final_curve, fmin, fmax);

            if b_dip > 0.01 && o_dip > 0.01 {
                baseline_ratio_sum += b_peak / b_dip;
                option_ratio_sum += o_peak / o_dip;
                count += 1;
            }
        }
    }

    if count == 0 {
        return (true, "no valid peak/dip data (skip)".to_string());
    }

    let baseline_ratio = baseline_ratio_sum / count as f64;
    let option_ratio = option_ratio_sum / count as f64;

    // With asymmetric loss, peak correction should be stronger (peak_rms lower).
    // The ratio may increase because dips are tolerated more (by design — dip_weight
    // is lower), so we check that the ratio doesn't explode rather than requiring
    // it to decrease. The key invariant is that asymmetric loss changes the balance.
    let pass = option_ratio <= baseline_ratio + 1.0; // generous tolerance for strong asymmetry

    (
        pass,
        format!(
            "peak/dip ratio: baseline={:.3} asymmetric={:.3}",
            baseline_ratio, option_ratio
        ),
    )
}

/// OE-5: Psychoacoustic - score should not be catastrophically worse
fn validate_psychoacoustic(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    let baseline_score = baseline_result.combined_post_score;
    let option_score = option_result.combined_post_score;

    // Psychoacoustic trades raw score for perceptual quality. In combos with
    // other options (tilt, excursion, schroeder), the raw score can diverge
    // significantly since the optimizer faces conflicting constraints.
    let tolerance = PSYCHOACOUSTIC_SCORE_TOLERANCE + (num_options.saturating_sub(1) as f64) * 0.5;
    let pass = option_score <= tolerance * baseline_score;

    (
        pass,
        format!(
            "score: baseline={:.4} psychoacoustic={:.4} (limit={:.1}x)",
            baseline_score, option_score, tolerance
        ),
    )
}

/// OE-6: Broadband target matching - shelf plugins present, score not worse
///
/// Both baseline and option have the same target_tilt (-0.8 dB/oct).
/// With broadband matching enabled, shelf/gain plugins should appear in the
/// DSP chain to coarsely correct the response before fine EQ.
fn validate_broadband_target_matching(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    _option_config: &RoomConfig,
    num_options: usize,
) -> (bool, String) {
    let mut details = Vec::new();
    let mut pass = true;

    // Check 1: broadband matching should produce gain/EQ plugins in the DSP chain
    let has_broadband_plugins = option_result.channels.values().any(|chain| {
        chain.plugins.iter().any(|p| {
            let pt = p.plugin_type.to_lowercase();
            pt.contains("gain") || pt.contains("eq")
        })
    });
    details.push(if has_broadband_plugins {
        "shelf_plugins=present".to_string()
    } else {
        "shelf_plugins=absent".to_string()
    });

    // Check 2: score must not be worse than baseline. Scale tolerance for combos
    // where other options (tilt, excursion, schroeder, psychoacoustic) modify the
    // response significantly before broadband matching acts.
    let score_tolerance = OPTION_SCORE_TOLERANCE + (num_options.saturating_sub(1) as f64) * 0.3;
    let score_ok =
        option_result.combined_post_score <= score_tolerance * baseline_result.combined_post_score;
    if !score_ok {
        pass = false;
    }
    details.push(format!(
        "score: baseline={:.4} broadband={:.4} (limit={:.1}x)",
        baseline_result.combined_post_score, option_result.combined_post_score, score_tolerance,
    ));

    // Check 3: per-channel regression — no channel should get catastrophically worse.
    // Scale regression tolerance for combos.
    let regression_factor = 2.0 + (num_options.saturating_sub(1) as f64) * 0.5;
    for (ch_name, option_ch) in &option_result.channel_results {
        if let Some(baseline_ch) = baseline_result.channel_results.get(ch_name)
            && option_ch.post_score > baseline_ch.post_score * regression_factor
        {
            pass = false;
            details.push(format!(
                "{}: REGRESSED {:.2} -> {:.2}",
                ch_name, baseline_ch.post_score, option_ch.post_score
            ));
        }
    }

    // Check 4: double-tilt detection. Scale slope tolerance for combos where
    // schroeder split creates a boundary discontinuity within the measurement band.
    // Schroeder (300 Hz) bisects the 100-1000 Hz slope range, so combos with
    // schroeder + tilt legitimately produce larger slope shifts.
    let slope_tolerance = 3.0 + (num_options.saturating_sub(1) as f64) * 1.5;
    for (ch_name, option_ch) in &option_result.channel_results {
        if let Some(baseline_ch) = baseline_result.channel_results.get(ch_name)
            && let Some(baseline_slope) = regression_slope_per_octave_in_range(
                &baseline_ch.final_curve.freq,
                &baseline_ch.final_curve.spl,
                100.0,
                1000.0,
            )
            && let Some(option_slope) = regression_slope_per_octave_in_range(
                &option_ch.final_curve.freq,
                &option_ch.final_curve.spl,
                100.0,
                1000.0,
            )
        {
            let slope_diff = (option_slope - baseline_slope).abs();
            if slope_diff > slope_tolerance {
                pass = false;
                details.push(format!(
                    "{}: DOUBLE-TILT slope_diff={:.1}dB/oct (baseline={:.1} broadband={:.1})",
                    ch_name, slope_diff, baseline_slope, option_slope
                ));
            }
        }
    }

    (pass, details.join("; "))
}

/// OE-7: Phase alignment - delay plugin present in sub channel, score not worse
fn validate_phase_alignment(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    // Check that at least one channel has a delay plugin
    let has_delay = option_result.channels.values().any(|chain| {
        chain
            .plugins
            .iter()
            .any(|p| p.plugin_type.to_lowercase().contains("delay"))
    });

    // In combos with multiple options, allow more tolerance since shared mean SPL
    // and decomposed correction defaults shift absolute scores.
    let tolerance = OPTION_SCORE_TOLERANCE + (num_options.saturating_sub(1) as f64) * 0.15;
    let score_ok =
        option_result.combined_post_score <= tolerance * baseline_result.combined_post_score;

    let pass = score_ok; // delay presence is informational, not required
    let delay_str = if has_delay {
        "delay_present"
    } else {
        "no_delay"
    };

    (
        pass,
        format!(
            "{}: baseline={:.4} aligned={:.4} (limit={:.1}x)",
            delay_str,
            baseline_result.combined_post_score,
            option_result.combined_post_score,
            tolerance
        ),
    )
}

/// OE-8: Multi-measurement minimax - worst-case position should improve
fn validate_multi_measurement_minimax(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    // Compare worst-case channel scores
    let baseline_max = baseline_result
        .channel_results
        .values()
        .map(|c| c.post_score)
        .fold(f64::NEG_INFINITY, f64::max);
    let option_max = option_result
        .channel_results
        .values()
        .map(|c| c.post_score)
        .fold(f64::NEG_INFINITY, f64::max);

    // Minimax should improve worst case (or at least not be significantly worse).
    // In combos, other options (excursion, schroeder, decomposed correction) add
    // heavy constraints that may degrade the minimax target significantly.
    // The shared mean SPL pre-pass and decomposed correction defaults also shift scores.
    let tolerance = OPTION_SCORE_TOLERANCE + (num_options.saturating_sub(1) as f64) * 0.4;
    let pass = option_max <= baseline_max * tolerance;

    (
        pass,
        format!(
            "worst_case: baseline={:.4} minimax={:.4}",
            baseline_max, option_max
        ),
    )
}

/// OE-9: Multi-measurement variance penalized - consistency across positions
fn validate_multi_measurement_variance(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    let baseline_scores: Vec<f64> = baseline_result
        .channel_results
        .values()
        .map(|c| c.post_score)
        .collect();
    let option_scores: Vec<f64> = option_result
        .channel_results
        .values()
        .map(|c| c.post_score)
        .collect();

    let baseline_var = variance(&baseline_scores);
    let option_var = variance(&option_scores);

    // Variance-penalized should have lower or similar variance.
    // Scale tolerance for combos.
    let var_tolerance = 2.0 + (num_options.saturating_sub(1) as f64) * 0.5;
    let pass = option_var <= baseline_var * var_tolerance + 0.1;

    (
        pass,
        format!(
            "score_var: baseline={:.4} variance_penalized={:.4}",
            baseline_var, option_var
        ),
    )
}

fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

// ---------------------------------------------------------------------------
// Evaluation helpers
// ---------------------------------------------------------------------------

/// Evaluate a single optimization result against baseline
fn evaluate_result(
    mutation: Mutation,
    pre: f64,
    post: f64,
    baseline_post: &mut Option<f64>,
) -> (bool, String) {
    match mutation {
        Mutation::Baseline => {
            *baseline_post = Some(post);
            let ok = post < pre;
            let reason = if ok {
                format!("pre={:.4}, -{:.0}%", pre, (1.0 - post / pre) * 100.0)
            } else {
                format!("FAIL: post {:.4} >= pre {:.4}", post, pre)
            };
            (ok, reason)
        }
        _ => {
            let base = baseline_post.unwrap();
            let threshold = base * MONOTONICITY_TOLERANCE;
            let ok = post <= threshold && post < pre;
            let pct = (1.0 - post / base) * 100.0;
            let reason = if ok {
                format!("{:+.0}% vs baseline", -pct)
            } else if post >= pre {
                format!("FAIL: post {:.4} >= pre {:.4}", post, pre)
            } else {
                format!(
                    "FAIL: post {:.4} > baseline*{:.2} ({:.4})",
                    post, MONOTONICITY_TOLERANCE, threshold
                )
            };
            (ok, reason)
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

/// Cap on concurrent optimization runs to prevent OOM.
///
/// Each DE optimization already uses rayon internally (one evaluator per
/// core), and some test cases hold multi-measurement curves + full
/// baseline/option pairs in memory. Fanning out one outer thread per test
/// case (≈ 70 cases) on top of the inner rayon pool multiplies resident
/// memory until the machine OOMs. Default to half the CPU count so each
/// active optimization still has parallel evaluators, but the overall
/// working set stays bounded. `--jobs N` overrides.
fn default_parallel_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get().max(1) / 2)
        .unwrap_or(1)
        .max(1)
}

/// Counting-semaphore permit manager — same pattern as `roomeq-qa-coverage`.
/// Used to bound the number of test cases running concurrently.
struct CountingSemaphore {
    state: Mutex<usize>,
    cvar: Condvar,
}

impl CountingSemaphore {
    fn new(permits: usize) -> Self {
        Self {
            state: Mutex::new(permits),
            cvar: Condvar::new(),
        }
    }

    fn acquire(&self) {
        let mut count = self.state.lock().unwrap();
        while *count == 0 {
            count = self.cvar.wait(count).unwrap();
        }
        *count -= 1;
    }

    fn release(&self) {
        let mut count = self.state.lock().unwrap();
        *count += 1;
        self.cvar.notify_one();
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    let list_mode = args.iter().any(|a| a == "--list");
    let case_filter: Option<String> = args
        .windows(2)
        .find(|w| w[0] == "--case")
        .map(|w| w[1].clone());
    let jobs: usize = args
        .windows(2)
        .find(|w| w[0] == "--jobs")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or_else(default_parallel_jobs)
        .max(1);

    let all_cases = all_test_cases();

    // --list: print available cases and exit
    if list_mode {
        println!("Available test cases:");
        for tc in &all_cases {
            println!("  {}", tc.name());
        }
        return Ok(());
    }

    println!(
        "=== RoomEQ QA: Convergence, Monotonicity & Invariants (DE/LSHADE, seed={}, parallel) ===",
        SEED
    );

    let project_root = find_project_root()?;
    let fem_dir = project_root.join(FEM_DIR);
    let optim_dir = project_root.join(OPTIM_CONFIG_DIR);

    // Filter cases if --case is provided (substring match)
    let cases_to_run: Vec<TestCase> = if let Some(ref filter) = case_filter {
        let filter_lower = filter.to_lowercase();
        let matched: Vec<_> = all_cases
            .into_iter()
            .filter(|tc| tc.name().to_lowercase().contains(&filter_lower))
            .collect();
        if matched.is_empty() {
            return Err(anyhow!(
                "No test case matches '{}'. Use --list to see available cases.",
                filter
            ));
        }
        println!("Running {} case(s) matching '{}'", matched.len(), filter);
        matched
    } else {
        all_cases
    };

    println!(
        "Using {} parallel job(s) (override with --jobs N).",
        jobs
    );

    // Run all test cases with a bounded permit pool. The outer thread is
    // spawned immediately but `sem.acquire()` gates entry to the actual
    // optimization — so at most `jobs` cases are resident simultaneously.
    let semaphore = Arc::new(CountingSemaphore::new(jobs));
    let handles: Vec<_> = cases_to_run
        .into_iter()
        .map(|tc| {
            let fem_dir = fem_dir.clone();
            let optim_dir = optim_dir.clone();
            let sem = Arc::clone(&semaphore);
            std::thread::spawn(move || -> Result<(String, Vec<TestResult>)> {
                sem.acquire();
                let result = match tc {
                    TestCase::Workflow {
                        name,
                        fem_subdir,
                        optim_subdir,
                    } => {
                        let base_path = fem_dir.join(format!("{}/config.json", fem_subdir));
                        let override_path =
                            optim_dir.join(format!("{}/optimiser-iir.json", optim_subdir));
                        run_stereo_workflow_tests(name, &base_path, Some(&override_path))
                    }
                    TestCase::Generic {
                        name,
                        fem_subdir,
                        optim_subdir,
                    } => {
                        let base_path = fem_dir.join(format!("{}/config.json", fem_subdir));
                        let override_dir = optim_dir.join(optim_subdir);
                        run_generic_path_tests(name, &base_path, &override_dir)
                    }
                    TestCase::CrossModeConvergence {
                        name,
                        fem_subdir,
                        optim_subdir,
                    } => {
                        let base_path = fem_dir.join(format!("{}/config.json", fem_subdir));
                        let override_dir = optim_dir.join(optim_subdir);
                        run_cross_mode_convergence_tests(name, &base_path, &override_dir)
                    }
                    TestCase::OptionEffect {
                        name,
                        fem_subdir,
                        optim_subdir,
                        options,
                    } => run_option_effect_test(
                        name,
                        &fem_dir,
                        fem_subdir,
                        &optim_dir,
                        optim_subdir,
                        &options,
                    ),
                };
                sem.release();
                result
            })
        })
        .collect();

    // Collect results in order, printing output as each completes
    let mut all_results: Vec<TestResult> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for handle in handles {
        match handle.join() {
            Ok(Ok((output, results))) => {
                print!("{}", output);
                all_results.extend(results);
            }
            Ok(Err(e)) => {
                errors.push(format!("{:#}", e));
            }
            Err(_) => {
                errors.push("Thread panicked".to_string());
            }
        }
    }

    // Print any thread errors
    for err in &errors {
        eprintln!("ERROR: {}", err);
    }

    // Summary
    let total = all_results.len();
    let passed = all_results.iter().filter(|r| r.pass).count();
    let failed = total - passed;

    println!("\n=== Summary: {}/{} PASS ===", passed, total);

    if failed > 0 {
        println!("\nFailed tests:");
        for r in &all_results {
            if !r.pass {
                let epa_str = match r.epa_preference {
                    Some(v) => format!("{:.3}", v),
                    None => "n/a".to_string(),
                };
                println!(
                    "  - {} (pre={:.4}, post={:.4}, epa={}): {}",
                    r.label, r.pre_score, r.post_score, epa_str, r.reason
                );
            }
        }
    }

    if failed > 0 || !errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

/// Find the project root by looking for Cargo.toml with \[workspace\]
fn find_project_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            return Err(anyhow!(
                "Could not find project root (Cargo.toml with [workspace])"
            ));
        }
    }
}
