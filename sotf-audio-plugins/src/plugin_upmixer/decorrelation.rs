// ============================================================================
// Decorrelation Functions
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::complex_mul_inplace_simd;
use rustfft::num_complex::Complex;

impl UpmixerPlugin {
    /// Generate decorrelation filters based on selected mode
    pub(super) fn generate_decorrelation_filters(&mut self) {
        // Diagnostic bypass: set all filters to identity (no phase change)
        if self.bypass_decorrelation {
            let len = self.decorrelation_filter_left.len();
            for i in 0..len {
                self.decorrelation_filter_left[i] = Complex::new(1.0, 0.0);
                self.decorrelation_filter_right[i] = Complex::new(1.0, 0.0);
            }
            return;
        }

        if self.decorrelation_mode == 1 {
            self.generate_lfo_base_phases();
            self.update_lfo_decorrelation();
        } else {
            self.generate_velvet_noise_decorrelators();
        }
    }

    /// Generate base phases for LFO-based decorrelation (random anchor points)
    pub(super) fn generate_lfo_base_phases(&mut self) {
        // Simple pseudo-random generator
        let mut seed = 12345u32;
        let mut rand_f32 = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed as f32) / (u32::MAX as f32)
        };

        let n = self.fft_size;
        let half = n / 2;

        let num_anchors = 16usize.min(half.max(2));
        let step = (half as f32) / (num_anchors.saturating_sub(1).max(1) as f32);

        let mut anchor_indices = Vec::with_capacity(num_anchors);
        let mut anchor_phases_left = Vec::with_capacity(num_anchors);
        let mut anchor_phases_right = Vec::with_capacity(num_anchors);

        for a in 0..num_anchors {
            let idx = (a as f32 * step).round() as usize;
            let idx = idx.min(half);
            anchor_indices.push(idx);

            if idx == 0 || idx == half {
                anchor_phases_left.push(0.0);
                anchor_phases_right.push(0.0);
            } else {
                anchor_phases_left.push(rand_f32() * 2.0 * std::f32::consts::PI);
                anchor_phases_right.push(rand_f32() * 2.0 * std::f32::consts::PI);
            }
        }

        *anchor_indices.first_mut().unwrap() = 0;
        *anchor_indices.last_mut().unwrap() = half;
        anchor_phases_left[0] = 0.0;
        anchor_phases_right[0] = 0.0;
        anchor_phases_left[num_anchors - 1] = 0.0;
        anchor_phases_right[num_anchors - 1] = 0.0;

        let interp_phase =
            |i: usize, idx_a: usize, idx_b: usize, phase_a: f32, phase_b: f32| -> f32 {
                if idx_b == idx_a {
                    return phase_a;
                }
                let t = (i as f32 - idx_a as f32) / (idx_b as f32 - idx_a as f32);
                let mut d = phase_b - phase_a;
                let pi = std::f32::consts::PI;
                if d > pi {
                    d -= 2.0 * pi;
                } else if d < -pi {
                    d += 2.0 * pi;
                }
                phase_a + d * t
            };

        let mut phases_left = vec![0.0f32; half + 1];
        let mut phases_right = vec![0.0f32; half + 1];

        for seg in 0..(num_anchors - 1) {
            let idx_a = anchor_indices[seg];
            let idx_b = anchor_indices[seg + 1];
            let phase_a_l = anchor_phases_left[seg];
            let phase_b_l = anchor_phases_left[seg + 1];
            let phase_a_r = anchor_phases_right[seg];
            let phase_b_r = anchor_phases_right[seg + 1];

            for i in idx_a..=idx_b {
                phases_left[i] = interp_phase(i, idx_a, idx_b, phase_a_l, phase_b_l);
                phases_right[i] = interp_phase(i, idx_a, idx_b, phase_a_r, phase_b_r);
            }
        }

        self.decor_base_phases_left = phases_left;
        self.decor_base_phases_right = phases_right;
        self.decor_lfo_phase = 0.0;
    }

    /// Update LFO-based decorrelation filters per frame
    pub(super) fn update_lfo_decorrelation(&mut self) {
        // Diagnostic bypass: skip update if bypass is enabled
        if self.bypass_decorrelation {
            return;
        }

        if self.sample_rate == 0 || self.fft_size == 0 {
            return;
        }

        let n = self.fft_size;
        let half = n / 2;
        if self.decor_base_phases_left.len() != half + 1
            || self.decor_base_phases_right.len() != half + 1
        {
            return;
        }

        let dt = self.hop_size as f32 / self.sample_rate as f32;
        // Use configurable LFO rate
        let rate_hz = self.decorrelation_lfo_rate_hz;
        let two_pi = std::f32::consts::PI * 2.0;
        self.decor_lfo_phase += two_pi * rate_hz * dt;
        if self.decor_lfo_phase > two_pi {
            self.decor_lfo_phase -= two_pi;
        }

        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
        let nyquist = self.sample_rate as f32 / 2.0;
        let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);

        // Critical frequencies for decorrelation shaping
        let mid_start = 800.0_f32; // Start reducing decorrelation in vocal range
        let mid_end = 4000.0_f32; // End of critical mid-range

        for i in 0..=half {
            let freq = i as f32 * freq_per_bin;

            let hf_ratio = if freq <= hf_start {
                0.0
            } else if freq >= nyquist {
                1.0
            } else {
                (freq - hf_start) / (nyquist - hf_start)
            };

            let mid_reduction = if freq < mid_start || freq > mid_end {
                1.0
            } else {
                let t = (freq - mid_start) / (mid_end - mid_start);
                0.3 + 0.7 * (std::f32::consts::PI * t).cos().abs()
            };

            let max_depth = 0.08_f32;
            let depth = max_depth * hf_ratio * mid_reduction;
            let phase_warp = (self.decor_lfo_phase + 0.37_f32 * i as f32).sin() * depth;

            let base_l = self.decor_base_phases_left[i];
            let base_r = self.decor_base_phases_right[i];

            let phi_l = base_l + phase_warp;
            let phi_r = base_r - phase_warp;

            self.decorrelation_filter_left[i] = Complex::from_polar(1.0, phi_l);
            self.decorrelation_filter_right[i] = Complex::from_polar(1.0, phi_r);
        }

        // DC and Nyquist must be real (no phase shift)
        self.decorrelation_filter_left[0] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_right[0] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_left[half] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_right[half] = Complex::new(1.0, 0.0);
    }

    /// Generate static decorrelation filters using Velvet Noise sequences.
    /// This creates a smooth, diffuse phase response without the "swooshing" artifacts
    /// of LFO-modulated phase shifters or the "metallic" sound of white noise.
    pub(super) fn generate_velvet_noise_decorrelators(&mut self) {
        // Generate two independent Velvet Noise sequences (Left and Right)
        // Use configurable duration (default 30ms)
        let duration_ms = self.velvet_noise_duration_ms;
        let seq_len = ((duration_ms / 1000.0) * self.sample_rate as f32) as usize;
        // Limit to half FFT size to avoid wrap-around issues while maintaining length
        let seq_len = seq_len.min(self.fft_size / 2).max(128);

        // Use configurable pulse density (default 2000 pulses/sec)
        let pulses_per_sec = self.velvet_noise_density;
        let grid_size = (self.sample_rate as f32 / pulses_per_sec).max(1.0) as usize;

        // Simple LCG for determinism
        let mut rng_seed = 12345u64;
        let mut rand_u32 = || {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng_seed >> 32) as u32
        };
        let mut rand_f32 = || rand_u32() as f32 / u32::MAX as f32;

        for ch in 0..2 {
            let mut time_buf = vec![0.0; self.fft_size];

            // Generate Velvet Noise
            let mut cursor = 0;
            // Add a small initial delay
            cursor += (rand_f32() * grid_size as f32) as usize;

            while cursor < seq_len {
                // Random position within grid
                let offset = (rand_f32() * grid_size as f32) as usize;
                let pos = (cursor + offset).min(self.fft_size - 1);

                // Random polarity (+1 or -1)
                let val = if rand_f32() > 0.5 { 1.0 } else { -1.0 };
                time_buf[pos] = val;

                cursor += grid_size;
            }

            // Apply fade-out window to avoid truncation artifacts
            // Without this, the sharp cutoff at seq_len creates high-frequency
            // ringing and metallic/scratchy artifacts in the frequency domain
            let fade_len = seq_len / 4; // Fade last 25%
            if fade_len > 0 {
                let fade_start = seq_len.saturating_sub(fade_len);
                for i in fade_start..seq_len {
                    let t = (i - fade_start) as f32 / fade_len as f32;
                    // Hann fade-out: cos^2 taper for smooth transition
                    let fade = 0.5 * (1.0 + (std::f32::consts::PI * t).cos());
                    time_buf[i] *= fade;
                }
            }

            // FFT to get frequency response
            let mut input_fft = self.fft_forward.make_input_vec();
            input_fft.copy_from_slice(&time_buf);
            let mut output_fft = self.fft_forward.make_output_vec();

            self.fft_forward
                .process(&mut input_fft, &mut output_fft)
                .unwrap();

            // Normalize Magnitude to 1.0 (All-Pass)
            for val in output_fft.iter_mut() {
                let norm = val.norm();
                if norm > 1e-9 {
                    *val /= norm;
                } else {
                    *val = Complex::new(1.0, 0.0);
                }
            }

            // DC and Nyquist bins must be real (no phase shift)
            // This prevents low-frequency rumble and high-frequency artifacts
            output_fft[0] = Complex::new(1.0, 0.0);
            let nyquist_idx = output_fft.len() - 1;
            output_fft[nyquist_idx] = Complex::new(1.0, 0.0);

            // Store in decorrelation filters
            let target = if ch == 0 {
                &mut self.decorrelation_filter_left
            } else {
                &mut self.decorrelation_filter_right
            };

            // Fill buffer up to its length
            for i in 0..target.len() {
                if i < output_fft.len() {
                    target[i] = output_fft[i];
                } else {
                    target[i] = Complex::new(0.0, 0.0);
                }
            }
        }
    }

    /// Apply adaptive decorrelation to ambient signals
    ///
    /// Blends between decorrelated and original signals based on strength parameter.
    /// Uses SIMD optimization for full decorrelation (strength >= 0.99).
    pub(super) fn apply_adaptive_decorrelation(&mut self, start: usize, end: usize, strength: f32) {
        // Fast path: full decorrelation (common case during steady-state)
        if strength >= 0.99 {
            let left_slice = &mut self.ambient_left[start..end];
            let right_slice = &mut self.ambient_right[start..end];
            let decor_left = &self.decorrelation_filter_left[start..end];
            let decor_right = &self.decorrelation_filter_right[start..end];

            complex_mul_inplace_simd(left_slice, decor_left);
            complex_mul_inplace_simd(right_slice, decor_right);
            return;
        }

        // Adaptive decorrelation: blend between decorrelated and original signals
        //
        // For each bin:
        //   decorrelated = signal * decorrelation_filter
        //   output = strength * decorrelated + (1 - strength) * signal
        //
        // This can be rewritten as:
        //   output = signal * (strength * decorrelation_filter + (1 - strength) * identity)
        //   output = signal * (strength * decorrelation_filter + (1 - strength))
        //
        // We compute the blended filter and apply it in one pass.

        let identity_weight = 1.0 - strength;

        for i in start..end {
            let decor_l = self.decorrelation_filter_left[i];
            let decor_r = self.decorrelation_filter_right[i];

            // Blend: strength * decor + (1 - strength) * identity
            // Identity is Complex::new(1.0, 0.0)
            let blended_l = Complex::new(
                strength * decor_l.re + identity_weight,
                strength * decor_l.im,
            );
            let blended_r = Complex::new(
                strength * decor_r.re + identity_weight,
                strength * decor_r.im,
            );

            self.ambient_left[i] *= blended_l;
            self.ambient_right[i] *= blended_r;
        }
    }
}
