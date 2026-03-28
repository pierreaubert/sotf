//! Biquad IIR filter types and implementations.

use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;
use std::fmt;

use crate::error::IirError;
use crate::{DEFAULT_Q_HIGH_LOW_PASS, DEFAULT_Q_HIGH_LOW_SHELF};

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
    /// All-pass filter
    AllPass,
    /// Low-shelf filter (Orfanidis design with prescribed Nyquist gain)
    LowshelfOrf,
    /// High-shelf filter (Orfanidis design with prescribed Nyquist gain)
    HighshelfOrf,
    /// Peaking filter (Vicanek matched analog response)
    PeakMatched,
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
            BiquadFilterType::AllPass => "AP",
            BiquadFilterType::LowshelfOrf => "LSO",
            BiquadFilterType::HighshelfOrf => "HSO",
            BiquadFilterType::PeakMatched => "PKM",
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
            BiquadFilterType::AllPass => "AllPass",
            BiquadFilterType::LowshelfOrf => "LowshelfOrf",
            BiquadFilterType::HighshelfOrf => "HighshelfOrf",
            BiquadFilterType::PeakMatched => "PeakMatched",
        }
    }
}

/// Biquad filter coefficients for external interpolation.
#[derive(Debug, Clone, Copy)]
pub struct BiquadCoefficients {
    /// Feedforward coefficient b0
    pub b0: f64,
    /// Feedforward coefficient b1
    pub b1: f64,
    /// Feedforward coefficient b2
    pub b2: f64,
    /// Feedback coefficient a1 (normalized, a0=1)
    pub a1: f64,
    /// Feedback coefficient a2 (normalized, a0=1)
    pub a2: f64,
}

