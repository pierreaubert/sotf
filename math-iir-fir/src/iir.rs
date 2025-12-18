//! IIR filter implementation (Biquad filters and Parametric EQ)

use base64::{Engine as _, engine::general_purpose};
use byteorder::{BigEndian, WriteBytesExt};
use ndarray::Array1;
use std::f64::consts::PI;
use std::fmt;

use crate::error::IirError;
use crate::{DEFAULT_Q_HIGH_LOW_PASS, DEFAULT_Q_HIGH_LOW_SHELF, q2bw};

/// Parametric EQ filter chain: a vector of (gain, Biquad) pairs.
///
/// Each element is a tuple of:
/// - `f64`: The linear gain multiplier for this stage
/// - `Biquad`: The biquad filter for this stage
pub type Peq = Vec<(f64, Biquad)>;

/// Filter types for biquad filters
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BiquadFilterType {
    /// Low-pass filter
    Lowpass,
    /// High-pass filter
    Highpass,
    /// High-pass filter
    HighpassVariableQ,
    /// Band-pass filter
    Bandpass,
    /// Peaking filter
    Peak,
    /// Notch filter
    Notch,
    /// Low-shelf filter
    Lowshelf,
    /// High-shelf filter
    Highshelf,
}

impl BiquadFilterType {
    /// Returns the short string representation of the filter type (e.g., "LP").
    pub fn short_name(&self) -> &'static str {
        match self {
            BiquadFilterType::Lowpass => "LP",
            BiquadFilterType::Highpass => "HP",
            BiquadFilterType::HighpassVariableQ => "HPQ",
            BiquadFilterType::Bandpass => "BP",
            BiquadFilterType::Peak => "PK",
            BiquadFilterType::Notch => "NO",
            BiquadFilterType::Lowshelf => "LS",
            BiquadFilterType::Highshelf => "HS",
        }
    }

    /// Returns the long string representation of the filter type (e.g., "Lowpass").
    pub fn long_name(&self) -> &'static str {
        match self {
            BiquadFilterType::Lowpass => "Lowpass",
            BiquadFilterType::Highpass => "Highpass",
            BiquadFilterType::HighpassVariableQ => "HighpassVariableQ",
            BiquadFilterType::Bandpass => "Bandpass",
            BiquadFilterType::Peak => "Peak",
            BiquadFilterType::Notch => "Notch",
            BiquadFilterType::Lowshelf => "Lowshelf",
            BiquadFilterType::Highshelf => "Highshelf",
        }
    }
}

/// Represents a single biquad IIR filter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Biquad {
    /// The type of filter
    pub filter_type: BiquadFilterType,
    /// Center frequency in Hz
    pub freq: f64,
    /// Sample rate in Hz
    pub srate: f64,
    /// Q factor (quality factor)
    pub q: f64,
    /// Gain in dB (for peaking and shelving filters)
    pub db_gain: f64,
    /// Filter coefficients
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    /// Filter state (for processing samples)
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    /// Pre-computed coefficients for fast frequency response calculation
    r_up0: f64,
    r_up1: f64,
    r_up2: f64,
    r_dw0: f64,
    r_dw1: f64,
    r_dw2: f64,
}

impl Biquad {
    /// Creates and initializes a new Biquad filter.
    ///
    /// This constructor applies default Q values for certain filter types and clamps
    /// invalid Q values to a minimum of 0.01 for numerical stability.
    ///
    /// # Arguments
    ///
    /// * `filter_type` - The type of filter to create
    /// * `freq` - Center/cutoff frequency in Hz
    /// * `srate` - Sample rate in Hz
    /// * `q` - Q factor (quality factor). Use 0.0 for default.
    /// * `db_gain` - Gain in dB (only used for Peak, Lowshelf, Highshelf)
    ///
    /// # Panics
    ///
    /// This method does not panic but silently clamps invalid parameters.
    /// Use [`try_new`](Self::try_new) for explicit error handling.
    pub fn new(filter_type: BiquadFilterType, freq: f64, srate: f64, q: f64, db_gain: f64) -> Self {
        let mut biquad = Biquad {
            filter_type,
            freq,
            srate,
            q,
            db_gain,
            a1: 0.0,
            a2: 0.0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            r_up0: 0.0,
            r_up1: 0.0,
            r_up2: 0.0,
            r_dw0: 0.0,
            r_dw1: 0.0,
            r_dw2: 0.0,
        };

        // Adjust Q based on filter type, matching Python logic
        if biquad.filter_type == BiquadFilterType::Notch {
            biquad.q = 30.0;
        } else if biquad.q == 0.0 {
            match biquad.filter_type {
                BiquadFilterType::Bandpass
                | BiquadFilterType::Highpass
                | BiquadFilterType::Lowpass => {
                    biquad.q = DEFAULT_Q_HIGH_LOW_PASS;
                }
                BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => {
                    biquad.q = DEFAULT_Q_HIGH_LOW_SHELF;
                }
                _ => {}
            }
        }

        // Safety clamp: ensure strictly positive Q to avoid division by zero in alpha = sn/(2*q)
        if biquad.q <= 0.0 {
            biquad.q = 1.0e-2;
        }

        biquad.compute_coeffs();
        biquad
    }

    /// Creates a new Biquad filter with validation.
    ///
    /// Unlike [`new`](Self::new), this method returns an error for invalid parameters
    /// instead of silently clamping them.
    ///
    /// # Arguments
    ///
    /// * `filter_type` - The type of filter to create
    /// * `freq` - Center/cutoff frequency in Hz (must be > 0 and < Nyquist)
    /// * `srate` - Sample rate in Hz (must be > 0)
    /// * `q` - Q factor (must be > 0, or 0.0 for default)
    /// * `db_gain` - Gain in dB (must be finite)
    ///
    /// # Errors
    ///
    /// Returns `IirError::InvalidSampleRate` if sample rate is <= 0.
    /// Returns `IirError::InvalidFrequency` if frequency is <= 0 or >= Nyquist.
    /// Returns `IirError::InvalidQ` if Q is negative (but not zero, which uses default).
    /// Returns `IirError::InvalidGain` if gain is not finite.
    ///
    /// # Example
    ///
    /// ```rust
    /// use autoeq_iir::{Biquad, BiquadFilterType, SRATE};
    ///
    /// // Valid filter
    /// let filter = Biquad::try_new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0)
    ///     .expect("valid parameters");
    ///
    /// // Invalid frequency (above Nyquist)
    /// let result = Biquad::try_new(BiquadFilterType::Peak, 30000.0, SRATE, 2.0, 0.0);
    /// assert!(result.is_err());
    /// ```
    pub fn try_new(
        filter_type: BiquadFilterType,
        freq: f64,
        srate: f64,
        q: f64,
        db_gain: f64,
    ) -> Result<Self, IirError> {
        // Validate sample rate
        if srate <= 0.0 || !srate.is_finite() {
            return Err(IirError::InvalidSampleRate { sample_rate: srate });
        }

        let nyquist = srate / 2.0;

        // Validate frequency
        if freq <= 0.0 || freq >= nyquist || !freq.is_finite() {
            return Err(IirError::InvalidFrequency { freq, nyquist });
        }

        // Validate Q (0.0 is allowed as it means "use default")
        if q < 0.0 || (q != 0.0 && !q.is_finite()) {
            return Err(IirError::InvalidQ { q });
        }

        // Validate gain
        if !db_gain.is_finite() {
            return Err(IirError::InvalidGain { gain_db: db_gain });
        }

        Ok(Self::new(filter_type, freq, srate, q, db_gain))
    }

    fn compute_coeffs(&mut self) {
        // Intermediate variables
        let a = 10.0_f64.powf(self.db_gain / 40.0);
        let omega = 2.0 * PI * self.freq / self.srate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * self.q);
        let beta = (a + a).sqrt();

        // Raw coefficients
        let (b0, b1, b2, a0, a1, a2);

        match self.filter_type {
            BiquadFilterType::Lowpass => {
                b0 = (1.0 - cs) / 2.0;
                b1 = 1.0 - cs;
                b2 = (1.0 - cs) / 2.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => {
                b0 = (1.0 + cs) / 2.0;
                b1 = -(1.0 + cs);
                b2 = (1.0 + cs) / 2.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Bandpass => {
                b0 = alpha;
                b1 = 0.0;
                b2 = -alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Notch => {
                b0 = 1.0;
                b1 = -2.0 * cs;
                b2 = 1.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Peak => {
                b0 = 1.0 + (alpha * a);
                b1 = -2.0 * cs;
                b2 = 1.0 - (alpha * a);
                a0 = 1.0 + (alpha / a);
                a1 = -2.0 * cs;
                a2 = 1.0 - (alpha / a);
            }
            BiquadFilterType::Lowshelf => {
                b0 = a * ((a + 1.0) - (a - 1.0) * cs + beta * sn);
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cs);
                b2 = a * ((a + 1.0) - (a - 1.0) * cs - beta * sn);
                a0 = (a + 1.0) + (a - 1.0) * cs + beta * sn;
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cs);
                a2 = (a + 1.0) + (a - 1.0) * cs - beta * sn;
            }
            BiquadFilterType::Highshelf => {
                b0 = a * ((a + 1.0) + (a - 1.0) * cs + beta * sn);
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cs);
                b2 = a * ((a + 1.0) + (a - 1.0) * cs - beta * sn);
                a0 = (a + 1.0) - (a - 1.0) * cs + beta * sn;
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cs);
                a2 = (a + 1.0) - (a - 1.0) * cs - beta * sn;
            }
        }

        // Normalize coefficients
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;

        // Pre-compute for result()
        self.r_up0 = (self.b0 + self.b1 + self.b2).powi(2);
        self.r_up1 = -4.0 * (self.b0 * self.b1 + 4.0 * self.b0 * self.b2 + self.b1 * self.b2);
        self.r_up2 = 16.0 * self.b0 * self.b2;
        self.r_dw0 = (1.0 + self.a1 + self.a2).powi(2);
        self.r_dw1 = -4.0 * (self.a1 + 4.0 * self.a2 + self.a1 * self.a2);
        self.r_dw2 = 16.0 * self.a2;
    }

    /// Processes a single audio sample through the filter.
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;

        y
    }

    /// Calculates the filter's magnitude response at a single frequency `f`.
    pub fn result(&self, f: f64) -> f64 {
        let phi = (PI * f / self.srate).sin().powi(2);
        let phi2 = phi * phi;

        let numerator = self.r_up0 + self.r_up1 * phi + self.r_up2 * phi2;
        let denominator = self.r_dw0 + self.r_dw1 * phi + self.r_dw2 * phi2;

        let result = (numerator / denominator).max(0.0);
        result.sqrt()
    }

    /// Calculates the filter's response in dB at a single frequency `f`.
    pub fn log_result(&self, f: f64) -> f64 {
        let result = self.result(f);
        if result > 0.0 {
            20.0 * result.log10()
        } else {
            -200.0 // Return a large negative number for silence
        }
    }

    /// Vectorized version to compute the SPL response for a vector of frequencies.
    /// This is the fast equivalent of the `np_log_result` Python method.
    pub fn np_log_result(&self, freq: &Array1<f64>) -> Array1<f64> {
        let coeff = PI / self.srate;
        let phi = (freq * coeff).mapv(f64::sin).mapv(|x| x.powi(2));
        let phi2 = &phi * &phi;

        let r_up = self.r_up0 + self.r_up1 * &phi + self.r_up2 * &phi2;
        let r_dw = self.r_dw0 + self.r_dw1 * &phi + self.r_dw2 * &phi2;
        let r = r_up / r_dw;

        // Clip to a minimum value to avoid log(0), then calculate dB
        let min_val = 1.0e-20;

        r.mapv(|val| val.max(min_val))
            .mapv(f64::sqrt)
            .mapv(f64::log10)
            * 20.0
    }

    /// Returns the filter coefficients as a tuple.
    pub fn constants(&self) -> (f64, f64, f64, f64, f64) {
        (self.a1, self.a2, self.b0, self.b1, self.b2)
    }
}

