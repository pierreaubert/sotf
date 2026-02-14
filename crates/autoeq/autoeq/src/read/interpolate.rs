use crate::Curve;
use ndarray::Array1;

/// Helper to interpolate a single value array in log frequency space
fn interpolate_log_space_vals(
    log_freq_out: &[f64],
    log_freq_in: &[f64],
    vals_in: &Array1<f64>,
) -> Array1<f64> {
    let n_out = log_freq_out.len();
    let n_in = log_freq_in.len();
    let mut vals_out = Array1::zeros(n_out);

    for i in 0..n_out {
        let target_log_freq = log_freq_out[i];

        // Find surrounding points for interpolation
        if target_log_freq <= log_freq_in[0] {
            // Extrapolate from first two points
            if n_in >= 2 {
                let slope = (vals_in[1] - vals_in[0]) / (log_freq_in[1] - log_freq_in[0]);
                vals_out[i] = vals_in[0] + slope * (target_log_freq - log_freq_in[0]);
            } else {
                vals_out[i] = vals_in[0];
            }
        } else if target_log_freq >= log_freq_in[n_in - 1] {
            // Extrapolate from last two points
            if n_in >= 2 {
                let slope = (vals_in[n_in - 1] - vals_in[n_in - 2])
                    / (log_freq_in[n_in - 1] - log_freq_in[n_in - 2]);
                vals_out[i] = vals_in[n_in - 1] + slope * (target_log_freq - log_freq_in[n_in - 1]);
            } else {
                vals_out[i] = vals_in[n_in - 1];
            }
        } else {
            // Linear interpolation between surrounding points
            let mut j = 0;
            while j < n_in - 1 && log_freq_in[j + 1] < target_log_freq {
                j += 1;
            }

            // Interpolate between j and j+1
            let t = (target_log_freq - log_freq_in[j]) / (log_freq_in[j + 1] - log_freq_in[j]);
            vals_out[i] = vals_in[j] * (1.0 - t) + vals_in[j + 1] * t;
        }
    }
    vals_out
}

/// Interpolate frequency response to a standard grid using linear interpolation in log space
///
/// # Arguments
/// * `freq_in` - Input frequency points
/// * `spl_in` - Input SPL values
/// * `freq_out` - Target frequency grid for interpolation
///
/// # Returns
/// * Interpolated SPL values on the target grid
pub fn interpolate_log_space(freq_out: &Array1<f64>, curve: &Curve) -> Curve {
    let freq_in = &curve.freq;

    // Convert to log space for interpolation
    let log_freq_in: Vec<f64> = freq_in.iter().map(|f| f.ln()).collect();
    let log_freq_out: Vec<f64> = freq_out.iter().map(|f| f.ln()).collect();

    let spl_out = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &curve.spl);

    let phase_out = curve
        .phase
        .as_ref()
        .map(|p| interpolate_log_space_vals(&log_freq_out, &log_freq_in, p));

    Curve {
        freq: freq_out.clone(),
        spl: spl_out,
        phase: phase_out,
    }
}

/// Create a standard logarithmic frequency grid
pub fn create_log_frequency_grid(n_points: usize, f_min: f64, f_max: f64) -> Array1<f64> {
    Array1::logspace(10.0, f_min.log10(), f_max.log10(), n_points)
}

/// Linear interpolation function
///
/// # Arguments
/// * `target_freqs` - Target frequencies to interpolate to
/// * `source_freqs` - Source frequency array
/// * `source_spls` - Source SPL values
///
/// # Returns
/// * Interpolated SPL values at target frequencies
pub fn interpolate(freqs: &Array1<f64>, curve: &Curve) -> Curve {
    let mut result_spl = Array1::zeros(freqs.len());
    let mut result_phase = curve.phase.as_ref().map(|_| Array1::zeros(freqs.len()));

    for (i, &target_freq) in freqs.iter().enumerate() {
        // Find the two nearest points in the source data
        let mut left_idx = 0;
        let mut right_idx = curve.freq.len() - 1;

        // Binary search for the closest points
        if target_freq <= curve.freq[0] {
            // Target frequency is below the range, use the first point
            result_spl[i] = curve.spl[0];
            if let (Some(res_p), Some(src_p)) = (result_phase.as_mut(), &curve.phase) {
                res_p[i] = src_p[0];
            }
        } else if target_freq >= curve.freq[curve.freq.len() - 1] {
            // Target frequency is above the range, use the last point
            result_spl[i] = curve.spl[curve.freq.len() - 1];
            if let (Some(res_p), Some(src_p)) = (result_phase.as_mut(), &curve.phase) {
                res_p[i] = src_p[curve.freq.len() - 1];
            }
        } else {
            // Find the two points that bracket the target frequency
            for j in 1..curve.freq.len() {
                if curve.freq[j] >= target_freq {
                    left_idx = j - 1;
                    right_idx = j;
                    break;
                }
            }

            // Linear interpolation
            let freq_left = curve.freq[left_idx];
            let freq_right = curve.freq[right_idx];
            let t = (target_freq - freq_left) / (freq_right - freq_left);

            let spl_left = curve.spl[left_idx];
            let spl_right = curve.spl[right_idx];
            result_spl[i] = spl_left + t * (spl_right - spl_left);

            if let (Some(res_p), Some(src_p)) = (result_phase.as_mut(), &curve.phase) {
                let p_left = src_p[left_idx];
                let p_right = src_p[right_idx];
                res_p[i] = p_left + t * (p_right - p_left);
            }
        }
    }

    Curve {
        freq: freqs.clone(),
        spl: result_spl,
        phase: result_phase,
    }
}