impl BiquadCoefficients {
    /// Linearly interpolate between two sets of coefficients.
    ///
    /// `t` ranges from 0.0 (fully `self`) to 1.0 (fully `other`).
    #[inline(always)]
    pub fn lerp(&self, other: &BiquadCoefficients, t: f64) -> BiquadCoefficients {
        BiquadCoefficients {
            b0: self.b0 + (other.b0 - self.b0) * t,
            b1: self.b1 + (other.b1 - self.b1) * t,
            b2: self.b2 + (other.b2 - self.b2) * t,
            a1: self.a1 + (other.a1 - self.a1) * t,
            a2: self.a2 + (other.a2 - self.a2) * t,
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
    /// Filter state for DF-I (for processing samples)
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    /// Filter state for Transposed Direct Form II
    s1: f64,
    s2: f64,
    /// When true, use Transposed Direct Form II instead of Direct Form I.
    /// TDF-II has better numerical properties for high-Q narrow filters.
    #[serde(default)]
    pub use_tdf2: bool,
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
            s1: 0.0,
            s2: 0.0,
            use_tdf2: false,
            r_up0: 0.0,
            r_up1: 0.0,
            r_up2: 0.0,
            r_dw0: 0.0,
            r_dw1: 0.0,
            r_dw2: 0.0,
        };

        // Adjust Q based on filter type, matching Python logic
        if biquad.q == 0.0 {
            match biquad.filter_type {
                BiquadFilterType::Notch => {
                    biquad.q = 30.0;
                }
                BiquadFilterType::Bandpass
                | BiquadFilterType::Highpass
                | BiquadFilterType::Lowpass => {
                    biquad.q = DEFAULT_Q_HIGH_LOW_PASS;
                }
                BiquadFilterType::Lowshelf
                | BiquadFilterType::Highshelf
                | BiquadFilterType::LowshelfOrf
                | BiquadFilterType::HighshelfOrf => {
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
    /// use math_audio_iir_fir::{Biquad, BiquadFilterType, SRATE};
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

    /// Update filter parameters and recompute coefficients **without** resetting
    /// the internal delay state (x1, x2, y1, y2). This allows click-free
    /// parameter changes on a running filter.
    pub fn update_params(
        &mut self,
        filter_type: BiquadFilterType,
        freq: f64,
        srate: f64,
        q: f64,
        db_gain: f64,
    ) {
        self.filter_type = filter_type;
        self.freq = freq;
        self.srate = srate;
        self.q = if q == 0.0 {
            match filter_type {
                BiquadFilterType::Notch => 30.0,
                BiquadFilterType::Bandpass
                | BiquadFilterType::Highpass
                | BiquadFilterType::Lowpass => DEFAULT_Q_HIGH_LOW_PASS,
                BiquadFilterType::Lowshelf
                | BiquadFilterType::Highshelf
                | BiquadFilterType::LowshelfOrf
                | BiquadFilterType::HighshelfOrf => DEFAULT_Q_HIGH_LOW_SHELF,
                _ => q,
            }
        } else {
            q
        };
        if self.q <= 0.0 {
            self.q = 1.0e-2;
        }
        self.db_gain = db_gain;
        self.compute_coeffs();
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
            BiquadFilterType::AllPass => {
                b0 = 1.0 - alpha;
                b1 = -2.0 * cs;
                b2 = 1.0 + alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::LowshelfOrf => {
                let g = 10.0_f64.powf(self.db_gain / 20.0);
                let a_val = 10.0_f64.powf(self.db_gain / 40.0);
                let beta_rbj = (a_val + a_val).sqrt();
                a0 = (a_val + 1.0) + (a_val - 1.0) * cs + beta_rbj * sn;
                a1 = -2.0 * ((a_val - 1.0) + (a_val + 1.0) * cs);
                a2 = (a_val + 1.0) + (a_val - 1.0) * cs - beta_rbj * sn;

                let sum_a = a0 + a1 + a2;
                let diff_a = a0 - a1 + a2;
                let sum_b = g * sum_a;
                let diff_b = diff_a;

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0;

                let w0 = omega;
                let cos_2w0 = (2.0 * w0).cos();
                let sin_2w0 = (2.0 * w0).sin();

                let a_re = a0 + a1 * cs + a2 * cos_2w0;
                let a_im = -a1 * sn - a2 * sin_2w0;
                let den_w0_sq = a_re * a_re + a_im * a_im;
                let target = g * den_w0_sq;

                let c1 = 2.0 * b1 * p * cs;
                let known = p * p / 2.0 + b1 * b1 + c1 + p * p / 2.0 * cos_2w0;
                let d_coeff = 0.5 - 0.5 * cos_2w0;

                let d_sq = if d_coeff.abs() > 1e-15 {
                    (target - known) / d_coeff
                } else {
                    0.0
                };
                let d_val = if d_sq >= 0.0 { d_sq.sqrt() } else { 0.0 };
                let d_signed = if g >= 1.0 { d_val } else { -d_val };

                b0 = (p + d_signed) / 2.0;
                b2 = (p - d_signed) / 2.0;
            }
            BiquadFilterType::HighshelfOrf => {
                let g = 10.0_f64.powf(self.db_gain / 20.0);
                let a_val = 10.0_f64.powf(self.db_gain / 40.0);
                let beta_rbj = (a_val + a_val).sqrt();
                a0 = (a_val + 1.0) - (a_val - 1.0) * cs + beta_rbj * sn;
                a1 = 2.0 * ((a_val - 1.0) - (a_val + 1.0) * cs);
                a2 = (a_val + 1.0) - (a_val - 1.0) * cs - beta_rbj * sn;

                let sum_a = a0 + a1 + a2;
                let diff_a = a0 - a1 + a2;
                let sum_b = sum_a;
                let diff_b = g * diff_a;

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0;

                let w0 = omega;
                let cos_2w0 = (2.0 * w0).cos();
                let sin_2w0 = (2.0 * w0).sin();

                let a_re = a0 + a1 * cs + a2 * cos_2w0;
                let a_im = -a1 * sn - a2 * sin_2w0;
                let den_w0_sq = a_re * a_re + a_im * a_im;
                let target = g * den_w0_sq;

                let c1 = 2.0 * b1 * p * cs;
                let known = p * p / 2.0 + b1 * b1 + c1 + p * p / 2.0 * cos_2w0;
                let d_coeff = 0.5 - 0.5 * cos_2w0;

                let d_sq = if d_coeff.abs() > 1e-15 {
                    (target - known) / d_coeff
                } else {
                    0.0
                };
                let d_val = if d_sq >= 0.0 { d_sq.sqrt() } else { 0.0 };
                let d_signed = if g >= 1.0 { d_val } else { -d_val };

                b0 = (p + d_signed) / 2.0;
                b2 = (p - d_signed) / 2.0;
            }
            BiquadFilterType::PeakMatched => {
                let gain_lin = 10.0_f64.powf(self.db_gain / 20.0);
                let gain_sq = gain_lin * gain_lin;
                let w0 = omega;
                let bw = w0 / self.q;
                let sigma = bw / 2.0;
                let r = (-sigma).exp();
                let r_sq = r * r;

                a0 = 1.0;
                a1 = -2.0 * r * cs;
                a2 = r_sq;

                let sum_a = 1.0 + a1 + a2;
                let diff_a = 1.0 - a1 + a2;
                let sum_b = sum_a;
                let diff_b = diff_a;

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0;

                let cos_2w0 = (2.0 * w0).cos();
                let sin_2w0 = (2.0 * w0).sin();
                let a_re = 1.0 + a1 * cs + a2 * cos_2w0;
                let a_im = -a1 * sn - a2 * sin_2w0;
                let den_w0_sq = a_re * a_re + a_im * a_im;
                let target_num_sq = gain_sq * den_w0_sq;

                let c1 = 2.0 * b1 * p * cs;
                let known = (p * p) / 2.0 + b1 * b1 + c1 + (p * p) / 2.0 * cos_2w0;
                let d_coeff = 0.5 - 0.5 * cos_2w0;

                let d_sq = if d_coeff.abs() > 1e-15 {
                    (target_num_sq - known) / d_coeff
                } else {
                    0.0
                };
                let d_val = if d_sq >= 0.0 { d_sq.sqrt() } else { 0.0 };
                let d_signed = if gain_lin >= 1.0 { d_val } else { -d_val };

                b0 = (p + d_signed) / 2.0;
                b2 = (p - d_signed) / 2.0;
            }
        }

        // Guard against degenerate a0 (extreme parameter combos)
        if a0.abs() < 1e-15 {
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
            self.r_up0 = 1.0;
            self.r_up1 = 0.0;
            self.r_up2 = 0.0;
            self.r_dw0 = 1.0;
            self.r_dw1 = 0.0;
            self.r_dw2 = 0.0;
            return;
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
    ///
    /// When `use_tdf2` is true, uses Transposed Direct Form II which has
    /// better numerical properties for high-Q narrow filters.
    pub fn process(&mut self, x: f64) -> f64 {
        if self.use_tdf2 {
            self.process_tdf2(x)
        } else {
            self.process_df1(x)
        }
    }

    /// Processes a single sample using Direct Form I.
    #[inline(always)]
    fn process_df1(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;

        y
    }

    /// Processes a single sample using Transposed Direct Form II.
    ///
    /// TDF-II uses two state variables (s1, s2) instead of four (x1, x2, y1, y2):
    /// ```text
    /// y  = b0*x + s1
    /// s1 = b1*x - a1*y + s2
    /// s2 = b2*x - a2*y
    /// ```
    ///
    /// This form has better numerical properties for high-Q narrow filters
    /// because it minimizes internal signal magnitudes.
    #[inline(always)]
    fn process_tdf2(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Processes a block of audio samples in-place.
    ///
    /// This method is more efficient than calling `process` for each sample
    /// as it avoids repeated struct field access and allows for better optimization.
    pub fn process_block(&mut self, samples: &mut [f64]) {
        if self.use_tdf2 {
            self.process_block_tdf2(samples);
        } else {
            self.process_block_df1(samples);
        }
    }

    fn process_block_df1(&mut self, samples: &mut [f64]) {
        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;
        let mut x1 = self.x1;
        let mut x2 = self.x2;
        let mut y1 = self.y1;
        let mut y2 = self.y2;

        for x in samples.iter_mut() {
            let input = *x;
            let output = b0 * input + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;

            x2 = x1;
            x1 = input;
            y2 = y1;
            y1 = output;

            *x = output;
        }

        self.x1 = x1;
        self.x2 = x2;
        self.y1 = y1;
        self.y2 = y2;
    }

    fn process_block_tdf2(&mut self, samples: &mut [f64]) {
        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;
        let mut s1 = self.s1;
        let mut s2 = self.s2;

        for x in samples.iter_mut() {
            let input = *x;
            let output = b0 * input + s1;
            s1 = b1 * input - a1 * output + s2;
            s2 = b2 * input - a2 * output;

            *x = output;
        }

        self.s1 = s1;
        self.s2 = s2;
    }

    /// Calculates the filter's complex frequency response at a single frequency `f`.
    pub fn complex_response(&self, f: f64) -> Complex64 {
        let omega = 2.0 * PI * f / self.srate;
        let z_inv = Complex64::from_polar(1.0, -omega);
        let z_inv2 = z_inv * z_inv;

        // Note: coeffs are already normalized by a0
        let num = self.b0 + self.b1 * z_inv + self.b2 * z_inv2;
        let den = 1.0 + self.a1 * z_inv + self.a2 * z_inv2;

        num / den
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

    /// Returns the normalized filter coefficients (a1, a2, b0, b1, b2).
    pub fn coefficients(&self) -> BiquadCoefficients {
        BiquadCoefficients {
            b0: self.b0,
            b1: self.b1,
            b2: self.b2,
            a1: self.a1,
            a2: self.a2,
        }
    }

    /// Processes a single sample using explicitly provided coefficients (DF-I).
    ///
    /// Used for coefficient interpolation during parameter transitions.
    #[inline(always)]
    pub fn process_with_coefficients(&mut self, x: f64, coeffs: &BiquadCoefficients) -> f64 {
        if self.use_tdf2 {
            let y = coeffs.b0 * x + self.s1;
            self.s1 = coeffs.b1 * x - coeffs.a1 * y + self.s2;
            self.s2 = coeffs.b2 * x - coeffs.a2 * y;
            y
        } else {
            let y = coeffs.b0 * x + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
                - coeffs.a1 * self.y1
                - coeffs.a2 * self.y2;
            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;
            y
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
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
        let bq = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 0.0, 3.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = bq.np_log_result(&freqs);
        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }

    // ========================================================================
    // Biquad Filter Type Tests
    // ========================================================================

    #[test]
    fn test_lowpass_filter_response() {
        let cutoff = 1000.0;
        let lp = Biquad::new(BiquadFilterType::Lowpass, cutoff, 48000.0, 0.0, 0.0);
        let dc_response = lp.log_result(10.0);
        assert!(approx_eq(dc_response, 0.0, 0.5), "Lowpass DC response should be ~0 dB, got {}", dc_response);
        let cutoff_response = lp.log_result(cutoff);
        assert!(approx_eq(cutoff_response, -3.0, 0.5), "Lowpass at cutoff should be ~-3 dB, got {}", cutoff_response);
        let high_response = lp.log_result(10000.0);
        assert!(high_response < -20.0, "Lowpass at 10x cutoff should be < -20 dB, got {}", high_response);
    }

    #[test]
    fn test_highpass_filter_response() {
        let cutoff = 1000.0;
        let hp = Biquad::new(BiquadFilterType::Highpass, cutoff, 48000.0, 0.0, 0.0);
        let low_response = hp.log_result(100.0);
        assert!(low_response < -20.0, "Highpass at 0.1x cutoff should be < -20 dB, got {}", low_response);
        let cutoff_response = hp.log_result(cutoff);
        assert!(approx_eq(cutoff_response, -3.0, 0.5), "Highpass at cutoff should be ~-3 dB, got {}", cutoff_response);
        let high_response = hp.log_result(10000.0);
        assert!(approx_eq(high_response, 0.0, 0.5), "Highpass high freq response should be ~0 dB, got {}", high_response);
    }

    #[test]
    fn test_bandpass_filter_response() {
        let center = 1000.0;
        let bp = Biquad::new(BiquadFilterType::Bandpass, center, 48000.0, 1.0, 0.0);
        let center_response = bp.log_result(center);
        let low_response = bp.log_result(100.0);
        assert!(low_response < center_response - 10.0, "Bandpass below center should be attenuated");
        let high_response = bp.log_result(10000.0);
        assert!(high_response < center_response - 10.0, "Bandpass above center should be attenuated");
    }

    #[test]
    fn test_notch_filter_response() {
        let center = 1000.0;
        let notch = Biquad::new(BiquadFilterType::Notch, center, 48000.0, 0.0, 0.0);
        let center_response = notch.log_result(center);
        assert!(center_response < -30.0, "Notch at center should be < -30 dB, got {}", center_response);
        let low_response = notch.log_result(100.0);
        assert!(approx_eq(low_response, 0.0, 1.0), "Notch away from center should be ~0 dB, got {}", low_response);
        let high_response = notch.log_result(10000.0);
        assert!(approx_eq(high_response, 0.0, 1.0), "Notch away from center should be ~0 dB, got {}", high_response);
    }

    #[test]
    fn test_peak_filter_boost() {
        let center = 1000.0;
        let gain_db = 6.0;
        let peak = Biquad::new(BiquadFilterType::Peak, center, 48000.0, 2.0, gain_db);
        let center_response = peak.log_result(center);
        assert!(approx_eq(center_response, gain_db, 0.5), "Peak at center should be ~{} dB, got {}", gain_db, center_response);
        let low_response = peak.log_result(100.0);
        assert!(low_response.abs() < 1.0, "Peak away from center should be ~0 dB, got {}", low_response);
    }

    #[test]
    fn test_peak_filter_cut() {
        let center = 1000.0;
        let gain_db = -6.0;
        let peak = Biquad::new(BiquadFilterType::Peak, center, 48000.0, 2.0, gain_db);
        let center_response = peak.log_result(center);
        assert!(approx_eq(center_response, gain_db, 0.5), "Peak cut at center should be ~{} dB, got {}", gain_db, center_response);
    }

    #[test]
    fn test_lowshelf_filter_response() {
        let freq = 200.0;
        let gain_db = 6.0;
        let ls = Biquad::new(BiquadFilterType::Lowshelf, freq, 48000.0, 0.7, gain_db);
        let low_response = ls.log_result(20.0);
        assert!(approx_eq(low_response, gain_db, 1.0), "Lowshelf below freq should be ~{} dB, got {}", gain_db, low_response);
        let high_response = ls.log_result(5000.0);
        assert!(approx_eq(high_response, 0.0, 1.0), "Lowshelf above freq should be ~0 dB, got {}", high_response);
    }

    #[test]
    fn test_highshelf_filter_response() {
        let freq = 5000.0;
        let gain_db = 6.0;
        let hs = Biquad::new(BiquadFilterType::Highshelf, freq, 48000.0, 0.7, gain_db);
        let low_response = hs.log_result(100.0);
        assert!(approx_eq(low_response, 0.0, 1.0), "Highshelf below freq should be ~0 dB, got {}", low_response);
        let high_response = hs.log_result(20000.0);
        assert!(approx_eq(high_response, gain_db, 1.0), "Highshelf above freq should be ~{} dB, got {}", gain_db, high_response);
    }

    #[test]
    fn test_allpass_filter_response() {
        let center = 1000.0;
        let ap = Biquad::new(BiquadFilterType::AllPass, center, 48000.0, 1.0, 0.0);
        let test_freqs = [20.0, 100.0, 1000.0, 5000.0, 20000.0];
        for &f in &test_freqs {
            let resp = ap.log_result(f);
            assert!(approx_eq(resp, 0.0, 1e-9), "All-Pass magnitude at {}Hz should be 0 dB, got {}", f, resp);
        }
        let resp = ap.complex_response(center);
        let phase = resp.arg();
        assert!(approx_eq(phase.abs(), PI, 1e-9), "All-Pass phase at center freq should be PI, got {}", phase);
    }

    // ========================================================================
    // Orfanidis Shelf Filter Tests
    // ========================================================================

    #[test]
    fn test_lowshelf_orf_response() {
        let freq = 200.0;
        let gain_db = 6.0;
        let ls = Biquad::new(BiquadFilterType::LowshelfOrf, freq, 48000.0, 0.7, gain_db);
        let low_response = ls.log_result(20.0);
        assert!(approx_eq(low_response, gain_db, 1.5), "LowshelfOrf below freq should be ~{} dB, got {}", gain_db, low_response);
        let high_response = ls.log_result(20000.0);
        assert!(approx_eq(high_response, 0.0, 1.5), "LowshelfOrf above freq should be ~0 dB, got {}", high_response);
        let nyquist_response = ls.log_result(23999.0);
        assert!(approx_eq(nyquist_response, 0.0, 0.5), "LowshelfOrf at Nyquist should be ~0 dB (prescribed), got {}", nyquist_response);
    }

    #[test]
    fn test_highshelf_orf_response() {
        let freq = 5000.0;
        let gain_db = 6.0;
        let hs = Biquad::new(BiquadFilterType::HighshelfOrf, freq, 48000.0, 0.7, gain_db);
        let low_response = hs.log_result(100.0);
        assert!(approx_eq(low_response, 0.0, 1.5), "HighshelfOrf below freq should be ~0 dB, got {}", low_response);
        let high_response = hs.log_result(20000.0);
        assert!(approx_eq(high_response, gain_db, 1.5), "HighshelfOrf above freq should be ~{} dB, got {}", gain_db, high_response);
        let dc_response = hs.log_result(10.0);
        assert!(approx_eq(dc_response, 0.0, 0.5), "HighshelfOrf at DC should be ~0 dB (prescribed), got {}", dc_response);
    }

    #[test]
    fn test_lowshelf_orf_cut() {
        let freq = 200.0;
        let gain_db = -6.0;
        let ls = Biquad::new(BiquadFilterType::LowshelfOrf, freq, 48000.0, 0.7, gain_db);
        let low_response = ls.log_result(20.0);
        assert!(approx_eq(low_response, gain_db, 1.5), "LowshelfOrf cut below freq should be ~{} dB, got {}", gain_db, low_response);
        let high_response = ls.log_result(20000.0);
        assert!(approx_eq(high_response, 0.0, 1.5), "LowshelfOrf cut above freq should be ~0 dB, got {}", high_response);
    }

    #[test]
    fn test_highshelf_orf_cut() {
        let freq = 5000.0;
        let gain_db = -6.0;
        let hs = Biquad::new(BiquadFilterType::HighshelfOrf, freq, 48000.0, 0.7, gain_db);
        let low_response = hs.log_result(100.0);
        assert!(approx_eq(low_response, 0.0, 1.5), "HighshelfOrf cut below freq should be ~0 dB, got {}", low_response);
        let high_response = hs.log_result(20000.0);
        assert!(approx_eq(high_response, gain_db, 1.5), "HighshelfOrf cut above freq should be ~{} dB, got {}", gain_db, high_response);
    }

    // ========================================================================
    // Vicanek Matched Peak Filter Tests
    // ========================================================================

    #[test]
    fn test_peak_matched_boost() {
        let center = 1000.0;
        let gain_db = 6.0;
        let peak = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);
        let center_response = peak.log_result(center);
        assert!(approx_eq(center_response, gain_db, 0.5), "PeakMatched at center should be ~{} dB, got {}", gain_db, center_response);
        let low_response = peak.log_result(100.0);
        assert!(low_response.abs() < 1.5, "PeakMatched away from center should be ~0 dB, got {}", low_response);
        let dc_response = peak.log_result(10.0);
        assert!(approx_eq(dc_response, 0.0, 0.5), "PeakMatched at DC should be ~0 dB, got {}", dc_response);
        let nyquist_response = peak.log_result(23999.0);
        assert!(approx_eq(nyquist_response, 0.0, 0.5), "PeakMatched at Nyquist should be ~0 dB, got {}", nyquist_response);
    }

    #[test]
    fn test_peak_matched_cut() {
        let center = 1000.0;
        let gain_db = -6.0;
        let peak = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);
        let center_response = peak.log_result(center);
        assert!(approx_eq(center_response, gain_db, 0.5), "PeakMatched cut at center should be ~{} dB, got {}", gain_db, center_response);
    }

    #[test]
    fn test_peak_matched_high_frequency() {
        let center = 10000.0;
        let gain_db = 6.0;
        let matched = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);
        let center_response = matched.log_result(center);
        assert!(approx_eq(center_response, gain_db, 1.0), "PeakMatched at high freq center should be ~{} dB, got {}", gain_db, center_response);
    }

    // ========================================================================
    // Notch Q Override Fix Tests
    // ========================================================================

    #[test]
    fn test_notch_explicit_q_respected() {
        let notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 5.0, 0.0);
        assert!(approx_eq(notch.q, 5.0, 1e-9), "Notch should use explicit Q=5.0, got {}", notch.q);
    }

    #[test]
    fn test_notch_default_q_when_zero() {
        let notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(approx_eq(notch.q, 30.0, 1e-9), "Notch with Q=0 should default to 30.0, got {}", notch.q);
    }

    #[test]
    fn test_notch_update_params_respects_q() {
        let mut notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(approx_eq(notch.q, 30.0, 1e-9));
        notch.update_params(BiquadFilterType::Notch, 1000.0, 48000.0, 5.0, 0.0);
        assert!(approx_eq(notch.q, 5.0, 1e-9), "update_params should use explicit Q=5.0, got {}", notch.q);
        notch.update_params(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(approx_eq(notch.q, 30.0, 1e-9), "update_params with Q=0 should default to 30.0, got {}", notch.q);
    }

    #[test]
    fn test_notch_with_explicit_q_wider_notch() {
        let narrow = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0); // Q=30
        let wide = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 2.0, 0.0);   // Q=2
        let narrow_off = narrow.log_result(900.0);
        let wide_off = wide.log_result(900.0);
        assert!(wide_off < narrow_off, "Wider notch (Q=2) should attenuate more at 900Hz: wide={}, narrow={}", wide_off, narrow_off);
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
        assert!(matches!(err, crate::error::IirError::InvalidSampleRate { .. }));
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
        assert!(matches!(err, crate::error::IirError::InvalidFrequency { .. }));
    }

    #[test]
    fn test_try_new_invalid_frequency_above_nyquist() {
        let result = Biquad::try_new(BiquadFilterType::Peak, 30000.0, 48000.0, 2.0, 3.0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::IirError::InvalidFrequency { .. }));
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
        let result = Biquad::try_new(BiquadFilterType::Lowpass, 1000.0, 48000.0, 0.0, 0.0);
        assert!(result.is_ok());
        let bq = result.unwrap();
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
        let mut output = 0.0;
        for _ in 0..1000 {
            output = lp.process(1.0);
        }
        assert!(approx_eq(output, 1.0, 0.01), "Lowpass should pass DC, got {}", output);
    }

    #[test]
    fn test_biquad_process_dc_highpass() {
        let mut hp = Biquad::new(BiquadFilterType::Highpass, 1000.0, 48000.0, 0.0, 0.0);
        let mut output = 0.0;
        for _ in 0..1000 {
            output = hp.process(1.0);
        }
        assert!(output.abs() < 0.01, "Highpass should block DC, got {}", output);
    }

    #[test]
    fn test_biquad_process_impulse_response() {
        let mut peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let first = peak.process(1.0);
        let second = peak.process(0.0);
        let third = peak.process(0.0);
        assert!(first.abs() > 0.0, "Impulse response should be non-zero");
        assert!(second.abs() > 0.0 || third.abs() > 0.0, "Peak filter should ring after impulse");
    }

    #[test]
    fn test_biquad_constants() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let (a1, a2, b0, b1, b2) = bq.constants();
        assert!(a1.is_finite());
        assert!(a2.is_finite());
        assert!(b0.is_finite());
        assert!(b1.is_finite());
        assert!(b2.is_finite());
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
        assert_eq!(BiquadFilterType::HighpassVariableQ.long_name(), "HighpassVariableQ");
        assert_eq!(BiquadFilterType::Bandpass.long_name(), "Bandpass");
        assert_eq!(BiquadFilterType::Peak.long_name(), "Peak");
        assert_eq!(BiquadFilterType::Notch.long_name(), "Notch");
        assert_eq!(BiquadFilterType::Lowshelf.long_name(), "Lowshelf");
        assert_eq!(BiquadFilterType::Highshelf.long_name(), "Highshelf");
    }

    // ========================================================================
    // TDF-II Tests
    // ========================================================================

    #[test]
    fn test_tdf2_matches_df1_for_peak() {
        let mut df1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let mut tdf2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        tdf2.use_tdf2 = true;
        for i in 0..1000 {
            let x = (i as f64 * 0.1).sin();
            let y_df1 = df1.process(x);
            let y_tdf2 = tdf2.process(x);
            assert!(approx_eq(y_df1, y_tdf2, 1e-10), "sample {}: df1={} tdf2={} diff={}", i, y_df1, y_tdf2, (y_df1 - y_tdf2).abs());
        }
    }

    #[test]
    fn test_tdf2_matches_df1_for_highshelf() {
        let mut df1 = Biquad::new(BiquadFilterType::Highshelf, 2000.0, 48000.0, 0.707, -3.0);
        let mut tdf2 = Biquad::new(BiquadFilterType::Highshelf, 2000.0, 48000.0, 0.707, -3.0);
        tdf2.use_tdf2 = true;
        for i in 0..1000 {
            let x = (i as f64 * 0.3).sin();
            let y_df1 = df1.process(x);
            let y_tdf2 = tdf2.process(x);
            assert!(approx_eq(y_df1, y_tdf2, 1e-10), "sample {}: df1={} tdf2={} diff={}", i, y_df1, y_tdf2, (y_df1 - y_tdf2).abs());
        }
    }

    #[test]
    fn test_tdf2_process_block_matches_single() {
        let mut single = Biquad::new(BiquadFilterType::Peak, 500.0, 48000.0, 4.0, 10.0);
        single.use_tdf2 = true;
        let mut block = Biquad::new(BiquadFilterType::Peak, 500.0, 48000.0, 4.0, 10.0);
        block.use_tdf2 = true;
        let input: Vec<f64> = (0..256).map(|i| (i as f64 * 0.05).sin()).collect();
        let single_out: Vec<f64> = input.iter().map(|&x| single.process(x)).collect();
        let mut block_buf = input.clone();
        block.process_block(&mut block_buf);
        for i in 0..256 {
            assert!(approx_eq(single_out[i], block_buf[i], 1e-12), "sample {}: single={} block={}", i, single_out[i], block_buf[i]);
        }
    }

    #[test]
    fn test_coefficients_and_lerp() {
        let f1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0);
        let f2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 12.0);
        let c1 = f1.coefficients();
        let c2 = f2.coefficients();
        let lerp0 = c1.lerp(&c2, 0.0);
        assert!(approx_eq(lerp0.b0, c1.b0, 1e-15));
        assert!(approx_eq(lerp0.a1, c1.a1, 1e-15));
        let lerp1 = c1.lerp(&c2, 1.0);
        assert!(approx_eq(lerp1.b0, c2.b0, 1e-15));
        assert!(approx_eq(lerp1.a1, c2.a1, 1e-15));
        let lerp_mid = c1.lerp(&c2, 0.5);
        assert!(approx_eq(lerp_mid.b0, (c1.b0 + c2.b0) / 2.0, 1e-15));
    }

    #[test]
    fn test_process_with_coefficients_matches_normal() {
        let mut f1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let mut f2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let coeffs = f2.coefficients();
        for i in 0..500 {
            let x = (i as f64 * 0.1).sin();
            let y1 = f1.process(x);
            let y2 = f2.process_with_coefficients(x, &coeffs);
            assert!(approx_eq(y1, y2, 1e-12), "sample {}: normal={} with_coeffs={}", i, y1, y2);
        }
    }
}