/// Implement the Display trait for pretty-printing, similar to __str__.
impl fmt::Display for Biquad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Type:{},Freq:{:.1},Rate:{:.1},Q:{:.1},Gain:{:.1}",
            self.filter_type.short_name(),
            self.freq,
            self.srate,
            self.q,
            self.db_gain
        )
    }
}

/// Represents a single filter in a parametric equalizer.
#[derive(Debug, Clone, Default)]
///
/// Center frequency in Hz
/// Q factor (quality factor)
/// Gain in dB
/// Type of filter (e.g., "PK", "LP", "HP")
pub struct FilterRow {
    /// Center frequency in Hz
    pub freq: f64,
    /// Q factor (quality factor)
    pub q: f64,
    /// Gain in dB
    pub gain: f64,
    /// Type of filter (e.g., "PK", "LP", "HP")
    pub kind: &'static str,
}

/// Compute the combined PEQ response (in dB) on a given frequency grid for a Peq.
///
/// # Arguments
/// * `freqs` - Frequency points for evaluation (Hz)
/// * `peq` - Parametric equalizer containing weighted biquad filters
/// * `_sample_rate` - Sample rate in Hz (unused, kept for API compatibility)
///
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn compute_peq_response(freqs: &Array1<f64>, peq: &Peq, _sample_rate: f64) -> Array1<f64> {
    if peq.is_empty() {
        return Array1::zeros(freqs.len());
    }
    let mut response = Array1::zeros(freqs.len());
    for (weight, filter) in peq {
        // Note: we're not using sample_rate here as filters already have their own srate
        response += &(filter.np_log_result(freqs) * *weight);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bw2q;
    use ndarray::array;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_bw_q_roundtrip() {
        let qs = [0.5, 1.0, 2.0, 5.0];
        for &q in &qs {
            let bw = q2bw(q);
            let q2 = bw2q(bw);
            assert!(
                approx_eq(q, q2, 1e-9),
                "roundtrip failed: q={} -> bw={} -> q2={}",
                q,
                bw,
                q2
            );
        }
    }

    #[test]
    fn test_biquad_np_log_result_is_finite() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 1.0, 6.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = bq.np_log_result(&freqs);
        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }

    #[test]
    fn peak_with_zero_q_is_safely_clamped() {
        // q==0 for Peak should be clamped internally to a small positive value
        let bq = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 0.0, 3.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = bq.np_log_result(&freqs);
        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }

    #[test]
    fn test_a_weighting() {
        // Test A-weighting at specific frequencies
        // At 1kHz, A-weighting should be close to 0 dB
        let w_1k = a_weighting_db(1000.0);
        assert!(
            (w_1k - 0.0).abs() < 1.0,
            "A-weighting at 1kHz should be ~0 dB"
        );

        // At 100Hz, A-weighting should be significantly negative (low frequencies attenuated)
        let w_100 = a_weighting_db(100.0);
        assert!(w_100 < -15.0, "A-weighting at 100Hz should be < -15 dB");

        // At 4kHz, A-weighting should be slightly positive
        let w_4k = a_weighting_db(4000.0);
        assert!(w_4k > 0.0, "A-weighting at 4kHz should be positive");
    }

    #[test]
    fn test_k_weighting() {
        // Test K-weighting approximation
        // Below 38Hz should be heavily attenuated
        let w_30 = k_weighting_db(30.0);
        assert!(w_30 < -5.0, "K-weighting at 30Hz should be attenuated");

        // Mid-frequencies should have less attenuation
        let w_1k = k_weighting_db(1000.0);
        assert!(
            w_1k > w_30,
            "K-weighting at 1kHz should be less attenuated than 30Hz"
        );

        // High frequencies should have boost
        let w_5k = k_weighting_db(5000.0);
        assert!(
            w_5k > w_1k,
            "K-weighting at 5kHz should have more gain than 1kHz"
        );
    }

    #[test]
    fn test_peq_loudness_gain_flat() {
        // Empty PEQ should return 0 dB
        let peq: Peq = vec![];
        let gain = peq_loudness_gain(&peq, "k");
        assert_eq!(gain, 0.0);
    }

    #[test]
    fn test_peq_loudness_gain_boost() {
        // PEQ with +6 dB peak at 1kHz should require negative gain compensation
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];

        let gain_k = peq_loudness_gain(&peq, "k");
        let gain_a = peq_loudness_gain(&peq, "a");

        println!("Test: +6 dB peak at 1kHz");
        println!("  K-weighted gain: {:.2} dB", gain_k);
        println!("  A-weighted gain: {:.2} dB", gain_a);

        // Should be negative (reducing gain to compensate for boost)
        assert!(
            gain_k < 0.0,
            "Gain compensation for boost should be negative (K-weighting)"
        );
        assert!(
            gain_a < 0.0,
            "Gain compensation for boost should be negative (A-weighting)"
        );

        // Should be roughly in the range of -1 to -4 dB for a +6 dB peak
        assert!(
            gain_k > -5.0 && gain_k < 0.0,
            "K-weighted gain should be between -5 and 0 dB"
        );
        assert!(
            gain_a > -5.0 && gain_a < 0.0,
            "A-weighted gain should be between -5 and 0 dB"
        );
    }

    #[test]
    fn test_peq_loudness_gain_demo() {
        println!("\n=== PEQ Loudness Compensation Demo ===\n");

        // Example 1: Mid-range boost
        println!("1. +6 dB peak at 1 kHz:");
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq1 = vec![(1.0, bq1)];
        println!(
            "   Anti-clip: {:.2} dB, K-weighted: {:.2} dB, A-weighted: {:.2} dB",
            peq_preamp_gain(&peq1),
            peq_loudness_gain(&peq1, "k"),
            peq_loudness_gain(&peq1, "a")
        );

        // Example 2: Bass boost
        println!("2. +6 dB bass at 100 Hz:");
        let bq2 = Biquad::new(BiquadFilterType::Peak, 100.0, 48000.0, 1.0, 6.0);
        let peq2 = vec![(1.0, bq2)];
        println!(
            "   Anti-clip: {:.2} dB, K-weighted: {:.2} dB, A-weighted: {:.2} dB",
            peq_preamp_gain(&peq2),
            peq_loudness_gain(&peq2, "k"),
            peq_loudness_gain(&peq2, "a")
        );

        // Example 3: Treble boost
        println!("3. +6 dB treble at 8 kHz:");
        let bq3 = Biquad::new(BiquadFilterType::Peak, 8000.0, 48000.0, 1.0, 6.0);
        let peq3 = vec![(1.0, bq3)];
        println!(
            "   Anti-clip: {:.2} dB, K-weighted: {:.2} dB, A-weighted: {:.2} dB",
            peq_preamp_gain(&peq3),
            peq_loudness_gain(&peq3, "k"),
            peq_loudness_gain(&peq3, "a")
        );

        // Example 4: V-shape EQ
        println!("4. V-shape: bass+4dB, mid-3dB, treble+3dB:");
        let bass = Biquad::new(BiquadFilterType::Lowshelf, 150.0, 48000.0, 0.7, 4.0);
        let mid = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -3.0);
        let treble = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 3.0);
        let peq4 = vec![(1.0, bass), (1.0, mid), (1.0, treble)];
        println!(
            "   Anti-clip: {:.2} dB, K-weighted: {:.2} dB, A-weighted: {:.2} dB",
            peq_preamp_gain(&peq4),
            peq_loudness_gain(&peq4, "k"),
            peq_loudness_gain(&peq4, "a")
        );

        println!("\nNote: Anti-clip prevents clipping, K/A-weighted maintains loudness balance");
    }

    #[test]
    fn test_peq_loudness_gain_cut() {
        // PEQ with -6 dB cut at 1kHz should require positive gain compensation
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -6.0);
        let peq = vec![(1.0, bq)];

        let gain_k = peq_loudness_gain(&peq, "k");
        let gain_a = peq_loudness_gain(&peq, "a");

        // Should be positive (increasing gain to compensate for cut)
        assert!(
            gain_k > 0.0,
            "Gain compensation for cut should be positive (K-weighting)"
        );
        assert!(
            gain_a > 0.0,
            "Gain compensation for cut should be positive (A-weighting)"
        );
    }

    #[test]
    fn test_peq_loudness_gain_bass_boost() {
        // Bass boost (100 Hz) should have different impact with K vs A weighting
        let bq = Biquad::new(BiquadFilterType::Peak, 100.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];

        let gain_k = peq_loudness_gain(&peq, "k");
        let gain_a = peq_loudness_gain(&peq, "a");

        // Both should be negative (compensation for boost)
        assert!(gain_k < 0.0);
        assert!(gain_a < 0.0);

        // A-weighting attenuates low frequencies more, so compensation should be less negative
        assert!(
            gain_a > gain_k,
            "A-weighted gain should be less negative (bass is less perceptually important)"
        );
    }

    // ========================================================================
    // Biquad Filter Type Tests
    // ========================================================================

    #[test]
    fn test_lowpass_filter_response() {
        let cutoff = 1000.0;
        let lp = Biquad::new(BiquadFilterType::Lowpass, cutoff, 48000.0, 0.0, 0.0);

        // DC response should be ~0 dB (unity gain)
        let dc_response = lp.log_result(10.0);
        assert!(
            approx_eq(dc_response, 0.0, 0.5),
            "Lowpass DC response should be ~0 dB, got {}",
            dc_response
        );

        // At cutoff, response should be ~-3 dB
        let cutoff_response = lp.log_result(cutoff);
        assert!(
            approx_eq(cutoff_response, -3.0, 0.5),
            "Lowpass at cutoff should be ~-3 dB, got {}",
            cutoff_response
        );

        // Well above cutoff should be attenuated
        let high_response = lp.log_result(10000.0);
        assert!(
            high_response < -20.0,
            "Lowpass at 10x cutoff should be < -20 dB, got {}",
            high_response
        );
    }

    #[test]
    fn test_highpass_filter_response() {
        let cutoff = 1000.0;
        let hp = Biquad::new(BiquadFilterType::Highpass, cutoff, 48000.0, 0.0, 0.0);

        // Well below cutoff should be attenuated
        let low_response = hp.log_result(100.0);
        assert!(
            low_response < -20.0,
            "Highpass at 0.1x cutoff should be < -20 dB, got {}",
            low_response
        );

        // At cutoff, response should be ~-3 dB
        let cutoff_response = hp.log_result(cutoff);
        assert!(
            approx_eq(cutoff_response, -3.0, 0.5),
            "Highpass at cutoff should be ~-3 dB, got {}",
            cutoff_response
        );

        // High frequency response should be ~0 dB
        let high_response = hp.log_result(10000.0);
        assert!(
            approx_eq(high_response, 0.0, 0.5),
            "Highpass high freq response should be ~0 dB, got {}",
            high_response
        );
    }

    #[test]
    fn test_bandpass_filter_response() {
        let center = 1000.0;
        let bp = Biquad::new(BiquadFilterType::Bandpass, center, 48000.0, 1.0, 0.0);

        // At center frequency, response should be maximum
        let center_response = bp.log_result(center);

        // Well below center should be attenuated
        let low_response = bp.log_result(100.0);
        assert!(
            low_response < center_response - 10.0,
            "Bandpass below center should be attenuated"
        );

        // Well above center should be attenuated
        let high_response = bp.log_result(10000.0);
        assert!(
            high_response < center_response - 10.0,
            "Bandpass above center should be attenuated"
        );
    }

    #[test]
    fn test_notch_filter_response() {
        let center = 1000.0;
        let notch = Biquad::new(BiquadFilterType::Notch, center, 48000.0, 0.0, 0.0);

        // At center frequency, response should be deeply attenuated
        let center_response = notch.log_result(center);
        assert!(
            center_response < -30.0,
            "Notch at center should be < -30 dB, got {}",
            center_response
        );

        // Away from center should be ~0 dB
        let low_response = notch.log_result(100.0);
        assert!(
            approx_eq(low_response, 0.0, 1.0),
            "Notch away from center should be ~0 dB, got {}",
            low_response
        );

        let high_response = notch.log_result(10000.0);
        assert!(
            approx_eq(high_response, 0.0, 1.0),
            "Notch away from center should be ~0 dB, got {}",
            high_response
        );
    }

    #[test]
    fn test_peak_filter_boost() {
        let center = 1000.0;
        let gain_db = 6.0;
        let peak = Biquad::new(BiquadFilterType::Peak, center, 48000.0, 2.0, gain_db);

        // At center frequency, response should match gain
        let center_response = peak.log_result(center);
        assert!(
            approx_eq(center_response, gain_db, 0.5),
            "Peak at center should be ~{} dB, got {}",
            gain_db,
            center_response
        );

        // Away from center should approach 0 dB
        let low_response = peak.log_result(100.0);
        assert!(
            low_response.abs() < 1.0,
            "Peak away from center should be ~0 dB, got {}",
            low_response
        );
    }

    #[test]
    fn test_peak_filter_cut() {
        let center = 1000.0;
        let gain_db = -6.0;
        let peak = Biquad::new(BiquadFilterType::Peak, center, 48000.0, 2.0, gain_db);

        // At center frequency, response should match gain
        let center_response = peak.log_result(center);
        assert!(
            approx_eq(center_response, gain_db, 0.5),
            "Peak cut at center should be ~{} dB, got {}",
            gain_db,
            center_response
        );
    }

    #[test]
    fn test_lowshelf_filter_response() {
        let freq = 200.0;
        let gain_db = 6.0;
        let ls = Biquad::new(BiquadFilterType::Lowshelf, freq, 48000.0, 0.7, gain_db);

        // Well below shelf frequency should have full gain
        let low_response = ls.log_result(20.0);
        assert!(
            approx_eq(low_response, gain_db, 1.0),
            "Lowshelf below freq should be ~{} dB, got {}",
            gain_db,
            low_response
        );

        // Well above shelf frequency should be ~0 dB
        let high_response = ls.log_result(5000.0);
        assert!(
            approx_eq(high_response, 0.0, 1.0),
            "Lowshelf above freq should be ~0 dB, got {}",
            high_response
        );
    }

    #[test]
    fn test_highshelf_filter_response() {
        let freq = 5000.0;
        let gain_db = 6.0;
        let hs = Biquad::new(BiquadFilterType::Highshelf, freq, 48000.0, 0.7, gain_db);

        // Well below shelf frequency should be ~0 dB
        let low_response = hs.log_result(100.0);
        assert!(
            approx_eq(low_response, 0.0, 1.0),
            "Highshelf below freq should be ~0 dB, got {}",
            low_response
        );

        // Well above shelf frequency should have full gain
        let high_response = hs.log_result(20000.0);
        assert!(
            approx_eq(high_response, gain_db, 1.0),
            "Highshelf above freq should be ~{} dB, got {}",
            gain_db,
            high_response
        );
    }

    // ========================================================================
    // Biquad try_new Validation Tests
    // ========================================================================

    #[test]
    fn test_try_new_valid_parameters() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 3.0);
        assert!(result.is_ok());
        let bq = result.unwrap();
        assert_eq!(bq.filter_type, BiquadFilterType::Peak);
        assert_eq!(bq.freq, 1000.0);
        assert_eq!(bq.srate, 48000.0);
        assert_eq!(bq.q, 2.0);
        assert_eq!(bq.db_gain, 3.0);
    }

    #[test]
    fn test_try_new_invalid_sample_rate_zero() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, 0.0, 2.0, 3.0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::IirError::InvalidSampleRate { .. }
        ));
    }

    #[test]
    fn test_try_new_invalid_sample_rate_negative() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, -48000.0, 2.0, 3.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_new_invalid_frequency_zero() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 0.0, 48000.0, 2.0, 3.0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::IirError::InvalidFrequency { .. }
        ));
    }

    #[test]
    fn test_try_new_invalid_frequency_above_nyquist() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 30000.0, 48000.0, 2.0, 3.0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::IirError::InvalidFrequency { .. }
        ));
    }

    #[test]
    fn test_try_new_invalid_q_negative() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, 48000.0, -1.0, 3.0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::IirError::InvalidQ { .. }));
    }

    #[test]
    fn test_try_new_q_zero_uses_default() {
        // Q=0 should use default, not error
        let result = Biquad::try_new(BiquadFilterType::Lowpass, 1000.0, 48000.0, 0.0, 0.0);
        assert!(result.is_ok());
        let bq = result.unwrap();
        // Default Q for lowpass is 1/sqrt(2)
        assert!(approx_eq(bq.q, crate::DEFAULT_Q_HIGH_LOW_PASS, 0.01));
    }

    #[test]
    fn test_try_new_invalid_gain_infinite() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, f64::INFINITY);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::IirError::InvalidGain { .. }));
    }

    #[test]
    fn test_try_new_invalid_gain_nan() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, f64::NAN);
        assert!(result.is_err());
    }

    // ========================================================================
    // Biquad Sample Processing Tests
    // ========================================================================

    #[test]
    fn test_biquad_process_dc_lowpass() {
        let mut lp = Biquad::new(BiquadFilterType::Lowpass, 1000.0, 48000.0, 0.0, 0.0);

        // Process DC signal (all 1.0)
        let mut output = 0.0;
        for _ in 0..1000 {
            output = lp.process(1.0);
        }

        // Lowpass should pass DC with unity gain
        assert!(
            approx_eq(output, 1.0, 0.01),
            "Lowpass should pass DC, got {}",
            output
        );
    }

    #[test]
    fn test_biquad_process_dc_highpass() {
        let mut hp = Biquad::new(BiquadFilterType::Highpass, 1000.0, 48000.0, 0.0, 0.0);

        // Process DC signal (all 1.0)
        let mut output = 0.0;
        for _ in 0..1000 {
            output = hp.process(1.0);
        }

        // Highpass should block DC
        assert!(
            output.abs() < 0.01,
            "Highpass should block DC, got {}",
            output
        );
    }

    #[test]
    fn test_biquad_process_impulse_response() {
        let mut peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);

        // Process impulse
        let first = peak.process(1.0);
        let second = peak.process(0.0);
        let third = peak.process(0.0);

        // First output should be non-zero
        assert!(first.abs() > 0.0, "Impulse response should be non-zero");

        // Filter should ring (subsequent outputs non-zero)
        assert!(
            second.abs() > 0.0 || third.abs() > 0.0,
            "Peak filter should ring after impulse"
        );
    }

    #[test]
    fn test_biquad_constants() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let (a1, a2, b0, b1, b2) = bq.constants();

        // Coefficients should be finite
        assert!(a1.is_finite());
        assert!(a2.is_finite());
        assert!(b0.is_finite());
        assert!(b1.is_finite());
        assert!(b2.is_finite());

        // b0 should be non-zero for a valid filter
        assert!(b0.abs() > 0.0);
    }

    #[test]
    fn test_biquad_display() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let display = format!("{}", bq);

        assert!(display.contains("PK"));
        assert!(display.contains("1000"));
        assert!(display.contains("48000"));
        assert!(display.contains("2.0"));
        assert!(display.contains("6.0"));
    }

    // ========================================================================
    // PEQ Function Tests
    // ========================================================================

    #[test]
    fn test_peq_equal_identical() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let peq1 = vec![(1.0, bq1)];
        let peq2 = vec![(1.0, bq2)];

        assert!(peq_equal(&peq1, &peq2));
    }

    #[test]
    fn test_peq_equal_different_weight() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let peq1 = vec![(1.0, bq1)];
        let peq2 = vec![(0.5, bq2)];

        assert!(!peq_equal(&peq1, &peq2));
    }

    #[test]
    fn test_peq_equal_different_filter_type() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let bq2 = Biquad::new(BiquadFilterType::Lowshelf, 1000.0, 48000.0, 2.0, 6.0);
        let peq1 = vec![(1.0, bq1)];
        let peq2 = vec![(1.0, bq2)];

        assert!(!peq_equal(&peq1, &peq2));
    }

    #[test]
    fn test_peq_equal_different_length() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let bq3 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, 3.0);
        let peq1 = vec![(1.0, bq1)];
        let peq2 = vec![(1.0, bq2), (1.0, bq3)];

        assert!(!peq_equal(&peq1, &peq2));
    }

    #[test]
    fn test_peq_equal_empty() {
        let peq1: Peq = vec![];
        let peq2: Peq = vec![];

        assert!(peq_equal(&peq1, &peq2));
    }

    #[test]
    fn test_compute_peq_response_empty() {
        let peq: Peq = vec![];
        let freqs = array![100.0, 1000.0, 10000.0];
        let response = compute_peq_response(&freqs, &peq, 48000.0);

        assert_eq!(response.len(), 3);
        for val in response.iter() {
            assert_eq!(*val, 0.0);
        }
    }

    #[test]
    fn test_compute_peq_response_single_filter() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let peq = vec![(1.0, bq.clone())];
        let freqs = array![100.0, 1000.0, 10000.0];
        let response = compute_peq_response(&freqs, &peq, 48000.0);

        // Response at center should be ~6 dB
        assert!(approx_eq(response[1], 6.0, 0.5));

        // Response away from center should be ~0 dB
        assert!(response[0].abs() < 1.0);
        assert!(response[2].abs() < 1.0);
    }

    #[test]
    fn test_compute_peq_response_weighted() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let peq = vec![(0.5, bq)]; // Half weight
        let freqs = array![1000.0];
        let response = compute_peq_response(&freqs, &peq, 48000.0);

        // Response should be half of 6 dB = 3 dB
        assert!(approx_eq(response[0], 3.0, 0.5));
    }

    #[test]
    fn test_compute_peq_response_multiple_filters() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 500.0, 48000.0, 2.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, 3.0);
        let peq = vec![(1.0, bq1), (1.0, bq2)];
        let freqs = array![500.0, 1000.0, 2000.0];
        let response = compute_peq_response(&freqs, &peq, 48000.0);

        // Peaks at 500 Hz and 2000 Hz
        assert!(response[0] > response[1]); // 500 Hz peak
        assert!(response[2] > response[1]); // 2000 Hz peak
    }

    #[test]
    fn test_peq_spl_matches_compute_peq_response() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let peq = vec![(1.0, bq)];
        let freqs = array![100.0, 1000.0, 10000.0];

        let spl = peq_spl(&freqs, &peq);
        let response = compute_peq_response(&freqs, &peq, 48000.0);

        for (s, r) in spl.iter().zip(response.iter()) {
            assert!(approx_eq(*s, *r, 1e-10));
        }
    }

    // ========================================================================
    // Filter Type Name Tests
    // ========================================================================

    #[test]
    fn test_filter_type_short_names() {
        assert_eq!(BiquadFilterType::Lowpass.short_name(), "LP");
        assert_eq!(BiquadFilterType::Highpass.short_name(), "HP");
        assert_eq!(BiquadFilterType::HighpassVariableQ.short_name(), "HPQ");
        assert_eq!(BiquadFilterType::Bandpass.short_name(), "BP");
        assert_eq!(BiquadFilterType::Peak.short_name(), "PK");
        assert_eq!(BiquadFilterType::Notch.short_name(), "NO");
        assert_eq!(BiquadFilterType::Lowshelf.short_name(), "LS");
        assert_eq!(BiquadFilterType::Highshelf.short_name(), "HS");
    }

    #[test]
    fn test_filter_type_long_names() {
        assert_eq!(BiquadFilterType::Lowpass.long_name(), "Lowpass");
        assert_eq!(BiquadFilterType::Highpass.long_name(), "Highpass");
        assert_eq!(
            BiquadFilterType::HighpassVariableQ.long_name(),
            "HighpassVariableQ"
        );
        assert_eq!(BiquadFilterType::Bandpass.long_name(), "Bandpass");
        assert_eq!(BiquadFilterType::Peak.long_name(), "Peak");
        assert_eq!(BiquadFilterType::Notch.long_name(), "Notch");
        assert_eq!(BiquadFilterType::Lowshelf.long_name(), "Lowshelf");
        assert_eq!(BiquadFilterType::Highshelf.long_name(), "Highshelf");
    }

    // ========================================================================
    // FilterRow Tests
    // ========================================================================

    #[test]
    fn test_filter_row_default() {
        let row = FilterRow::default();
        assert_eq!(row.freq, 0.0);
        assert_eq!(row.q, 0.0);
        assert_eq!(row.gain, 0.0);
        assert_eq!(row.kind, "");
    }

    #[test]
    fn test_filter_row_creation() {
        let row = FilterRow {
            freq: 1000.0,
            q: 2.0,
            gain: 6.0,
            kind: "PK",
        };
        assert_eq!(row.freq, 1000.0);
        assert_eq!(row.q, 2.0);
        assert_eq!(row.gain, 6.0);
        assert_eq!(row.kind, "PK");
    }
}

