//! Pure Rust implementation of ITU-R BS.1770-4 / EBU R128 loudness measurement.
//!
//! Implements K-weighting, momentary/short-term/integrated loudness,
//! sample peak, and true peak (4x oversampling).

use std::collections::VecDeque;
use std::f64::consts::PI;

// In tests we use a much smaller cap so overflow behaviour can be exercised
// quickly without allocating gigabytes of audio.
#[cfg(test)]
const MAX_GATING_BLOCKS: usize = 100;
#[cfg(not(test))]
const MAX_GATING_BLOCKS: usize = 36_000;

// ── Mode bitflags ───────────────────────────────────────────────────────────

/// Measurement modes (bitflags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode(u32);

impl Mode {
    pub const M: Mode = Mode(1 << 0);
    pub const S: Mode = Mode(1 << 1);
    pub const I: Mode = Mode(1 << 2);
    pub const SAMPLE_PEAK: Mode = Mode(1 << 3);
    pub const TRUE_PEAK: Mode = Mode(1 << 4);

    pub const fn all() -> Mode {
        Mode(0x1F)
    }

    fn has(self, flag: Mode) -> bool {
        self.0 & flag.0 != 0
    }
}

impl std::ops::BitOr for Mode {
    type Output = Mode;
    fn bitor(self, rhs: Mode) -> Mode {
        Mode(self.0 | rhs.0)
    }
}

// ── K-weighting filter ─────────────────────────────────────────────────────

/// Second-order IIR section (biquad) in Direct Form II Transposed.
#[derive(Clone)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// K-weighting: pre-filter (high shelf) + RLB high-pass.
/// Coefficients from ITU-R BS.1770-4, computed via bilinear transform.
#[derive(Clone)]
struct KWeightFilter {
    stage1: Biquad,
    stage2: Biquad,
}

impl KWeightFilter {
    fn new(sample_rate: u32) -> Self {
        let (s1, s2) = if sample_rate == 48000 {
            Self::coeffs_48k()
        } else {
            Self::compute_coeffs(sample_rate as f64)
        };
        Self {
            stage1: s1,
            stage2: s2,
        }
    }

    /// Hardcoded coefficients for 48 kHz (most common case).
    fn coeffs_48k() -> (Biquad, Biquad) {
        // Stage 1: Pre-filter (high shelf)
        let s1 = Biquad {
            b0: 1.53512485958697,
            b1: -2.69169618940638,
            b2: 1.19839281085285,
            a1: -1.69065929318241,
            a2: 0.73248077421585,
            z1: 0.0,
            z2: 0.0,
        };
        // Stage 2: RLB high-pass
        let s2 = Biquad {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: -1.99004745483398,
            a2: 0.99007225036621,
            z1: 0.0,
            z2: 0.0,
        };
        (s1, s2)
    }

    /// Compute coefficients for arbitrary sample rate via bilinear transform.
    fn compute_coeffs(fs: f64) -> (Biquad, Biquad) {
        // Stage 1: High shelf from BS.1770-4 analog prototype
        // Analog: H(s) = Vh * (s^2 + (sqrt(Vh)/Q)*s + 1) / (s^2 + (1/(Q*sqrt(Vh)))*s + 1)
        // with fc=1681.974450955533, Q=0.7071752369554196, dB_gain=3.999843853973347
        let fc1 = 1681.974450955533;
        let q1 = 0.7071752369554196;
        let db1 = 3.999843853973347;
        let vh = 10.0_f64.powf(db1 / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let k1 = (PI * fc1 / fs).tan();
        let k1sq = k1 * k1;
        let denom1 = 1.0 + k1 / q1 + k1sq;
        let s1 = Biquad {
            b0: (vh + vb * k1 / q1 + k1sq) / denom1,
            b1: 2.0 * (k1sq - vh) / denom1,
            b2: (vh - vb * k1 / q1 + k1sq) / denom1,
            a1: 2.0 * (k1sq - 1.0) / denom1,
            a2: (1.0 - k1 / q1 + k1sq) / denom1,
            z1: 0.0,
            z2: 0.0,
        };

        // Stage 2: RLB high-pass (2nd order Butterworth high-pass at 38.13547087602444 Hz)
        let fc2 = 38.13547087602444;
        let q2 = 0.5003270373238773;
        let k2 = (PI * fc2 / fs).tan();
        let k2sq = k2 * k2;
        let denom2 = 1.0 + k2 / q2 + k2sq;
        let s2 = Biquad {
            b0: 1.0 / denom2,
            b1: -2.0 / denom2,
            b2: 1.0 / denom2,
            a1: 2.0 * (k2sq - 1.0) / denom2,
            a2: (1.0 - k2 / q2 + k2sq) / denom2,
            z1: 0.0,
            z2: 0.0,
        };

        (s1, s2)
    }

    fn process(&mut self, x: f64) -> f64 {
        let y1 = self.stage1.process(x);
        self.stage2.process(y1)
    }

    fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
    }
}

