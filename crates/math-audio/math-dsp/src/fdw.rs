//! Frequency-dependent windowing for room impulse responses.
//!
//! FDW analyzes an impulse response with a frequency-dependent Morlet-style
//! gate: long windows in the bass and progressively shorter windows toward
//! the treble. The resulting direct-energy ratio can be used as a
//! per-frequency correction-depth curve.

use crate::analysis::smooth_response_f32;
use rayon::prelude::*;

const MIN_POWER: f32 = 1.0e-30;

/// Configuration for frequency-dependent windowing.
#[derive(Debug, Clone)]
pub struct FdwConfig {
    /// Number of logarithmically spaced output frequency points.
    pub num_points: usize,
    /// Lowest analyzed frequency in Hz.
    pub min_freq_hz: f32,
    /// Highest analyzed frequency in Hz. Clamped to Nyquist at runtime.
    pub max_freq_hz: f32,
    /// Window length in cycles at each frequency before min/max clamping.
    pub cycles: f32,
    /// Minimum analysis window length in milliseconds.
    pub min_window_ms: f32,
    /// Maximum analysis window length in milliseconds.
    pub max_window_ms: f32,
    /// Optional octave smoothing for magnitude and direct-energy ratio.
    pub smoothing_octaves: f32,
    /// Hop size as a fraction of the current window length for energy scans.
    pub hop_fraction: f32,
    /// Direct gate radius in units of the current half-window support.
    pub direct_gate_width_windows: f32,
}

impl Default for FdwConfig {
    fn default() -> Self {
        Self {
            num_points: 512,
            min_freq_hz: 20.0,
            max_freq_hz: 20_000.0,
            cycles: 8.0,
            min_window_ms: 3.0,
            max_window_ms: 500.0,
            smoothing_octaves: 1.0 / 24.0,
            hop_fraction: 0.25,
            direct_gate_width_windows: 1.0,
        }
    }
}

/// FDW analysis result.
#[derive(Debug, Clone)]
pub struct FdwAnalysis {
    /// Log-spaced frequencies in Hz.
    pub frequencies: Vec<f32>,
    /// FDW-gated magnitude around the direct sound, in dB.
    pub magnitude_db: Vec<f32>,
    /// Full time-frequency energy magnitude, in dB.
    pub full_energy_db: Vec<f32>,
    /// Direct / total time-frequency energy ratio, smoothed and clamped to 0..1.
    pub direct_energy_ratio: Vec<f32>,
    /// Alias for `direct_energy_ratio`, named for correction-depth consumers.
    pub correction_depth: Vec<f32>,
    /// Analysis window length per frequency, in milliseconds.
    pub window_ms: Vec<f32>,
    /// Direct sound sample used as the FDW gate center.
    pub direct_sound_sample: usize,
}

struct FdwFrequencyPoint {
    magnitude_db: f32,
    full_energy_db: f32,
    direct_energy_ratio: f32,
    window_ms: f32,
}

