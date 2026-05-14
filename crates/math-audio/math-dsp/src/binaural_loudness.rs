//! Binaural loudness measurement per ITU-R BS.1770-4.
//!
//! BS.1770-4 specifies a K-weighting filter, mean-square energy integration,
//! and channel-weighted summation to obtain loudness in LUFS. The standard
//! defines explicit channel weights for mono, stereo, and 5.1 loudspeaker
//! layouts but does not single out binaural (headphone) reproduction.
//!
//! For binaural content (a 2-channel ear-signal pair produced by HRTF
//! rendering or dummy-head capture), industry practice is to apply the
//! BS.1770-4 algorithm with the same coefficients as the stereo case:
//! channel weights `G_L = G_R = 1.0`, no LFE, no surround +1.5 dB
//! correction. Both ear signals are K-weighted independently, their
//! mean-square energies summed, and the result reported in LUFS.
//!
//! This module wraps [`EbuR128`] with a binaural-specific API:
//!
//! * Always 2 channels (left ear, right ear).
//! * Convenience constructors and per-ear accessors.
//! * Cumulative true-peak tracking across the entire programme (the
//!   underlying [`EbuR128::prev_true_peak`] is snapshot-and-reset, so we
//!   accumulate locally).
//! * A one-shot [`measure_binaural`] helper for offline analysis.
//! * A surround → binaural downmix path ([`BinauralDownmix`],
//!   [`BinauralLoudness::add_surround_f32`], [`measure_binaural_from_surround`])
//!   for measuring loudness of multichannel content as it would be heard
//!   over headphones. Uses ITU-R BS.775 stereo-downmix coefficients by
//!   default — a level-only approximation of true HRTF rendering (no
//!   spectral or ITD cues). Callers with HRTF magnitude data should pass
//!   broadband per-channel gains via [`BinauralDownmix::from_matrix`].
//!
//! # Example
//!
//! ```
//! use math_audio_dsp::binaural_loudness::{BinauralLoudness, BinauralChannel};
//!
//! let mut meter = BinauralLoudness::new(48_000).unwrap();
//! let frames = vec![0.0f32; 48_000 * 2]; // 1 s of stereo silence
//! meter.add_interleaved_f32(&frames).unwrap();
//! let lufs = meter.integrated_lufs().unwrap();
//! assert!(lufs == f64::NEG_INFINITY || lufs < -100.0);
//! let _ = meter.sample_peak(BinauralChannel::Left).unwrap();
//! ```

use crate::ebur128::{EbuR128, Mode};

/// Identifies one ear of the binaural pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinauralChannel {
    Left,
    Right,
}

impl BinauralChannel {
    fn index(self) -> u32 {
        match self {
            BinauralChannel::Left => 0,
            BinauralChannel::Right => 1,
        }
    }
}

/// One-shot result of [`measure_binaural`].
#[derive(Debug, Clone, Copy)]
pub struct BinauralLoudnessResult {
    /// Integrated programme loudness (LUFS), with EBU R128 two-pass gating.
    pub integrated_lufs: f64,
    /// Momentary loudness (400 ms window) at end of measurement.
    pub momentary_lufs: f64,
    /// Short-term loudness (3 s window) at end of measurement.
    pub shortterm_lufs: f64,
    /// Sample peak (linear, 0..1+) for the left ear.
    pub sample_peak_left: f64,
    /// Sample peak (linear, 0..1+) for the right ear.
    pub sample_peak_right: f64,
    /// True peak (linear, 0..1+) for the left ear, BS.1770-4 4× oversampled.
    pub true_peak_left: f64,
    /// True peak (linear, 0..1+) for the right ear, BS.1770-4 4× oversampled.
    pub true_peak_right: f64,
}

/// Streaming binaural-loudness meter applying ITU-R BS.1770-4 K-weighting
/// and gated integration to a 2-channel ear-signal pair.
pub struct BinauralLoudness {
    ebur128: EbuR128,
    sample_rate: u32,
    // Cumulative true-peak tracking (EbuR128's prev_true_peak is
    // snapshot-and-reset; we accumulate the max across snapshots).
    cum_true_peak: [f64; 2],
    // Interleave scratch buffer for add_separate_f32, reused across calls.
    interleave_buf: Vec<f32>,
}

