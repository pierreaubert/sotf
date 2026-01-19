use crate::error::Result;
use crate::Curve;
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Optimize group delay alignment between a subwoofer and a speaker
///
/// Returns the optimal delay (in ms) to apply to the speaker to align it with the subwoofer.
/// A positive value means the speaker should be delayed.
/// A negative value means the speaker is "too late" and the subwoofer should be delayed (or speaker advanced).
pub fn optimize_group_delay(
    sub: &Curve,
    speaker: &Curve,
    min_freq: f64,
    max_freq: f64,
) -> Result<f64> {
    // 1. Convert to complex responses
    // Ensure both curves share the same frequency grid (interpolate speaker to sub)
    // For simplicity, we assume the frequency grids are compatible or dense enough.
    // roomeq usually works with standardized measurements or interpolated ones.
    // We'll interpolate speaker to sub's frequencies for the calculation.

    let freq = &sub.freq;

    // Interpolate speaker SPL and Phase to sub frequencies
    let speaker_interp = interpolate_curve(speaker, freq);

    let sub_complex = curve_to_complex(sub);
    let speaker_complex = curve_to_complex(&speaker_interp);

    // 2. Define search range (e.g. +/- 30ms)
    // Subwoofers can have significant group delay (e.g. 10-20ms).
    // Room reflections can also add delay.
    let range_ms = 30.0;
    let step_ms = 0.5; // Coarse search
    let fine_step_ms = 0.05; // Fine search

    let mut best_delay = 0.0;
    let mut best_score = f64::INFINITY;

    // Coarse grid search
    let start = -range_ms;
    let end = range_ms;
    let mut d = start;
    while d <= end {
        let score = evaluate_delay(d, freq, &sub_complex, &speaker_complex, min_freq, max_freq);
        if score < best_score {
            best_score = score;
            best_delay = d;
        }
        d += step_ms;
    }

    // Fine search around best_delay
    let fine_range = step_ms * 2.0;
    let start = best_delay - fine_range;
    let end = best_delay + fine_range;
    let mut d = start;
    while d <= end {
        let score = evaluate_delay(d, freq, &sub_complex, &speaker_complex, min_freq, max_freq);
        if score < best_score {
            best_score = score;
            best_delay = d;
        }
        d += fine_step_ms;
    }

    Ok(best_delay)
}

