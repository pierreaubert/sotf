/// Inverse of `quality_to_optimizer_params`: map a population value back to 0.0-1.0.
pub fn population_to_quality(population: usize) -> f32 {
    const POP_MIN: f32 = 30.0;
    const POP_MAX: f32 = 300.0;
    if population as f32 <= POP_MIN {
        0.0
    } else if population as f32 >= POP_MAX {
        1.0
    } else {
        let log_min = POP_MIN.ln();
        let log_max = POP_MAX.ln();
        ((population as f32).ln() - log_min) / (log_max - log_min)
    }
}

pub(super) fn lerp_exp(min: f32, max: f32, t: f32) -> f32 {
    let log_min = min.ln();
    let log_max = max.ln();
    (log_min + t * (log_max - log_min)).exp()
}
