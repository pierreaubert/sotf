//! FIR filter implementation with windowing functions

use ndarray::Array1;
use std::f64::consts::PI;
use std::fmt;

/// Window function types for FIR filter design
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WindowType {
    /// Rectangular window (no windowing)
    Rectangular,
    /// Hamming window
    Hamming,
    /// Hann (Hanning) window
    Hann,
    /// Blackman window
    Blackman,
    /// Kaiser window (requires beta parameter)
    Kaiser,
}

impl WindowType {
    /// Returns the short string representation of the window type.
    pub fn short_name(&self) -> &'static str {
        match self {
            WindowType::Rectangular => "RECT",
            WindowType::Hamming => "HAMM",
            WindowType::Hann => "HANN",
            WindowType::Blackman => "BLKM",
            WindowType::Kaiser => "KAIS",
        }
    }

    /// Returns the long string representation of the window type.
    pub fn long_name(&self) -> &'static str {
        match self {
            WindowType::Rectangular => "Rectangular",
            WindowType::Hamming => "Hamming",
            WindowType::Hann => "Hann",
            WindowType::Blackman => "Blackman",
            WindowType::Kaiser => "Kaiser",
        }
    }
}

/// FIR filter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FirFilterType {
    /// Low-pass filter
    Lowpass,
    /// High-pass filter
    Highpass,
    /// Band-pass filter
    Bandpass,
    /// Band-stop (notch) filter
    Bandstop,
    /// Custom filter (user-provided coefficients)
    Custom,
}

impl FirFilterType {
    /// Returns the short string representation of the filter type.
    pub fn short_name(&self) -> &'static str {
        match self {
            FirFilterType::Lowpass => "LP",
            FirFilterType::Highpass => "HP",
            FirFilterType::Bandpass => "BP",
            FirFilterType::Bandstop => "BS",
            FirFilterType::Custom => "CUSTOM",
        }
    }

    /// Returns the long string representation of the filter type.
    pub fn long_name(&self) -> &'static str {
        match self {
            FirFilterType::Lowpass => "Lowpass",
            FirFilterType::Highpass => "Highpass",
            FirFilterType::Bandpass => "Bandpass",
            FirFilterType::Bandstop => "Bandstop",
            FirFilterType::Custom => "Custom",
        }
    }
}

/// Represents a single FIR filter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fir {
    /// The type of filter
    pub filter_type: FirFilterType,
    /// Filter coefficients (taps)
    coeffs: Vec<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    /// Cutoff frequency (or lower cutoff for bandpass/bandstop) in Hz
    pub freq: f64,
    /// Upper cutoff frequency (for bandpass/bandstop) in Hz
    pub freq_upper: Option<f64>,
    /// Window type used
    pub window: WindowType,
    /// Kaiser window beta parameter (if applicable)
    pub kaiser_beta: f64,
    /// Circular buffer for filter state
    state: Vec<f64>,
    /// Current position in the circular buffer
    state_pos: usize,
}

impl Fir {
    /// Creates a new FIR filter with custom coefficients.
    ///
    /// # Arguments
    /// * `coeffs` - Filter coefficients (taps)
    /// * `srate` - Sample rate in Hz
    ///
    /// # Panics
    /// Panics in debug mode if:
    /// - `coeffs` is empty
    /// - `srate` is not positive
    pub fn new_custom(coeffs: Vec<f64>, srate: f64) -> Self {
        debug_assert!(!coeffs.is_empty(), "FIR filter must have at least one tap");
        debug_assert!(srate > 0.0, "Sample rate must be positive");

        let n_taps = coeffs.len();
        Fir {
            filter_type: FirFilterType::Custom,
            coeffs,
            srate,
            freq: 0.0,
            freq_upper: None,
            window: WindowType::Rectangular,
            kaiser_beta: 0.0,
            state: vec![0.0; n_taps],
            state_pos: 0,
        }
    }

