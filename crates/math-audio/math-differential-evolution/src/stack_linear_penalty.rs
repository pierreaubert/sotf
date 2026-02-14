use ndarray::{Array1, Array2};

use crate::LinearPenalty;

pub(crate) fn stack_linear_penalty(dst: &mut LinearPenalty, src: &LinearPenalty) {
    // Vertically stack A, lb, ub; pick max weight to enforce strongest among merged
    let a_dst = dst.a.clone();
    let a_src = src.a.clone();
    let rows = a_dst.nrows() + a_src.nrows();
    let cols = a_dst.ncols();
    assert_eq!(
        cols,
        a_src.ncols(),
        "LinearPenalty A width mismatch while stacking"
    );
    let mut a_new = Array2::<f64>::zeros((rows, cols));
    // copy
    for i in 0..a_dst.nrows() {
        for j in 0..cols {
            a_new[(i, j)] = a_dst[(i, j)];
        }
    }
    for i in 0..a_src.nrows() {
        for j in 0..cols {
            a_new[(a_dst.nrows() + i, j)] = a_src[(i, j)];
        }
    }
    let mut lb_new = Array1::<f64>::zeros(rows);
    let mut ub_new = Array1::<f64>::zeros(rows);
    for i in 0..a_dst.nrows() {
        lb_new[i] = dst.lb[i];
        ub_new[i] = dst.ub[i];
    }
    for i in 0..a_src.nrows() {
        lb_new[a_dst.nrows() + i] = src.lb[i];
        ub_new[a_dst.nrows() + i] = src.ub[i];
    }
    dst.a = a_new;
    dst.lb = lb_new;
    dst.ub = ub_new;
    dst.weight = dst.weight.max(src.weight);
}
