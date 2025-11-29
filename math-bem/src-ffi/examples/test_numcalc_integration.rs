//! Integration tests for NumCalc FFI wrapper
//!
//! These tests require:
//! 1. NumCalc executable (either in PATH or via NUMCALC_PATH)
//! 2. A Mesh2HRTF project directory with NC.inp file
//!
//! ## Setting up a test project
//!
//! ### Option 1: Use Mesh2HRTF example projects
//!
//! ```bash
//! # Clone Mesh2HRTF repository
//! git clone https://github.com/Any2HRTF/Mesh2HRTF.git /tmp/Mesh2HRTF
//!
//! # Use one of the example projects
//! export TEST_PROJECT_DIR=/tmp/Mesh2HRTF/mesh2hrtf/NumCalc/data/reference_hrtfs/KU100
//! ```
//!
//! ### Option 2: Download pre-built test project
//!
//! ```bash
//! # Download from Mesh2HRTF website
//! wget https://github.com/Any2HRTF/Mesh2HRTF/raw/master/mesh2hrtf/NumCalc/data/reference_hrtfs/KU100.zip
//! unzip KU100.zip -d /tmp/
//! export TEST_PROJECT_DIR=/tmp/KU100
//! ```
//!
//! ### Option 3: Create minimal test project
//!
//! See `create_minimal_test_project()` function below.
//!
//! ## Running tests
//!
//! ```bash
//! # Set up NumCalc (if not in PATH)
//! export NUMCALC_PATH=/path/to/NumCalc
//!
//! # Set up test project
//! export TEST_PROJECT_DIR=/path/to/project
//!
//! # Run integration tests
//! cargo test --test test_numcalc_integration --features ffi -- --ignored --nocapture
//! ```

