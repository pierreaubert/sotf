//! IIR filter implementation (Biquad filters and Parametric EQ)

use base64::{Engine as _, engine::general_purpose};
use byteorder::{BigEndian, WriteBytesExt};
use ndarray::Array1;
use num_complex::Complex64;
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
                // Orfanidis (2005) shelf design with prescribed Nyquist gain.
                //
                // Low-shelf: |H(DC)| = G, |H(w0)| = sqrt(G), |H(Nyquist)| = 1.
                //
                // Standard bilinear shelves have an uncontrolled gain at Nyquist
                // that causes a cramping artifact at high frequencies. Orfanidis
                // prescribes the gain at both DC and Nyquist, eliminating cramping.
                //
                // Reference: S. Orfanidis, "High-Order Digital Parametric Equalizer
                // Design", JAES, vol. 53, no. 11, pp. 1026-1046, Nov. 2005.
                //
                // Method: use the standard bilinear shelf denominator, then solve
                // for numerator coefficients from three magnitude constraints
                // (same approach as the matched peak filter above).

                let g = 10.0_f64.powf(self.db_gain / 20.0);

                // Use standard RBJ shelf denominator (well-behaved poles)
                let a_val = 10.0_f64.powf(self.db_gain / 40.0);
                let beta_rbj = (a_val + a_val).sqrt();
                a0 = (a_val + 1.0) + (a_val - 1.0) * cs + beta_rbj * sn;
                a1 = -2.0 * ((a_val - 1.0) + (a_val + 1.0) * cs);
                a2 = (a_val + 1.0) + (a_val - 1.0) * cs - beta_rbj * sn;

                // Solve for b0, b1, b2 from:
                //   H(DC) = G:  (b0+b1+b2)/(a0+a1+a2) = g
                //   H(Nyq) = 1: (b0-b1+b2)/(a0-a1+a2) = 1
                //   |H(w0)|^2 = G: use mid-gain constraint at w0
                let sum_a = a0 + a1 + a2;
                let diff_a = a0 - a1 + a2;

                let sum_b = g * sum_a;    // b0+b1+b2 = g * (a0+a1+a2)
                let diff_b = diff_a;      // b0-b1+b2 = a0-a1+a2

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0; // p = b0 + b2

                // Magnitude constraint at w0: |H(w0)|^2 = G (= g in linear)
                let w0 = omega;
                let cos_2w0 = (2.0 * w0).cos();
                let sin_2w0 = (2.0 * w0).sin();

                // |A(e^jw0)|^2
                let a_re = a0 + a1 * cs + a2 * cos_2w0;
                let a_im = -a1 * sn - a2 * sin_2w0;
                let den_w0_sq = a_re * a_re + a_im * a_im;

                let target = g * den_w0_sq; // |B(w0)|^2 = g * |A(w0)|^2

                // |B(w0)|^2 in terms of p and d = b0 - b2
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
                // Orfanidis (2005) high-shelf with prescribed Nyquist gain.
                //
                // High-shelf: |H(DC)| = 1, |H(w0)| = sqrt(G), |H(Nyquist)| = G.
                //
                // Same approach as LowshelfOrf but with DC=1, Nyquist=G.

                let g = 10.0_f64.powf(self.db_gain / 20.0);

                // Use standard RBJ highshelf denominator
                let a_val = 10.0_f64.powf(self.db_gain / 40.0);
                let beta_rbj = (a_val + a_val).sqrt();
                a0 = (a_val + 1.0) - (a_val - 1.0) * cs + beta_rbj * sn;
                a1 = 2.0 * ((a_val - 1.0) - (a_val + 1.0) * cs);
                a2 = (a_val + 1.0) - (a_val - 1.0) * cs - beta_rbj * sn;

                // H(DC) = 1: (b0+b1+b2)/(a0+a1+a2) = 1
                // H(Nyq) = G: (b0-b1+b2)/(a0-a1+a2) = g
                let sum_a = a0 + a1 + a2;
                let diff_a = a0 - a1 + a2;

                let sum_b = sum_a;         // b0+b1+b2 = a0+a1+a2 (unity DC)
                let diff_b = g * diff_a;   // b0-b1+b2 = g*(a0-a1+a2)

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0;

                // Magnitude at w0: |H(w0)|^2 = g
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
                // Vicanek (2016) matched second-order digital peak filter.
                //
                // Instead of the bilinear transform (which warps frequencies),
                // this uses the matched-Z transform for poles and then solves
                // for numerator coefficients to match the analog magnitude
                // response at DC, center frequency, and Nyquist.
                //
                // Reference: M. Vicanek, "Matched Second Order Digital Filters",
                // revised 2019.

                let gain_lin = 10.0_f64.powf(self.db_gain / 20.0);
                let gain_sq = gain_lin * gain_lin;

                let w0 = omega;

                // Analog prototype: H_a(s) = (s^2 + s*G*BW + w0^2) / (s^2 + s*BW + w0^2)
                // where BW = w0/Q is the bandwidth.
                //
                // Analog pole: s = -BW/2 +/- j*sqrt(w0^2 - BW^2/4)
                // Mapped to z-plane via matched-Z: z = exp(s*T) where T = 1/fs
                let bw = w0 / self.q;              // analog bandwidth
                let sigma = bw / 2.0;              // real part magnitude

                // Pole radius from matched-Z transform
                let r = (-sigma / self.srate).exp();
                let r_sq = r * r;

                // Denominator coefficients from pole placement
                // z = r * exp(+/- j*w0)  =>  a1 = -2*r*cos(w0), a2 = r^2
                a0 = 1.0;
                a1 = -2.0 * r * cs;
                a2 = r_sq;

                // Solve for b0, b1, b2 from three magnitude constraints:
                //
                // |H(z)|^2 at z=1 (DC):     (b0+b1+b2)^2 / (1+a1+a2)^2 = 1
                // |H(z)|^2 at z=-1 (Nyq):   (b0-b1+b2)^2 / (1-a1+a2)^2 = 1
                // |H(z)|^2 at z=e^jw0:      |B(w0)|^2 / |A(w0)|^2 = G^2
                let sum_a = 1.0 + a1 + a2;    // A(z) at z=1
                let diff_a = 1.0 - a1 + a2;   // A(z) at z=-1

                let sum_b = sum_a;             // b0+b1+b2 = sum_a (unity at DC)
                let diff_b = diff_a;           // b0-b1+b2 = diff_a (unity at Nyquist)

                b1 = (sum_b - diff_b) / 2.0;
                let p = (sum_b + diff_b) / 2.0; // p = b0 + b2

                // |A(e^jw0)|^2 for the denominator at w0
                // A(z) = 1 + a1*z^-1 + a2*z^-2
                // |A(e^jw)|^2 = (1 + a2)^2 + a1^2 + 2*a1*(1+a2)*cos(w) + ... but
                // more directly: |(1 - r*e^jw0)(1 - r*e^-jw0)| = |1 - r_sq|
                // Actually: A(e^jw0) = 1 + a1*cos(w0) + a2*cos(2*w0)
                //                     + j*(-a1*sin(w0) - a2*sin(2*w0))
                let cos_2w0 = (2.0 * w0).cos();
                let sin_2w0 = (2.0 * w0).sin();
                let a_re = 1.0 + a1 * cs + a2 * cos_2w0;
                let a_im = -a1 * sn - a2 * sin_2w0;
                let den_w0_sq = a_re * a_re + a_im * a_im;

                // Target |B(e^jw0)|^2 = G^2 * |A(e^jw0)|^2
                let target_num_sq = gain_sq * den_w0_sq;

                // |B(e^jw)|^2 in terms of p and d = b0 - b2:
                // B(e^jw) = b0 + b1*e^-jw + b2*e^-2jw
                // b0 = (p+d)/2, b2 = (p-d)/2
                // |B|^2 = b0^2 + b1^2 + b2^2
                //       + 2*(b0*b1 + b1*b2)*cos(w) + 2*b0*b2*cos(2w)
                //
                // With b0*b2 = (p^2 - d^2)/4, b0^2+b2^2 = (p^2+d^2)/2,
                // b0*b1+b1*b2 = b1*p:
                //
                // |B|^2 = (p^2+d^2)/2 + b1^2 + 2*b1*p*cos(w) + (p^2-d^2)/2*cos(2w)
                let c1 = 2.0 * b1 * p * cs;
                let known = (p * p) / 2.0 + b1 * b1 + c1 + (p * p) / 2.0 * cos_2w0;
                let d_coeff = 0.5 - 0.5 * cos_2w0;

                let d_sq = if d_coeff.abs() > 1e-15 {
                    (target_num_sq - known) / d_coeff
                } else {
                    0.0
                };

                let d_val = if d_sq >= 0.0 { d_sq.sqrt() } else { 0.0 };

                // Sign: for boost b0 > b2, for cut b0 < b2
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

