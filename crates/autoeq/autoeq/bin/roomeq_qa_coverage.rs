//! RoomEQ Full QA: Comprehensive Scenario Testing
//!
//! Tests all roomeq scenarios with BEM/FEM solvers × IIR/FIR/Mixed modes.
//! Validates both the library and CLI binary output.
//!
//! Checks performed per test case:
//! 1. Minimum improvement threshold (varies by room size)
//! 2. Per-channel regression (no individual channel may get worse)
//! 3. Output sanity (filters exist, frequencies/gains valid)
//! 4. Absolute score ceiling (post_score below maximum for room size)
//!
//! Usage:
//!   cargo run --bin roomeq-qa-full --release              # run all tests
//!   cargo run --bin roomeq-qa-full --release -- --quick    # fast subset
//!   cargo run --bin roomeq-qa-full --release -- --list     # list scenarios
//!   cargo run --bin roomeq-qa-full --release -- --matrix    # show test matrix
//!   cargo run --bin roomeq-qa-full --release -- --junit     # JUnit XML output

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use clap::Parser;

use autoeq::roomeq::{
    merge_json_objects, optimize_room, CallbackAction, ProcessingMode, RoomConfig,
    RoomOptimizationResult,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SAMPLE_RATE: f64 = 48000.0;
const SEED: u64 = 42;

const QA_MAXEVAL: usize = 500; // Fast mode for QA

const FEM_DIR: &str = "data_tests/roomeq/generated/fem";
const BEM_DIR: &str = "data_tests/roomeq/generated/bem";
const OPTIM_CONFIG_DIR: &str = "data_tests/roomeq/generated/optimiser-config";

// ---------------------------------------------------------------------------
// Room Size Classification & Thresholds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum RoomSize {
    Small,
    Medium,
    Large,
}

impl RoomSize {
    /// Determine room size from scenario name prefix.
    fn from_scenario(scenario: &str) -> RoomSize {
        if scenario.starts_with("small_") {
            RoomSize::Small
        } else if scenario.starts_with("medium_") {
            RoomSize::Medium
        } else if scenario.starts_with("large_") {
            RoomSize::Large
        } else {
            panic!("Unknown room size prefix in scenario: {}", scenario);
        }
    }

    /// Minimum improvement percentage required for this room size.
    /// Small rooms have severe room modes so EQ should help substantially.
    /// Large rooms are dominated by direct sound, so less improvement is expected.
    fn min_improvement_pct(&self) -> f64 {
        match self {
            RoomSize::Small => 15.0,
            RoomSize::Medium => 12.0,
            RoomSize::Large => 8.0,
        }
    }

    /// Maximum acceptable post-optimization score.
    /// Ensures the system achieves a usable level of correction, not just "better than before".
    fn max_post_score(&self) -> f64 {
        match self {
            RoomSize::Small => 8.0,
            RoomSize::Medium => 10.0,
            RoomSize::Large => 12.0,
        }
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "roomeq-qa-full")]
#[command(about = "Comprehensive RoomEQ QA with full scenario matrix")]
struct Args {
    /// Run only fast subset (small rooms, FEM only, IIR only)
    #[arg(long)]
    quick: bool,

    /// List all available scenarios
    #[arg(long)]
    list: bool,

    /// Show test matrix without running
    #[arg(long)]
    matrix: bool,

    /// Output JUnit XML to file
    #[arg(long)]
    junit: Option<PathBuf>,

    /// Filter by scenario name (substring match)
    #[arg(long)]
    scenario: Option<String>,

    /// Filter by solver (bem, fem, or both)
    #[arg(long)]
    solver: Option<String>,

    /// Filter by mode (iir, fir, mixed, or all)
    #[arg(long)]
    mode: Option<String>,

    /// Number of parallel jobs (default: num CPUs)
    #[arg(long)]
    jobs: Option<usize>,

    /// Maximum evaluations per optimization (default: 500)
    #[arg(long)]
    maxeval: Option<usize>,

    /// Fail if any test fails (default: true, use --no-fail to disable)
    #[arg(long = "fail", default_value = "true")]
    fail: bool,
}

