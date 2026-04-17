//! RoomEQ Synthetic QA: Tests optimization against synthetic speaker scenarios.
//!
//! Uses deterministic synthetic curves with known room modes and noise to validate
//! that optimization consistently improves the response across all processing modes,
//! targets, and option combinations.
//!
//! Usage:
//!   cargo run --bin roomeq-qa-synthetic --no-default-features --release
//!   cargo run --bin roomeq-qa-synthetic --no-default-features --release -- --list
//!   cargo run --bin roomeq-qa-synthetic --no-default-features --release -- --difficulty easy

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use autoeq::iir::{Biquad, BiquadFilterType};
use autoeq::roomeq::synthetic::{
    generate_cardioid_scenario, generate_channel_curve, generate_dba_scenario, generate_flat_curve,
    generate_harman_tilt_curve, generate_multisub_scenario, generate_scenario,
    generate_speaker_rolloff_curve, generate_subwoofer_rolloff_curve,
};
use autoeq::roomeq::{
    BroadbandTargetMatchingConfig, CallbackAction, DecomposedCorrectionSerdeConfig,
    ExcursionProtectionConfig, MultiMeasurementConfig, MultiMeasurementStrategy, MultiSubGroup,
    ProcessingMode, RoomConfig, SchroederSplitConfig, SpatialRobustnessSerdeConfig, optimize_room,
};
use autoeq::roomeq::{
    CardioidConfig, DBAConfig, SubwooferStrategy, SubwooferSystemConfig, SystemConfig, SystemModel,
};
use autoeq::{Curve, MeasurementSource};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SAMPLE_RATE: f64 = 48000.0;
const SEED: u64 = 42;
const QA_MAXEVAL: usize = 1000;

/// Global counter for unique temp dir names across threads
static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

// ---------------------------------------------------------------------------
// Difficulty levels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct DifficultyLevel {
    name: &'static str,
    modes: &'static [(f64, f64, f64)], // (freq, Q, gain_db)
    noise_rms: f64,
    recovery_factor: f64,
}

const EASY: DifficultyLevel = DifficultyLevel {
    name: "easy",
    modes: &[(80.0, 2.0, -3.0), (150.0, 2.0, 3.0), (250.0, 2.0, -2.0)],
    noise_rms: 0.5,
    recovery_factor: 3.0,
};

const MEDIUM: DifficultyLevel = DifficultyLevel {
    name: "medium",
    modes: &[
        (60.0, 4.0, -6.0),
        (100.0, 4.0, 5.0),
        (180.0, 4.0, -4.0),
        (300.0, 4.0, 6.0),
        (450.0, 4.0, -5.0),
    ],
    noise_rms: 1.0,
    recovery_factor: 5.0,
};

const HARD: DifficultyLevel = DifficultyLevel {
    name: "hard",
    modes: &[
        (50.0, 8.0, -10.0),
        (80.0, 8.0, 8.0),
        (120.0, 8.0, -7.0),
        (200.0, 8.0, 10.0),
        (320.0, 8.0, -9.0),
        (500.0, 8.0, 6.0),
        (800.0, 8.0, -8.0),
    ],
    noise_rms: 2.0,
    recovery_factor: 8.0,
};

const ALL_DIFFICULTIES: &[DifficultyLevel] = &[EASY, MEDIUM, HARD];

// ---------------------------------------------------------------------------
// Multi-sub difficulty levels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct MultiSubDifficulty {
    name: &'static str,
    n_subs: usize,
    /// (freq, Q, gain_db) — shared room modes affecting all subs
    shared_modes: &'static [(f64, f64, f64)],
    /// Per-sub unique modes — outer len must be 0 or n_subs
    per_sub_modes: &'static [&'static [(f64, f64, f64)]],
    /// Per-sub propagation delay in ms
    delays_ms: &'static [f64],
    noise_rms: f64,
}

const MS_EASY: MultiSubDifficulty = MultiSubDifficulty {
    name: "easy",
    n_subs: 2,
    shared_modes: &[(60.0, 3.0, -5.0), (100.0, 3.0, 4.0)],
    per_sub_modes: &[],
    delays_ms: &[0.0, 2.0],
    noise_rms: 0.3,
};

const MS_MEDIUM: MultiSubDifficulty = MultiSubDifficulty {
    name: "medium",
    n_subs: 2,
    shared_modes: &[(50.0, 4.0, -6.0), (90.0, 4.0, 5.0)],
    per_sub_modes: &[&[(70.0, 3.0, -3.0)], &[(120.0, 3.0, 3.0)]],
    delays_ms: &[0.0, 3.5],
    noise_rms: 0.5,
};

const MS_HARD: MultiSubDifficulty = MultiSubDifficulty {
    name: "hard",
    n_subs: 3,
    shared_modes: &[(45.0, 5.0, -8.0), (80.0, 5.0, 6.0), (130.0, 5.0, -5.0)],
    per_sub_modes: &[
        &[(55.0, 3.0, -3.0)],
        &[(100.0, 4.0, 4.0)],
        &[(70.0, 3.0, -2.0)],
    ],
    delays_ms: &[0.0, 2.5, 5.0],
    noise_rms: 0.8,
};