    /// Creates a lowpass FIR filter using the windowed-sinc method.
    ///
    /// # Arguments
    /// * `n_taps` - Number of filter taps (must be odd)
    /// * `cutoff` - Cutoff frequency in Hz
    /// * `srate` - Sample rate in Hz
    /// * `window` - Window function to use
    /// * `kaiser_beta` - Beta parameter for Kaiser window (ignored for other windows)
    ///
    /// # Panics
    /// Panics in debug mode if:
    /// - `n_taps` is zero
    /// - `srate` is not positive
    /// - `cutoff` is not positive or >= Nyquist frequency (srate/2)
    pub fn lowpass(
        n_taps: usize,
        cutoff: f64,
        srate: f64,
        window: WindowType,
        kaiser_beta: f64,
    ) -> Self {
        debug_assert!(n_taps > 0, "Number of taps must be positive");
        debug_assert!(srate > 0.0, "Sample rate must be positive");
        debug_assert!(
            cutoff > 0.0 && cutoff < srate / 2.0,
            "Cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            cutoff
        );

        let coeffs = design_fir_lowpass(n_taps, cutoff, srate, window, kaiser_beta);
        let n = coeffs.len();
        Fir {
            filter_type: FirFilterType::Lowpass,
            coeffs,
            srate,
            freq: cutoff,
            freq_upper: None,
            window,
            kaiser_beta,
            state: vec![0.0; n],
            state_pos: 0,
        }
    }

    /// Creates a highpass FIR filter using spectral inversion of a lowpass filter.
    ///
    /// # Arguments
    /// * `n_taps` - Number of filter taps (must be odd)
    /// * `cutoff` - Cutoff frequency in Hz
    /// * `srate` - Sample rate in Hz
    /// * `window` - Window function to use
    /// * `kaiser_beta` - Beta parameter for Kaiser window (ignored for other windows)
    ///
    /// # Panics
    /// Panics in debug mode if:
    /// - `n_taps` is zero
    /// - `srate` is not positive
    /// - `cutoff` is not positive or >= Nyquist frequency (srate/2)
    pub fn highpass(
        n_taps: usize,
        cutoff: f64,
        srate: f64,
        window: WindowType,
        kaiser_beta: f64,
    ) -> Self {
        debug_assert!(n_taps > 0, "Number of taps must be positive");
        debug_assert!(srate > 0.0, "Sample rate must be positive");
        debug_assert!(
            cutoff > 0.0 && cutoff < srate / 2.0,
            "Cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            cutoff
        );

        let coeffs = design_fir_highpass(n_taps, cutoff, srate, window, kaiser_beta);
        let n = coeffs.len();
        Fir {
            filter_type: FirFilterType::Highpass,
            coeffs,
            srate,
            freq: cutoff,
            freq_upper: None,
            window,
            kaiser_beta,
            state: vec![0.0; n],
            state_pos: 0,
        }
    }

    /// Creates a bandpass FIR filter by multiplying two sinc functions.
    ///
    /// # Arguments
    /// * `n_taps` - Number of filter taps (must be odd)
    /// * `freq_low` - Lower cutoff frequency in Hz
    /// * `freq_high` - Upper cutoff frequency in Hz
    /// * `srate` - Sample rate in Hz
    /// * `window` - Window function to use
    /// * `kaiser_beta` - Beta parameter for Kaiser window (ignored for other windows)
    ///
    /// # Panics
    /// Panics in debug mode if:
    /// - `n_taps` is zero
    /// - `srate` is not positive
    /// - `freq_low` is not positive or >= Nyquist frequency (srate/2)
    /// - `freq_high` is not positive or >= Nyquist frequency (srate/2)
    /// - `freq_low` >= `freq_high`
    pub fn bandpass(
        n_taps: usize,
        freq_low: f64,
        freq_high: f64,
        srate: f64,
        window: WindowType,
        kaiser_beta: f64,
    ) -> Self {
        debug_assert!(n_taps > 0, "Number of taps must be positive");
        debug_assert!(srate > 0.0, "Sample rate must be positive");
        debug_assert!(
            freq_low > 0.0 && freq_low < srate / 2.0,
            "Lower cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            freq_low
        );
        debug_assert!(
            freq_high > 0.0 && freq_high < srate / 2.0,
            "Upper cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            freq_high
        );
        debug_assert!(
            freq_low < freq_high,
            "Lower cutoff frequency ({}Hz) must be less than upper cutoff frequency ({}Hz)",
            freq_low,
            freq_high
        );

        let coeffs = design_fir_bandpass(n_taps, freq_low, freq_high, srate, window, kaiser_beta);
        let n = coeffs.len();
        Fir {
            filter_type: FirFilterType::Bandpass,
            coeffs,
            srate,
            freq: freq_low,
            freq_upper: Some(freq_high),
            window,
            kaiser_beta,
            state: vec![0.0; n],
            state_pos: 0,
        }
    }

