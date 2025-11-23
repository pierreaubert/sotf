//! BEM Runner - CLI for running NumCalc BEM solver
//!
//! This binary provides a command-line interface to the NumCalc BEM solver,
//! supporting single-frequency, multi-frequency, and parallel execution modes.

use anyhow::{Context, Result};
use bem::ffi::{MemoryEstimate, NumCalcConfig, NumCalcRunner};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "bem-runner")]
#[command(author, version, about, long_about = None)]
#[command(
    about = "BEM Runner - CLI for NumCalc BEM solver",
    long_about = "Command-line interface for running the NumCalc Boundary Element Method solver.\n\
                  Supports single-frequency, multi-frequency, and parallel execution modes."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    debug: bool,

    /// Output results as JSON
    #[arg(short, long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run NumCalc for a single frequency or frequency range
    Run {
        /// Project directory containing NC.inp
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: PathBuf,

        /// Starting frequency index (0-based)
        #[arg(short = 's', long)]
        freq_start: Option<usize>,

        /// Ending frequency index (0-based, inclusive)
        #[arg(short = 'e', long)]
        freq_end: Option<usize>,

        /// Maximum solver iterations
        #[arg(short = 'n', long, default_value = "250")]
        max_iterations: usize,

        /// Timeout in seconds
        #[arg(short = 't', long)]
        timeout: Option<u64>,

        /// Check mesh normals before solving
        #[arg(long)]
        check_normals: bool,
    },

    /// Estimate memory requirements for the problem
    EstimateMemory {
        /// Project directory containing NC.inp
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: PathBuf,
    },

    /// Run parallel solver for multiple frequencies
    Parallel {
        /// Project directory containing NC.inp
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: PathBuf,

        /// Number of parallel workers
        #[arg(short = 'j', long, default_value = "4")]
        workers: usize,

        /// Starting frequency index (0-based)
        #[arg(short = 's', long, default_value = "0")]
        freq_start: usize,

        /// Ending frequency index (0-based, inclusive)
        #[arg(short = 'e', long)]
        freq_end: usize,

        /// Maximum solver iterations per frequency
        #[arg(short = 'n', long, default_value = "250")]
        max_iterations: usize,

        /// Timeout per frequency in seconds
        #[arg(short = 't', long)]
        timeout: Option<u64>,
    },

    /// Validate project setup without running solver
    Validate {
        /// Project directory containing NC.inp
        #[arg(value_name = "PROJECT_DIR")]
        project_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    setup_logging(cli.verbose, cli.debug)?;

    match &cli.command {
        Commands::Run {
            project_dir,
            freq_start,
            freq_end,
            max_iterations,
            timeout,
            check_normals,
        } => run_solver(
            project_dir,
            *freq_start,
            *freq_end,
            *max_iterations,
            *timeout,
            *check_normals,
            cli.json,
        ),

        Commands::EstimateMemory { project_dir } => estimate_memory(project_dir, cli.json),

        Commands::Parallel {
            project_dir,
            workers,
            freq_start,
            freq_end,
            max_iterations,
            timeout,
        } => run_parallel(
            project_dir,
            *workers,
            *freq_start,
            *freq_end,
            *max_iterations,
            *timeout,
            cli.json,
        ),

        Commands::Validate { project_dir } => validate_project(project_dir, cli.json),
    }
}

