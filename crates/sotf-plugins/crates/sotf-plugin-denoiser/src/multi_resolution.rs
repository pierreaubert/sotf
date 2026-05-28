// ============================================================================
// Multi-Resolution STFT Processing
// ============================================================================
//
// Dual-resolution denoising that combines two STFT instances:
//   - Small FFT (512 samples):  ~4 ms time resolution  — good for transients
//   - Large FFT (2048 samples): ~43 ms freq resolution  — good for steady noise
//
// Algorithm per FFT block (called from the large-FFT path):
//   1. Feed the same audio into the small-FFT input accumulator.
//   2. Each time the small accumulator has 512 samples, run:
//      a. Forward FFT (sqrt-Hann windowed)
//      b. IMCRA noise estimation (independent state)
//      c. Wiener gain computation (stored in `smoothed_gain[k_small]`)
//      d. Spectral flux = Σ |M_k - M_k_prev| for the current frame
//   3. When the large FFT block fires, combine gains:
//        final_gain[k] = flux_weight * small_gain[k_mapped]
//                      + (1 − flux_weight) * large_gain[k]
//      where k_mapped = round(k * small_spectrum_size / large_spectrum_size)
//      and   flux_weight = smoothed_flux clamped to [0, 1]
//
// The combined gains are written into `self.smoothed_gain` so the rest of
// the large-FFT pipeline (apply_gains_and_inverse_fft, overlap_add) is
// unchanged.

use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

/// Small FFT size for transient-resolution arm
pub(super) const SMALL_FFT_SIZE: usize = 512;

/// Spectral flux threshold: flux values above this → transient weight = 1.0.
/// Flux is the mean per-bin magnitude change normalised by the spectrum size.
const FLUX_HIGH: f32 = 0.08;
/// Below this flux value the weight is 0.0 (pure large-FFT gains).
const FLUX_LOW: f32 = 0.01;

/// Bootstrap frames for the small-FFT IMCRA path (same logic as the main MCRA).
const SMALL_BOOTSTRAP_FRAMES: usize = 5;

/// Per-channel small-FFT state.
/// Only holds what is actively used during processing — no inverse FFT plan
/// because the small-FFT path only computes gains; audio reconstruction uses
/// the large-FFT OLA pipeline unchanged.
pub(super) struct SmallFftState {
    // Forward FFT plan
    fft_forward: Arc<dyn RealToComplex<f32>>,

    // sqrt(Hann) analysis window [SMALL_FFT_SIZE]
    window: Vec<f32>,

    // Time-domain scratch (one channel) [SMALL_FFT_SIZE]
    time_domain: Vec<f32>,
    // Frequency-domain scratch [SMALL_FFT_SIZE / 2 + 1]
    freq_domain: Vec<Complex<f32>>,

    // IMCRA noise estimation state [SMALL_FFT_SIZE / 2 + 1]
    noise_psd: Vec<f32>,
    smoothed_psd: Vec<f32>,
    min_psd: Vec<f32>,
    min_psd_b: Vec<f32>,
    speech_presence: Vec<f32>,
    frame_counter: usize,

    // Temporally-smoothed Wiener gains — used by `combine_gains()` [SMALL_FFT_SIZE / 2 + 1]
    pub smoothed_gain: Vec<f32>,

    // Previous magnitude for spectral flux [SMALL_FFT_SIZE / 2 + 1]
    prev_mag: Vec<f32>,
    // Latest per-frame flux value (averaged across bins, smoothed across frames)
    pub current_flux: f32,
}

impl SmallFftState {
    pub fn new() -> Self {
        let fft_size = SMALL_FFT_SIZE;
        let spectrum_size = fft_size / 2 + 1;

        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);

        let window = sotf_host::stft_common::generate_sqrt_hann_window(fft_size);

        Self {
            fft_forward,
            window,
            time_domain: vec![0.0_f32; fft_size],
            freq_domain: vec![Complex::new(0.0, 0.0); spectrum_size],
            noise_psd: vec![1e-6_f32; spectrum_size],
            smoothed_psd: vec![1e-6_f32; spectrum_size],
            min_psd: vec![1e-6_f32; spectrum_size],
            min_psd_b: vec![1e-6_f32; spectrum_size],
            speech_presence: vec![0.0_f32; spectrum_size],
            frame_counter: 0,
            smoothed_gain: vec![1.0_f32; spectrum_size],
            prev_mag: vec![0.0_f32; spectrum_size],
            current_flux: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.noise_psd.fill(1e-6);
        self.smoothed_psd.fill(1e-6);
        self.min_psd.fill(1e-6);
        self.min_psd_b.fill(1e-6);
        self.speech_presence.fill(0.0);
        self.frame_counter = 0;
        self.smoothed_gain.fill(1.0);
        self.prev_mag.fill(0.0);
        self.current_flux = 0.0;
        self.time_domain.fill(0.0);
        self.freq_domain
            .iter_mut()
            .for_each(|c| *c = Complex::new(0.0, 0.0));
    }
}