impl Args {
    fn maxeval(&self) -> usize {
        self.maxeval.unwrap_or(QA_MAXEVAL)
    }

    fn jobs(&self) -> usize {
        self.jobs.unwrap_or(num_cpus())
    }
}

// ---------------------------------------------------------------------------
// Test Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Solver {
    Fem,
    Bem,
}

impl Solver {
    fn name(&self) -> &'static str {
        match self {
            Solver::Fem => "fem",
            Solver::Bem => "bem",
        }
    }

    fn dir(&self) -> &'static str {
        match self {
            Solver::Fem => FEM_DIR,
            Solver::Bem => BEM_DIR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProcessingMethod {
    Iir,
    Fir,
    Mixed,
}

impl ProcessingMethod {
    fn name(&self) -> &'static str {
        match self {
            ProcessingMethod::Iir => "iir",
            ProcessingMethod::Fir => "fir",
            ProcessingMethod::Mixed => "mixed",
        }
    }

    fn mode(&self) -> ProcessingMode {
        match self {
            ProcessingMethod::Iir => ProcessingMode::LowLatency,
            ProcessingMethod::Fir => ProcessingMode::PhaseLinear,
            ProcessingMethod::Mixed => ProcessingMode::Hybrid,
        }
    }

    fn config_file(&self) -> &'static str {
        match self {
            ProcessingMethod::Iir => "optimiser-iir.json",
            ProcessingMethod::Fir => "optimiser-fir.json",
            ProcessingMethod::Mixed => "optimiser-mixed.json",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestCase {
    scenario: String,
    description: String,
    solver: Solver,
    method: ProcessingMethod,
}

impl TestCase {
    fn name(&self) -> String {
        format!(
            "{} {} {}",
            self.scenario,
            self.solver.name(),
            self.method.name()
        )
    }

    fn config_path(&self) -> PathBuf {
        let base = self.solver.dir();
        PathBuf::from(base).join(&self.scenario).join("config.json")
    }

    fn override_path(&self) -> PathBuf {
        let optim_dir = PathBuf::from(OPTIM_CONFIG_DIR);
        optim_dir
            .join(&self.scenario)
            .join(self.method.config_file())
    }

    fn room_size(&self) -> RoomSize {
        RoomSize::from_scenario(&self.scenario)
    }
}

// ---------------------------------------------------------------------------
// All Test Cases
// ---------------------------------------------------------------------------

fn all_scenarios() -> Vec<&'static str> {
    vec![
        // Small room
        "small_stereo_2_0",
        "small_stereo_2_1",
        "small_stereo_2_2_mso",
        "small_stereo_2_2_cardioid",
        "small_stereo_2_2_group",
        // Medium room
        "medium_stereo_2_0",
        "medium_stereo_2_1",
        "medium_multi_sub_4",
        "medium_multi_seat",
        // Large room
        "large_stereo_2_0",
        "large_stereo_2_1",
        "large_multi_sub_4",
        "large_multi_seat_2_1",
        // Medium room surround
        "medium_surround_5_0",
        "medium_surround_5_1",
        "medium_surround_5_1_4",
        // Large room surround
        "large_surround_5_1",
        "large_surround_5_1_4",
    ]
}

fn scenario_description(name: &str) -> String {
    match name {
        "small_stereo_2_0" => "Small 3x3x2.4m, stereo 2.0, fullrange".to_string(),
        "small_stereo_2_1" => "Small 3x3x2.4m, 2.1, sub at front-left".to_string(),
        "small_stereo_2_2_mso" => "Small 3x3x2.4m, 2 subs corners (MSO)".to_string(),
        "small_stereo_2_2_cardioid" => "Small 3x3x2.4m, stacked cardioid subs".to_string(),
        "small_stereo_2_2_group" => "Small 3x3x2.4m, grouped subs below mains".to_string(),
        "medium_stereo_2_0" => "Medium 5x4x2.5m, stereo 2.0, fullrange".to_string(),
        "medium_stereo_2_1" => "Medium 5x4x2.5m, 2.1".to_string(),
        "medium_multi_sub_4" => "Medium 5x4x2.5m, 4 corner subs".to_string(),
        "medium_multi_seat" => "Medium 5x4x2.5m, stereo, 3 seats".to_string(),
        "large_stereo_2_0" => "Large 7x5.5x2.6m, stereo 2.0, fullrange".to_string(),
        "large_stereo_2_1" => "Large 7x5.5x2.6m, 2.1".to_string(),
        "large_multi_sub_4" => "Large 7x5.5x2.6m, 4 corner subs".to_string(),
        "large_multi_seat_2_1" => "Large 7x5.5x2.6m, 2.1, 3 seats".to_string(),
        "medium_surround_5_0" => "Medium 5x4x2.5m, 5.0 surround, fullrange".to_string(),
        "medium_surround_5_1" => "Medium 5x4x2.5m, 5.1 surround".to_string(),
        "medium_surround_5_1_4" => "Medium 5x4x2.5m, 5.1.4 Dolby Atmos".to_string(),
        "large_surround_5_1" => "Large 7x5.5x2.6m, 5.1 surround".to_string(),
        "large_surround_5_1_4" => "Large 7x5.5x2.6m, 5.1.4 Dolby Atmos".to_string(),
        _ => name.to_string(),
    }
}

fn build_test_matrix(
    quick: bool,
    solver_filter: Option<&str>,
    mode_filter: Option<&str>,
) -> Vec<TestCase> {
    let solvers: Vec<Solver> = if quick {
        vec![Solver::Fem]
    } else {
        vec![Solver::Fem, Solver::Bem]
    };

    let methods: Vec<ProcessingMethod> = if quick {
        vec![ProcessingMethod::Iir]
    } else {
        vec![
            ProcessingMethod::Iir,
            ProcessingMethod::Fir,
            ProcessingMethod::Mixed,
        ]
    };

    let scenarios: Vec<&str> = if quick {
        vec!["small_stereo_2_0", "small_stereo_2_1", "medium_stereo_2_0"]
    } else {
        all_scenarios()
    };

    let mut test_cases = Vec::new();

    for scenario in scenarios {
        for solver in &solvers {
            // Skip BEM for multi-seat (too slow for now)
            if solver == &Solver::Bem && scenario.contains("multi_seat") {
                continue;
            }

            // Apply solver filter
            if let Some(f) = solver_filter
                && solver.name() != f
                && (f != "both" || (solver.name() != "fem" && solver.name() != "bem"))
            {
                continue;
            }

            for method in &methods {
                // Apply mode filter
                if let Some(f) = mode_filter
                    && method.name() != f && f != "all"
                {
                    continue;
                }

                test_cases.push(TestCase {
                    scenario: scenario.to_string(),
                    description: scenario_description(scenario),
                    solver: *solver,
                    method: *method,
                });
            }
        }
    }

    test_cases
}

fn print_matrix(test_cases: &[TestCase]) {
    println!("Test Matrix ({} cases):\n", test_cases.len());
    println!("{:<30} {:>6} {:>8}", "Scenario", "Solver", "Mode");
    println!("{:-<30} {:-<6} {:-<8}", "", "", "");

    for tc in test_cases {
        println!(
            "{:<30} {:>6} {:>8}",
            tc.scenario,
            tc.solver.name(),
            tc.method.name()
        );
    }
}

// ---------------------------------------------------------------------------
// Config Loading
// ---------------------------------------------------------------------------

fn load_config_for_test(tc: &TestCase) -> Result<(RoomConfig, PathBuf)> {
    let config_path = tc.config_path();
    let override_path = tc.override_path();

    let config_json = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config: {:?}", config_path))?;

    let mut config_value: serde_json::Value =
        serde_json::from_str(&config_json).with_context(|| "Failed to parse config JSON")?;

    if override_path.exists() {
        let override_json = std::fs::read_to_string(&override_path)
            .with_context(|| format!("Failed to read override: {:?}", override_path))?;
        let override_value: serde_json::Value = serde_json::from_str(&override_json)
            .with_context(|| "Failed to parse override JSON")?;
        merge_json_objects(&mut config_value, &override_value);
    }

    let config_dir = config_path.parent().unwrap_or(Path::new(".")).to_path_buf();

    let mut room_config: RoomConfig =
        serde_json::from_value(config_value).with_context(|| "Failed to deserialize config")?;

    room_config.resolve_paths(&config_dir);
    room_config.optimizer.processing_mode = tc.method.mode();

    Ok((room_config, config_dir))
}

fn apply_qa_overrides(config: &mut RoomConfig, maxeval: usize) {
    config.optimizer.algorithm = "cobyla".to_string();
    config.optimizer.max_iter = maxeval;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(SEED);

    // Ensure FIR config exists when processing mode requires it
    match config.optimizer.processing_mode {
        ProcessingMode::PhaseLinear | ProcessingMode::Hybrid => {
            if config.optimizer.fir.is_none() {
                config.optimizer.fir = Some(autoeq::roomeq::FirConfig {
                    taps: 4096,
                    phase: "kirkeby".to_string(),
                    correct_excess_phase: false,
                    phase_smoothing: 0.167,
                });
            }
        }
        ProcessingMode::LowLatency => {}
    }
}

// ---------------------------------------------------------------------------
// Optimization Runner
// ---------------------------------------------------------------------------

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
// Result Validation
// ---------------------------------------------------------------------------

/// Validate the optimization result beyond just "post < pre".
/// Returns a list of failure reasons (empty = all checks passed).
fn validate_result(
    result: &RoomOptimizationResult,
    room_size: RoomSize,
    method: ProcessingMethod,
) -> Vec<String> {
    let mut failures = Vec::new();

    let pre = result.combined_pre_score;
    let post = result.combined_post_score;

    // Check 1: post must be better than pre
    if post >= pre {
        failures.push(format!(
            "no improvement: post {:.4} >= pre {:.4}",
            post, pre
        ));
        return failures; // remaining checks meaningless if no improvement at all
    }

    // Check 2: minimum improvement threshold
    let improvement_pct = (1.0 - post / pre) * 100.0;
    let min_improvement = room_size.min_improvement_pct();
    if improvement_pct < min_improvement {
        failures.push(format!(
            "insufficient improvement: {:.1}% < {:.1}% minimum for {:?} room",
            improvement_pct, min_improvement, room_size
        ));
    }

    // Check 3: absolute score ceiling
    let max_post = room_size.max_post_score();
    if post > max_post {
        failures.push(format!(
            "post_score {:.4} exceeds maximum {:.1} for {:?} room",
            post, max_post, room_size
        ));
    }

    // Check 4: per-channel regression (strictly worse, not equal)
    for (name, ch_result) in &result.channel_results {
        if ch_result.post_score > ch_result.pre_score {
            failures.push(format!(
                "channel '{}' regressed: {:.4} -> {:.4}",
                name, ch_result.pre_score, ch_result.post_score
            ));
        }
    }

    // Check 5: output sanity — filters must exist and be valid
    // Only require filters when the channel actually improved (pre > post).
    // When pre == post, the optimizer found no beneficial EQ (e.g., cardioid sub),
    // so missing filters is expected, not an error.
    for (name, ch_result) in &result.channel_results {
        let improved = ch_result.post_score < ch_result.pre_score;
        let has_biquads = !ch_result.biquads.is_empty();
        let has_fir = ch_result
            .fir_coeffs
            .as_ref()
            .is_some_and(|c| !c.is_empty());

        match method {
            ProcessingMethod::Iir => {
                if improved && !has_biquads {
                    failures.push(format!("channel '{}': IIR mode but no biquad filters", name));
                }
            }
            ProcessingMethod::Fir => {
                if improved && !has_fir {
                    failures.push(format!("channel '{}': FIR mode but no FIR coefficients", name));
                }
            }
            ProcessingMethod::Mixed => {
                if improved && !has_biquads && !has_fir {
                    failures.push(format!(
                        "channel '{}': Mixed mode but no filters at all",
                        name
                    ));
                }
            }
        }

        // Validate biquad filter parameters
        for (i, bq) in ch_result.biquads.iter().enumerate() {
            if bq.freq < 10.0 || bq.freq > 24000.0 {
                failures.push(format!(
                    "channel '{}' filter {}: frequency {:.1} Hz out of range [10, 24000]",
                    name, i, bq.freq
                ));
            }
            if bq.db_gain.abs() < 0.05 {
                failures.push(format!(
                    "channel '{}' filter {}: near-zero gain {:.3} dB (useless filter)",
                    name, i, bq.db_gain
                ));
            }
        }
    }

    failures
}

// ---------------------------------------------------------------------------
// Test Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestResult {
    name: String,
    scenario: String,
    solver: String,
    method: String,
    pre_score: f64,
    post_score: f64,
    passed: bool,
    error: Option<String>,
    duration_ms: u64,
}

