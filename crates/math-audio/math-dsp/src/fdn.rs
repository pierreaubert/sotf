//! Feedback Delay Network (FDN) for Late Reverb Synthesis
//!
//! Provides a parametric late reverb tail using N delay lines with a unitary
//! feedback matrix. Used by the binaural plugin to extend early reflections
//! into a full room impulse response.
//!
//! Design: Hadamard feedback matrix (energy-preserving), mutually-prime delay
//! lengths, per-line 1-pole absorption filters for frequency-dependent decay.
//!
//! Reference: Jot, J.M. & Chaigne, A. (1991). "Digital Delay Networks for
//! Designing Artificial Reverberators."

/// Feedback Delay Network for late reverb synthesis.
///
/// N delay lines (default 8) with a Hadamard mixing matrix for energy-preserving
/// feedback, per-line lowpass absorption for frequency-dependent decay, and
/// stereo output tapping.
pub struct Fdn {
    num_lines: usize,
    delay_lines: Vec<Vec<f32>>,
    delay_lengths: Vec<usize>,
    write_positions: Vec<usize>,
    /// Hadamard feedback matrix (flattened NxN, row-major)
    feedback_matrix: Vec<f32>,
    /// Global feedback gain (< 1.0 for stability)
    feedback_gain: f32,
    /// Per-line 1-pole lowpass absorption: state
    absorption_state: Vec<f32>,
    /// Per-line absorption coefficient (0..1, higher = more HF damping)
    absorption_coeff: Vec<f32>,
    /// Input gains per delay line
    input_gains: Vec<f32>,
    /// Output taps: [line][2] for stereo (left, right)
    output_gains: Vec<[f32; 2]>,
    /// Pre-allocated scratch for reading delay line outputs before feedback
    scratch: Vec<f32>,
}

impl Fdn {
    /// Create a new FDN with `num_lines` delay lines.
    ///
    /// `num_lines` must be a power of 2 (for Hadamard matrix). Typical: 8 or 16.
    pub fn new(num_lines: usize, sample_rate: u32) -> Self {
        let num_lines = num_lines.next_power_of_two().max(2);

        // Mutually-prime delay lengths based on room size ~medium room
        let base_delays = prime_delays(num_lines, sample_rate);
        let delay_lines: Vec<Vec<f32>> = base_delays.iter().map(|&len| vec![0.0; len]).collect();

        let feedback_matrix = hadamard_flat(num_lines);
        let scale = 1.0 / (num_lines as f32).sqrt();

        // Distribute input and output across lines
        let input_gains = vec![scale; num_lines];
        let output_gains: Vec<[f32; 2]> = (0..num_lines)
            .map(|i| {
                let angle = std::f32::consts::PI * i as f32 / num_lines as f32;
                [angle.cos(), angle.sin()]
            })
            .collect();

        Self {
            num_lines,
            delay_lines,
            delay_lengths: base_delays,
            write_positions: vec![0; num_lines],
            feedback_matrix,
            feedback_gain: 0.7,
            absorption_state: vec![0.0; num_lines],
            absorption_coeff: vec![0.3; num_lines],
            input_gains,
            output_gains,
            scratch: vec![0.0; num_lines],
        }
    }

    /// Configure room parameters.
    ///
    /// `rt60`: Reverb time in seconds (0.1 - 10.0)
    /// `damping`: HF damping (0.0 = bright, 1.0 = dark)
    /// `size`: Room size factor (0.5 = small, 2.0 = large)
    pub fn set_room_params(&mut self, rt60: f32, damping: f32, size: f32, sample_rate: u32) {
        // Scale delay lengths by room size
        let base_delays = prime_delays(self.num_lines, sample_rate);
        for (i, &base) in base_delays.iter().enumerate() {
            let new_len = ((base as f32 * size) as usize).max(1);
            if new_len != self.delay_lengths[i] {
                self.delay_lengths[i] = new_len;
                self.delay_lines[i].resize(new_len, 0.0);
                self.write_positions[i] %= new_len;
            }
        }

        // Feedback gain from RT60: g = 10^(-3 * delay / (RT60 * SR))
        // Use the average delay length for the global gain
        let avg_delay = self.delay_lengths.iter().sum::<usize>() as f32 / self.num_lines as f32;
        let rt60_samples = rt60 * sample_rate as f32;
        self.feedback_gain = if rt60_samples > 0.0 {
            10.0_f32
                .powf(-3.0 * avg_delay / rt60_samples)
                .clamp(0.0, 0.999)
        } else {
            0.0
        };

        // Absorption from damping
        let damp = damping.clamp(0.0, 1.0);
        for coeff in &mut self.absorption_coeff {
            *coeff = damp * 0.6 + 0.1; // 0.1 (bright) to 0.7 (dark)
        }
    }

    /// Process one stereo sample pair through the FDN.
    ///
    /// Input is mixed into all delay lines. Output is tapped from all lines.
    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let n = self.num_lines;
        let mono_in = (left + right) * 0.5;

        // Read current outputs from all delay lines
        for i in 0..n {
            let pos = self.write_positions[i];
            self.scratch[i] = self.delay_lines[i][pos];
        }

        // Compute feedback: new_input[i] = sum_j(matrix[i][j] * scratch[j]) * feedback_gain
        // Plus input contribution
        for i in 0..n {
            let mut feedback_sum = 0.0;
            for j in 0..n {
                feedback_sum += self.feedback_matrix[i * n + j] * self.scratch[j];
            }
            let input = mono_in * self.input_gains[i];
            let fb = feedback_sum * self.feedback_gain;

            // 1-pole absorption: lowpass the feedback
            let alpha = self.absorption_coeff[i];
            self.absorption_state[i] = alpha * self.absorption_state[i] + (1.0 - alpha) * fb;

            // Write to delay line
            let pos = self.write_positions[i];
            self.delay_lines[i][pos] = input + self.absorption_state[i];
            self.write_positions[i] = (pos + 1) % self.delay_lengths[i];
        }