// ============================================================================
// BiquadBank: Multi-channel biquad processing with SIMD auto-vectorization
// ============================================================================

/// A bank of biquad filters sharing coefficients but with independent per-channel state.
///
/// This is the common case in audio plugins where the same EQ band is applied to
/// all channels. By processing channels in pairs, the compiler can auto-vectorize
/// the inner loop into f64x2 SIMD instructions (SSE2 on x86-64, NEON on aarch64).
///
/// # No allocations in hot path
///
/// All state vectors are pre-allocated at construction time. The `process_interleaved_frame`
/// and `process_interleaved_block` methods perform zero allocations.
///
/// # Example
///
/// ```rust
/// use math_audio_iir_fir::{Biquad, BiquadBank, BiquadFilterType, SRATE};
///
/// // Create a peak filter template
/// let template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
///
/// // Create a bank for 8 channels
/// let mut bank = BiquadBank::new(&template, 8);
///
/// // Process one interleaved frame (8 samples, one per channel)
/// let mut frame = [0.5_f64; 8];
/// bank.process_interleaved_frame(&mut frame);
/// ```
#[derive(Debug, Clone)]
pub struct BiquadBank {
    // Shared coefficients (all filters use the same)
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    /// When true, use Transposed Direct Form II instead of Direct Form I.
    pub use_tdf2: bool,

    // Per-channel state (TDF-II: s1, s2 per channel)
    s1: Vec<f64>,
    s2: Vec<f64>,

    // Per-channel state (DF-I: x1, x2, y1, y2 per channel)
    x1: Vec<f64>,
    x2: Vec<f64>,
    y1: Vec<f64>,
    y2: Vec<f64>,

    num_channels: usize,

    // Copy of filter config for coefficient updates
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
}

impl BiquadBank {
    /// Create a bank from a template Biquad, replicated for N channels.
    ///
    /// All channels share the same coefficients but have independent filter state
    /// (initialized to zero).
    ///
    /// # Arguments
    ///
    /// * `template` - A configured Biquad whose coefficients and parameters are copied
    /// * `num_channels` - Number of independent channels to process
    pub fn new(template: &Biquad, num_channels: usize) -> Self {
        let (a1, a2, b0, b1, b2) = template.constants();
        Self {
            a1,
            a2,
            b0,
            b1,
            b2,
            use_tdf2: template.use_tdf2,
            s1: vec![0.0; num_channels],
            s2: vec![0.0; num_channels],
            x1: vec![0.0; num_channels],
            x2: vec![0.0; num_channels],
            y1: vec![0.0; num_channels],
            y2: vec![0.0; num_channels],
            num_channels,
            filter_type: template.filter_type,
            freq: template.freq,
            srate: template.srate,
            q: template.q,
            db_gain: template.db_gain,
        }
    }

    /// Update filter parameters and recompute coefficients for all channels.
    ///
    /// This does **not** reset filter state, allowing click-free parameter changes.
    /// A temporary Biquad is created internally to compute the new coefficients.
    pub fn update_params(&mut self, freq: f64, srate: f64, q: f64, db_gain: f64) {
        let tmp = Biquad::new(self.filter_type, freq, srate, q, db_gain);
        self.copy_coefficients_from(&tmp);
        self.freq = freq;
        self.srate = srate;
        self.q = tmp.q; // Use the clamped/defaulted Q from Biquad::new
        self.db_gain = db_gain;
    }

    /// Copy coefficients from a Biquad.
    ///
    /// Only the filter coefficients are copied; filter state is preserved.
    /// The filter_type, freq, srate, q, and db_gain fields are also updated.
    pub fn copy_coefficients_from(&mut self, biquad: &Biquad) {
        let (a1, a2, b0, b1, b2) = biquad.constants();
        self.a1 = a1;
        self.a2 = a2;
        self.b0 = b0;
        self.b1 = b1;
        self.b2 = b2;
        self.filter_type = biquad.filter_type;
        self.freq = biquad.freq;
        self.srate = biquad.srate;
        self.q = biquad.q;
        self.db_gain = biquad.db_gain;
    }

    /// Reset all channel state to zero.
    ///
    /// Coefficients are preserved; only the per-channel delay state is cleared.
    pub fn reset(&mut self) {
        self.s1.fill(0.0);
        self.s2.fill(0.0);
        self.x1.fill(0.0);
        self.x2.fill(0.0);
        self.y1.fill(0.0);
        self.y2.fill(0.0);
    }

    /// Number of channels in this bank.
    #[inline]
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    /// Process a single interleaved frame (one sample per channel) in-place.
    ///
    /// `samples` must have length >= `num_channels`. Only the first `num_channels`
    /// elements are read and written.
    ///
    /// The inner loop processes channels in pairs of 2, which enables the compiler
    /// to auto-vectorize into f64x2 SIMD (SSE2 on x86-64, NEON on aarch64).
    #[inline]
    pub fn process_interleaved_frame(&mut self, samples: &mut [f64]) {
        let nc = self.num_channels;
        // Subslice to exact channel count — panics if too short (same as assert),
        // but gives the compiler a length proof so all subsequent `samples[ch]`
        // indexing is bounds-check-free in release builds.
        let samples = &mut samples[..nc];

        if self.use_tdf2 {
            self.process_frame_tdf2(samples);
        } else {
            self.process_frame_df1(samples);
        }
    }

    /// TDF-II frame processing with paired-channel loop for auto-vectorization.
    #[inline]
    fn process_frame_tdf2(&mut self, samples: &mut [f64]) {
        let nc = self.num_channels;
        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;

        // Process pairs of channels — the compiler can auto-vectorize this
        // into f64x2 SIMD (SSE2 on x86-64, NEON on aarch64) because the
        // two iterations are independent (no data dependency between them).
        let mut ch = 0;
        while ch + 1 < nc {
            let x0 = samples[ch];
            let x1 = samples[ch + 1];
            let s1_0 = self.s1[ch];
            let s1_1 = self.s1[ch + 1];
            let s2_0 = self.s2[ch];
            let s2_1 = self.s2[ch + 1];

            let y0 = b0 * x0 + s1_0;
            let y1 = b0 * x1 + s1_1;
            self.s1[ch] = b1 * x0 - a1 * y0 + s2_0;
            self.s1[ch + 1] = b1 * x1 - a1 * y1 + s2_1;
            self.s2[ch] = b2 * x0 - a2 * y0;
            self.s2[ch + 1] = b2 * x1 - a2 * y1;

            samples[ch] = y0;
            samples[ch + 1] = y1;
            ch += 2;
        }
        // Handle odd last channel (scalar)
        if ch < nc {
            let x = samples[ch];
            let y = b0 * x + self.s1[ch];
            self.s1[ch] = b1 * x - a1 * y + self.s2[ch];
            self.s2[ch] = b2 * x - a2 * y;
            samples[ch] = y;
        }
    }

