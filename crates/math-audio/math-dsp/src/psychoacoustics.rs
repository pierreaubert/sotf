//! Psychoacoustic and expensive-objective DSP helpers.
//!
//! These functions are reusable building blocks for perceptual losses. They do
//! not depend on any optimiser or AutoEQ-specific parameter layout.

/// Standard Bark band center frequencies in Hz for 24 critical bands.
pub const BARK_CENTER_FREQUENCIES: [f64; 24] = [
    50.0, 150.0, 250.0, 350.0, 450.0, 570.0, 700.0, 840.0, 1000.0, 1170.0, 1370.0, 1600.0, 1850.0,
    2150.0, 2500.0, 2900.0, 3400.0, 4000.0, 4800.0, 5800.0, 7000.0, 8500.0, 10500.0, 13500.0,
];

/// Standard Bark band edge frequencies in Hz, 25 edges defining 24 bands.
pub const BARK_BAND_EDGES: [f64; 25] = [
    0.0, 100.0, 200.0, 300.0, 400.0, 510.0, 630.0, 770.0, 920.0, 1080.0, 1270.0, 1480.0, 1720.0,
    2000.0, 2320.0, 2700.0, 3150.0, 3700.0, 4400.0, 5300.0, 6400.0, 7700.0, 9500.0, 12000.0,
    15500.0,
];

/// Outer/middle ear transfer function in dB relative to free-field.
pub const OUTER_EAR_TF: [f64; 24] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 6.0, 7.0, 9.0, 10.0, 9.0,
    7.0, 5.0, 3.0, 2.0, 0.0,
];

/// Absolute threshold of hearing in dB SPL per Bark band.
pub const THRESHOLD_IN_QUIET: [f64; 24] = [
    40.0, 30.0, 22.0, 17.0, 14.0, 10.0, 7.0, 5.0, 4.0, 4.0, 4.0, 4.0, 5.0, 6.0, 7.0, 9.0, 12.0,
    15.0, 19.0, 25.0, 33.0, 44.0, 58.0, 80.0,
];

/// Sharpness weighting function per Bark band.
pub const SHARPNESS_WEIGHT: [f64; 24] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.2, 1.5, 1.8, 2.2,
    2.7, 3.3, 4.0, 5.0, 6.2,
];

const ROUGHNESS_B1: f64 = 3.5;
const ROUGHNESS_B2: f64 = 5.75;
const ROUGHNESS_S_NUMERATOR: f64 = 0.24;
const ROUGHNESS_S_FREQUENCY_FACTOR: f64 = 0.0207;
const ROUGHNESS_S_OFFSET: f64 = 18.96;

/// Cached perceptual features derived from a frequency response.
#[derive(Debug, Clone)]
pub struct PsychoacousticFeatures {
    /// Average SPL per Bark band.
    pub bark_levels: [f64; 24],
    /// Specific loudness in sone/Bark.
    pub specific_loudness: [f64; 24],
    /// Total loudness in sone.
    pub total_loudness_sone: f64,
    /// Zwicker sharpness in acum.
    pub sharpness_acum: f64,
    /// Pairwise sensory roughness estimate.
    pub roughness: f64,
}

impl PsychoacousticFeatures {
    /// Compute reusable psychoacoustic features from a frequency response.
    ///
    /// The response is calibrated so its interpolated 1 kHz level equals
    /// `listening_level_phon`, matching the definition of phon at 1 kHz.
    pub fn from_response(freqs: &[f64], spl_db: &[f64], listening_level_phon: f64) -> Self {
        let calibrated_spl = calibrate_to_listening_level(freqs, spl_db, listening_level_phon);
        let bark_levels = bark_spectrum(freqs, &calibrated_spl);
        let specific_loudness = specific_loudness(freqs, spl_db, listening_level_phon);
        let total_loudness_sone = total_loudness(&specific_loudness);
        let sharpness_acum = sharpness(&specific_loudness);
        let roughness = roughness_from_spectrum(freqs, &calibrated_spl);
        Self {
            bark_levels,
            specific_loudness,
            total_loudness_sone,
            sharpness_acum,
            roughness,
        }
    }
}

/// Convert frequency in Hz to Bark scale using Zwicker's formula.
pub fn hz_to_bark(f: f64) -> f64 {
    13.0 * (0.00076 * f).atan() + 3.5 * ((f / 7500.0).powi(2)).atan()
}

