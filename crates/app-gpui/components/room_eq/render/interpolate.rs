pub(super) fn interpolate_log_frequency_at_db(
    lower: (f64, f64),
    upper: (f64, f64),
    target_db: f64,
) -> f64 {
    let denom = upper.1 - lower.1;
    if denom.abs() < 1e-12 {
        return (lower.0 * upper.0).sqrt();
    }
    let t = ((target_db - lower.1) / denom).clamp(0.0, 1.0);
    (lower.0.ln() + t * (upper.0.ln() - lower.0.ln())).exp()
}

/// Interpolate a sampled curve at a single frequency using log-frequency linear interpolation.
pub(super) fn interpolate_value_at(frequencies: &[f64], values: &[f64], target_freq: f64) -> f64 {
    let mut points = frequencies.iter().copied().zip(values.iter().copied());
    let Some((mut prev_freq, mut prev_value)) = points.next() else {
        return 0.0;
    };

    if target_freq <= prev_freq {
        return prev_value;
    }

    for (freq, value) in points {
        if target_freq <= freq {
            let denom = freq.ln() - prev_freq.ln();
            if denom.abs() < 1e-12 {
                return prev_value;
            }
            let t = (target_freq.ln() - prev_freq.ln()) / denom;
            return prev_value + t * (value - prev_value);
        }
        prev_freq = freq;
        prev_value = value;
    }

    prev_value
}