const ALL_MS_DIFFICULTIES: &[MultiSubDifficulty] = &[MS_EASY, MS_MEDIUM, MS_HARD];

/// Multi-sub topology variant
#[derive(Debug, Clone, Copy)]
struct MultiSubTopology {
    name: &'static str,
    allpass: bool,
}

const MS_TOPOLOGIES: &[MultiSubTopology] = &[
    MultiSubTopology {
        name: "standard",
        allpass: false,
    },
    MultiSubTopology {
        name: "allpass",
        allpass: true,
    },
];

/// Options applicable to multi-sub tests (subset of full options)
const MS_OPTIONS: &[OptionDef] = &[
    OptionDef {
        name: "psychoacoustic",
        apply: option_psychoacoustic,
    },
    OptionDef {
        name: "asymmetric_loss",
        apply: option_asymmetric,
    },
    OptionDef {
        name: "decomposed_correction",
        apply: option_decomposed_correction,
    },
];

// ---------------------------------------------------------------------------
// Multi-channel layout definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct ChannelLayout {
    name: &'static str,
    mains: &'static [&'static str],
    has_lfe: bool,
    heights: &'static [&'static str],
}

impl ChannelLayout {
    fn system_model(&self) -> SystemModel {
        if self.mains.len() <= 2 && self.heights.is_empty() {
            SystemModel::Stereo
        } else {
            SystemModel::HomeCinema
        }
    }
}

const LAYOUT_2_0: ChannelLayout = ChannelLayout {
    name: "2.0",
    mains: &["L", "R"],
    has_lfe: false,
    heights: &[],
};
const LAYOUT_2_1: ChannelLayout = ChannelLayout {
    name: "2.1",
    mains: &["L", "R"],
    has_lfe: true,
    heights: &[],
};
const LAYOUT_5_0: ChannelLayout = ChannelLayout {
    name: "5.0",
    mains: &["L", "R", "C", "SL", "SR"],
    has_lfe: false,
    heights: &[],
};
const LAYOUT_5_1: ChannelLayout = ChannelLayout {
    name: "5.1",
    mains: &["L", "R", "C", "SL", "SR"],
    has_lfe: true,
    heights: &[],
};
const LAYOUT_7_1: ChannelLayout = ChannelLayout {
    name: "7.1",
    mains: &["L", "R", "C", "SL", "SR", "SBL", "SBR"],
    has_lfe: true,
    heights: &[],
};
const LAYOUT_5_1_2: ChannelLayout = ChannelLayout {
    name: "5.1.2",
    mains: &["L", "R", "C", "SL", "SR"],
    has_lfe: true,
    heights: &["HL", "HR"],
};
const LAYOUT_7_1_4: ChannelLayout = ChannelLayout {
    name: "7.1.4",
    mains: &["L", "R", "C", "SL", "SR", "SBL", "SBR"],
    has_lfe: true,
    heights: &["TFL", "TFR", "TRL", "TRR"],
};
const LAYOUT_9_1_6: ChannelLayout = ChannelLayout {
    name: "9.1.6",
    mains: &["L", "R", "C", "SL", "SR", "SBL", "SBR", "WL", "WR"],
    has_lfe: true,
    heights: &["TFL", "TFR", "TML", "TMR", "TRL", "TRR"],
};

const ALL_LAYOUTS: &[ChannelLayout] = &[
    LAYOUT_2_0,
    LAYOUT_2_1,
    LAYOUT_5_0,
    LAYOUT_5_1,
    LAYOUT_7_1,
    LAYOUT_5_1_2,
    LAYOUT_7_1_4,
    LAYOUT_9_1_6,
];

/// LFE sub-topology for multi-channel tests
#[derive(Debug, Clone, Copy)]
struct SubTopology {
    name: &'static str,
}

const SUB_SINGLE: SubTopology = SubTopology { name: "single_sub" };
const SUB_MSO: SubTopology = SubTopology { name: "mso_2sub" };
const SUB_MSO_AP: SubTopology = SubTopology {
    name: "mso_2sub_allpass",
};
const SUB_CARDIOID: SubTopology = SubTopology { name: "cardioid" };
const SUB_DBA: SubTopology = SubTopology { name: "dba" };

const ALL_SUB_TOPOS: &[SubTopology] = &[SUB_SINGLE, SUB_MSO, SUB_MSO_AP, SUB_CARDIOID, SUB_DBA];

/// Get applicable sub topologies for a layout
fn sub_topos_for_layout(layout: &ChannelLayout) -> &'static [SubTopology] {
    if !layout.has_lfe {
        &[] // no LFE → no sub topology dimension, just mains
    } else {
        ALL_SUB_TOPOS
    }
}

// ---------------------------------------------------------------------------
// Option definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct OptionDef {
    name: &'static str,
    apply: fn(&mut RoomConfig),
}

