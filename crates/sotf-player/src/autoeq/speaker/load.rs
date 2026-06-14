use super::types::MeasurementInput;
use super::types::PreviewCurves;

/// Load a MeasurementInput into an autoeq DriverMeasurement
pub(super) fn load_measurement_as_driver(
    input: &MeasurementInput,
) -> Result<autoeq::loss::DriverMeasurement, String> {
    match input {
        MeasurementInput::CsvFile(path) => {
            let paths = vec![path.clone()];
            let measurements = autoeq::workflow::load_driver_measurements_from_files(&paths)
                .map_err(|e| e.to_string())?;
            measurements
                .into_iter()
                .next()
                .ok_or_else(|| "No measurement loaded from CSV".to_string())
        }
        MeasurementInput::Curve(curve) => {
            let freq = ndarray::Array1::from_vec(curve.freq.iter().copied().collect());
            let spl = ndarray::Array1::from_vec(curve.spl.iter().copied().collect());
            let phase = curve
                .phase
                .as_ref()
                .map(|p| ndarray::Array1::from_vec(p.iter().copied().collect()));
            Ok(autoeq::loss::DriverMeasurement::new(freq, spl, phase))
        }
        MeasurementInput::Spinorama { .. } => {
            Err("Spinorama input not supported for multi-driver optimization".to_string())
        }
    }
}

/// Load and compute preview curves for display before optimization
pub fn load_preview_curves(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(load_preview_curves_async(
        speaker,
        version,
        measurement,
        curve_name,
    ))
}

/// Async version of load_preview_curves
pub async fn load_preview_curves_async(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<PreviewCurves, String> {
    // Load input curve using library function
    let (input_curve, _spin_data) =
        autoeq::load_spinorama_with_spin(speaker, version, measurement, curve_name)
            .await
            .map_err(|e| e.to_string())?;

    // Create standard frequency grid
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Normalize input curve
    let input_normalized = autoeq::normalize_and_interpolate_response(&standard_freq, &input_curve);

    // Build target curve using default args
    let args = autoeq::Args::speaker_defaults();
    let target_curve =
        autoeq::workflow::build_target_curve(&args, &standard_freq, &input_normalized)
            .map_err(|e| e.to_string())?;

    // Compute deviation
    let frequencies: Vec<f64> = standard_freq.iter().copied().collect();
    let input_vec: Vec<f64> = input_normalized.spl.iter().copied().collect();
    let target_vec: Vec<f64> = target_curve.spl.iter().copied().collect();
    let deviation_vec: Vec<f64> = target_vec
        .iter()
        .zip(input_vec.iter())
        .map(|(t, i)| t - i)
        .collect();

    Ok(PreviewCurves {
        frequencies,
        input_curve: input_vec,
        target_curve: target_vec,
        deviation_curve: deviation_vec,
    })
}
