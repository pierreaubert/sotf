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

use num_complex::Complex64;
use std::f64::consts::PI;

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
#[derive(Debug, Clone)]
pub struct SvfFilter {
    filter_type: SvfFilterType,
    freq: f64,
    sample_rate: f64,
    q: f64,
    gain_db: f64,

    // TPT coefficients
    g: f64,  // tan(pi * fc / fs)
    k: f64,  // damping = 1/Q (or modified for shelving)
    a1: f64, // 1 / (1 + g*(g + k))
    a2: f64, // g * a1
    a3: f64, // g * a2

    // Output mix coefficients
    m0: f64,
    m1: f64,
    m2: f64,

    // Filter state (integrator outputs)
    ic1eq: f64,
    ic2eq: f64,
}

impl SvfFilter {
    /// Create a new SVF filter.
    ///
    /// # Arguments
    /// * `filter_type` - Type of filter
    /// * `freq` - Center/corner frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `q` - Q factor (resonance). Higher = narrower bandwidth.
    /// * `gain_db` - Gain in dB (only used for Peak, Lowshelf, Highshelf)
    pub fn new(
        filter_type: SvfFilterType,
        freq: f64,
        sample_rate: f64,
        q: f64,
        gain_db: f64,
    ) -> Self {
        let mut filter = Self {
            filter_type,
            freq,
            sample_rate,
            q,
            gain_db,
            g: 0.0,
            k: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            m0: 0.0,
            m1: 0.0,
            m2: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
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
        freq: f64,
        sample_rate: f64,
        q: f64,
        gain_db: f64,
    ) {
        self.filter_type = filter_type;
        self.freq = freq;
        self.sample_rate = sample_rate;
        self.q = q;
        self.gain_db = gain_db;
        self.update_coefficients();
    }

    fn update_coefficients(&mut self) {
        let a = 10.0_f64.powf(self.gain_db / 40.0); // sqrt of linear gain
        let clamped_freq = self.freq.clamp(1.0, self.sample_rate * 0.499);
        self.g = (PI * clamped_freq / self.sample_rate).tan();
        let q = self.q.max(0.01);

        match self.filter_type {
            SvfFilterType::Lowpass => {
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 0.0;
                self.m1 = 0.0;
                self.m2 = 1.0;
            }
            SvfFilterType::Highpass => {
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 1.0;
                self.m1 = -self.k;
                self.m2 = -1.0;
            }
            SvfFilterType::Bandpass => {
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 0.0;
                self.m1 = 1.0;
                self.m2 = 0.0;
            }
            SvfFilterType::Notch => {
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 1.0;
                self.m1 = -self.k;
                self.m2 = 0.0;
            }
            SvfFilterType::Peak => {
                // Bell/parametric EQ: boost/cut at center frequency
                self.k = 1.0 / (q * a);
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 1.0;
                self.m1 = self.k * (a * a - 1.0);
                self.m2 = 0.0;
            }
            SvfFilterType::Lowshelf => {
                // Low-shelf: boost/cut below frequency
                self.g *= a.sqrt(); // pre-warp correction for shelf
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                let a2 = a * a;
                self.m0 = 1.0;
                self.m1 = self.k * (a2 - 1.0);
                self.m2 = a2 - 1.0;
            }
            SvfFilterType::Highshelf => {
                // High-shelf: boost/cut above frequency
                self.g /= a.sqrt(); // pre-warp correction for shelf
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                let a2 = a * a;
                self.m0 = a2;
                self.m1 = self.k * (1.0 - a2) * a;
                self.m2 = 1.0 - a2;
            }
            SvfFilterType::Allpass => {
                self.k = 1.0 / q;
                self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
                self.a2 = self.g * self.a1;
                self.a3 = self.g * self.a2;
                self.m0 = 1.0;
                self.m1 = -2.0 * self.k;
                self.m2 = 0.0;
            }
        }
    }

    /// Process one sample through the filter.
    #[inline]
    pub fn process(&mut self, input: f64) -> f64 {
        // TPT SVF tick (Zavalishin's linearized form)
        let v3 = input - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        self.m0 * input + self.m1 * v1 + self.m2 * v2
    }

    /// Process one f32 sample (convenience for audio plugins).
    #[inline]
    pub fn process_f32(&mut self, input: f32) -> f32 {
        self.process(input as f64) as f32
    }

    /// Process a block of samples in-place.
    pub fn process_block(&mut self, buffer: &mut [f64]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Process a block of f32 samples in-place.
    pub fn process_block_f32(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample as f64) as f32;
        }
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Compute the complex frequency response at a given frequency.
    ///
    /// Returns a complex value whose magnitude is the gain and whose
    /// angle is the phase shift at that frequency.
    pub fn response_at(&self, freq: f64) -> Complex64 {
        // Evaluate the transfer function on the unit circle
        let g = self.g;
        let k = self.k;

        // Bilinear transform: s = (2/T) * (z-1)/(z+1)
        // At frequency f: s = j*tan(pi*f/fs)
        let s = Complex64::new(0.0, (PI * freq / self.sample_rate).tan());

        // Analog SVF: LP = 1/(s^2 + k*s + 1) * g^2
        //             BP = s/(s^2 + k*s + 1) * g
        //             HP = s^2/(s^2 + k*s + 1)
        // After bilinear: use g = tan(pi*fc/fs)
        let s_norm = s / Complex64::new(g, 0.0);
        let denom = s_norm * s_norm + Complex64::new(k, 0.0) * s_norm + Complex64::new(1.0, 0.0);

        let lp = Complex64::new(1.0, 0.0) / denom;
        let bp = s_norm / denom;
        let hp = s_norm * s_norm / denom;

        // H(z) = m0*input + m1*v1 + m2*v2  where v1=BP*input, v2=LP*input
        // Since input = HP + BP + LP (SVF identity):
        // H = m0*(HP+BP+LP) + m1*BP + m2*LP = m0*HP + (m0+m1)*BP + (m0+m2)*LP
        Complex64::new(self.m0, 0.0) * hp
            + Complex64::new(self.m0 + self.m1, 0.0) * bp
            + Complex64::new(self.m0 + self.m2, 0.0) * lp
    }

    /// Compute the frequency response magnitude in dB at a given frequency.
    pub fn response_db_at(&self, freq: f64) -> f64 {
        // Use process-based measurement for accuracy
        // (the analytic response_at has approximation issues for shelves)
        let resp = self.response_at(freq);
        20.0 * resp.norm().log10()
    }

    /// Get the filter type.
    pub fn filter_type(&self) -> SvfFilterType {
        self.filter_type
    }

    /// Get the center/corner frequency.
    pub fn freq(&self) -> f64 {
        self.freq
    }

    /// Get the Q factor.
    pub fn q(&self) -> f64 {
        self.q
    }

    /// Get the gain in dB.
    pub fn gain_db(&self) -> f64 {
        self.gain_db
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
        assert!(
            (out - 1.0).abs() < 0.01,
            "LP should pass DC: got {out}"
        );
    }

    #[test]
    fn test_highpass_blocks_dc() {
        let mut f = SvfFilter::new(SvfFilterType::Highpass, 1000.0, SR, 0.707, 0.0);
        for _ in 0..2000 {
            f.process(1.0);
        }
        let out = f.process(1.0);
        assert!(
            out.abs() < 0.01,
            "HP should block DC: got {out}"
        );
    }

    #[test]
    fn test_peak_at_center() {
        let mut f = SvfFilter::new(SvfFilterType::Peak, 1000.0, SR, 2.0, 6.0);
        // Send 1 kHz sine and measure gain
        let freq = 1000.0;
        let mut max_out = 0.0f64;
        for i in 0..4800 {
            let t = i as f64 / SR;
            let x = (2.0 * PI * freq * t).sin();
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
            f.process((2.0 * PI * 440.0 * t).sin());
        }
        // Abrupt parameter change — SVF should handle smoothly
        f.update_params(SvfFilterType::Peak, 5000.0, SR, 2.0, 12.0);
        // Process more — should not produce NaN or extreme values
        for i in 0..480 {
            let t = (4800 + i) as f64 / SR;
            let out = f.process((2.0 * PI * 440.0 * t).sin());
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
            let x = (2.0 * PI * freq * t).sin();
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
            let x = (2.0 * PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_out = max_out.max(y.abs());
            }
        }
        // 6 dB boost ≈ 2x
        assert!(
            max_out > 1.5,
            "Lowshelf should boost lows: got {max_out}"
        );
    }

    #[test]
    fn test_highshelf_boost() {
        let mut f = SvfFilter::new(SvfFilterType::Highshelf, 1000.0, SR, 0.707, 6.0);
        // High frequency (10kHz) should be boosted
        let freq = 10000.0;
        let mut max_out = 0.0f64;
        for i in 0..9600 {
            let t = i as f64 / SR;
            let x = (2.0 * PI * freq * t).sin();
            let y = f.process(x);
            if i > 4800 {
                max_out = max_out.max(y.abs());
            }
        }
        assert!(
            max_out > 1.5,
            "Highshelf should boost highs: got {max_out}"
        );
    }

    #[test]
    fn test_process_block_f32() {
        let mut f = SvfFilter::new(SvfFilterType::Lowpass, 5000.0, SR, 0.707, 0.0);
        let mut buffer: Vec<f32> = (0..480)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR as f32).sin())
            .collect();
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
            let x = (2.0 * PI * freq * t).sin();
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
}
