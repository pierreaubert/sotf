//! Testing and validation infrastructure
//!
//! Tools for comparing BEM results with analytical solutions,
//! computing error metrics, and exporting to JSON for visualization.

use math_audio_wave::analytical::AnalyticalSolution;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod json_output;
pub mod validation;

// Re-export these modules only when their contents are used elsewhere
// For now, they're only used when the testing feature is active
#[allow(unused_imports)]
pub use json_output::*;
#[allow(unused_imports)]
pub use validation::*;

/// Comparison between BEM and analytical solution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Test name
    pub test_name: String,

    /// Dimensionality (1, 2, or 3)
    pub dimensions: usize,

    /// Test parameters
    pub parameters: TestParameters,

    /// Analytical solution data
    pub analytical: SolutionData,

    /// BEM solution data
    pub bem: SolutionData,

    /// Error metrics
    pub errors: ErrorMetrics,

    /// Execution metadata
    pub metadata: ExecutionMetadata,
}

/// Test parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestParameters {
    /// Wave number k
    pub wave_number: f64,

    /// Frequency (Hz)
    pub frequency: f64,

    /// Wavelength (m)
    pub wavelength: f64,

    /// Characteristic dimension (radius, length, etc.)
    pub characteristic_dimension: f64,

    /// Dimensionless parameter (ka, kL, etc.)
    pub dimensionless_param: f64,

    /// Number of elements in BEM mesh
    pub num_elements: Option<usize>,

    /// Elements per wavelength
    pub elements_per_wavelength: Option<f64>,

    /// Additional custom parameters
    #[serde(flatten)]
    pub custom: serde_json::Value,
}

/// Solution data (positions and pressure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolutionData {
    /// Evaluation positions [[x, y, z], ...]
    pub positions: Vec<[f64; 3]>,

    /// Real part of pressure
    pub pressure_real: Vec<f64>,

    /// Imaginary part of pressure
    pub pressure_imag: Vec<f64>,

    /// Magnitude |p|
    pub magnitude: Vec<f64>,

    /// Phase arg(p) in radians
    pub phase: Vec<f64>,
}

/// Error metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Relative L2 error: ||p_bem - p_analytical||₂ / ||p_analytical||₂
    pub l2_relative: f64,

    /// Absolute L2 error: ||p_bem - p_analytical||₂
    pub l2_absolute: f64,

    /// L∞ error: max|p_bem - p_analytical|
    pub linf: f64,

    /// Mean absolute error
    pub mean_absolute: f64,

    /// RMS error
    pub rms: f64,

    /// Maximum relative error at any point
    pub max_relative: f64,

    /// Correlation coefficient
    pub correlation: f64,

    /// Pointwise errors (for plotting)
    pub pointwise_errors: Vec<f64>,
}

/// Execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Timestamp (ISO 8601)
    pub timestamp: String,

    /// Git commit hash
    pub git_commit: String,

    /// Execution time (milliseconds)
    pub execution_time_ms: u64,

    /// Peak memory usage (MB)
    pub memory_peak_mb: f64,

    /// Rust version
    pub rust_version: String,

    /// Library version
    pub bem_version: String,
}

impl ValidationResult {
    /// Create from analytical and BEM solutions
    pub fn new(
        test_name: impl Into<String>,
        analytical: &AnalyticalSolution,
        bem_pressure: Vec<Complex64>,
        execution_time_ms: u64,
        memory_peak_mb: f64,
    ) -> Self {
        // Ensure same number of points
        assert_eq!(
            analytical.positions.len(),
            bem_pressure.len(),
            "Analytical and BEM must have same number of points"
        );

        // Compute errors
        let errors = ErrorMetrics::compute(&analytical.pressure, &bem_pressure);

        // Convert positions to arrays
        let positions: Vec<[f64; 3]> = analytical
            .positions
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect();

        // Analytical data
        let analytical_data = SolutionData {
            positions: positions.clone(),
            pressure_real: analytical.real(),
            pressure_imag: analytical.imag(),
            magnitude: analytical.magnitude(),
            phase: analytical.phase(),
        };

        // BEM data
        let bem_data = SolutionData {
            positions,
            pressure_real: bem_pressure.iter().map(|p| p.re).collect(),
            pressure_imag: bem_pressure.iter().map(|p| p.im).collect(),
            magnitude: bem_pressure.iter().map(|p| p.norm()).collect(),
            phase: bem_pressure.iter().map(|p| p.arg()).collect(),
        };

        // Parameters
        let wavelength = 2.0 * std::f64::consts::PI / analytical.wave_number;
        let characteristic_dimension = analytical
            .metadata
            .get("radius")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        let parameters = TestParameters {
            wave_number: analytical.wave_number,
            frequency: analytical.frequency,
            wavelength,
            characteristic_dimension,
            dimensionless_param: analytical.wave_number * characteristic_dimension,
            num_elements: None, // Fill from BEM metadata
            elements_per_wavelength: None,
            custom: analytical.metadata.clone(),
        };

        // Metadata
        let metadata = ExecutionMetadata {
            timestamp: chrono::Utc::now().to_rfc3339(),
            git_commit: env!("GIT_HASH").to_string(),
            execution_time_ms,
            memory_peak_mb,
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            bem_version: crate::VERSION.to_string(),
        };

        Self {
            test_name: test_name.into(),
            dimensions: analytical.dimensions,
            parameters,
            analytical: analytical_data,
            bem: bem_data,
            errors,
            metadata,
        }
    }