/// Holds the complete multi-resolution state: per-channel `SmallFftState` plus
/// the shared input accumulator that feeds samples into the small-FFT path.
pub(super) struct MultiResState {
    /// Per-channel small-FFT processing state.
    pub channels: Vec<SmallFftState>,

    /// Interleaved sample accumulator for the small-FFT path.
    /// Length = SMALL_FFT_SIZE * num_channels * 2 (double buffer for safety).
    input_buffer: Vec<f32>,
    input_buffer_fill: usize,

    /// Scratch block for windowing (avoids per-block allocation).
    temp_input_block: Vec<f32>,

    /// Smoothed flux weight [0, 1] — high = transient (use small-FFT gains).
    pub flux_weight: f32,

    // MCRA hyper-parameters (copied from the main plugin at creation time)
    mcra_alpha_s: f32,
    mcra_alpha_p: f32,
    mcra_l: usize,
    mcra_delta: f32,
}

impl MultiResState {
    pub fn new(
        num_channels: usize,
        mcra_alpha_s: f32,
        mcra_alpha_p: f32,
        mcra_l: usize,
        mcra_delta: f32,
    ) -> Self {
        let fft_size = SMALL_FFT_SIZE;
        let input_buffer = vec![0.0_f32; fft_size * num_channels * 2];
        let temp_input_block = vec![0.0_f32; fft_size * num_channels];

        Self {
            channels: (0..num_channels).map(|_| SmallFftState::new()).collect(),
            input_buffer,
            input_buffer_fill: 0,
            temp_input_block,
            flux_weight: 0.0,
            mcra_alpha_s,
            mcra_alpha_p,
            mcra_l,
            mcra_delta,
        }
    }

    pub fn reset(&mut self) {
        for ch in &mut self.channels {
            ch.reset();
        }
        self.input_buffer.fill(0.0);
        self.input_buffer_fill = 0;
        self.flux_weight = 0.0;
    }

