use ndarray::Array1;
use rand::Rng;

/// Wrapper Local Search (WLS) strategy for local refinement
/// Uses Cauchy distribution to perturb selected dimensions
pub(crate) fn apply_wls<R: Rng + ?Sized>(
    x: &Array1<f64>,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    scale: f64,
    rng: &mut R,
) -> Array1<f64> {
    let mut result = x.clone();
    let n_dims = x.len();

    // Generate random wrapper mask - selects which dimensions to perturb
    let n_selected = rng.random_range(1..=n_dims.max(1));
    let mut dimensions: Vec<usize> = (0..n_dims).collect();
    use rand::seq::SliceRandom;
    dimensions.shuffle(rng);
    let selected_dims = &dimensions[0..n_selected];

    // Apply normal random perturbation to selected dimensions (simplified)
    for &dim in selected_dims {
        let perturbation = (rng.random::<f64>() - 0.5) * scale * 2.0;
        let new_val = x[dim] + perturbation;
        // Clip to bounds
        result[dim] = new_val.max(lower[dim]).min(upper[dim]);
    }

    result
}