    /// Save to JSON file
    pub fn save_json(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load from JSON file
    pub fn load_json(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let result = serde_json::from_str(&json)?;
        Ok(result)
    }

    /// Print summary to stdout
    pub fn print_summary(&self) {
        println!("╔══════════════════════════════════════════════════════╗");
        println!("║  BEM Validation: {}  ║", self.test_name);
        println!("╠══════════════════════════════════════════════════════╣");
        println!(
            "║  Dimensions: {}D                                      ║",
            self.dimensions
        );
        println!(
            "║  Wave number k: {:.4}                              ║",
            self.parameters.wave_number
        );
        println!(
            "║  Frequency: {:.2} Hz                               ║",
            self.parameters.frequency
        );
        println!(
            "║  ka: {:.4}                                         ║",
            self.parameters.dimensionless_param
        );
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║  Error Metrics:                                      ║");
        println!(
            "║    L2 (relative): {:.6}                          ║",
            self.errors.l2_relative
        );
        println!(
            "║    L∞:            {:.6}                          ║",
            self.errors.linf
        );
        println!(
            "║    Mean abs:      {:.6}                          ║",
            self.errors.mean_absolute
        );
        println!(
            "║    RMS:           {:.6}                          ║",
            self.errors.rms
        );
        println!(
            "║    Max relative:  {:.6}                          ║",
            self.errors.max_relative
        );
        println!(
            "║    Correlation:   {:.6}                          ║",
            self.errors.correlation
        );
        println!("╠══════════════════════════════════════════════════════╣");
        println!(
            "║  Execution time: {} ms                            ║",
            self.metadata.execution_time_ms
        );
        println!(
            "║  Memory peak: {:.2} MB                             ║",
            self.metadata.memory_peak_mb
        );
        println!("╚══════════════════════════════════════════════════════╝");
    }

    /// Check if test passed (based on error threshold)
    pub fn passed(&self, l2_threshold: f64) -> bool {
        self.errors.l2_relative < l2_threshold
    }
}

impl ErrorMetrics {
    /// Compute all error metrics
    pub fn compute(analytical: &[Complex64], bem: &[Complex64]) -> Self {
        assert_eq!(analytical.len(), bem.len());

        let n = analytical.len() as f64;

        // Pointwise errors
        let pointwise_errors: Vec<f64> = analytical
            .iter()
            .zip(bem.iter())
            .map(|(a, b)| (a - b).norm())
            .collect();

        // L2 absolute error
        let l2_absolute = pointwise_errors.iter().map(|e| e * e).sum::<f64>().sqrt();

        // L2 relative error
        let analytical_norm = analytical.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        let l2_relative = if analytical_norm > 1e-15 {
            l2_absolute / analytical_norm
        } else {
            l2_absolute
        };

        // L∞ error
        let linf = pointwise_errors.iter().cloned().fold(0.0_f64, f64::max);

        // Mean absolute error
        let mean_absolute = pointwise_errors.iter().sum::<f64>() / n;

        // RMS error
        let rms = (pointwise_errors.iter().map(|e| e * e).sum::<f64>() / n).sqrt();

        // Maximum relative error
        let max_relative = analytical
            .iter()
            .zip(bem.iter())
            .map(|(a, b)| {
                let a_norm = a.norm();
                if a_norm > 1e-15 {
                    (a - b).norm() / a_norm
                } else {
                    (a - b).norm()
                }
            })
            .fold(0.0_f64, f64::max);

        // Correlation coefficient
        let correlation = compute_correlation(analytical, bem);

        Self {
            l2_relative,
            l2_absolute,
            linf,
            mean_absolute,
            rms,
            max_relative,
            correlation,
            pointwise_errors,
        }
    }
}

/// Compute correlation coefficient between two complex signals
fn compute_correlation(a: &[Complex64], b: &[Complex64]) -> f64 {
    let n = a.len() as f64;

    // Use magnitude for correlation
    let a_mag: Vec<f64> = a.iter().map(|x| x.norm()).collect();
    let b_mag: Vec<f64> = b.iter().map(|x| x.norm()).collect();

    let a_mean = a_mag.iter().sum::<f64>() / n;
    let b_mean = b_mag.iter().sum::<f64>() / n;

    let numerator: f64 = a_mag
        .iter()
        .zip(b_mag.iter())
        .map(|(a_i, b_i)| (a_i - a_mean) * (b_i - b_mean))
        .sum();

    let a_var: f64 = a_mag.iter().map(|a_i| (a_i - a_mean).powi(2)).sum();
    let b_var: f64 = b_mag.iter().map(|b_i| (b_i - b_mean).powi(2)).sum();

    if a_var > 1e-15 && b_var > 1e-15 {
        numerator / (a_var * b_var).sqrt()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_metrics_perfect_match() {
        let data = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.5, 0.5),
            Complex64::new(0.0, 1.0),
        ];

        let errors = ErrorMetrics::compute(&data, &data);

        assert!(errors.l2_relative < 1e-10);
        assert!(errors.l2_absolute < 1e-10);
        assert!(errors.linf < 1e-10);
    }

    #[test]
    fn test_error_metrics_nonzero() {
        let analytical = vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.5)];

        let bem = vec![Complex64::new(1.01, 0.01), Complex64::new(0.51, 0.51)];

        let errors = ErrorMetrics::compute(&analytical, &bem);

        assert!(errors.l2_relative > 0.0);
        assert!(errors.l2_relative < 0.1); // Should be small
    }
}