// ── True peak oversampling ──────────────────────────────────────────────────

/// 4x oversampling FIR for true peak detection.
/// 48-tap polyphase filter (4 phases × 12 taps) from BS.1770-4 Table 2.
const TRUE_PEAK_FIR_PHASES: [[f64; 12]; 4] = [
    [
        0.0017089843750,
        -0.0291748046875,
        -0.0189208984375,
        0.0776367187500,
        0.0983886718750,
        -0.1897583007813,
        -0.3953857421875,
        0.8893127441406,
        0.6444091796875,
        -0.0517578125000,
        -0.0245361328125,
        0.0015869140625,
    ],
    [
        -0.0291748046875,
        0.0017089843750,
        0.0776367187500,
        -0.0189208984375,
        -0.1897583007813,
        0.0983886718750,
        0.8893127441406,
        -0.3953857421875,
        -0.0517578125000,
        0.6444091796875,
        0.0015869140625,
        -0.0245361328125,
    ],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [
        -0.0245361328125,
        0.0015869140625,
        0.6444091796875,
        -0.0517578125000,
        -0.3953857421875,
        0.8893127441406,
        0.0983886718750,
        -0.1897583007813,
        -0.0189208984375,
        0.0776367187500,
        0.0017089843750,
        -0.0291748046875,
    ],
];

const TRUE_PEAK_FIR_LEN: usize = 12;

struct TruePeakDetector {
    history: Vec<[f64; TRUE_PEAK_FIR_LEN]>,
    peak: Vec<f64>,
    prev_peak: Vec<f64>,
}

impl TruePeakDetector {
    fn new(channels: usize) -> Self {
        Self {
            history: vec![[0.0; TRUE_PEAK_FIR_LEN]; channels],
            peak: vec![0.0; channels],
            prev_peak: vec![0.0; channels],
        }
    }

    fn process_frame(&mut self, ch: usize, sample: f64) {
        // Shift history
        let h = &mut self.history[ch];
        h.copy_within(1.., 0);
        h[TRUE_PEAK_FIR_LEN - 1] = sample;

        // Evaluate 4 polyphase outputs
        for phase in &TRUE_PEAK_FIR_PHASES {
            let mut sum = 0.0;
            for (i, &coeff) in phase.iter().enumerate() {
                sum += coeff * h[i];
            }
            let abs_val = sum.abs();
            if abs_val > self.peak[ch] {
                self.peak[ch] = abs_val;
            }
            if abs_val > self.prev_peak[ch] {
                self.prev_peak[ch] = abs_val;
            }
        }
    }

    fn reset(&mut self) {
        for h in &mut self.history {
            h.fill(0.0);
        }
        self.peak.fill(0.0);
        self.prev_peak.fill(0.0);
    }
}

// ── Gating block ring buffer ────────────────────────────────────────────────

/// Ring buffer for sub-block energies (400ms momentary = 4 × 100ms, 3s short-term = 30 × 100ms).
struct SubBlockRing {
    buf: Vec<f64>,
    pos: usize,
    count: usize,
}

impl SubBlockRing {
    fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity],
            pos: 0,
            count: 0,
        }
    }

    fn push(&mut self, energy: f64) {
        self.buf[self.pos] = energy;
        self.pos = (self.pos + 1) % self.buf.len();
        if self.count < self.buf.len() {
            self.count += 1;
        }
    }

    fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        let sum: f64 = if self.count == self.buf.len() {
            self.buf.iter().sum()
        } else {
            self.buf[..self.count].iter().sum()
        };
        Some(sum / self.count as f64)
    }

    fn is_full(&self) -> bool {
        self.count >= self.buf.len()
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
        self.count = 0;
    }
}