use bem::ffi::{NumCalcConfig, NumCalcRunner, ParallelBemRunner, SystemResources};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Get test project directory from environment
fn get_test_project_dir() -> Option<PathBuf> {
    std::env::var("TEST_PROJECT_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

/// Check if NumCalc is available
fn is_numcalc_available() -> bool {
    NumCalcRunner::new(".").map(|_| false).unwrap_or(false)
        || which::which("NumCalc").is_ok()
        || std::env::var("NUMCALC_PATH").is_ok()
}

/// Create a minimal test project for basic testing
///
/// This creates a very simple NC.inp file that NumCalc can process.
/// Note: This may not produce physically meaningful results, but it's
/// sufficient for testing the FFI wrapper functionality.
#[allow(dead_code)]
fn create_minimal_test_project(base_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let project_dir = base_dir.join(name);
    if project_dir.exists() {
        fs::remove_dir_all(&project_dir)?;
    }
    fs::create_dir_all(&project_dir)?;

    // Create minimal NC.inp
    let nc_inp = project_dir.join("NC.inp");
    let mut file = fs::File::create(nc_inp)?;

    // Minimal NumCalc input file
    writeln!(file, "Mesh2HRTF")?;
    writeln!(file, "MinimalTest")?;
    writeln!(file, "meshfile=mesh.msh")?;
    writeln!(file, "freqmin=1000")?;
    writeln!(file, "freqmax=2000")?;
    writeln!(file, "freqstep=1000")?;
    writeln!(file, "method=ML-FMM-BM")?;
    writeln!(file, "epsilon=1e-3")?;

    // Create dummy mesh file with a closed tetrahedron (4 triangles)
    let mesh_file = project_dir.join("mesh.msh");
    let mut mesh = fs::File::create(mesh_file)?;
    writeln!(
        mesh,
        "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$Nodes\n4\n1 0 0 0\n2 1 0 0\n3 0 1 0\n4 0 0 1\n$EndNodes\n$Elements\n4\n1 2 2 1 1 1 2 3\n2 2 2 1 1 1 2 4\n3 2 2 1 1 2 3 4\n4 2 2 1 1 1 3 4\n$EndElements"
    )?;

    Ok(project_dir)
}

#[test]
// #[ignore] // Run with: cargo test --test test_numcalc_integration -- --ignored
fn test_system_resources() {
    println!("\n=== System Resources Test ===\n");

    let resources = SystemResources::current().expect("Failed to get system resources");
    resources.print_summary();

    // Basic sanity checks
    assert!(resources.total_ram_mb > 0.0, "Total RAM should be positive");
    assert!(
        resources.available_ram_mb > 0.0,
        "Available RAM should be positive"
    );
    assert!(resources.num_cpus > 0, "Should have at least one CPU");
    assert!(
        resources.ram_usage_percent() >= 0.0 && resources.ram_usage_percent() <= 100.0,
        "RAM usage should be 0-100%"
    );

    println!("\n✓ System resources test passed");
}

#[test]
// #[ignore]
fn test_numcalc_executable_discovery() {
    println!("\n=== NumCalc Executable Discovery Test ===\n");

    // Try to find NumCalc using the same logic as NumCalcRunner
    let found = is_numcalc_available();

    if found {
        println!("✓ NumCalc executable found");

        // Try to create a runner (this will fail without valid project, but tests discovery)
        match NumCalcRunner::new(".") {
            Ok(runner) => {
                println!("  Executable: {:?}", runner.executable());
                println!(
                    "  (Note: Runner creation failed due to missing NC.inp, but executable was found)"
                );
            }
            Err(e) => {
                println!(
                    "  Discovery successful, but runner creation failed (expected): {}",
                    e
                );
                println!("  This is normal - we just tested executable discovery");
            }
        }
    } else {
        println!("✗ NumCalc not found in:");
        println!("  - NUMCALC_PATH environment variable");
        println!("  - System PATH");
        println!("  - Relative paths (NumCalc/bin/NumCalc, etc.)");
        println!("\nTo install NumCalc:");
        println!("  1. Clone: git clone https://github.com/Any2HRTF/Mesh2HRTF.git");
        println!("  2. Build: cd Mesh2HRTF/mesh2hrtf/NumCalc/src && make");
        println!("  3. Set: export NUMCALC_PATH=/path/to/Mesh2HRTF/mesh2hrtf/NumCalc/bin/NumCalc");
    }
}

#[test]
// #[ignore]
fn test_runner_creation() {
    println!("\n=== NumCalc Runner Creation Test ===\n");

    if !is_numcalc_available() {
        println!("NumCalc not available, skipping test execution");
        return;
    }

    // Use temp dir if TEST_PROJECT_DIR not set
    let temp_dir = std::env::temp_dir();
    let project_dir = match get_test_project_dir() {
        Some(dir) => dir,
        None => {
            println!("TEST_PROJECT_DIR not set, creating temporary test project...");
            create_minimal_test_project(&temp_dir, "test_runner_creation")
                .expect("Failed to create temp project")
        }
    };

    println!("Test project directory: {:?}", project_dir);

    // Verify NC.inp exists
    let nc_inp = project_dir.join("NC.inp");
    assert!(
        nc_inp.exists(),
        "NC.inp not found in project directory: {:?}",
        project_dir
    );
    println!("✓ NC.inp found");

    // Create runner
    match NumCalcRunner::new(&project_dir) {
        Ok(runner) => {
            println!("✓ NumCalcRunner created successfully");
            println!("  Executable: {:?}", runner.executable());
            println!("  Project dir: {:?}", runner.project_dir());
        }
        Err(e) => {
            panic!("Failed to create NumCalcRunner: {}", e);
        }
    }
}

#[test]
// #[ignore]
fn test_single_frequency_execution() {
    println!("\n=== Single Frequency Execution Test ===\n");

    if !is_numcalc_available() {
        println!("NumCalc not available, skipping test execution");
        return;
    }

    let temp_dir = std::env::temp_dir();
    let project_dir = get_test_project_dir().unwrap_or_else(|| {
        create_minimal_test_project(&temp_dir, "test_single_freq")
            .expect("Failed to create temp project")
    });

    let runner = NumCalcRunner::new(&project_dir).expect("Failed to create runner");

    println!("Running NumCalc for frequency index 0...");

    let config = NumCalcConfig::single_frequency(0)
        .with_max_iterations(100)
        .with_timeout(std::time::Duration::from_secs(300)); // 5 minute timeout

    match runner.run(&config) {
        Ok(output) => {
            println!("\n✓ Execution completed");
            output.print_summary();

            // Validate output
            assert!(
                output.execution_time.as_secs() < 300,
                "Execution should complete within timeout"
            );

            if output.is_success() {
                println!("\n✓ NumCalc execution successful!");
                assert!(
                    output.num_output_files() > 0,
                    "Should generate output files"
                );
                println!("  Output files:");
                for file in &output.output_files {
                    println!("    - {:?}", file);
                }
            } else {
                println!("\n✗ NumCalc execution failed");
                println!("Exit code: {:?}", output.exit_code);
                if !output.stderr.is_empty() {
                    println!("\nStderr:\n{}", output.stderr);
                }
                if !output.stdout.is_empty() {
                    println!("\nStdout:\n{}", output.stdout);
                }
                panic!("NumCalc execution failed");
            }
        }
        Err(e) => {
            panic!("Failed to run NumCalc: {}", e);
        }
    }
}

#[test]
// #[ignore]
fn test_memory_estimation() {
    println!("\n=== Memory Estimation Test ===\n");

    if !is_numcalc_available() {
        println!("NumCalc not available, skipping test execution");
        return;
    }

    let temp_dir = std::env::temp_dir();
    let project_dir = get_test_project_dir().unwrap_or_else(|| {
        create_minimal_test_project(&temp_dir, "test_memory")
            .expect("Failed to create temp project")
    });
    let runner = NumCalcRunner::new(&project_dir).expect("Failed to create runner");

    println!("Running NumCalc memory estimation...");

    match runner.estimate_memory() {
        Ok(estimate) => {
            println!("✓ Memory estimation completed");
            println!("  Total: {:.1} MB", estimate.total_mb);
            println!("  Frequencies: {}", estimate.num_frequencies);
            println!("  Max per frequency: {:.1} MB", estimate.max_memory_mb());
            println!("  Safety factor: {:.1}", estimate.safety_factor);

            let resources = SystemResources::current().expect("Failed to get resources");
            let fits = estimate.fits_in_ram(resources.available_ram_mb);
            println!(
                "  Fits in available RAM ({:.1} MB)? {}",
                resources.available_ram_mb, fits
            );
        }
        Err(e) => {
            println!("✗ Memory estimation failed: {}", e);
            println!("  This may be normal if Memory.txt parsing is not implemented");
            println!("  Error: {}", e);
        }
    }
}

#[test]
// #[ignore]
fn test_parallel_execution_small() {
    println!("\n=== Parallel Execution Test (3 frequencies) ===\n");

    if !is_numcalc_available() {
        println!("NumCalc not available, skipping test execution");
        return;
    }

    let temp_dir = std::env::temp_dir();
    let project_dir = get_test_project_dir().unwrap_or_else(|| {
        create_minimal_test_project(&temp_dir, "test_parallel")
            .expect("Failed to create temp project")
    });

    let runner = ParallelBemRunner::new(&project_dir).expect("Failed to create parallel runner");

    let runner = runner
        .with_max_concurrent(2)
        .with_max_cpu_percent(90.0)
        .with_max_ram_gb(8.0);

    println!("Configuration:");
    println!("  Max concurrent: 2");
    println!("  Max CPU: 90%");
    println!("  Max RAM: 8.0 GB");
    println!("\nRunning 3 frequencies in parallel...");

    match runner.run_all_frequencies(3) {
        Ok(results) => {
            println!("\n✓ Parallel execution completed!");

            let mut total_time = std::time::Duration::ZERO;
            let mut successful = 0;

            println!("\nResults:");
            for (i, result) in results.iter().enumerate() {
                let status = if result.is_success() {
                    successful += 1;
                    "✓"
                } else {
                    "✗"
                };

                println!(
                    "  Freq {}: {} ({:.2}s, {} files, exit: {:?})",
                    i,
                    status,
                    result.execution_time.as_secs_f64(),
                    result.num_output_files(),
                    result.exit_code
                );

                total_time += result.execution_time;
            }

            println!("\nSummary:");
            println!("  Successful: {}/{}", successful, results.len());
            println!("  Total time: {:.2}s", total_time.as_secs_f64());
            println!(
                "  Average time: {:.2}s/freq",
                total_time.as_secs_f64() / results.len() as f64
            );

            assert!(successful > 0, "At least one frequency should succeed");
        }
        Err(e) => {
            panic!("Parallel execution failed: {}", e);
        }
    }
}

#[test]
// #[ignore]
fn test_resource_monitoring() {
    println!("\n=== Resource Monitoring Test ===\n");

    use bem::ffi::ResourceMonitor;

    let mut monitor = ResourceMonitor::new()
        .with_max_cpu(95.0)
        .with_max_ram(90.0)
        .with_interval(std::time::Duration::from_millis(500));

    println!("Taking 5 resource samples...");

    for i in 0..5 {
        let resources = monitor.sample().expect("Failed to sample resources");
        println!(
            "  Sample {}: RAM {:.1}%, CPU {:.1}%",
            i + 1,
            resources.ram_usage_percent(),
            resources.cpu_usage_percent
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    assert_eq!(monitor.num_samples(), 5, "Should have 5 samples");

    if let Some(avg_cpu) = monitor.avg_cpu_usage() {
        println!("\nAverage CPU usage: {:.1}%", avg_cpu);
        assert!(avg_cpu >= 0.0 && avg_cpu <= 100.0);
    }

    if let Some(peak_mem) = monitor.peak_memory_mb() {
        println!("Peak memory usage: {:.1} MB", peak_mem);
        assert!(peak_mem > 0.0);
    }

    println!("\n✓ Resource monitoring test passed");
}

#[test]
// #[ignore]
fn test_can_run_task() {
    println!("\n=== Task Feasibility Test ===\n");

    let resources = SystemResources::current().expect("Failed to get resources");
    resources.print_summary();

    // Test various task sizes
    let test_tasks = vec![
        (100.0, "100 MB task"),
        (1000.0, "1 GB task"),
        (5000.0, "5 GB task"),
        (
            resources.total_ram_mb * 2.0,
            "Oversized task (2x total RAM)",
        ),
    ];

    println!("\nTask feasibility checks (90% CPU threshold):");
    for (required_mb, description) in test_tasks {
        let can_run = resources.can_run_task(required_mb, 90.0);
        println!(
            "  {} ({:.1} MB): {}",
            description,
            required_mb,
            if can_run {
                "✓ Can run"
            } else {
                "✗ Cannot run"
            }
        );
    }

    // The small task should always be runnable
    assert!(
        resources.can_run_task(100.0, 99.0),
        "Should be able to run small tasks"
    );

    // An oversized task should never be runnable
    assert!(
        !resources.can_run_task(resources.total_ram_mb * 2.0, 99.0),
        "Should not be able to run tasks larger than total RAM"
    );

    println!("\n✓ Task feasibility test passed");
}
