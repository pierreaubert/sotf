//! Multi-driver crossover optimization.
//!
//! Defines the driver measurement types, crossover filter choices, and
//! the loss / combined-response functions used by the drivers-flat and
//! multi-sub optimization workflows.

use super::flat::flat_loss;
use crate::Curve;
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Crossover filter type for multi-driver optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CrossoverType {
    /// 2nd order Butterworth (12 dB/octave)
    Butterworth2,
    /// 2nd order Linkwitz-Riley (12 dB/octave)
    LinkwitzRiley2,
    /// 4th order Linkwitz-Riley (24 dB/octave) — most common
    #[serde(alias = "LR24")]
    LinkwitzRiley4,
    /// 8th order Linkwitz-Riley (48 dB/octave)
    #[serde(alias = "LR48")]
    LinkwitzRiley8,
    /// No crossover filter (for multi-sub optimization)
    None,
}

impl Default for CrossoverType {
    fn default() -> Self {
        CrossoverType::LinkwitzRiley4
    }
}

impl CrossoverType {
    /// Convert to a short plugin-compatible string.
    pub fn to_plugin_string(&self) -> &'static str {
        match self {
            CrossoverType::Butterworth2 => "Butterworth12",
            CrossoverType::LinkwitzRiley2 => "LR12",
            CrossoverType::LinkwitzRiley4 => "LR24",
            CrossoverType::LinkwitzRiley8 => "LR48",
            CrossoverType::None => "None",
        }
    }

    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            CrossoverType::Butterworth2 => "2nd order Butterworth",
            CrossoverType::LinkwitzRiley2 => "2nd order Linkwitz-Riley",
            CrossoverType::LinkwitzRiley4 => "4th order Linkwitz-Riley",
            CrossoverType::LinkwitzRiley8 => "8th order Linkwitz-Riley",
            CrossoverType::None => "No Crossover (Multi-Sub)",
        }
    }
}

impl std::str::FromStr for CrossoverType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "butterworth2" | "bw2" | "butterworth12" | "bw12" => Ok(CrossoverType::Butterworth2),
            "lr2" | "lr12" | "linkwitzriley2" | "linkwitzriley12" => {
                Ok(CrossoverType::LinkwitzRiley2)
            }
            "lr4" | "lr24" | "linkwitzriley4" | "linkwitzriley24" => {
                Ok(CrossoverType::LinkwitzRiley4)
            }
            "lr8" | "lr48" | "linkwitzriley8" | "linkwitzriley48" => {
                Ok(CrossoverType::LinkwitzRiley8)
            }
            "none" => Ok(CrossoverType::None),
            _ => Err(format!("Unknown crossover type: {}", s)),
        }
    }
}

impl std::fmt::Display for CrossoverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_plugin_string())
    }
}

/// Measurement data for a single driver
#[derive(Debug, Clone)]
pub struct DriverMeasurement {
    /// Frequency points in Hz
    pub freq: Array1<f64>,
    /// SPL measurements in dB
    pub spl: Array1<f64>,
    /// Phase measurements in degrees (optional for now)
    pub phase: Option<Array1<f64>>,
}

impl DriverMeasurement {
    /// Create a new DriverMeasurement
    pub fn new(freq: Array1<f64>, spl: Array1<f64>, phase: Option<Array1<f64>>) -> Self {
        assert_eq!(freq.len(), spl.len(), "freq and spl must have same length");
        if let Some(ref p) = phase {
            assert_eq!(freq.len(), p.len(), "freq and phase must have same length");
        }
        Self { freq, spl, phase }
    }

    /// Get the frequency range covered by this driver
    pub fn freq_range(&self) -> (f64, f64) {
        let min_freq = self.freq.iter().copied().fold(f64::INFINITY, f64::min);
        let max_freq = self.freq.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min_freq, max_freq)
    }

    /// Get the mean frequency (geometric mean)
    pub fn mean_freq(&self) -> f64 {
        let (min_freq, max_freq) = self.freq_range();
        (min_freq * max_freq).sqrt()
    }
}

/// Data required for multi-driver crossover optimization
#[derive(Debug, Clone)]
pub struct DriversLossData {
    /// Measurements for each driver (sorted by frequency range, lowest first)
    pub drivers: Vec<DriverMeasurement>,
    /// Crossover type to use between driver pairs
    pub crossover_type: CrossoverType,
    /// Common frequency grid for evaluation
    pub freq_grid: Array1<f64>,
}