fn evaluate_delay(
    delay_ms: f64,
    freq: &Array1<f64>,
    sub: &Array1<Complex64>,
    speaker: &Array1<Complex64>,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    // Apply delay to speaker: exp(-j * 2pi * f * delay)
    // delay is in ms, f in Hz.
    // delay_s = delay_ms / 1000.0

    let delay_s = delay_ms / 1000.0;
    // Pre-allocate the full vector to avoid repeated allocations in the loop
    let mut combined_complex = vec![Complex64::new(0.0, 0.0); freq.len()];

    for (i, ((&f, &sub_val), &speaker_val)) in
        freq.iter().zip(sub.iter()).zip(speaker.iter()).enumerate()
    {
        let w = 2.0 * PI * f;
        let phase_shift = -w * delay_s;
        let rot = Complex64::from_polar(1.0, phase_shift);

        combined_complex[i] = sub_val + speaker_val * rot;
    }

    // Calculate Group Delay of the sum
    let gd = calculate_group_delay(freq, &combined_complex);

    // Calculate score: std dev of GD in [min_freq, max_freq]
    // Filter GD to range
    let mut values = Vec::new();
    for i in 0..freq.len() {
        if freq[i] >= min_freq && freq[i] <= max_freq && gd[i].is_finite() {
            values.push(gd[i]);
        }
    }

    if values.is_empty() {
        return f64::INFINITY;
    }

    // Standard Deviation
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn calculate_group_delay(freq: &Array1<f64>, complex: &[Complex64]) -> Vec<f64> {
    // GD = -d(phi)/dw
    // dw = 2pi * df
    // phi is unwrapped phase

    let mut phases = Vec::with_capacity(complex.len());
    for c in complex {
        phases.push(c.arg());
    }

    let unwrapped = unwrap_phase(&phases);
    let mut gd = vec![0.0; freq.len()];

    // Finite difference
    for i in 0..freq.len() - 1 {
        let d_phi = unwrapped[i + 1] - unwrapped[i];
        let d_f = freq[i + 1] - freq[i];
        let d_w = 2.0 * PI * d_f;

        if d_w.abs() > 1e-9 {
            gd[i] = -d_phi / d_w; // Result in seconds
        }
    }
    // Fill last point
    if freq.len() > 1 {
        gd[freq.len() - 1] = gd[freq.len() - 2];
    }

    // Convert to ms for easier reasoning, though for std dev it doesn't matter (just scaling)
    gd.iter().map(|v| v * 1000.0).collect()
}

fn unwrap_phase(phase: &[f64]) -> Vec<f64> {
    let mut unwrapped = Vec::with_capacity(phase.len());
    if phase.is_empty() {
        return unwrapped;
    }

    let mut prev = phase[0];
    unwrapped.push(prev);
    let mut offset = 0.0;

    for &p in phase.iter().skip(1) {
        let diff = p - prev;
        if diff > PI {
            offset -= 2.0 * PI;
        } else if diff < -PI {
            offset += 2.0 * PI;
        }
        unwrapped.push(p + offset);
        prev = p;
    }
    unwrapped
}

fn curve_to_complex(curve: &Curve) -> Array1<Complex64> {
    let mut out = Array1::default(curve.spl.len());
    for i in 0..curve.spl.len() {
        let mag = 10.0_f64.powf(curve.spl[i] / 20.0);
        // Phase is optional, default to 0 if missing (minimum phase approx could be better but this is simple)
        let phase_deg = curve.phase.as_ref().map(|p| p[i]).unwrap_or(0.0);
        let phase_rad = phase_deg.to_radians();
        out[i] = Complex64::from_polar(mag, phase_rad);
    }
    out
}

fn interpolate_curve(curve: &Curve, target_freq: &Array1<f64>) -> Curve {
    // Interpolate in Complex domain to avoid phase wrapping issues
    let complex_in = curve_to_complex(curve);

    let mut spl = Array1::zeros(target_freq.len());
    let mut phase = Array1::zeros(target_freq.len());
    let has_phase = curve.phase.is_some();

    for (i, &f) in target_freq.iter().enumerate() {
        // Interpolate Real and Imaginary parts
        let re = interp_linear_complex(&curve.freq, &complex_in, f, |c| c.re);
        let im = interp_linear_complex(&curve.freq, &complex_in, f, |c| c.im);
        let c = Complex64::new(re, im);

        spl[i] = 20.0 * c.norm().max(1e-12).log10();
        if has_phase {
            phase[i] = c.arg().to_degrees();
        }
    }

    Curve {
        freq: target_freq.clone(),
        spl,
        phase: if has_phase { Some(phase) } else { None },
    }
}

fn interp_linear_complex<F>(
    x: &Array1<f64>,
    y: &Array1<Complex64>,
    target: f64,
    extractor: F,
) -> f64
where
    F: Fn(&Complex64) -> f64,
{
    if target <= x[0] {
        return extractor(&y[0]);
    }
    if target >= x[x.len() - 1] {
        return extractor(&y[y.len() - 1]);
    }

    // Binary search
    let idx = match x
        .as_slice()
        .unwrap()
        .binary_search_by(|v| v.partial_cmp(&target).unwrap())
    {
        Ok(i) => i,
        Err(i) => i - 1,
    };

    let x0 = x[idx];
    let x1 = x[idx + 1];
    let y0 = extractor(&y[idx]);
    let y1 = extractor(&y[idx + 1]);

    let t = (target - x0) / (x1 - x0);
    y0 + t * (y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_unwrap_phase() {
        // -170, -175, 175 (wrap), 170
        // unwrapped: -170, -175, -185, -190
        let phase = vec![
            -170.0_f64.to_radians(),
            -175.0_f64.to_radians(),
            175.0_f64.to_radians(),
            170.0_f64.to_radians(),
        ];
        let unwrapped = unwrap_phase(&phase);

        let expected = vec![-170.0, -175.0, -185.0, -190.0];
        for (u, e) in unwrapped.iter().zip(expected.iter()) {
            assert!(
                (u.to_degrees() - e).abs() < 1e-5,
                "Got {}, expected {}",
                u.to_degrees(),
                e
            );
        }
    }

    #[test]
    fn test_calculate_group_delay_constant() {
        // Linear phase: phi = -w * T
        // GD = T
        let delay_s = 0.010; // 10ms
        let freqs = Array1::linspace(20.0, 100.0, 10);
        let mut complex = Vec::new();

        for &f in &freqs {
            let w = 2.0 * PI * f;
            let phi = -w * delay_s;
            complex.push(Complex64::from_polar(1.0, phi));
        }

        let gd = calculate_group_delay(&freqs, &complex);

        for &d in &gd {
            // gd is in ms
            assert!((d - 10.0).abs() < 0.1, "Expected 10ms, got {}", d);
        }
    }

    #[test]
    fn test_optimize_group_delay_alignment() {
        // Simulate LR2 Crossover at 80 Hz
        // LP: 1 / (1 + j f/fc)
        // HP: (j f/fc) / (1 + j f/fc)
        // Perfect sum = 1.
        // If we delay HP, we get GD variance.

        let fc = 80.0;
        let freqs = Array1::linspace(20.0, 200.0, 100);
        let mut sub_spl = Array1::zeros(freqs.len());
        let mut sub_phase = Array1::zeros(freqs.len());
        let mut spk_spl = Array1::zeros(freqs.len());
        let mut spk_phase = Array1::zeros(freqs.len());

        // Target delay to find: 5ms
        // We simulate that the Speaker (HP) is physically EARLIER (0 delay)
        // and the Sub (LP) is LATER (has extra 5ms delay on top of filter)
        // Optimization should tell us to delay Speaker by 5ms.
        let sub_extra_delay_s = 0.005;

        for i in 0..freqs.len() {
            let f = freqs[i];
            let w = 2.0 * PI * f;
            let s = Complex64::new(0.0, f / fc);

            // LP Filter
            let lp = Complex64::new(1.0, 0.0) / (Complex64::new(1.0, 0.0) + s);
            // Apply extra delay to sub
            let sub_rot = Complex64::from_polar(1.0, -w * sub_extra_delay_s);
            let sub_final = lp * sub_rot;

            sub_spl[i] = 20.0 * sub_final.norm().log10();
            sub_phase[i] = sub_final.arg().to_degrees();

            // HP Filter (no extra delay)
            let hp = s / (Complex64::new(1.0, 0.0) + s);
            spk_spl[i] = 20.0 * hp.norm().log10();
            spk_phase[i] = hp.arg().to_degrees();
        }

        let sub = Curve {
            freq: freqs.clone(),
            spl: sub_spl,
            phase: Some(sub_phase),
        };
        let spk = Curve {
            freq: freqs.clone(),
            spl: spk_spl,
            phase: Some(spk_phase),
        };

        // Optimize in crossover region
        let delay = optimize_group_delay(&sub, &spk, 40.0, 160.0).unwrap();

        // Expect delay ~ 5.0ms
        assert!((delay - 5.0).abs() < 0.25, "Expected 5.0ms, got {}", delay);
    }
}
