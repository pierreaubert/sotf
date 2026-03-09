use ndarray::Array1;
use rand::Rng;

pub(crate) fn exponential_crossover<R: Rng + ?Sized>(
    target: &Array1<f64>,
    mutant: &Array1<f64>,
    cr: f64,
    rng: &mut R,
) -> Array1<f64> {
    let n = target.len();
    let mut trial = target.clone();
    let mut j = rng.random_range(0..n);
    let mut l = 0usize;
    // ensure at least one parameter from mutant
    loop {
        trial[j] = mutant[j];
        l += 1;
        j = (j + 1) % n;
        if rng.random::<f64>() >= cr || l >= n {
            break;
        }
    }
    trial
}