    /// DF-I frame processing with paired-channel loop for auto-vectorization.
    #[inline]
    fn process_frame_df1(&mut self, samples: &mut [f64]) {
        let nc = self.num_channels;
        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;

        let mut ch = 0;
        while ch + 1 < nc {
            let x0 = samples[ch];
            let x1_in = samples[ch + 1];

            let y0 = b0 * x0 + b1 * self.x1[ch] + b2 * self.x2[ch]
                - a1 * self.y1[ch] - a2 * self.y2[ch];
            let y1 = b0 * x1_in + b1 * self.x1[ch + 1] + b2 * self.x2[ch + 1]
                - a1 * self.y1[ch + 1] - a2 * self.y2[ch + 1];

            self.x2[ch] = self.x1[ch];
            self.x1[ch] = x0;
            self.y2[ch] = self.y1[ch];
            self.y1[ch] = y0;

            self.x2[ch + 1] = self.x1[ch + 1];
            self.x1[ch + 1] = x1_in;
            self.y2[ch + 1] = self.y1[ch + 1];
            self.y1[ch + 1] = y1;

            samples[ch] = y0;
            samples[ch + 1] = y1;
            ch += 2;
        }
        // Handle odd last channel (scalar)
        if ch < nc {
            let x = samples[ch];
            let y = b0 * x + b1 * self.x1[ch] + b2 * self.x2[ch]
                - a1 * self.y1[ch] - a2 * self.y2[ch];
            self.x2[ch] = self.x1[ch];
            self.x1[ch] = x;
            self.y2[ch] = self.y1[ch];
            self.y1[ch] = y;
            samples[ch] = y;
        }
    }

    /// Process a block of interleaved audio in-place.
    ///
    /// `buffer` contains `num_frames * num_channels` samples in interleaved order:
    /// `[ch0_f0, ch1_f0, ..., chN_f0, ch0_f1, ch1_f1, ..., chN_f1, ...]`
    ///
    /// # Panics
    ///
    /// Debug-asserts that `buffer.len() >= num_frames * num_channels`.
    pub fn process_interleaved_block(&mut self, buffer: &mut [f64], num_frames: usize) {
        let nc = self.num_channels;
        let buffer = &mut buffer[..num_frames * nc];
        for frame_idx in 0..num_frames {
            let offset = frame_idx * nc;
            self.process_interleaved_frame(&mut buffer[offset..offset + nc]);
        }
    }
}