    /// Creates a bandstop FIR filter using spectral inversion of a bandpass filter.
    ///
    /// # Arguments
    /// * `n_taps` - Number of filter taps (must be odd)
    /// * `freq_low` - Lower cutoff frequency in Hz
    /// * `freq_high` - Upper cutoff frequency in Hz
    /// * `srate` - Sample rate in Hz
    /// * `window` - Window function to use
    /// * `kaiser_beta` - Beta parameter for Kaiser window (ignored for other windows)
    ///
    /// # Panics
    /// Panics in debug mode if:
    /// - `n_taps` is zero
    /// - `srate` is not positive
    /// - `freq_low` is not positive or >= Nyquist frequency (srate/2)
    /// - `freq_high` is not positive or >= Nyquist frequency (srate/2)
    /// - `freq_low` >= `freq_high`
    pub fn bandstop(
        n_taps: usize,
        freq_low: f64,
        freq_high: f64,
        srate: f64,
        window: WindowType,
        kaiser_beta: f64,
    ) -> Self {
        debug_assert!(n_taps > 0, "Number of taps must be positive");
        debug_assert!(srate > 0.0, "Sample rate must be positive");
        debug_assert!(
            freq_low > 0.0 && freq_low < srate / 2.0,
            "Lower cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            freq_low
        );
        debug_assert!(
            freq_high > 0.0 && freq_high < srate / 2.0,
            "Upper cutoff frequency must be positive and below Nyquist ({}Hz), got {}Hz",
            srate / 2.0,
            freq_high
        );
        debug_assert!(
            freq_low < freq_high,
            "Lower cutoff frequency ({}Hz) must be less than upper cutoff frequency ({}Hz)",
            freq_low,
            freq_high
        );

        let coeffs = design_fir_bandstop(n_taps, freq_low, freq_high, srate, window, kaiser_beta);
        let n = coeffs.len();
        Fir {
            filter_type: FirFilterType::Bandstop,
            coeffs,
            srate,
            freq: freq_low,
            freq_upper: Some(freq_high),
            window,
            kaiser_beta,
            state: vec![0.0; n],
            state_pos: 0,
        }
    }

    /// Returns the number of filter taps (coefficients).
    pub fn n_taps(&self) -> usize {
        self.coeffs.len()
    }

    /// Returns a reference to the filter coefficients.
    pub fn coeffs(&self) -> &[f64] {
        &self.coeffs
    }

    /// Resets the filter state to zero.
    pub fn reset(&mut self) {
        self.state.fill(0.0);
        self.state_pos = 0;
    }

    /// Processes a single audio sample through the filter.
    pub fn process(&mut self, x: f64) -> f64 {
        // Store input sample in circular buffer
        self.state[self.state_pos] = x;

        // Compute output using convolution
        let mut y = 0.0;
        let n_taps = self.coeffs.len();
        for i in 0..n_taps {
            let state_idx = (self.state_pos + n_taps - i) % n_taps;
            y += self.coeffs[i] * self.state[state_idx];
        }

        // Update circular buffer position
        self.state_pos = (self.state_pos + 1) % n_taps;

        y
    }