impl TestResult {
    fn success(
        name: &str,
        scenario: &str,
        solver: &str,
        method: &str,
        pre: f64,
        post: f64,
        validation_failures: Vec<String>,
        dur: u64,
    ) -> Self {
        let passed = validation_failures.is_empty();
        let error = if validation_failures.is_empty() {
            None
        } else {
            Some(validation_failures.join("; "))
        };
        Self {
            name: name.to_string(),
            scenario: scenario.to_string(),
            solver: solver.to_string(),
            method: method.to_string(),
            pre_score: pre,
            post_score: post,
            passed,
            error,
            duration_ms: dur,
        }
    }

    fn failure(
        name: &str,
        scenario: &str,
        solver: &str,
        method: &str,
        err: String,
        dur: u64,
    ) -> Self {
        Self {
            name: name.to_string(),
            scenario: scenario.to_string(),
            solver: solver.to_string(),
            method: method.to_string(),
            pre_score: 0.0,
            post_score: 0.0,
            passed: false,
            error: Some(err),
            duration_ms: dur,
        }
    }

    fn junit_xml(&self) -> String {
        let mut xml = format!(
            r#"    <testcase name="{}" classname="roomeq.{}" time="{}">"#,
            self.name,
            self.scenario,
            self.duration_ms as f64 / 1000.0
        );

        if !self.passed {
            let msg = if let Some(ref err) = self.error {
                err.clone()
            } else {
                format!(
                    "post_score {:.4} >= pre_score {:.4}",
                    self.post_score, self.pre_score
                )
            };
            let escaped = msg
                .replace('"', "&quot;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            xml.push_str(&format!(
                "\n      <failure message=\"{}\" type=\"AssertionError\"/>",
                escaped
            ));
        }