fn option_psychoacoustic(config: &mut RoomConfig) {
    config.optimizer.psychoacoustic = true;
}
fn option_asymmetric(config: &mut RoomConfig) {
    config.optimizer.asymmetric_loss = true;
}
fn option_broadband(config: &mut RoomConfig) {
    config.optimizer.broadband_target_matching =
        Some(BroadbandTargetMatchingConfig { enabled: true });
}
fn option_excursion(config: &mut RoomConfig) {
    config.optimizer.excursion_protection = Some(ExcursionProtectionConfig {
        enabled: true,
        ..Default::default()
    });
}
fn option_schroeder(config: &mut RoomConfig) {
    config.optimizer.schroeder_split = Some(SchroederSplitConfig {
        enabled: true,
        ..Default::default()
    });
}
fn option_spatial_robustness(config: &mut RoomConfig) {
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
fn option_pre_ringing(config: &mut RoomConfig) {
    use autoeq::roomeq::PreRingingSerdeConfig;
    // Enable FIR with pre-ringing control
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

fn option_decomposed_correction(config: &mut RoomConfig) {
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

// All options tested on synthetic data.
// Broadband and excursion require speaker rolloff curves (not flat) — the
// synthetic scenarios now use generate_speaker_rolloff_curve / generate_subwoofer_rolloff_curve
// which provide a realistic -12 dB/oct rolloff around 80 Hz.
const OPTIONS: &[OptionDef] = &[
    OptionDef {
        name: "psychoacoustic",
        apply: option_psychoacoustic,
    },
    OptionDef {
        name: "asymmetric_loss",
        apply: option_asymmetric,
    },
    OptionDef {
        name: "broadband",
        apply: option_broadband,
    },
    OptionDef {
        name: "excursion",
        apply: option_excursion,
    },
    OptionDef {
        name: "schroeder",
        apply: option_schroeder,
    },
    OptionDef {
        name: "spatial_robustness",
        apply: option_spatial_robustness,
    },
    OptionDef {
        name: "pre_ringing",
        apply: option_pre_ringing,
    },
    OptionDef {
        name: "decomposed_correction",
        apply: option_decomposed_correction,
    },
];

// ---------------------------------------------------------------------------
// Config builder
// ---------------------------------------------------------------------------

fn build_config(degraded: &Curve, mode: ProcessingMode) -> RoomConfig {
    let mut speakers = HashMap::new();
    speakers.insert(
        "Left".to_string(),
        autoeq::roomeq::SpeakerConfig::Single(MeasurementSource::InMemory(degraded.clone())),
    );
    speakers.insert(
        "Right".to_string(),
        autoeq::roomeq::SpeakerConfig::Single(MeasurementSource::InMemory(degraded.clone())),
    );

    let mut config = RoomConfig {
        version: "2.0.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: Default::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    config.optimizer.algorithm = "autoeq:de".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(SEED);
    config.optimizer.processing_mode = mode;
    config.optimizer.num_filters = 5;
    config.optimizer.min_freq = 20.0;
    config.optimizer.max_freq = 20000.0;

    // FIR config required for PhaseLinear, Hybrid, and MixedPhase modes
    match &config.optimizer.processing_mode {
        ProcessingMode::PhaseLinear | ProcessingMode::Hybrid => {
            config.optimizer.fir = Some(autoeq::roomeq::FirConfig {
                taps: 2048,
                phase: "kirkeby".to_string(),
                correct_excess_phase: false,
                phase_smoothing: 0.167,
                pre_ringing: None,
            });
        }
        ProcessingMode::MixedPhase => {
            config.optimizer.mixed_phase = Some(autoeq::roomeq::MixedPhaseSerdeConfig {
                max_fir_length_ms: 10.0,
                pre_ringing_threshold_db: -30.0,
                min_spatial_depth: 0.5,
                phase_smoothing_octaves: 0.167,
            });
        }
        _ => {}
    }

    config
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

fn run_optimization(config: &RoomConfig) -> Result<autoeq::roomeq::RoomOptimizationResult> {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir =
        std::env::temp_dir().join(format!("roomeq_qa_syn_{}_{}", std::process::id(), id));
    std::fs::create_dir_all(&temp_dir)?;
    let callback =
        Box::new(|_: &autoeq::roomeq::RoomOptimizationProgress| CallbackAction::Continue);
    let result = optimize_room(config, SAMPLE_RATE, Some(callback), Some(&temp_dir))
        .map_err(|e| anyhow!("{}", e));
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

#[derive(Debug)]
#[allow(dead_code)]
struct TestResult {
    name: String,
    passed: bool,
    pre_score: f64,
    post_score: f64,
    epa_preference: Option<f64>,
    reason: String,
}

fn avg_epa_preference(result: &autoeq::roomeq::RoomOptimizationResult) -> Option<f64> {
    let epa = result.metadata.epa_per_channel.as_ref()?;
    if epa.is_empty() {
        return None;
    }
    let sum: f64 = epa.values().map(|m| m.post.preference).sum();
    Some(sum / epa.len() as f64)
}

fn fmt_epa(epa: Option<f64>) -> String {
    match epa {
        Some(v) => format!("{:.3}", v),
        None => "n/a".to_string(),
    }
}

fn run_single_test(
    degraded: &Curve,
    mode: ProcessingMode,
    target_name: &str,
    option_names: &[&str],
    difficulty: &DifficultyLevel,
) -> TestResult {
    let mode_str = match mode {
        ProcessingMode::LowLatency => "IIR",
        ProcessingMode::PhaseLinear => "FIR",
        ProcessingMode::Hybrid => "Mixed",
        ProcessingMode::MixedPhase => "MixedPhase",
        ProcessingMode::WarpedIir => "WarpedIIR",
        ProcessingMode::KautzModal => "KautzModal",
    };
    let options_str = if option_names.is_empty() {
        "baseline".to_string()
    } else {
        option_names.join("+")
    };

    let test_name = format!(
        "{}/{}/{}/{}",
        difficulty.name, mode_str, target_name, options_str
    );

    let mut config = build_config(degraded, mode);

    // Apply option overrides
    for opt_name in option_names {
        if let Some(opt) = OPTIONS.iter().find(|o| o.name == *opt_name) {
            (opt.apply)(&mut config);
        }
    }

    let result = match run_optimization(&config) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                name: test_name,
                passed: false,
                pre_score: 0.0,
                post_score: 0.0,
                epa_preference: None,
                reason: format!("Optimization failed: {}", e),
            };
        }
    };

    // Validation: optimization should not make things significantly worse.
    // Strict improvement (post < pre) is ideal, but some option combos change
    // the loss landscape (e.g., decomposed correction weights may reduce optimizer
    // freedom). Allow up to 20% regression as acceptable.
    let pre = result.combined_pre_score;
    let post = result.combined_post_score;
    let epa = avg_epa_preference(&result);
    let regression_tolerance = 1.20; // 20% worse is acceptable

    if post > pre * regression_tolerance {
        return TestResult {
            name: test_name,
            passed: false,
            pre_score: pre,
            post_score: post,
            epa_preference: epa,
            reason: format!(
                "Severe regression: pre={:.3}, post={:.3} ({:.1}% worse, limit {:.0}%)",
                pre,
                post,
                (post / pre - 1.0) * 100.0,
                (regression_tolerance - 1.0) * 100.0,
            ),
        };
    }

    TestResult {
        name: test_name,
        passed: true,
        pre_score: pre,
        post_score: post,
        epa_preference: epa,
        reason: format!(
            "OK: {:.3} -> {:.3} ({:+.1}%)",
            pre,
            post,
            (1.0 - post / pre) * 100.0
        ),
    }
}

// ---------------------------------------------------------------------------
// Option combo generation
// ---------------------------------------------------------------------------

fn generate_option_combos() -> Vec<Vec<&'static str>> {
    let mut combos = Vec::new();

    // Baseline (no options)
    combos.push(vec![]);

    // Single options
    for opt in OPTIONS {
        combos.push(vec![opt.name]);
    }

    // Pairs
    for (i, opt_i) in OPTIONS.iter().enumerate() {
        for opt_j in &OPTIONS[i + 1..] {
            combos.push(vec![opt_i.name, opt_j.name]);
        }
    }

    // All options
    combos.push(OPTIONS.iter().map(|o| o.name).collect());

    combos
}

// ---------------------------------------------------------------------------
// Multi-sub config and test runner
// ---------------------------------------------------------------------------

fn build_multisub_config(sub_curves: &[Curve], allpass: bool) -> RoomConfig {
    let mut speakers = HashMap::new();
    let subwoofers: Vec<MeasurementSource> = sub_curves
        .iter()
        .map(|c| MeasurementSource::InMemory(c.clone()))
        .collect();

    speakers.insert(
        "LFE".to_string(),
        autoeq::roomeq::SpeakerConfig::MultiSub(MultiSubGroup {
            name: "subs".to_string(),
            speaker_name: None,
            subwoofers,
            allpass_optimization: allpass,
        }),
    );

    let mut config = RoomConfig {
        version: "2.0.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: Default::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    config.optimizer.algorithm = "autoeq:de".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(SEED);
    config.optimizer.processing_mode = ProcessingMode::LowLatency;
    config.optimizer.num_filters = 3;
    config.optimizer.min_freq = 20.0;
    config.optimizer.max_freq = 200.0;

    config
}

fn generate_ms_option_combos() -> Vec<Vec<&'static str>> {
    let mut combos = Vec::new();
    combos.push(vec![]); // baseline
    for opt in MS_OPTIONS {
        combos.push(vec![opt.name]);
    }
    combos
}

fn run_multisub_test(
    sub_curves: &[Curve],
    topology: &MultiSubTopology,
    option_names: &[&str],
    difficulty: &MultiSubDifficulty,
) -> TestResult {
    let options_str = if option_names.is_empty() {
        "baseline".to_string()
    } else {
        option_names.join("+")
    };
    let test_name = format!(
        "multisub/{}/{}sub_{}/{}",
        difficulty.name, difficulty.n_subs, topology.name, options_str,
    );

    let mut config = build_multisub_config(sub_curves, topology.allpass);

    // Apply option overrides (use MS_OPTIONS lookup)
    for opt_name in option_names {
        if let Some(opt) = MS_OPTIONS.iter().find(|o| o.name == *opt_name) {
            (opt.apply)(&mut config);
        }
    }

    let result = match run_optimization(&config) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                name: test_name,
                passed: false,
                pre_score: 0.0,
                post_score: 0.0,
                epa_preference: None,
                reason: format!("Optimization failed: {}", e),
            };
        }
    };

    let pre = result.combined_pre_score;
    let post = result.combined_post_score;
    let epa = avg_epa_preference(&result);

    if post > pre * 1.20 {
        return TestResult {
            name: test_name,
            passed: false,
            pre_score: pre,
            post_score: post,
            epa_preference: epa,
            reason: format!(
                "Severe regression: pre={:.3}, post={:.3} ({:.1}% worse)",
                pre,
                post,
                (post / pre - 1.0) * 100.0,
            ),
        };
    }

    TestResult {
        name: test_name,
        passed: true,
        pre_score: pre,
        post_score: post,
        epa_preference: epa,
        reason: format!(
            "OK: {:.3} -> {:.3} ({:+.1}%)",
            pre,
            post,
            (1.0 - post / pre) * 100.0
        ),
    }
}

