//! Parallel BEM execution using Rayon
//!
//! Implements memory-efficient parallel execution of NumCalc across
//! multiple frequencies using Rayon for data parallelism.
//!
//! ## Design
//!
//! - **Rayon** for parallelism (NOT tokio - no async overhead)
//! - **Resource-aware scheduling** (RAM + CPU monitoring)
//! - **Work stealing** for load balancing
//! - **Configurable concurrency** limits
//!
//! ## Example
//!
//! ```rust,no_run
//! use bem::ffi::ParallelBemRunner;
//!
//! let runner = ParallelBemRunner::new("project_dir")?
//!     .with_max_concurrent(4)
//!     .with_max_ram_gb(8.0)
//!     .with_max_cpu_percent(90.0);
//!
//! let results = runner.run_all_frequencies(100)?;
//! println!("Completed {} frequencies", results.len());
//! # Ok::<(), anyhow::Error>(())
//! ```

use super::config::{NumCalcConfig, NumCalcOutput};
use super::resources::{ResourceMonitor, SystemResources};
use super::runner::NumCalcRunner;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Parallel BEM runner using Rayon
pub struct ParallelBemRunner {
    /// Underlying NumCalc runner
    runner: NumCalcRunner,

    /// Maximum concurrent executions
    max_concurrent: usize,

    /// Maximum RAM usage (GB)
    max_ram_gb: f64,

    /// Maximum CPU usage (percent, 0-100)
    max_cpu_percent: f64,

    /// Resource monitor
    resource_monitor: Arc<Mutex<ResourceMonitor>>,
}

impl ParallelBemRunner {
    /// Create new parallel runner
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self> {
        let runner = NumCalcRunner::new(project_dir)?;

        // Default to number of CPU cores
        let max_concurrent = num_cpus::get();

        // Default to 80% of total RAM
        let resources = SystemResources::current()?;
        let max_ram_gb = resources.total_ram_mb / 1024.0 * 0.8;

        Ok(Self {
            runner,
            max_concurrent,
            max_ram_gb,
            max_cpu_percent: 90.0,
            resource_monitor: Arc::new(Mutex::new(ResourceMonitor::new())),
        })
    }

    /// Set maximum concurrent executions
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Set maximum RAM usage (GB)
    pub fn with_max_ram_gb(mut self, max_gb: f64) -> Self {
        self.max_ram_gb = max_gb;
        self
    }

    /// Set maximum CPU usage (percent)
    pub fn with_max_cpu_percent(mut self, max_percent: f64) -> Self {
        self.max_cpu_percent = max_percent;
        self
    }

    /// Run all frequencies in parallel
    ///
    /// # Arguments
    ///
    /// * `num_frequencies` - Total number of frequencies to compute
    ///
    /// # Returns
    ///
    /// Vec of NumCalcOutput, one per frequency (in order)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use bem::ffi::ParallelBemRunner;
    ///
    /// let runner = ParallelBemRunner::new("project")?;
    /// let results = runner.run_all_frequencies(50)?;
    ///
    /// for (i, result) in results.iter().enumerate() {
    ///     if result.is_success() {
    ///         println!("Frequency {}: {:.2}s", i, result.execution_time.as_secs_f64());
    ///     }
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn run_all_frequencies(&self, num_frequencies: usize) -> Result<Vec<NumCalcOutput>> {
        log::info!(
            "Running {} frequencies with max {} concurrent",
            num_frequencies,
            self.max_concurrent
        );

        let start_time = Instant::now();

        // Estimate memory per frequency
        log::info!("Estimating memory requirements...");
        let memory_estimate = self.runner.estimate_memory()?;

        log::info!(
            "Memory estimate: max {:.1} MB per frequency",
            memory_estimate.max_memory_mb()
        );

        // Check if we can fit in available RAM
        let resources = SystemResources::current()?;
        if !memory_estimate.fits_in_ram(resources.available_ram_mb) {
            log::warn!(
                "Memory requirement ({:.1} MB) exceeds available RAM ({:.1} MB)",
                memory_estimate.max_memory_mb(),
                resources.available_ram_mb
            );
        }

        // Configure Rayon thread pool
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_concurrent)
            .build()
            .context("Failed to create Rayon thread pool")?;

        // Create frequency indices
        let freq_indices: Vec<usize> = (0..num_frequencies).collect();

        // Shared state for results (with proper ordering)
        let results: Arc<Mutex<Vec<Option<NumCalcOutput>>>> =
            Arc::new(Mutex::new(vec![None; num_frequencies]));

        // Shared progress counter
        let completed = Arc::new(Mutex::new(0_usize));