impl DriversLossData {
    /// Create a new DriversLossData instance
    ///
    /// # Arguments
    /// * `drivers` - Vector of driver measurements (will be sorted by frequency)
    /// * `crossover_type` - Type of crossover filter to use
    pub fn new(mut drivers: Vec<DriverMeasurement>, crossover_type: CrossoverType) -> Self {
        assert!(
            drivers.len() >= 2 && drivers.len() <= 4,
            "Must have 2-4 drivers, got {}",
            drivers.len()
        );

        // Sort drivers by their mean frequency (woofer -> midrange -> tweeter)
        drivers.sort_by(|a, b| {
            a.mean_freq()
                .partial_cmp(&b.mean_freq())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Create a common frequency grid spanning all drivers
        // Use logarithmic spacing from lowest to highest frequency
        let min_freq = drivers
            .iter()
            .map(|d| d.freq_range().0)
            .fold(f64::INFINITY, f64::min);
        let max_freq = drivers
            .iter()
            .map(|d| d.freq_range().1)
            .fold(f64::NEG_INFINITY, f64::max);

        // Create log-spaced frequency grid (10 points per octave)
        let freq_grid = crate::read::create_log_frequency_grid(
            10 * 10, // 10 octaves * 10 points per octave
            min_freq.max(20.0),
            max_freq.min(20000.0),
        );

        Self {
            drivers,
            crossover_type,
            freq_grid,
        }
    }
}

/// Helper to compute complex response of a biquad filter
fn biquad_complex_response(biquad: &crate::iir::Biquad, f: f64) -> Complex64 {
    let (a1, a2, b0, b1, b2) = biquad.constants();
    let omega = 2.0 * PI * f / biquad.srate;
    // z^-1 = e^(-j*omega) = cos(-omega) + j*sin(-omega)
    let z_inv = Complex64::from_polar(1.0, -omega);
    let z_inv2 = z_inv * z_inv;

    let num = b0 + b1 * z_inv + b2 * z_inv2;
    let den = 1.0 + a1 * z_inv + a2 * z_inv2;

    num / den
}

/// Interpolate and prepare driver curves on a common frequency grid
fn prepare_driver_curves(data: &DriversLossData, crossover_freqs: &[f64]) -> Vec<Curve> {
    let n_drivers = data.drivers.len();
    let mut driver_curves = Vec::new();
    for (i, driver) in data.drivers.iter().enumerate() {
        let (passband_low, passband_high) = if let CrossoverType::None = data.crossover_type {
            (20.0, 20000.0)
        } else {
            (
                if i == 0 { 20.0 } else { crossover_freqs[i - 1] },
                if i == n_drivers - 1 {
                    20000.0
                } else {
                    crossover_freqs[i]
                },
            )
        };

        let interpolated = crate::read::normalize_and_interpolate_response_with_range(
            &data.freq_grid,
            &Curve {
                freq: driver.freq.clone(),
                spl: driver.spl.clone(),
                phase: driver.phase.clone(),
            },
            passband_low,
            passband_high,
        );
        driver_curves.push(interpolated);
    }
    driver_curves
}

/// Build crossover filters for a single driver
fn build_crossover_filters_for_driver(
    driver_index: usize,
    n_drivers: usize,
    crossover_type: CrossoverType,
    crossover_freqs: &[f64],
    sample_rate: f64,
) -> Vec<(f64, crate::iir::Biquad)> {
    use crate::iir::{
        peq_butterworth_highpass, peq_butterworth_lowpass, peq_linkwitzriley_highpass,
        peq_linkwitzriley_lowpass,
    };

    let mut filters = Vec::new();
    if let CrossoverType::None = crossover_type {
        return filters;
    }

    if driver_index > 0 {
        let xover_freq = crossover_freqs[driver_index - 1];
        let hp_peq = match crossover_type {
            CrossoverType::Butterworth2 => peq_butterworth_highpass(2, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley2 => peq_linkwitzriley_highpass(2, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley4 => peq_linkwitzriley_highpass(4, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley8 => peq_linkwitzriley_highpass(8, xover_freq, sample_rate),
            CrossoverType::None => vec![],
        };
        filters.extend(hp_peq);
    }
    if driver_index < n_drivers - 1 {
        let xover_freq = crossover_freqs[driver_index];
        let lp_peq = match crossover_type {
            CrossoverType::Butterworth2 => peq_butterworth_lowpass(2, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley2 => peq_linkwitzriley_lowpass(2, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley4 => peq_linkwitzriley_lowpass(4, xover_freq, sample_rate),
            CrossoverType::LinkwitzRiley8 => peq_linkwitzriley_lowpass(8, xover_freq, sample_rate),
            CrossoverType::None => vec![],
        };
        filters.extend(lp_peq);
    }
    filters
}

/// Compute the complex response of a single driver at all frequency points
fn compute_single_driver_complex(
    freq_grid: &Array1<f64>,
    curve: &Curve,
    gain: f64,
    delay_s: f64,
    filters: &[(f64, crate::iir::Biquad)],
) -> Array1<Complex64> {
    let mag_factor = 10.0_f64.powf(gain / 20.0);
    let mut result = Array1::<Complex64>::zeros(freq_grid.len());

    for j in 0..freq_grid.len() {
        let f = freq_grid[j];
        let spl = curve.spl[j];

        let z_driver = if let Some(phase) = &curve.phase {
            let phi = phase[j].to_radians();
            let m = 10.0_f64.powf(spl / 20.0);
            Complex64::from_polar(m, phi)
        } else {
            let m = 10.0_f64.powf(spl / 20.0);
            Complex64::new(m, 0.0)
        };

        let phi_delay = -2.0 * PI * f * delay_s;
        let z_delay = Complex64::from_polar(1.0, phi_delay);

        let mut z_filters = Complex64::new(1.0, 0.0);
        for (_, biquad) in filters {
            z_filters *= biquad_complex_response(biquad, f);
        }

        result[j] = z_driver * mag_factor * z_filters * z_delay;
    }

    result
}

/// Validate driver arguments (shared by combined and per-driver functions)
fn validate_driver_args(
    data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    delays: Option<&[f64]>,
) {
    let n_drivers = data.drivers.len();
    assert_eq!(gains.len(), n_drivers);
    if !matches!(data.crossover_type, CrossoverType::None) {
        assert_eq!(crossover_freqs.len(), n_drivers - 1);
    }
    if let Some(d) = delays {
        assert_eq!(d.len(), n_drivers);
    }
}

/// Compute the combined frequency response of multiple drivers with crossovers, gains, and delays
///
/// # Arguments
/// * `data` - DriversLossData containing driver measurements and crossover type
/// * `gains` - Gain in dB for each driver
/// * `crossover_freqs` - Crossover frequencies between successive driver pairs
/// * `delays` - Optional delay in ms for each driver
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Combined frequency response in dB on the common frequency grid
pub fn compute_drivers_combined_response(
    data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    delays: Option<&[f64]>,
    sample_rate: f64,
) -> Array1<f64> {
    validate_driver_args(data, gains, crossover_freqs, delays);

    let n_drivers = data.drivers.len();
    let driver_curves = prepare_driver_curves(data, crossover_freqs);

    // Sum complex responses
    let mut combined_complex = Array1::<Complex64>::zeros(data.freq_grid.len());

    for i in 0..n_drivers {
        let delay_s = delays.map(|d| d[i]).unwrap_or(0.0) / 1000.0;
        let filters = build_crossover_filters_for_driver(
            i,
            n_drivers,
            data.crossover_type,
            crossover_freqs,
            sample_rate,
        );
        let driver_complex = compute_single_driver_complex(
            &data.freq_grid,
            &driver_curves[i],
            gains[i],
            delay_s,
            &filters,
        );
        combined_complex += &driver_complex;
    }

    // Convert back to dB SPL
    combined_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10())
}

/// Compute per-driver frequency responses with crossovers, gains, and delays applied
///
/// Same logic as `compute_drivers_combined_response()` but returns individual
/// driver responses instead of summing them into a single combined response.
///
/// # Arguments
/// * `data` - DriversLossData containing driver measurements and crossover type
/// * `gains` - Gain in dB for each driver
/// * `crossover_freqs` - Crossover frequencies between successive driver pairs
/// * `delays` - Optional delay in ms for each driver
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Vec of per-driver frequency responses in dB on the common frequency grid
pub fn compute_per_driver_responses(
    data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    delays: Option<&[f64]>,
    sample_rate: f64,
) -> Vec<Array1<f64>> {
    validate_driver_args(data, gains, crossover_freqs, delays);

    let n_drivers = data.drivers.len();
    let driver_curves = prepare_driver_curves(data, crossover_freqs);

    let mut results = Vec::with_capacity(n_drivers);
    for i in 0..n_drivers {
        let delay_s = delays.map(|d| d[i]).unwrap_or(0.0) / 1000.0;
        let filters = build_crossover_filters_for_driver(
            i,
            n_drivers,
            data.crossover_type,
            crossover_freqs,
            sample_rate,
        );
        let driver_complex = compute_single_driver_complex(
            &data.freq_grid,
            &driver_curves[i],
            gains[i],
            delay_s,
            &filters,
        );
        results.push(driver_complex.mapv(|z| 20.0 * z.norm().max(1e-12).log10()));
    }

    results
}

/// Compute the loss for multi-driver crossover optimization
///
/// # Arguments
/// * `data` - DriversLossData containing driver measurements
/// * `gains` - Gain in dB for each driver
/// * `crossover_freqs` - Crossover frequencies between successive driver pairs
/// * `sample_rate` - Sample rate for filter design
/// * `min_freq` - Minimum frequency for loss evaluation
/// * `max_freq` - Maximum frequency for loss evaluation
///
/// # Returns
/// * Loss value (lower is better)
pub fn drivers_flat_loss(
    data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    delays: Option<&[f64]>,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    // Compute combined response
    let combined_response =
        compute_drivers_combined_response(data, gains, crossover_freqs, delays, sample_rate);

    // Normalize the response (subtract the mean in the evaluation range)
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..data.freq_grid.len() {
        let freq = data.freq_grid[i];
        if freq >= min_freq && freq <= max_freq {
            sum += combined_response[i];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let normalized = &combined_response - mean;

    // Compute flatness loss (RMS deviation from zero)
    flat_loss(&data.freq_grid, &normalized, min_freq, max_freq)
}