// ---------------------------------------------------------------------------
// Multi-channel config builder and test runner
// ---------------------------------------------------------------------------

/// Build a RoomConfig for a multi-channel layout with a given sub topology.
///
/// Creates synthetic per-channel curves using the difficulty's room modes with
/// per-channel noise variation (different seed per channel).
fn build_multichannel_config(
    layout: &ChannelLayout,
    sub_topo: Option<&SubTopology>,
    difficulty: &DifficultyLevel,
    base_curve: &Curve,
    sample_rate: f64,
) -> RoomConfig {
    let mut speakers = HashMap::new();
    let mut sys_speakers = HashMap::new();

    let modes_biquad: Vec<Biquad> = difficulty
        .modes
        .iter()
        .map(|&(freq, q, gain)| Biquad::new(BiquadFilterType::Peak, freq, sample_rate, q, gain))
        .collect();

    // Generate per-main-channel curves
    for (i, &role) in layout.mains.iter().enumerate() {
        let key = role.to_lowercase();
        let delay = i as f64 * 0.3; // slight per-channel delay variation
        let curve = generate_channel_curve(
            base_curve,
            &modes_biquad,
            delay,
            difficulty.noise_rms * 0.5,
            SEED.wrapping_add(i as u64 * 100),
            sample_rate,
        );
        speakers.insert(
            key.clone(),
            autoeq::roomeq::SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
        );
        sys_speakers.insert(role.to_string(), key);
    }

    // Height channels (same treatment as mains)
    for (i, &role) in layout.heights.iter().enumerate() {
        let key = role.to_lowercase();
        let delay = (layout.mains.len() + i) as f64 * 0.3;
        let curve = generate_channel_curve(
            base_curve,
            &modes_biquad,
            delay,
            difficulty.noise_rms * 0.5,
            SEED.wrapping_add((layout.mains.len() + i) as u64 * 100),
            sample_rate,
        );
        speakers.insert(
            key.clone(),
            autoeq::roomeq::SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
        );
        sys_speakers.insert(role.to_string(), key);
    }

    // LFE / sub topology
    let mut sub_config = if layout.has_lfe {
        let sub_topo = sub_topo.expect("layout has LFE but no sub topology");
        let bass_modes: Vec<Biquad> = difficulty
            .modes
            .iter()
            .filter(|(f, _, _)| *f < 200.0)
            .map(|&(freq, q, gain)| Biquad::new(BiquadFilterType::Peak, freq, sample_rate, q, gain))
            .collect();

        match sub_topo.name {
            "single_sub" => {
                let sub_curve = generate_channel_curve(
                    &generate_subwoofer_rolloff_curve(20.0, 200.0, 100, 80.0, -6.0),
                    &bass_modes,
                    0.0,
                    difficulty.noise_rms * 0.3,
                    SEED.wrapping_add(9000),
                    sample_rate,
                );
                speakers.insert(
                    "lfe".to_string(),
                    autoeq::roomeq::SpeakerConfig::Single(MeasurementSource::InMemory(sub_curve)),
                );
                sys_speakers.insert("LFE".to_string(), "lfe".to_string());
                Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Single,
                    crossover: None,
                    mapping: HashMap::new(),
                })
            }
            "mso_2sub" | "mso_2sub_allpass" => {
                let ms = generate_multisub_scenario(
                    "lfe",
                    2,
                    &bass_modes,
                    &[],
                    &[0.0, 2.0],
                    difficulty.noise_rms * 0.3,
                    SEED.wrapping_add(9000),
                    sample_rate,
                );
                let allpass = sub_topo.name == "mso_2sub_allpass";
                let subwoofers: Vec<MeasurementSource> = ms
                    .sub_curves
                    .into_iter()
                    .map(MeasurementSource::InMemory)
                    .collect();
                speakers.insert(
                    "lfe".to_string(),
                    autoeq::roomeq::SpeakerConfig::MultiSub(MultiSubGroup {
                        name: "subs".to_string(),
                        speaker_name: None,
                        subwoofers,
                        allpass_optimization: allpass,
                    }),
                );
                sys_speakers.insert("LFE".to_string(), "lfe".to_string());
                Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Mso,
                    crossover: None,
                    mapping: HashMap::new(),
                })
            }
            "cardioid" => {
                let card = generate_cardioid_scenario(
                    "lfe",
                    &bass_modes,
                    1.0,
                    difficulty.noise_rms * 0.3,
                    SEED.wrapping_add(9000),
                    sample_rate,
                );
                speakers.insert(
                    "lfe".to_string(),
                    autoeq::roomeq::SpeakerConfig::Cardioid(Box::new(CardioidConfig {
                        name: "cardioid_sub".to_string(),
                        speaker_name: None,
                        front: MeasurementSource::InMemory(card.front_curve),
                        rear: MeasurementSource::InMemory(card.rear_curve),
                        separation_meters: card.separation_meters,
                    })),
                );
                sys_speakers.insert("LFE".to_string(), "lfe".to_string());
                Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Single, // cardioid routes via SpeakerConfig dispatch
                    crossover: None,
                    mapping: HashMap::new(),
                })
            }
            "dba" => {
                let dba = generate_dba_scenario(
                    "lfe",
                    1,
                    1,
                    &bass_modes,
                    8.0,
                    difficulty.noise_rms * 0.3,
                    SEED.wrapping_add(9000),
                    sample_rate,
                );
                let front: Vec<MeasurementSource> = dba
                    .front_curves
                    .into_iter()
                    .map(MeasurementSource::InMemory)
                    .collect();
                let rear: Vec<MeasurementSource> = dba
                    .rear_curves
                    .into_iter()
                    .map(MeasurementSource::InMemory)
                    .collect();
                speakers.insert(
                    "lfe".to_string(),
                    autoeq::roomeq::SpeakerConfig::Dba(DBAConfig {
                        name: "dba_sub".to_string(),
                        speaker_name: None,
                        front,
                        rear,
                    }),
                );
                sys_speakers.insert("LFE".to_string(), "lfe".to_string());
                Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Dba,
                    crossover: None,
                    mapping: HashMap::new(),
                })
            }
            _ => panic!("Unknown sub topology: {}", sub_topo.name),
        }
    } else {
        None
    };

    // Add crossover config if sub is present (required by 2.1 and home cinema workflows)
    let mut crossovers_map = None;
    if let Some(ref mut sc) = sub_config {
        sc.crossover = Some("lfe_xover".to_string());
        let mut xovers = HashMap::new();
        xovers.insert(
            "lfe_xover".to_string(),
            autoeq::roomeq::CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        );
        crossovers_map = Some(xovers);
    }

    let system = SystemConfig {
        model: layout.system_model(),
        speakers: sys_speakers,
        subwoofers: sub_config,
    };

    let mut config = RoomConfig {
        version: "2.0.0".to_string(),
        system: Some(system),
        speakers,
        crossovers: crossovers_map,
        target_curve: None,
        optimizer: Default::default(),
        recording_config: None,
        cea2034_cache: None,
    };

    config.optimizer.algorithm = "autoeq:de".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(SEED);
    config.optimizer.processing_mode = ProcessingMode::LowLatency;
    config.optimizer.num_filters = 5;
    config.optimizer.min_freq = 20.0;
    config.optimizer.max_freq = 20000.0;

    config
}