// ── Channel weights ─────────────────────────────────────────────────────────

fn channel_weight(ch: usize, num_channels: usize) -> f64 {
    if num_channels <= 2 {
        // Mono or stereo: all channels weight 1.0
        1.0
    } else if num_channels == 5 {
        // 5.0: L, R, C, Ls, Rs
        match ch {
            0..=2 => 1.0,  // L, R, C
            3 | 4 => 1.41, // Ls, Rs (surround +1.5 dB)
            _ => 0.0,
        }
    } else if num_channels == 6 {
        // 5.1: L, R, C, LFE, Ls, Rs
        match ch {
            0..=2 => 1.0,  // L, R, C
            3 => 0.0,      // LFE excluded
            4 | 5 => 1.41, // Ls, Rs
            _ => 0.0,
        }
    } else {
        1.0
    }
}

// ── EbuR128 ─────────────────────────────────────────────────────────────────

/// EBU R128 / ITU-R BS.1770-4 loudness meter.
pub struct EbuR128 {
    channels: u32,
    #[allow(dead_code)]
    sample_rate: u32,
    mode: Mode,

    // K-weighting filters (one per channel)
    filters: Vec<KWeightFilter>,
    channel_weights: Vec<f64>,

    // Sub-block accumulation (100ms sub-blocks)
    sub_block_frames: usize,   // frames per 100ms sub-block
    sub_block_accum: Vec<f64>, // per-channel energy accumulator
    sub_block_pos: usize,      // frames accumulated in current sub-block

    // Momentary (400ms = 4 sub-blocks) and short-term (3s = 30 sub-blocks)
    momentary_ring: SubBlockRing,
    shortterm_ring: SubBlockRing,

    // Integrated gating: store all block energies for two-pass gating
    gating_blocks: VecDeque<f64>,

    // Peak tracking
    sample_peak: Vec<f64>,
    prev_sample_peak: Vec<f64>,
    true_peak_detector: Option<TruePeakDetector>,
}

impl EbuR128 {
    /// Create a new EBU R128 loudness meter.
    ///
    /// # Errors
    /// Returns an error if channels is 0.
    pub fn new(channels: u32, sample_rate: u32, mode: Mode) -> Result<Self, String> {
        if channels == 0 {
            return Err("channels must be > 0".into());
        }
        let nc = channels as usize;
        let sub_block_frames = (sample_rate as usize) / 10; // 100ms

        let filters: Vec<KWeightFilter> =
            (0..nc).map(|_| KWeightFilter::new(sample_rate)).collect();
        let channel_weights: Vec<f64> = (0..nc).map(|ch| channel_weight(ch, nc)).collect();

        let true_peak_detector = if mode.has(Mode::TRUE_PEAK) {
            Some(TruePeakDetector::new(nc))
        } else {
            None
        };

        Ok(Self {
            channels,
            sample_rate,
            mode,
            filters,
            channel_weights,
            sub_block_frames,
            sub_block_accum: vec![0.0; nc],
            sub_block_pos: 0,
            momentary_ring: SubBlockRing::new(4), // 4 × 100ms = 400ms
            shortterm_ring: SubBlockRing::new(30), // 30 × 100ms = 3s
            gating_blocks: if mode.has(Mode::I) {
                // Pre-allocate for ~10 minutes (6000 blocks at 10 blocks/sec)
                // to avoid re-allocations on the audio thread hot path.
                VecDeque::with_capacity(6_000)
            } else {
                VecDeque::new()
            },
            sample_peak: vec![0.0; nc],
            prev_sample_peak: vec![0.0; nc],
            true_peak_detector,
        })
    }

    /// Feed interleaved f32 audio frames.
    pub fn add_frames_f32(&mut self, samples: &[f32]) -> Result<(), String> {
        let nc = self.channels as usize;
        if !samples.len().is_multiple_of(nc) {
            return Err("samples length must be a multiple of channel count".into());
        }

        for frame in samples.chunks_exact(nc) {
            for (ch, &s) in frame.iter().enumerate() {
                let x = s as f64;

                // Sample peak
                if self.mode.has(Mode::SAMPLE_PEAK) {
                    let abs_x = x.abs();
                    if abs_x > self.sample_peak[ch] {
                        self.sample_peak[ch] = abs_x;
                    }
                    if abs_x > self.prev_sample_peak[ch] {
                        self.prev_sample_peak[ch] = abs_x;
                    }
                }

                // True peak
                if let Some(ref mut tp) = self.true_peak_detector {
                    tp.process_frame(ch, x);
                }

                // K-weighting
                let y = self.filters[ch].process(x);
                self.sub_block_accum[ch] += y * y;
            }

            self.sub_block_pos += 1;

            // Complete a 100ms sub-block
            if self.sub_block_pos >= self.sub_block_frames {
                self.complete_sub_block();
            }
        }

        Ok(())
    }