/// Check if two PEQs are equal
///
/// Compares two PEQ vectors for equality, checking both weights and biquad parameters
pub fn peq_equal(left: &Peq, right: &Peq) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.iter().zip(right.iter()).all(|((w1, b1), (w2, b2))| {
        // Compare weights
        (w1 - w2).abs() < f64::EPSILON &&
        // Compare biquad parameters
        b1.filter_type == b2.filter_type &&
        (b1.freq - b2.freq).abs() < f64::EPSILON &&
        (b1.srate - b2.srate).abs() < f64::EPSILON &&
        (b1.q - b2.q).abs() < f64::EPSILON &&
        (b1.db_gain - b2.db_gain).abs() < f64::EPSILON
    })
}

/// Compute SPL for each frequency given a PEQ
///
/// # Arguments
/// * `freq` - Array of frequencies to compute response for
/// * `peq` - PEQ vector containing weighted biquad filters
///
/// # Returns
/// * Array of SPL values in dB for each frequency
pub fn peq_spl(freq: &Array1<f64>, peq: &Peq) -> Array1<f64> {
    let mut current_filter = Array1::zeros(freq.len());

    for (weight, iir) in peq {
        current_filter += &(iir.np_log_result(freq) * *weight);
    }

    current_filter
}

/// Compute A-weighting in dB for a given frequency
///
/// A-weighting approximates the frequency response of the human ear
/// and is used for loudness estimation.
///
/// # Arguments
/// * `f` - Frequency in Hz
///
/// # Returns
/// * A-weighting value in dB
fn a_weighting_db(f: f64) -> f64 {
    // A-weighting formula (IEC 61672-1)
    let f2 = f * f;
    let f4 = f2 * f2;

    let numerator = 12194.0_f64.powi(2) * f4;
    let denominator = (f2 + 20.6_f64.powi(2))
        * ((f2 + 107.7_f64.powi(2)) * (f2 + 737.9_f64.powi(2))).sqrt()
        * (f2 + 12194.0_f64.powi(2));

    let ra = numerator / denominator;
    20.0 * ra.log10() + 2.0 // +2.0 is normalization constant
}

