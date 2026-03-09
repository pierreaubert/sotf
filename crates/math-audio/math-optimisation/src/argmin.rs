use ndarray::Array1;

pub(crate) fn argmin(v: &Array1<f64>) -> (usize, f64) {
    let mut best_i = 0usize;
    let mut best_v = v[0];
    for (i, &val) in v.iter().enumerate() {
        if val < best_v {
            best_v = val;
            best_i = i;
        }
    }
    (best_i, best_v)
}
