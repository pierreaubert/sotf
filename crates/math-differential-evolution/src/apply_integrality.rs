use ndarray::Array1;

pub(crate) fn apply_integrality(
    x: &mut Array1<f64>,
    mask: &[bool],
    lower: &Array1<f64>,
    upper: &Array1<f64>,
) {
    for i in 0..x.len() {
        if i < mask.len() && mask[i] {
            x[i] = x[i].round();
            if x[i] < lower[i] {
                x[i] = lower[i].ceil();
            }
            if x[i] > upper[i] {
                x[i] = upper[i].floor();
            }
        }
    }
}
