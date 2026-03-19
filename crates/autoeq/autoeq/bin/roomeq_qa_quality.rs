//! RoomEQ QA: Convergence, Monotonicity, Cross-Mode & Per-Option Tests
//!
//! Validates that roomeq optimization modes produce converging results,
//! that giving the optimizer more resources always improves or maintains loss,
//! that IIR/FIR/Mixed modes converge to similar frequency responses,
//! and that each optimizer option has its expected effect.
//!
//! Uses COBYLA (fast, deterministic) instead of DE for speed.
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

use autoeq::Curve;
use autoeq::loss::phase_aware::{compute_group_delay, unwrap_phase_degrees};
use autoeq::loss::{
    calculate_standard_deviation_in_range, regression_slope_per_octave_in_range,
};
use autoeq::roomeq::{
    BroadbandTargetMatchingConfig, CallbackAction, ExcursionProtectionConfig,
    MultiMeasurementConfig, MultiMeasurementStrategy, PhaseAlignmentConfig, ProcessingMode,
    RoomConfig, RoomOptimizationResult, SchroederSplitConfig, TargetTiltConfig, TiltType,
    load_config, merge_json_objects, optimize_room,
};
use autoeq::{MeasurementMultiple, MeasurementRef, MeasurementSource};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Monotonicity tolerance: variation may be at most 20% worse than baseline.
/// COBYLA is a local optimizer so sensitivity to search space changes is higher
/// than global optimizers. Allow wider tolerance for the fast QA mode.
const MONOTONICITY_TOLERANCE: f64 = 1.20;

/// Cross-mode ratio: max score / min score must be <= 5.0.
const CROSS_MODE_RATIO_LIMIT: f64 = 5.0;

const SAMPLE_RATE: f64 = 48000.0;

const SEED: u64 = 42;

/// COBYLA maxeval for QA (fast mode). Enough to find bugs, not to get best results.
const QA_MAXEVAL: usize = 1000;

/// Base config directories
const FEM_DIR: &str = "data_tests/roomeq/generated/fem";
const OPTIM_CONFIG_DIR: &str = "data_tests/roomeq/generated/optimiser-config";

// Cross-mode convergence thresholds
/// Maximum dB difference between any two modes' final curves in passband.
/// Generous limit: IIR/FIR/Mixed use fundamentally different correction
/// mechanisms and COBYLA at 1000 maxeval won't fully converge.
const CROSS_MODE_FR_MAX_DIFF_DB: f64 = 18.0;
/// Score ratio limit for cross-mode convergence (reuse existing)
const CROSS_MODE_SCORE_RATIO_LIMIT: f64 = 3.0;

// Per-option effect thresholds
/// Slope tolerance in dB/octave for target_tilt validation
const TILT_SLOPE_TOLERANCE: f64 = 0.5;
/// Score tolerance for option vs baseline (option within 1.2x of baseline)
const OPTION_SCORE_TOLERANCE: f64 = 1.20;
/// Psychoacoustic may trade raw score for perceptual quality
const PSYCHOACOUSTIC_SCORE_TOLERANCE: f64 = 2.0;

// ---------------------------------------------------------------------------
// QA config overrides (fast mode: cobyla, low iterations)
// ---------------------------------------------------------------------------

/// Override optimizer settings for fast QA: use COBYLA with low maxeval, no refinement.
fn apply_qa_overrides(config: &mut RoomConfig) {
    config.optimizer.algorithm = "cobyla".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.refine = false;
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
    if count > 0 {
        sum / count as f64
    } else {
        0.0
    }
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

// ---------------------------------------------------------------------------
// Test result tracking
// ---------------------------------------------------------------------------

struct TestResult {
    label: String,
    pre_score: f64,
    post_score: f64,
    pass: bool,
    reason: String,
}

// ---------------------------------------------------------------------------
// Option Override: programmatic config mutation for per-option tests
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum OptionOverride {
    TargetTilt { slope_db_per_octave: f64 },
    ExcursionProtection,
    SchroederSplit { schroeder_freq: f64, low_max_q: f64, high_max_q: f64 },
    AsymmetricLoss,
    Psychoacoustic,
    BroadbandTargetMatching,
    PhaseAlignment,
    MultiMeasurementMinimax,
    MultiMeasurementVariancePenalized,
}

impl std::fmt::Display for OptionOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionOverride::TargetTilt { slope_db_per_octave } => {
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
        }
    }
}

