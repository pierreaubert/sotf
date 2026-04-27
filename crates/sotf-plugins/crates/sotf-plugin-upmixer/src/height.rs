// ============================================================================
// Height Channel Processing
// ============================================================================
//
// Includes:
// 1. Spectral smoothing and temporal smoothing of height_band_gains
// 2. Spectral flux onset detection and coherence gating for height channel content
//
// The spectral flux gate selects content appropriate for height channels:
// - High spectral flux (onsets/transients) -> good for height (creates spatial excitement)
// - Low coherence (reverberant tails) -> good for height (creates envelopment)
// The gate output modulates height_band_gains to focus height content on these moments.

use super::{UpmixerPlugin, frequency_domain::HEIGHT_MASK_FLOOR};

/// Spectral flux smoothing alpha for height onset detection
const HEIGHT_FLUX_SMOOTH_ALPHA: f32 = 0.12;
/// Minimum gate value to prevent complete silence in height channels
const HEIGHT_GATE_FLOOR: f32 = 0.15;
/// Ignore tiny gate target changes; they create modulation with no useful audible intent.
const HEIGHT_GATE_DEADBAND: f32 = 0.01;
/// Fastest per-frame gate rise. Keeps transient lift audible without bin-rate chatter.
const HEIGHT_GATE_MAX_RISE: f32 = 0.08;
/// Fastest per-frame gate fall. Release is deliberately slower to avoid grain.
const HEIGHT_GATE_MAX_FALL: f32 = 0.035;
/// Gate smoothing alpha while rising toward onset/reverb targets.
const HEIGHT_GATE_ATTACK_ALPHA: f32 = 0.18;
/// Gate smoothing alpha while falling back toward the floor.
const HEIGHT_GATE_RELEASE_ALPHA: f32 = 0.055;
/// Threshold multiplier: flux must exceed baseline * this to trigger onset gate
const HEIGHT_ONSET_THRESHOLD: f32 = 1.5;
/// How much the onset gate can boost height content (0.0 = no boost, 1.0 = full boost)
const HEIGHT_ONSET_BOOST: f32 = 0.6;
/// How much low coherence contributes to height gate (reverb tail detection)
const HEIGHT_REVERB_WEIGHT: f32 = 0.4;
/// Fastest per-frame increase in final height gains after all masking.
const HEIGHT_GAIN_MAX_RISE: f32 = 0.035;
/// Fastest per-frame decrease in final height gains after all masking.
const HEIGHT_GAIN_MAX_FALL: f32 = 0.07;

#[inline(always)]
fn smooth_slew_limited(
    previous: f32,
    target: f32,
    attack_alpha: f32,
    release_alpha: f32,
    max_rise: f32,
    max_fall: f32,
    deadband: f32,
) -> f32 {
    let diff = target - previous;
    if diff.abs() <= deadband {
        return previous;
    }

    let alpha = if diff > 0.0 {
        attack_alpha
    } else {
        release_alpha
    };
    let smoothed = previous + alpha * diff;
    let limited_diff = (smoothed - previous).clamp(-max_fall, max_rise);
    (previous + limited_diff).clamp(HEIGHT_GATE_FLOOR, 1.0)
}