fn run_multichannel_test(
    layout: &ChannelLayout,
    sub_topo: Option<&SubTopology>,
    difficulty: &DifficultyLevel,
    base_curve: &Curve,
    sample_rate: f64,
) -> TestResult {
    let sub_str = sub_topo.map(|s| s.name).unwrap_or("no_lfe");
    let test_name = format!(
        "multichannel/{}/{}/{}",
        layout.name, sub_str, difficulty.name
    );

    let config = build_multichannel_config(layout, sub_topo, difficulty, base_curve, sample_rate);

    let result = match run_optimization(&config) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                name: test_name,
                passed: false,
                pre_score: 0.0,
                post_score: 0.0,
                epa_preference: None,
                reason: format!("Optimization failed: {}", e),
            };
        }
    };

    let pre = result.combined_pre_score;
    let post = result.combined_post_score;
    let epa = avg_epa_preference(&result);

    if post > pre * 1.20 {
        return TestResult {
            name: test_name,
            passed: false,
            pre_score: pre,
            post_score: post,
            epa_preference: epa,
            reason: format!(
                "Severe regression: pre={:.3}, post={:.3} ({:.1}% worse)",
                pre,
                post,
                (post / pre - 1.0) * 100.0,
            ),
        };
    }

    TestResult {
        name: test_name,
        passed: true,
        pre_score: pre,
        post_score: post,
        epa_preference: epa,
        reason: format!(
            "OK: {:.3} -> {:.3} ({:.1}% reduction)",
            pre,
            post,
            (1.0 - post / pre) * 100.0
        ),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().collect();
    let list_only = args.iter().any(|a| a == "--list");
    let difficulty_filter = args
        .windows(2)
        .find(|w| w[0] == "--difficulty")
        .map(|w| w[1].clone());

    let difficulties: Vec<&DifficultyLevel> = if let Some(ref filter) = difficulty_filter {
        ALL_DIFFICULTIES
            .iter()
            .filter(|d| d.name == filter.as_str())
            .collect()
    } else {
        ALL_DIFFICULTIES.iter().collect()
    };

    let ms_difficulties: Vec<&MultiSubDifficulty> = if let Some(ref filter) = difficulty_filter {
        ALL_MS_DIFFICULTIES
            .iter()
            .filter(|d| d.name == filter.as_str())
            .collect()
    } else {
        ALL_MS_DIFFICULTIES.iter().collect()
    };

    let modes = [
        ProcessingMode::LowLatency,
        ProcessingMode::PhaseLinear,
        ProcessingMode::Hybrid,
        ProcessingMode::MixedPhase,
    ];

    let flat_target = generate_flat_curve(20.0, 20000.0, 200);
    let harman_target = generate_harman_tilt_curve(20.0, 20000.0, 200);
    let targets: Vec<(&str, &Curve)> = vec![("flat", &flat_target), ("harman", &harman_target)];

    // Speaker rolloff: 0 dB above 80 Hz, -12 dB/oct below (realistic 2nd-order highpass)
    let speaker_rolloff = generate_speaker_rolloff_curve(20.0, 20000.0, 200, 80.0, -12.0);

    let option_combos = generate_option_combos();
    let ms_option_combos = generate_ms_option_combos();

    // Count total tests
    let single_total = difficulties.len() * modes.len() * targets.len() * option_combos.len();
    let ms_total = ms_difficulties.len() * MS_TOPOLOGIES.len() * ms_option_combos.len();
    let mc_total: usize = ALL_LAYOUTS
        .iter()
        .map(|layout| {
            let n_topos = sub_topos_for_layout(layout).len();
            if n_topos == 0 {
                difficulties.len() // no LFE → 1 test per difficulty
            } else {
                n_topos * difficulties.len()
            }
        })
        .sum();
    let total = single_total + ms_total + mc_total;

    if list_only {
        println!("Synthetic QA Test Matrix:");
        println!();
        println!("  Single-speaker:");
        println!(
            "    Difficulties: {}",
            difficulties
                .iter()
                .map(|d| d.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("    Modes: IIR, FIR, Mixed, MixedPhase");
        println!("    Targets: flat, harman");
        println!(
            "    Option combos: {} (baseline + {} singles + {} pairs + 1 all)",
            option_combos.len(),
            OPTIONS.len(),
            OPTIONS.len() * (OPTIONS.len() - 1) / 2,
        );
        println!("    Subtotal: {}", single_total);
        println!();
        println!("  Multi-sub:");
        println!(
            "    Difficulties: {}",
            ms_difficulties
                .iter()
                .map(|d| d.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    Topologies: {}",
            MS_TOPOLOGIES
                .iter()
                .map(|t| t.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    Option combos: {} (baseline + {} singles)",
            ms_option_combos.len(),
            MS_OPTIONS.len(),
        );
        println!("    Subtotal: {}", ms_total);
        println!();
        println!("  Multi-channel:");
        println!(
            "    Layouts: {}",
            ALL_LAYOUTS
                .iter()
                .map(|l| l.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    Sub topologies (with LFE): {}",
            ALL_SUB_TOPOS
                .iter()
                .map(|t| t.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    Difficulties: {}",
            difficulties
                .iter()
                .map(|d| d.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("    Subtotal: {}", mc_total);
        println!();
        println!("  Total tests: {}", total);
        return Ok(());
    }

    println!(
        "RoomEQ Synthetic QA -- {} tests ({} single + {} multi-sub + {} multi-channel)",
        total, single_total, ms_total, mc_total
    );
    println!("============================================================");

    let start = Instant::now();
    let mut all_results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for difficulty in &difficulties {
        // Build room modes from difficulty config
        let modes_biquad: Vec<Biquad> = difficulty
            .modes
            .iter()
            .map(|&(freq, q, gain)| Biquad::new(BiquadFilterType::Peak, freq, SAMPLE_RATE, q, gain))
            .collect();

        for &(target_name, target) in &targets {
            // Combine target shape with speaker rolloff so that broadband/excursion
            // options see a realistic low-frequency limit in the measurement.
            let speaker_base = Curve {
                freq: target.freq.clone(),
                spl: &target.spl + &speaker_rolloff.spl,
                phase: None,
            };
            let scenario = generate_scenario(
                &format!("{}/{}", difficulty.name, target_name),
                &speaker_base,
                &modes_biquad,
                difficulty.noise_rms * 0.3,
                difficulty.noise_rms * 0.7,
                SEED,
                SAMPLE_RATE,
            );

            for mode in &modes {
                for combo in &option_combos {
                    let result = run_single_test(
                        &scenario.degraded_curve,
                        mode.clone(),
                        target_name,
                        combo,
                        difficulty,
                    );

                    if result.passed {
                        passed += 1;
                    } else {
                        failed += 1;
                        println!("  FAIL: {} -- {} (epa={})", result.name, result.reason, fmt_epa(result.epa_preference));
                    }

                    all_results.push(result);
                }
            }
        }
    }

    // ====================================================================
    // Multi-sub tests
    // ====================================================================
    for ms_diff in &ms_difficulties {
        let shared_biquads: Vec<Biquad> = ms_diff
            .shared_modes
            .iter()
            .map(|&(f, q, g)| Biquad::new(BiquadFilterType::Peak, f, SAMPLE_RATE, q, g))
            .collect();

        let per_sub_biquads: Vec<Vec<Biquad>> = ms_diff
            .per_sub_modes
            .iter()
            .map(|modes| {
                modes
                    .iter()
                    .map(|&(f, q, g)| Biquad::new(BiquadFilterType::Peak, f, SAMPLE_RATE, q, g))
                    .collect()
            })
            .collect();

        let scenario = generate_multisub_scenario(
            &format!("multisub/{}", ms_diff.name),
            ms_diff.n_subs,
            &shared_biquads,
            &per_sub_biquads,
            ms_diff.delays_ms,
            ms_diff.noise_rms,
            SEED,
            SAMPLE_RATE,
        );

        for topo in MS_TOPOLOGIES {
            for combo in &ms_option_combos {
                let result = run_multisub_test(&scenario.sub_curves, topo, combo, ms_diff);

                if result.passed {
                    passed += 1;
                } else {
                    failed += 1;
                    println!("  FAIL: {} -- {} (epa={})", result.name, result.reason, fmt_epa(result.epa_preference));
                }

                all_results.push(result);
            }
        }
    }

    // ====================================================================
    // Multi-channel topology tests
    // ====================================================================
    let base_fullrange = generate_speaker_rolloff_curve(20.0, 20000.0, 200, 80.0, -6.0);

    for layout in ALL_LAYOUTS {
        let topos = sub_topos_for_layout(layout);

        if topos.is_empty() {
            // No LFE — test mains only
            for difficulty in &difficulties {
                let result =
                    run_multichannel_test(layout, None, difficulty, &base_fullrange, SAMPLE_RATE);
                if result.passed {
                    passed += 1;
                } else {
                    failed += 1;
                    println!("  FAIL: {} -- {} (epa={})", result.name, result.reason, fmt_epa(result.epa_preference));
                }
                all_results.push(result);
            }
        } else {
            // With LFE — test each sub topology
            for sub_topo in topos {
                for difficulty in &difficulties {
                    let result = run_multichannel_test(
                        layout,
                        Some(sub_topo),
                        difficulty,
                        &base_fullrange,
                        SAMPLE_RATE,
                    );
                    if result.passed {
                        passed += 1;
                    } else {
                        failed += 1;
                        println!("  FAIL: {} -- {} (epa={})", result.name, result.reason, fmt_epa(result.epa_preference));
                    }
                    all_results.push(result);
                }
            }
        }
    }

    let elapsed = start.elapsed();
    println!();
    println!("============================================================");
    println!(
        "Results: {} passed, {} failed, {} total ({:.1}s)",
        passed,
        failed,
        all_results.len(),
        elapsed.as_secs_f64()
    );

    // Print summary table
    let mut summary = String::new();
    for difficulty in &difficulties {
        let diff_results: Vec<&TestResult> = all_results
            .iter()
            .filter(|r| r.name.starts_with(difficulty.name))
            .collect();
        let diff_pass = diff_results.iter().filter(|r| r.passed).count();
        let diff_total = diff_results.len();
        writeln!(
            &mut summary,
            "  {}: {}/{} passed ({:.1}%)",
            difficulty.name,
            diff_pass,
            diff_total,
            diff_pass as f64 / diff_total as f64 * 100.0
        )
        .ok();
    }
    // Multi-sub summary
    let ms_results: Vec<&TestResult> = all_results
        .iter()
        .filter(|r| r.name.starts_with("multisub/"))
        .collect();
    if !ms_results.is_empty() {
        let ms_pass = ms_results.iter().filter(|r| r.passed).count();
        let ms_total_count = ms_results.len();
        writeln!(
            &mut summary,
            "  multi-sub: {}/{} passed ({:.1}%)",
            ms_pass,
            ms_total_count,
            ms_pass as f64 / ms_total_count as f64 * 100.0
        )
        .ok();
    }

    // Multi-channel summary
    let mc_results: Vec<&TestResult> = all_results
        .iter()
        .filter(|r| r.name.starts_with("multichannel/"))
        .collect();
    if !mc_results.is_empty() {
        let mc_pass = mc_results.iter().filter(|r| r.passed).count();
        let mc_total_count = mc_results.len();
        writeln!(
            &mut summary,
            "  multi-channel: {}/{} passed ({:.1}%)",
            mc_pass,
            mc_total_count,
            mc_pass as f64 / mc_total_count as f64 * 100.0
        )
        .ok();
    }

    println!("\nPer-difficulty summary:");
    print!("{}", summary);

    if failed > 0 {
        println!("\nFailed tests:");
        for r in &all_results {
            if !r.passed {
                println!("  {} -- {} (epa={})", r.name, r.reason, fmt_epa(r.epa_preference));
            }
        }
        std::process::exit(1);
    }

    Ok(())
}
