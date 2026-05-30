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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_masked_dimensions_and_clamps_to_integer_bounds() {
        let mut x = Array1::from(vec![1.6, 2.2, 9.9, -3.2]);
        let mask = [true, false, true, true];
        let lower = Array1::from(vec![0.0, 0.0, 0.0, -2.5]);
        let upper = Array1::from(vec![10.0, 10.0, 4.2, 10.0]);

        apply_integrality(&mut x, &mask, &lower, &upper);

        assert_eq!(x[0], 2.0);
        assert_eq!(x[1], 2.2);
        assert_eq!(x[2], 4.0);
        assert_eq!(x[3], -2.0);
    }

    #[test]
    fn short_mask_leaves_missing_dimensions_continuous() {
        let mut x = Array1::from(vec![1.6, 2.2]);
        let mask = [true];
        let lower = Array1::from(vec![0.0, 0.0]);
        let upper = Array1::from(vec![10.0, 10.0]);

        apply_integrality(&mut x, &mask, &lower, &upper);

        assert_eq!(x[0], 2.0);
        assert_eq!(x[1], 2.2);
    }
}
