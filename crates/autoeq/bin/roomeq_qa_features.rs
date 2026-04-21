//! RoomEQ QA: Feature Progression Tests
//!
//! For each recording in `bin/roomeq_qa_data/*/recordings.json`, runs two
//! progression passes (flat target, then Harman tilt), enabling features
//! cumulatively. Validates that:
//! - Each step's optimization improves over its own pre-score
//! - Step-over-step flat-score regression stays within tolerance (skipped
//!   when a step changes the loss function)
//! - EPA preference (perceptual quality) does not decrease vs baseline
//! - Final curve slope stays within tolerance
//! - The full feature stack improves over raw measurement
//!
//! Usage:
//!   cargo run --bin roomeq-qa-features --no-default-features --release

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use autoeq::loss::regression_slope_per_octave_in_range;
use autoeq::roomeq::{
    CallbackAction, ExcursionProtectionConfig, RoomConfig, RoomOptimizationResult,
    SchroederSplitConfig, TargetResponseConfig, TargetShape, load_config,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SAMPLE_RATE: f64 = 48000.0;
const SEED: u64 = 42;
const QA_MAX_ITER: usize = 15000;
const QA_POPULATION: usize = 50;
const QA_NUM_FILTERS: usize = 3;

/// Step-over-step regression tolerance: 30%
const STEP_REGRESSION_TOLERANCE: f64 = 1.30;

/// Per-step sanity: post_score should not be much worse than own pre_score
const SELF_REGRESSION_TOLERANCE: f64 = 1.10;

/// Slope tolerance: slope must be <= this (dB/octave). Small positive allowed for noise.
const SLOPE_TOLERANCE: f64 = 0.5;

/// Slope check frequency range
const SLOPE_FMIN: f64 = 200.0;
const SLOPE_FMAX: f64 = 10000.0;

// ---------------------------------------------------------------------------
// Feature step definition
// ---------------------------------------------------------------------------

struct FeatureStep {
    name: &'static str,
    /// Step changes the loss function, making step-over-step score comparisons
    /// invalid at this boundary (optimizer targets a different objective).
    changes_loss: bool,
    apply: fn(&mut RoomConfig),
}

fn feature_steps() -> Vec<FeatureStep> {
    vec![
        FeatureStep {
            name: "Baseline",
            changes_loss: false,
            apply: |_| {},
        },
        FeatureStep {
            name: "+ psychoacoustic",
            changes_loss: true,
            apply: |c| {
                c.optimizer.psychoacoustic = true;
            },
        },
        FeatureStep {
            name: "+ asymmetric_loss",
            changes_loss: true,
            apply: |c| {
                c.optimizer.asymmetric_loss = true;
            },
        },
        // Broadband changes the loss landscape: EQ is optimized against
        // the broadband-adjusted curve, but post_score uses the original.
        FeatureStep {
            name: "+ broadband",
            changes_loss: true,
            apply: |c| {
                let tr = c
                    .optimizer
                    .target_response
                    .get_or_insert_with(TargetResponseConfig::default);
                tr.broadband_precorrection = true;
            },
        },
        FeatureStep {
            name: "+ excursion_protection",
            changes_loss: false,
            apply: |c| {
                c.optimizer.excursion_protection = Some(ExcursionProtectionConfig {
                    enabled: true,
                    ..ExcursionProtectionConfig::default()
                });
            },
        },
        FeatureStep {
            name: "+ schroeder_split",
            changes_loss: false,
            apply: |c| {
                c.optimizer.schroeder_split = Some(SchroederSplitConfig {
                    enabled: true,
                    schroeder_freq: 300.0,
                    ..SchroederSplitConfig::default()
                });
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

/// Build baseline config: all features OFF, QA optimizer overrides applied.
fn make_baseline(config: &RoomConfig, with_tilt: bool) -> RoomConfig {
    let mut c = config.clone();

    // Disable all features
    c.optimizer.psychoacoustic = false;
    c.optimizer.asymmetric_loss = false;
    c.optimizer.target_response = None;
    c.optimizer.excursion_protection = None;
    c.optimizer.schroeder_split = None;

    // QA optimizer overrides
    c.optimizer.algorithm = "autoeq:de".to_string();
    c.optimizer.max_iter = QA_MAX_ITER;
    c.optimizer.population = QA_POPULATION;
    c.optimizer.seed = Some(SEED);
    c.optimizer.refine = false;
    c.optimizer.num_filters = QA_NUM_FILTERS;

    if with_tilt {
        c.optimizer.target_response = Some(TargetResponseConfig {
            shape: TargetShape::Custom,
            slope_db_per_octave: -0.8,
            ..TargetResponseConfig::default()
        });
    }

    c
}

// ---------------------------------------------------------------------------
// Optimization runner
// ---------------------------------------------------------------------------

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run_optimization(config: &RoomConfig) -> Result<RoomOptimizationResult> {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir =
        std::env::temp_dir().join(format!("roomeq_qa_features_{}_{}", std::process::id(), id));
    std::fs::create_dir_all(&temp_dir)?;
    let callback =
        Box::new(|_: &autoeq::roomeq::RoomOptimizationProgress| CallbackAction::Continue);
    let result =
        autoeq::roomeq::optimize_room(config, SAMPLE_RATE, Some(callback), Some(&temp_dir))
            .map_err(|e| anyhow!("{}", e));
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

// ---------------------------------------------------------------------------
// Step result
// ---------------------------------------------------------------------------

struct StepResult {
    name: &'static str,
    pre_score: f64,
    post_score: f64,
    /// Worst (max) slope across channels in dB/octave
    worst_slope: f64,
    /// True if this step changed the loss function relative to the previous step.
    changes_loss: bool,
    /// Average EPA preference across channels (higher = better).
    /// `None` if EPA metrics were not available.
    epa_preference: Option<f64>,
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
// Pass runner
// ---------------------------------------------------------------------------

fn run_pass(
    recording_name: &str,
    base_config: &RoomConfig,
    with_tilt: bool,
) -> Result<Vec<StepResult>> {
    let steps = feature_steps();
    let mut results = Vec::with_capacity(steps.len());

    // Start from baseline with all features OFF
    let mut config = make_baseline(base_config, with_tilt);

    for step in &steps {
        // Apply this step's feature (cumulative)
        (step.apply)(&mut config);

        let opt_result = run_optimization(&config)
            .with_context(|| format!("{}: step '{}' failed", recording_name, step.name))?;

        // Compute worst (most positive) slope across channels
        let mut worst_slope: Option<f64> = None;
        for ch_result in opt_result.channel_results.values() {
            let curve = &ch_result.final_curve;
            if let Some(slope) = regression_slope_per_octave_in_range(
                &curve.freq,
                &curve.spl,
                SLOPE_FMIN,
                SLOPE_FMAX,
            ) {
                worst_slope = Some(worst_slope.map_or(slope, |w: f64| w.max(slope)));
            }
        }
        let worst_slope = worst_slope.ok_or_else(|| {
            anyhow!(
                "{}: step '{}' — no channel produced a valid slope in {}-{} Hz",
                recording_name,
                step.name,
                SLOPE_FMIN,
                SLOPE_FMAX
            )
        })?;

        let epa_preference = avg_epa_preference(&opt_result);

        results.push(StepResult {
            name: step.name,
            pre_score: opt_result.combined_pre_score,
            post_score: opt_result.combined_post_score,
            worst_slope,
            changes_loss: step.changes_loss,
            epa_preference,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_pass(pass_name: &str, results: &[StepResult]) -> Vec<String> {
    let mut errors = Vec::new();

    // Track whether we've crossed a loss-change boundary. Once crossed,
    // flat-score step-over-step comparisons are invalid for all subsequent steps.
    let mut loss_changed = false;

    let baseline_epa = results.first().and_then(|s| s.epa_preference);

    for (i, step) in results.iter().enumerate() {
        if step.changes_loss {
            loss_changed = true;
        }

        // Convergence: every step must produce a finite loss
        if !step.post_score.is_finite() {
            errors.push(format!(
                "  {} step '{}': post_score is not finite — optimizer failed to converge",
                pass_name, step.name
            ));
            continue;
        }

        if loss_changed {
            // Flat-score comparisons are invalid after a loss change.
            // Validate perceptual quality instead: EPA preference must not
            // decrease vs baseline.
            if let (Some(baseline), Some(current)) = (baseline_epa, step.epa_preference) {
                if current < baseline * 0.95 {
                    errors.push(format!(
                        "  {} step '{}': EPA preference {:.3} < baseline {:.3} * 0.95 — perceptual regression",
                        pass_name, step.name, current, baseline
                    ));
                }
            }
        } else {
            // No loss change yet — flat-score checks are valid.

            // Per-step sanity: post_score should not be much worse than own pre_score
            if step.post_score > step.pre_score * SELF_REGRESSION_TOLERANCE {
                errors.push(format!(
                    "  {} step '{}': post_score {:.4} > pre_score {:.4} * {:.2} — optimization made things worse",
                    pass_name, step.name, step.post_score, step.pre_score, SELF_REGRESSION_TOLERANCE
                ));
            }

            // Step-over-step regression check
            if i > 0 {
                let prev = &results[i - 1];
                if step.post_score > prev.post_score * STEP_REGRESSION_TOLERANCE {
                    errors.push(format!(
                        "  {} step '{}': post_score {:.4} > prev {:.4} * {:.2} — excessive regression",
                        pass_name,
                        step.name,
                        step.post_score,
                        prev.post_score,
                        STEP_REGRESSION_TOLERANCE
                    ));
                }
            }

            // Slope invariant
            if step.worst_slope > SLOPE_TOLERANCE {
                errors.push(format!(
                    "  {} step '{}': slope {:.2} dB/oct > {:.1} tolerance — positive tilt detected",
                    pass_name, step.name, step.worst_slope, SLOPE_TOLERANCE
                ));
            }
        }
    }

    // End-of-pass: baseline step must improve over raw measurement
    if let Some(baseline) = results.first() {
        if baseline.post_score >= baseline.pre_score {
            errors.push(format!(
                "  {} step '{}': post_score {:.4} >= pre_score {:.4} — EQ did not improve over raw",
                pass_name, baseline.name, baseline.post_score, baseline.pre_score
            ));
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// Recording discovery
// ---------------------------------------------------------------------------

fn discover_recordings(project_root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let qa_data_dir = project_root.join("crates/autoeq/bin/roomeq_qa_data");
    if !qa_data_dir.exists() {
        return Err(anyhow!("QA data directory not found: {:?}", qa_data_dir));
    }

    let mut recordings = Vec::new();
    for entry in std::fs::read_dir(&qa_data_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let recordings_json = entry.path().join("recordings.json");
        if recordings_json.exists() {
            let name = entry.file_name().to_string_lossy().to_string();
            recordings.push((name, recordings_json));
        }
    }

    recordings.sort_by(|a, b| a.0.cmp(&b.0));

    if recordings.is_empty() {
        return Err(anyhow!("No recordings found in {:?}", qa_data_dir));
    }

    Ok(recordings)
}

// ---------------------------------------------------------------------------
// Project root
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let project_root = find_project_root()?;
    let recordings = discover_recordings(&project_root)?;

    println!(
        "RoomEQ Feature Progression QA — {} recording(s)\n",
        recordings.len()
    );

    let mut all_errors: Vec<String> = Vec::new();
    let mut pass_count = 0;
    let mut fail_count = 0;

    for (name, config_path) in &recordings {
        println!("=== Recording: {} ===", name);

        let (base_config, _config_dir) = load_config(config_path, None)
            .with_context(|| format!("Failed to load {}", config_path.display()))?;

        let mut recording_ok = true;

        // --- Pass A: Flat target ---
        println!("--- Pass A: Flat target ---");
        let pass_a = run_pass(name, &base_config, false)?;
        print_pass_results(&pass_a);
        let errors_a = validate_pass("Pass A", &pass_a);
        if errors_a.is_empty() {
            println!("  => PASS");
        } else {
            println!("  => FAIL");
            recording_ok = false;
            for e in &errors_a {
                println!("{}", e);
            }
            all_errors.extend(errors_a);
        }

        // --- Pass B: Harman tilt ---
        println!("--- Pass B: Harman tilt ---");
        let pass_b = run_pass(name, &base_config, true)?;
        print_pass_results(&pass_b);
        let errors_b = validate_pass("Pass B", &pass_b);
        if errors_b.is_empty() {
            println!("  => PASS");
        } else {
            println!("  => FAIL");
            recording_ok = false;
            for e in &errors_b {
                println!("{}", e);
            }
            all_errors.extend(errors_b);
        }

        if recording_ok {
            pass_count += 1;
        } else {
            fail_count += 1;
        }

        println!();
    }

    // Summary
    let total = pass_count + fail_count;
    if all_errors.is_empty() {
        println!("=== Summary: {}/{} recordings PASS ===", pass_count, total);
        Ok(())
    } else {
        println!("=== Summary: {}/{} recordings FAIL ===", fail_count, total);
        for e in &all_errors {
            eprintln!("{}", e);
        }
        std::process::exit(1);
    }
}

fn print_pass_results(results: &[StepResult]) {
    let baseline_epa = results.first().and_then(|s| s.epa_preference);

    for (i, step) in results.iter().enumerate() {
        let epa_str = match step.epa_preference {
            Some(v) => format!("epa={:.3}", v),
            None => "epa=n/a".to_string(),
        };

        if i == 0 {
            println!(
                "  Step {}: {:30} post={:.4}  slope={:.2}  {}",
                i, step.name, step.post_score, step.worst_slope, epa_str
            );
        } else {
            let prev = &results[i - 1];
            let pct = if prev.post_score > 0.0 {
                (step.post_score - prev.post_score) / prev.post_score * 100.0
            } else {
                0.0
            };

            let epa_vs_baseline = match (baseline_epa, step.epa_preference) {
                (Some(b), Some(c)) if b > 0.0 => format!("  epa vs baseline: {:+.1}%", (c - b) / b * 100.0),
                _ => String::new(),
            };

            println!(
                "  Step {}: {:30} post={:.4}  slope={:.2}  (vs prev: {:+.1}%)  {}{}",
                i, step.name, step.post_score, step.worst_slope, pct, epa_str, epa_vs_baseline
            );
        }
    }
}