    /// Processes a block of audio samples in-place.
    pub fn process_block(&mut self, samples: &mut [f64]) {
        let n_taps = self.coeffs.len();

        for sample in samples.iter_mut() {
            let x = *sample;
            // Store input sample in circular buffer
            self.state[self.state_pos] = x;

            // Compute output using convolution
            let mut y = 0.0;
            for i in 0..n_taps {
                let state_idx = (self.state_pos + n_taps - i) % n_taps;
                y += self.coeffs[i] * self.state[state_idx];
            }

            // Update circular buffer position
            self.state_pos = (self.state_pos + 1) % n_taps;

            *sample = y;
        }
    }

    /// Calculates the filter's magnitude response at a single frequency `f`.
    pub fn result(&self, f: f64) -> f64 {
        let omega = 2.0 * PI * f / self.srate;
        let mut real = 0.0;
        let mut imag = 0.0;

        for (n, &coeff) in self.coeffs.iter().enumerate() {
            let phase = -(n as f64) * omega;
            real += coeff * phase.cos();
            imag += coeff * phase.sin();
        }

        (real * real + imag * imag).sqrt()
    }

    /// Calculates the filter's response in dB at a single frequency `f`.
    pub fn log_result(&self, f: f64) -> f64 {
        let result = self.result(f);
        if result > 0.0 {
            20.0 * result.log10()
        } else {
            -200.0
        }
    }

    /// Vectorized version to compute the SPL response for a vector of frequencies.
    ///
    /// # Performance
    /// This implementation avoids per-tap allocations by using a direct nested loop.
    pub fn np_log_result(&self, freq: &Array1<f64>) -> Array1<f64> {
        let n_freqs = freq.len();
        let omega_base = 2.0 * PI / self.srate;

        let mut real: Array1<f64> = Array1::zeros(n_freqs);
        let mut imag: Array1<f64> = Array1::zeros(n_freqs);

        for (n, &coeff) in self.coeffs.iter().enumerate() {
            let n_f = -(n as f64);
            for i in 0..n_freqs {
                let phase = n_f * freq[i] * omega_base;
                real[i] += coeff * phase.cos();
                imag[i] += coeff * phase.sin();
            }
        }

        // Compute magnitude and convert to dB
        let mut magnitude = Array1::zeros(n_freqs);
        let min_val = 1.0e-20;

        for i in 0..n_freqs {
            let mag_sq = real[i] * real[i] + imag[i] * imag[i];
            let mag = mag_sq.sqrt().max(min_val);
            magnitude[i] = 20.0 * mag.log10();
        }

        magnitude
    }
}

impl fmt::Display for Fir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.filter_type {
            FirFilterType::Bandpass | FirFilterType::Bandstop => {
                write!(
                    f,
                    "Type:{},Freq:{:.1}-{:.1},Rate:{:.1},Taps:{},Window:{}",
                    self.filter_type.short_name(),
                    self.freq,
                    self.freq_upper.unwrap_or(0.0),
                    self.srate,
                    self.n_taps(),
                    self.window.short_name()
                )
            }
            _ => {
                write!(
                    f,
                    "Type:{},Freq:{:.1},Rate:{:.1},Taps:{},Window:{}",
                    self.filter_type.short_name(),
                    self.freq,
                    self.srate,
                    self.n_taps(),
                    self.window.short_name()
                )
            }
        }
    }
}

// Window functions

/// Modified Bessel function of the first kind (I0), used for Kaiser window.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let mut k = 1.0;

    let half_x_sq = (x / 2.0) * (x / 2.0);

    // Series expansion: I₀(x) = Σ ((x/2)^k / k!)²
    // Each term is: term_k = term_{k-1} * (x/2)² / k²
    loop {
        term *= half_x_sq / (k * k);
        sum += term;
        if term < 1e-12 * sum {
            break;
        }
        k += 1.0;
    }

    sum
}