/// Compute K-weighting in dB for a given frequency
///
/// K-weighting is used by EBU R128 loudness measurement standard.
/// It's composed of a pre-filter and RLB weighting.
///
/// # Arguments
/// * `f` - Frequency in Hz
///
/// # Returns
/// * K-weighting value in dB (approximate)
fn k_weighting_db(f: f64) -> f64 {
    // Simplified K-weighting approximation
    // Based on high-shelf at ~1500Hz and high-pass around 40Hz

    // High-pass stage (4th order Butterworth at 38Hz)
    let f_hp = 38.0;
    let hp_response = if f > 1.0 {
        20.0 * 4.0 * (f / f_hp).log10() // 4th order = 80 dB/decade
    } else {
        -200.0
    };
    let hp_gain = hp_response.min(0.0);

    // High-shelf stage (+4 dB above 1500 Hz)
    let f_hs = 1500.0;
    let hs_gain = if f > f_hs {
        4.0 * (1.0 - (f_hs / f).powf(2.0).min(1.0))
    } else {
        0.0
    };

    hp_gain + hs_gain
}

/// Compute loudness-weighted gain adjustment for PEQ to maintain spectral balance
///
/// This function estimates the perceived loudness change caused by a PEQ
/// by analyzing its frequency response with perceptual weighting.
/// Much faster than full Replay Gain analysis.
///
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
/// * `weighting` - Weighting type: "a" for A-weighting, "k" for K-weighting (EBU R128-like)
///
/// # Returns
/// * Gain adjustment in dB to maintain similar loudness (0 dB = no change needed)
///
/// # Example
/// ```no_run
/// use autoeq_iir::{Biquad, BiquadFilterType, peq_loudness_gain};
///
/// let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
/// let peq = vec![(1.0, bq)];
/// let gain_adj = peq_loudness_gain(&peq, "k");
/// println!("Apply {} dB to maintain loudness balance", gain_adj);
/// ```
pub fn peq_loudness_gain(peq: &Peq, weighting: &str) -> f64 {
    if peq.is_empty() {
        return 0.0;
    }

    // Generate logarithmic frequency array from 20Hz to 20kHz with 500 points
    // More points than preamp_gain for better loudness integration
    let n_points = 500;
    let freq = Array1::logspace(
        10.0,
        (2.0f64 * 10.0).log10(),
        (2.0f64 * 10000.0).log10(),
        n_points,
    );

    // Get PEQ frequency response in dB
    let peq_response_db = peq_spl(&freq, peq);

    // Apply perceptual weighting
    let weighted_change: f64 = freq
        .iter()
        .zip(peq_response_db.iter())
        .map(|(f, peq_db)| {
            let weight_db = match weighting {
                "a" => a_weighting_db(*f),
                "k" => k_weighting_db(*f),
                _ => 0.0, // No weighting
            };

            // Convert to linear domain for integration
            // Original: 10^(weight/20)
            // After PEQ: 10^((weight + peq)/20)
            // Ratio: 10^(peq/20)

            let weight_linear = 10.0_f64.powf(weight_db / 20.0);
            let peq_ratio = 10.0_f64.powf(*peq_db / 20.0);

            // Weighted energy change
            weight_linear * weight_linear * (peq_ratio * peq_ratio - 1.0)
        })
        .sum();

    // Average weighted energy change across frequency
    let avg_energy_change = weighted_change / n_points as f64;

    // Convert back to dB (half because we squared for energy)
    // Negative because we want to compensate (reduce if PEQ increases loudness)
    let loudness_change_db = 10.0 * (1.0 + avg_energy_change).log10();

    -loudness_change_db
}

/// Compute preamp gain for a PEQ: well adapted to computers
///
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
///
/// # Returns
/// * Preamp gain in dB (negative value to prevent clipping)
pub fn peq_preamp_gain(peq: &Peq) -> f64 {
    // Generate logarithmic frequency array from 20Hz to 20kHz with 200 points
    let freq = Array1::logspace(
        10.0,
        (2.0f64 * 10.0).log10(),
        (2.0f64 * 10000.0).log10(),
        200,
    );
    let spl = peq_spl(&freq, peq);

    // Find maximum positive gain and return its negative
    let overall = spl
        .iter()
        .cloned()
        .fold(0.0f64, |acc, x| acc.max(x.max(0.0)));
    -overall
}

/// Compute preamp gain for a PEQ and look at the worst case
///
/// Note that we add 0.2 dB to have a margin for clipping
///
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
///
/// # Returns
/// * Preamp gain in dB (negative value to prevent clipping)
pub fn peq_preamp_gain_max(peq: &Peq) -> f64 {
    if peq.is_empty() {
        return 0.0;
    }

    // Generate logarithmic frequency array from 20Hz to 20kHz with 200 points
    let freq = Array1::logspace(
        10.0,
        (2.0f64 * 10.0).log10(),
        (2.0f64 * 10000.0).log10(),
        200,
    );
    let spl = peq_spl(&freq, peq);

    // Find maximum individual filter contribution
    let mut individual: f64 = 0.0;
    for (_, iir) in peq {
        let single_peq = vec![(1.0, iir.clone())];
        let single_spl = peq_spl(&freq, &single_peq);
        let single_max = single_spl.iter().cloned().fold(0.0f64, |acc, x| acc.max(x));
        individual = individual.max(single_max);
    }

    // Find overall maximum positive gain
    let overall = spl
        .iter()
        .cloned()
        .fold(0.0f64, |acc, x| acc.max(x.max(0.0)));

    // Take worst case and add safety margin
    -(individual.max(overall) + 0.2)
}

