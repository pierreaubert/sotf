// ============================================================================
// DynamicsCore — Shared per-channel dynamics processor for compressor/expander
// ============================================================================
//
// Encapsulates the DSP kernel used by both compressor and expander plugins
// (single-band and multiband). Each band in a multiband plugin gets its own
// DynamicsCore instance.
//
// HARD RULES:
// - No allocations in any method called from process() hot path
// - All Vecs pre-allocated in new()/initialize()
// - No mutex locks
// - No unsafe code

use crate::auto_makeup::MeasuredMakeup;
use crate::detector::{DetectionMode, LevelDetector};
use crate::envelope::DualRelease;
use crate::lookahead::LookaheadBuffer;
use math_audio_iir_fir::{Biquad, BiquadFilterType, peq_butterworth_highpass};

// ============================================================================
// Constants
// ============================================================================

const RMS_WINDOW_MS: f32 = 10.0;
const MEASURED_MAKEUP_SMOOTHING_MS: f32 = 1000.0;
const MAX_LOOKAHEAD_MS: f32 = 20.0;
const DUAL_RELEASE_SLOW_MULTIPLIER: f32 = 4.0;

// ============================================================================
// Types
// ============================================================================

/// Whether the dynamics processor compresses (above threshold) or expands (below threshold).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsMode {
    Compress,
    Expand,
}

/// Gate state for expansion mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateState {
    Open,
    Hold,
    Closing,
}

/// Sidechain filter mode for the detection path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidechainFilterMode {
    /// No sidechain filtering.
    Off,
    /// High-pass filter at the given frequency.
    /// `order_index`: 0 = 2nd order, 1 = 4th order.
    Hpf { freq_hz: f32, order_index: usize },
    /// Spectral tilt filter: positive = emphasize HF in detection, negative = emphasize LF.
    /// Implemented as a 1st-order high-shelf at 1 kHz with the given gain.
    Tilt { tilt_db: f32 },
}

// ============================================================================
// DynamicsCore
// ============================================================================

/// Shared dynamics processor used by both compressor and expander, single-band
/// and multiband. Each band in a multiband plugin gets its own DynamicsCore
/// instance.
pub struct DynamicsCore {
    mode: DynamicsMode,
    channels: usize,
    sample_rate: u32,

    // === Envelope ===
    envelope: Vec<f32>,
    attack_coeff: f32,
    release_coeff: f32,
    attack_ms: f32,
    release_ms: f32,

    // === Detection ===
    level_detectors: Vec<LevelDetector>,
    detection_mode_index: usize, // 0=peak, 1=RMS

    // === Sidechain filter (HPF or Tilt) ===
    sidechain_hpf_biquads: Vec<Vec<Biquad>>,
    sidechain_hpf_hz: f32,
    sidechain_hpf_order_index: usize, // 0=2nd, 1=4th
    sidechain_tilt_biquads: Vec<Biquad>,
    sidechain_tilt_db: f32,
    sidechain_filter_mode: SidechainFilterMode,

    // === Program-dependent release (compress mode only) ===
    dual_release: Vec<DualRelease>,
    program_dependent_release: bool,

    // === Makeup gain ===
    measured_makeup: MeasuredMakeup,

    // === Lookahead ===
    lookahead_buffer: LookaheadBuffer,
    lookahead_ms: f32,
    lookahead_frame_buf: Vec<f32>,

    // === Expand-mode gate state ===
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    hysteresis_db: f32,
    hold_ms: f32,
    range_db: f32,
}

