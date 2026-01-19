use ndarray::Array1;
use rand::Rng;

pub(crate) fn binomial_crossover<R: Rng + ?Sized>(
    target: &Array1<f64>,
    mutant: &Array1<f64>,
    cr: f64,
    rng: &mut R,
) -> Array1<f64> {
    let n = target.len();
    let jrand = rng.random_range(0..n);
    let mut trial = target.clone();
    for j in 0..n {
        if j == jrand || rng.random::<f64>() < cr {
            trial[j] = mutant[j];
        }
    }
    trial
}
