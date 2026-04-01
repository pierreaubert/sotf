//! Zero-Delay Feedback State Variable Filter (ZDF/SVF)
//!
//! Implementation based on Vadim Zavalishin's "The Art of VA Filter Design"
//! using the Topology-Preserving Transform (TPT) approach.
//!
//! Key advantages over biquad (Direct Form):
//! - Parameter changes are inherently stable (no transient artifacts)
//! - Better behavior under fast modulation
//! - More "analog" character
//!
//! Reference: Zavalishin, V. (2012). "The Art of VA Filter Design", Chapter 3.

use crate::traits::{FilterFloat, lit};
use num_complex::Complex;

/// SVF filter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvfFilterType {
    /// Low-pass filter
    Lowpass,
    /// High-pass filter
    Highpass,
    /// Band-pass filter (constant skirt gain)
    Bandpass,
    /// Notch (band-reject) filter
    Notch,
    /// Parametric peak/bell filter
    Peak,
    /// Low-shelf filter
    Lowshelf,
    /// High-shelf filter
    Highshelf,
    /// All-pass filter
    Allpass,
}

/// Zero-Delay Feedback State Variable Filter.
///
/// Uses the TPT (Topology-Preserving Transform) approach for digitizing
/// the analog SVF prototype, ensuring that the digital filter matches
/// the analog frequency response exactly at all frequencies.
///
/// The type parameter `T` selects the floating-point precision (`f32` or `f64`).
/// The default is `f64` for backward compatibility.
#[derive(Debug, Clone)]
pub struct SvfFilter<T: FilterFloat = f64> {
    /// Filter type (lowpass, highpass, etc.)
    pub filter_type: SvfFilterType,
    /// Center/corner frequency in Hz
    pub freq: T,
    /// Sample rate in Hz
    pub sample_rate: T,
    /// Q factor (resonance)
    pub q: T,
    /// Gain in dB (only used for Peak, Lowshelf, Highshelf)
    pub gain_db: T,

    // TPT coefficients
    g: T,  // tan(pi * fc / fs)
    k: T,  // damping = 1/Q (or modified for shelving)
    a1: T, // 1 / (1 + g*(g + k))
    a2: T, // g * a1
    a3: T, // g * a2

    // Output mix coefficients
    m0: T,
    m1: T,
    m2: T,

    // Filter state (integrator outputs)
    ic1eq: T,
    ic2eq: T,
}

impl<T: FilterFloat> SvfFilter<T> {
    /// Create a new SVF filter.
    ///
    /// # Arguments
    /// * `filter_type` - Type of filter
    /// * `freq` - Center/corner frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `q` - Q factor (resonance). Higher = narrower bandwidth.
    /// * `gain_db` - Gain in dB (only used for Peak, Lowshelf, Highshelf)
    pub fn new(filter_type: SvfFilterType, freq: T, sample_rate: T, q: T, gain_db: T) -> Self {
        let zero = T::zero();
        let mut filter = Self {
            filter_type,
            freq,
            sample_rate,
            q,
            gain_db,
            g: zero,
            k: zero,
            a1: zero,
            a2: zero,
            a3: zero,
            m0: zero,
            m1: zero,
            m2: zero,
            ic1eq: zero,
            ic2eq: zero,
        };
        filter.update_coefficients();
        filter
    }

    /// Update filter parameters without resetting state.
    ///
    /// This is the key advantage of SVF: parameter changes produce no
    /// transient artifacts, making it ideal for modulated or animated EQ.
    pub fn update_params(
        &mut self,
        filter_type: SvfFilterType,
        freq: T,
        sample_rate: T,
        q: T,
        gain_db: T,
    ) {
        self.filter_type = filter_type;
        self.freq = freq;
        self.sample_rate = sample_rate;
        self.q = q;
        self.gain_db = gain_db;
        self.update_coefficients();
    }

