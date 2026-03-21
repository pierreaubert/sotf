// ============================================================================
// Decorrelation Functions
// ============================================================================

use super::UpmixerPlugin;
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
        // Numerical Recipes LCG — intentionally simple for audio-thread use.
        // Crypto quality is irrelevant here; we only need decorrelated anchor phases.
        // The LCG is deterministic (fixed seed) so the decorrelation pattern is
        // reproducible across runs, and the constants (1664525 / 1013904223) give a
        // full 2^32 period which is more than sufficient for 16 anchor points.
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

    /// Precompute per-bin LFO depth table (depends on sample_rate, fft_size, bandpass_hz, lfe_cutoff_hz).
    /// Call in initialize() and when bandpass_hz or lfe_cutoff_hz changes.
    pub(super) fn precompute_lfo_depth_table(&mut self) {
        let half = self.fft_size / 2;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
        let nyquist = self.sample_rate as f32 / 2.0;
        let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);

        let mid_start = 800.0_f32;
        let mid_end = 4000.0_f32;
        let max_depth = 0.08_f32;

        self.cached_lfo_depth_table.resize(half + 1, 0.0);
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

            self.cached_lfo_depth_table[i] = max_depth * hf_ratio * mid_reduction;
        }
        // DC and Nyquist: zero depth (force real-only)
        self.cached_lfo_depth_table[0] = 0.0;
        self.cached_lfo_depth_table[half] = 0.0;
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

        // Use precomputed depth table (avoids per-bin hf_ratio, mid_reduction, cos() calculations)
        let has_depth_table = self.cached_lfo_depth_table.len() == half + 1;

        for i in 0..=half {
            let depth = if has_depth_table {
                self.cached_lfo_depth_table[i]
            } else {
                // Fallback: compute inline if table not yet built
                let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
                let nyquist = self.sample_rate as f32 / 2.0;
                let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);
                let freq = i as f32 * freq_per_bin;
                let hf_ratio = if freq <= hf_start {
                    0.0
                } else if freq >= nyquist {
                    1.0
                } else {
                    (freq - hf_start) / (nyquist - hf_start)
                };
                let mid_start = 800.0_f32;
                let mid_end = 4000.0_f32;
                let mid_reduction = if freq < mid_start || freq > mid_end {
                    1.0
                } else {
                    let t = (freq - mid_start) / (mid_end - mid_start);
                    0.3 + 0.7 * (std::f32::consts::PI * t).cos().abs()
                };
                0.08_f32 * hf_ratio * mid_reduction
            };

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
                for (i, sample) in time_buf
                    .iter_mut()
                    .enumerate()
                    .take(seq_len)
                    .skip(fade_start)
                {
                    let t = (i - fade_start) as f32 / fade_len as f32;
                    // Hann fade-out: cos^2 taper for smooth transition
                    let fade = 0.5 * (1.0 + (std::f32::consts::PI * t).cos());
                    *sample *= fade;
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

    /// Generate unique decorrelation filters per output channel.
    /// Front speakers and LFE get identity filters; surround and height channels
    /// each get a unique velvet noise filter (different seed per channel).
    pub(super) fn generate_per_channel_decorrelation_filters(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let num_ch = self.num_output_channels;

        self.decorrelation_filters = Vec::with_capacity(num_ch);

        for ch_idx in 0..num_ch {
            let speaker = &self.speaker_config.speakers[ch_idx];
            let is_front = speaker.azimuth.abs() < 80.0 && speaker.elevation.abs() < 10.0;

            if speaker.is_lfe || is_front {
                // Identity filter for front speakers and LFE
                self.decorrelation_filters
                    .push(vec![Complex::new(1.0, 0.0); spectrum_size]);
                continue;
            }

            // Generate unique velvet noise filter with channel-dependent seed
            let seed_base = 54321u64 + (ch_idx as u64 * 7919);
            let filter = self.generate_velvet_noise_filter_with_seed(seed_base, spectrum_size);
            self.decorrelation_filters.push(filter);
        }
    }

    /// Generate a single velvet noise all-pass filter with a specific seed
    fn generate_velvet_noise_filter_with_seed(
        &self,
        seed: u64,
        spectrum_size: usize,
    ) -> Vec<Complex<f32>> {
        let duration_ms = self.velvet_noise_duration_ms;
        let seq_len = ((duration_ms / 1000.0) * self.sample_rate as f32) as usize;
        let seq_len = seq_len.clamp(128, self.fft_size / 2);

        let pulses_per_sec = self.velvet_noise_density;
        let grid_size = (self.sample_rate as f32 / pulses_per_sec).max(1.0) as usize;

        let mut rng_seed = seed;
        let mut rand_u32 = || {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng_seed >> 32) as u32
        };
        let mut rand_f32 = || rand_u32() as f32 / u32::MAX as f32;

        let mut time_buf = vec![0.0f32; self.fft_size];

        let mut cursor = (rand_f32() * grid_size as f32) as usize;
        while cursor < seq_len {
            let offset = (rand_f32() * grid_size as f32) as usize;
            let pos = (cursor + offset).min(self.fft_size - 1);
            let val = if rand_f32() > 0.5 { 1.0 } else { -1.0 };
            time_buf[pos] = val;
            cursor += grid_size;
        }

        // Fade-out window
        let fade_len = seq_len / 4;
        if fade_len > 0 {
            let fade_start = seq_len.saturating_sub(fade_len);
            for (i, sample) in time_buf
                .iter_mut()
                .enumerate()
                .take(seq_len)
                .skip(fade_start)
            {
                let t = (i - fade_start) as f32 / fade_len as f32;
                let fade = 0.5 * (1.0 + (std::f32::consts::PI * t).cos());
                *sample *= fade;
            }
        }

        // FFT
        let mut input_fft = self.fft_forward.make_input_vec();
        input_fft.copy_from_slice(&time_buf);
        let mut output_fft = self.fft_forward.make_output_vec();
        self.fft_forward
            .process(&mut input_fft, &mut output_fft)
            .unwrap();

        // Normalize to all-pass
        for val in output_fft.iter_mut() {
            let norm = val.norm();
            if norm > 1e-9 {
                *val /= norm;
            } else {
                *val = Complex::new(1.0, 0.0);
            }
        }

        // DC and Nyquist: real only
        output_fft[0] = Complex::new(1.0, 0.0);
        let last = output_fft.len() - 1;
        output_fft[last] = Complex::new(1.0, 0.0);

        // Copy to result
        let mut result = vec![Complex::new(1.0, 0.0); spectrum_size];
        for (i, val) in output_fft.iter().enumerate() {
            if i < result.len() {
                result[i] = *val;
            }
        }
        result
    }
}