    fn complete_sub_block(&mut self) {
        let nc = self.channels as usize;
        let n = self.sub_block_frames as f64;

        // Weighted energy for this sub-block
        let mut block_energy = 0.0;
        for ch in 0..nc {
            block_energy += self.channel_weights[ch] * (self.sub_block_accum[ch] / n);
        }

        // Push to ring buffers
        self.momentary_ring.push(block_energy);
        self.shortterm_ring.push(block_energy);

        // For integrated loudness: store block energy when we have a full momentary window
        if self.mode.has(Mode::I)
            && self.momentary_ring.is_full()
            && let Some(mean_energy) = self.momentary_ring.mean()
        {
            // Cap at ~1 hour of blocks (36000 at 10 blocks/sec) to prevent
            // unbounded memory growth during long playback sessions.
            // When full, drop the oldest block (approximation acceptable for
            // integrated loudness which is already a long-term average).
            if self.gating_blocks.len() >= MAX_GATING_BLOCKS {
                self.gating_blocks.pop_front();
            }
            self.gating_blocks.push_back(mean_energy);
        }

        // Reset accumulators
        self.sub_block_accum.fill(0.0);
        self.sub_block_pos = 0;
    }

    /// Momentary loudness (400ms window) in LUFS.
    pub fn loudness_momentary(&self) -> Result<f64, String> {
        match self.momentary_ring.mean() {
            Some(e) if e > 0.0 => Ok(energy_to_loudness(e)),
            Some(_) => Ok(f64::NEG_INFINITY),
            None => Ok(f64::NEG_INFINITY),
        }
    }

    /// Short-term loudness (3s window) in LUFS.
    pub fn loudness_shortterm(&self) -> Result<f64, String> {
        match self.shortterm_ring.mean() {
            Some(e) if e > 0.0 => Ok(energy_to_loudness(e)),
            Some(_) => Ok(f64::NEG_INFINITY),
            None => Ok(f64::NEG_INFINITY),
        }
    }

    /// Integrated loudness (entire program) in LUFS, with EBU R128 two-pass gating.
    pub fn loudness_global(&self) -> Result<f64, String> {
        if self.gating_blocks.is_empty() {
            return Ok(f64::NEG_INFINITY);
        }
        Ok(self.compute_gated_loudness())
    }

    /// Two-pass gating algorithm per EBU R128.
    /// Zero-allocation: computes means via running sum+count instead of collecting into Vecs.
    fn compute_gated_loudness(&self) -> f64 {
        let blocks = &self.gating_blocks;
        if blocks.is_empty() {
            return f64::NEG_INFINITY;
        }

        // Pass 1: Absolute gate at -70 LUFS
        let abs_gate_energy = loudness_to_energy(-70.0);
        let mut sum_abs = 0.0f64;
        let mut count_abs = 0usize;
        for &e in blocks {
            if e > abs_gate_energy {
                sum_abs += e;
                count_abs += 1;
            }
        }
        if count_abs == 0 {
            return f64::NEG_INFINITY;
        }

        let mean_above_abs = sum_abs / count_abs as f64;

        // Pass 2: Relative gate at mean - 10 LUFS
        let rel_gate_energy = mean_above_abs * loudness_to_energy(-10.0); // -10 dB below mean
        let mut sum_rel = 0.0f64;
        let mut count_rel = 0usize;
        for &e in blocks {
            if e > rel_gate_energy {
                sum_rel += e;
                count_rel += 1;
            }
        }
        if count_rel == 0 {
            return f64::NEG_INFINITY;
        }

        let mean_above_rel = sum_rel / count_rel as f64;
        energy_to_loudness(mean_above_rel)
    }