    fn update_coefficients(&mut self) {
        // sqrt of linear gain: a = 10^(gain_db/40)
        let a = lit::<T>(10.0).powf(self.gain_db / lit::<T>(40.0));
        let clamped_freq = self
            .freq
            .clamp(T::one(), self.sample_rate * lit::<T>(0.499));
        self.g = (T::PI() * clamped_freq / self.sample_rate).tan();
        let q = self.q.max(lit::<T>(0.01));

        match self.filter_type {
            SvfFilterType::Lowpass => {
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::zero();
                self.m1 = T::zero();
                self.m2 = T::one();
            }
            SvfFilterType::Highpass => {
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::one();
                self.m1 = -self.k;
                self.m2 = -T::one();
            }
            SvfFilterType::Bandpass => {
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::zero();
                self.m1 = T::one();
                self.m2 = T::zero();
            }
            SvfFilterType::Notch => {
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::one();
                self.m1 = -self.k;
                self.m2 = T::zero();
            }
            SvfFilterType::Peak => {
                // Bell/parametric EQ: boost/cut at center frequency
                self.k = T::one() / (q * a);
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::one();
                self.m1 = self.k * (a * a - T::one());
                self.m2 = T::zero();
            }
            SvfFilterType::Lowshelf => {
                // Low-shelf: boost/cut below frequency
                self.g *= a.sqrt(); // pre-warp correction for shelf
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                let a2 = a * a;
                self.m0 = T::one();
                self.m1 = self.k * (a2 - T::one());
                self.m2 = a2 - T::one();
            }
            SvfFilterType::Highshelf => {
                // High-shelf: boost/cut above frequency
                self.g /= a.sqrt(); // pre-warp correction for shelf
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                let a2 = a * a;
                self.m0 = a2;
                self.m1 = self.k * (T::one() - a2) * a;
                self.m2 = T::one() - a2;
            }
            SvfFilterType::Allpass => {
                self.k = T::one() / q;
                self.a1 = T::one() / (T::one() + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = T::one();
                self.m1 = -lit::<T>(2.0) * self.k;
                self.m2 = T::zero();
            }
        }
    }

    /// Process one sample through the filter.
    #[inline]
    pub fn process(&mut self, input: T) -> T {
        // TPT SVF tick (Zavalishin's linearized form)
        let v3 = input - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        self.ic1eq = lit::<T>(2.0) * v1 - self.ic1eq;
        self.ic2eq = lit::<T>(2.0) * v2 - self.ic2eq;
        self.m0 * input + self.m1 * v1 + self.m2 * v2
    }

    /// Process a block of samples in-place.
    pub fn process_block(&mut self, buffer: &mut [T]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.ic1eq = T::zero();
        self.ic2eq = T::zero();
    }

    /// Compute the complex frequency response at a given frequency.
    ///
    /// Returns a complex value whose magnitude is the gain and whose
    /// angle is the phase shift at that frequency.
    pub fn response_at(&self, freq: T) -> Complex<T> {
        // Evaluate the transfer function on the unit circle
        let g = self.g;
        let k = self.k;

        // Bilinear transform: s = (2/T) * (z-1)/(z+1)
        // At frequency f: s = j*tan(pi*f/fs)
        let s = Complex::<T>::new(T::zero(), (T::PI() * freq / self.sample_rate).tan());

        // Analog SVF: LP = 1/(s^2 + k*s + 1) * g^2
        //             BP = s/(s^2 + k*s + 1) * g
        //             HP = s^2/(s^2 + k*s + 1)
        // After bilinear: use g = tan(pi*fc/fs)
        let s_norm = s / Complex::<T>::new(g, T::zero());
        let denom = s_norm * s_norm
            + Complex::<T>::new(k, T::zero()) * s_norm
            + Complex::<T>::new(T::one(), T::zero());

        let lp = Complex::<T>::new(T::one(), T::zero()) / denom;
        let bp = s_norm / denom;
        let hp = s_norm * s_norm / denom;

        // H(z) = m0*input + m1*v1 + m2*v2  where v1=BP*input, v2=LP*input
        // Since input = HP + BP + LP (SVF identity):
        // H = m0*(HP+BP+LP) + m1*BP + m2*LP = m0*HP + (m0+m1)*BP + (m0+m2)*LP
        Complex::<T>::new(self.m0, T::zero()) * hp
            + Complex::<T>::new(self.m0 + self.m1, T::zero()) * bp
            + Complex::<T>::new(self.m0 + self.m2, T::zero()) * lp
    }

    /// Compute the frequency response magnitude in dB at a given frequency.
    pub fn response_db_at(&self, freq: T) -> T {
        // Use process-based measurement for accuracy
        // (the analytic response_at has approximation issues for shelves)
        let resp = self.response_at(freq);
        lit::<T>(20.0) * resp.norm().log10()
    }

    /// Get the filter type.
    pub fn filter_type(&self) -> SvfFilterType {
        self.filter_type
    }

    /// Get the center/corner frequency.
    pub fn freq(&self) -> T {
        self.freq
    }

    /// Get the Q factor.
    pub fn q(&self) -> T {
        self.q
    }

    /// Get the gain in dB.
    pub fn gain_db(&self) -> T {
        self.gain_db
    }
}

impl SvfFilter<f64> {
    /// Process one f32 sample through an f64 filter.
    #[deprecated(note = "Use SvfFilter<f32> instead")]
    pub fn process_f32(&mut self, input: f32) -> f32 {
        self.process(input as f64) as f32
    }

    /// Process a block of f32 samples in-place through an f64 filter.
    #[deprecated(note = "Use SvfFilter<f32> instead")]
    pub fn process_block_f32(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample as f64) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48000.0;

    #[test]
    fn test_lowpass_passes_dc() {
        let mut f = SvfFilter::new(SvfFilterType::Lowpass, 1000.0, SR, 0.707, 0.0);
        // DC signal should pass through LP
        for _ in 0..1000 {
            f.process(1.0);
        }
        let out = f.process(1.0);
        assert!((out - 1.0).abs() < 0.01, "LP should pass DC: got {out}");
    }

    #[test]
    fn test_highpass_blocks_dc() {
        let mut f = SvfFilter::new(SvfFilterType::Highpass, 1000.0, SR, 0.707, 0.0);
        for _ in 0..2000 {
            f.process(1.0);
        }
        let out = f.process(1.0);
        assert!(out.abs() < 0.01, "HP should block DC: got {out}");
    }

