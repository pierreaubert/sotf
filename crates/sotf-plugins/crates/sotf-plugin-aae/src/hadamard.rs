//! 8×8 Hadamard transform using in-place butterfly operations.
//!
//! The Hadamard matrix is orthogonal and energy-preserving, making it ideal
//! as the FDN mixing matrix. All eigenvalues have magnitude 1, so stability
//! depends only on the per-line attenuation coefficients.
//!
//! Computed via 3 stages of butterfly operations: O(N log N) instead of O(N²).

/// Apply the normalized 8×8 Hadamard transform in-place.
///
/// Equivalent to multiplying by H₈/√8 where H₈ is the standard Hadamard matrix.
/// Uses 3 stages of 2-point butterflies (add/subtract pairs).
#[inline]
pub fn hadamard8(x: &mut [f32; 8]) {
    // Stage 1: pairs (0,1), (2,3), (4,5), (6,7)
    butterfly(x, 0, 1);
    butterfly(x, 2, 3);
    butterfly(x, 4, 5);
    butterfly(x, 6, 7);

    // Stage 2: pairs (0,2), (1,3), (4,6), (5,7)
    butterfly(x, 0, 2);
    butterfly(x, 1, 3);
    butterfly(x, 4, 6);
    butterfly(x, 5, 7);

    // Stage 3: pairs (0,4), (1,5), (2,6), (3,7)
    butterfly(x, 0, 4);
    butterfly(x, 1, 5);
    butterfly(x, 2, 6);
    butterfly(x, 3, 7);

    // Normalize by 1/√8 for energy preservation
    let norm = 1.0 / (8.0_f32).sqrt();
    for v in x.iter_mut() {
        *v *= norm;
    }
}

/// 2-point butterfly: (a, b) → (a+b, a-b)
#[inline(always)]
fn butterfly(x: &mut [f32; 8], i: usize, j: usize) {
    let sum = x[i] + x[j];
    let diff = x[i] - x[j];
    x[i] = sum;
    x[j] = diff;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_orthogonality() {
        // H * H^T = I for normalized Hadamard
        // Apply Hadamard twice to a unit vector; should get the original back
        let mut x = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        hadamard8(&mut x);
        hadamard8(&mut x);
        // Should recover [1, 0, 0, 0, 0, 0, 0, 0]
        assert!((x[0] - 1.0).abs() < 1e-5, "x[0] = {}", x[0]);
        for i in 1..8 {
            assert!(x[i].abs() < 1e-5, "x[{i}] = {}", x[i]);
        }
    }

    #[test]
    fn test_hadamard_energy_preservation() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let energy_in: f32 = input.iter().map(|v| v * v).sum();

        let mut x = input;
        hadamard8(&mut x);
        let energy_out: f32 = x.iter().map(|v| v * v).sum();

        assert!(
            (energy_in - energy_out).abs() < 1e-3,
            "Energy in={energy_in}, out={energy_out}"
        );
    }

    #[test]
    fn test_hadamard_uniform_input() {
        // All-ones input: H * [1,1,1,1,1,1,1,1]^T / sqrt(8)
        // Should concentrate energy in first element: sqrt(8) * 1/sqrt(8) = 1.0 in [0]
        // and 0 in all others (since all same value)
        let mut x = [1.0; 8];
        hadamard8(&mut x);
        // x[0] should be sqrt(8) * 1/sqrt(8) = 8/sqrt(8) * 1/sqrt(8) = 8/8 ... no
        // Actually: sum of 8 ones = 8, then normalize by 1/sqrt(8) → 8/sqrt(8) = sqrt(8) ≈ 2.828
        let expected_0 = (8.0_f32).sqrt();
        assert!(
            (x[0] - expected_0).abs() < 1e-4,
            "x[0]={}, expected {}",
            x[0],
            expected_0
        );
        for i in 1..8 {
            assert!(x[i].abs() < 1e-5, "x[{i}] = {} should be 0", x[i]);
        }
    }

    #[test]
    fn test_hadamard_zero_input() {
        let mut x = [0.0; 8];
        hadamard8(&mut x);
        for v in &x {
            assert!(v.abs() < 1e-10);
        }
    }
}