impl BinauralLoudness {
    /// Create a binaural loudness meter at the given sample rate.
    ///
    /// All BS.1770-4 measurements are enabled: momentary, short-term,
    /// integrated, sample peak, and true peak.
    pub fn new(sample_rate: u32) -> Result<Self, String> {
        Self::with_mode(sample_rate, Mode::all())
    }

    /// Create a binaural loudness meter with explicit measurement modes.
    pub fn with_mode(sample_rate: u32, mode: Mode) -> Result<Self, String> {
        let ebur128 = EbuR128::new(2, sample_rate, mode)?;
        Ok(Self {
            ebur128,
            sample_rate,
            cum_true_peak: [0.0; 2],
            interleave_buf: Vec::new(),
        })
    }

    /// Sample rate this meter was configured for (Hz).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Feed interleaved stereo frames `[L0, R0, L1, R1, ...]`.
    pub fn add_interleaved_f32(&mut self, samples: &[f32]) -> Result<(), String> {
        if !samples.len().is_multiple_of(2) {
            return Err("binaural samples length must be even (interleaved L/R)".into());
        }
        self.ebur128.add_frames_f32(samples)?;
        self.refresh_cumulative_true_peak();
        Ok(())
    }

    /// Feed separate left / right buffers of equal length.
    pub fn add_separate_f32(&mut self, left: &[f32], right: &[f32]) -> Result<(), String> {
        if left.len() != right.len() {
            return Err(format!(
                "left and right buffers must have equal length (got {} vs {})",
                left.len(),
                right.len()
            ));
        }
        let needed = left.len() * 2;
        if self.interleave_buf.len() < needed {
            self.interleave_buf.resize(needed, 0.0);
        }
        let buf = &mut self.interleave_buf[..needed];
        for (i, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
            buf[i * 2] = l;
            buf[i * 2 + 1] = r;
        }
        self.ebur128.add_frames_f32(buf)?;
        self.refresh_cumulative_true_peak();
        Ok(())
    }

    /// Drain BS.1770-4 true-peak deltas into the cumulative tracker.
    /// `prev_true_peak` resets after read, so we max-merge each call.
    fn refresh_cumulative_true_peak(&mut self) {
        for ch in 0..2u32 {
            // prev_true_peak returns 0.0 when TRUE_PEAK mode is disabled.
            if let Ok(p) = self.ebur128.prev_true_peak(ch) {
                let i = ch as usize;
                if p > self.cum_true_peak[i] {
                    self.cum_true_peak[i] = p;
                }
            }
        }
    }

    /// Momentary loudness (400 ms window) in LUFS.
    pub fn momentary_lufs(&self) -> Result<f64, String> {
        self.ebur128.loudness_momentary()
    }

    /// Short-term loudness (3 s window) in LUFS.
    pub fn shortterm_lufs(&self) -> Result<f64, String> {
        self.ebur128.loudness_shortterm()
    }

    /// Integrated programme loudness in LUFS with EBU R128 two-pass gating.
    pub fn integrated_lufs(&self) -> Result<f64, String> {
        self.ebur128.loudness_global()
    }

    /// Cumulative sample peak (linear) for the given ear.
    pub fn sample_peak(&self, channel: BinauralChannel) -> Result<f64, String> {
        self.ebur128.sample_peak(channel.index())
    }

    /// Cumulative true peak (linear, BS.1770-4 4× oversampled) for the given ear.
    pub fn true_peak(&self, channel: BinauralChannel) -> f64 {
        self.cum_true_peak[channel.index() as usize]
    }

