//! Transfer operators for multigrid
//!
//! Implements prolongation (coarse-to-fine) and restriction (fine-to-coarse) operators.

use super::hierarchy::TransferMatrix;
use num_complex::Complex64;

/// Prolongate solution from coarse grid to fine grid
///
/// u_fine = P * u_coarse
pub fn prolongate(p: &TransferMatrix, u_coarse: &[Complex64]) -> Vec<Complex64> {
    p.apply(u_coarse)
}

/// Restrict residual from fine grid to coarse grid
///
/// r_coarse = R * r_fine
pub fn restrict(r: &TransferMatrix, r_fine: &[Complex64]) -> Vec<Complex64> {
    r.apply(r_fine)
}

/// Full weighting restriction for structured grids
/// Uses weights based on position relative to coarse nodes
pub fn full_weighting_1d(fine: &[Complex64]) -> Vec<Complex64> {
    let n_fine = fine.len();
    let n_coarse = (n_fine + 1) / 2;
    let mut coarse = vec![Complex64::new(0.0, 0.0); n_coarse];

    for i in 0..n_coarse {
        let fi = 2 * i;
        if fi == 0 {
            coarse[i] = fine[0];
        } else if fi >= n_fine - 1 {
            coarse[i] = fine[n_fine - 1];
        } else {
            // Interior: full weighting 1/4 * (f[fi-1] + 2*f[fi] + f[fi+1])
            coarse[i] = Complex64::new(0.25, 0.0) * fine[fi - 1]
                + Complex64::new(0.5, 0.0) * fine[fi]
                + Complex64::new(0.25, 0.0) * fine[fi + 1];
        }
    }

    coarse
}

/// Linear interpolation for structured grids
pub fn linear_interpolation_1d(coarse: &[Complex64]) -> Vec<Complex64> {
    let n_coarse = coarse.len();
    let n_fine = 2 * n_coarse - 1;
    let mut fine = vec![Complex64::new(0.0, 0.0); n_fine];

    for i in 0..n_coarse {
        fine[2 * i] = coarse[i];
    }

    for i in 0..n_coarse - 1 {
        fine[2 * i + 1] = Complex64::new(0.5, 0.0) * (coarse[i] + coarse[i + 1]);
    }

    fine
}

/// Injection restriction (simple downsampling)
pub fn injection_restrict(fine: &[Complex64], stride: usize) -> Vec<Complex64> {
    fine.iter().step_by(stride).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_weighting_1d() {
        let fine = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(4.0, 0.0),
            Complex64::new(5.0, 0.0),
        ];

        let coarse = full_weighting_1d(&fine);
        assert_eq!(coarse.len(), 3);
        assert!((coarse[0].re - 1.0).abs() < 1e-10); // Boundary
        assert!((coarse[1].re - 3.0).abs() < 1e-10); // Center
    }

    #[test]
    fn test_linear_interpolation_1d() {
        let coarse = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(5.0, 0.0),
        ];

        let fine = linear_interpolation_1d(&coarse);
        assert_eq!(fine.len(), 5);
        assert!((fine[0].re - 1.0).abs() < 1e-10);
        assert!((fine[1].re - 2.0).abs() < 1e-10); // Interpolated
        assert!((fine[2].re - 3.0).abs() < 1e-10);
        assert!((fine[3].re - 4.0).abs() < 1e-10); // Interpolated
        assert!((fine[4].re - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_injection() {
        let fine = vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(4.0, 0.0),
            Complex64::new(5.0, 0.0),
        ];

        let coarse = injection_restrict(&fine, 2);
        assert_eq!(coarse.len(), 3);
        assert!((coarse[0].re - 1.0).abs() < 1e-10);
        assert!((coarse[1].re - 3.0).abs() < 1e-10);
        assert!((coarse[2].re - 5.0).abs() < 1e-10);
    }
}