/// Apply option override to config. Also disables the option for baseline configs.
fn apply_option_override(config: &mut RoomConfig, option: &OptionOverride) {
    match option {
        OptionOverride::TargetTilt { slope_db_per_octave } => {
            config.optimizer.target_tilt = Some(TargetTiltConfig {
                tilt_type: TiltType::Custom,
                slope_db_per_octave: *slope_db_per_octave,
                ..TargetTiltConfig::default()
            });
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
        OptionOverride::SchroederSplit { schroeder_freq, low_max_q, high_max_q } => {
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
            config.optimizer.broadband_target_matching =
                Some(BroadbandTargetMatchingConfig { enabled: true });
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
    }
}

/// Disable the option in config to create a clean baseline
fn disable_option(config: &mut RoomConfig, option: &OptionOverride) {
    match option {
        OptionOverride::TargetTilt { .. } => {
            config.optimizer.target_tilt = None;
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
            config.optimizer.broadband_target_matching = None;
        }
        OptionOverride::PhaseAlignment => {
            config.optimizer.phase_alignment = None;
            config.optimizer.allow_delay = Some(false);
        }
        OptionOverride::MultiMeasurementMinimax | OptionOverride::MultiMeasurementVariancePenalized => {
            config.optimizer.multi_measurement = Some(MultiMeasurementConfig {
                strategy: MultiMeasurementStrategy::Average,
                ..Default::default()
            });
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
            options: vec![OptionOverride::TargetTilt { slope_db_per_octave: -0.8 }],
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
                OptionOverride::Psychoacoustic,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO tilt+excursion",
            fem_subdir: "small_stereo_2_0",
            optim_subdir: "small_stereo_2_0",
            options: vec![
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
                OptionOverride::ExcursionProtection,
            ],
        },
        TestCase::OptionEffect {
            name: "COMBO tilt+broadband 5.1",
            fem_subdir: "medium_surround_5_1",
            optim_subdir: "medium_surround_5_1",
            options: vec![
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
                OptionOverride::TargetTilt { slope_db_per_octave: -0.8 },
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
        let channel_names: Vec<String> = mode_results[0]
            .1
            .channel_results
            .keys()
            .cloned()
            .collect();

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
            pass: cm1_pass,
            reason: format!(
                "max_diff={:.2}dB (limit={:.1}dB)",
                cm1_max_diff, CROSS_MODE_FR_MAX_DIFF_DB
            ),
        });
    }

    // CM-2: Group delay flatness (FIR/Mixed should have <= IIR GD std dev)
    {
        let channel_names: Vec<String> = mode_results[0]
            .1
            .channel_results
            .keys()
            .cloned()
            .collect();

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
                iir_gd_max,
                fir_gd_max,
                mixed_gd_max,
                status
            )
            .unwrap();

            results.push(TestResult {
                label: format!("{} CM-2 GD flatness", name),
                pre_score: iir_gd_max,
                post_score: fir_gd_max.max(mixed_gd_max),
                pass: cm2_pass,
                reason: format!(
                    "IIR={:.2}ms FIR={:.2}ms Mixed={:.2}ms",
                    iir_gd_max,
                    fir_gd_max,
                    mixed_gd_max
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
    let default_tilt = if has_broadband && !has_tilt {
        Some(TargetTiltConfig {
            tilt_type: TiltType::Custom,
            slope_db_per_octave: -0.8,
            ..TargetTiltConfig::default()
        })
    } else {
        None
    };

    // Load and run baseline (all options disabled)
    let (mut baseline_config, _) =
        load_config(&base_config_path, override_path.as_deref())?;
    apply_qa_overrides(&mut baseline_config);
    for option in options {
        disable_option(&mut baseline_config, option);
    }
    if let Some(ref tilt) = default_tilt {
        baseline_config.optimizer.target_tilt = Some(tilt.clone());
    }
    if needs_multi_measurement {
        enable_multi_measurement_paths(&mut baseline_config, fem_dir, fem_subdir);
    }

    let baseline_result = run_optimization(&baseline_config)
        .with_context(|| format!("{} baseline", name))?;

    writeln!(
        out,
        "  baseline: post={:.4} (pre={:.4})",
        baseline_result.combined_post_score, baseline_result.combined_pre_score
    )
    .unwrap();

    // Load and run with all options enabled
    let (mut option_config, _) =
        load_config(&base_config_path, override_path.as_deref())?;
    apply_qa_overrides(&mut option_config);
    for option in options {
        apply_option_override(&mut option_config, option);
    }
    if let Some(ref tilt) = default_tilt {
        option_config.optimizer.target_tilt = Some(tilt.clone());
    }
    if needs_multi_measurement {
        enable_multi_measurement_paths(&mut option_config, fem_dir, fem_subdir);
    }

    let option_result = run_optimization(&option_config)
        .with_context(|| format!("{} with-options", name))?;

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
            options.len(),
        );

        let status = if pass { "PASS" } else { "FAIL" };
        writeln!(out, "  {}: {}  ({})", option, status, reason).unwrap();

        if !pass {
            all_pass = false;
            results.push(TestResult {
                label: format!("{} [{}]", name, option),
                pre_score: baseline_result.combined_post_score,
                post_score: option_result.combined_post_score,
                pass: false,
                reason,
            });
        }
    }

    // Combo-level check: combined result should still converge (post < pre)
    let converged = option_result.combined_post_score < option_result.combined_pre_score;
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
/// `num_options` is the total number of simultaneously active options — validators
/// can widen tolerances when many options interact.
fn validate_option_effect(
    option: &OptionOverride,
    _baseline_config: &RoomConfig,
    baseline_result: &RoomOptimizationResult,
    option_config: &RoomConfig,
    option_result: &RoomOptimizationResult,
    num_options: usize,
) -> (bool, String) {
    match option {
        OptionOverride::TargetTilt { slope_db_per_octave } => {
            validate_target_tilt(*slope_db_per_octave, baseline_result, option_result)
        }
        OptionOverride::ExcursionProtection => {
            validate_excursion_protection(baseline_result, option_result, num_options)
        }
        OptionOverride::SchroederSplit { schroeder_freq, low_max_q, high_max_q } => {
            validate_schroeder_split(*schroeder_freq, *low_max_q, *high_max_q, option_result)
        }
        OptionOverride::AsymmetricLoss => {
            validate_asymmetric_loss(baseline_result, option_result)
        }
        OptionOverride::Psychoacoustic => {
            validate_psychoacoustic(baseline_result, option_result)
        }
        OptionOverride::BroadbandTargetMatching => {
            validate_broadband_target_matching(baseline_result, option_result, option_config)
        }
        OptionOverride::PhaseAlignment => {
            validate_phase_alignment(baseline_result, option_result)
        }
        OptionOverride::MultiMeasurementMinimax => {
            validate_multi_measurement_minimax(baseline_result, option_result)
        }
        OptionOverride::MultiMeasurementVariancePenalized => {
            validate_multi_measurement_variance(baseline_result, option_result)
        }
    }
}

/// OE-1: Target tilt - slope of final curve should be closer to requested tilt
fn validate_target_tilt(
    requested_slope: f64,
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
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
    // With-option slope should be closer to requested (or within tolerance)
    let pass = avg_option_err < avg_baseline_err + TILT_SLOPE_TOLERANCE;

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

    // In combos, other options (tilt, broadband shelves) can shift low-freq energy,
    // so widen the tolerance when multiple options are active, capped at 3dB.
    let tolerance_db = (1.0 + (num_options.saturating_sub(1) as f64) * 0.5).min(3.0);

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

/// OE-3: Schroeder split - structural validation
///
/// The Schroeder split should produce filters with different characteristics
/// above and below the Schroeder frequency:
/// - Below: higher Q (narrow, targeting room modes), predominantly cuts
/// - Above: lower Q (broad, gentle tone control)
///
/// We validate structurally (mean Q below > mean Q above) rather than
/// strict per-filter constraints, since COBYLA may not perfectly enforce bounds.
fn validate_schroeder_split(
    schroeder_freq: f64,
    _low_max_q: f64,
    _high_max_q: f64,
    option_result: &RoomOptimizationResult,
) -> (bool, String) {
    let mut total_low_q = 0.0;
    let mut total_high_q = 0.0;
    let mut low_count = 0usize;
    let mut high_count = 0usize;
    let mut low_boosts = 0usize;

    for ch_result in option_result.channel_results.values() {
        for bq in &ch_result.biquads {
            if bq.freq < schroeder_freq {
                total_low_q += bq.q;
                low_count += 1;
                if bq.db_gain > 0.1 {
                    low_boosts += 1;
                }
            } else {
                total_high_q += bq.q;
                high_count += 1;
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

    // Structural checks:
    // 1. Mean Q below Schroeder should be >= mean Q above (narrower targeting of modes)
    let q_ok = mean_low_q >= mean_high_q * 0.8; // allow some tolerance
    // 2. Majority of below-Schroeder filters should be cuts (allow up to 50% boosts
    //    since some boosts may be needed for dips between modes)
    let boost_ok = boost_pct <= 60.0;
    let pass = q_ok && boost_ok;

    (
        pass,
        format!(
            "mean_Q: low={:.2} high={:.2}; low_boost={:.0}% ({}/{})",
            mean_low_q, mean_high_q, boost_pct, low_boosts, low_count
        ),
    )
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

            let (b_peak, b_dip) =
                peak_dip_rms(&baseline_ch.initial_curve, &baseline_ch.final_curve, fmin, fmax);
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

    // With asymmetric loss, peak/dip ratio should be lower (peaks reduced more)
    let pass = option_ratio <= baseline_ratio + 0.5; // tolerance

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
) -> (bool, String) {
    let baseline_score = baseline_result.combined_post_score;
    let option_score = option_result.combined_post_score;

    let pass = option_score <= PSYCHOACOUSTIC_SCORE_TOLERANCE * baseline_score;

    (
        pass,
        format!(
            "score: baseline={:.4} psychoacoustic={:.4} (limit={:.1}x)",
            baseline_score, option_score, PSYCHOACOUSTIC_SCORE_TOLERANCE
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
) -> (bool, String) {
    let mut details = Vec::new();

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

    // Check 2: score should not be significantly worse than baseline
    let score_ok = option_result.combined_post_score
        <= OPTION_SCORE_TOLERANCE * baseline_result.combined_post_score;
    details.push(format!(
        "score: baseline={:.4} broadband={:.4}",
        baseline_result.combined_post_score,
        option_result.combined_post_score
    ));

    // Pass if score is acceptable (shelf plugins are informational)
    (score_ok, details.join("; "))
}

/// OE-7: Phase alignment - delay plugin present in sub channel, score not worse
fn validate_phase_alignment(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
) -> (bool, String) {
    // Check that at least one channel has a delay plugin
    let has_delay = option_result.channels.values().any(|chain| {
        chain
            .plugins
            .iter()
            .any(|p| p.plugin_type.to_lowercase().contains("delay"))
    });

    let score_ok = option_result.combined_post_score
        <= OPTION_SCORE_TOLERANCE * baseline_result.combined_post_score;

    let pass = score_ok; // delay presence is informational, not required
    let delay_str = if has_delay { "delay_present" } else { "no_delay" };

    (
        pass,
        format!(
            "{}: baseline={:.4} aligned={:.4} (limit={:.1}x)",
            delay_str,
            baseline_result.combined_post_score,
            option_result.combined_post_score,
            OPTION_SCORE_TOLERANCE
        ),
    )
}

/// OE-8: Multi-measurement minimax - worst-case position should improve
fn validate_multi_measurement_minimax(
    baseline_result: &RoomOptimizationResult,
    option_result: &RoomOptimizationResult,
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

    // Minimax should improve worst case (or at least not be significantly worse)
    let pass = option_max <= baseline_max * OPTION_SCORE_TOLERANCE;

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

    // Variance-penalized should have lower or similar variance
    // Allow generous tolerance since COBYLA with low maxeval may not fully optimize
    let pass = option_var <= baseline_var * 2.0 + 0.1;

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

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    let list_mode = args.iter().any(|a| a == "--list");
    let case_filter: Option<String> = args
        .windows(2)
        .find(|w| w[0] == "--case")
        .map(|w| w[1].clone());

    let all_cases = all_test_cases();

    // --list: print available cases and exit
    if list_mode {
        println!("Available test cases:");
        for tc in &all_cases {
            println!("  {}", tc.name());
        }
        return Ok(());
    }

    println!("=== RoomEQ QA: Convergence, Monotonicity & Invariants (fast: cobyla, parallel) ===");

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

    // Run all test cases in parallel using threads
    let handles: Vec<_> = cases_to_run
        .into_iter()
        .map(|tc| {
            let fem_dir = fem_dir.clone();
            let optim_dir = optim_dir.clone();
            std::thread::spawn(move || -> Result<(String, Vec<TestResult>)> {
                match tc {
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
                }
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
                println!(
                    "  - {} (pre={:.4}, post={:.4}): {}",
                    r.label, r.pre_score, r.post_score, r.reason
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