    /// Feed surround (multichannel) frames by downmixing through `downmix`
    /// to a binaural ear-signal pair, then accumulating BS.1770-4 statistics.
    ///
    /// `samples` must be interleaved with exactly `downmix.channels()`
    /// channels per frame. The downmix is a linear gain matrix applied
    /// frame-by-frame in `f64`; see [`BinauralDownmix`] for caveats and
    /// preset constructors.
    pub fn add_surround_f32(
        &mut self,
        samples: &[f32],
        downmix: &BinauralDownmix,
    ) -> Result<(), String> {
        let nc = downmix.channels();
        if nc == 0 {
            return Err("binaural downmix matrix is empty".into());
        }
        if !samples.len().is_multiple_of(nc) {
            return Err(format!(
                "samples length ({}) must be a multiple of source channel count ({})",
                samples.len(),
                nc
            ));
        }
        let num_frames = samples.len() / nc;
        let needed = num_frames * 2;
        if self.interleave_buf.len() < needed {
            self.interleave_buf.resize(needed, 0.0);
        }
        let buf = &mut self.interleave_buf[..needed];
        let coeffs = downmix.coeffs();
        for (i, frame) in samples.chunks_exact(nc).enumerate() {
            let mut l = 0.0f64;
            let mut r = 0.0f64;
            for (ch, &s) in frame.iter().enumerate() {
                let x = s as f64;
                l += x * coeffs[ch][0];
                r += x * coeffs[ch][1];
            }
            buf[i * 2] = l as f32;
            buf[i * 2 + 1] = r as f32;
        }
        self.ebur128.add_frames_f32(buf)?;
        self.refresh_cumulative_true_peak();
        Ok(())
    }

    /// Snapshot the current measurement state as a [`BinauralLoudnessResult`].
    pub fn snapshot(&self) -> Result<BinauralLoudnessResult, String> {
        Ok(BinauralLoudnessResult {
            integrated_lufs: self.integrated_lufs()?,
            momentary_lufs: self.momentary_lufs()?,
            shortterm_lufs: self.shortterm_lufs()?,
            sample_peak_left: self.sample_peak(BinauralChannel::Left)?,
            sample_peak_right: self.sample_peak(BinauralChannel::Right)?,
            true_peak_left: self.true_peak(BinauralChannel::Left),
            true_peak_right: self.true_peak(BinauralChannel::Right),
        })
    }

    /// Reset all internal state (filters, gating blocks, peaks).
    pub fn reset(&mut self) {
        self.ebur128.reset();
        self.cum_true_peak = [0.0; 2];
    }
}

/// Per-channel binaural downmix gain matrix.
///
/// Each row holds the `[L_ear, R_ear]` linear gains applied to one source
/// channel when summing into the binaural pair. This is a **level-only**
/// approximation of HRTF rendering: it captures channel weighting and
/// stereo placement but not spectral cues (HRTF magnitude response) or
/// interaural time differences. It is suitable for BS.1770-4 loudness
/// estimation of how multichannel content will sit on headphones, not
/// for spatial-quality assessment.
///
/// Preset constructors implement the ITU-R BS.775 stereo downmix
/// coefficients (centre → both ears at −3 dB; surrounds → same-side ear
/// at −3 dB; LFE excluded per BS.1770-4). Callers with HRTF magnitude
/// data should derive broadband per-channel gains and supply them via
/// [`BinauralDownmix::from_matrix`].
#[derive(Debug, Clone)]
pub struct BinauralDownmix {
    coeffs: Vec<[f64; 2]>,
}

/// Standard channel orderings supported by [`BinauralDownmix`] presets.
///
/// All layouts follow the SMPTE / WAV interleaving convention used
/// elsewhere in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurroundLayout {
    /// 5.0: `L, R, C, Ls, Rs` (5 channels).
    FiveZero,
    /// 5.1: `L, R, C, LFE, Ls, Rs` (6 channels).
    FiveOne,
    /// 7.1: `L, R, C, LFE, Ls, Rs, Lrs, Rrs` (8 channels).
    SevenOne,
}

impl BinauralDownmix {
    /// Build a downmix from explicit per-channel `[L, R]` gains.
    pub fn from_matrix(coeffs: Vec<[f64; 2]>) -> Result<Self, String> {
        if coeffs.is_empty() {
            return Err("binaural downmix matrix must have at least one channel".into());
        }
        Ok(Self { coeffs })
    }