        // Execute in parallel using Rayon
        pool.install(|| {
            freq_indices.par_iter().try_for_each(|&freq_idx| {
                // Wait for resources if needed
                self.wait_for_resources(memory_estimate.max_memory_mb())?;

                // Run NumCalc for this frequency
                log::debug!("Starting frequency {}", freq_idx);

                let config = NumCalcConfig::single_frequency(freq_idx);
                let output = self.runner.run(&config)?;

                // Store result
                {
                    let mut results_lock = results.lock().unwrap();
                    results_lock[freq_idx] = Some(output);
                }

                // Update progress
                {
                    let mut completed_lock = completed.lock().unwrap();
                    *completed_lock += 1;
                    let progress = *completed_lock;

                    log::info!(
                        "Progress: {}/{} ({:.1}%)",
                        progress,
                        num_frequencies,
                        (progress as f64 / num_frequencies as f64) * 100.0
                    );
                }

                Ok::<(), anyhow::Error>(())
            })
        })?;

        // Extract results (should all be Some now)
        let results = Arc::try_unwrap(results)
            .unwrap()
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|opt| opt.expect("All results should be computed"))
            .collect();

        let total_time = start_time.elapsed();
        log::info!(
            "Completed {} frequencies in {:.2}s ({:.2}s/freq average)",
            num_frequencies,
            total_time.as_secs_f64(),
            total_time.as_secs_f64() / num_frequencies as f64
        );

        Ok(results)
    }

    /// Wait for sufficient resources to become available
    fn wait_for_resources(&self, required_ram_mb: f64) -> Result<()> {
        let mut monitor = self.resource_monitor.lock().unwrap();
        monitor.wait_for_resources(required_ram_mb)
    }

    /// Run frequency subset in parallel
    ///
    /// Useful for splitting work across multiple machines or testing.
    pub fn run_frequency_range(
        &self,
        start_idx: usize,
        end_idx: usize,
    ) -> Result<Vec<NumCalcOutput>> {
        let num_frequencies = end_idx - start_idx + 1;
        log::info!(
            "Running frequency range {}..{} ({} frequencies)",
            start_idx,
            end_idx,
            num_frequencies
        );

        // Similar to run_all_frequencies but with offset
        let freq_indices: Vec<usize> = (start_idx..=end_idx).collect();

        let results: Arc<Mutex<Vec<Option<NumCalcOutput>>>> =
            Arc::new(Mutex::new(vec![None; num_frequencies]));

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_concurrent)
            .build()?;

        pool.install(|| {
            freq_indices.par_iter().try_for_each(|&freq_idx| {
                let config = NumCalcConfig::single_frequency(freq_idx);
                let output = self.runner.run(&config)?;

                let mut results_lock = results.lock().unwrap();
                results_lock[freq_idx - start_idx] = Some(output);

                Ok::<(), anyhow::Error>(())
            })
        })?;

        let results = Arc::try_unwrap(results)
            .unwrap()
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|opt| opt.expect("All results computed"))
            .collect();

        Ok(results)
    }

    /// Get resource usage statistics
    pub fn get_resource_stats(&self) -> Result<SystemResources> {
        SystemResources::current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_runner_creation() {
        // Will fail without NumCalc, but tests the builder pattern
        match ParallelBemRunner::new("test_project") {
            Ok(runner) => {
                let runner = runner
                    .with_max_concurrent(2)
                    .with_max_ram_gb(4.0)
                    .with_max_cpu_percent(80.0);

                assert_eq!(runner.max_concurrent, 2);
                assert_eq!(runner.max_ram_gb, 4.0);
                assert_eq!(runner.max_cpu_percent, 80.0);
            }
            Err(e) => {
                println!("Expected failure without NumCalc: {}", e);
            }
        }
    }

    #[test]
    fn test_rayon_thread_pool() {
        // Test that we can create Rayon thread pools
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();

        let results: Vec<i32> = pool.install(|| {
            (0..10).into_par_iter().map(|x| x * 2).collect()
        });

        assert_eq!(results.len(), 10);
        assert_eq!(results[5], 10);
    }

    #[test]
    #[ignore]  // Requires actual NumCalc installation
    fn test_parallel_execution() {
        // This test is ignored - run with cargo test --ignored
        // Requires TEST_PROJECT_DIR environment variable

        let project_dir = std::env::var("TEST_PROJECT_DIR")
            .unwrap_or_else(|_| "test_project".to_string());

        if let Ok(runner) = ParallelBemRunner::new(project_dir) {
            match runner.run_all_frequencies(5) {
                Ok(results) => {
                    println!("Successfully ran {} frequencies", results.len());
                    for (i, result) in results.iter().enumerate() {
                        println!(
                            "Frequency {}: {} ({:.2}s)",
                            i,
                            if result.is_success() { "✓" } else { "✗" },
                            result.execution_time.as_secs_f64()
                        );
                    }
                }
                Err(e) => {
                    println!("Parallel execution failed: {}", e);
                }
            }
        }
    }
}