    /// Feed `samples` (interleaved, all channels) into the small-FFT accumulator.
    /// Process small FFT blocks whenever the accumulator is full.
    /// After this call `flux_weight` reflects the detected transient intensity.
    pub fn feed_and_process(
        &mut self,
        samples: &[f32],
        num_channels: usize,
        reduction_linear: f32,
        floor_linear: f32,
    ) {
        let block_samples = SMALL_FFT_SIZE * num_channels;
        let mut pos = 0;

        while pos < samples.len() {
            let space = self.input_buffer.len() - self.input_buffer_fill;
            let to_copy = (samples.len() - pos).min(space);

            self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + to_copy]
                .copy_from_slice(&samples[pos..pos + to_copy]);
            self.input_buffer_fill += to_copy;
            pos += to_copy;

            while self.input_buffer_fill >= block_samples {
                self.process_small_block(num_channels, reduction_linear, floor_linear);
            }
        }
    }

    fn process_small_block(
        &mut self,
        num_channels: usize,
        reduction_linear: f32,
        floor_linear: f32,
    ) {
        let small_fft_size = SMALL_FFT_SIZE;
        let hop_size = small_fft_size / 2;
        let spectrum_size = small_fft_size / 2 + 1;
        let block_samples = small_fft_size * num_channels;
        let shift_samples = hop_size * num_channels;

        // Copy block to temp buffer before shifting.
        self.temp_input_block[..block_samples].copy_from_slice(&self.input_buffer[..block_samples]);

        // Forward FFT per channel: de-interleave + window, then FFT
        for ch in 0..num_channels {
            let state = &mut self.channels[ch];
            for i in 0..small_fft_size {
                state.time_domain[i] =
                    self.temp_input_block[i * num_channels + ch] * state.window[i];
            }
            state
                .fft_forward
                .process(&mut state.time_domain, &mut state.freq_domain)
                .expect("small FFT forward failed");
        }

        // Shift input buffer left by hop_size (consume processed overlap)
        self.input_buffer.copy_within(shift_samples.., 0);
        self.input_buffer_fill -= shift_samples;

        // Per-channel: IMCRA noise estimation → Wiener gains → spectral flux
        let mcra_alpha_s = self.mcra_alpha_s;
        let mcra_alpha_p = self.mcra_alpha_p;
        let mcra_l = self.mcra_l;
        let mcra_delta = self.mcra_delta;
        const EPSILON: f32 = 1e-10;

        let mut total_flux = 0.0_f32;
        let mut channels_with_flux: usize = 0;

        for ch in 0..num_channels {
            let state = &mut self.channels[ch];

            // Bootstrap phase: accumulate power, initialise IMCRA on completion
            if state.frame_counter < SMALL_BOOTSTRAP_FRAMES {
                for k in 0..spectrum_size {
                    let power = state.freq_domain[k].norm_sqr();
                    state.noise_psd[k] += power;
                }
                state.frame_counter += 1;
                if state.frame_counter >= SMALL_BOOTSTRAP_FRAMES {
                    let n = state.frame_counter as f32;
                    let min_power = 1e-6_f32;
                    for k in 0..spectrum_size {
                        let avg = (state.noise_psd[k] / n).max(min_power);
                        state.noise_psd[k] = avg;
                        state.smoothed_psd[k] = avg;
                        state.min_psd[k] = avg;
                        state.min_psd_b[k] = avg;
                        state.speech_presence[k] = 0.0;
                    }
                }
                // Gains stay at 1.0 during bootstrap
                continue;
            }

            // IMCRA noise estimation
            let frame = state.frame_counter;
            let half_l = mcra_l / 2;
            let reset_a = frame.is_multiple_of(mcra_l);
            let reset_b = frame % mcra_l == half_l;

            for k in 0..spectrum_size {
                let power = state.freq_domain[k].norm_sqr();

                let s_tmp = mcra_alpha_s * state.smoothed_psd[k] + (1.0 - mcra_alpha_s) * power;
                state.smoothed_psd[k] = s_tmp;

                if reset_a {
                    state.min_psd[k] = s_tmp;
                } else {
                    state.min_psd[k] = state.min_psd[k].min(s_tmp);
                }
                if reset_b {
                    state.min_psd_b[k] = s_tmp;
                } else {
                    state.min_psd_b[k] = state.min_psd_b[k].min(s_tmp);
                }

                let s_min = state.min_psd[k].min(state.min_psd_b[k]).max(EPSILON);
                let s_r = s_tmp / s_min;
                let indicator = if s_r > mcra_delta { 1.0_f32 } else { 0.0 };

                let p = mcra_alpha_p * state.speech_presence[k] + (1.0 - mcra_alpha_p) * indicator;
                state.speech_presence[k] = p;

                let alpha_d = mcra_alpha_s + (1.0 - mcra_alpha_s) * p;
                state.noise_psd[k] = alpha_d * state.noise_psd[k] + (1.0 - alpha_d) * power;
            }
            state.frame_counter += 1;

            // Wiener gain computation — no temporal smoothing here.
            // The large-FFT path in calculate_wiener_gains() applies its own
            // temporal smoother after combine_gains(); applying it here too
            // would cause double-smoothing (~2 extra frames of attack/release lag).
            for k in 0..spectrum_size {
                let signal_power = state.freq_domain[k].norm_sqr();
                let noise_power = state.noise_psd[k].max(EPSILON);
                let snr = ((signal_power - noise_power).max(0.0)) / noise_power;
                state.smoothed_gain[k] = (snr / (snr + reduction_linear)).max(floor_linear);
            }

            // Spectral flux: mean |magnitude_change| across bins
            let mut flux = 0.0_f32;
            for k in 0..spectrum_size {
                let mag = state.freq_domain[k].norm();
                flux += (mag - state.prev_mag[k]).abs();
                state.prev_mag[k] = mag;
            }
            state.current_flux = flux / spectrum_size as f32;
            total_flux += state.current_flux;
            channels_with_flux += 1;
        }

        // Update smoothed flux weight
        if channels_with_flux > 0 {
            let avg_flux = total_flux / channels_with_flux as f32;
            // Map raw flux to [0, 1] via linear ramp between FLUX_LOW and FLUX_HIGH
            let raw_weight = ((avg_flux - FLUX_LOW) / (FLUX_HIGH - FLUX_LOW)).clamp(0.0, 1.0);
            // One-pole smoothing (~2-frame time constant)
            const FLUX_SMOOTH: f32 = 0.5;
            self.flux_weight = FLUX_SMOOTH * self.flux_weight + (1.0 - FLUX_SMOOTH) * raw_weight;
        }
    }

    /// Combine small-FFT gains with large-FFT gains.
    ///
    /// For each large-FFT bin k (0..large_spectrum_size):
    ///   k_small = k * small_spectrum_size / large_spectrum_size   (integer rounding)
    ///   final[k] = flux_weight * small_gain[ch][k_small]
    ///            + (1 − flux_weight) * large_gain[ch][k]
    ///
    /// The result is written back into `large_gain[ch][k]` in place.
    pub fn combine_gains(
        &self,
        large_gain: &mut [Vec<f32>],
        num_channels: usize,
        large_spectrum_size: usize,
    ) {
        let small_spectrum_size = SMALL_FFT_SIZE / 2 + 1;
        let flux_w = self.flux_weight;
        let large_w = 1.0 - flux_w;

        // We need the bin index `k` for the frequency-mapping arithmetic, so we
        // use enumerate() on the output slice combined with zip for the channels.
        for (ch_state, large) in self
            .channels
            .iter()
            .take(num_channels)
            .zip(large_gain.iter_mut().take(num_channels))
        {
            let small_gains = &ch_state.smoothed_gain;
            for (k, g_large) in large.iter_mut().enumerate().take(large_spectrum_size) {
                // Integer linear frequency mapping large bin k → small bin.
                // Equivalent to round(k * small_spectrum_size / large_spectrum_size).
                let k_small = ((k * small_spectrum_size + large_spectrum_size / 2)
                    / large_spectrum_size)
                    .min(small_spectrum_size - 1);

                *g_large = flux_w * small_gains[k_small] + large_w * *g_large;
            }
        }
    }
}