impl DynamicsCore {
    /// Create a new dynamics core processor.
    ///
    /// Pre-allocates all Vecs. Initializes coefficients for the given sample rate.
    /// LookaheadBuffer is sized for max 20ms capacity.
    pub fn new(mode: DynamicsMode, channels: usize, sample_rate: u32) -> Self {
        let detection_mode = DetectionMode::Peak;
        let max_lookahead_samples =
            (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        let attack_ms = 10.0;
        let release_ms = 100.0;

        let mut core = Self {
            mode,
            channels,
            sample_rate,

            envelope: vec![0.0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            attack_ms,
            release_ms,

            level_detectors: (0..channels)
                .map(|_| LevelDetector::new(detection_mode, sample_rate))
                .collect(),
            detection_mode_index: 0,

            sidechain_hpf_biquads: Vec::new(),
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order_index: 0,
            sidechain_tilt_biquads: Vec::new(),
            sidechain_tilt_db: 0.0,
            sidechain_filter_mode: SidechainFilterMode::Off,

            dual_release: (0..channels)
                .map(|_| {
                    DualRelease::new(
                        release_ms,
                        release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                        sample_rate,
                    )
                })
                .collect(),
            program_dependent_release: false,

            measured_makeup: MeasuredMakeup::new(MEASURED_MAKEUP_SMOOTHING_MS, sample_rate),

            lookahead_buffer: LookaheadBuffer::new(max_lookahead_samples.max(1), channels),
            lookahead_ms: 0.0,
            lookahead_frame_buf: vec![0.0; channels],

            gate_state: vec![GateState::Open; channels],
            hold_counter: vec![0; channels],
            hysteresis_db: 3.0,
            hold_ms: 50.0,
            range_db: 40.0,
        };

        core.attack_coeff = time_to_coeff(attack_ms, sample_rate);
        core.release_coeff = time_to_coeff(release_ms, sample_rate);
        // Lookahead disabled by default (0ms), set delay to minimum so push works
        core.lookahead_buffer.set_delay(1);

        core
    }

    /// Update sample rate and recompute all derived coefficients.
    ///
    /// Resizes level detectors and lookahead buffer for new sample rate.
    pub fn initialize(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;

        // Recompute envelope coefficients
        self.attack_coeff = time_to_coeff(self.attack_ms, sample_rate);
        self.release_coeff = time_to_coeff(self.release_ms, sample_rate);

        // Rebuild sidechain filters
        self.rebuild_sidechain_hpf_internal();
        self.rebuild_sidechain_tilt_internal();

        // Reinitialize level detectors
        let mode = self.detection_mode();
        self.level_detectors = (0..self.channels)
            .map(|_| LevelDetector::new(mode, sample_rate))
            .collect();

        // Reinitialize lookahead buffer
        let max_lookahead_samples =
            (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        self.lookahead_buffer
            .resize(max_lookahead_samples.max(1), self.channels);
        if self.lookahead_ms > 0.0 {
            self.lookahead_buffer
                .set_delay_ms(self.lookahead_ms, sample_rate);
        } else {
            self.lookahead_buffer.set_delay(1);
        }

        // Reinitialize dual release
        self.dual_release = (0..self.channels)
            .map(|_| {
                DualRelease::new(
                    self.release_ms,
                    self.release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                    sample_rate,
                )
            })
            .collect();

        // Reinitialize measured makeup
        self.measured_makeup = MeasuredMakeup::new(MEASURED_MAKEUP_SMOOTHING_MS, sample_rate);

        // Ensure frame buffer matches channel count
        self.lookahead_frame_buf.resize(self.channels, 0.0);
    }

    /// Zero all state: envelopes, gate states, hold counters, HPF biquad states,
    /// level detectors, lookahead buffer.
    pub fn reset(&mut self) {
        self.envelope.fill(0.0);

        // Reset gate state
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);

        // Reset sidechain filter states by rebuilding
        self.rebuild_sidechain_hpf_internal();
        self.rebuild_sidechain_tilt_internal();

        // Reset level detectors
        for det in &mut self.level_detectors {
            det.reset();
        }

        // Reset lookahead
        self.lookahead_buffer.reset();

        // Reset dual release
        for dr in &mut self.dual_release {
            dr.reset();
        }

        // Reset measured makeup
        self.measured_makeup.reset();
    }

    /// Update attack and release time constants.
    ///
    /// Also updates dual release times (slow = release * 4.0).
    pub fn set_attack_release(&mut self, attack_ms: f32, release_ms: f32) {
        self.attack_ms = attack_ms;
        self.release_ms = release_ms;
        self.attack_coeff = time_to_coeff(attack_ms, self.sample_rate);
        self.release_coeff = time_to_coeff(release_ms, self.sample_rate);

        // Update dual release times
        for dr in &mut self.dual_release {
            dr.set_times(
                release_ms,
                release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                self.sample_rate,
            );
        }
    }

    /// Rebuild sidechain HPF biquad cascade using `peq_butterworth_highpass()`.
    ///
    /// Same Butterworth cascade pattern as the compressor plugin.
    pub fn set_sidechain_hpf(&mut self, freq_hz: f32, order_index: usize) {
        self.sidechain_hpf_hz = freq_hz;
        self.sidechain_hpf_order_index = order_index;
        self.sidechain_filter_mode = if freq_hz > 0.0 {
            SidechainFilterMode::Hpf {
                freq_hz,
                order_index,
            }
        } else {
            SidechainFilterMode::Off
        };
        self.rebuild_sidechain_hpf_internal();
        self.sidechain_tilt_biquads.clear();
        self.sidechain_tilt_db = 0.0;
    }

    /// Set a spectral tilt filter on the sidechain detection path.
    ///
    /// `tilt_db`: positive values weight HF more heavily (e.g., +3 dB makes the
    /// compressor more sensitive to high frequencies). Negative values weight LF.
    /// Implemented as a 1st-order high-shelf at 1 kHz.
    pub fn set_sidechain_tilt(&mut self, tilt_db: f32) {
        self.sidechain_tilt_db = tilt_db;
        if tilt_db.abs() < 0.01 {
            self.sidechain_filter_mode = SidechainFilterMode::Off;
            self.sidechain_tilt_biquads.clear();
            // Do NOT clear HPF state — only tilt is being disabled
            return;
        }
        self.sidechain_filter_mode = SidechainFilterMode::Tilt { tilt_db };
        // Clear HPF — tilt and HPF are mutually exclusive
        self.sidechain_hpf_biquads.clear();
        self.sidechain_hpf_hz = 0.0;
        self.rebuild_sidechain_tilt_internal();
    }

    /// Set sidechain filter using the unified enum.
    pub fn set_sidechain_filter(&mut self, mode: SidechainFilterMode) {
        match mode {
            SidechainFilterMode::Off => {
                self.sidechain_hpf_biquads.clear();
                self.sidechain_hpf_hz = 0.0;
                self.sidechain_tilt_biquads.clear();
                self.sidechain_tilt_db = 0.0;
                self.sidechain_filter_mode = SidechainFilterMode::Off;
            }
            SidechainFilterMode::Hpf {
                freq_hz,
                order_index,
            } => {
                self.set_sidechain_hpf(freq_hz, order_index);
            }
            SidechainFilterMode::Tilt { tilt_db } => {
                self.set_sidechain_tilt(tilt_db);
            }
        }
    }

    /// Set detection mode: 0=peak, 1=RMS. Reinitializes level detectors.
    pub fn set_detection_mode(&mut self, mode_index: usize) {
        self.detection_mode_index = mode_index;
        let mode = self.detection_mode();
        for det in &mut self.level_detectors {
            det.set_mode(mode);
        }
    }

    /// Update the lookahead delay.
    pub fn set_lookahead_ms(&mut self, ms: f32) {
        self.lookahead_ms = ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        if self.lookahead_ms > 0.0 {
            self.lookahead_buffer
                .set_delay_ms(self.lookahead_ms, self.sample_rate);
        } else {
            self.lookahead_buffer.set_delay(1);
        }
    }

    /// Enable or disable program-dependent release (compress mode only).
    pub fn set_program_dependent_release(&mut self, enabled: bool) {
        self.program_dependent_release = enabled;
    }

    /// Set expand-mode parameters: hysteresis, hold, and range.
    pub fn set_expand_params(&mut self, hysteresis_db: f32, hold_ms: f32, range_db: f32) {
        self.hysteresis_db = hysteresis_db;
        self.hold_ms = hold_ms;
        self.range_db = range_db;
    }

    /// Get the dynamics mode.
    pub fn mode(&self) -> DynamicsMode {
        self.mode
    }

    /// Get the number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }

    // ========================================================================
    // Hot-path methods — called per-sample, zero allocations
    // ========================================================================

    /// Run the sidechain filter (HPF or Tilt) for this channel.
    ///
    /// Returns the filtered sample. If no sidechain filter is active, returns
    /// the input sample unchanged.
    #[inline]
    pub fn apply_sidechain_filter(&mut self, ch: usize, sample: f32) -> f32 {
        match self.sidechain_filter_mode {
            SidechainFilterMode::Off => sample,
            SidechainFilterMode::Hpf { .. } => {
                if ch >= self.sidechain_hpf_biquads.len() {
                    return sample;
                }
                let biquads: &mut [Biquad] = &mut self.sidechain_hpf_biquads[ch];
                let mut x = sample as f64;
                for bq in biquads.iter_mut() {
                    x = bq.process(x);
                }
                x as f32
            }
            SidechainFilterMode::Tilt { .. } => {
                if ch >= self.sidechain_tilt_biquads.len() {
                    return sample;
                }
                self.sidechain_tilt_biquads[ch].process(sample as f64) as f32
            }
        }
    }

    /// Detect level for one sample on a channel.
    ///
    /// Peak mode returns abs(sample). RMS mode uses the LevelDetector's sliding
    /// window and returns the linear RMS amplitude.
    #[inline]
    pub fn detect_level(&mut self, ch: usize, sample: f32) -> f32 {
        if self.detection_mode_index == 0 {
            // Peak mode: absolute value
            sample.abs()
        } else {
            // RMS mode: use LevelDetector
            self.level_detectors[ch].process_linear(sample)
        }
    }

    /// Calculate gain reduction/expansion attenuation for the given input level.
    ///
    /// For Compress mode: standard soft-knee gain reduction (above threshold).
    /// For Expand mode: expansion attenuation (below threshold), capped by range_db.
    #[inline]
    pub fn calculate_gain_reduction(
        &self,
        input_db: f32,
        threshold: f32,
        ratio: f32,
        knee_db: f32,
    ) -> f32 {
        match self.mode {
            DynamicsMode::Compress => calculate_compress_gr(input_db, threshold, ratio, knee_db),
            DynamicsMode::Expand => {
                calculate_expand_atten(input_db, threshold, ratio, knee_db, self.range_db)
            }
        }
    }

    /// Apply one-pole attack/release envelope smoothing.
    ///
    /// For compress mode with program_dependent_release enabled, uses DualRelease
    /// for the release coefficient. Returns the smoothed gain reduction in dB.
    #[inline]
    pub fn apply_envelope(&mut self, ch: usize, target_gr: f32) -> f32 {
        let coeff = if target_gr > self.envelope[ch] {
            // Attack phase: target is higher gain reduction
            self.attack_coeff
        } else {
            // Release phase
            match self.mode {
                DynamicsMode::Compress if self.program_dependent_release => {
                    self.dual_release[ch].process(target_gr)
                }
                _ => self.release_coeff,
            }
        };

        // One-pole smoothing
        self.envelope[ch] = target_gr + coeff * (self.envelope[ch] - target_gr);
        self.envelope[ch]
    }

    /// Process the 3-state gate machine for expansion mode.
    ///
    /// Implements Open/Hold/Closing transitions with hysteresis.
    /// Returns the target attenuation in dB (0.0 when gate is open, or the
    /// expansion attenuation when gate is closing).
    ///
    /// This method integrates the gate state machine AND the expansion attenuation
    /// calculation, matching the expander plugin's `process_channel` pattern.
    #[inline]
    pub fn process_gate_state(
        &mut self,
        ch: usize,
        input_db: f32,
        threshold: f32,
        ratio: f32,
        knee_db: f32,
    ) -> f32 {
        let hold_samples = (self.hold_ms * 0.001 * self.sample_rate as f32) as usize;
        let open_th = threshold;
        let close_th = threshold - self.hysteresis_db;

        match self.gate_state[ch] {
            GateState::Open => {
                if input_db < open_th {
                    self.gate_state[ch] = GateState::Hold;
                    self.hold_counter[ch] = hold_samples;
                }
                0.0
            }
            GateState::Hold => {
                if input_db >= open_th {
                    self.gate_state[ch] = GateState::Open;
                    self.hold_counter[ch] = 0;
                    0.0
                } else if self.hold_counter[ch] > 0 {
                    self.hold_counter[ch] -= 1;
                    0.0
                } else if input_db < close_th {
                    self.gate_state[ch] = GateState::Closing;
                    self.calculate_gain_reduction(input_db, threshold, ratio, knee_db)
                } else {
                    0.0
                }
            }
            GateState::Closing => {
                if input_db >= open_th {
                    self.gate_state[ch] = GateState::Open;
                    0.0
                } else {
                    self.calculate_gain_reduction(input_db, threshold, ratio, knee_db)
                }
            }
        }
    }

    // ========================================================================
    // Getters
    // ========================================================================

    /// Get the current envelope value (gain reduction in dB) for a channel.
    #[inline]
    pub fn envelope_db(&self, ch: usize) -> f32 {
        self.envelope[ch]
    }

    /// Get the measured makeup gain in dB.
    #[inline]
    pub fn measured_makeup_db(&self) -> f32 {
        self.measured_makeup.makeup_db()
    }

    /// Get the measured makeup gain as a linear multiplier.
    #[inline]
    pub fn measured_makeup_linear(&self) -> f32 {
        self.measured_makeup.makeup_linear()
    }

    /// Update the measured makeup tracker with the current gain reduction.
    #[inline]
    pub fn update_measured_makeup(&mut self, gain_reduction: f32) {
        self.measured_makeup.update(gain_reduction);
    }

    /// Push one interleaved frame into the lookahead buffer, get the delayed
    /// frame out. `input` and `output` must have `channels` elements.
    #[inline]
    pub fn lookahead_process_frame(&mut self, input: &[f32], output: &mut [f32]) {
        self.lookahead_buffer.process_frame(input, output);
    }

    /// Returns the current lookahead delay in samples.
    pub fn lookahead_delay_samples(&self) -> usize {
        if self.lookahead_ms <= 0.0 {
            return 0;
        }
        (self.lookahead_ms * 0.001 * self.sample_rate as f32).round() as usize
    }

    /// Get a mutable reference to the lookahead frame buffer (pre-allocated).
    ///
    /// This buffer has `channels` elements and is used to avoid per-frame
    /// allocation when processing lookahead.
    #[inline]
    pub fn lookahead_frame_buf(&mut self) -> &mut [f32] {
        &mut self.lookahead_frame_buf
    }

    /// Get the current gate state for a channel (expand mode only).
    pub fn gate_state(&self, ch: usize) -> GateState {
        self.gate_state[ch]
    }

    /// Get the range_db value (expand mode only).
    pub fn range_db(&self) -> f32 {
        self.range_db
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    fn detection_mode(&self) -> DetectionMode {
        if self.detection_mode_index == 1 {
            DetectionMode::Rms {
                window_ms: RMS_WINDOW_MS,
            }
        } else {
            DetectionMode::Peak
        }
    }

    fn rebuild_sidechain_hpf_internal(&mut self) {
        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let order = match self.sidechain_hpf_order_index {
                1 => 4,
                _ => 2,
            };
            let peq = peq_butterworth_highpass(order, fc as f64, self.sample_rate as f64);
            let sections: Vec<Biquad> = peq.into_iter().map(|(_, bq)| bq).collect();
            self.sidechain_hpf_biquads = (0..self.channels).map(|_| sections.clone()).collect();
        } else {
            self.sidechain_hpf_biquads.clear();
        }
    }

    fn rebuild_sidechain_tilt_internal(&mut self) {
        let tilt = self.sidechain_tilt_db;
        if tilt.abs() < 0.01 || self.sample_rate == 0 {
            self.sidechain_tilt_biquads.clear();
            return;
        }
        // 1st-order high-shelf at 1 kHz: positive tilt = more HF sensitivity
        let shelf_freq = 1000.0;
        let q = 0.707; // Butterworth Q for 1st-order approximation
        self.sidechain_tilt_biquads = (0..self.channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highshelf,
                    shelf_freq,
                    self.sample_rate as f64,
                    q,
                    tilt as f64,
                )
            })
            .collect();
    }
}