/// Format PEQ as APO configuration string
///
/// # Arguments
/// * `comment` - Comment string to include at the top
/// * `peq` - PEQ vector containing weighted biquad filters
///
/// # Returns
/// * String formatted for EqualizerAPO
pub fn peq_format_apo(comment: &str, peq: &Peq) -> String {
    let mut res = Vec::new();
    res.push(comment.to_string());
    res.push(format!("Preamp: {:.1} dB", peq_preamp_gain(peq)));
    res.push(String::new());

    // Sort filters by frequency in ascending order
    let mut sorted_peq: Vec<(f64, &Biquad)> = peq.iter().map(|(_, iir)| (iir.freq, iir)).collect();
    sorted_peq.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (i, (_, iir)) in sorted_peq.iter().enumerate() {
        match iir.filter_type {
            BiquadFilterType::Peak | BiquadFilterType::Notch | BiquadFilterType::Bandpass => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:0.2}",
                    i + 1,
                    iir.filter_type.short_name(),
                    iir.freq as i32,
                    iir.db_gain,
                    iir.q
                ));
            }
            BiquadFilterType::Lowpass | BiquadFilterType::Highpass => {
                if (iir.q - DEFAULT_Q_HIGH_LOW_PASS).abs() < f64::EPSILON {
                    res.push(format!(
                        "Filter {:2}: ON {:2} Fc {:5} Hz",
                        i + 1,
                        iir.filter_type.short_name(),
                        iir.freq as i32
                    ));
                } else {
                    res.push(format!(
                        "Filter {:2}: ON {:2}Q Fc {:5} Hz Q {:0.2}",
                        i + 1,
                        iir.filter_type.short_name(),
                        iir.freq as i32,
                        iir.q
                    ));
                }
            }
            BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:.2}",
                    i + 1,
                    iir.filter_type.short_name(),
                    iir.freq as i32,
                    iir.db_gain,
                    iir.q
                ));
            }
            BiquadFilterType::HighpassVariableQ => {
                res.push(format!(
                    "Filter {:2}: ON HPQ Fc {:5} Hz Q {:0.2}",
                    i + 1,
                    iir.freq as i32,
                    iir.q
                ));
            }
        }
    }

    res.push(String::new());
    res.join("\n")
}

/// Compute Q values for Butterworth filters
///
/// # Arguments
/// * `order` - Filter order
///
/// # Returns
/// * Vector of Q values for each biquad section
pub fn peq_butterworth_q(order: usize) -> Vec<f64> {
    let odd = !order.is_multiple_of(2);
    let mut q_values = Vec::new();

    for i in 0..order / 2 {
        let q = 2.0 * (PI / order as f64 * (i as f64 + 0.5)).sin();
        q_values.push(1.0 / q);
    }

    if odd {
        q_values.push(-1.0);
    }

    q_values
}

/// Create Butterworth lowpass filter
///
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
///
/// # Returns
/// * PEQ containing the Butterworth lowpass filter sections
pub fn peq_butterworth_lowpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_butterworth_q(order);
    q_values
        .into_iter()
        .map(|q| {
            (
                1.0,
                Biquad::new(BiquadFilterType::Lowpass, freq, srate, q, 0.0),
            )
        })
        .collect()
}

/// Create Butterworth highpass filter
///
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
///
/// # Returns
/// * PEQ containing the Butterworth highpass filter sections
pub fn peq_butterworth_highpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_butterworth_q(order);
    q_values
        .into_iter()
        .map(|q| {
            (
                1.0,
                Biquad::new(BiquadFilterType::Highpass, freq, srate, q, 0.0),
            )
        })
        .collect()
}

/// Compute Q values for Linkwitz-Riley filters
///
/// # Arguments
/// * `order` - Filter order
///
/// # Returns
/// * Vector of Q values for each biquad section
pub fn peq_linkwitzriley_q(order: usize) -> Vec<f64> {
    let q_bw = peq_butterworth_q(order / 2);
    let mut q_values = Vec::new();

    if !order.is_multiple_of(4) {
        // Odd number of pairs
        q_values.extend_from_slice(&q_bw[..q_bw.len() - 1]);
        q_values.extend_from_slice(&q_bw[..q_bw.len() - 1]);
        q_values.push(0.5);
    } else {
        // Even number of pairs
        q_values.extend_from_slice(&q_bw);
        q_values.extend_from_slice(&q_bw);
    }

    q_values
}

/// Create Linkwitz-Riley lowpass filter
///
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
///
/// # Returns
/// * PEQ containing the Linkwitz-Riley lowpass filter sections
pub fn peq_linkwitzriley_lowpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_linkwitzriley_q(order);
    q_values
        .into_iter()
        .map(|q| {
            (
                1.0,
                Biquad::new(BiquadFilterType::Lowpass, freq, srate, q, 0.0),
            )
        })
        .collect()
}

/// Create Linkwitz-Riley highpass filter
///
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
///
/// # Returns
/// * PEQ containing the Linkwitz-Riley highpass filter sections
pub fn peq_linkwitzriley_highpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_linkwitzriley_q(order);
    q_values
        .into_iter()
        .map(|q| {
            (
                1.0,
                Biquad::new(BiquadFilterType::Highpass, freq, srate, q, 0.0),
            )
        })
        .collect()
}

/// Print a formatted table of the parametric EQ filters from a Peq.
pub fn peq_print(peq: &Peq) {
    // Build filter rows from Peq
    let mut rows: Vec<FilterRow> = Vec::new();
    for (_weight, filter) in peq {
        rows.push(FilterRow {
            freq: filter.freq,
            q: filter.q,
            gain: filter.db_gain,
            kind: filter.filter_type.short_name(),
        });
    }

    // Sort by frequency
    rows.sort_by(|a, b| {
        a.freq
            .partial_cmp(&b.freq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("+-# -|-Freq (Hz)--|-Q ---------|-Gain (dB)--|-Type-----+");
    for (i, r) in rows.iter().enumerate() {
        println!(
            "| {:<2} | {:<10.2} | {:<10.3} | {:<+10.3} | {:<8} |",
            i + 1,
            r.freq,
            r.q,
            r.gain,
            r.kind
        );
    }
    println!("+----|------------|------------|------------|----------+");
}

#[cfg(test)]
mod peq_tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_peq_equal() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq3 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 4.0);

        let peq1 = vec![(1.0, bq1.clone()), (0.5, bq2.clone())];
        let peq2 = vec![(1.0, bq1), (0.5, bq2)];
        let peq3 = vec![(1.0, bq3)];

        assert!(peq_equal(&peq1, &peq2));
        assert!(!peq_equal(&peq1, &peq3));
        assert!(!peq_equal(&peq1, &vec![]));
    }

    #[test]
    fn test_peq_spl() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];
        let freq = array![100.0, 1000.0, 10000.0];

        let spl = peq_spl(&freq, &peq);

        // Should have gain close to 6 dB at 1kHz
        assert!(spl[1] > 5.0 && spl[1] < 7.0);
        // Should have less gain at other frequencies
        assert!(spl[0].abs() < 1.0);
        assert!(spl[2].abs() < 1.0);
    }

    #[test]
    fn test_peq_preamp_gain() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];

        let gain = peq_preamp_gain(&peq);

        // Should be negative to prevent clipping
        assert!(gain < 0.0);
        // Should be around -6 dB to compensate for the +6 dB boost
        assert!(gain > -7.0 && gain < -5.0);
    }

    #[test]
    fn test_peq_format_apo() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, bq)];

        let apo_str = peq_format_apo("Test EQ", &peq);

        assert!(apo_str.contains("Test EQ"));
        assert!(apo_str.contains("Preamp:"));
        assert!(apo_str.contains("Filter  1:"));
        assert!(apo_str.contains("PK"));
        assert!(apo_str.contains("1000 Hz"));
        assert!(apo_str.contains("+3.00 dB"));
    }

    #[test]
    fn test_butterworth_q() {
        let q_values = peq_butterworth_q(4);
        assert_eq!(q_values.len(), 2);

        // For 4th order Butterworth, should have specific Q values
        assert!((q_values[0] - 1.3065630).abs() < 1e-6);
        assert!((q_values[1] - 0.5411961).abs() < 1e-6);
    }

    #[test]
    fn test_butterworth_filters() {
        let lp = peq_butterworth_lowpass(4, 1000.0, 48000.0);
        let hp = peq_butterworth_highpass(4, 1000.0, 48000.0);

        assert_eq!(lp.len(), 2);
        assert_eq!(hp.len(), 2);

        // All filters should have weight 1.0 and correct type
        for (weight, bq) in &lp {
            assert_eq!(*weight, 1.0);
            assert_eq!(bq.filter_type, BiquadFilterType::Lowpass);
            assert_eq!(bq.freq, 1000.0);
        }

        for (weight, bq) in &hp {
            assert_eq!(*weight, 1.0);
            assert_eq!(bq.filter_type, BiquadFilterType::Highpass);
            assert_eq!(bq.freq, 1000.0);
        }
    }

    #[test]
    fn test_linkwitzriley_filters() {
        let lp = peq_linkwitzriley_lowpass(4, 1000.0, 48000.0);
        let hp = peq_linkwitzriley_highpass(4, 1000.0, 48000.0);

        // 4th order LR should have 2 sections (each with Q = 0.7071...)
        assert_eq!(lp.len(), 2);
        assert_eq!(hp.len(), 2);

        // All should be unit weight
        for (weight, _) in &lp {
            assert_eq!(*weight, 1.0);
        }
        for (weight, _) in &hp {
            assert_eq!(*weight, 1.0);
        }
    }
}

// ----------------------------------------------------------------------
// RME Format Functions
// ----------------------------------------------------------------------

/// Convert BiquadFilterType to RME format code
///
/// # Arguments
/// * `filter_type` - The biquad filter type
/// * `pos` - The position (1-based index) of the filter in the chain
///
/// # Returns
/// * RME type code as f64, or -1.0 if unsupported
///
/// # Notes
/// RME format codes depend on both filter type and position:
/// - PK (Peak): 0.0
/// - LP (Lowpass): 3.0 at pos 1, 2.0 at pos 3 or 9
/// - HP (Highpass): 2.0 at pos 1, 3.0 at pos 3 or 9
/// - LS/HS (Lowshelf/Highshelf): 1.0 at pos 1, 3, or 9
fn biquad_to_rme_type(filter_type: BiquadFilterType, pos: usize) -> f64 {
    match filter_type {
        BiquadFilterType::Peak => 0.0,
        BiquadFilterType::Lowpass => {
            if pos == 1 {
                3.0
            } else if pos == 3 || pos == 9 {
                2.0
            } else {
                -1.0
            }
        }
        BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => {
            if pos == 1 {
                2.0
            } else if pos == 3 || pos == 9 {
                3.0
            } else {
                -1.0
            }
        }
        BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => {
            if pos == 1 || pos == 3 || pos == 9 {
                1.0
            } else {
                -1.0
            }
        }
        _ => -1.0,
    }
}