    /// ITU-R BS.775 binaural-equivalent stereo downmix for the given layout.
    pub fn bs775(layout: SurroundLayout) -> Self {
        // -3 dB amplitude factor used by BS.775 for centre and surround mix.
        let a = std::f64::consts::FRAC_1_SQRT_2;
        let coeffs: Vec<[f64; 2]> = match layout {
            // L,  R,  C,    Ls,    Rs
            SurroundLayout::FiveZero => vec![
                [1.0, 0.0],
                [0.0, 1.0],
                [a, a],
                [a, 0.0],
                [0.0, a],
            ],
            // L,  R,  C,    LFE, Ls,    Rs
            SurroundLayout::FiveOne => vec![
                [1.0, 0.0],
                [0.0, 1.0],
                [a, a],
                [0.0, 0.0],
                [a, 0.0],
                [0.0, a],
            ],
            // L, R, C, LFE, Ls, Rs, Lrs, Rrs
            SurroundLayout::SevenOne => vec![
                [1.0, 0.0],
                [0.0, 1.0],
                [a, a],
                [0.0, 0.0],
                [a, 0.0],
                [0.0, a],
                [a, 0.0],
                [0.0, a],
            ],
        };
        Self { coeffs }
    }

    /// Number of source channels expected by this matrix.
    pub fn channels(&self) -> usize {
        self.coeffs.len()
    }

    /// Per-channel `[L, R]` gain rows.
    pub fn coeffs(&self) -> &[[f64; 2]] {
        &self.coeffs
    }
}

/// One-shot binaural loudness measurement on a complete interleaved
/// stereo buffer. Convenient for offline analysis.
pub fn measure_binaural(
    samples_interleaved: &[f32],
    sample_rate: u32,
) -> Result<BinauralLoudnessResult, String> {
    let mut meter = BinauralLoudness::new(sample_rate)?;
    meter.add_interleaved_f32(samples_interleaved)?;
    meter.snapshot()
}