/// Analyze an impulse response with frequency-dependent windowing.
///
/// `direct_sound_sample` should be the detected direct-sound TOA when available.
/// If omitted, the largest absolute sample in the IR is used.
pub fn analyze_impulse_response_fdw(
    impulse_response: &[f32],
    sample_rate: f32,
    direct_sound_sample: Option<usize>,
    config: &FdwConfig,
) -> Result<FdwAnalysis, String> {
    if impulse_response.is_empty() {
        return Err("impulse_response must be non-empty".to_string());
    }
    if sample_rate <= 0.0 || !sample_rate.is_finite() {
        return Err("sample_rate must be positive and finite".to_string());
    }
    if config.num_points == 0 {
        return Err("num_points must be > 0".to_string());
    }
    if config.min_freq_hz <= 0.0 || !config.min_freq_hz.is_finite() {
        return Err("min_freq_hz must be positive and finite".to_string());
    }
    if config.max_freq_hz <= 0.0 || !config.max_freq_hz.is_finite() {
        return Err("max_freq_hz must be positive and finite".to_string());
    }
    if config.cycles <= 0.0 || !config.cycles.is_finite() {
        return Err("cycles must be positive and finite".to_string());
    }
    if config.min_window_ms <= 0.0
        || config.max_window_ms <= 0.0
        || config.min_window_ms > config.max_window_ms
    {
        return Err("window limits must be positive and ordered".to_string());
    }

    let nyquist = sample_rate * 0.5;
    let min_freq = config.min_freq_hz.min(nyquist);
    let max_freq = config.max_freq_hz.min(nyquist);
    if min_freq >= max_freq {
        return Err(
            "frequency range must span at least one positive band below Nyquist".to_string(),
        );
    }

    let direct_sample = direct_sound_sample
        .unwrap_or_else(|| find_direct_sound_sample(impulse_response))
        .min(impulse_response.len() - 1);
    let frequencies = log_spaced_frequencies(config.num_points, min_freq, max_freq);

    let points: Vec<FdwFrequencyPoint> = frequencies
        .par_iter()
        .map(|&freq| {
            let window_samples = fdw_window_samples(freq, sample_rate, config);
            let sigma_samples = (window_samples as f32 / 6.0).max(1.0);
            let half_support = (sigma_samples * 3.0).ceil() as usize;

            let direct_power = morlet_power_at(
                impulse_response,
                direct_sample,
                freq,
                sample_rate,
                sigma_samples,
                half_support,
            );
            let (direct_tf_energy, total_tf_energy) = time_frequency_energy(
                impulse_response,
                direct_sample,
                freq,
                sample_rate,
                window_samples,
                sigma_samples,
                half_support,
                config,
            );

            FdwFrequencyPoint {
                magnitude_db: power_to_db(direct_power),
                full_energy_db: power_to_db(total_tf_energy),
                direct_energy_ratio: if total_tf_energy > MIN_POWER {
                    (direct_tf_energy / total_tf_energy).clamp(0.0, 1.0)
                } else {
                    0.0
                },
                window_ms: window_samples as f32 / sample_rate * 1000.0,
            }
        })
        .collect();

    let mut magnitude_db = Vec::with_capacity(points.len());
    let mut full_energy_db = Vec::with_capacity(points.len());
    let mut direct_energy_ratio = Vec::with_capacity(points.len());
    let mut window_ms = Vec::with_capacity(points.len());

    for point in points {
        magnitude_db.push(point.magnitude_db);
        full_energy_db.push(point.full_energy_db);
        direct_energy_ratio.push(point.direct_energy_ratio);
        window_ms.push(point.window_ms);
    }

    if config.smoothing_octaves > 0.0 {
        magnitude_db = smooth_response_f32(&frequencies, &magnitude_db, config.smoothing_octaves);
        full_energy_db =
            smooth_response_f32(&frequencies, &full_energy_db, config.smoothing_octaves);
        direct_energy_ratio =
            smooth_response_f32(&frequencies, &direct_energy_ratio, config.smoothing_octaves)
                .into_iter()
                .map(|v| v.clamp(0.0, 1.0))
                .collect();
    }

    Ok(FdwAnalysis {
        frequencies,
        magnitude_db,
        full_energy_db,
        correction_depth: direct_energy_ratio.clone(),
        direct_energy_ratio,
        window_ms,
        direct_sound_sample: direct_sample,
    })
}

