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
use autoeq::roomeq::{
    BroadbandTargetMatchingConfig, CallbackAction, ExcursionProtectionConfig, ProcessingMode,
    RoomConfig, SchroederSplitConfig, optimize_room,
};
use autoeq::roomeq::synthetic::{generate_flat_curve, generate_harman_tilt_curve, generate_scenario};
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
    modes: &[
        (80.0, 2.0, -3.0),
        (150.0, 2.0, 3.0),
        (250.0, 2.0, -2.0),
    ],
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
    config.optimizer.broadband_target_matching = Some(BroadbandTargetMatchingConfig {
        enabled: true,
    });
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

const OPTIONS: &[OptionDef] = &[
    OptionDef { name: "psychoacoustic", apply: option_psychoacoustic },
    OptionDef { name: "asymmetric_loss", apply: option_asymmetric },
    OptionDef { name: "broadband", apply: option_broadband },
    OptionDef { name: "excursion", apply: option_excursion },
    OptionDef { name: "schroeder", apply: option_schroeder },
];

// ---------------------------------------------------------------------------
// Config builder
// ---------------------------------------------------------------------------

fn build_config(
    degraded: &Curve,
    mode: ProcessingMode,
) -> RoomConfig {
    let mut speakers = HashMap::new();
    speakers.insert(
        "Left".to_string(),
        autoeq::roomeq::SpeakerConfig::Single(
            MeasurementSource::InMemory(degraded.clone()),
        ),
    );
    speakers.insert(
        "Right".to_string(),
        autoeq::roomeq::SpeakerConfig::Single(
            MeasurementSource::InMemory(degraded.clone()),
        ),
    );

    let mut config = RoomConfig {
        version: "2.0.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: Default::default(),
        recording_config: None,
    };

    config.optimizer.algorithm = "autoeq:de".to_string();
    config.optimizer.max_iter = QA_MAXEVAL;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(SEED);
    config.optimizer.processing_mode = mode;
    config.optimizer.num_filters = 5;
    config.optimizer.min_freq = 20.0;
    config.optimizer.max_freq = 20000.0;

    config
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

fn run_optimization(config: &RoomConfig) -> Result<autoeq::roomeq::RoomOptimizationResult> {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir().join(format!("roomeq_qa_syn_{}_{}", std::process::id(), id));
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
    reason: String,
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
                reason: format!("Optimization failed: {}", e),
            };
        }
    };

    // Validation: post_score < pre_score (optimization should improve)
    let pre = result.combined_pre_score;
    let post = result.combined_post_score;

    if post >= pre {
        return TestResult {
            name: test_name,
            passed: false,
            pre_score: pre,
            post_score: post,
            reason: format!("No improvement: pre={:.3}, post={:.3}", pre, post),
        };
    }

    TestResult {
        name: test_name,
        passed: true,
        pre_score: pre,
        post_score: post,
        reason: format!(
            "OK: {:.3} -> {:.3} ({:.1}% reduction)",
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
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn"),
    )
    .init();

    let args: Vec<String> = std::env::args().collect();
    let list_only = args.iter().any(|a| a == "--list");
    let difficulty_filter = args.windows(2).find(|w| w[0] == "--difficulty").map(|w| w[1].clone());

    let difficulties: Vec<&DifficultyLevel> = if let Some(ref filter) = difficulty_filter {
        ALL_DIFFICULTIES.iter().filter(|d| d.name == filter.as_str()).collect()
    } else {
        ALL_DIFFICULTIES.iter().collect()
    };

    let modes = [
        ProcessingMode::LowLatency,
        ProcessingMode::PhaseLinear,
        ProcessingMode::Hybrid,
    ];

    let flat_target = generate_flat_curve(20.0, 20000.0, 200);
    let harman_target = generate_harman_tilt_curve(20.0, 20000.0, 200);
    let targets: Vec<(&str, &Curve)> = vec![("flat", &flat_target), ("harman", &harman_target)];

    let option_combos = generate_option_combos();

    // Count total tests
    let total = difficulties.len() * modes.len() * targets.len() * option_combos.len();

    if list_only {
        println!("Synthetic QA Test Matrix:");
        println!("  Difficulties: {}", difficulties.iter().map(|d| d.name).collect::<Vec<_>>().join(", "));
        println!("  Modes: IIR, FIR, Mixed");
        println!("  Targets: flat, harman");
        println!("  Option combos: {} (baseline + {} singles + {} pairs + 1 all)",
            option_combos.len(),
            OPTIONS.len(),
            OPTIONS.len() * (OPTIONS.len() - 1) / 2,
        );
        println!("  Total tests: {}", total);
        return Ok(());
    }

    println!("RoomEQ Synthetic QA -- {} tests", total);
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
            let scenario = generate_scenario(
                &format!("{}/{}", difficulty.name, target_name),
                target,
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
                        println!("  FAIL: {} -- {}", result.name, result.reason);
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
    println!("\nPer-difficulty summary:");
    print!("{}", summary);

    if failed > 0 {
        println!("\nFailed tests:");
        for r in &all_results {
            if !r.passed {
                println!("  {} -- {}", r.name, r.reason);
            }
        }
        std::process::exit(1);
    }

    Ok(())
}