/// Convert Bark scale value back to Hz using Newton-Raphson iteration.
pub fn bark_to_hz(z: f64) -> f64 {
    let mut f = 600.0 * z.sinh() / 7.0;
    f = f.clamp(1.0, 25000.0);
    for _ in 0..5 {
        let z_est = hz_to_bark(f);
        let err = z_est - z;
        let delta = f * 1e-6 + 0.01;
        let dz_df = (hz_to_bark(f + delta) - z_est) / delta;
        if dz_df.abs() < 1e-15 {
            break;
        }
        f -= err / dz_df;
        f = f.clamp(1.0, 25000.0);
    }
    f
}

/// Critical bandwidth in Hz at a given center frequency.
pub fn critical_bandwidth(f: f64) -> f64 {
    25.0 + 75.0 * (1.0 + 1.4 * (f / 1000.0).powi(2)).powf(0.69)
}

fn calibrate_to_listening_level(
    freqs: &[f64],
    spl_db: &[f64],
    listening_level_phon: f64,
) -> Vec<f64> {
    let mut points = freqs
        .iter()
        .zip(spl_db.iter())
        .filter_map(|(&f, &level)| {
            (f.is_finite() && f > 0.0 && level.is_finite()).then_some((f, level))
        })
        .collect::<Vec<_>>();
    if points.is_empty() {
        return Vec::new();
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let reference = interpolate_log_frequency(&points, 1000.0).unwrap_or(points[0].1);
    let offset = listening_level_phon.clamp(-20.0, 140.0) - reference;
    spl_db
        .iter()
        .map(|&level| {
            if level.is_finite() {
                level + offset
            } else {
                -100.0
            }
        })
        .collect()
}

fn interpolate_log_frequency(points: &[(f64, f64)], target_hz: f64) -> Option<f64> {
    if points.is_empty() || target_hz <= 0.0 {
        return None;
    }
    if target_hz <= points[0].0 {
        return Some(points[0].1);
    }
    for pair in points.windows(2) {
        let (f0, y0) = pair[0];
        let (f1, y1) = pair[1];
        if target_hz <= f1 {
            let denom = f1.ln() - f0.ln();
            if denom.abs() < 1e-12 {
                return Some(y0);
            }
            let t = ((target_hz.ln() - f0.ln()) / denom).clamp(0.0, 1.0);
            return Some(y0 + t * (y1 - y0));
        }
    }
    points.last().map(|(_, level)| *level)
}

/// Decompose a frequency response into 24 Bark bands by energy-averaging SPL.
pub fn bark_spectrum(freqs: &[f64], spl_db: &[f64]) -> [f64; 24] {
    let mut result = [-100.0_f64; 24];
    for band in 0..24 {
        let lo = BARK_BAND_EDGES[band];
        let hi = BARK_BAND_EDGES[band + 1];
        let mut energy_sum = 0.0_f64;
        let mut count = 0u32;
        for (&f, &level) in freqs.iter().zip(spl_db.iter()) {
            if f >= lo && f < hi {
                energy_sum += 10.0_f64.powf(level / 10.0);
                count += 1;
            }
        }
        if count > 0 {
            result[band] = 10.0 * (energy_sum / count as f64).log10();
        }
    }
    result
}

/// Apply a Bark-band spreading function and return excitation levels.
pub fn excitation_pattern(bark_levels: &[f64; 24]) -> [f64; 24] {
    let mut excitation = [0.0_f64; 24];
    for (target, slot) in excitation.iter_mut().enumerate() {
        let mut total_power = 0.0_f64;
        for source in 0..24 {
            let level = bark_levels[source];
            if level <= -90.0 {
                continue;
            }
            let distance = (target as f64) - (source as f64);
            let fc = BARK_CENTER_FREQUENCIES[source];
            let attenuation = if distance < 0.0 {
                27.0 * distance.abs()
            } else if distance > 0.0 {
                (24.0 + 230.0 / fc - 0.2 * level).max(5.0) * distance
            } else {
                0.0
            };
            total_power += db_to_power(level - attenuation);
        }
        *slot = if total_power > 0.0 {
            10.0 * total_power.log10()
        } else {
            -100.0
        };
    }
    excitation
}

/// Compute specific loudness in sone/Bark for each Bark band.
///
/// The input response is level-calibrated so the interpolated 1 kHz value is
/// `listening_level_phon`, then converted to excitation and specific loudness.
pub fn specific_loudness(freqs: &[f64], spl_db: &[f64], listening_level_phon: f64) -> [f64; 24] {
    let calibrated_spl = calibrate_to_listening_level(freqs, spl_db, listening_level_phon);
    let mut band_levels = bark_spectrum(freqs, &calibrated_spl);
    for i in 0..24 {
        band_levels[i] += OUTER_EAR_TF[i];
    }
    let excitation = excitation_pattern(&band_levels);
    let mut specific = [0.0_f64; 24];
    for i in 0..24 {
        let e = db_to_power(excitation[i]);
        let e_tq = db_to_power(THRESHOLD_IN_QUIET[i]);
        let ratio = e / e_tq;
        let val = 0.08 * ((0.5 + 0.5 * ratio).powf(0.23) - 1.0);
        specific[i] = val.max(0.0);
    }
    specific
}

/// Total loudness in sone, integrated across Bark bands.
pub fn total_loudness(specific: &[f64; 24]) -> f64 {
    specific.iter().sum()
}

/// Compute Zwicker sharpness in acum from specific loudness.
pub fn sharpness(specific_loudness: &[f64; 24]) -> f64 {
    let numerator = specific_loudness
        .iter()
        .enumerate()
        .map(|(z, n)| n * SHARPNESS_WEIGHT[z] * (z + 1) as f64)
        .sum::<f64>();
    let denominator = total_loudness(specific_loudness).max(0.001);
    0.11 * numerator / denominator
}

/// Estimate pairwise psychoacoustic sensory roughness from prominent spectrum components.
///
/// The model extracts local spectral maxima and applies the standard
/// Plomp-Levelt/Sethares pair interaction curve with Vassilakis-style amplitude
/// weighting. A single isolated component is not rough; roughness is produced
/// by pairs close enough to interact within an auditory critical band.
pub fn spectral_roughness(freqs: &[f64], spl_db: &[f64]) -> f64 {
    let partials = prominent_partials(freqs, spl_db);
    if partials.len() < 2 {
        return 0.0;
    }

    let mut total = 0.0;
    for i in 0..partials.len() {
        for j in (i + 1)..partials.len() {
            total += roughness_pair(partials[i], partials[j]);
        }
    }

    (4.0 * total / partials.len() as f64).min(2.0)
}

/// Estimate roughness from a frequency response.
pub fn roughness_from_spectrum(freqs: &[f64], spl_db: &[f64]) -> f64 {
    spectral_roughness(freqs, spl_db)
}

#[derive(Clone, Copy)]
struct RoughnessPartial {
    frequency_hz: f64,
    amplitude: f64,
}

fn prominent_partials(freqs: &[f64], spl_db: &[f64]) -> Vec<RoughnessPartial> {
    let mut points = freqs
        .iter()
        .zip(spl_db.iter())
        .filter_map(|(&f, &level)| {
            (f.is_finite() && level.is_finite() && (20.0..=15_500.0).contains(&f))
                .then_some((f, level))
        })
        .collect::<Vec<_>>();
    if points.len() < 3 {
        return Vec::new();
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let max_level = points
        .iter()
        .map(|(_, level)| *level)
        .fold(f64::NEG_INFINITY, f64::max);

    let mut partials = Vec::new();
    for i in 1..(points.len() - 1) {
        let (frequency_hz, level) = points[i];
        let left = points[i - 1].1;
        let right = points[i + 1].1;
        let local_max = (level >= left && level > right) || (level > left && level >= right);
        if !local_max {
            continue;
        }
        let window = 24usize;
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(points.len());
        let left_floor = points[lo..i]
            .iter()
            .map(|(_, level)| *level)
            .fold(level, f64::min);
        let right_floor = points[(i + 1)..hi]
            .iter()
            .map(|(_, level)| *level)
            .fold(level, f64::min);
        let prominence = level - left_floor.max(right_floor);
        if prominence < 1.0 {
            continue;
        }
        let amplitude = 10.0_f64
            .powf(((level - max_level) / 20.0).clamp(-6.0, 0.0))
            .max(1e-6);
        partials.push(RoughnessPartial {
            frequency_hz,
            amplitude,
        });
    }

    merge_close_partials(partials)
}

fn merge_close_partials(mut partials: Vec<RoughnessPartial>) -> Vec<RoughnessPartial> {
    partials.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
    let mut merged: Vec<RoughnessPartial> = Vec::new();
    for partial in partials {
        if let Some(last) = merged.last_mut() {
            let bark_distance =
                (hz_to_bark(partial.frequency_hz) - hz_to_bark(last.frequency_hz)).abs();
            if bark_distance < 0.1 {
                if partial.amplitude > last.amplitude {
                    *last = partial;
                }
                continue;
            }
        }
        merged.push(partial);
    }
    merged
}

fn roughness_pair(a: RoughnessPartial, b: RoughnessPartial) -> f64 {
    let f_min = a.frequency_hz.min(b.frequency_hz);
    let delta_f = (a.frequency_hz - b.frequency_hz).abs();
    if delta_f <= 0.0 {
        return 0.0;
    }
    let s = ROUGHNESS_S_NUMERATOR / (ROUGHNESS_S_FREQUENCY_FACTOR * f_min + ROUGHNESS_S_OFFSET);
    let x = s * delta_f;
    let interaction = ((-ROUGHNESS_B1 * x).exp() - (-ROUGHNESS_B2 * x).exp()).max(0.0);
    let a_min = a.amplitude.min(b.amplitude);
    let a_max = a.amplitude.max(b.amplitude);
    let level_balance = if a_min + a_max > 0.0 {
        (2.0 * a_min / (a_min + a_max)).powf(3.11)
    } else {
        0.0
    };
    (a_min * a_max).powf(0.1) * level_balance * interaction
}

/// Time-domain linear convolution.
pub fn convolve_time_domain(signal: &[f64], impulse: &[f64]) -> Vec<f64> {
    if signal.is_empty() || impulse.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0.0; signal.len() + impulse.len() - 1];
    for (i, &x) in signal.iter().enumerate() {
        for (j, &h) in impulse.iter().enumerate() {
            out[i + j] += x * h;
        }
    }
    out
}

/// Convolve stereo input through a 2x2 HRTF/CTC matrix.
///
/// Returns `(left_ear, right_ear)` where:
/// `left_ear = left * h_ll + right * h_rl` and
/// `right_ear = left * h_lr + right * h_rr`.
pub fn convolve_stereo_hrtf(
    left: &[f64],
    right: &[f64],
    h_ll: &[f64],
    h_lr: &[f64],
    h_rl: &[f64],
    h_rr: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let ll = convolve_time_domain(left, h_ll);
    let rl = convolve_time_domain(right, h_rl);
    let lr = convolve_time_domain(left, h_lr);
    let rr = convolve_time_domain(right, h_rr);
    (sum_aligned(&ll, &rl), sum_aligned(&lr, &rr))
}

fn db_to_power(db: f64) -> f64 {
    10.0_f64.powf(db / 10.0)
}

fn sum_aligned(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len().max(b.len());
    let mut out = vec![0.0; n];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bark_roundtrip_is_close() {
        for &f in &[100.0, 1000.0, 4000.0, 10000.0] {
            let back = bark_to_hz(hz_to_bark(f));
            assert!((back - f).abs() / f < 0.02);
        }
    }

    #[test]
    fn flat_response_has_low_roughness() {
        let freqs = (0..1000)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / 999.0)
            .collect::<Vec<_>>();
        let spl = vec![70.0; freqs.len()];
        assert!(roughness_from_spectrum(&freqs, &spl) < 1e-10);
    }

    #[test]
    fn listening_level_calibrates_loudness() {
        let freqs = (0..1000)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / 999.0)
            .collect::<Vec<_>>();
        let relative = vec![0.0; freqs.len()];
        let quiet = total_loudness(&specific_loudness(&freqs, &relative, 50.0));
        let loud = total_loudness(&specific_loudness(&freqs, &relative, 80.0));
        assert!(loud > quiet * 2.0, "loud={loud}, quiet={quiet}");
    }

    #[test]
    fn close_prominent_components_are_rougher_than_wide_components() {
        let freqs = (0..4000)
            .map(|i| 20.0 + (3000.0 - 20.0) * i as f64 / 3999.0)
            .collect::<Vec<_>>();
        let close = peaked_response(&freqs, &[1000.0, 1070.0]);
        let wide = peaked_response(&freqs, &[1000.0, 1800.0]);
        let close_r = roughness_from_spectrum(&freqs, &close);
        let wide_r = roughness_from_spectrum(&freqs, &wide);
        assert!(
            close_r > wide_r,
            "close partials should be rougher: close={close_r}, wide={wide_r}"
        );
    }

    #[test]
    fn convolution_matches_manual_result() {
        let y = convolve_time_domain(&[1.0, 2.0, 3.0], &[0.5, 1.0]);
        assert_eq!(y, vec![0.5, 2.0, 3.5, 3.0]);
    }

    fn peaked_response(freqs: &[f64], peaks: &[f64]) -> Vec<f64> {
        freqs
            .iter()
            .map(|&f| {
                let peak = peaks
                    .iter()
                    .map(|&center| {
                        let distance = (f - center) / 8.0;
                        24.0 * (-0.5 * distance * distance).exp()
                    })
                    .fold(0.0, f64::max);
                60.0 + peak
            })
            .collect()
    }
}
