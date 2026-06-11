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
                let denom = log_freq_in[1] - log_freq_in[0];
                if denom.abs() < 1e-10 {
                    vals_out[i] = vals_in[0];
                } else {
                    let slope = (vals_in[1] - vals_in[0]) / denom;
                    vals_out[i] = vals_in[0] + slope * (target_log_freq - log_freq_in[0]);
                }
            } else {
                vals_out[i] = vals_in[0];
            }
        } else if target_log_freq >= log_freq_in[n_in - 1] {
            // Extrapolate from last two points
            if n_in >= 2 {
                let denom = log_freq_in[n_in - 1] - log_freq_in[n_in - 2];
                if denom.abs() < 1e-10 {
                    vals_out[i] = vals_in[n_in - 1];
                } else {
                    let slope = (vals_in[n_in - 1] - vals_in[n_in - 2]) / denom;
                    vals_out[i] =
                        vals_in[n_in - 1] + slope * (target_log_freq - log_freq_in[n_in - 1]);
                }
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
            let denom = log_freq_in[j + 1] - log_freq_in[j];
            if denom.abs() < 1e-10 {
                vals_out[i] = vals_in[j];
            } else {
                let t = (target_log_freq - log_freq_in[j]) / denom;
                vals_out[i] = vals_in[j] * (1.0 - t) + vals_in[j + 1] * t;
            }
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

    // Convert to log space for interpolation (clamp to 1e-6 to avoid ln(0) = -inf)
    let log_freq_in: Vec<f64> = freq_in.iter().map(|&f| f.max(1e-6).ln()).collect();
    let log_freq_out: Vec<f64> = freq_out.iter().map(|&f| f.max(1e-6).ln()).collect();

    let spl_out = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &curve.spl);

    let phase_out = curve
        .phase
        .as_ref()
        .map(|p| interpolate_log_space_vals(&log_freq_out, &log_freq_in, p));
    let coherence_out = curve
        .coherence
        .as_ref()
        .map(|c| interpolate_log_space_vals(&log_freq_out, &log_freq_in, c));
    let noise_floor_db_out = curve
        .noise_floor_db
        .as_ref()
        .map(|n| interpolate_log_space_vals(&log_freq_out, &log_freq_in, n));

    Curve {
        freq: freq_out.clone(),
        spl: spl_out,
        phase: phase_out,
        coherence: coherence_out,
        noise_floor_db: noise_floor_db_out,
        ..Default::default()
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
    debug_assert!(
        curve
            .freq
            .as_slice()
            .unwrap()
            .windows(2)
            .all(|w| w[0] <= w[1]),
        "interpolate() requires sorted frequencies"
    );
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
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_log_space_zero_freq() {
        let curve = Curve {
            freq: Array1::from_vec(vec![0.0, 100.0, 1000.0, 10000.0]),
            spl: Array1::from_vec(vec![80.0, 85.0, 90.0, 88.0]),
            phase: None,
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![50.0, 500.0, 5000.0]);
        let result = interpolate_log_space(&freq_out, &curve);
        // No NaN or Inf in output
        for &v in result.spl.iter() {
            assert!(
                v.is_finite(),
                "interpolate_log_space produced non-finite value: {}",
                v
            );
        }
    }

    #[test]
    fn interpolate_log_space_vals_exact_match() {
        let log_freq_in = vec![1.0_f64.ln(), 10.0_f64.ln(), 100.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![0.0, 1.0, 2.0]);
        let log_freq_out = vec![1.0_f64.ln(), 10.0_f64.ln(), 100.0_f64.ln()];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert_eq!(result.len(), 3);
        assert!((result[0] - 0.0).abs() < 1e-12);
        assert!((result[1] - 1.0).abs() < 1e-12);
        assert!((result[2] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_log_space_vals_interior() {
        let log_freq_in = vec![1.0_f64.ln(), 100.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![0.0, 2.0]);
        let log_freq_out = vec![10.0_f64.ln()];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert_eq!(result.len(), 1);
        // log10(10)=1 is midpoint between log10(1)=0 and log10(100)=2
        assert!((result[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_log_space_vals_extrapolate_below() {
        let log_freq_in = vec![10.0_f64.ln(), 100.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![1.0, 2.0]);
        let log_freq_out = vec![1.0_f64.ln()];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert_eq!(result.len(), 1);
        // slope = 1/ln(10); result = 1 + slope*(ln(1)-ln(10)) = 0
        assert!((result[0] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_log_space_vals_extrapolate_above() {
        let log_freq_in = vec![1.0_f64.ln(), 10.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![0.0, 1.0]);
        let log_freq_out = vec![100.0_f64.ln()];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert_eq!(result.len(), 1);
        // slope = 1/ln(10); result = 1 + slope*(ln(100)-ln(10)) = 2
        assert!((result[0] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_log_space_vals_single_point() {
        let log_freq_in = vec![100.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![5.0]);
        let log_freq_out = vec![1.0_f64.ln(), 100.0_f64.ln(), 1000.0_f64.ln()];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert_eq!(result.len(), 3);
        for v in result.iter() {
            assert!((v - 5.0).abs() < 1e-12);
        }
    }

    #[test]
    fn interpolate_log_space_vals_zero_denom() {
        // Equal log frequencies should not panic
        let log_freq_in = vec![1.0, 1.0, 2.0];
        let vals_in = Array1::from_vec(vec![0.0, 1.0, 2.0]);
        let log_freq_out = vec![1.5];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert!(result[0].is_finite());
    }

    #[test]
    fn interpolate_log_space_vals_empty_out() {
        let log_freq_in = vec![1.0_f64.ln(), 10.0_f64.ln()];
        let vals_in = Array1::from_vec(vec![0.0, 1.0]);
        let log_freq_out: Vec<f64> = vec![];
        let result = interpolate_log_space_vals(&log_freq_out, &log_freq_in, &vals_in);
        assert!(result.is_empty());
    }

    #[test]
    fn interpolate_log_space_vals_preserves_phase() {
        let freq_in = Array1::from_vec(vec![100.0, 1000.0, 10000.0]);
        let curve = Curve {
            freq: freq_in.clone(),
            spl: Array1::from_vec(vec![0.0, 1.0, 2.0]),
            phase: Some(Array1::from_vec(vec![10.0, 20.0, 30.0])),
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![100.0, 1000.0, 10000.0]);
        let result = interpolate_log_space(&freq_out, &curve);
        assert!(result.phase.is_some());
        let phase = result.phase.unwrap();
        assert!((phase[0] - 10.0).abs() < 1e-9);
        assert!((phase[1] - 20.0).abs() < 1e-9);
        assert!((phase[2] - 30.0).abs() < 1e-9);
    }

    #[test]
    fn create_log_frequency_grid_bounds_and_length() {
        let grid = create_log_frequency_grid(50, 20.0, 20000.0);
        assert_eq!(grid.len(), 50);
        assert!((grid[0] - 20.0).abs() < 1e-9, "first point should be f_min");
        assert!(
            (grid[49] - 20000.0).abs() < 1e-9,
            "last point should be f_max"
        );
        // Log-spaced means ratio between consecutive points is constant
        let ratio0 = grid[1] / grid[0];
        let ratio1 = grid[2] / grid[1];
        assert!((ratio0 - ratio1).abs() < 1e-6, "grid should be log-spaced");
    }

    #[test]
    fn interpolate_linear_exact_match() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0, 10000.0]),
            spl: Array1::from_vec(vec![0.0, 10.0, 5.0]),
            phase: None,
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![100.0, 1000.0, 10000.0]);
        let result = interpolate(&freq_out, &curve);
        assert!((result.spl[0] - 0.0).abs() < 1e-12);
        assert!((result.spl[1] - 10.0).abs() < 1e-12);
        assert!((result.spl[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_linear_interior() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 200.0]),
            spl: Array1::from_vec(vec![0.0, 10.0]),
            phase: None,
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![150.0]);
        let result = interpolate(&freq_out, &curve);
        assert!((result.spl[0] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_linear_extrapolates_below() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 200.0]),
            spl: Array1::from_vec(vec![0.0, 10.0]),
            phase: None,
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![50.0]);
        let result = interpolate(&freq_out, &curve);
        // Below range uses first point
        assert!((result.spl[0] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_linear_extrapolates_above() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 200.0]),
            spl: Array1::from_vec(vec![0.0, 10.0]),
            phase: None,
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![300.0]);
        let result = interpolate(&freq_out, &curve);
        // Above range uses last point
        assert!((result.spl[0] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn interpolate_linear_preserves_phase() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 200.0]),
            spl: Array1::from_vec(vec![0.0, 10.0]),
            phase: Some(Array1::from_vec(vec![0.0, 90.0])),
            ..Default::default()
        };
        let freq_out = Array1::from_vec(vec![150.0]);
        let result = interpolate(&freq_out, &curve);
        assert!(result.phase.is_some());
        let phase = result.phase.unwrap();
        assert!((phase[0] - 45.0).abs() < 1e-12);
    }
}