/// Format PEQ as RME TotalMix channel preset XML
///
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
///
/// # Returns
/// * String formatted as RME TotalMix channel preset XML
///
/// # Notes
/// Generates XML in the format expected by RME TotalMix channel EQ (max 3 bands).
/// Includes LC Grade and LC Freq defaults, followed by Band parameters for
/// frequency, Q, and gain, then Band Type specifications.
pub fn peq_format_rme_channel(peq: &Peq) -> String {
    #[allow(clippy::vec_init_then_push)]
    let mut lines = vec![
        "<Preset>".to_string(),
        "  <Equalizer>".to_string(),
        "    <Params>".to_string(),
        "\t<val e=\"LC Grade\" v=\"1.00,\"/>".to_string(),
        "\t<val e=\"LC Freq\" v=\"20.00,\"/>".to_string(),
    ];

    // Add Band parameters (freq, Q, gain)
    for (i, (_, biquad)) in peq.iter().enumerate() {
        lines.push(format!(
            "      <val e=\"Band{} Freq\" v=\"{:7.2},\"/>",
            i + 1,
            biquad.freq
        ));
        lines.push(format!(
            "      <val e=\"Band{} Q\" v=\"{:4.2},\"/>",
            i + 1,
            biquad.q
        ));
        lines.push(format!(
            "        <val e=\"Band{} Gain\" v=\"{:4.2},\"/>",
            i + 1,
            biquad.db_gain
        ));
    }

    // Add Band types
    for (i, (_, biquad)) in peq.iter().enumerate() {
        let rme_type = biquad_to_rme_type(biquad.filter_type, i + 1);
        if rme_type >= 0.0 {
            lines.push(format!(
                "        <val e=\"Band{} Type\" v=\"{:4.2},\"/>",
                i + 1,
                rme_type
            ));
        }
    }

    lines.push("    </Params>".to_string());
    lines.push("  </Equalizer>".to_string());
    lines.push("</Preset>".to_string());

    lines.join("\n")
}

// ----------------------------------------------------------------------
// RME TotalMix Room EQ Format Functions
// ----------------------------------------------------------------------

/// Get priority for filter type (higher number = higher priority)
///
/// # Arguments
/// * `filter_type` - The type of filter
///
/// # Returns
/// * Priority value (higher = more important to keep)
///
/// # Notes
/// Priority levels:
/// - Lowshelf/Highshelf: 9 (important for overall curve)
/// - Lowpass/Highpass: 7 (medium priority)
/// - Bandpass: 5 (lower priority)
/// - Peak: 3 (lowest priority, most common)
/// - Default: 1
#[allow(dead_code)]
fn get_filter_priority(filter_type: BiquadFilterType) -> u8 {
    match filter_type {
        BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => 9,
        BiquadFilterType::Lowpass => 7,
        BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => 7,
        BiquadFilterType::Bandpass => 5,
        BiquadFilterType::Peak => 3,
        _ => 1,
    }
}

/// Filter PEQs while preserving order and prioritizing important filter types
///
/// # Arguments
/// * `peqs` - List of PEQ items in original order
/// * `max_count` - Maximum number of PEQ items to keep
///
/// # Returns
/// * List of PEQ items in original order, limited to max_count, with low-priority/low-gain filters removed
///
/// # Notes
/// If the input has fewer than or equal to max_count items, returns a clone.
/// Otherwise, sorts by priority (descending) then by absolute gain (descending),
/// takes the top max_count items, and returns them in their original order.
#[allow(dead_code)]
fn filter_peqs_by_gain(peqs: &Peq, max_count: usize) -> Peq {
    if peqs.len() <= max_count {
        return peqs.clone();
    }

    // Create list of (index, peq_item, priority, abs_gain) for sorting
    let mut indexed_peqs: Vec<(usize, &(f64, Biquad), u8, f64)> = peqs
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let priority = get_filter_priority(item.1.filter_type);
            let abs_gain = item.1.db_gain.abs();
            (i, item, priority, abs_gain)
        })
        .collect();

    // Sort by priority (descending) then by absolute gain (descending)
    // This puts high-priority, high-gain filters first
    indexed_peqs.sort_by(|a, b| {
        b.2.cmp(&a.2) // Compare priority (descending)
            .then_with(|| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal))
        // Then abs_gain (descending)
    });

    // Take the top max_count filters
    let mut selected: Vec<(usize, (f64, Biquad))> = indexed_peqs
        .into_iter()
        .take(max_count)
        .map(|(idx, item, _, _)| (idx, item.clone()))
        .collect();

    // Sort selected filters back to original order by index
    selected.sort_by_key(|(idx, _)| *idx);

    // Extract just the PEQ items
    selected.into_iter().map(|(_, item)| item).collect()
}

/// Enforce RME room EQ constraints
///
/// # Arguments
/// * `peqs` - List of PEQ items
///
/// # Returns
/// * List of exactly 9 PEQ items with RME room EQ constraints applied:
///   - Position 1 can be: PK, LS, HS, LP, or HP (lowest freq non-PK preferred)
///   - Positions 2-8 can only be PK
///   - Position 9 can be: PK, LS, HS, LP, or HP (highest freq non-PK preferred)
///   - Missing slots filled with zero-gain PK filters at 1kHz
///
/// # Notes
/// RME room EQ hardware constraints:
/// - Total of 9 bands maximum
/// - Only positions 1 and 9 support non-PK filter types
/// - If more than 2 non-PK filters exist, picks lowest and highest frequency
fn enforce_rme_room_filter_constraints(peqs: &Peq) -> Peq {
    // Separate filters by category
    let mut pk_filters: Vec<(f64, Biquad)> = Vec::new();
    let mut non_pk_filters: Vec<(f64, Biquad)> = Vec::new();

    for item in peqs {
        match item.1.filter_type {
            BiquadFilterType::Peak => pk_filters.push(item.clone()),
            BiquadFilterType::Lowshelf
            | BiquadFilterType::Highshelf
            | BiquadFilterType::Lowpass
            | BiquadFilterType::Highpass
            | BiquadFilterType::HighpassVariableQ => non_pk_filters.push(item.clone()),
            _ => {
                // Convert unsupported types to PK
                eprintln!(
                    "Warning: Filter type {:?} not supported by RME room EQ, converting to PK",
                    item.1.filter_type
                );
                let mut converted = item.1.clone();
                converted.filter_type = BiquadFilterType::Peak;
                pk_filters.push((item.0, converted));
            }
        }
    }

    // Select non-PK filters for positions 1 and 9
    let mut selected_low: Option<(f64, Biquad)> = None;
    let mut selected_high: Option<(f64, Biquad)> = None;

    if non_pk_filters.len() > 2 {
        eprintln!(
            "Warning: RME room EQ supports at most 2 non-PK filters (positions 1 and 9). \
             Found {} non-PK filters. Selecting lowest and highest frequency filters.",
            non_pk_filters.len()
        );
    }

    if !non_pk_filters.is_empty() {
        // Sort non-PK filters by frequency
        let mut sorted_non_pk = non_pk_filters.clone();
        sorted_non_pk.sort_by(|a, b| {
            a.1.freq
                .partial_cmp(&b.1.freq)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Select lowest frequency for position 1
        selected_low = Some(sorted_non_pk[0].clone());

        // If we have more than one non-PK, select highest frequency for position 9
        if sorted_non_pk.len() > 1 {
            selected_high = Some(sorted_non_pk[sorted_non_pk.len() - 1].clone());
        }
    }

    // Build the result with exactly 9 bands
    let mut result: Vec<(f64, Biquad)> = Vec::new();

    // Position 1: non-PK (if available) or first PK
    if let Some(low) = selected_low {
        result.push(low);
    } else if !pk_filters.is_empty() {
        result.push(pk_filters.remove(0));
    } else {
        // Create dummy zero-gain PK filter
        result.push((
            1.0,
            Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0),
        ));
    }

    // Positions 2-8: PK filters only (7 slots)
    let mut middle_count = 0;
    while middle_count < 7 {
        if !pk_filters.is_empty() {
            result.push(pk_filters.remove(0));
        } else {
            // Fill with zero-gain PK filters
            result.push((
                1.0,
                Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0),
            ));
        }
        middle_count += 1;
    }

    // Position 9: non-PK (if available) or PK
    if let Some(high) = selected_high {
        result.push(high);
    } else if !pk_filters.is_empty() {
        result.push(pk_filters.remove(0));
    } else {
        // Create dummy zero-gain PK filter
        result.push((
            1.0,
            Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0),
        ));
    }

    // Warn if we had to drop PK filters
    if !pk_filters.is_empty() {
        eprintln!(
            "Warning: {} PK filters were dropped due to 9-band limit",
            pk_filters.len()
        );
    }

    result
}

/// Format PEQ as RME TotalMix room EQ preset XML (dual channel)
///
/// # Arguments
/// * `left` - PEQ vector for left channel
/// * `right` - PEQ vector for right channel (if empty, left channel is used for both)
///
/// # Returns
/// * String formatted as RME TotalMix room EQ preset XML
///
/// # Notes
/// Generates XML in the format expected by RME TotalMix room EQ.
/// Always outputs exactly 9 bands per channel with RME constraints:
/// - Position 1 can be: PK, LS, HS, LP, or HP (lowest freq non-PK preferred)
/// - Positions 2-8 can only be PK
/// - Position 9 can be: PK, LS, HS, LP, or HP (highest freq non-PK preferred)
pub fn peq_format_rme_room(left: &Peq, right: &Peq) -> String {
    // Apply RME room EQ constraints - returns exactly 9 bands
    let left_constrained = enforce_rme_room_filter_constraints(left);
    let right_constrained = if !right.is_empty() {
        enforce_rme_room_filter_constraints(right)
    } else {
        left_constrained.clone()
    };

    let mut lines = Vec::new();

    // Helper closure to process a channel's PEQ items
    let process_channel = |peqs: &Peq, lines: &mut Vec<String>| {
        // Add frequency, Q, and gain parameters
        for (i, (_, biquad)) in peqs.iter().enumerate() {
            lines.push(format!(
                "        <val e=\"REQ Band{} Freq\" v=\"{:7.2},\"/>",
                i + 1,
                biquad.freq
            ));
            lines.push(format!(
                "        <val e=\"REQ Band{} Q\" v=\"{:4.2},\"/>",
                i + 1,
                biquad.q
            ));
            lines.push(format!(
                "        <val e=\"REQ Band{} Gain\" v=\"{:4.2},\"/>",
                i + 1,
                biquad.db_gain
            ));
        }

        // Add type parameters
        for (i, (_, biquad)) in peqs.iter().enumerate() {
            let rme_type = biquad_to_rme_type(biquad.filter_type, i + 1);
            if rme_type >= 0.0 {
                lines.push(format!(
                    "        <val e=\"REQ Band{} Type\" v=\"{:4.2},\"/>",
                    i + 1,
                    rme_type
                ));
            }
        }
    };

    lines.push("<Preset>".to_string());

    // Process left channel
    let preamp_gain = 0.0;
    lines.push("  <Room EQ L>".to_string());
    lines.push("    <Params>".to_string());
    lines.push("\t<val e=\"REQ Delay\" v=\"0.00,\"/>".to_string());
    process_channel(&left_constrained, &mut lines);
    lines.push(format!(
        "\t<val e=\"REQ Chan Gain\" v=\"{},\"/>",
        preamp_gain
    ));
    lines.push("    </Params>".to_string());
    lines.push("  </Room EQ L>".to_string());

    // Process right channel (use left if right is empty)
    lines.push("  <Room EQ R>".to_string());
    lines.push("    <Params>".to_string());
    lines.push("\t<val e=\"REQ Delay\" v=\"0.00,\"/>".to_string());
    if !right_constrained.is_empty() {
        process_channel(&right_constrained, &mut lines);
    } else {
        process_channel(&left_constrained, &mut lines);
    }
    lines.push(format!(
        "\t<val e=\"REQ Chan Gain\" v=\"{},\"/>",
        preamp_gain
    ));
    lines.push("    </Params>".to_string());
    lines.push("  </Room EQ R>".to_string());

    lines.push("</Preset>".to_string());

    lines.join("\n")
}