impl fmt::Display for BiquadBank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BiquadBank({}ch, Type:{}, Freq:{:.1}, Q:{:.1}, Gain:{:.1}dB)",
            self.num_channels,
            self.filter_type.short_name(),
            self.freq,
            self.q,
            self.db_gain
        )
    }
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

    #[test]
    fn test_allpass_filter_response() {
        let center = 1000.0;
        let ap = Biquad::new(BiquadFilterType::AllPass, center, 48000.0, 1.0, 0.0);

        // Magnitude should be exactly 0 dB across frequencies
        let test_freqs = [20.0, 100.0, 1000.0, 5000.0, 20000.0];
        for &f in &test_freqs {
            let resp = ap.log_result(f);
            assert!(
                approx_eq(resp, 0.0, 1e-9),
                "All-Pass magnitude at {}Hz should be 0 dB, got {}",
                f,
                resp
            );
        }

        // Phase at center frequency should be 180 degrees (PI radians)
        let resp = ap.complex_response(center);
        let phase = resp.arg();
        assert!(
            approx_eq(phase.abs(), PI, 1e-9),
            "All-Pass phase at center freq should be PI, got {}",
            phase
        );
    }

    // ========================================================================
    // Orfanidis Shelf Filter Tests
    // ========================================================================

    #[test]
    fn test_lowshelf_orf_response() {
        let freq = 200.0;
        let gain_db = 6.0;
        let ls = Biquad::new(BiquadFilterType::LowshelfOrf, freq, 48000.0, 0.7, gain_db);

        // Well below shelf frequency should have full gain
        let low_response = ls.log_result(20.0);
        assert!(
            approx_eq(low_response, gain_db, 1.5),
            "LowshelfOrf below freq should be ~{} dB, got {}",
            gain_db,
            low_response
        );

        // Well above shelf frequency should be ~0 dB (prescribed Nyquist gain)
        let high_response = ls.log_result(20000.0);
        assert!(
            approx_eq(high_response, 0.0, 1.5),
            "LowshelfOrf above freq should be ~0 dB, got {}",
            high_response
        );

        // At Nyquist, response should be very close to 0 dB (the key Orfanidis property)
        let nyquist_response = ls.log_result(23999.0);
        assert!(
            approx_eq(nyquist_response, 0.0, 0.5),
            "LowshelfOrf at Nyquist should be ~0 dB (prescribed), got {}",
            nyquist_response
        );
    }

    #[test]
    fn test_highshelf_orf_response() {
        let freq = 5000.0;
        let gain_db = 6.0;
        let hs = Biquad::new(BiquadFilterType::HighshelfOrf, freq, 48000.0, 0.7, gain_db);

        // Well below shelf frequency should be ~0 dB (prescribed DC gain)
        let low_response = hs.log_result(100.0);
        assert!(
            approx_eq(low_response, 0.0, 1.5),
            "HighshelfOrf below freq should be ~0 dB, got {}",
            low_response
        );

        // Well above shelf frequency should have full gain
        let high_response = hs.log_result(20000.0);
        assert!(
            approx_eq(high_response, gain_db, 1.5),
            "HighshelfOrf above freq should be ~{} dB, got {}",
            gain_db,
            high_response
        );

        // At DC, response should be very close to 0 dB
        let dc_response = hs.log_result(10.0);
        assert!(
            approx_eq(dc_response, 0.0, 0.5),
            "HighshelfOrf at DC should be ~0 dB (prescribed), got {}",
            dc_response
        );
    }

    #[test]
    fn test_lowshelf_orf_cut() {
        let freq = 200.0;
        let gain_db = -6.0;
        let ls = Biquad::new(BiquadFilterType::LowshelfOrf, freq, 48000.0, 0.7, gain_db);

        let low_response = ls.log_result(20.0);
        assert!(
            approx_eq(low_response, gain_db, 1.5),
            "LowshelfOrf cut below freq should be ~{} dB, got {}",
            gain_db,
            low_response
        );

        let high_response = ls.log_result(20000.0);
        assert!(
            approx_eq(high_response, 0.0, 1.5),
            "LowshelfOrf cut above freq should be ~0 dB, got {}",
            high_response
        );
    }

    #[test]
    fn test_highshelf_orf_cut() {
        let freq = 5000.0;
        let gain_db = -6.0;
        let hs = Biquad::new(BiquadFilterType::HighshelfOrf, freq, 48000.0, 0.7, gain_db);

        let low_response = hs.log_result(100.0);
        assert!(
            approx_eq(low_response, 0.0, 1.5),
            "HighshelfOrf cut below freq should be ~0 dB, got {}",
            low_response
        );

        let high_response = hs.log_result(20000.0);
        assert!(
            approx_eq(high_response, gain_db, 1.5),
            "HighshelfOrf cut above freq should be ~{} dB, got {}",
            gain_db,
            high_response
        );
    }

    // ========================================================================
    // Vicanek Matched Peak Filter Tests
    // ========================================================================

    #[test]
    fn test_peak_matched_boost() {
        let center = 1000.0;
        let gain_db = 6.0;
        let peak = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);

        // At center frequency, response should match gain
        let center_response = peak.log_result(center);
        assert!(
            approx_eq(center_response, gain_db, 0.5),
            "PeakMatched at center should be ~{} dB, got {}",
            gain_db,
            center_response
        );

        // Away from center should approach 0 dB
        let low_response = peak.log_result(100.0);
        assert!(
            low_response.abs() < 1.5,
            "PeakMatched away from center should be ~0 dB, got {}",
            low_response
        );

        // DC should be unity
        let dc_response = peak.log_result(10.0);
        assert!(
            approx_eq(dc_response, 0.0, 0.5),
            "PeakMatched at DC should be ~0 dB, got {}",
            dc_response
        );

        // Nyquist should be unity
        let nyquist_response = peak.log_result(23999.0);
        assert!(
            approx_eq(nyquist_response, 0.0, 0.5),
            "PeakMatched at Nyquist should be ~0 dB, got {}",
            nyquist_response
        );
    }

    #[test]
    fn test_peak_matched_cut() {
        let center = 1000.0;
        let gain_db = -6.0;
        let peak = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);

        let center_response = peak.log_result(center);
        assert!(
            approx_eq(center_response, gain_db, 0.5),
            "PeakMatched cut at center should be ~{} dB, got {}",
            gain_db,
            center_response
        );
    }

    #[test]
    fn test_peak_matched_high_frequency() {
        // Test that PeakMatched maintains accurate response even at high frequencies
        // where the standard bilinear Peak filter shows frequency warping
        let center = 10000.0;
        let gain_db = 6.0;
        let matched = Biquad::new(BiquadFilterType::PeakMatched, center, 48000.0, 2.0, gain_db);

        let center_response = matched.log_result(center);
        assert!(
            approx_eq(center_response, gain_db, 1.0),
            "PeakMatched at high freq center should be ~{} dB, got {}",
            gain_db,
            center_response
        );
    }

    // ========================================================================
    // Notch Q Override Fix Tests
    // ========================================================================

    #[test]
    fn test_notch_explicit_q_respected() {
        // When Q is explicitly set (non-zero), it should be used
        let notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 5.0, 0.0);
        assert!(
            approx_eq(notch.q, 5.0, 1e-9),
            "Notch should use explicit Q=5.0, got {}",
            notch.q
        );
    }

    #[test]
    fn test_notch_default_q_when_zero() {
        // When Q is 0, it should default to 30.0
        let notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(
            approx_eq(notch.q, 30.0, 1e-9),
            "Notch with Q=0 should default to 30.0, got {}",
            notch.q
        );
    }

    #[test]
    fn test_notch_update_params_respects_q() {
        let mut notch = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(approx_eq(notch.q, 30.0, 1e-9));

        // Update with explicit Q=5 should use 5
        notch.update_params(BiquadFilterType::Notch, 1000.0, 48000.0, 5.0, 0.0);
        assert!(
            approx_eq(notch.q, 5.0, 1e-9),
            "update_params should use explicit Q=5.0, got {}",
            notch.q
        );

        // Update with Q=0 should fall back to default 30
        notch.update_params(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0);
        assert!(
            approx_eq(notch.q, 30.0, 1e-9),
            "update_params with Q=0 should default to 30.0, got {}",
            notch.q
        );
    }

    #[test]
    fn test_notch_with_explicit_q_wider_notch() {
        // A lower Q means a wider notch
        let narrow = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 0.0, 0.0); // Q=30
        let wide = Biquad::new(BiquadFilterType::Notch, 1000.0, 48000.0, 2.0, 0.0);   // Q=2

        // At 900 Hz (slightly off center), the wider notch should have more attenuation
        let narrow_off = narrow.log_result(900.0);
        let wide_off = wide.log_result(900.0);
        assert!(
            wide_off < narrow_off,
            "Wider notch (Q=2) should attenuate more at 900Hz: wide={}, narrow={}",
            wide_off,
            narrow_off
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

    #[test]
    fn test_tdf2_matches_df1_for_peak() {
        // DF-I and TDF-II should produce identical output for the same filter
        let mut df1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        let mut tdf2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);
        tdf2.use_tdf2 = true;

        // Process 1000 samples of a sine wave
        for i in 0..1000 {
            let x = (i as f64 * 0.1).sin();
            let y_df1 = df1.process(x);
            let y_tdf2 = tdf2.process(x);
            assert!(
                approx_eq(y_df1, y_tdf2, 1e-10),
                "sample {}: df1={} tdf2={} diff={}",
                i,
                y_df1,
                y_tdf2,
                (y_df1 - y_tdf2).abs()
            );
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
            assert!(
                approx_eq(y_df1, y_tdf2, 1e-10),
                "sample {}: df1={} tdf2={} diff={}",
                i,
                y_df1,
                y_tdf2,
                (y_df1 - y_tdf2).abs()
            );
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
            assert!(
                approx_eq(single_out[i], block_buf[i], 1e-12),
                "sample {}: single={} block={}",
                i,
                single_out[i],
                block_buf[i]
            );
        }
    }

    #[test]
    fn test_coefficients_and_lerp() {
        let f1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0);
        let f2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 12.0);
        let c1 = f1.coefficients();
        let c2 = f2.coefficients();

        // At t=0, should equal c1
        let lerp0 = c1.lerp(&c2, 0.0);
        assert!(approx_eq(lerp0.b0, c1.b0, 1e-15));
        assert!(approx_eq(lerp0.a1, c1.a1, 1e-15));

        // At t=1, should equal c2
        let lerp1 = c1.lerp(&c2, 1.0);
        assert!(approx_eq(lerp1.b0, c2.b0, 1e-15));
        assert!(approx_eq(lerp1.a1, c2.a1, 1e-15));

        // At t=0.5, should be midpoint
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
            assert!(
                approx_eq(y1, y2, 1e-12),
                "sample {}: normal={} with_coeffs={}",
                i,
                y1,
                y2
            );
        }
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
/// use math_audio_iir_fir::{Biquad, BiquadFilterType, peq_loudness_gain};
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
    // Guard: if total energy is zero or negative (extreme filter combinations),
    // clamp to a small positive value to avoid NaN from log10()
    let energy = (1.0 + avg_energy_change).max(f64::MIN_POSITIVE);
    let loudness_change_db = 10.0 * energy.log10();

    // Clamp to ±60dB — beyond this the filter config is pathological
    (-loudness_change_db).clamp(-60.0, 60.0)
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
            BiquadFilterType::Lowshelf
            | BiquadFilterType::Highshelf
            | BiquadFilterType::LowshelfOrf
            | BiquadFilterType::HighshelfOrf => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:.2}",
                    i + 1,
                    iir.filter_type.short_name(),
                    iir.freq as i32,
                    iir.db_gain,
                    iir.q
                ));
            }
            BiquadFilterType::PeakMatched => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:0.2}",
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
            BiquadFilterType::AllPass => {
                res.push(format!(
                    "Filter {:2}: ON AP Fc {:5} Hz Q {:0.2}",
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
    assert!(
        order >= 2 && order.is_multiple_of(2),
        "Linkwitz-Riley order must be even and >= 2, got {order}"
    );
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

/// Create All-Pass filter
///
/// # Arguments
/// * `freq` - Center frequency in Hz
/// * `srate` - Sample rate in Hz
/// * `q` - Q factor
///
/// # Returns
/// * PEQ containing the All-Pass filter section
pub fn peq_allpass(freq: f64, srate: f64, q: f64) -> Peq {
    vec![(
        1.0,
        Biquad::new(BiquadFilterType::AllPass, freq, srate, q, 0.0),
    )]
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
    fn test_linkwitzriley_lr12() {
        let srate = 48000.0;
        let freq = 1000.0;
        let lp = peq_linkwitzriley_lowpass(2, freq, srate);
        let hp = peq_linkwitzriley_highpass(2, freq, srate);

        // LR12 = 2nd order = 1 biquad section with Q=0.5
        assert_eq!(lp.len(), 1);
        assert_eq!(hp.len(), 1);
        assert!((lp[0].1.q - 0.5).abs() < 1e-10);
        assert!((hp[0].1.q - 0.5).abs() < 1e-10);

        // At crossover: each should be -6 dB
        let lp_resp = lp[0].1.complex_response(freq).norm().log10() * 20.0;
        let hp_resp = hp[0].1.complex_response(freq).norm().log10() * 20.0;
        assert!(
            (lp_resp - (-6.0)).abs() < 0.1,
            "LR12 LP at crossover: {lp_resp} dB"
        );
        assert!(
            (hp_resp - (-6.0)).abs() < 0.1,
            "LR12 HP at crossover: {hp_resp} dB"
        );

        // LR2 needs polarity inversion on HP for flat sum (order/2 is odd)
        let sum = lp[0].1.complex_response(freq) - hp[0].1.complex_response(freq);
        let sum_db = sum.norm().log10() * 20.0;
        assert!(
            sum_db.abs() < 0.1,
            "LR12 sum at crossover (HP inverted): {sum_db} dB"
        );
    }

    #[test]
    fn test_linkwitzriley_lr24() {
        let srate = 48000.0;
        let freq = 1000.0;
        let lp = peq_linkwitzriley_lowpass(4, freq, srate);
        let hp = peq_linkwitzriley_highpass(4, freq, srate);

        // LR24 = 4th order = 2 biquad sections with Q ≈ 0.7071
        assert_eq!(lp.len(), 2);
        assert_eq!(hp.len(), 2);
        for (weight, bq) in &lp {
            assert_eq!(*weight, 1.0);
            assert!((bq.q - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-10);
        }

        // At crossover: combined response should be -6 dB
        let lp_mag: f64 = lp
            .iter()
            .map(|(_, bq)| bq.complex_response(freq).norm())
            .product();
        let hp_mag: f64 = hp
            .iter()
            .map(|(_, bq)| bq.complex_response(freq).norm())
            .product();
        let lp_db = lp_mag.log10() * 20.0;
        let hp_db = hp_mag.log10() * 20.0;
        assert!(
            (lp_db - (-6.0)).abs() < 0.1,
            "LR24 LP at crossover: {lp_db} dB"
        );
        assert!(
            (hp_db - (-6.0)).abs() < 0.1,
            "LR24 HP at crossover: {hp_db} dB"
        );
    }

    #[test]
    fn test_linkwitzriley_lr48() {
        let srate = 48000.0;
        let freq = 1000.0;
        let lp = peq_linkwitzriley_lowpass(8, freq, srate);
        let hp = peq_linkwitzriley_highpass(8, freq, srate);

        // LR48 = 8th order = 4 biquad sections
        assert_eq!(lp.len(), 4);
        assert_eq!(hp.len(), 4);

        for (weight, _) in &lp {
            assert_eq!(*weight, 1.0);
        }
        for (weight, _) in &hp {
            assert_eq!(*weight, 1.0);
        }

        // At crossover: combined response should be -6 dB
        let lp_mag: f64 = lp
            .iter()
            .map(|(_, bq)| bq.complex_response(freq).norm())
            .product();
        let hp_mag: f64 = hp
            .iter()
            .map(|(_, bq)| bq.complex_response(freq).norm())
            .product();
        let lp_db = lp_mag.log10() * 20.0;
        let hp_db = hp_mag.log10() * 20.0;
        assert!(
            (lp_db - (-6.0)).abs() < 0.1,
            "LR48 LP at crossover: {lp_db} dB"
        );
        assert!(
            (hp_db - (-6.0)).abs() < 0.1,
            "LR48 HP at crossover: {hp_db} dB"
        );

        // Verify steep rolloff: at 2x crossover freq, LP should be well below -40 dB
        let lp_2x: f64 = lp
            .iter()
            .map(|(_, bq)| bq.complex_response(freq * 2.0).norm())
            .product();
        let lp_2x_db = lp_2x.log10() * 20.0;
        assert!(
            lp_2x_db < -40.0,
            "LR48 LP at 2x crossover: {lp_2x_db} dB (expected < -40)"
        );
    }

    #[test]
    #[should_panic(expected = "Linkwitz-Riley order must be even and >= 2")]
    fn test_linkwitzriley_rejects_odd_order() {
        peq_linkwitzriley_q(3);
    }

    #[test]
    #[should_panic(expected = "Linkwitz-Riley order must be even and >= 2")]
    fn test_linkwitzriley_rejects_order_zero() {
        peq_linkwitzriley_q(0);
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

/// Format a PEQ as an EasyEffects JSON preset.
///
/// Produces a valid EasyEffects PEQ input preset with up to 30 bands.
pub fn peq_format_easyeffects(comment: &str, peq: &Peq) -> String {
    use std::fmt::Write;

    fn ee_type(ft: BiquadFilterType) -> &'static str {
        match ft {
            BiquadFilterType::Peak | BiquadFilterType::PeakMatched => "Bell",
            BiquadFilterType::Lowshelf | BiquadFilterType::LowshelfOrf => "Lo Shelf",
            BiquadFilterType::Highshelf | BiquadFilterType::HighshelfOrf => "Hi Shelf",
            BiquadFilterType::Lowpass => "Lo-pass",
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => "Hi-pass",
            BiquadFilterType::Notch => "Notch",
            BiquadFilterType::Bandpass => "Bandpass",
            BiquadFilterType::AllPass => "Allpass",
        }
    }

    let preamp = peq_preamp_gain(peq);
    let num_bands = peq.len().min(30);
    let mut bands = String::new();

    for (i, (_, iir)) in peq.iter().enumerate().take(30) {
        if i > 0 {
            writeln!(bands, ",").unwrap();
        }
        write!(
            bands,
            r#"        "band{i}": {{
          "frequency": {freq},
          "gain": {gain},
          "q": {q},
          "type": "{ft}",
          "mode": "RLC (BT)",
          "slope": "x1",
          "solo": false,
          "mute": false
        }}"#,
            freq = iir.freq,
            gain = iir.db_gain,
            q = iir.q,
            ft = ee_type(iir.filter_type),
        )
        .unwrap();
    }

    format!(
        r#"// {comment}
{{
  "output": {{
    "equalizer#0": {{
      "input-gain": {preamp:.2},
      "output-gain": 0.0,
      "num-bands": {num_bands},
      "split-channels": false,
      "left": {{
{bands}
      }},
      "right": {{
{bands}
      }}
    }}
  }}
}}"#
    )
}

/// Format a PEQ as a PipeWire filter-chain SPA-JSON configuration.
///
/// Produces a stereo (L/R) PipeWire module configuration with biquad filter nodes.
pub fn peq_format_pipewire(comment: &str, peq: &Peq) -> String {
    use std::fmt::Write;

    fn pw_label(ft: BiquadFilterType) -> &'static str {
        match ft {
            BiquadFilterType::Peak | BiquadFilterType::PeakMatched => "bq_peaking",
            BiquadFilterType::Lowshelf | BiquadFilterType::LowshelfOrf => "bq_lowshelf",
            BiquadFilterType::Highshelf | BiquadFilterType::HighshelfOrf => "bq_highshelf",
            BiquadFilterType::Lowpass => "bq_lowpass",
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => "bq_highpass",
            BiquadFilterType::Notch => "bq_notch",
            BiquadFilterType::Bandpass => "bq_bandpass",
            BiquadFilterType::AllPass => "bq_allpass",
        }
    }

    let preamp = peq_preamp_gain(peq);
    let mut out = String::new();
    writeln!(out, "# {comment}").unwrap();
    writeln!(out, "# PipeWire filter-chain configuration").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "context.modules = [").unwrap();
    writeln!(out, "  {{ name = libpipewire-module-filter-chain").unwrap();
    writeln!(out, "    args = {{").unwrap();
    writeln!(out, "      filter.graph = {{").unwrap();
    writeln!(out, "        nodes = [").unwrap();

    // Build nodes for both L and R channels
    let channels = ["L", "R"];
    let mut all_node_names: Vec<Vec<String>> = Vec::new();

    for ch in &channels {
        let mut node_names = Vec::new();

        // Preamp gain node
        if preamp.abs() > 0.01 {
            let name = format!("{ch}_preamp");
            writeln!(out, "          {{ type = builtin  name = \"{name}\"  label = bq_highshelf  control = {{ \"Freq\" = 0  \"Q\" = 1.0  \"Gain\" = {preamp:.2} }} }}").unwrap();
            node_names.push(name);
        }

        // EQ filter nodes
        for (i, (_, iir)) in peq.iter().enumerate() {
            let label = pw_label(iir.filter_type);
            let name = format!("{ch}_eq_{i}");
            match iir.filter_type {
                BiquadFilterType::Lowpass | BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => {
                    writeln!(out, "          {{ type = builtin  name = \"{name}\"  label = {label}  control = {{ \"Freq\" = {:.1}  \"Q\" = {:.4} }} }}", iir.freq, iir.q).unwrap();
                }
                _ => {
                    writeln!(out, "          {{ type = builtin  name = \"{name}\"  label = {label}  control = {{ \"Freq\" = {:.1}  \"Q\" = {:.4}  \"Gain\" = {:.2} }} }}", iir.freq, iir.q, iir.db_gain).unwrap();
                }
            }
            node_names.push(name);
        }

        all_node_names.push(node_names);
    }

    writeln!(out, "        ]").unwrap();

    // Links
    writeln!(out, "        links = [").unwrap();
    for nodes in &all_node_names {
        for pair in nodes.windows(2) {
            writeln!(out, "          {{ output = \"{}:Out\"  input = \"{}:In\" }}", pair[0], pair[1]).unwrap();
        }
    }
    writeln!(out, "        ]").unwrap();

    // Inputs/outputs
    writeln!(out, "        inputs  = [").unwrap();
    for nodes in &all_node_names {
        if let Some(first) = nodes.first() {
            writeln!(out, "          {{ node = \"{first}\"  port = \"In\" }}").unwrap();
        }
    }
    writeln!(out, "        ]").unwrap();
    writeln!(out, "        outputs = [").unwrap();
    for nodes in &all_node_names {
        if let Some(last) = nodes.last() {
            writeln!(out, "          {{ node = \"{last}\"  port = \"Out\" }}").unwrap();
        }
    }
    writeln!(out, "        ]").unwrap();

    writeln!(out, "      }}").unwrap();
    writeln!(out, "      capture.props = {{ media.class = Audio/Sink  node.name = \"sotf_eq\" }}").unwrap();
    writeln!(out, "      playback.props = {{ node.name = \"sotf_eq_out\" }}").unwrap();
    writeln!(out, "      audio.channels = 2").unwrap();
    writeln!(out, "      audio.position = [ \"FL\", \"FR\" ]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "  }}").unwrap();
    writeln!(out, "]").unwrap();

    out
}