fn find_direct_sound_sample(impulse_response: &[f32]) -> usize {
    impulse_response
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.abs()
                .partial_cmp(&b.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn log_spaced_frequencies(num_points: usize, min_freq: f32, max_freq: f32) -> Vec<f32> {
    if num_points == 1 {
        return vec![min_freq];
    }

    let log_start = min_freq.ln();
    let log_end = max_freq.ln();
    (0..num_points)
        .map(|i| (log_start + (log_end - log_start) * i as f32 / (num_points - 1) as f32).exp())
        .collect()
}

fn fdw_window_samples(freq: f32, sample_rate: f32, config: &FdwConfig) -> usize {
    let unclamped_ms = config.cycles / freq * 1000.0;
    let window_ms = unclamped_ms.clamp(config.min_window_ms, config.max_window_ms);
    let mut samples = (window_ms / 1000.0 * sample_rate).round() as usize;
    samples = samples.max(3);
    if samples.is_multiple_of(2) {
        samples + 1
    } else {
        samples
    }
}

fn morlet_power_at(
    impulse_response: &[f32],
    center: usize,
    freq: f32,
    sample_rate: f32,
    sigma_samples: f32,
    half_support: usize,
) -> f32 {
    let start = center.saturating_sub(half_support);
    let end = (center + half_support + 1).min(impulse_response.len());
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let mut re = 0.0_f32;
    let mut im = 0.0_f32;

    for (offset, &sample) in impulse_response[start..end].iter().enumerate() {
        let idx = start + offset;
        let rel = idx as isize - center as isize;
        let x = rel as f32 / sigma_samples;
        let window = (-0.5 * x * x).exp();
        let phase = -omega * rel as f32;
        re += sample * window * phase.cos();
        im += sample * window * phase.sin();
    }

    re * re + im * im
}

fn time_frequency_energy(
    impulse_response: &[f32],
    direct_sample: usize,
    freq: f32,
    sample_rate: f32,
    window_samples: usize,
    sigma_samples: f32,
    half_support: usize,
    config: &FdwConfig,
) -> (f32, f32) {
    let hop_fraction = config.hop_fraction.clamp(0.05, 1.0);
    let hop = ((window_samples as f32 * hop_fraction).round() as usize).max(1);
    let direct_radius =
        ((half_support as f32) * config.direct_gate_width_windows.max(0.0)).round() as usize;

    let mut centers = Vec::with_capacity(impulse_response.len() / hop + 3);
    let mut center = 0usize;
    while center < impulse_response.len() {
        centers.push(center);
        center = center.saturating_add(hop);
    }
    centers.push(direct_sample);
    centers.push(impulse_response.len() - 1);
    centers.sort_unstable();
    centers.dedup();

    let mut direct_energy = 0.0_f32;
    let mut total_energy = 0.0_f32;

    for center in centers {
        let power = morlet_power_at(
            impulse_response,
            center,
            freq,
            sample_rate,
            sigma_samples,
            half_support,
        );
        total_energy += power;
        if center.abs_diff(direct_sample) <= direct_radius {
            direct_energy += power;
        }
    }

    (direct_energy, total_energy)
}

fn power_to_db(power: f32) -> f32 {
    10.0 * power.max(MIN_POWER).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdw_magnitude_is_flat_for_single_impulse() {
        let mut ir = vec![0.0_f32; 4096];
        ir[256] = 1.0;
        let config = FdwConfig {
            num_points: 96,
            min_freq_hz: 40.0,
            max_freq_hz: 16_000.0,
            smoothing_octaves: 0.0,
            ..Default::default()
        };

        let analysis = analyze_impulse_response_fdw(&ir, 48_000.0, Some(256), &config).unwrap();
        let max = analysis
            .magnitude_db
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let min = analysis
            .magnitude_db
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);

        assert!(
            max - min < 0.01,
            "single-sample impulse should remain flat after FDW"
        );
        assert!(
            analysis.direct_energy_ratio.iter().all(|&v| v > 0.99),
            "dry impulse should be fully direct"
        );
    }

    #[test]
    fn fdw_rejects_late_reflection_more_at_high_frequency() {
        let sample_rate = 48_000.0;
        let mut ir = vec![0.0_f32; 8192];
        ir[128] = 1.0;
        ir[128 + (0.010 * sample_rate) as usize] = 0.8;
        let config = FdwConfig {
            num_points: 160,
            min_freq_hz: 40.0,
            max_freq_hz: 8_000.0,
            smoothing_octaves: 0.0,
            ..Default::default()
        };

        let analysis = analyze_impulse_response_fdw(&ir, sample_rate, Some(128), &config).unwrap();
        let low = nearest_ratio(&analysis, 80.0);
        let high = nearest_ratio(&analysis, 4_000.0);

        assert!(
            low > 0.95,
            "bass FDW window should include modal/early energy, got {low:.3}"
        );
        assert!(
            high < 0.75,
            "HF FDW window should reject the 10ms reflection, got {high:.3}"
        );
    }

    #[test]
    fn fdw_validates_empty_ir() {
        let err = analyze_impulse_response_fdw(&[], 48_000.0, None, &FdwConfig::default())
            .expect_err("empty IR should be rejected");
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn fdw_rejects_zero_sample_rate() {
        let ir = vec![1.0_f32; 512];
        let err = analyze_impulse_response_fdw(&ir, 0.0, None, &FdwConfig::default())
            .expect_err("zero sample rate should be rejected");
        assert!(err.contains("sample_rate"));
    }

    #[test]
    fn fdw_rejects_nan_sample_rate() {
        let ir = vec![1.0_f32; 512];
        let err = analyze_impulse_response_fdw(&ir, f32::NAN, None, &FdwConfig::default())
            .expect_err("NaN sample rate should be rejected");
        assert!(err.contains("sample_rate"));
    }

    #[test]
    fn fdw_rejects_invalid_freq_range() {
        let ir = vec![1.0_f32; 512];
        let mut config = FdwConfig::default();
        config.min_freq_hz = 400.0;
        config.max_freq_hz = 200.0;
        let err = analyze_impulse_response_fdw(&ir, 48000.0, None, &config)
            .expect_err("invalid freq range should be rejected");
        assert!(err.contains("frequency range"));
    }

    #[test]
    fn fdw_rejects_bad_cycles() {
        let ir = vec![1.0_f32; 512];
        let mut config = FdwConfig::default();
        config.cycles = 0.0;
        let err = analyze_impulse_response_fdw(&ir, 48000.0, None, &config)
            .expect_err("zero cycles should be rejected");
        assert!(err.contains("cycles"));
    }

    #[test]
    fn fdw_rejects_bad_window_limits() {
        let ir = vec![1.0_f32; 512];
        let mut config = FdwConfig::default();
        config.min_window_ms = 500.0;
        config.max_window_ms = 100.0;
        let err = analyze_impulse_response_fdw(&ir, 48000.0, None, &config)
            .expect_err("reversed window limits should be rejected");
        assert!(err.contains("window limits"));
    }

    #[test]
    fn fdw_auto_detects_direct_sound() {
        let mut ir = vec![0.0_f32; 4096];
        ir[300] = 1.0;
        let analysis =
            analyze_impulse_response_fdw(&ir, 48000.0, None, &FdwConfig::default()).unwrap();
        assert_eq!(analysis.direct_sound_sample, 300);
    }

    #[test]
    fn fdw_single_point() {
        let mut ir = vec![0.0_f32; 4096];
        ir[256] = 1.0;
        let mut config = FdwConfig::default();
        config.num_points = 1;
        config.smoothing_octaves = 0.0;
        let analysis = analyze_impulse_response_fdw(&ir, 48000.0, Some(256), &config).unwrap();
        assert_eq!(analysis.frequencies.len(), 1);
        assert_eq!(analysis.magnitude_db.len(), 1);
    }

    fn nearest_ratio(analysis: &FdwAnalysis, freq: f32) -> f32 {
        analysis
            .frequencies
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - freq)
                    .abs()
                    .partial_cmp(&(*b - freq).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| analysis.direct_energy_ratio[idx])
            .unwrap()
    }
}