// ----------------------------------------------------------------------
// Apple AUNBandEQ (aupreset) Format Functions
// ----------------------------------------------------------------------

// Apple AUNBandEQ parameter constants
const K_AUNBANDEQ_PARAM_BYPASS_BAND: i32 = 1000;
const K_AUNBANDEQ_PARAM_FILTER_TYPE: i32 = 2000;
const K_AUNBANDEQ_PARAM_FREQUENCY: i32 = 3000;
const K_AUNBANDEQ_PARAM_GAIN: i32 = 4000;
const K_AUNBANDEQ_PARAM_BANDWIDTH: i32 = 5000;

// Apple AUNBandEQ filter type constants
const K_AUNBANDEQ_FILTER_TYPE_PARAMETRIC: i32 = 0;
#[allow(dead_code)]
const K_AUNBANDEQ_FILTER_TYPE_2ND_ORDER_BUTTERWORTH_LOW_PASS: i32 = 1;
#[allow(dead_code)]
const K_AUNBANDEQ_FILTER_TYPE_2ND_ORDER_BUTTERWORTH_HIGH_PASS: i32 = 2;
const K_AUNBANDEQ_FILTER_TYPE_RESONANT_LOW_PASS: i32 = 3;
const K_AUNBANDEQ_FILTER_TYPE_RESONANT_HIGH_PASS: i32 = 4;
const K_AUNBANDEQ_FILTER_TYPE_BAND_PASS: i32 = 5;
const K_AUNBANDEQ_FILTER_TYPE_LOW_SHELF: i32 = 7;
const K_AUNBANDEQ_FILTER_TYPE_HIGH_SHELF: i32 = 8;

/// Convert BiquadFilterType to Apple AUNBandEQ filter type constant
///
/// # Arguments
/// * `filter_type` - The biquad filter type
///
/// # Returns
/// * Apple AUNBandEQ filter type constant, or -1 if unsupported
fn biquad_to_apple_type(filter_type: BiquadFilterType) -> i32 {
    match filter_type {
        BiquadFilterType::Peak => K_AUNBANDEQ_FILTER_TYPE_PARAMETRIC,
        BiquadFilterType::Highshelf => K_AUNBANDEQ_FILTER_TYPE_HIGH_SHELF,
        BiquadFilterType::Lowshelf => K_AUNBANDEQ_FILTER_TYPE_LOW_SHELF,
        BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => {
            K_AUNBANDEQ_FILTER_TYPE_RESONANT_HIGH_PASS
        }
        BiquadFilterType::Lowpass => K_AUNBANDEQ_FILTER_TYPE_RESONANT_LOW_PASS,
        BiquadFilterType::Bandpass => K_AUNBANDEQ_FILTER_TYPE_BAND_PASS,
        _ => -1,
    }
}