        // Tap outputs (stereo)
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for i in 0..n {
            out_l += self.scratch[i] * self.output_gains[i][0];
            out_r += self.scratch[i] * self.output_gains[i][1];
        }

        // Safety clamp: ±4 gives ~24 dB of headroom above unity, which is
        // enough for dense late reverb while preventing runaway feedback from
        // producing non-finite output if the FDN matrix becomes unstable.
        (out_l.clamp(-4.0, 4.0), out_r.clamp(-4.0, 4.0))
    }

    /// Reset all delay line state.
    pub fn reset(&mut self) {
        for dl in &mut self.delay_lines {
            dl.fill(0.0);
        }
        self.write_positions.fill(0);
        self.absorption_state.fill(0.0);
    }
}

/// Generate a Hadamard matrix of size N (flattened, row-major).
/// N must be a power of 2. The matrix is normalized by 1/sqrt(N).
fn hadamard_flat(n: usize) -> Vec<f32> {
    let mut h = vec![0.0f32; n * n];
    h[0] = 1.0;

    let mut size = 1;
    while size < n {
        // Copy top-left quadrant to other three quadrants
        for i in 0..size {
            for j in 0..size {
                let val = h[i * n + j];
                h[i * n + (j + size)] = val; // top-right
                h[(i + size) * n + j] = val; // bottom-left
                h[(i + size) * n + (j + size)] = -val; // bottom-right (negated)
            }
        }
        size *= 2;
    }

    // Normalize
    let scale = 1.0 / (n as f32).sqrt();
    for v in &mut h {
        *v *= scale;
    }
    h
}

/// Generate mutually-prime delay lengths for N delay lines.
/// Uses small primes scaled to produce delays in the 20-80ms range.
fn prime_delays(n: usize, sample_rate: u32) -> Vec<usize> {
    // Target delay range: 20-80ms
    let min_samples = (0.020 * sample_rate as f32) as usize;
    let max_samples = (0.080 * sample_rate as f32) as usize;

    // Primes in [min_samples, max_samples]
    let primes: Vec<usize> = (min_samples..=max_samples)
        .filter(|&n| is_prime(n))
        .collect();

    // Pick N evenly-spaced primes
    if primes.len() >= n {
        let step = primes.len() / n;
        (0..n)
            .map(|i| primes[(i * step).min(primes.len() - 1)])
            .collect()
    } else {
        // Fallback: use primes near target delays
        let base = (min_samples + max_samples) / 2;
        (0..n).map(|i| next_prime(base + i * 7)).collect()
    }
}

fn is_prime(n: usize) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n.is_multiple_of(2) || n.is_multiple_of(3) {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n.is_multiple_of(i) || n.is_multiple_of(i + 2) {
            return false;
        }
        i += 6;
    }
    true
}

fn next_prime(n: usize) -> usize {
    let mut p = n;
    while !is_prime(p) {
        p += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_orthogonal() {
        let n = 4;
        let h = hadamard_flat(n);
        // H * H^T should be identity (since H is normalized)
        for i in 0..n {
            for j in 0..n {
                let mut dot = 0.0;
                for k in 0..n {
                    dot += h[i * n + k] * h[j * n + k];
                }
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (dot - expected).abs() < 1e-5,
                    "H*H^T[{i}][{j}] = {dot}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn test_fdn_energy_decay() {
        let mut fdn = Fdn::new(8, 48000);
        fdn.set_room_params(1.0, 0.3, 1.0, 48000);

        // Feed impulse
        let (l, r) = fdn.process_stereo(1.0, 1.0);
        let _ = (l, r);

        // Process silence and measure energy decay
        let mut energy = 0.0f32;
        for _ in 0..48000 {
            let (l, r) = fdn.process_stereo(0.0, 0.0);
            energy += l * l + r * r;
        }

        // Should have produced some reverb energy
        assert!(energy > 0.01, "FDN produced no energy: {energy}");
        // But energy should be finite (stable)
        assert!(energy.is_finite(), "FDN energy is not finite");
    }

    #[test]
    fn test_fdn_stability() {
        let mut fdn = Fdn::new(8, 48000);
        fdn.set_room_params(3.0, 0.5, 1.5, 48000);

        // Process many frames with input
        for _ in 0..96000 {
            let (l, r) = fdn.process_stereo(0.01, 0.01);
            assert!(
                l.is_finite() && r.is_finite(),
                "Non-finite output: {l}, {r}"
            );
            assert!(l.abs() < 5.0 && r.abs() < 5.0, "Output too large: {l}, {r}");
        }
    }

    #[test]
    fn test_fdn_reset() {
        let mut fdn = Fdn::new(4, 48000);
        fdn.process_stereo(1.0, 1.0);
        fdn.reset();
        let (l, r) = fdn.process_stereo(0.0, 0.0);
        assert!(
            l.abs() < 1e-10 && r.abs() < 1e-10,
            "State not cleared: {l}, {r}"
        );
    }

    #[test]
    fn test_prime_delays() {
        let delays = prime_delays(8, 48000);
        assert_eq!(delays.len(), 8);
        // All should be prime
        for &d in &delays {
            assert!(is_prime(d), "{d} is not prime");
        }
        // All should be in 20-80ms range
        for &d in &delays {
            assert!((960..=3840).contains(&d), "Delay {d} out of range");
        }
    }
}