/// One-shot binaural loudness measurement for a multichannel programme.
///
/// Downmixes `samples_interleaved` through `downmix` (which must match
/// the source channel count) to a binaural pair, then applies BS.1770-4
/// loudness measurement.
pub fn measure_binaural_from_surround(
    samples_interleaved: &[f32],
    sample_rate: u32,
    downmix: &BinauralDownmix,
) -> Result<BinauralLoudnessResult, String> {
    let mut meter = BinauralLoudness::new(sample_rate)?;
    meter.add_surround_f32(samples_interleaved, downmix)?;
    meter.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine_stereo(freq: f64, amp: f64, sr: u32, secs: u32) -> Vec<f32> {
        let n = (sr as usize) * (secs as usize);
        let mut out = vec![0.0f32; n * 2];
        for i in 0..n {
            let t = i as f64 / sr as f64;
            let s = (2.0 * PI * freq * t).sin() * amp;
            out[i * 2] = s as f32;
            out[i * 2 + 1] = s as f32;
        }
        out
    }

    #[test]
    fn rejects_odd_length() {
        let mut meter = BinauralLoudness::new(48_000).unwrap();
        let bad = vec![0.0f32; 5];
        assert!(meter.add_interleaved_f32(&bad).is_err());
    }

    #[test]
    fn separate_buffers_must_match_length() {
        let mut meter = BinauralLoudness::new(48_000).unwrap();
        let l = vec![0.0f32; 10];
        let r = vec![0.0f32; 11];
        assert!(meter.add_separate_f32(&l, &r).is_err());
    }

    #[test]
    fn silence_returns_neg_inf() {
        let mut meter = BinauralLoudness::new(48_000).unwrap();
        let frames = vec![0.0f32; 48_000 * 2];
        meter.add_interleaved_f32(&frames).unwrap();
        let lufs = meter.integrated_lufs().unwrap();
        assert!(lufs == f64::NEG_INFINITY || lufs < -100.0);
        assert_eq!(meter.sample_peak(BinauralChannel::Left).unwrap(), 0.0);
        assert_eq!(meter.true_peak(BinauralChannel::Right), 0.0);
    }

    #[test]
    fn sine_1khz_matches_bs1770_stereo() {
        // BS.1770-4 specifies that a 0 dBFS 1 kHz sine in a stereo bus
        // produces approximately -3.01 LUFS before K-weighting and
        // ~-0.3 LUFS after K-weighting (which has +0.2 dB at 1 kHz).
        // Binaural with G_L = G_R = 1.0 must yield the same result.
        let sr = 48_000u32;
        let samples = sine_stereo(1_000.0, 1.0, sr, 5);
        let result = measure_binaural(&samples, sr).unwrap();
        assert!(
            result.integrated_lufs > -2.0 && result.integrated_lufs < 1.0,
            "expected ~-0.3 LUFS, got {}",
            result.integrated_lufs
        );
        // Peaks for full-scale sine should hit close to 1.0.
        assert!(
            result.sample_peak_left > 0.99,
            "left sample peak {}",
            result.sample_peak_left
        );
        assert!(
            result.true_peak_right >= result.sample_peak_right,
            "true peak ({}) must be >= sample peak ({})",
            result.true_peak_right,
            result.sample_peak_right
        );
    }

    #[test]
    fn asymmetric_channels_tracked_independently() {
        let sr = 48_000u32;
        let n = (sr as usize) * 2;
        let mut frames = vec![0.0f32; n * 2];
        // Left = 0 dBFS sine, right = silence.
        for i in 0..n {
            let t = i as f64 / sr as f64;
            frames[i * 2] = (2.0 * PI * 1_000.0 * t).sin() as f32;
            frames[i * 2 + 1] = 0.0;
        }
        let result = measure_binaural(&frames, sr).unwrap();
        assert!(
            result.sample_peak_left > 0.99,
            "left peak should be ~1.0, got {}",
            result.sample_peak_left
        );
        assert_eq!(result.sample_peak_right, 0.0);
        assert!(result.true_peak_left >= result.sample_peak_left);
        assert_eq!(result.true_peak_right, 0.0);
        // Halving the active channels removes 3 dB: expect roughly -3 LUFS
        // relative to the symmetric stereo case (~-0.3 LUFS), i.e. ~-3.3.
        assert!(
            result.integrated_lufs > -6.0 && result.integrated_lufs < -1.0,
            "expected ~-3.3 LUFS for single-ear sine, got {}",
            result.integrated_lufs
        );
    }

    #[test]
    fn interleaved_and_separate_agree() {
        let sr = 48_000u32;
        let n = (sr as usize) * 2;
        let mut interleaved = vec![0.0f32; n * 2];
        let mut left = vec![0.0f32; n];
        let mut right = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f64 / sr as f64;
            let l = (2.0 * PI * 440.0 * t).sin() as f32 * 0.5;
            let r = (2.0 * PI * 880.0 * t).sin() as f32 * 0.25;
            interleaved[i * 2] = l;
            interleaved[i * 2 + 1] = r;
            left[i] = l;
            right[i] = r;
        }

        let mut m_inter = BinauralLoudness::new(sr).unwrap();
        m_inter.add_interleaved_f32(&interleaved).unwrap();
        let mut m_sep = BinauralLoudness::new(sr).unwrap();
        m_sep.add_separate_f32(&left, &right).unwrap();

        let a = m_inter.integrated_lufs().unwrap();
        let b = m_sep.integrated_lufs().unwrap();
        assert!(
            (a - b).abs() < 1e-9,
            "interleaved {a} should equal separate {b}"
        );
        assert_eq!(
            m_inter.sample_peak(BinauralChannel::Left).unwrap(),
            m_sep.sample_peak(BinauralChannel::Left).unwrap()
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut meter = BinauralLoudness::new(48_000).unwrap();
        let frames = sine_stereo(1_000.0, 0.5, 48_000, 2);
        meter.add_interleaved_f32(&frames).unwrap();
        assert!(meter.sample_peak(BinauralChannel::Left).unwrap() > 0.0);
        meter.reset();
        assert_eq!(meter.sample_peak(BinauralChannel::Left).unwrap(), 0.0);
        assert_eq!(meter.true_peak(BinauralChannel::Left), 0.0);
        let lufs = meter.integrated_lufs().unwrap();
        assert!(lufs == f64::NEG_INFINITY || lufs < -100.0);
    }

    #[test]
    fn surround_downmix_rejects_wrong_channel_count() {
        let mut meter = BinauralLoudness::new(48_000).unwrap();
        let dm = BinauralDownmix::bs775(SurroundLayout::FiveOne); // 6 channels
        let bad = vec![0.0f32; 10]; // 10 is not a multiple of 6
        assert!(meter.add_surround_f32(&bad, &dm).is_err());
    }

    #[test]
    fn surround_downmix_empty_matrix_rejected() {
        assert!(BinauralDownmix::from_matrix(vec![]).is_err());
    }

    #[test]
    fn bs775_5_1_lfe_excluded() {
        // LFE-only 5.1 signal must yield silence after the BS.775 downmix
        // (LFE coefficients are [0.0, 0.0]).
        let sr = 48_000u32;
        let n = (sr as usize) * 2;
        let mut frames = vec![0.0f32; n * 6];
        for i in 0..n {
            let t = i as f64 / sr as f64;
            frames[i * 6 + 3] = (2.0 * PI * 60.0 * t).sin() as f32; // LFE only
        }
        let dm = BinauralDownmix::bs775(SurroundLayout::FiveOne);
        let result = measure_binaural_from_surround(&frames, sr, &dm).unwrap();
        assert_eq!(result.sample_peak_left, 0.0);
        assert_eq!(result.sample_peak_right, 0.0);
        assert!(
            result.integrated_lufs == f64::NEG_INFINITY || result.integrated_lufs < -100.0,
            "LFE-only must be silence after downmix, got {}",
            result.integrated_lufs
        );
    }

    #[test]
    fn bs775_5_1_centre_routes_to_both_ears() {
        // Centre-only 5.1: BS.775 distributes C → both ears at -3 dB
        // (1/√2 ≈ 0.707), so a 0 dBFS centre tone produces ~0.707 in
        // each ear. Loudness should be close to a 0.707-amplitude
        // stereo sine (~-3.3 LUFS).
        let sr = 48_000u32;
        let n = (sr as usize) * 5;
        let mut frames = vec![0.0f32; n * 6];
        for i in 0..n {
            let t = i as f64 / sr as f64;
            frames[i * 6 + 2] = (2.0 * PI * 1_000.0 * t).sin() as f32; // C only
        }
        let dm = BinauralDownmix::bs775(SurroundLayout::FiveOne);
        let result = measure_binaural_from_surround(&frames, sr, &dm).unwrap();
        // 1/√2 ≈ 0.7071; both ears should be ~0.7071.
        assert!(
            (result.sample_peak_left - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-3,
            "left peak should be ~0.707, got {}",
            result.sample_peak_left
        );
        assert!(
            (result.sample_peak_right - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-3,
            "right peak should be ~0.707, got {}",
            result.sample_peak_right
        );
        // -3 dB amplitude both ears -> ~-3.3 LUFS relative to full-scale stereo (~-0.3 LUFS).
        assert!(
            result.integrated_lufs > -6.0 && result.integrated_lufs < -1.0,
            "expected ~-3.3 LUFS, got {}",
            result.integrated_lufs
        );
    }

    #[test]
    fn bs775_5_0_and_5_1_agree_when_lfe_silent() {
        // 5.0 measurement should equal 5.1 measurement when LFE is silent.
        let sr = 48_000u32;
        let n = (sr as usize) * 3;
        // Build a 5.0 buffer with content in L, R, C, Ls, Rs.
        let mut frames_5_0 = vec![0.0f32; n * 5];
        let mut frames_5_1 = vec![0.0f32; n * 6];
        for i in 0..n {
            let t = i as f64 / sr as f64;
            let l = (2.0 * PI * 440.0 * t).sin() as f32 * 0.3;
            let r = (2.0 * PI * 660.0 * t).sin() as f32 * 0.3;
            let c = (2.0 * PI * 1_000.0 * t).sin() as f32 * 0.2;
            let ls = (2.0 * PI * 800.0 * t).sin() as f32 * 0.15;
            let rs = (2.0 * PI * 900.0 * t).sin() as f32 * 0.15;
            frames_5_0[i * 5] = l;
            frames_5_0[i * 5 + 1] = r;
            frames_5_0[i * 5 + 2] = c;
            frames_5_0[i * 5 + 3] = ls;
            frames_5_0[i * 5 + 4] = rs;
            frames_5_1[i * 6] = l;
            frames_5_1[i * 6 + 1] = r;
            frames_5_1[i * 6 + 2] = c;
            frames_5_1[i * 6 + 3] = 0.0; // LFE silent
            frames_5_1[i * 6 + 4] = ls;
            frames_5_1[i * 6 + 5] = rs;
        }
        let dm50 = BinauralDownmix::bs775(SurroundLayout::FiveZero);
        let dm51 = BinauralDownmix::bs775(SurroundLayout::FiveOne);
        let a = measure_binaural_from_surround(&frames_5_0, sr, &dm50).unwrap();
        let b = measure_binaural_from_surround(&frames_5_1, sr, &dm51).unwrap();
        assert!(
            (a.integrated_lufs - b.integrated_lufs).abs() < 1e-6,
            "5.0 ({}) vs 5.1-with-silent-LFE ({}) should match",
            a.integrated_lufs,
            b.integrated_lufs
        );
    }

    #[test]
    fn custom_matrix_can_pass_through_stereo() {
        // Identity 2x[L,R] matrix should make add_surround_f32 behave
        // identically to add_interleaved_f32 for 2-channel input.
        let sr = 48_000u32;
        let samples = sine_stereo(1_000.0, 0.6, sr, 2);

        let identity = BinauralDownmix::from_matrix(vec![[1.0, 0.0], [0.0, 1.0]]).unwrap();

        let mut a = BinauralLoudness::new(sr).unwrap();
        a.add_interleaved_f32(&samples).unwrap();
        let mut b = BinauralLoudness::new(sr).unwrap();
        b.add_surround_f32(&samples, &identity).unwrap();

        assert!(
            (a.integrated_lufs().unwrap() - b.integrated_lufs().unwrap()).abs() < 1e-9,
            "identity downmix must match interleaved path"
        );
        assert_eq!(
            a.sample_peak(BinauralChannel::Left).unwrap(),
            b.sample_peak(BinauralChannel::Left).unwrap()
        );
    }

    #[test]
    fn bs775_7_1_channel_count() {
        let dm = BinauralDownmix::bs775(SurroundLayout::SevenOne);
        assert_eq!(dm.channels(), 8);
        // LFE row must be zero.
        assert_eq!(dm.coeffs()[3], [0.0, 0.0]);
    }

    #[test]
    fn true_peak_accumulates_across_calls() {
        // Submit two short bursts; true peak after the second call must be
        // at least the max of both bursts (cumulative tracking).
        let sr = 48_000u32;
        let burst_a = sine_stereo(1_000.0, 0.5, sr, 1);
        let burst_b = sine_stereo(1_000.0, 0.9, sr, 1);
        let mut meter = BinauralLoudness::new(sr).unwrap();
        meter.add_interleaved_f32(&burst_a).unwrap();
        let peak_after_a = meter.true_peak(BinauralChannel::Left);
        meter.add_interleaved_f32(&burst_b).unwrap();
        let peak_after_b = meter.true_peak(BinauralChannel::Left);
        assert!(peak_after_a > 0.4 && peak_after_a < 0.6);
        assert!(peak_after_b >= peak_after_a);
        assert!(peak_after_b > 0.8);
    }
}
