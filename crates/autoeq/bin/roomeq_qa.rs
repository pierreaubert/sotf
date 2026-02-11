//! RoomEQ QA: Convergence & Monotonicity Tests
//!
//! Validates that roomeq optimization modes produce converging results
//! and that giving the optimizer more resources (more filters, wider Q/dB bounds)
//! always improves or maintains loss.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

use autoeq::roomeq::{
    CallbackAction, ProcessingMode, RoomConfig, load_config, merge_json_objects, optimize_room,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Monotonicity tolerance: variation may be at most 15% worse than baseline.
/// Stochastic optimization with DE can produce slightly worse results when
/// expanding the search space (larger bounds = harder search). Even with
/// single-threaded rayon and fixed seed, NLopt refinement (COBYLA) can
/// introduce minor non-determinism.
const MONOTONICITY_TOLERANCE: f64 = 1.15;

/// Cross-mode ratio: max score / min score must be <= 5.0.
/// IIR (generic path, no stereo alignment) vs FIR/Mixed have fundamentally
/// different capabilities, so we allow wider divergence.
const CROSS_MODE_RATIO_LIMIT: f64 = 5.0;

const SAMPLE_RATE: f64 = 48000.0;

const SEED: u64 = 42;

/// Base config directories
const FEM_DIR: &str = "data_tests/roomeq/generated/fem";
const OPTIM_CONFIG_DIR: &str = "data_tests/roomeq/generated/optimiser-config";

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
    room_config.optimizer.seed = Some(SEED);

    Ok((room_config, config_dir))
}

// ---------------------------------------------------------------------------
// Optimization runner
// ---------------------------------------------------------------------------

fn run_optimization(config: &RoomConfig) -> Result<autoeq::roomeq::RoomOptimizationResult> {
    let temp_dir = std::env::temp_dir().join(format!("roomeq_qa_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;
    let callback = Box::new(|_: &autoeq::roomeq::RoomOptimizationProgress| {
        CallbackAction::Continue
    });
    let result = optimize_room(config, SAMPLE_RATE, Some(callback), Some(&temp_dir))
        .map_err(|e| anyhow!("{}", e));
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
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
// Test runners
// ---------------------------------------------------------------------------

fn run_stereo_workflow_tests(
    name: &str,
    base_config_path: &Path,
    override_config_path: Option<&Path>,
    results: &mut Vec<TestResult>,
) -> Result<()> {
    println!("\n--- {} (IIR workflow) ---", name);

    let mut baseline_post: Option<f64> = None;

    for mutation in IIR_MUTATIONS {
        let (mut config, _) = load_config(base_config_path, override_config_path)?;
        config.optimizer.seed = Some(SEED);
        apply_mutation(&mut config, *mutation);

        let result =
            run_optimization(&config).with_context(|| format!("{} IIR {}", name, mutation))?;

        let pre = result.combined_pre_score;
        let post = result.combined_post_score;

        let (pass, reason) = evaluate_result(*mutation, pre, post, &mut baseline_post);

        let status = if pass { "PASS" } else { "FAIL" };
        println!(
            "  IIR {:>14}: post={:.4}  {}  ({})",
            mutation.to_string(),
            post,
            status,
            reason
        );

        results.push(TestResult {
            label: format!("{} IIR {}", name, mutation),
            pre_score: pre,
            post_score: post,
            pass,
            reason,
        });
    }

    Ok(())
}

fn run_generic_path_tests(
    name: &str,
    base_config_path: &Path,
    override_config_dir: &Path,
    results: &mut Vec<TestResult>,
) -> Result<()> {
    println!("\n--- Generic Path ({}, all modes) ---", name);

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
            println!(
                "  {} {:>14}: post={:.4}  {}  ({})",
                mode_name,
                mutation.to_string(),
                post,
                status,
                reason
            );

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

        println!(
            "\n  Cross-mode: {} ratio={:.2}x  {}",
            mode_scores, ratio, status
        );

        results.push(TestResult {
            label: format!("{} cross-mode", name),
            pre_score: 0.0,
            post_score: 0.0,
            pass,
            reason: format!(
                "ratio={:.2}x (limit={:.1}x)",
                ratio, CROSS_MODE_RATIO_LIMIT
            ),
        });
    }

    Ok(())
}

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

    // Force single-threaded rayon to ensure deterministic results with seed=42.
    // Parallel DE evaluation is non-deterministic due to thread scheduling,
    // which causes flaky test results even with a fixed seed.
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .expect("Failed to initialize rayon thread pool with 1 thread");

    println!("=== RoomEQ QA: Convergence & Monotonicity ===");

    let project_root = find_project_root()?;
    let fem_dir = project_root.join(FEM_DIR);
    let optim_dir = project_root.join(OPTIM_CONFIG_DIR);

    let mut results: Vec<TestResult> = Vec::new();

    // Part A: Stereo workflows (IIR only)
    // 2.0: use IIR override config (optimizer-only overrides, works well)
    run_stereo_workflow_tests(
        "Stereo 2.0",
        &fem_dir.join("small_stereo_2_0/config.json"),
        Some(&optim_dir.join("small_stereo_2_0/optimiser-iir.json")),
        &mut results,
    )?;

    // 2.1: stereo with subwoofer and crossover optimization
    run_stereo_workflow_tests(
        "Stereo 2.1",
        &fem_dir.join("small_stereo_2_1/config.json"),
        Some(&optim_dir.join("small_stereo_2_1/optimiser-iir.json")),
        &mut results,
    )?;

    // 2.2: stereo with subwoofer and crossover optimization
    run_stereo_workflow_tests(
        "Stereo 2.2",
        &fem_dir.join("small_stereo_2_2/config.json"),
        Some(&optim_dir.join("small_stereo_2_2/optimiser-iir.json")),
        &mut results,
    )?;

    // Part B: Generic path (all 3 modes) — uses small_stereo_2_0 with system removed
    run_generic_path_tests(
        "small_stereo_2_0",
        &fem_dir.join("small_stereo_2_0/config.json"),
        &optim_dir.join("small_stereo_2_0"),
        &mut results,
    )?;

    // Summary
    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;

    println!("\n=== Summary: {}/{} PASS ===", passed, total);

    if failed > 0 {
        println!("\nFailed tests:");
        for r in &results {
            if !r.pass {
                println!(
                    "  - {} (pre={:.4}, post={:.4}): {}",
                    r.label, r.pre_score, r.post_score, r.reason
                );
            }
        }
        std::process::exit(1);
    }

    Ok(())
}

/// Find the project root by looking for Cargo.toml with [workspace]
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