fn setup_logging(verbose: bool, debug: bool) -> Result<()> {
    let log_level = if debug {
        "debug"
    } else if verbose {
        "info"
    } else {
        "warn"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    Ok(())
}

fn run_solver(
    project_dir: &PathBuf,
    freq_start: Option<usize>,
    freq_end: Option<usize>,
    max_iterations: usize,
    timeout_secs: Option<u64>,
    check_normals: bool,
    json_output: bool,
) -> Result<()> {
    let runner = NumCalcRunner::new(project_dir).context("Failed to create NumCalc runner")?;

    let mut config = NumCalcConfig::default();
    config.freq_start_idx = freq_start;
    config.freq_end_idx = freq_end;
    config.max_iterations = max_iterations;
    config.timeout = timeout_secs.map(Duration::from_secs);
    config.check_normals = check_normals;

    log::info!("Running NumCalc solver...");
    log::info!("  Project: {:?}", runner.project_dir());
    log::info!("  Frequency range: {:?} to {:?}", freq_start, freq_end);
    log::info!("  Max iterations: {}", max_iterations);

    let output = runner
        .run(&config)
        .context("Failed to run NumCalc solver")?;

    if json_output {
        let json = serde_json::json!({
            "success": output.success,
            "exit_code": output.exit_code,
            "execution_time_secs": output.execution_time.as_secs_f64(),
            "output_files": output.output_files,
            "peak_memory_mb": output.peak_memory_mb,
            "frequency_index": output.frequency_index,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("\n{}", "=".repeat(80));
        println!("NumCalc Execution Summary");
        println!("{}", "=".repeat(80));
        println!(
            "Status: {}",
            if output.success {
                "✓ SUCCESS"
            } else {
                "✗ FAILED"
            }
        );
        println!("Exit code: {:?}", output.exit_code);
        println!(
            "Execution time: {:.2}s",
            output.execution_time.as_secs_f64()
        );
        println!("Output files: {}", output.num_output_files());

        if !output.output_files.is_empty() {
            println!("\nGenerated files:");
            for file in &output.output_files {
                println!("  - {:?}", file);
            }
        }

        if !output.stdout.is_empty() {
            println!("\n{}", "-".repeat(80));
            println!("STDOUT:");
            println!("{}", "-".repeat(80));
            println!("{}", output.stdout);
        }

        if !output.stderr.is_empty() {
            println!("\n{}", "-".repeat(80));
            println!("STDERR:");
            println!("{}", "-".repeat(80));
            println!("{}", output.stderr);
        }
    }

    if !output.success {
        anyhow::bail!(
            "NumCalc execution failed with exit code {:?}",
            output.exit_code
        );
    }

    Ok(())
}

fn estimate_memory(project_dir: &PathBuf, json_output: bool) -> Result<()> {
    let runner = NumCalcRunner::new(project_dir).context("Failed to create NumCalc runner")?;

    log::info!("Estimating memory requirements...");

    let estimate = runner
        .estimate_memory()
        .context("Failed to estimate memory")?;

    if json_output {
        let json = serde_json::to_value(&estimate)?;
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("\n{}", "=".repeat(80));
        println!("Memory Estimate");
        println!("{}", "=".repeat(80));
        println!("{:#?}", estimate);
    }

    Ok(())
}

fn run_parallel(
    project_dir: &PathBuf,
    workers: usize,
    freq_start: usize,
    freq_end: usize,
    max_iterations: usize,
    timeout_secs: Option<u64>,
    json_output: bool,
) -> Result<()> {
    use rayon::prelude::*;

    log::info!("Running parallel NumCalc solver...");
    log::info!("  Workers: {}", workers);
    log::info!("  Frequency range: {} to {}", freq_start, freq_end);

    // Configure rayon thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build_global()
        .context("Failed to configure thread pool")?;

    let frequencies: Vec<usize> = (freq_start..=freq_end).collect();

    log::info!(
        "Processing {} frequencies with {} workers",
        frequencies.len(),
        workers
    );

    let results: Vec<Result<_>> = frequencies
        .par_iter()
        .map(|&freq_idx| {
            let runner = NumCalcRunner::new(project_dir)?;

            let mut config = NumCalcConfig::default();
            config.freq_start_idx = Some(freq_idx);
            config.freq_end_idx = Some(freq_idx);
            config.max_iterations = max_iterations;
            config.timeout = timeout_secs.map(Duration::from_secs);

            log::info!("Processing frequency index {}...", freq_idx);

            runner.run(&config)
        })
        .collect();

    // Process results
    let mut successes = 0;
    let mut failures = 0;
    let mut total_time = Duration::ZERO;

    for (idx, result) in results.iter().enumerate() {
        match result {
            Ok(output) => {
                if output.success {
                    successes += 1;
                } else {
                    failures += 1;
                }
                total_time += output.execution_time;

                if !json_output {
                    log::info!(
                        "Frequency {} (index {}): {} in {:.2}s",
                        freq_start + idx,
                        freq_start + idx,
                        if output.success { "✓" } else { "✗" },
                        output.execution_time.as_secs_f64()
                    );
                }
            }
            Err(e) => {
                failures += 1;
                log::error!("Frequency {} failed: {}", freq_start + idx, e);
            }
        }
    }

    if json_output {
        let json = serde_json::json!({
            "total_frequencies": frequencies.len(),
            "successes": successes,
            "failures": failures,
            "total_time_secs": total_time.as_secs_f64(),
            "average_time_secs": total_time.as_secs_f64() / frequencies.len() as f64,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("\n{}", "=".repeat(80));
        println!("Parallel Execution Summary");
        println!("{}", "=".repeat(80));
        println!("Total frequencies: {}", frequencies.len());
        println!("Successes: {}", successes);
        println!("Failures: {}", failures);
        println!("Total time: {:.2}s", total_time.as_secs_f64());
        println!(
            "Average time per frequency: {:.2}s",
            total_time.as_secs_f64() / frequencies.len() as f64
        );
    }

    if failures > 0 {
        anyhow::bail!("{} frequencies failed to compute", failures);
    }

    Ok(())
}

fn validate_project(project_dir: &PathBuf, json_output: bool) -> Result<()> {
    // Check project directory exists
    if !project_dir.exists() {
        anyhow::bail!("Project directory does not exist: {:?}", project_dir);
    }

    // Check NC.inp exists
    let nc_inp = project_dir.join("NC.inp");
    if !nc_inp.exists() {
        anyhow::bail!("NC.inp not found in project directory");
    }

    // Try to create runner (this validates NumCalc executable)
    let runner =
        NumCalcRunner::new(project_dir).context("Failed to validate NumCalc installation")?;

    if json_output {
        let json = serde_json::json!({
            "valid": true,
            "project_dir": runner.project_dir(),
            "executable": runner.executable(),
            "nc_inp": nc_inp,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("\n{}", "=".repeat(80));
        println!("Project Validation");
        println!("{}", "=".repeat(80));
        println!("✓ Project directory: {:?}", runner.project_dir());
        println!("✓ NC.inp found: {:?}", nc_inp);
        println!("✓ NumCalc executable: {:?}", runner.executable());
        println!("\nProject is ready to run!");
    }

    Ok(())
}
