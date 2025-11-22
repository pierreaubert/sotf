//! NumCalc FFI Demo
//!
//! Demonstrates how to use the NumCalc FFI wrapper.
//!
//! # Usage
//!
//! ```bash
//! # Set up test project directory
//! export TEST_PROJECT_DIR=/path/to/mesh2hrtf/project
//!
//! # Run demo
//! cargo run --release --example numcalc_ffi_demo --features ffi
//! ```

use bem::ffi::{NumCalcConfig, NumCalcRunner, ParallelBemRunner};
use bem::ffi::SystemResources;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║         NumCalc FFI Wrapper Demonstration             ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Get project directory from environment or use example
    let project_dir = std::env::var("TEST_PROJECT_DIR")
        .unwrap_or_else(|_| {
            eprintln!("Warning: TEST_PROJECT_DIR not set, using 'example_project'");
            "example_project".to_string()
        });

    println!("Project directory: {}\n", project_dir);

    // Part 1: System Resources
    println!("═══ Part 1: System Resources ═══\n");
    demo_system_resources()?;

    // Part 2: Single Frequency Execution
    println!("\n═══ Part 2: Single Frequency Execution ═══\n");
    demo_single_frequency(&project_dir)?;

    // Part 3: Memory Estimation
    println!("\n═══ Part 3: Memory Estimation ═══\n");
    demo_memory_estimation(&project_dir)?;

    // Part 4: Parallel Execution
    println!("\n═══ Part 4: Parallel Frequency Sweep ═══\n");
    demo_parallel_execution(&project_dir)?;

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                Demo Complete!                          ║");
    println!("╚════════════════════════════════════════════════════════╝");

    Ok(())
}

fn demo_system_resources() -> anyhow::Result<()> {
    let resources = SystemResources::current()?;
    resources.print_summary();

    println!("\nResource checks:");
    println!("  Can run 100 MB task? {}", resources.can_run_task(100.0, 95.0));
    println!("  Can run 10 GB task? {}", resources.can_run_task(10_000.0, 95.0));

    Ok(())
}

fn demo_single_frequency(project_dir: &str) -> anyhow::Result<()> {
    // Try to create runner
    match NumCalcRunner::new(project_dir) {
        Ok(runner) => {
            println!("✓ NumCalc runner created successfully");
            println!("  Executable: {:?}", runner.executable());
            println!("  Project dir: {:?}", runner.project_dir());

            // Run single frequency
            println!("\nRunning frequency index 0...");

            let config = NumCalcConfig::single_frequency(0)
                .with_max_iterations(100);

            match runner.run(&config) {
                Ok(output) => {
                    output.print_summary();

                    if output.is_success() {
                        println!("✓ Execution successful!");
                    } else {
                        println!("✗ Execution failed");
                        if !output.stderr.is_empty() {
                            println!("Error output:\n{}", output.stderr);
                        }
                    }
                }
                Err(e) => {
                    println!("✗ Execution error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Could not create runner: {}", e);
            println!("  (This is expected if NumCalc is not installed)");
        }
    }

    Ok(())
}

fn demo_memory_estimation(project_dir: &str) -> anyhow::Result<()> {
    match NumCalcRunner::new(project_dir) {
        Ok(runner) => {
            println!("Estimating memory requirements...");

            match runner.estimate_memory() {
                Ok(estimate) => {
                    println!("✓ Memory estimation complete");
                    println!("  Total: {:.1} MB", estimate.total_mb);
                    println!("  Max per frequency: {:.1} MB", estimate.max_memory_mb());
                    println!("  Number of frequencies: {}", estimate.num_frequencies);

                    let resources = SystemResources::current()?;
                    let fits = estimate.fits_in_ram(resources.available_ram_mb);
                    println!("  Fits in available RAM ({:.1} MB)? {}", resources.available_ram_mb, fits);
                }
                Err(e) => {
                    println!("✗ Memory estimation failed: {}", e);
                }
            }
        }
        Err(_) => {
            println!("Skipping (NumCalc not available)");
        }
    }

    Ok(())
}

fn demo_parallel_execution(project_dir: &str) -> anyhow::Result<()> {
    match ParallelBemRunner::new(project_dir) {
        Ok(runner) => {
            let runner = runner
                .with_max_concurrent(2)  // Limit for demo
                .with_max_cpu_percent(90.0)
                .with_max_ram_gb(4.0);

            println!("Parallel runner configured:");
            println!("  Max concurrent: 2");
            println!("  Max CPU: 90%");
            println!("  Max RAM: 4.0 GB");

            println!("\nRunning 5 frequencies in parallel...");

            match runner.run_all_frequencies(5) {
                Ok(results) => {
                    println!("✓ Parallel execution complete!");
                    println!("\nResults:");

                    let mut total_time = std::time::Duration::ZERO;
                    let mut successful = 0;

                    for (i, result) in results.iter().enumerate() {
                        let status = if result.is_success() {
                            successful += 1;
                            "✓"
                        } else {
                            "✗"
                        };

                        println!(
                            "  Freq {}: {} ({:.2}s, {} files)",
                            i,
                            status,
                            result.execution_time.as_secs_f64(),
                            result.num_output_files()
                        );

                        total_time += result.execution_time;
                    }

                    println!("\nSummary:");
                    println!("  Successful: {}/{}", successful, results.len());
                    println!("  Total time: {:.2}s", total_time.as_secs_f64());
                    println!("  Average time: {:.2}s/freq", total_time.as_secs_f64() / results.len() as f64);
                }
                Err(e) => {
                    println!("✗ Parallel execution failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Could not create parallel runner: {}", e);
            println!("  (This is expected if NumCalc is not installed)");
        }
    }

    Ok(())
}