impl UpmixerPlugin {
    /// Compute the height spectral flux gate.
    ///
    /// For each bin, computes spectral flux (positive half-wave rectified difference
    /// from previous frame). High flux indicates onsets; low coherence indicates reverb.
    /// Both are good candidates for height channel content.
    ///
    /// Updates `height_flux_gate` per-bin and `height_spectral_flux_smooth` baseline.
    #[inline]
    pub(super) fn compute_height_flux_gate(&mut self) {
        let spec_size = self.fft_size / 2 + 1;
        let bandpass_bin = self.cached_bandpass_bin;

        // Compute per-bin spectral flux (half-wave rectified)
        let mut total_flux = 0.0_f32;
        let mut bin_count = 0_u32;

        for i in bandpass_bin..spec_size {
            let l = self.freq_domain_left[i];
            let r = self.freq_domain_right[i];
            let current_mag = (l.norm_sqr() + r.norm_sqr()).sqrt();
            let prev_mag = self.height_prev_magnitude[i];
            let diff = current_mag - prev_mag;
            self.height_prev_magnitude[i] = current_mag;

            if diff > 0.0 {
                total_flux += diff;
            }
            bin_count += 1;
        }

        // Normalize flux by bin count
        let avg_flux = if bin_count > 0 {
            total_flux / bin_count as f32
        } else {
            0.0
        };

        // Bootstrap baseline
        if self.height_spectral_flux_smooth < 1e-12 && avg_flux > 0.0 {
            self.height_spectral_flux_smooth = avg_flux;
        }

        // Smooth the flux baseline
        let flux_alpha = if avg_flux > self.height_spectral_flux_smooth {
            HEIGHT_FLUX_SMOOTH_ALPHA
        } else {
            HEIGHT_FLUX_SMOOTH_ALPHA * 0.3 // Slower release for baseline
        };
        self.height_spectral_flux_smooth +=
            flux_alpha * (avg_flux - self.height_spectral_flux_smooth);

        // Compute onset ratio (how much current flux exceeds baseline)
        let onset_ratio = if self.height_spectral_flux_smooth > 1e-9 {
            (avg_flux / self.height_spectral_flux_smooth - 1.0).max(0.0)
        } else {
            0.0
        };

        // Onset gate: ramps from HEIGHT_GATE_FLOOR to 1.0 based on onset strength
        let onset_gate = if onset_ratio > (HEIGHT_ONSET_THRESHOLD - 1.0) {
            let normalized = (onset_ratio / HEIGHT_ONSET_THRESHOLD).min(1.0);
            HEIGHT_GATE_FLOOR + (1.0 - HEIGHT_GATE_FLOOR) * normalized * HEIGHT_ONSET_BOOST
        } else {
            HEIGHT_GATE_FLOOR
        };

        // Combine onset gate with per-band coherence-based reverb gate
        // Low coherence bands get a higher gate (more suitable for height)
        for band_idx in 0..self.erb_bands.len() {
            let start = self.erb_bands[band_idx];
            let end = if band_idx + 1 < self.erb_bands.len() {
                self.erb_bands[band_idx + 1]
            } else {
                spec_size
            };

            if start < bandpass_bin {
                // Below bandpass: no height content
                for i in start..end.min(bandpass_bin) {
                    self.height_flux_gate[i] = smooth_slew_limited(
                        self.height_flux_gate[i],
                        HEIGHT_GATE_FLOOR,
                        HEIGHT_GATE_ATTACK_ALPHA,
                        HEIGHT_GATE_RELEASE_ALPHA,
                        HEIGHT_GATE_MAX_RISE,
                        HEIGHT_GATE_MAX_FALL,
                        HEIGHT_GATE_DEADBAND,
                    );
                }
            }

            let band_start = start.max(bandpass_bin);
            if band_start >= end {
                continue;
            }

            // Use smoothed coherence for this band: low coherence = reverb tail
            let band_coh = if band_idx < self.smoothed_coherence.len() {
                self.smoothed_coherence[band_idx]
            } else {
                0.5
            };
            let reverb_gate = (1.0 - band_coh) * HEIGHT_REVERB_WEIGHT;

            // Combined gate: onset detection OR reverb detection, clamped
            let combined = (onset_gate + reverb_gate).clamp(HEIGHT_GATE_FLOOR, 1.0);

            for i in band_start..end {
                self.height_flux_gate[i] = smooth_slew_limited(
                    self.height_flux_gate[i],
                    combined,
                    HEIGHT_GATE_ATTACK_ALPHA,
                    HEIGHT_GATE_RELEASE_ALPHA,
                    HEIGHT_GATE_MAX_RISE,
                    HEIGHT_GATE_MAX_FALL,
                    HEIGHT_GATE_DEADBAND,
                );
            }
        }
    }