/// Generates a window function for FIR filter design.
///
/// # Arguments
/// * `n` - Window length
/// * `window_type` - Type of window to generate
/// * `kaiser_beta` - Beta parameter for Kaiser window (ignored for other types)
///
/// # Returns
/// Vector of window coefficients
pub fn generate_window(n: usize, window_type: WindowType, kaiser_beta: f64) -> Vec<f64> {
    let mut window = vec![0.0; n];

    match window_type {
        WindowType::Rectangular => {
            window.fill(1.0);
        }
        WindowType::Hamming => {
            for (i, w) in window.iter_mut().enumerate() {
                *w = 0.54 - 0.46 * (2.0 * PI * i as f64 / (n - 1) as f64).cos();
            }
        }
        WindowType::Hann => {
            for (i, w) in window.iter_mut().enumerate() {
                *w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n - 1) as f64).cos());
            }
        }
        WindowType::Blackman => {
            for (i, w) in window.iter_mut().enumerate() {
                let arg = 2.0 * PI * i as f64 / (n - 1) as f64;
                *w = 0.42 - 0.5 * arg.cos() + 0.08 * (2.0 * arg).cos();
            }
        }
        WindowType::Kaiser => {
            let i0_beta = bessel_i0(kaiser_beta);
            let n_minus_1 = (n - 1) as f64;
            for (i, w) in window.iter_mut().enumerate() {
                let x =
                    kaiser_beta * (1.0 - ((2.0 * i as f64 - n_minus_1) / n_minus_1).powi(2)).sqrt();
                *w = bessel_i0(x) / i0_beta;
            }
        }
    }

    window
}

/// Designs a lowpass FIR filter using the windowed-sinc method.
fn design_fir_lowpass(
    n_taps: usize,
    cutoff: f64,
    srate: f64,
    window: WindowType,
    kaiser_beta: f64,
) -> Vec<f64> {
    // Ensure odd number of taps for symmetry
    let n = if n_taps.is_multiple_of(2) {
        n_taps + 1
    } else {
        n_taps
    };

    let mut h = vec![0.0; n];
    let fc = cutoff / srate; // Normalized cutoff frequency
    let m = (n - 1) as f64 / 2.0;

    // Generate ideal lowpass sinc function
    for (i, h_val) in h.iter_mut().enumerate() {
        let x = i as f64 - m;
        if x == 0.0 {
            *h_val = 2.0 * fc;
        } else {
            *h_val = (2.0 * PI * fc * x).sin() / (PI * x);
        }
    }

    // Apply window
    let window_coeffs = generate_window(n, window, kaiser_beta);
    for (i, h_val) in h.iter_mut().enumerate() {
        *h_val *= window_coeffs[i];
    }

    // Normalize to unit gain at DC
    let sum: f64 = h.iter().sum();
    if sum.abs() > 1e-10 {
        for h_val in h.iter_mut() {
            *h_val /= sum;
        }
    }

    h
}

/// Designs a highpass FIR filter using spectral inversion.
fn design_fir_highpass(
    n_taps: usize,
    cutoff: f64,
    srate: f64,
    window: WindowType,
    kaiser_beta: f64,
) -> Vec<f64> {
    // Start with lowpass filter
    let mut h = design_fir_lowpass(n_taps, cutoff, srate, window, kaiser_beta);

    // Spectral inversion: negate all coefficients and add 1 to center tap
    let m = h.len() / 2;
    for h_val in h.iter_mut() {
        *h_val = -*h_val;
    }
    h[m] += 1.0;

    h
}

/// Designs a bandpass FIR filter.
fn design_fir_bandpass(
    n_taps: usize,
    freq_low: f64,
    freq_high: f64,
    srate: f64,
    window: WindowType,
    kaiser_beta: f64,
) -> Vec<f64> {
    // Ensure odd number of taps
    let n = if n_taps.is_multiple_of(2) {
        n_taps + 1
    } else {
        n_taps
    };

    let mut h = vec![0.0; n];
    let fc_low = freq_low / srate;
    let fc_high = freq_high / srate;
    let m = (n - 1) as f64 / 2.0;

    // Generate ideal bandpass filter (difference of two sinc functions)
    for (i, h_val) in h.iter_mut().enumerate() {
        let x = i as f64 - m;
        if x == 0.0 {
            *h_val = 2.0 * (fc_high - fc_low);
        } else {
            let sinc_high = (2.0 * PI * fc_high * x).sin() / (PI * x);
            let sinc_low = (2.0 * PI * fc_low * x).sin() / (PI * x);
            *h_val = sinc_high - sinc_low;
        }
    }

    // Apply window
    let window_coeffs = generate_window(n, window, kaiser_beta);
    for (i, h_val) in h.iter_mut().enumerate() {
        *h_val *= window_coeffs[i];
    }

    h
}