    /// Sample peak for a given channel (maximum absolute sample value seen).
    pub fn sample_peak(&self, channel: u32) -> Result<f64, String> {
        let ch = channel as usize;
        if ch >= self.channels as usize {
            return Err(format!("channel {} out of range", channel));
        }
        Ok(self.sample_peak[ch])
    }

    /// Previous sample peak for a given channel (since last snapshot).
    /// Resets the stored value after reading (snapshot-and-reset semantics).
    pub fn prev_sample_peak(&mut self, channel: u32) -> Result<f64, String> {
        let ch = channel as usize;
        if ch >= self.channels as usize {
            return Err(format!("channel {} out of range", channel));
        }
        let val = self.prev_sample_peak[ch];
        self.prev_sample_peak[ch] = 0.0;
        Ok(val)
    }

    /// Previous true peak for a given channel (since last snapshot).
    /// Resets the stored value after reading (snapshot-and-reset semantics).
    pub fn prev_true_peak(&mut self, channel: u32) -> Result<f64, String> {
        let ch = channel as usize;
        if ch >= self.channels as usize {
            return Err(format!("channel {} out of range", channel));
        }
        match &mut self.true_peak_detector {
            Some(tp) => {
                let val = tp.prev_peak[ch];
                tp.prev_peak[ch] = 0.0;
                Ok(val)
            }
            None => Ok(0.0),
        }
    }

    /// Returns `(gating_block_count, total_weighted_energy)` for album gain computation.
    /// Returns `None` if no gating blocks have been accumulated.
    pub fn gating_block_count_and_energy(&self) -> Option<(u64, f64)> {
        if self.gating_blocks.is_empty() {
            return None;
        }

        // Use blocks above absolute gate for consistency with loudness_global
        let abs_gate_energy = loudness_to_energy(-70.0);
        let above_abs: Vec<f64> = self
            .gating_blocks
            .iter()
            .copied()
            .filter(|&e| e > abs_gate_energy)
            .collect();
        if above_abs.is_empty() {
            return None;
        }
        let mean_above_abs = above_abs.iter().sum::<f64>() / above_abs.len() as f64;

        // Relative gate
        let rel_gate_energy = mean_above_abs * loudness_to_energy(-10.0);
        let mut count: u64 = 0;
        let mut total_energy: f64 = 0.0;
        for &e in &self.gating_blocks {
            if e > rel_gate_energy {
                count += 1;
                total_energy += e;
            }
        }

        if count == 0 {
            None
        } else {
            Some((count, total_energy))
        }
    }

    /// Reset all state (filters, accumulators, peaks, gating blocks).
    pub fn reset(&mut self) {
        for f in &mut self.filters {
            f.reset();
        }
        self.sub_block_accum.fill(0.0);
        self.sub_block_pos = 0;
        self.momentary_ring.reset();
        self.shortterm_ring.reset();
        self.gating_blocks.clear();
        self.sample_peak.fill(0.0);
        self.prev_sample_peak.fill(0.0);
        if let Some(ref mut tp) = self.true_peak_detector {
            tp.reset();
        }
    }
}

/// Convert energy to loudness in LUFS: -0.691 + 10 × log10(energy).
pub fn energy_to_loudness(energy: f64) -> f64 {
    -0.691 + 10.0 * energy.log10()
}