        xml.push_str("\n    </testcase>");
        xml
    }
}

// ---------------------------------------------------------------------------
// Test Runner
// ---------------------------------------------------------------------------

fn run_test_case(tc: &TestCase, maxeval: usize) -> TestResult {
    let start = std::time::Instant::now();

    let name = tc.name();
    let scenario = tc.scenario.clone();
    let solver = tc.solver.name().to_string();
    let method = tc.method.name().to_string();

    // Check if config exists
    if !tc.config_path().exists() {
        let err = format!("Config not found: {:?}", tc.config_path());
        return TestResult::failure(
            &name,
            &scenario,
            &solver,
            &method,
            err,
            start.elapsed().as_millis() as u64,
        );
    }

    // Load and configure
    let mut config = match load_config_for_test(tc) {
        Ok((c, _)) => c,
        Err(e) => {
            return TestResult::failure(
                &name,
                &scenario,
                &solver,
                &method,
                format!("{:#}", e),
                start.elapsed().as_millis() as u64,
            );
        }
    };

    apply_qa_overrides(&mut config, maxeval);

    // Run optimization
    let result = match run_optimization(&config) {
        Ok(r) => r,
        Err(e) => {
            return TestResult::failure(
                &name,
                &scenario,
                &solver,
                &method,
                format!("{:#}", e),
                start.elapsed().as_millis() as u64,
            );
        }
    };

    let pre = result.combined_pre_score;
    let post = result.combined_post_score;
    let dur = start.elapsed().as_millis() as u64;

    let validation_failures = validate_result(&result, tc.room_size(), tc.method);

    TestResult::success(&name, &scenario, &solver, &method, pre, post, validation_failures, dur)
}

