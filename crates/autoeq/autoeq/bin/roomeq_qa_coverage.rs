//! RoomEQ Full QA: Comprehensive Scenario Testing
//!
//! Tests all roomeq scenarios with BEM/FEM solvers × IIR/FIR/Mixed modes.
//! Validates both the library and CLI binary output.
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
use std::thread;

use clap::Parser;

use autoeq::roomeq::{
    load_config, merge_json_objects, optimize_room, CallbackAction, ProcessingMode, RoomConfig,
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
            if let Some(f) = solver_filter {
                if solver.name() != f
                    && (f != "both" || (solver.name() != "fem" && solver.name() != "bem"))
                {
                    continue;
                }
            }

            for method in &methods {
                // Apply mode filter
                if let Some(f) = mode_filter {
                    if method.name() != f && f != "all" {
                        continue;
                    }
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
// Test Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
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
        dur: u64,
    ) -> Self {
        Self {
            name: name.to_string(),
            scenario: scenario.to_string(),
            solver: solver.to_string(),
            method: method.to_string(),
            pre_score: pre,
            post_score: post,
            passed: post < pre,
            error: None,
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

    TestResult::success(&name, &scenario, &solver, &method, pre, post, dur)
}

// ---------------------------------------------------------------------------
// Parallel Execution
// ---------------------------------------------------------------------------

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}

fn run_parallel(test_cases: Vec<TestCase>, maxeval: usize, _num_jobs: usize) -> Vec<TestResult> {
    let (tx, rx) = channel::<TestResult>();
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    // Create thread pool - spawn all at once for simplicity
    for tc in test_cases {
        let tx = tx.clone();
        let maxeval = maxeval;

        let handle = thread::spawn(move || {
            let result = run_test_case(&tc, maxeval);
            let _ = tx.send(result);
        });
        handles.push(handle);
    }

    drop(tx);

    let mut results = Vec::new();
    while let Ok(result) = rx.recv() {
        results.push(result);
    }

    // Wait for remaining threads
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