    /// Smooth height_band_gains to reduce bin-to-bin and frame-to-frame variance
    ///
    /// This applies:
    /// 1. Spectral smoothing: 5-point sliding window average across adjacent bins
    ///    Edge bins handled separately so the main loop has a fixed 5-point window
    ///    with constant multiplier (* 0.2), enabling LLVM auto-vectorization.
    /// 2. Modulation by height flux gate (onset detection + reverb gating)
    /// 3. Temporal smoothing: exponential averaging with previous frame
    ///
    /// This reduces "grainy" artifacts from bin-level processing within ERB bands.
    #[inline]
    pub(super) fn smooth_height_gains(&mut self) {
        let n = self.fft_size / 2 + 1;
        let mut smoothed = std::mem::take(&mut self.height_band_gains_temp);

        // Spectral smoothing: edge-separated 5-point moving average.
        // Handling edge bins separately eliminates per-iteration count tracking
        // and division, letting LLVM auto-vectorize the main loop (1021+ iterations).
        match n {
            0 => {}
            1 => {
                smoothed[0] = self.height_band_gains[0];
            }
            2 => {
                let s = (self.height_band_gains[0] + self.height_band_gains[1]) * 0.5;
                smoothed[0] = s;
                smoothed[1] = s;
            }
            3 => {
                let s = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2])
                    / 3.0;
                smoothed[0] = s;
                smoothed[1] = s;
                smoothed[2] = s;
            }
            4 => {
                smoothed[0] = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2])
                    / 3.0;
                let mid = (self.height_band_gains[0]
                    + self.height_band_gains[1]
                    + self.height_band_gains[2]
                    + self.height_band_gains[3])
                    * 0.25;
                smoothed[1] = mid;
                smoothed[2] = mid;
                smoothed[3] = (self.height_band_gains[1]
                    + self.height_band_gains[2]
                    + self.height_band_gains[3])
                    / 3.0;
            }
            _ => {
                let src = &self.height_band_gains[..n];

                // First 2 bins: partial window
                smoothed[0] = (src[0] + src[1] + src[2]) / 3.0;
                smoothed[1] = (src[0] + src[1] + src[2] + src[3]) * 0.25;

                // Main loop: fixed 5-point average, branchless, auto-vectorizable
                for i in 2..n - 2 {
                    smoothed[i] =
                        (src[i - 2] + src[i - 1] + src[i] + src[i + 1] + src[i + 2]) * 0.2;
                }

                // Last 2 bins: partial window
                smoothed[n - 2] = (src[n - 4] + src[n - 3] + src[n - 2] + src[n - 1]) * 0.25;
                smoothed[n - 1] = (src[n - 3] + src[n - 2] + src[n - 1]) / 3.0;
            }
        }

        // Apply height flux gate modulation: multiply smoothed gains by the gate
        // This focuses height content on onsets and reverberant tails
        for (s, &gate) in smoothed
            .iter_mut()
            .zip(self.height_flux_gate.iter())
            .take(n)
        {
            *s *= gate;
        }

        // Temporal smoothing: asymmetric attack/release blend with previous frame.
        // Fast attack for transient ducking, slow release to prevent crackle on mask recovery.
        let attack_alpha = 0.16_f32;
        let release_alpha = 0.045_f32;
        for (s, (gain, prev)) in smoothed
            .iter()
            .zip(
                self.height_band_gains
                    .iter_mut()
                    .zip(self.height_band_gains_prev.iter_mut()),
            )
            .take(n)
        {
            let alpha = if *s < *prev {
                attack_alpha
            } else {
                release_alpha
            };
            let blended = alpha * s + (1.0 - alpha) * *prev;
            let delta = (blended - *prev).clamp(-HEIGHT_GAIN_MAX_FALL, HEIGHT_GAIN_MAX_RISE);
            let blended = (*prev + delta).clamp(HEIGHT_MASK_FLOOR, 1.0);
            *gain = blended;
            *prev = blended;
        }

        self.height_band_gains_temp = smoothed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_gate_slew_limiter_caps_frame_steps() {
        let up = smooth_slew_limited(
            HEIGHT_GATE_FLOOR,
            1.0,
            HEIGHT_GATE_ATTACK_ALPHA,
            HEIGHT_GATE_RELEASE_ALPHA,
            HEIGHT_GATE_MAX_RISE,
            HEIGHT_GATE_MAX_FALL,
            HEIGHT_GATE_DEADBAND,
        );
        assert!(
            up - HEIGHT_GATE_FLOOR <= HEIGHT_GATE_MAX_RISE + 1e-6,
            "gate rose too quickly: {}",
            up - HEIGHT_GATE_FLOOR
        );

        let down = smooth_slew_limited(
            1.0,
            HEIGHT_GATE_FLOOR,
            HEIGHT_GATE_ATTACK_ALPHA,
            HEIGHT_GATE_RELEASE_ALPHA,
            HEIGHT_GATE_MAX_RISE,
            HEIGHT_GATE_MAX_FALL,
            HEIGHT_GATE_DEADBAND,
        );
        assert!(
            1.0 - down <= HEIGHT_GATE_MAX_FALL + 1e-6,
            "gate fell too quickly: {}",
            1.0 - down
        );
    }
}
