//! Headphone EQ optimization
//!
//! Provides thin wrappers around the autoeq library for headphone equalization.
//! Most functionality is delegated to the library.

use std::path::PathBuf;

// Re-export types from autoeq for convenience
pub use autoeq::{HeadphoneOptResult, VisualizationCurves};

/// Bundled target curve data
pub mod target_curves {
    pub const HARMAN_OVER_EAR_2018: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2018.csv");
    pub const HARMAN_OVER_EAR_2015: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2015.csv");
    pub const HARMAN_OVER_EAR_2013: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2013.csv");
    pub const HARMAN_IN_EAR_2019: &str =
        include_str!("../../../../data_tests/targets/harman-in-ear-2019.csv");
}

/// Result of headphone EQ optimization with all curves for visualization
#[derive(Clone, Debug)]
pub struct HeadphoneOptimizationResult {
    /// Optimized biquad filters
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Frequency points (Hz) - log-spaced
    pub frequencies: Vec<f64>,
    /// Input headphone measurement curve (dB)
    pub input_curve: Vec<f64>,
    /// Target curve (dB)
    pub target_curve: Vec<f64>,
    /// Deviation from target = target - input (dB)
    pub deviation_curve: Vec<f64>,
    /// Combined filter response (dB)
    pub filter_response: Vec<f64>,
    /// Error = deviation - filter_response (dB)
    pub error_curve: Vec<f64>,
    /// Corrected response = input + filter_response (dB)
    pub corrected_curve: Vec<f64>,
    /// Individual filter responses (each filter's dB response)
    pub individual_filter_responses: Vec<Vec<f64>>,
    /// Path where results were saved
    pub output_path: String,
    /// Optimization history (iteration, loss)
    pub optimization_history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

impl From<HeadphoneOptResult> for HeadphoneOptimizationResult {
    fn from(result: HeadphoneOptResult) -> Self {
        Self {
            biquads: result.biquads,
            frequencies: result.curves.frequencies,
            input_curve: result.curves.input_curve,
            target_curve: result.curves.target_curve,
            deviation_curve: result.curves.deviation_curve,
            filter_response: result.curves.filter_response,
            error_curve: result.curves.error_curve,
            corrected_curve: result.curves.corrected_curve,
            individual_filter_responses: result.curves.individual_filter_responses,
            output_path: String::new(),
            optimization_history: result.history,
            initial_loss: result.initial_loss,
            final_loss: result.final_loss,
        }
    }
}

/// Run headphone EQ optimization
///
/// # Arguments
/// * `curve_path` - Path to the headphone measurement CSV file
/// * `target` - Target curve identifier (e.g., "harman-over-ear-2018", "custom")
/// * `target_custom_path` - Path to custom target curve (only used if target is "custom")
/// * `args` - Optimization arguments (use Args::headphone_defaults() as base)
/// * `_export_format` - Export format for the resulting EQ file (unused, for compatibility)
///
/// # Returns
/// The optimization result with all curves for visualization
pub fn run_headphone_optimization(
    curve_path: &str,
    target: &str,
    target_custom_path: &str,
    args: &autoeq::Args,
    _export_format: &str,
) -> Result<HeadphoneOptimizationResult, String> {
    // Load headphone measurement
    let curve_path = PathBuf::from(curve_path);

    // Load target curve
    let target_curve = load_target_curve(target, target_custom_path)?;

    // Use library function (no progress callback, no config)
    let optim_params = autoeq::OptimParams::from(args);
    let result = autoeq::optimize_headphone(
        &curve_path,
        &target_curve,
        &optim_params,
        None, // No progress config
        None::<fn(&autoeq::ProgressUpdate) -> autoeq::de::CallbackAction>,
    )
    .map_err(|e| e.to_string())?;

    Ok(HeadphoneOptimizationResult::from(result))
}

/// Run headphone EQ optimization with a progress callback
pub fn run_headphone_optimization_with_callback<F>(
    curve_path: &str,
    target: &str,
    target_custom_path: &str,
    args: &autoeq::Args,
    progress_callback: Option<F>,
) -> Result<HeadphoneOptimizationResult, String>
where
    F: FnMut(&autoeq::ProgressUpdate) -> autoeq::de::CallbackAction + Send + 'static,
{
    let curve_path = PathBuf::from(curve_path);
    let target_curve = load_target_curve(target, target_custom_path)?;

    let progress_config = Some(autoeq::ProgressCallbackConfig {
        interval: 50,
        include_biquads: false,
        include_filter_response: false,
        frequencies: Vec::new(),
    });

    let optim_params = autoeq::OptimParams::from(args);
    let result = autoeq::optimize_headphone(
        &curve_path,
        &target_curve,
        &optim_params,
        progress_config,
        progress_callback,
    )
    .map_err(|e| e.to_string())?;

    Ok(HeadphoneOptimizationResult::from(result))
}

/// Load target curve from bundled data or custom file
pub fn load_target_curve(target: &str, custom_path: &str) -> Result<autoeq::Curve, String> {
    match target {
        "harman-over-ear-2018" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2018),
        "harman-over-ear-2015" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2015),
        "harman-over-ear-2013" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2013),
        "harman-in-ear-2019" => parse_csv_curve(target_curves::HARMAN_IN_EAR_2019),
        "custom" => autoeq::read::read_curve_from_csv(&PathBuf::from(custom_path))
            .map_err(|e| format!("Failed to read custom target curve: {}", e)),
        _ => autoeq::read::read_curve_from_csv(&PathBuf::from(custom_path))
            .map_err(|e| format!("A target curve is required for headphone: {}", e)),
    }
}

/// Parse a CSV string into a Curve
pub fn parse_csv_curve(csv_data: &str) -> Result<autoeq::Curve, String> {
    use ndarray::Array1;

    let mut freq = Vec::new();
    let mut spl = Vec::new();

    for (i, line) in csv_data.lines().enumerate() {
        // Skip header line
        if i == 0 && line.contains("frequency") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(f), Ok(s)) = (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
            ) {
                freq.push(f);
                spl.push(s);
            }
        }
    }

    if freq.is_empty() {
        return Err("No valid data found in CSV".to_string());
    }

    Ok(autoeq::Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
        ..Default::default()
    })
}
