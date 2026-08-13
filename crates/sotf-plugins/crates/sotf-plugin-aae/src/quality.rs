//! Deterministic, dependency-free measurements used by the offline AAE QA program.
//!
//! These routines are deliberately not part of the realtime renderer. They operate
//! on complete rendered signals and may allocate. Keeping them in the plugin crate
//! makes the measurement definitions independently regression-testable instead of
//! burying acceptance criteria in a one-off executable.

use std::f64::consts::TAU;

#[derive(Debug, Clone, Copy)]
pub struct DecayEstimate {
    pub rt60_seconds: f64,
    pub slope_db_per_second: f64,
    pub r_squared: f64,
    pub points: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct EchoDensityEstimate {
    pub mixing_time_seconds: Option<f64>,
    pub peak_normalized_density: f64,
}

#[derive(Debug, Clone)]
pub struct SpatialEstimate {
    pub channel_energy: Vec<f64>,
    pub normalized_energy_entropy: f64,
    pub mean_absolute_coherence: f64,
    pub diffuseness: f64,
    pub energy_vector: [f64; 3],
    pub energy_vector_magnitude: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TransferEstimate {
    pub gain_db: f64,
    pub phase_degrees: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct DistortionEstimate {
    pub thd_db: f64,
    pub imd_db: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfusionMatrix {
    pub true_positive: usize,
    pub false_positive: usize,
    pub true_negative: usize,
    pub false_negative: usize,
}

impl ConfusionMatrix {
    pub fn precision(self) -> f64 {
        self.true_positive as f64 / (self.true_positive + self.false_positive).max(1) as f64
    }

    pub fn recall(self) -> f64 {
        self.true_positive as f64 / (self.true_positive + self.false_negative).max(1) as f64
    }
}

/// Estimate RT60 from a Schroeder backwards energy integral.
///
/// The regression is performed over `upper_db..lower_db` relative to the peak
/// integrated energy. A -5..-35 dB interval is a T30 estimate extrapolated to
/// 60 dB, while -5..-25 dB is the corresponding T20 estimate.
pub fn schroeder_rt60(
    impulse_response: &[f32],
    sample_rate: u32,
    upper_db: f64,
    lower_db: f64,
) -> Option<DecayEstimate> {
    if impulse_response.is_empty() || sample_rate == 0 || upper_db <= lower_db {
        return None;
    }
    let mut integrated = vec![0.0_f64; impulse_response.len()];
    let mut sum = 0.0_f64;
    for (index, sample) in impulse_response.iter().enumerate().rev() {
        let value = f64::from(*sample);
        sum += value * value;
        integrated[index] = sum;
    }
    let reference = integrated[0];
    if reference <= 1e-30 {
        return None;
    }
    let mut count = 0usize;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    for (index, energy) in integrated.into_iter().enumerate() {
        let db = 10.0 * (energy / reference).max(1e-30).log10();
        if db <= upper_db && db >= lower_db {
            let time = index as f64 / sample_rate as f64;
            count += 1;
            sx += time;
            sy += db;
            sxx += time * time;
            syy += db * db;
            sxy += time * db;
        }
    }
    if count < 8 {
        return None;
    }
    let n = count as f64;
    let denominator = n * sxx - sx * sx;
    if denominator.abs() < 1e-20 {
        return None;
    }
    let slope = (n * sxy - sx * sy) / denominator;
    if slope >= 0.0 || !slope.is_finite() {
        return None;
    }
    let covariance = n * sxy - sx * sy;
    let r_denominator = ((n * sxx - sx * sx) * (n * syy - sy * sy)).sqrt();
    Some(DecayEstimate {
        rt60_seconds: -60.0 / slope,
        slope_db_per_second: slope,
        r_squared: if r_denominator > 0.0 {
            (covariance / r_denominator).powi(2)
        } else {
            0.0
        },
        points: count,
    })
}

/// Constant-Q RBJ band-pass used before the Schroeder integration.
pub fn octave_band(signal: &[f32], sample_rate: u32, center_hz: f64) -> Vec<f32> {
    if signal.is_empty() || sample_rate == 0 || center_hz <= 0.0 {
        return Vec::new();
    }
    let q = 1.0 / 2.0_f64.sqrt();
    let omega = TAU * center_hz / sample_rate as f64;
    let (sin, cos) = omega.sin_cos();
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b0 = alpha / a0;
    let b1 = 0.0;
    let b2 = -alpha / a0;
    let a1 = -2.0 * cos / a0;
    let a2 = (1.0 - alpha) / a0;
    let mut x1 = 0.0;
    let mut x2 = 0.0;
    let mut y1 = 0.0;
    let mut y2 = 0.0;
    signal
        .iter()
        .map(|sample| {
            let x = f64::from(*sample);
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            y as f32
        })
        .collect()
}

/// Abel-style normalized echo density. A Gaussian sequence has an expected
/// exceedance probability of 0.3173 above one local standard deviation, hence
/// a normalized density of one.
pub fn echo_density(
    signal: &[f32],
    sample_rate: u32,
    window_samples: usize,
    hop_samples: usize,
) -> EchoDensityEstimate {
    const GAUSSIAN_EXCEEDANCE: f64 = 0.317_310_5;
    if window_samples < 8 || hop_samples == 0 || signal.len() < window_samples {
        return EchoDensityEstimate {
            mixing_time_seconds: None,
            peak_normalized_density: 0.0,
        };
    }
    let mut peak = 0.0_f64;
    let mut consecutive = 0usize;
    let mut mixing_time = None;
    for start in (0..=signal.len() - window_samples).step_by(hop_samples) {
        let window = &signal[start..start + window_samples];
        let variance = window
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum::<f64>()
            / window_samples as f64;
        let sigma = variance.sqrt();
        let exceedances = if sigma > 1e-15 {
            window
                .iter()
                .filter(|sample| f64::from(**sample).abs() > sigma)
                .count()
        } else {
            0
        };
        let density = exceedances as f64 / window_samples as f64 / GAUSSIAN_EXCEEDANCE;
        peak = peak.max(density);
        if density >= 0.9 {
            consecutive += 1;
            if consecutive >= 3 && mixing_time.is_none() {
                let first = start.saturating_sub(2 * hop_samples);
                mixing_time = Some(first as f64 / sample_rate.max(1) as f64);
            }
        } else {
            consecutive = 0;
        }
    }
    EchoDensityEstimate {
        mixing_time_seconds: mixing_time,
        peak_normalized_density: peak,
    }
}

/// Multichannel spatial statistics. `directions` are unit vectors in channel
/// order. Pass `None` for channels such as LFE that must be excluded.
pub fn spatial_metrics(
    interleaved: &[f32],
    channels: usize,
    directions: &[Option<[f64; 3]>],
) -> Option<SpatialEstimate> {
    if channels < 2 || interleaved.len() < channels || directions.len() != channels {
        return None;
    }
    let frames = interleaved.len() / channels;
    let active: Vec<usize> = directions
        .iter()
        .enumerate()
        .filter_map(|(index, direction)| direction.map(|_| index))
        .collect();
    if active.len() < 2 {
        return None;
    }
    let mut energies = vec![0.0_f64; channels];
    for frame in interleaved.chunks_exact(channels) {
        for &channel in &active {
            energies[channel] += f64::from(frame[channel]).powi(2);
        }
    }
    let total = active.iter().map(|index| energies[*index]).sum::<f64>();
    if total <= 1e-30 {
        return None;
    }
    let entropy_denominator = (active.len() as f64).ln();
    let entropy = -active
        .iter()
        .map(|index| {
            let p = energies[*index] / total;
            if p > 0.0 { p * p.ln() } else { 0.0 }
        })
        .sum::<f64>()
        / entropy_denominator;
    let mut coherence_sum = 0.0;
    let mut pairs = 0usize;
    for (position, &a) in active.iter().enumerate() {
        for &b in &active[position + 1..] {
            let cross = (0..frames)
                .map(|frame| {
                    f64::from(interleaved[frame * channels + a])
                        * f64::from(interleaved[frame * channels + b])
                })
                .sum::<f64>();
            coherence_sum += (cross / (energies[a] * energies[b]).sqrt().max(1e-30)).abs();
            pairs += 1;
        }
    }
    let mean_coherence = coherence_sum / pairs.max(1) as f64;
    let mut vector = [0.0_f64; 3];
    for &channel in &active {
        let weight = energies[channel] / total;
        let direction = directions[channel].expect("active direction");
        for axis in 0..3 {
            vector[axis] += direction[axis] * weight;
        }
    }
    Some(SpatialEstimate {
        channel_energy: energies,
        normalized_energy_entropy: entropy,
        mean_absolute_coherence: mean_coherence,
        diffuseness: entropy * (1.0 - mean_coherence).clamp(0.0, 1.0),
        energy_vector: vector,
        energy_vector_magnitude: vector.iter().map(|value| value * value).sum::<f64>().sqrt(),
    })
}

fn complex_projection(signal: &[f32], sample_rate: u32, frequency_hz: f64) -> (f64, f64) {
    let mut real = 0.0;
    let mut imaginary = 0.0;
    for (index, sample) in signal.iter().enumerate() {
        let phase = TAU * frequency_hz * index as f64 / sample_rate as f64;
        real += f64::from(*sample) * phase.cos();
        imaginary -= f64::from(*sample) * phase.sin();
    }
    let scale = 2.0 / signal.len().max(1) as f64;
    (
        (real * real + imaginary * imaginary).sqrt() * scale,
        imaginary.atan2(real),
    )
}

/// Peak sinusoidal amplitude at an exact frequency using a rectangular-window
/// complex projection. QA fixtures use integer-cycle observation intervals.
pub fn tone_amplitude(signal: &[f32], sample_rate: u32, frequency_hz: f64) -> f64 {
    complex_projection(signal, sample_rate, frequency_hz).0
}

pub fn transfer_at_frequency(
    input: &[f32],
    output: &[f32],
    sample_rate: u32,
    frequency_hz: f64,
) -> Option<TransferEstimate> {
    if input.len() != output.len() || input.is_empty() || sample_rate == 0 {
        return None;
    }
    let (input_amplitude, input_phase) = complex_projection(input, sample_rate, frequency_hz);
    let (output_amplitude, output_phase) = complex_projection(output, sample_rate, frequency_hz);
    if input_amplitude <= 1e-15 {
        return None;
    }
    let mut phase = (output_phase - input_phase).to_degrees();
    while phase > 180.0 {
        phase -= 360.0;
    }
    while phase <= -180.0 {
        phase += 360.0;
    }
    Some(TransferEstimate {
        gain_db: 20.0 * (output_amplitude / input_amplitude).max(1e-15).log10(),
        phase_degrees: phase,
    })
}

pub fn modulation_sideband_db(
    signal: &[f32],
    sample_rate: u32,
    carrier_hz: f64,
    offsets_hz: &[f64],
) -> Option<f64> {
    let (carrier, _) = complex_projection(signal, sample_rate, carrier_hz);
    if carrier <= 1e-15 {
        return None;
    }
    let sideband = offsets_hz
        .iter()
        .flat_map(|offset| [carrier_hz - offset, carrier_hz + offset])
        .filter(|frequency| *frequency > 0.0)
        .map(|frequency| complex_projection(signal, sample_rate, frequency).0)
        .fold(0.0_f64, f64::max);
    Some(20.0 * (sideband / carrier).max(1e-15).log10())
}

pub fn distortion_metrics(
    signal: &[f32],
    sample_rate: u32,
    fundamental_hz: f64,
    second_tone_hz: Option<f64>,
) -> Option<DistortionEstimate> {
    let (fundamental_amplitude, _) = complex_projection(signal, sample_rate, fundamental_hz);
    if fundamental_amplitude <= 1e-15 {
        return None;
    }
    let harmonic_power = (2..=5)
        .map(|harmonic| {
            complex_projection(signal, sample_rate, fundamental_hz * harmonic as f64)
                .0
                .powi(2)
        })
        .sum::<f64>();
    let thd = harmonic_power.sqrt() / fundamental_amplitude;
    let imd = if let Some(second) = second_tone_hz {
        let (second_amplitude, _) = complex_projection(signal, sample_rate, second);
        let reference = fundamental_amplitude.hypot(second_amplitude);
        let products = [
            (second - fundamental_hz).abs(),
            second + fundamental_hz,
            (2.0 * fundamental_hz - second).abs(),
            (2.0 * second - fundamental_hz).abs(),
        ];
        products
            .iter()
            .filter(|frequency| **frequency > 0.0)
            .map(|frequency| {
                complex_projection(signal, sample_rate, *frequency)
                    .0
                    .powi(2)
            })
            .sum::<f64>()
            .sqrt()
            / reference.max(1e-15)
    } else {
        0.0
    };
    Some(DistortionEstimate {
        thd_db: 20.0 * thd.max(1e-15).log10(),
        imd_db: 20.0 * imd.max(1e-15).log10(),
    })
}

pub fn confusion_matrix(expected: &[bool], observed: &[bool]) -> Option<ConfusionMatrix> {
    if expected.len() != observed.len() || expected.is_empty() {
        return None;
    }
    let mut result = ConfusionMatrix {
        true_positive: 0,
        false_positive: 0,
        true_negative: 0,
        false_negative: 0,
    };
    for (&expected, &observed) in expected.iter().zip(observed) {
        match (expected, observed) {
            (true, true) => result.true_positive += 1,
            (false, true) => result.false_positive += 1,
            (false, false) => result.true_negative += 1,
            (true, false) => result.false_negative += 1,
        }
    }
    Some(result)
}

/// Total variation and maximum step of a sampled gain trajectory. Values are
/// reported in dB so they remain comparable across absolute wet levels.
pub fn gain_pumping(gains: &[f32]) -> (f64, f64) {
    let db: Vec<f64> = gains
        .iter()
        .map(|gain| 20.0 * f64::from(*gain).max(1e-9).log10())
        .collect();
    db.windows(2).fold((0.0_f64, 0.0_f64), |(sum, peak), pair| {
        let delta = (pair[1] - pair[0]).abs();
        (sum + delta, peak.max(delta))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schroeder_recovers_exponential_rt60() {
        let sample_rate = 48_000;
        let expected = 1.2;
        let signal: Vec<f32> = (0..sample_rate * 2)
            .map(|index| {
                let time = index as f64 / sample_rate as f64;
                let noise = ((index as f64 * 12.9898).sin() * 43_758.545_3).fract() * 2.0 - 1.0;
                (noise * (-6.907_755 * time / expected).exp()) as f32
            })
            .collect();
        let estimate = schroeder_rt60(&signal, sample_rate, -5.0, -35.0).unwrap();
        assert!(
            (estimate.rt60_seconds - expected).abs() < 0.05,
            "{estimate:?}"
        );
        assert!(estimate.r_squared > 0.995, "{estimate:?}");
    }

    #[test]
    fn echo_density_distinguishes_sparse_and_dense_sequences() {
        let sample_rate = 48_000;
        let mut signal = vec![0.0_f32; 12_000];
        for (index, sample) in signal.iter_mut().enumerate().skip(4_000) {
            *sample = (((index * 1_103_515_245 + 12_345) & 0xffff) as f32 / 32_768.0) - 1.0;
        }
        let estimate = echo_density(&signal, sample_rate, 512, 128);
        assert!(estimate.mixing_time_seconds.unwrap() >= 0.07);
        assert!(estimate.peak_normalized_density > 0.85);
    }

    #[test]
    fn spatial_metrics_identify_coherent_and_diffuse_fields() {
        let directions = [Some([1.0, 0.0, 0.0]), Some([-1.0, 0.0, 0.0])];
        let coherent: Vec<f32> = (0..4096)
            .flat_map(|index| {
                let value = (TAU * 17.0 * index as f64 / 4096.0).sin() as f32;
                [value, value]
            })
            .collect();
        let diffuse: Vec<f32> = (0..4096)
            .flat_map(|index| {
                [
                    (TAU * 17.0 * index as f64 / 4096.0).sin() as f32,
                    (TAU * 31.0 * index as f64 / 4096.0).sin() as f32,
                ]
            })
            .collect();
        let coherent = spatial_metrics(&coherent, 2, &directions).unwrap();
        let diffuse = spatial_metrics(&diffuse, 2, &directions).unwrap();
        assert!(coherent.mean_absolute_coherence > 0.99);
        assert!(diffuse.mean_absolute_coherence < 1e-3);
        assert!(diffuse.diffuseness > coherent.diffuseness + 0.9);
    }

    #[test]
    fn transfer_reports_known_gain_and_phase() {
        let sample_rate = 48_000;
        let frequency = 1_000.0;
        let input: Vec<f32> = (0..48_000)
            .map(|index| (TAU * frequency * index as f64 / sample_rate as f64).cos() as f32)
            .collect();
        let output: Vec<f32> = (0..48_000)
            .map(|index| {
                (0.25
                    * (TAU * frequency * index as f64 / sample_rate as f64
                        + std::f64::consts::PI / 3.0)
                        .cos()) as f32
            })
            .collect();
        let estimate = transfer_at_frequency(&input, &output, sample_rate, frequency).unwrap();
        assert!((estimate.gain_db + 12.041_2).abs() < 0.01, "{estimate:?}");
        assert!((estimate.phase_degrees - 60.0).abs() < 0.1, "{estimate:?}");
    }

    #[test]
    fn distortion_and_detector_statistics_have_numeric_oracles() {
        let sample_rate = 48_000;
        let signal: Vec<f32> = (0..48_000)
            .map(|index| {
                let phase = TAU * 1_000.0 * index as f64 / sample_rate as f64;
                (phase.sin() + 0.01 * (2.0 * phase).sin()) as f32
            })
            .collect();
        let distortion = distortion_metrics(&signal, sample_rate, 1_000.0, None).unwrap();
        assert!((distortion.thd_db + 40.0).abs() < 0.05, "{distortion:?}");

        let two_tone: Vec<f32> = (0..sample_rate)
            .map(|index| {
                let time = index as f64 / sample_rate as f64;
                ((TAU * 700.0 * time).sin() + (TAU * 1_200.0 * time).sin()) as f32
            })
            .collect();
        let linear = distortion_metrics(&two_tone, sample_rate, 700.0, Some(1_200.0)).unwrap();
        assert!(linear.imd_db < -120.0, "{linear:?}");
        let matrix =
            confusion_matrix(&[true, true, false, false], &[true, false, true, false]).unwrap();
        assert_eq!(
            matrix,
            ConfusionMatrix {
                true_positive: 1,
                false_positive: 1,
                true_negative: 1,
                false_negative: 1
            }
        );
        assert_eq!(matrix.precision(), 0.5);
        assert_eq!(matrix.recall(), 0.5);
        let (variation, peak) = gain_pumping(&[1.0, 0.5, 0.5, 1.0]);
        assert!((variation - 12.041_2).abs() < 0.01);
        assert!((peak - 6.020_6).abs() < 0.01);
    }
}
