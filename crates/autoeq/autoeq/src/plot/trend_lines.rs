use ndarray::Array1;

/// Calculate linear regression slope and intercept for a curve between two frequencies
///
/// # Arguments
/// * `freqs` - Frequency points
/// * `values` - Corresponding values (SPL)
/// * `f_min` - Minimum frequency for regression (Hz)
/// * `f_max` - Maximum frequency for regression (Hz)
///
/// # Returns
/// Option<(slope, intercept)> where slope is in dB/octave and intercept is at 1kHz
pub fn calculate_tonal_balance(
    freqs: &Array1<f64>,
    values: &Array1<f64>,
    f_min: f64,
    f_max: f64,
) -> Option<(f64, f64)> {
    let mut x_vals = Vec::new();
    let mut y_vals = Vec::new();

    // Collect points within the frequency range
    for (i, &f) in freqs.iter().enumerate() {
        if f >= f_min && f <= f_max {
            // Use log2(f) for octave-based slope calculation
            x_vals.push(f.log2());
            y_vals.push(values[i]);
        }
    }

    if x_vals.len() < 2 {
        return None;
    }

    // Calculate linear regression: y = a*x + b
    // where x is log2(f) and y is SPL
    let n = x_vals.len() as f64;
    let sum_x: f64 = x_vals.iter().sum();
    let sum_y: f64 = y_vals.iter().sum();
    let sum_xy: f64 = x_vals.iter().zip(y_vals.iter()).map(|(x, y)| x * y).sum();
    let sum_x2: f64 = x_vals.iter().map(|x| x * x).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-12 {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y * sum_x2 - sum_x * sum_xy) / denom;

    // Convert slope from dB/log2(f) to dB/octave
    let slope_octave = slope;

    // Calculate intercept at 1kHz (log2(1000) = 9.965784...)
    let intercept_1khz = intercept + slope * 1000f64.log2();

    Some((slope_octave, intercept_1khz))
}

/// Generate points for a linear regression line
///
/// # Arguments
/// * `slope` - Slope in dB/octave
/// * `intercept` - Intercept at 1kHz
/// * `freqs` - Frequency points to evaluate the line at
///
/// # Returns
/// Array of SPL values for the regression line
pub fn generate_regression_line(slope: f64, intercept: f64, freqs: &Array1<f64>) -> Array1<f64> {
    freqs.mapv(|f| intercept + slope * (f / 1000.0).log2())
}

/// Create a scatter plot trace for a regression line
///
/// # Arguments
/// * `freqs` - Frequency points
/// * `values` - SPL values for the regression line
/// * `name` - Name for the trace
/// * `color` - Color for the line
/// * `x_axis` - X axis identifier (optional)
/// * `y_axis` - Y axis identifier (optional)
///
/// # Returns
/// Scatter plot trace
pub fn create_regression_trace(
    freqs: &Array1<f64>,
    values: &Array1<f64>,
    name: &str,
    color: &str,
    x_axis: Option<&str>,
    y_axis: Option<&str>,
) -> plotly::Scatter<f64, f64> {
    let mut trace = plotly::Scatter::new(freqs.to_vec(), values.to_vec())
        .mode(plotly::common::Mode::Lines)
        .name(name)
        .line(
            plotly::common::Line::new()
                .color(color.to_string())
                .width(3.0),
        );

    if let Some(axis) = x_axis {
        trace = trace.x_axis(axis);
    }

    if let Some(axis) = y_axis {
        trace = trace.y_axis(axis);
    }

    *trace
}