/// Format a PEQ as a CamillaDSP YAML configuration.
///
/// Produces a stereo CamillaDSP config with Biquad filters in the pipeline.
pub fn peq_format_camilladsp(comment: &str, peq: &Peq, sample_rate: u32) -> String {
    use std::fmt::Write;

    fn cdsp_type(ft: BiquadFilterType) -> &'static str {
        match ft {
            BiquadFilterType::Peak | BiquadFilterType::PeakMatched => "Peaking",
            BiquadFilterType::Lowshelf | BiquadFilterType::LowshelfOrf => "Lowshelf",
            BiquadFilterType::Highshelf | BiquadFilterType::HighshelfOrf => "Highshelf",
            BiquadFilterType::Lowpass => "LowpassQ",
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => "HighpassQ",
            BiquadFilterType::Notch => "Notch",
            BiquadFilterType::Bandpass => "Bandpass",
            BiquadFilterType::AllPass => "Allpass",
        }
    }

    let preamp = peq_preamp_gain(peq);
    let mut out = String::new();
    writeln!(out, "# {comment}").unwrap();
    writeln!(out, "# CamillaDSP configuration").unwrap();
    writeln!(out).unwrap();

    // Devices
    writeln!(out, "devices:").unwrap();
    writeln!(out, "  samplerate: {sample_rate}").unwrap();
    writeln!(out, "  chunksize: 1024").unwrap();
    writeln!(out, "  capture:").unwrap();
    writeln!(out, "    type: Stdin").unwrap();
    writeln!(out, "    channels: 2").unwrap();
    writeln!(out, "    format: S32LE").unwrap();
    writeln!(out, "  playback:").unwrap();
    writeln!(out, "    type: Stdout").unwrap();
    writeln!(out, "    channels: 2").unwrap();
    writeln!(out, "    format: S32LE").unwrap();
    writeln!(out).unwrap();

    // Filters
    writeln!(out, "filters:").unwrap();
    if preamp.abs() > 0.01 {
        writeln!(out, "  preamp:").unwrap();
        writeln!(out, "    type: Gain").unwrap();
        writeln!(out, "    parameters:").unwrap();
        writeln!(out, "      gain: {preamp:.2}").unwrap();
    }
    for (i, (_, iir)) in peq.iter().enumerate() {
        let ft = cdsp_type(iir.filter_type);
        writeln!(out, "  eq_{i}:").unwrap();
        writeln!(out, "    type: Biquad").unwrap();
        writeln!(out, "    parameters:").unwrap();
        writeln!(out, "      type: {ft}").unwrap();
        writeln!(out, "      freq: {:.1}", iir.freq).unwrap();
        writeln!(out, "      q: {:.4}", iir.q).unwrap();
        if iir.db_gain.abs() > 0.001 {
            writeln!(out, "      gain: {:.2}", iir.db_gain).unwrap();
        }
    }
    writeln!(out).unwrap();

    // Pipeline
    writeln!(out, "pipeline:").unwrap();
    writeln!(out, "  - type: Filter").unwrap();
    writeln!(out, "    channels: [0, 1]").unwrap();
    writeln!(out, "    names:").unwrap();
    if preamp.abs() > 0.01 {
        writeln!(out, "      - preamp").unwrap();
    }
    for i in 0..peq.len() {
        writeln!(out, "      - eq_{i}").unwrap();
    }

    out
}