/// Designs a bandstop FIR filter using spectral inversion.
fn design_fir_bandstop(
    n_taps: usize,
    freq_low: f64,
    freq_high: f64,
    srate: f64,
    window: WindowType,
    kaiser_beta: f64,
) -> Vec<f64> {
    // Start with bandpass filter
    let mut h = design_fir_bandpass(n_taps, freq_low, freq_high, srate, window, kaiser_beta);

    // Spectral inversion: negate all coefficients and add 1 to center tap
    let m = h.len() / 2;
    for h_val in h.iter_mut() {
        *h_val = -*h_val;
    }
    h[m] += 1.0;

    h
}

/// Type alias for a collection of weighted FIR filters
pub type FirBank = Vec<(f64, Fir)>;

/// Compute the combined FIR bank response (in dB) on a given frequency grid.
///
/// # Arguments
/// * `freqs` - Frequency points for evaluation (Hz)
/// * `fir_bank` - Collection of weighted FIR filters
///
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn compute_fir_bank_response(freqs: &Array1<f64>, fir_bank: &FirBank) -> Array1<f64> {
    if fir_bank.is_empty() {
        return Array1::zeros(freqs.len());
    }
    let mut response = Array1::zeros(freqs.len());
    for (weight, filter) in fir_bank {
        response += &(filter.np_log_result(freqs) * *weight);
    }
    response
}

/// Compute the FIR bank SPL response at given frequencies.
pub fn fir_bank_spl(freq: &Array1<f64>, fir_bank: &FirBank) -> Array1<f64> {
    compute_fir_bank_response(freq, fir_bank)
}