// ---------------------------------------------------------------------------
// Parallel Execution
// ---------------------------------------------------------------------------

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}

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

fn run_parallel(test_cases: Vec<TestCase>, maxeval: usize, num_jobs: usize) -> Vec<TestResult> {
    let (tx, rx) = channel::<TestResult>();
    let semaphore = Arc::new(CountingSemaphore::new(num_jobs));
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    for tc in test_cases {
        let tx = tx.clone();
        let sem = Arc::clone(&semaphore);

        let handle = thread::spawn(move || {
            sem.acquire();
            let result = run_test_case(&tc, maxeval);
            sem.release();
            let _ = tx.send(result);
        });
        handles.push(handle);
    }

    drop(tx);

    let mut results = Vec::new();
    while let Ok(result) = rx.recv() {
        results.push(result);
    }

    for handle in handles {
        let _ = handle.join();
    }

    results
}

// ---------------------------------------------------------------------------
// JUnit XML Output
// ---------------------------------------------------------------------------

fn write_junit_xml(results: &[TestResult], output: &Path) -> Result<()> {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str("\n<testsuite name=\"roomeq-qa-full\" ");
    xml.push_str(&format!("tests=\"{}\" ", results.len()));
    xml.push_str(&format!(
        "failures=\"{}\" ",
        results.iter().filter(|r| !r.passed).count()
    ));
    xml.push_str(&format!(
        "errors=\"{}\" ",
        results.iter().filter(|r| r.error.is_some()).count()
    ));
    xml.push_str(&format!(
        "time=\"{}\" ",
        results.iter().map(|r| r.duration_ms).sum::<u64>() as f64 / 1000.0
    ));
    xml.push_str(">\n");

    for result in results {
        xml.push_str(&result.junit_xml());
        xml.push('\n');
    }

    xml.push_str("</testsuite>\n");

    std::fs::write(output, xml)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args = Args::parse();
    let project_root = find_project_root()?;
    std::env::set_current_dir(&project_root)?;

    // List scenarios
    if args.list {
        println!("Available scenarios:");
        for s in all_scenarios() {
            println!("  {}: {}", s, scenario_description(s));
        }
        return Ok(());
    }

    // Build test matrix
    let test_cases = build_test_matrix(args.quick, args.solver.as_deref(), args.mode.as_deref());

    // Show matrix
    if args.matrix {
        print_matrix(&test_cases);
        return Ok(());
    }

    // Apply scenario filter
    let test_cases: Vec<TestCase> = if let Some(ref filter) = args.scenario {
        let filter_lower = filter.to_lowercase();
        test_cases
            .into_iter()
            .filter(|tc| tc.scenario.to_lowercase().contains(&filter_lower))
            .collect()
    } else {
        test_cases
    };

    if test_cases.is_empty() {
        println!("No test cases to run.");
        return Ok(());
    }

    println!("=== RoomEQ Full QA ===");
    println!(
        "Running {} test cases with {} parallel jobs",
        test_cases.len(),
        args.jobs()
    );
    if args.quick {
        println!("QUICK MODE: Small rooms, FEM only, IIR only");
    }
    println!();

    // Run tests
    let results = run_parallel(test_cases, args.maxeval(), args.jobs());

    // Print results
    let mut passed = 0;
    let mut failed = 0;

    for result in &results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        if result.passed {
            passed += 1;
            println!(
                "[{}] {} ({}ms): {:.4} -> {:.4} ({:.1}% improvement)",
                status,
                result.name,
                result.duration_ms,
                result.pre_score,
                result.post_score,
                (1.0 - result.post_score / result.pre_score.max(0.001)) * 100.0
            );
        } else {
            failed += 1;
            if let Some(ref err) = result.error {
                eprintln!("[{}] {}: {}", status, result.name, err);
            } else {
                eprintln!(
                    "[{}] {}: pre={:.4}, post={:.4}",
                    status, result.name, result.pre_score, result.post_score
                );
            }
        }
    }

    // Summary
    println!("\n=== Summary: {}/{} PASS ===", passed, passed + failed);
    println!(
        "Total time: {}ms",
        results.iter().map(|r| r.duration_ms).sum::<u64>()
    );

    // JUnit output
    if let Some(ref junit_path) = args.junit {
        write_junit_xml(&results, junit_path)?;
        println!("JUnit XML written to: {}", junit_path.display());
    }

    // Exit code
    if args.fail && failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

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
            return Err(anyhow!("Could not find project root"));
        }
    }
}