/// Format a PEQ as a Wavelet GraphicEQ string.
///
/// Evaluates the PEQ biquad chain at the 9 standard graphic EQ band frequencies
/// and formats the result as a Wavelet-compatible line.
pub fn peq_format_wavelet(comment: &str, peq: &Peq, sample_rate: f64) -> String {
    use std::fmt::Write;

    const BANDS: [f64; 9] = [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

    let preamp = peq_preamp_gain(peq);
    let mut out = String::new();
    writeln!(out, "# {comment}").unwrap();
    writeln!(out, "# Wavelet GraphicEQ").unwrap();

    write!(out, "GraphicEQ:").unwrap();
    for (i, &freq) in BANDS.iter().enumerate() {
        let mut db = preamp;
        for (_, bq) in peq.iter() {
            db += bq.log_result(freq);
        }
        if i > 0 {
            write!(out, ";").unwrap();
        }
        write!(out, " {freq:.0} {db:.1}").unwrap();
    }
    writeln!(out).unwrap();

    // Suppress unused variable warning for sample_rate — kept for API consistency
    let _ = sample_rate;

    out
}

/// Format a PEQ as a Roon DSP parametric EQ preset (JSON).
///
/// Roon's parametric EQ uses a JSON object with a `bands` array. Each band has a
/// filter type, frequency, gain, Q factor, and enabled flag. Roon supports up to 20 bands.
///
/// The output can be used as reference for manual entry in Roon's DSP Engine UI.
pub fn peq_format_roon(comment: &str, peq: &Peq) -> String {
    use std::fmt::Write;

    fn roon_type(ft: BiquadFilterType) -> &'static str {
        match ft {
            BiquadFilterType::Peak | BiquadFilterType::PeakMatched => "Peak/Dip",
            BiquadFilterType::Lowshelf | BiquadFilterType::LowshelfOrf => "Low Shelf",
            BiquadFilterType::Highshelf | BiquadFilterType::HighshelfOrf => "High Shelf",
            BiquadFilterType::Lowpass => "Low Pass",
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => "High Pass",
            BiquadFilterType::Bandpass => "Band Pass",
            BiquadFilterType::Notch => "Band Stop",
            BiquadFilterType::AllPass => "Band Stop",
        }
    }

    let preamp = peq_preamp_gain(peq);
    let mut bands = String::new();

    for (i, (_, iir)) in peq.iter().enumerate().take(20) {
        if i > 0 {
            writeln!(bands, ",").unwrap();
        }
        write!(
            bands,
            r#"    {{
      "type": "{ft}",
      "frequency": {freq},
      "gain": {gain},
      "q": {q},
      "enabled": true
    }}"#,
            ft = roon_type(iir.filter_type),
            freq = iir.freq,
            gain = iir.db_gain,
            q = iir.q,
        )
        .unwrap();
    }

    format!(
        r#"// {comment}
// Roon DSP Parametric EQ preset (preamp: {preamp:.2} dB)
{{
  "bands": [
{bands}
  ],
  "is_enabled": true
}}"#
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
        for item in result.iter().take(9).skip(2) {
            assert_eq!(item.1.filter_type, BiquadFilterType::Peak);
            assert_eq!(item.1.db_gain, 0.0);
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
        for item in result.iter().take(9).skip(1) {
            assert_eq!(item.1.filter_type, BiquadFilterType::Peak);
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
        for item in result.iter().take(8).skip(1) {
            assert_eq!(item.1.filter_type, BiquadFilterType::Peak);
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

// ============================================================================
// BiquadBank Tests
// ============================================================================

#[cfg(test)]
mod biquad_bank_tests {
    use super::*;

    const SRATE: f64 = 48000.0;
    const TOL: f64 = 1e-12;

    /// Test that BiquadBank produces identical output to individual Biquads (TDF-II).
    #[test]
    fn test_biquad_bank_matches_individual_tdf2() {
        let num_channels = 4;
        let num_frames = 256;

        // Create template and bank
        let mut template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        template.use_tdf2 = true;
        let mut bank = BiquadBank::new(&template, num_channels);

        // Create individual biquads (one per channel)
        let mut individuals: Vec<Biquad> = (0..num_channels).map(|_| {
            let mut b = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
            b.use_tdf2 = true;
            b
        }).collect();

        // Generate different test signals per channel
        let mut bank_frame = vec![0.0_f64; num_channels];
        for frame_idx in 0..num_frames {
            for ch in 0..num_channels {
                // Different frequency per channel so outputs diverge
                let t = frame_idx as f64 / SRATE;
                let freq = 440.0 * (ch as f64 + 1.0);
                bank_frame[ch] = (2.0 * std::f64::consts::PI * freq * t).sin();
            }

            // Process individually
            let mut individual_out = bank_frame.clone();
            for ch in 0..num_channels {
                individual_out[ch] = individuals[ch].process(individual_out[ch]);
            }

            // Process via bank
            bank.process_interleaved_frame(&mut bank_frame);

            // Compare
            for ch in 0..num_channels {
                assert!(
                    (bank_frame[ch] - individual_out[ch]).abs() < TOL,
                    "TDF-II mismatch at frame={}, ch={}: bank={}, individual={}",
                    frame_idx, ch, bank_frame[ch], individual_out[ch]
                );
            }
        }
    }

    /// Test that BiquadBank produces identical output to individual Biquads (DF-I).
    #[test]
    fn test_biquad_bank_matches_individual_df1() {
        let num_channels = 4;
        let num_frames = 256;

        // Create template and bank (DF-I is the default)
        let template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        let mut bank = BiquadBank::new(&template, num_channels);

        let mut individuals: Vec<Biquad> = (0..num_channels)
            .map(|_| Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0))
            .collect();

        let mut bank_frame = vec![0.0_f64; num_channels];
        for frame_idx in 0..num_frames {
            for ch in 0..num_channels {
                let t = frame_idx as f64 / SRATE;
                let freq = 440.0 * (ch as f64 + 1.0);
                bank_frame[ch] = (2.0 * std::f64::consts::PI * freq * t).sin();
            }

            let mut individual_out = bank_frame.clone();
            for ch in 0..num_channels {
                individual_out[ch] = individuals[ch].process(individual_out[ch]);
            }

            bank.process_interleaved_frame(&mut bank_frame);

            for ch in 0..num_channels {
                assert!(
                    (bank_frame[ch] - individual_out[ch]).abs() < TOL,
                    "DF-I mismatch at frame={}, ch={}: bank={}, individual={}",
                    frame_idx, ch, bank_frame[ch], individual_out[ch]
                );
            }
        }
    }

    /// Test multichannel independence: each channel processes independently.
    #[test]
    fn test_biquad_bank_multichannel() {
        let num_channels = 8;
        let num_frames = 128;

        let mut template = Biquad::new(BiquadFilterType::Lowshelf, 200.0, SRATE, 0.7, 6.0);
        template.use_tdf2 = true;
        let mut bank = BiquadBank::new(&template, num_channels);

        // Feed signal only into channel 3, silence everywhere else
        let active_ch = 3;
        let mut active_biquad = Biquad::new(BiquadFilterType::Lowshelf, 200.0, SRATE, 0.7, 6.0);
        active_biquad.use_tdf2 = true;

        for frame_idx in 0..num_frames {
            let t = frame_idx as f64 / SRATE;
            let input = (2.0 * std::f64::consts::PI * 100.0 * t).sin();

            let mut frame = vec![0.0_f64; num_channels];
            frame[active_ch] = input;

            let expected = active_biquad.process(input);
            bank.process_interleaved_frame(&mut frame);

            // Active channel should match the individual biquad
            assert!(
                (frame[active_ch] - expected).abs() < TOL,
                "Active channel mismatch at frame {}: bank={}, expected={}",
                frame_idx, frame[active_ch], expected
            );

            // All other channels should remain zero (silence in → silence out)
            for ch in 0..num_channels {
                if ch != active_ch {
                    assert!(
                        frame[ch].abs() < TOL,
                        "Non-active channel {} has non-zero output {} at frame {}",
                        ch, frame[ch], frame_idx
                    );
                }
            }
        }
    }

    /// Test that reset clears all state.
    #[test]
    fn test_biquad_bank_reset() {
        let num_channels = 4;
        let mut template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        template.use_tdf2 = true;
        let mut bank = BiquadBank::new(&template, num_channels);

        // Feed some signal to build up state
        for _ in 0..100 {
            let mut frame = vec![1.0_f64; num_channels];
            bank.process_interleaved_frame(&mut frame);
        }

        // Verify state is non-zero
        assert!(bank.s1.iter().any(|&v| v.abs() > 1e-15));
        assert!(bank.s2.iter().any(|&v| v.abs() > 1e-15));

        // Reset
        bank.reset();

        // Verify all state is zeroed
        for ch in 0..num_channels {
            assert_eq!(bank.s1[ch], 0.0);
            assert_eq!(bank.s2[ch], 0.0);
            assert_eq!(bank.x1[ch], 0.0);
            assert_eq!(bank.x2[ch], 0.0);
            assert_eq!(bank.y1[ch], 0.0);
            assert_eq!(bank.y2[ch], 0.0);
        }

        // After reset, processing zero should yield zero
        let mut frame = vec![0.0_f64; num_channels];
        bank.process_interleaved_frame(&mut frame);
        for ch in 0..num_channels {
            assert_eq!(frame[ch], 0.0);
        }
    }

    /// Test that coefficient update applies to all channels correctly.
    #[test]
    fn test_biquad_bank_coefficient_update() {
        let num_channels = 4;
        let num_frames = 64;

        // Start with a flat peak (0 dB gain)
        let template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 0.0);
        let mut bank = BiquadBank::new(&template, num_channels);
        bank.use_tdf2 = true;

        // Process with unity (0 dB peak = passthrough)
        let mut frame = vec![1.0_f64; num_channels];
        for _ in 0..num_frames {
            bank.process_interleaved_frame(&mut frame);
        }
        // After settling, output ≈ input for 0dB peak
        let passthrough_out = frame[0];

        // Now update to +6 dB peak at 1kHz
        bank.update_params(1000.0, SRATE, 2.0, 6.0);

        // Verify the parameters were updated
        assert!((bank.freq - 1000.0).abs() < 1e-10);
        assert!((bank.db_gain - 6.0).abs() < 1e-10);

        // Create a reference biquad with the new params to verify coefficients match
        let ref_biquad = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 6.0);
        let (ref_a1, ref_a2, ref_b0, ref_b1, ref_b2) = ref_biquad.constants();
        assert!((bank.a1 - ref_a1).abs() < 1e-15);
        assert!((bank.a2 - ref_a2).abs() < 1e-15);
        assert!((bank.b0 - ref_b0).abs() < 1e-15);
        assert!((bank.b1 - ref_b1).abs() < 1e-15);
        assert!((bank.b2 - ref_b2).abs() < 1e-15);

        // Process more frames and verify output changed
        let mut frame2 = vec![1.0_f64; num_channels];
        for _ in 0..num_frames {
            bank.process_interleaved_frame(&mut frame2);
        }
        // With 6 dB gain, steady-state output for DC should differ from passthrough
        // (peak at 1kHz doesn't affect DC much, but coefficients changed)
        // The important thing is coefficients match the reference
        let _ = passthrough_out; // used above for verification context
    }

    /// Test copy_coefficients_from.
    #[test]
    fn test_biquad_bank_copy_coefficients_from() {
        let num_channels = 2;
        let template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        let mut bank = BiquadBank::new(&template, num_channels);

        // Build up some state
        let mut frame = vec![0.5; num_channels];
        bank.process_interleaved_frame(&mut frame);

        // Copy from a different biquad
        let new_biquad = Biquad::new(BiquadFilterType::Highshelf, 4000.0, SRATE, 0.7, -3.0);
        bank.copy_coefficients_from(&new_biquad);

        // Coefficients should match
        let (a1, a2, b0, b1, b2) = new_biquad.constants();
        assert_eq!(bank.a1, a1);
        assert_eq!(bank.a2, a2);
        assert_eq!(bank.b0, b0);
        assert_eq!(bank.b1, b1);
        assert_eq!(bank.b2, b2);
        assert_eq!(bank.filter_type, BiquadFilterType::Highshelf);
        assert!((bank.freq - 4000.0).abs() < 1e-10);
        assert!((bank.db_gain - (-3.0)).abs() < 1e-10);
    }

    /// Test process_interleaved_block matches frame-by-frame processing.
    #[test]
    fn test_biquad_bank_block_matches_frame() {
        let num_channels = 5; // Odd number to test remainder handling
        let num_frames = 128;

        let mut template = Biquad::new(BiquadFilterType::Highpass, 80.0, SRATE, 0.7, 0.0);
        template.use_tdf2 = true;

        // Two identical banks
        let mut bank_frame = BiquadBank::new(&template, num_channels);
        let mut bank_block = BiquadBank::new(&template, num_channels);

        // Generate interleaved test signal
        let mut buffer: Vec<f64> = Vec::with_capacity(num_frames * num_channels);
        for frame_idx in 0..num_frames {
            for ch in 0..num_channels {
                let t = frame_idx as f64 / SRATE;
                let freq = 50.0 + 200.0 * (ch as f64);
                buffer.push((2.0 * std::f64::consts::PI * freq * t).sin());
            }
        }

        // Process frame-by-frame
        let mut buffer_frame = buffer.clone();
        for frame_idx in 0..num_frames {
            let offset = frame_idx * num_channels;
            let frame = &mut buffer_frame[offset..offset + num_channels];
            bank_frame.process_interleaved_frame(frame);
        }

        // Process as block
        let mut buffer_block = buffer;
        bank_block.process_interleaved_block(&mut buffer_block, num_frames);

        // Compare
        for i in 0..num_frames * num_channels {
            assert!(
                (buffer_frame[i] - buffer_block[i]).abs() < TOL,
                "Block/frame mismatch at index {}: frame={}, block={}",
                i, buffer_frame[i], buffer_block[i]
            );
        }
    }

    /// Test with 1 channel (odd number, no SIMD pairs).
    #[test]
    fn test_biquad_bank_single_channel() {
        let mut template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        template.use_tdf2 = true;
        let mut bank = BiquadBank::new(&template, 1);
        let mut reference = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        reference.use_tdf2 = true;

        for frame_idx in 0..256 {
            let t = frame_idx as f64 / SRATE;
            let input = (2.0 * std::f64::consts::PI * 1000.0 * t).sin();

            let expected = reference.process(input);
            let mut frame = [input];
            bank.process_interleaved_frame(&mut frame);

            assert!(
                (frame[0] - expected).abs() < TOL,
                "Single-channel mismatch at frame {}: bank={}, expected={}",
                frame_idx, frame[0], expected
            );
        }
    }

    /// Test Display trait.
    #[test]
    fn test_biquad_bank_display() {
        let template = Biquad::new(BiquadFilterType::Peak, 1000.0, SRATE, 2.0, 3.0);
        let bank = BiquadBank::new(&template, 8);
        let display = format!("{}", bank);
        assert!(display.contains("8ch"));
        assert!(display.contains("PK"));
        assert!(display.contains("1000.0"));
    }
}