/// Calculate the recommended preamp gain to avoid clipping for a FIR bank.
///
/// This computes the maximum gain across the audible frequency range
/// and returns the negative of that value.
pub fn fir_bank_preamp_gain(fir_bank: &FirBank) -> f64 {
    if fir_bank.is_empty() {
        return 0.0;
    }

    // Sample frequencies across the audible range
    let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 500);
    let response = fir_bank_spl(&freqs, fir_bank);

    // Find maximum gain
    let max_gain = response.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    // Return negative of max gain (to reduce overall level)
    -max_gain
}
#[cfg(test)]
mod fir_tests {
    use super::*;
    use ndarray::array;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_fir_lowpass_creation() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        assert_eq!(fir.filter_type, FirFilterType::Lowpass);
        assert_eq!(fir.n_taps(), 51);
        assert_eq!(fir.freq, 1000.0);
        assert_eq!(fir.srate, 48000.0);
        assert_eq!(fir.window, WindowType::Hamming);
    }

    #[test]
    fn test_fir_highpass_creation() {
        let fir = Fir::highpass(51, 1000.0, 48000.0, WindowType::Hann, 0.0);
        assert_eq!(fir.filter_type, FirFilterType::Highpass);
        assert_eq!(fir.n_taps(), 51);
        assert_eq!(fir.freq, 1000.0);
    }

    #[test]
    fn test_fir_bandpass_creation() {
        let fir = Fir::bandpass(51, 500.0, 2000.0, 48000.0, WindowType::Blackman, 0.0);
        assert_eq!(fir.filter_type, FirFilterType::Bandpass);
        assert_eq!(fir.n_taps(), 51);
        assert_eq!(fir.freq, 500.0);
        assert_eq!(fir.freq_upper, Some(2000.0));
    }

    #[test]
    fn test_fir_bandstop_creation() {
        let fir = Fir::bandstop(51, 500.0, 2000.0, 48000.0, WindowType::Kaiser, 5.0);
        assert_eq!(fir.filter_type, FirFilterType::Bandstop);
        assert_eq!(fir.n_taps(), 51);
        assert_eq!(fir.kaiser_beta, 5.0);
    }

    #[test]
    fn test_fir_custom_creation() {
        let coeffs = vec![0.1, 0.2, 0.4, 0.2, 0.1];
        let fir = Fir::new_custom(coeffs.clone(), 48000.0);
        assert_eq!(fir.filter_type, FirFilterType::Custom);
        assert_eq!(fir.n_taps(), 5);
        assert_eq!(fir.coeffs(), &coeffs);
    }

    #[test]
    fn test_fir_reset() {
        let mut fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);

        // Process some samples
        fir.process(1.0);
        fir.process(0.5);

        // Reset should clear state
        fir.reset();

        // State should be zeroed (can't directly check internal state, but we verify no crash)
        let _ = fir.process(0.0);
    }

    #[test]
    fn test_fir_lowpass_dc_response() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);

        // DC (0 Hz) should have ~0 dB gain for lowpass
        let response = fir.log_result(0.0);
        assert!(
            response > -1.0 && response < 1.0,
            "DC response should be near 0 dB, got {:.2}",
            response
        );
    }

    #[test]
    fn test_fir_highpass_dc_response() {
        let fir = Fir::highpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);

        // DC (0 Hz) should have strong attenuation for highpass
        let response = fir.log_result(0.0);
        assert!(
            response < -20.0,
            "DC response for highpass should be < -20 dB, got {:.2}",
            response
        );
    }

    #[test]
    fn test_fir_process_dc() {
        let mut fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);

        // Process DC signal (constant 1.0)
        let mut output = 0.0;
        for _ in 0..100 {
            output = fir.process(1.0);
        }

        // After settling, lowpass output should be close to input for DC
        assert!(
            approx_eq(output, 1.0, 0.1),
            "DC output should be ~1.0, got {:.4}",
            output
        );
    }

    #[test]
    fn test_fir_np_log_result_is_finite() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = fir.np_log_result(&freqs);

        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }

    #[test]
    fn test_window_rectangular() {
        let window = generate_window(5, WindowType::Rectangular, 0.0);
        assert_eq!(window.len(), 5);
        for &w in &window {
            assert_eq!(w, 1.0);
        }
    }

    #[test]
    fn test_window_hamming() {
        let window = generate_window(5, WindowType::Hamming, 0.0);
        assert_eq!(window.len(), 5);

        // Hamming window should have maximum at center
        let max_val = window.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        assert!(window[2] >= max_val - 1e-10);

        // Edges should be lower than center
        assert!(window[0] < window[2]);
        assert!(window[4] < window[2]);
    }

    #[test]
    fn test_window_hann() {
        let window = generate_window(5, WindowType::Hann, 0.0);
        assert_eq!(window.len(), 5);

        // Hann window edges should be near zero
        assert!(window[0] < 0.1);
        assert!(window[4] < 0.1);

        // Center should be near 1.0
        assert!(window[2] > 0.9);
    }

    #[test]
    fn test_window_blackman() {
        let window = generate_window(5, WindowType::Blackman, 0.0);
        assert_eq!(window.len(), 5);

        // Blackman window should have maximum at center
        let max_val = window.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        assert!(window[2] >= max_val - 1e-10);
    }

    #[test]
    fn test_window_kaiser() {
        let window = generate_window(5, WindowType::Kaiser, 5.0);
        assert_eq!(window.len(), 5);

        // All values should be positive
        for &w in &window {
            assert!(w > 0.0);
        }

        // Center should be maximum (1.0 for Kaiser)
        assert!(approx_eq(window[2], 1.0, 1e-10));
    }

    #[test]
    fn test_bessel_i0() {
        // Test modified Bessel function at known points
        assert!(approx_eq(bessel_i0(0.0), 1.0, 1e-10));
        assert!(approx_eq(bessel_i0(1.0), 1.266, 0.001));
        assert!(approx_eq(bessel_i0(5.0), 27.24, 0.01));
    }

    #[test]
    fn test_fir_bank_empty() {
        let bank: FirBank = vec![];
        let freqs = array![100.0, 1000.0, 10000.0];
        let response = fir_bank_spl(&freqs, &bank);

        // Empty bank should give zero response
        for &r in response.iter() {
            assert_eq!(r, 0.0);
        }
    }

    #[test]
    fn test_fir_bank_single_filter() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        let bank = vec![(1.0, fir)];
        let freqs = array![100.0, 1000.0, 10000.0];
        let response = fir_bank_spl(&freqs, &bank);

        // Should have finite response at all frequencies
        for &r in response.iter() {
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_fir_bank_preamp_gain() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        let bank = vec![(1.0, fir)];

        let gain = fir_bank_preamp_gain(&bank);

        // Preamp gain should be small and negative (or zero)
        assert!(
            gain <= 1.0,
            "Preamp gain should be <= 1.0 dB, got {:.2}",
            gain
        );
    }

    #[test]
    fn test_fir_display() {
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        let display = format!("{}", fir);

        assert!(display.contains("LP"));
        assert!(display.contains("1000"));
        assert!(display.contains("48000"));
        assert!(display.contains("51"));
        assert!(display.contains("HAMM"));
    }

    #[test]
    fn test_fir_bandpass_display() {
        let fir = Fir::bandpass(51, 500.0, 2000.0, 48000.0, WindowType::Hann, 0.0);
        let display = format!("{}", fir);

        assert!(display.contains("BP"));
        assert!(display.contains("500"));
        assert!(display.contains("2000"));
    }

    #[test]
    fn test_fir_coeffs_symmetry() {
        // Linear phase FIR filters should have symmetric coefficients
        let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
        let coeffs = fir.coeffs();
        let n = coeffs.len();

        // Check symmetry
        for i in 0..n / 2 {
            assert!(
                approx_eq(coeffs[i], coeffs[n - 1 - i], 1e-10),
                "Coeffs should be symmetric: coeffs[{}]={:.6} != coeffs[{}]={:.6}",
                i,
                coeffs[i],
                n - 1 - i,
                coeffs[n - 1 - i]
            );
        }
    }

    #[test]
    fn test_fir_lowpass_cutoff_attenuation() {
        let cutoff = 1000.0;
        let fir = Fir::lowpass(101, cutoff, 48000.0, WindowType::Blackman, 0.0);

        // At cutoff frequency, should have some attenuation (typically -6 dB for ideal)
        let response_at_cutoff = fir.log_result(cutoff);
        assert!(
            response_at_cutoff < -3.0 && response_at_cutoff > -10.0,
            "Response at cutoff should be between -3 and -10 dB, got {:.2}",
            response_at_cutoff
        );

        // Well below cutoff should have minimal attenuation
        let response_below = fir.log_result(cutoff / 4.0);
        assert!(
            response_below > -3.0,
            "Response well below cutoff should be > -3 dB, got {:.2}",
            response_below
        );

        // Well above cutoff should have strong attenuation
        let response_above = fir.log_result(cutoff * 4.0);
        assert!(
            response_above < -20.0,
            "Response well above cutoff should be < -20 dB, got {:.2}",
            response_above
        );
    }

    #[test]
    fn test_fir_highpass_cutoff_attenuation() {
        let cutoff = 1000.0;
        let fir = Fir::highpass(101, cutoff, 48000.0, WindowType::Blackman, 0.0);

        // Well below cutoff should have strong attenuation
        let response_below = fir.log_result(cutoff / 4.0);
        assert!(
            response_below < -20.0,
            "Response well below cutoff should be < -20 dB, got {:.2}",
            response_below
        );

        // Well above cutoff should have minimal attenuation
        let response_above = fir.log_result(cutoff * 4.0);
        assert!(
            response_above > -3.0,
            "Response well above cutoff should be > -3 dB, got {:.2}",
            response_above
        );
    }
}
