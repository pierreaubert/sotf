use ndarray::Array1;

/// Clamp only positive dB values to +max_db, leave negatives unchanged
///
/// # Arguments
/// * `arr` - Array of SPL values
/// * `max_db` - Maximum positive dB value
///
/// # Returns
/// * Array with positive values clamped to max_db
pub fn clamp_positive_only(arr: &Array1<f64>, max_db: f64) -> Array1<f64> {
    arr.mapv(|v| if v > 0.0 { v.min(max_db) } else { v })
}

#[cfg(test)]
mod tests {
    use super::clamp_positive_only;
    use ndarray::Array1;

    #[test]
    fn clamp_positive_only_clamps_only_positive_side() {
        let arr = Array1::from(vec![-15.0, -1.0, 0.0, 1.0, 10.0, 25.0]);
        let out = clamp_positive_only(&arr, 12.0);
        assert_eq!(out.to_vec(), vec![-15.0, -1.0, 0.0, 1.0, 10.0, 12.0]);
    }
}
