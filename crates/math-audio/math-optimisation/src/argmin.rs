use ndarray::Array1;

/// Return the index and value of the smallest *finite* element.
/// Non-finite entries (`NaN`, `inf`, `-inf`) are treated as `+inf` so
/// they can never be selected as the minimum.
pub(crate) fn argmin(v: &Array1<f64>) -> (usize, f64) {
    let mut best_i = 0usize;
    let mut best_v = if v[0].is_finite() {
        v[0]
    } else {
        f64::INFINITY
    };
    for (i, &val) in v.iter().enumerate().skip(1) {
        let finite_val = if val.is_finite() { val } else { f64::INFINITY };
        if finite_val < best_v {
            best_v = finite_val;
            best_i = i;
        }
    }
    (best_i, best_v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argmin_basic() {
        let v = Array1::from(vec![3.0, 1.0, 2.0]);
        assert_eq!(argmin(&v), (1, 1.0));
    }

    #[test]
    fn argmin_skips_nan() {
        let v = Array1::from(vec![f64::NAN, 5.0, 1.0]);
        let (i, val) = argmin(&v);
        assert_eq!(i, 2);
        assert_eq!(val, 1.0);
    }

    #[test]
    fn argmin_skips_inf() {
        let v = Array1::from(vec![f64::INFINITY, 3.0, f64::NEG_INFINITY]);
        let (i, val) = argmin(&v);
        assert_eq!(i, 1);
        assert_eq!(val, 3.0);
    }

    #[test]
    fn argmin_all_nan() {
        let v = Array1::from(vec![f64::NAN, f64::NAN]);
        let (i, val) = argmin(&v);
        assert_eq!(i, 0);
        assert_eq!(val, f64::INFINITY);
    }
}