/// Format PEQ as Apple AUNBandEQ preset (aupreset) plist XML
///
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
/// * `name` - Name for the preset
///
/// # Returns
/// * String formatted as Apple AUNBandEQ preset plist XML
///
/// # Notes
/// Generates a plist XML file containing base64-encoded binary data
/// in the format expected by Apple's AUNBandEQ audio unit.
/// Supports up to 16 bands with parameters for bypass, type, frequency,
/// gain, and bandwidth.
pub fn peq_format_aupreset(peq: &Peq, name: &str) -> String {
    let len_peq = peq.len().min(16); // Max 16 bands for Apple
    let preamp_gain = peq_preamp_gain(peq);

    // Build binary data structure
    let mut buffer = Vec::new();

    // Header: 5 values (4 integers + 1 float)
    // Structure: [0, 0, ndata (81), 0, preamp_gain]
    buffer.write_i32::<BigEndian>(0).unwrap();
    buffer.write_i32::<BigEndian>(0).unwrap();
    buffer.write_i32::<BigEndian>(81).unwrap(); // ndata is always 81
    buffer.write_i32::<BigEndian>(0).unwrap();
    buffer.write_f32::<BigEndian>(preamp_gain as f32).unwrap();

    // Create parameter map
    let mut params = std::collections::BTreeMap::new();

    // Add parameters for each band
    for (i, (_, biquad)) in peq.iter().take(16).enumerate() {
        let idx = i as i32;
        params.insert(K_AUNBANDEQ_PARAM_BYPASS_BAND + idx, 0.0f32); // 0.0 = enabled
        params.insert(
            K_AUNBANDEQ_PARAM_FILTER_TYPE + idx,
            biquad_to_apple_type(biquad.filter_type) as f32,
        );
        params.insert(K_AUNBANDEQ_PARAM_FREQUENCY + idx, biquad.freq as f32);
        params.insert(K_AUNBANDEQ_PARAM_GAIN + idx, biquad.db_gain as f32);
        params.insert(K_AUNBANDEQ_PARAM_BANDWIDTH + idx, q2bw(biquad.q) as f32);
    }

    // Fill remaining bands (up to 16) with disabled/zero values
    for i in len_peq..16 {
        let idx = i as i32;
        params.insert(K_AUNBANDEQ_PARAM_BYPASS_BAND + idx, 1.0f32); // 1.0 = disabled
        params.insert(K_AUNBANDEQ_PARAM_FILTER_TYPE + idx, 0.0f32);
        params.insert(K_AUNBANDEQ_PARAM_FREQUENCY + idx, 0.0f32);
        params.insert(K_AUNBANDEQ_PARAM_GAIN + idx, 0.0f32);
        params.insert(K_AUNBANDEQ_PARAM_BANDWIDTH + idx, 0.0f32);
    }

    // Write parameters in sorted order (param_id, value) pairs
    for (param_id, value) in params.iter() {
        buffer.write_i32::<BigEndian>(*param_id).unwrap();
        buffer.write_f32::<BigEndian>(*value).unwrap();
    }

    // Base64 encode the buffer
    let b64_text = general_purpose::STANDARD.encode(&buffer);

    // Format as chunks of 68 characters with tabs
    let chunk_size = 68;
    let mut data_lines = Vec::new();
    for chunk in b64_text.as_bytes().chunks(chunk_size) {
        data_lines.push(format!("\t{}", String::from_utf8_lossy(chunk)));
    }
    let data_section = data_lines.join("\n");

    // Build the plist XML
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>ParametricType</key>
	<integer>11</integer>
	<key>data</key>
	<data>
{}
	</data>
	<key>manufacturer</key>
	<integer>1634758764</integer>
	<key>name</key>
	<string>{}</string>
	<key>numberOfBands</key>
	<integer>{}</integer>
	<key>subtype</key>
	<integer>1851942257</integer>
	<key>type</key>
	<integer>1635083896</integer>
	<key>version</key>
	<integer>0</integer>
</dict>
</plist>
"#,
        data_section, name, len_peq
    )
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn test_peq_format_rme_channel_single_peak() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, bq)];
        let rme_str = peq_format_rme_channel(&peq);

        // Verify structure
        assert!(rme_str.contains("<Preset>"));
        assert!(rme_str.contains("<Equalizer>"));
        assert!(rme_str.contains("<Params>"));
        assert!(rme_str.contains("LC Grade"));
        assert!(rme_str.contains("LC Freq"));
        assert!(rme_str.contains("Band1 Freq"));
        assert!(rme_str.contains("Band1 Q"));
        assert!(rme_str.contains("Band1 Gain"));
        assert!(rme_str.contains("Band1 Type"));
        assert!(rme_str.contains("</Preset>"));

        // Peak filter should have type 0.0
        assert!(rme_str.contains("0.00"));
    }

    #[test]
    fn test_peq_format_rme_channel_empty() {
        let peq: Peq = vec![];
        let rme_str = peq_format_rme_channel(&peq);

        // Should still have basic structure
        assert!(rme_str.contains("<Preset>"));
        assert!(rme_str.contains("<Equalizer>"));
        assert!(rme_str.contains("LC Grade"));
        assert!(rme_str.contains("</Preset>"));
    }

    #[test]
    fn test_peq_format_rme_channel_multiple_bands() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, -2.0);
        let peq = vec![(1.0, bq1), (1.0, bq2)];
        let rme_str = peq_format_rme_channel(&peq);

        assert!(rme_str.contains("Band1 Freq"));
        assert!(rme_str.contains("Band2 Freq"));
        assert!(rme_str.contains("Band1 Type"));
        assert!(rme_str.contains("Band2 Type"));
    }

    #[test]
    fn test_peq_format_aupreset_single_peak() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, bq)];
        let aupreset_str = peq_format_aupreset(&peq, "Test EQ");

        // Verify plist structure
        assert!(aupreset_str.contains("<?xml version="));
        assert!(aupreset_str.contains("<!DOCTYPE plist"));
        assert!(aupreset_str.contains("<plist version=\"1.0\">"));
        assert!(aupreset_str.contains("<dict>"));
        assert!(aupreset_str.contains("<key>ParametricType</key>"));
        assert!(aupreset_str.contains("<key>data</key>"));
        assert!(aupreset_str.contains("<data>"));
        assert!(aupreset_str.contains("<key>name</key>"));
        assert!(aupreset_str.contains("<string>Test EQ</string>"));
        assert!(aupreset_str.contains("<key>numberOfBands</key>"));
        assert!(aupreset_str.contains("<integer>1</integer>"));
        assert!(aupreset_str.contains("</plist>"));
    }

    #[test]
    fn test_peq_format_aupreset_empty() {
        let peq: Peq = vec![];
        let aupreset_str = peq_format_aupreset(&peq, "Empty EQ");

        // Should still generate valid plist
        assert!(aupreset_str.contains("<?xml version="));
        assert!(aupreset_str.contains("<string>Empty EQ</string>"));
        assert!(aupreset_str.contains("<integer>0</integer>"));
    }

    #[test]
    fn test_peq_format_aupreset_multiple_bands() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 2.0);
        let bq3 = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, -1.0);
        let peq = vec![(1.0, bq1), (1.0, bq2), (1.0, bq3)];
        let aupreset_str = peq_format_aupreset(&peq, "Multi Band EQ");

        assert!(aupreset_str.contains("<string>Multi Band EQ</string>"));
        assert!(aupreset_str.contains("<integer>3</integer>"));
        // Should have base64 encoded data
        assert!(aupreset_str.contains("<data>"));
    }

    #[test]
    fn test_peq_format_aupreset_max_bands() {
        // Test with more than 16 bands (should cap at 16)
        let mut peq = Vec::new();
        for i in 0..20 {
            let freq = 100.0 + (i as f64 * 100.0);
            let bq = Biquad::new(BiquadFilterType::Peak, freq, 48000.0, 1.0, 1.0);
            peq.push((1.0, bq));
        }
        let aupreset_str = peq_format_aupreset(&peq, "Max Bands EQ");

        // Should cap at 16 bands
        assert!(aupreset_str.contains("<integer>16</integer>"));
    }

    #[test]
    fn test_biquad_to_apple_type() {
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Peak),
            K_AUNBANDEQ_FILTER_TYPE_PARAMETRIC
        );
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Highshelf),
            K_AUNBANDEQ_FILTER_TYPE_HIGH_SHELF
        );
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Lowshelf),
            K_AUNBANDEQ_FILTER_TYPE_LOW_SHELF
        );
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Highpass),
            K_AUNBANDEQ_FILTER_TYPE_RESONANT_HIGH_PASS
        );
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Lowpass),
            K_AUNBANDEQ_FILTER_TYPE_RESONANT_LOW_PASS
        );
        assert_eq!(
            biquad_to_apple_type(BiquadFilterType::Bandpass),
            K_AUNBANDEQ_FILTER_TYPE_BAND_PASS
        );
    }

    #[test]
    fn test_biquad_to_rme_type() {
        // Peak should always be 0.0
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Peak, 1), 0.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Peak, 2), 0.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Peak, 3), 0.0);

        // Lowpass position-dependent
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowpass, 1), 3.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowpass, 3), 2.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowpass, 9), 2.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowpass, 2), -1.0);

        // Highpass position-dependent
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highpass, 1), 2.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highpass, 3), 3.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highpass, 9), 3.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highpass, 2), -1.0);

        // Lowshelf position-dependent
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowshelf, 1), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowshelf, 3), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowshelf, 9), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Lowshelf, 2), -1.0);

        // Highshelf position-dependent
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highshelf, 1), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highshelf, 3), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highshelf, 9), 1.0);
        assert_eq!(biquad_to_rme_type(BiquadFilterType::Highshelf, 2), -1.0);
    }

    #[test]
    fn test_get_filter_priority() {
        assert_eq!(get_filter_priority(BiquadFilterType::Lowshelf), 9);
        assert_eq!(get_filter_priority(BiquadFilterType::Highshelf), 9);
        assert_eq!(get_filter_priority(BiquadFilterType::Lowpass), 7);
        assert_eq!(get_filter_priority(BiquadFilterType::Highpass), 7);
        assert_eq!(get_filter_priority(BiquadFilterType::HighpassVariableQ), 7);
        assert_eq!(get_filter_priority(BiquadFilterType::Bandpass), 5);
        assert_eq!(get_filter_priority(BiquadFilterType::Peak), 3);
        assert_eq!(get_filter_priority(BiquadFilterType::Notch), 1);
    }

    #[test]
    fn test_filter_peqs_by_gain_under_limit() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 1.0, 2.0);
        let peq = vec![(1.0, bq1), (1.0, bq2)];
        let filtered = filter_peqs_by_gain(&peq, 5);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_peqs_by_gain_over_limit() {
        // Create 12 peak filters with varying gains
        let mut peq = Vec::new();
        for i in 0..12 {
            let gain = (i as f64) * 0.5 + 0.5; // gains from 0.5 to 6.0
            let bq = Biquad::new(
                BiquadFilterType::Peak,
                1000.0 + (i as f64 * 100.0),
                48000.0,
                1.0,
                gain,
            );
            peq.push((1.0, bq));
        }
        let filtered = filter_peqs_by_gain(&peq, 9);
        assert_eq!(filtered.len(), 9);
        // Should keep the ones with higher gains (indices 3-11)
        // First frequency should be from index 3 (1300 Hz)
        assert!((filtered[0].1.freq - 1300.0).abs() < 1.0);
    }

    #[test]
    fn test_filter_peqs_by_gain_priority() {
        // Mix of different filter types with varying gains
        let mut peq = Vec::new();
        // Add 6 peaks with high gains
        for i in 0..6 {
            let bq = Biquad::new(
                BiquadFilterType::Peak,
                1000.0 + (i as f64 * 100.0),
                48000.0,
                1.0,
                5.0,
            );
            peq.push((1.0, bq));
        }
        // Add 2 lowshelf with lower gain but higher priority
        let ls1 = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, 2.0);
        let ls2 = Biquad::new(BiquadFilterType::Lowshelf, 120.0, 48000.0, 0.7, 2.5);
        peq.push((1.0, ls1));
        peq.push((1.0, ls2));
        // Add 3 more peaks
        for i in 6..9 {
            let bq = Biquad::new(
                BiquadFilterType::Peak,
                1000.0 + (i as f64 * 100.0),
                48000.0,
                1.0,
                4.0,
            );
            peq.push((1.0, bq));
        }
        let filtered = filter_peqs_by_gain(&peq, 9);
        assert_eq!(filtered.len(), 9);
        // Both lowshelf filters should be kept due to higher priority
        let lowshelf_count = filtered
            .iter()
            .filter(|(_, bq)| bq.filter_type == BiquadFilterType::Lowshelf)
            .count();
        assert_eq!(lowshelf_count, 2);
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_empty() {
        let peq: Peq = vec![];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands (filled with zero-gain PK)
        assert_eq!(result.len(), 9);
        // All should be zero-gain PK filters
        for (_, bq) in &result {
            assert_eq!(bq.filter_type, BiquadFilterType::Peak);
            assert_eq!(bq.db_gain, 0.0);
        }
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_no_shelves() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 1.0, 2.0);
        let peq = vec![(1.0, bq1), (1.0, bq2)];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands
        assert_eq!(result.len(), 9);
        // First 2 should be the input PK filters
        assert!((result[0].1.freq - 1000.0).abs() < 1.0);
        assert!((result[1].1.freq - 2000.0).abs() < 1.0);
        // Rest should be zero-gain PK filters
        for i in 2..9 {
            assert_eq!(result[i].1.filter_type, BiquadFilterType::Peak);
            assert_eq!(result[i].1.db_gain, 0.0);
        }
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_single_lowshelf() {
        let ls = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, 2.0);
        let pk1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let pk2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 1.0, 2.0);
        let peq = vec![(1.0, pk1), (1.0, ls.clone()), (1.0, pk2)];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands
        assert_eq!(result.len(), 9);
        // Lowshelf should be in position 1 (lowest freq non-PK)
        assert_eq!(result[0].1.filter_type, BiquadFilterType::Lowshelf);
        assert!((result[0].1.freq - 100.0).abs() < 1.0);
        // PK filters in positions 2-9
        for i in 1..9 {
            assert_eq!(result[i].1.filter_type, BiquadFilterType::Peak);
        }
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_multiple_lowshelf() {
        let ls1 = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, 2.0);
        let ls2 = Biquad::new(BiquadFilterType::Lowshelf, 120.0, 48000.0, 0.7, 4.0);
        let pk = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, ls1), (1.0, pk), (1.0, ls2)];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands
        assert_eq!(result.len(), 9);
        // Position 1: lowest freq lowshelf (ls1 at 100Hz)
        assert_eq!(result[0].1.filter_type, BiquadFilterType::Lowshelf);
        assert!((result[0].1.freq - 100.0).abs() < 1.0);
        // Position 9: highest freq lowshelf (ls2 at 120Hz)
        assert_eq!(result[8].1.filter_type, BiquadFilterType::Lowshelf);
        assert!((result[8].1.freq - 120.0).abs() < 1.0);
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_multiple_highshelf() {
        let hs1 = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 1.5);
        let hs2 = Biquad::new(BiquadFilterType::Highshelf, 10000.0, 48000.0, 0.7, 3.0);
        let pk = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, pk), (1.0, hs1), (1.0, hs2)];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands
        assert_eq!(result.len(), 9);
        // Position 1: lowest freq highshelf (hs1 at 8000Hz)
        assert_eq!(result[0].1.filter_type, BiquadFilterType::Highshelf);
        assert!((result[0].1.freq - 8000.0).abs() < 1.0);
        // Position 9: highest freq highshelf (hs2 at 10000Hz)
        assert_eq!(result[8].1.filter_type, BiquadFilterType::Highshelf);
        assert!((result[8].1.freq - 10000.0).abs() < 1.0);
    }

    #[test]
    fn test_enforce_rme_room_filter_constraints_both_shelves() {
        let ls = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, 2.0);
        let hs = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 1.5);
        let pk1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let pk2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 1.0, 2.0);
        let peq = vec![(1.0, pk1), (1.0, ls), (1.0, pk2), (1.0, hs)];
        let result = enforce_rme_room_filter_constraints(&peq);
        // Should always return exactly 9 bands
        assert_eq!(result.len(), 9);
        // Position 1: Lowshelf (lowest freq non-PK at 100Hz)
        assert_eq!(result[0].1.filter_type, BiquadFilterType::Lowshelf);
        assert!((result[0].1.freq - 100.0).abs() < 1.0);
        // Position 9: Highshelf (highest freq non-PK at 8000Hz)
        assert_eq!(result[8].1.filter_type, BiquadFilterType::Highshelf);
        assert!((result[8].1.freq - 8000.0).abs() < 1.0);
        // Positions 2-8 should be PK filters
        for i in 1..8 {
            assert_eq!(result[i].1.filter_type, BiquadFilterType::Peak);
        }
    }

    #[test]
    fn test_peq_format_rme_room_single_channel() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, -2.0);
        let left = vec![(1.0, bq1), (1.0, bq2)];
        let right: Peq = vec![];
        let rme_str = peq_format_rme_room(&left, &right);

        // Verify structure
        assert!(rme_str.contains("<Preset>"));
        assert!(rme_str.contains("<Room EQ L>"));
        assert!(rme_str.contains("<Room EQ R>"));
        assert!(rme_str.contains("<Params>"));
        assert!(rme_str.contains("REQ Delay"));
        assert!(rme_str.contains("REQ Band1 Freq"));
        assert!(rme_str.contains("REQ Band1 Q"));
        assert!(rme_str.contains("REQ Band1 Gain"));
        assert!(rme_str.contains("REQ Band1 Type"));
        assert!(rme_str.contains("REQ Band2 Freq"));
        assert!(rme_str.contains("REQ Chan Gain"));
        assert!(rme_str.contains("</Preset>"));
    }

    #[test]
    fn test_peq_format_rme_room_dual_channel() {
        let bq_left = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq_right = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 2.0, -2.0);
        let left = vec![(1.0, bq_left)];
        let right = vec![(1.0, bq_right)];
        let rme_str = peq_format_rme_room(&left, &right);

        // Both channels should have different frequencies
        assert!(rme_str.contains("1000.00"));
        assert!(rme_str.contains("2000.00"));
    }

    #[test]
    fn test_peq_format_rme_room_max_bands() {
        // Test with 9 bands (at limit)
        let mut left = Vec::new();
        for i in 0..9 {
            let freq = 100.0 + (i as f64 * 100.0);
            let bq = Biquad::new(BiquadFilterType::Peak, freq, 48000.0, 1.0, 1.0);
            left.push((1.0, bq));
        }
        let right: Peq = vec![];
        let rme_str = peq_format_rme_room(&left, &right);

        // Should contain all 9 bands
        assert!(rme_str.contains("REQ Band1 Freq"));
        assert!(rme_str.contains("REQ Band9 Freq"));
    }

    #[test]
    fn test_peq_format_rme_room_over_limit() {
        // Test with more than 9 bands (should be filtered)
        let mut left = Vec::new();
        for i in 0..12 {
            let freq = 100.0 + (i as f64 * 100.0);
            let gain = (i as f64) * 0.5 + 0.5;
            let bq = Biquad::new(BiquadFilterType::Peak, freq, 48000.0, 1.0, gain);
            left.push((1.0, bq));
        }
        let right: Peq = vec![];
        let rme_str = peq_format_rme_room(&left, &right);

        // Should only contain 9 bands
        assert!(rme_str.contains("REQ Band9 Freq"));
        assert!(!rme_str.contains("REQ Band10 Freq"));
    }
}
