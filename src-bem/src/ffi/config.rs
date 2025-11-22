//! NumCalc configuration and output types

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for NumCalc execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumCalcConfig {
    /// Starting frequency index (0-based)
    pub freq_start_idx: Option<usize>,

    /// Ending frequency index (inclusive)
    pub freq_end_idx: Option<usize>,

    /// Maximum iterations for CGS solver
    /// Default: 250
    pub max_iterations: usize,

    /// Estimate RAM consumption (creates Memory.txt)
    pub estimate_ram: bool,

    /// Check that all normal vectors point toward same domain
    pub check_normals: bool,

    /// Timeout for execution (None = no timeout)
    pub timeout: Option<Duration>,

    /// Working directory (defaults to project directory)
    pub working_dir: Option<PathBuf>,
}

impl Default for NumCalcConfig {
    fn default() -> Self {
        Self {
            freq_start_idx: None,
            freq_end_idx: None,
            max_iterations: 250,
            estimate_ram: false,
            check_normals: false,
            timeout: None,
            working_dir: None,
        }
    }
}

impl NumCalcConfig {
    /// Create config for single frequency
    pub fn single_frequency(freq_idx: usize) -> Self {
        Self {
            freq_start_idx: Some(freq_idx),
            freq_end_idx: Some(freq_idx),
            ..Default::default()
        }
    }

    /// Create config for frequency range
    pub fn frequency_range(start: usize, end: usize) -> Self {
        Self {
            freq_start_idx: Some(start),
            freq_end_idx: Some(end),
            ..Default::default()
        }
    }

    /// Create config for RAM estimation only
    pub fn estimate_memory() -> Self {
        Self {
            estimate_ram: true,
            ..Default::default()
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set max iterations
    pub fn with_max_iterations(mut self, max_iter: usize) -> Self {
        self.max_iterations = max_iter;
        self
    }
}

/// Output from NumCalc execution
#[derive(Debug, Clone)]
pub struct NumCalcOutput {
    /// Success status
    pub success: bool,

    /// Exit code
    pub exit_code: Option<i32>,

    /// Standard output
    pub stdout: String,

    /// Standard error
    pub stderr: String,

    /// Output files generated
    pub output_files: Vec<PathBuf>,

    /// Execution time
    pub execution_time: Duration,

    /// Peak memory usage (MB)
    pub peak_memory_mb: Option<f64>,

    /// Frequency index (if single frequency)
    pub frequency_index: Option<usize>,
}

impl NumCalcOutput {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.success && self.exit_code == Some(0)
    }

    /// Get number of output files
    pub fn num_output_files(&self) -> usize {
        self.output_files.len()
    }

    /// Print summary to stdout
    pub fn print_summary(&self) {
        println!("NumCalc Execution Summary:");
        println!("  Success: {}", self.success);
        println!("  Exit code: {:?}", self.exit_code);
        println!("  Execution time: {:.2}s", self.execution_time.as_secs_f64());
        println!("  Output files: {}", self.num_output_files());
        if let Some(mem) = self.peak_memory_mb {
            println!("  Peak memory: {:.2} MB", mem);
        }
        if let Some(freq_idx) = self.frequency_index {
            println!("  Frequency index: {}", freq_idx);
        }
    }
}

/// Memory estimation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEstimate {
    /// Total memory required (MB)
    pub total_mb: f64,

    /// Memory per frequency step (MB)
    pub per_frequency_mb: Vec<f64>,

    /// Number of frequencies
    pub num_frequencies: usize,

    /// Safety factor applied
    pub safety_factor: f64,
}

impl MemoryEstimate {
    /// Parse from Memory.txt file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse Memory.txt content
    pub fn parse(content: &str) -> anyhow::Result<Self> {
        // TODO: Implement actual parsing based on Memory.txt format
        // For now, return placeholder
        Ok(Self {
            total_mb: 0.0,
            per_frequency_mb: vec![],
            num_frequencies: 0,
            safety_factor: 1.2,
        })
    }

    /// Get maximum memory requirement
    pub fn max_memory_mb(&self) -> f64 {
        self.per_frequency_mb
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
    }

    /// Check if memory requirement fits in available RAM
    pub fn fits_in_ram(&self, available_mb: f64) -> bool {
        self.max_memory_mb() * self.safety_factor < available_mb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builders() {
        let config = NumCalcConfig::single_frequency(5);
        assert_eq!(config.freq_start_idx, Some(5));
        assert_eq!(config.freq_end_idx, Some(5));

        let config = NumCalcConfig::frequency_range(0, 10);
        assert_eq!(config.freq_start_idx, Some(0));
        assert_eq!(config.freq_end_idx, Some(10));

        let config = NumCalcConfig::estimate_memory();
        assert!(config.estimate_ram);
    }

    #[test]
    fn test_config_with_methods() {
        let config = NumCalcConfig::default()
            .with_timeout(Duration::from_secs(600))
            .with_max_iterations(500);

        assert_eq!(config.timeout, Some(Duration::from_secs(600)));
        assert_eq!(config.max_iterations, 500);
    }

    #[test]
    fn test_output_is_success() {
        let output = NumCalcOutput {
            success: true,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            output_files: vec![],
            execution_time: Duration::from_secs(1),
            peak_memory_mb: Some(100.0),
            frequency_index: None,
        };

        assert!(output.is_success());
    }

    #[test]
    fn test_memory_estimate_max() {
        let estimate = MemoryEstimate {
            total_mb: 1000.0,
            per_frequency_mb: vec![50.0, 100.0, 75.0, 120.0],
            num_frequencies: 4,
            safety_factor: 1.2,
        };

        assert_eq!(estimate.max_memory_mb(), 120.0);
        assert!(estimate.fits_in_ram(200.0));
        assert!(!estimate.fits_in_ram(100.0));
    }
}