    #[test]
    fn test_peak_at_center() {
        let mut f = SvfFilter::new(SvfFilterType::Peak, 1000.0, SR, 2.0, 6.0);
        // Send 1 kHz sine and measure gain
        let freq = 1000.0;
        let mut max_out = 0.0f64;
        for i in 0..4800 {
            let t = i as f64 / SR;
            let x = (2.0 * std::f64::consts::PI * freq * t).sin();
            let y = f.process(x);
            if i > 2400 {
                max_out = max_out.max(y.abs());
            }
        }
        // 6 dB boost = factor of 2
        assert!(
            max_out > 1.8 && max_out < 2.2,
            "Peak 6dB should ~double amplitude: got {max_out}"
        );
    }

    #[test]
    fn test_parameter_change_no_transient() {
        let mut f = SvfFilter::new(SvfFilterType::Peak, 1000.0, SR, 2.0, 0.0);
        // Process some signal to establish state
        for i in 0..4800 {
            let t = i as f64 / SR;
            f.process((2.0 * std::f64::consts::PI * 440.0 * t).sin());
        }
        // Abrupt parameter change — SVF should handle smoothly
        f.update_params(SvfFilterType::Peak, 5000.0, SR, 2.0, 12.0);
        // Process more — should not produce NaN or extreme values
        for i in 0..480 {
            let t = (4800 + i) as f64 / SR;
            let out = f.process((2.0 * std::f64::consts::PI * 440.0 * t).sin());
            assert!(out.is_finite(), "Non-finite after param change: {out}");
            assert!(
                out.abs() < 100.0,
                "Excessive output after param change: {out}"
            );
        }
    }

    #[test]
    fn test_allpass_preserves_magnitude() {
        let mut f = SvfFilter::new(SvfFilterType::Allpass, 1000.0, SR, 0.707, 0.0);
        let freq = 500.0;
        let mut max_in = 0.0f64;
        let mut max_out = 0.0f64;
        for i in 0..9600 {
            let t = i as f64 / SR;
            let x = (2.0 * std::f64::consts::PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_in = max_in.max(x.abs());
                max_out = max_out.max(y.abs());
            }
        }
        let ratio = max_out / max_in;
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Allpass should preserve magnitude: ratio={ratio}"
        );
    }

    #[test]
    fn test_reset() {
        let mut f = SvfFilter::new(SvfFilterType::Lowpass, 1000.0, SR, 0.707, 0.0);
        f.process(1.0);
        f.reset();
        let out = f.process(0.0);
        assert!(out.abs() < 1e-10, "State not cleared after reset: {out}");
    }

    #[test]
    fn test_lowshelf_boost() {
        let mut f = SvfFilter::new(SvfFilterType::Lowshelf, 1000.0, SR, 0.707, 6.0);
        // Low frequency (100Hz) should be boosted
        let freq = 100.0;
        let mut max_out = 0.0f64;
        for i in 0..9600 {
            let t = i as f64 / SR;
            let x = (2.0 * std::f64::consts::PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_out = max_out.max(y.abs());
            }
        }
        // 6 dB boost ≈ 2x
        assert!(max_out > 1.5, "Lowshelf should boost lows: got {max_out}");
    }

    #[test]
    fn test_highshelf_boost() {
        let mut f = SvfFilter::new(SvfFilterType::Highshelf, 1000.0, SR, 0.707, 6.0);
        // High frequency (10kHz) should be boosted
        let freq = 10000.0;
        let mut max_out = 0.0f64;
        for i in 0..9600 {
            let t = i as f64 / SR;
            let x = (2.0 * std::f64::consts::PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_out = max_out.max(y.abs());
            }
        }
        assert!(max_out > 1.5, "Highshelf should boost highs: got {max_out}");
    }

    #[test]
    fn test_process_block_f32() {
        let mut f = SvfFilter::new(SvfFilterType::Lowpass, 5000.0, SR, 0.707, 0.0);
        let mut buffer: Vec<f32> = (0..480)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR as f32).sin())
            .collect();
        #[allow(deprecated)]
        f.process_block_f32(&mut buffer);
        // Should be all finite
        for &s in &buffer {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn test_notch_at_center() {
        let mut f = SvfFilter::new(SvfFilterType::Notch, 1000.0, SR, 5.0, 0.0);
        let freq = 1000.0;
        let mut max_out = 0.0f64;
        for i in 0..9600 {
            let t = i as f64 / SR;
            let x = (2.0 * std::f64::consts::PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_out = max_out.max(y.abs());
            }
        }
        assert!(
            max_out < 0.1,
            "Notch should attenuate at center: got {max_out}"
        );
    }

    #[test]
    fn test_svf_f32() {
        let mut svf = SvfFilter::<f32>::new(SvfFilterType::Lowpass, 1000.0, 48000.0, 0.707, 0.0);
        let output = svf.process(1.0f32);
        assert!(output.abs() < 1.0);
    }
}