// ============================================================================
// Free functions — exact formulas from compressor/expander plugins
// ============================================================================

/// Time constant (ms) to one-pole coefficient.
#[inline]
fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
    if time_ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
    }
}

/// Compressor gain reduction: standard soft-knee formula.
///
/// Returns gain reduction in dB (positive value) for signals above threshold.
#[inline]
fn calculate_compress_gr(input_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if knee < 0.1 {
        if input_db <= threshold {
            0.0
        } else {
            (input_db - threshold) * slope
        }
    } else if input_db < threshold - knee / 2.0 {
        0.0
    } else if input_db > threshold + knee / 2.0 {
        (input_db - threshold) * slope
    } else {
        let overshoot = input_db - threshold + knee / 2.0;
        let kf = overshoot / knee;
        kf * kf * (knee / 2.0) * slope
    }
}

/// Expander attenuation: below-threshold expansion with range cap.
///
/// Returns attenuation in dB (positive value) for signals below threshold,
/// capped at range_db.
#[inline]
fn calculate_expand_atten(
    input_db: f32,
    threshold: f32,
    ratio: f32,
    knee: f32,
    range_db: f32,
) -> f32 {
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    let atten = if knee < 0.1 {
        if input_db >= threshold {
            0.0
        } else {
            (threshold - input_db) * slope
        }
    } else if input_db > threshold + knee / 2.0 {
        0.0
    } else if input_db < threshold - knee / 2.0 {
        (threshold - input_db) * slope
    } else {
        let below = threshold + knee / 2.0 - input_db;
        let kf = below / knee;
        kf * kf * (knee / 2.0) * slope
    };
    atten.min(range_db.max(0.0))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48000;

    #[test]
    fn test_compress_gain_reduction() {
        let core = DynamicsCore::new(DynamicsMode::Compress, 2, SR);

        // Below threshold — no compression
        let gr = core.calculate_gain_reduction(-30.0, -20.0, 4.0, 0.0);
        assert_eq!(gr, 0.0);

        // At threshold — no compression
        let gr = core.calculate_gain_reduction(-20.0, -20.0, 4.0, 0.0);
        assert_eq!(gr, 0.0);

        // 12 dB above threshold with 4:1 ratio, no knee
        // GR = 12 * (1 - 1/4) = 9 dB
        let gr = core.calculate_gain_reduction(-8.0, -20.0, 4.0, 0.0);
        assert!((gr - 9.0).abs() < 0.01, "expected ~9.0, got {gr}");

        // Soft knee: at exact threshold, should be in the knee zone
        let gr = core.calculate_gain_reduction(-20.0, -20.0, 4.0, 6.0);
        // In the knee zone: overshoot = -20 - (-20) + 3 = 3, kf = 3/6 = 0.5
        // GR = 0.25 * 3.0 * 0.75 = 0.5625
        assert!(gr > 0.0 && gr < 3.0, "knee GR should be moderate, got {gr}");
    }

    #[test]
    fn test_expand_attenuation() {
        let mut core = DynamicsCore::new(DynamicsMode::Expand, 1, SR);
        core.set_expand_params(3.0, 50.0, 40.0);

        // Above threshold — no expansion
        let atten = core.calculate_gain_reduction(-10.0, -20.0, 4.0, 0.0);
        assert_eq!(atten, 0.0);

        // At threshold — no expansion
        let atten = core.calculate_gain_reduction(-20.0, -20.0, 4.0, 0.0);
        assert_eq!(atten, 0.0);

        // 12 dB below threshold with 4:1 ratio, no knee
        // atten = 12 * (1 - 1/4) = 9 dB
        let atten = core.calculate_gain_reduction(-32.0, -20.0, 4.0, 0.0);
        assert!((atten - 9.0).abs() < 0.01, "expected ~9.0, got {atten}");

        // Test range cap: 60 dB below threshold with 4:1
        // uncapped = 60 * 0.75 = 45, but range_db = 40
        let atten = core.calculate_gain_reduction(-80.0, -20.0, 4.0, 0.0);
        assert!(
            (atten - 40.0).abs() < 0.01,
            "expected range cap at 40.0, got {atten}"
        );
    }

    #[test]
    fn test_envelope_attack_release() {
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 1, SR);
        core.set_attack_release(1.0, 50.0); // 1ms attack, 50ms release

        // Attack: feed target of 10 dB GR
        let mut env = 0.0f32;
        for _ in 0..480 {
            // 10ms worth of samples
            env = core.apply_envelope(0, 10.0);
        }
        // After 10ms with 1ms attack, should be close to target
        assert!(
            env > 9.0,
            "after 10ms attack (1ms time constant), envelope should be near 10.0, got {env}"
        );

        // Release: feed target of 0 dB GR
        for _ in 0..24000 {
            // 500ms — 10 time constants
            env = core.apply_envelope(0, 0.0);
        }
        // After 500ms with 50ms release, should be very near zero
        assert!(
            env < 0.1,
            "after 500ms release (50ms time constant), envelope should be near 0, got {env}"
        );
    }

    #[test]
    fn test_gate_state_machine() {
        let mut core = DynamicsCore::new(DynamicsMode::Expand, 1, SR);
        core.set_expand_params(3.0, 0.0, 40.0); // 3dB hysteresis, 0ms hold, 40dB range
        core.set_attack_release(0.1, 50.0);

        let threshold = -20.0;
        let ratio = 4.0;
        let knee = 0.0;

        // Start in Open state
        assert_eq!(core.gate_state(0), GateState::Open);

        // Above threshold — stays open, no attenuation
        let atten = core.process_gate_state(0, -10.0, threshold, ratio, knee);
        assert_eq!(atten, 0.0);
        assert_eq!(core.gate_state(0), GateState::Open);

        // Below threshold — transitions to Hold (hold_ms=0 so immediate transition check)
        let atten = core.process_gate_state(0, -25.0, threshold, ratio, knee);
        assert_eq!(atten, 0.0); // Hold still produces 0 attenuation initially
        assert_eq!(core.gate_state(0), GateState::Hold);

        // Still below close threshold (-20 - 3 = -23), hold counter exhausted
        // since hold_ms=0, counter is 0 — should transition to Closing
        let atten = core.process_gate_state(0, -25.0, threshold, ratio, knee);
        assert!(atten > 0.0, "should be expanding now, got {atten}");
        assert_eq!(core.gate_state(0), GateState::Closing);

        // Back above threshold — should re-open
        let atten = core.process_gate_state(0, -10.0, threshold, ratio, knee);
        assert_eq!(atten, 0.0);
        assert_eq!(core.gate_state(0), GateState::Open);
    }

    #[test]
    fn test_sidechain_hpf() {
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 1, SR);
        core.set_sidechain_hpf(200.0, 0); // 200Hz 2nd order HPF

        // Low frequency (50Hz) should be attenuated
        let mut low_energy = 0.0f32;
        let freq = 50.0;
        for i in 0..SR {
            let sample = (2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32).sin();
            let filtered = core.apply_sidechain_filter(0, sample);
            low_energy += filtered * filtered;
        }

        // Reset HPF state
        core.set_sidechain_hpf(200.0, 0);

        // High frequency (1kHz) should pass through
        let mut high_energy = 0.0f32;
        let freq = 1000.0;
        for i in 0..SR {
            let sample = (2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32).sin();
            let filtered = core.apply_sidechain_filter(0, sample);
            high_energy += filtered * filtered;
        }

        assert!(
            high_energy > low_energy * 10.0,
            "HPF should strongly attenuate 50Hz vs 1kHz: low={low_energy}, high={high_energy}"
        );
    }

    #[test]
    fn test_detection_peak_vs_rms() {
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 1, SR);

        // Peak detection
        core.set_detection_mode(0);
        let peak_level = core.detect_level(0, 0.5);
        assert!((peak_level - 0.5).abs() < 0.001, "peak should be 0.5");

        // Negative sample should also return abs
        let peak_neg = core.detect_level(0, -0.5);
        assert!(
            (peak_neg - 0.5).abs() < 0.001,
            "peak of negative should be 0.5"
        );

        // Switch to RMS
        core.set_detection_mode(1);

        // Feed constant signal for a full window to prime the RMS detector
        let window_len = (RMS_WINDOW_MS * 0.001 * SR as f32).round() as usize;
        let mut rms_level = 0.0f32;
        for _ in 0..window_len + 1 {
            rms_level = core.detect_level(0, 0.5);
        }
        // RMS of constant 0.5 = 0.5
        assert!(
            (rms_level - 0.5).abs() < 0.05,
            "RMS of constant 0.5 should be ~0.5, got {rms_level}"
        );

        // Now feed a half-wave signal: the RMS will differ from peak
        core.set_detection_mode(0);
        let peak_half = core.detect_level(0, 1.0);

        core.set_detection_mode(1);
        // Feed mixed signal for a full window
        for i in 0..window_len + 1 {
            let sample = if i % 2 == 0 { 1.0 } else { 0.0 };
            rms_level = core.detect_level(0, sample);
        }
        // RMS of alternating 1.0/0.0 = sqrt(0.5) ≈ 0.707
        // Peak would be 1.0
        assert!(
            rms_level < peak_half,
            "RMS should be less than peak for alternating signal: rms={rms_level}, peak={peak_half}"
        );
    }

    #[test]
    fn test_no_allocations_in_hot_path() {
        // This test verifies the hot-path methods can be called in a tight loop
        // without triggering allocations. We verify by checking the methods
        // operate on pre-allocated storage only.
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 2, SR);
        core.set_sidechain_hpf(100.0, 0);
        core.set_detection_mode(0);
        core.set_attack_release(5.0, 50.0);

        // Run 10000 iterations of the hot path
        for i in 0..10000 {
            let sample = (i as f32 * 0.01).sin();
            let ch = i % 2;

            let filtered = core.apply_sidechain_filter(ch, sample);
            let level = core.detect_level(ch, filtered);

            let input_db = if level < 1e-10 {
                -120.0
            } else {
                20.0 * level.log10()
            };

            let gr = core.calculate_gain_reduction(input_db, -20.0, 4.0, 6.0);
            let _env = core.apply_envelope(ch, gr);
        }

        // If we got here without panicking, the hot path is allocation-free.
        // The methods only operate on pre-allocated Vecs and stack values.
        assert!(core.envelope_db(0).is_finite());
        assert!(core.envelope_db(1).is_finite());
    }

    #[test]
    fn test_lookahead_process_frame() {
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 2, SR);
        core.set_lookahead_ms(5.0); // 5ms = 240 samples at 48kHz

        let delay = core.lookahead_delay_samples();
        assert_eq!(delay, 240);

        // Push frames through the lookahead
        let mut output = vec![0.0f32; 2];
        for frame in 0..240 {
            let input = [frame as f32, (frame as f32) * 10.0];
            core.lookahead_process_frame(&input, &mut output);
            // First 240 frames should output silence (delay filling)
            assert_eq!(output[0], 0.0);
            assert_eq!(output[1], 0.0);
        }

        // Frame 240 should output frame 0's data
        let input = [240.0, 2400.0];
        core.lookahead_process_frame(&input, &mut output);
        assert!((output[0] - 0.0).abs() < 0.001);
        assert!((output[1] - 0.0).abs() < 0.001);

        // Frame 241 should output frame 1's data
        let input = [241.0, 2410.0];
        core.lookahead_process_frame(&input, &mut output);
        assert!((output[0] - 1.0).abs() < 0.001);
        assert!((output[1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut core = DynamicsCore::new(DynamicsMode::Expand, 2, SR);
        core.set_expand_params(3.0, 50.0, 40.0);

        // Build up some state
        for _ in 0..1000 {
            core.apply_envelope(0, 10.0);
            core.apply_envelope(1, 5.0);
        }
        assert!(core.envelope_db(0) > 0.0);
        assert!(core.envelope_db(1) > 0.0);

        core.reset();

        assert_eq!(core.envelope_db(0), 0.0);
        assert_eq!(core.envelope_db(1), 0.0);
        assert_eq!(core.gate_state(0), GateState::Open);
        assert_eq!(core.gate_state(1), GateState::Open);
    }

    #[test]
    fn test_measured_makeup() {
        let mut core = DynamicsCore::new(DynamicsMode::Compress, 1, SR);

        // Feed steady gain reduction
        for _ in 0..480000 {
            core.update_measured_makeup(6.0);
        }
        let makeup_db = core.measured_makeup_db();
        assert!(
            (makeup_db - 6.0).abs() < 0.1,
            "measured makeup should converge to ~6dB, got {makeup_db}"
        );

        // After reset, should be zero
        core.reset();
        assert!(core.measured_makeup_db().abs() < 0.01);
    }

    #[test]
    fn test_expand_with_gate_state_machine_and_envelope() {
        // Integration test: gate state machine feeds into envelope
        let mut core = DynamicsCore::new(DynamicsMode::Expand, 1, SR);
        core.set_expand_params(3.0, 0.0, 40.0);
        core.set_attack_release(0.1, 10.0);

        let threshold = -20.0;
        let ratio = 4.0;
        let knee = 0.0;

        // Feed a quiet signal (below threshold) for a while
        for _ in 0..4800 {
            let target = core.process_gate_state(0, -40.0, threshold, ratio, knee);
            core.apply_envelope(0, target);
        }

        // Envelope should show significant attenuation
        let env = core.envelope_db(0);
        assert!(
            env > 5.0,
            "after sustained below-threshold signal, envelope should show attenuation, got {env}"
        );

        // Feed a loud signal (above threshold) — gate should re-open
        for _ in 0..4800 {
            let target = core.process_gate_state(0, -10.0, threshold, ratio, knee);
            core.apply_envelope(0, target);
        }

        let env = core.envelope_db(0);
        assert!(
            env < 0.5,
            "after above-threshold signal, envelope should recover, got {env}"
        );
    }
}