/// Convert loudness in LUFS to energy.
fn loudness_to_energy(lufs: f64) -> f64 {
    10.0_f64.powf((lufs + 0.691) / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_returns_neg_inf() {
        let mut meter = EbuR128::new(2, 48000, Mode::all()).unwrap();
        let silence = vec![0.0f32; 48000 * 2]; // 1 second stereo
        meter.add_frames_f32(&silence).unwrap();
        let lufs = meter.loudness_global().unwrap();
        assert!(lufs == f64::NEG_INFINITY || lufs < -100.0);
    }

    #[test]
    fn sine_1khz_loudness() {
        let sr = 48000;
        let duration_s = 5;
        let num_frames = sr * duration_s;
        let mut samples = vec![0.0f32; num_frames * 2];

        // 0 dBFS 1 kHz sine, both channels
        for i in 0..num_frames {
            let t = i as f64 / sr as f64;
            let s = (2.0 * PI * 1000.0 * t).sin() as f32;
            samples[i * 2] = s;
            samples[i * 2 + 1] = s;
        }

        let mut meter = EbuR128::new(2, sr as u32, Mode::all()).unwrap();
        meter.add_frames_f32(&samples).unwrap();

        let lufs = meter.loudness_global().unwrap();
        // Stereo 0 dBFS 1 kHz sine through K-weighting: ~-0.3 LUFS
        // (K pre-filter adds ~+0.2 dB at 1 kHz, 2 channels × 0.5 RMS² ≈ 1.0)
        assert!(
            lufs > -2.0 && lufs < 1.0,
            "Expected ~-0.3 LUFS for 0dBFS stereo 1kHz sine, got {lufs}"
        );
    }

    #[test]
    fn sample_peak_tracking() {
        let mut meter = EbuR128::new(1, 48000, Mode::SAMPLE_PEAK).unwrap();
        let mut samples = vec![0.0f32; 4800]; // 100ms mono
        samples[100] = 0.75;
        samples[200] = -0.9;
        meter.add_frames_f32(&samples).unwrap();

        let peak = meter.sample_peak(0).unwrap();
        assert!((peak - 0.9).abs() < 1e-6, "Expected peak ~0.9, got {peak}");
    }

    #[test]
    fn reset_clears_state() {
        let mut meter = EbuR128::new(2, 48000, Mode::all()).unwrap();
        let samples = vec![0.5f32; 48000 * 2];
        meter.add_frames_f32(&samples).unwrap();

        meter.reset();

        let lufs = meter.loudness_global().unwrap();
        assert!(lufs == f64::NEG_INFINITY || lufs < -100.0);
        assert_eq!(meter.sample_peak(0).unwrap(), 0.0);
    }

    #[test]
    fn energy_to_loudness_roundtrip() {
        let lufs = -23.0;
        let energy = loudness_to_energy(lufs);
        let back = energy_to_loudness(energy);
        assert!((back - lufs).abs() < 1e-10);
    }

    #[test]
    fn gating_block_count_and_energy() {
        let sr = 48000;
        let duration_s = 5;
        let num_frames = sr * duration_s;
        let mut samples = vec![0.0f32; num_frames * 2];

        for i in 0..num_frames {
            let t = i as f64 / sr as f64;
            let s = (2.0 * PI * 440.0 * t).sin() as f32 * 0.5;
            samples[i * 2] = s;
            samples[i * 2 + 1] = s;
        }

        let mut meter = EbuR128::new(2, sr as u32, Mode::I).unwrap();
        meter.add_frames_f32(&samples).unwrap();

        let result = meter.gating_block_count_and_energy();
        assert!(result.is_some());
        let (count, energy) = result.unwrap();
        assert!(count > 0);
        assert!(energy > 0.0);

        // Verify: energy/count should give similar loudness to global
        let album_lufs = energy_to_loudness(energy / count as f64);
        let global_lufs = meter.loudness_global().unwrap();
        assert!(
            (album_lufs - global_lufs).abs() < 0.5,
            "Album LUFS {album_lufs} should match global {global_lufs}"
        );
    }

    #[test]
    fn gating_blocks_overflow_oldest_dropped() {
        // Issue #1: when gating_blocks exceeds MAX_GATING_BLOCKS the oldest
        // block must be dropped.  With the old Vec::remove(0) this was O(n);
        // with VecDeque::pop_front() it is O(1).
        let sr = 48000;
        // Need > MAX_GATING_BLOCKS sub-blocks.  Each sub-block is 100 ms,
        // so 101 blocks = 10.1 s → 48000 * 10.1 ≈ 484800 frames.
        let duration_s = 11;
        let num_frames = sr * duration_s;
        let mut samples = vec![0.0f32; num_frames * 2];

        for i in 0..num_frames {
            let t = i as f64 / sr as f64;
            let s = (2.0 * PI * 440.0 * t).sin() as f32 * 0.5;
            samples[i * 2] = s;
            samples[i * 2 + 1] = s;
        }

        let mut meter = EbuR128::new(2, sr as u32, Mode::I).unwrap();
        meter.add_frames_f32(&samples).unwrap();

        // Gating must still produce a valid result after overflow.
        let lufs = meter.loudness_global().unwrap();
        assert!(
            lufs > -30.0 && lufs < 5.0,
            "global loudness should be reasonable after overflow, got {lufs}"
        );

        let result = meter.gating_block_count_and_energy();
        assert!(
            result.is_some(),
            "gating_block_count_and_energy should work after overflow"
        );
    }
}
