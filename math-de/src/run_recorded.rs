//! Recording wrapper for differential evolution for testing purposes

use crate::recorder::OptimizationRecorder;
use crate::{DEConfig, DEReport, differential_evolution};
use autoeq_env::get_records_dir;
use ndarray::Array1;

/// Run differential evolution with recording of evaluations to CSV
///
/// This wrapper function is primarily used for testing and analysis.
/// It records every function evaluation to CSV files for later analysis.
pub fn run_recorded_differential_evolution<F>(
    function_name: &str,
    func: F,
    bounds: &[(f64, f64)],
    config: DEConfig,
) -> Result<(DEReport, String), Box<dyn std::error::Error>>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync + 'static,
{
    // Get the records directory from AUTOEQ_DIR environment variable
    let records_dir =
        get_records_dir().map_err(|e| format!("Failed to get records directory: {}", e))?;
    let output_dir = records_dir.to_string_lossy().to_string();

    // Create recorder for this optimization run
    let recorder = std::sync::Arc::new(OptimizationRecorder::with_output_dir(
        function_name.to_string(),
        output_dir.to_string(),
    ));

    // Create wrapped objective function that records evaluations
    let recorder_clone = recorder.clone();
    let recorded_func = move |x: &Array1<f64>| -> f64 {
        let f_value = func(x);
        recorder_clone.record_evaluation(x, f_value);
        f_value
    };

    // Run differential evolution with the wrapped function
    let result = differential_evolution(&recorded_func, bounds, config);

    // Finalize recording and get CSV file paths
    let csv_files = recorder.finalize()?;

    // Return the DE result and the primary CSV file path
    let csv_path = if !csv_files.is_empty() {
        csv_files[0].clone()
    } else {
        format!("{}/{}.csv", output_dir, function_name)
    };

    Ok((result, csv_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEConfigBuilder;

    #[test]
    fn test_run_recorded_basic() {
        // Simple quadratic function for testing
        let quadratic = |x: &Array1<f64>| -> f64 { x.iter().map(|&xi| xi * xi).sum() };

        let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(20)
            .popsize(10)
            .build();

        let result =
            run_recorded_differential_evolution("test_quadratic", quadratic, &bounds, config);

        match result {
            Ok((report, csv_path)) => {
                // Should find minimum near origin
                println!("Result: f = {:.6e}, x = {:?}", report.fun, report.x);
                assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
                for &xi in report.x.iter() {
                    assert!(xi.abs() < 1e-1, "Variable too far from 0: {}", xi);
                }

                // CSV file should be created
                println!("CSV saved to: {}", csv_path);
            }
            Err(e) => {
                println!(
                    "Test requires AUTOEQ_DIR to be set. Error: {}\nPlease run: export AUTOEQ_DIR=/Users/pierrre/src.local/autoeq",
                    e
                );
                panic!("Test requires AUTOEQ_DIR to be set.");
            }
        }
    }
}
