// ============================================================================
// LUFS Targeting — Loudness-aware automatic gain adjustment
// ============================================================================
//
// Measures output loudness (ITU-R BS.1770-4 / EBU R128) and adjusts gain
// to hit a target LUFS value. Used by:
// - Limiter (LUFS-based limiting mode)
// - Saturation (auto-loudness compensation for A/B testing)
//
// HARD RULES:
// - No allocations in process_block()
// - Uses existing LoudnessMonitor from analyzer_loudness_monitor.rs

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::smoothing::Smoother;

/// LUFS-aware gain computer that measures output loudness and adjusts
/// gain to target a specific LUFS value.
pub struct LufsTarget {
    monitor: LoudnessMonitor,
    target_lufs: f32,
    gain_smoother: Smoother,
    current_gain_db: f32,
    max_gain_db: f32,
    min_gain_db: f32,
    enabled: bool,
    channels: usize,
    frames_processed: u64,
    /// Minimum frames before gain adjustment starts (allow measurement to stabilize)
    warmup_frames: u64,
}

impl LufsTarget {
    /// Create a new LUFS target processor.
    ///
    /// Returns `Err` if the loudness monitor fails to initialize.
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, String> {
        let monitor = LoudnessMonitor::new(channels as u32, sample_rate)?;
        Ok(Self {
            monitor,
            target_lufs: -14.0,
            gain_smoother: Smoother::new(0.0, 500.0, sample_rate), // 500ms smoothing
            current_gain_db: 0.0,
            max_gain_db: 12.0,
            min_gain_db: -24.0,
            enabled: true,
            channels,
            frames_processed: 0,
            // ~400ms warmup at 48kHz for integrated loudness to stabilize
            warmup_frames: (sample_rate as u64) / 3,
        })
    }

    /// Set the target loudness in LUFS.
    pub fn set_target(&mut self, target_lufs: f32) {
        self.target_lufs = target_lufs;
    }

    /// Set the maximum gain adjustment in dB (default: +12 dB).
    pub fn set_max_gain(&mut self, max_db: f32) {
        self.max_gain_db = max_db;
    }

    /// Set the minimum gain adjustment in dB (default: -24 dB).
    pub fn set_min_gain(&mut self, min_db: f32) {
        self.min_gain_db = min_db;
    }

    /// Enable or disable LUFS targeting.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.gain_smoother.set_target(0.0);
        }
    }

    /// Feed audio to the loudness monitor and compute the correction gain.
    ///
    /// `buffer`: Interleaved audio samples (same format as plugin process).
    /// `num_frames`: Number of frames in the buffer.
    ///
    /// Returns the current correction gain as a linear multiplier.
    /// Apply this to the output signal to match the target LUFS.
    pub fn process_block(&mut self, buffer: &[f32], num_frames: usize) -> f32 {
        if !self.enabled {
            return 1.0;
        }

        let total_samples = num_frames * self.channels;
        let slice = &buffer[..total_samples.min(buffer.len())];

        // Feed to loudness monitor
        let _ = self.monitor.add_frames(slice);
        self.frames_processed += num_frames as u64;

        // Don't adjust during warmup
        if self.frames_processed < self.warmup_frames {
            return 1.0;
        }

        // Get current loudness (short-term for responsiveness)
        let loudness_data = self.monitor.get_loudness();
        let measured = loudness_data.shortterm_lufs as f32;

        // Only adjust if we have a valid measurement
        if measured <= -120.0 || measured.is_nan() || measured.is_infinite() {
            return db_to_linear(self.gain_smoother.next_n(num_frames));
        }

        // Compute gain adjustment
        let gain_db = (self.target_lufs - measured).clamp(self.min_gain_db, self.max_gain_db);
        self.gain_smoother.set_target(gain_db);
        self.current_gain_db = self.gain_smoother.next_n(num_frames);

        db_to_linear(self.current_gain_db)
    }

    /// Get the current correction gain in dB.
    pub fn current_gain_db(&self) -> f32 {
        self.current_gain_db
    }

    /// Get the target LUFS value.
    pub fn target_lufs(&self) -> f32 {
        self.target_lufs
    }

    /// Get the most recently measured short-term loudness in LUFS.
    pub fn measured_lufs(&mut self) -> f32 {
        self.monitor.get_loudness().shortterm_lufs as f32
    }

    /// Reset the loudness monitor and gain state.
    pub fn reset(&mut self) {
        let _ = self.monitor.reset();
        self.gain_smoother.reset(0.0);
        self.current_gain_db = 0.0;
        self.frames_processed = 0;
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    crate::db_to_linear(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let target = LufsTarget::new(2, 48000);
        assert!(target.is_ok());
    }

    #[test]
    fn test_disabled_returns_unity() {
        let mut target = LufsTarget::new(2, 48000).unwrap();
        target.set_enabled(false);
        let silence = vec![0.0f32; 2048];
        let gain = target.process_block(&silence, 1024);
        assert!((gain - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_warmup_returns_unity() {
        let mut target = LufsTarget::new(2, 48000).unwrap();
        target.set_target(-14.0);
        // During warmup, gain should be 1.0
        let silence = vec![0.0f32; 2048];
        let gain = target.process_block(&silence, 1024);
        assert!((gain - 1.0).abs() < 1e-6, "Warmup gain: {gain}");
    }

    #[test]
    fn test_gain_clamping() {
        let mut target = LufsTarget::new(2, 48000).unwrap();
        target.set_max_gain(6.0);
        target.set_min_gain(-6.0);
        // Feed very quiet signal (forces large positive gain)
        let sr = 48000;
        let frames_per_block = 1024;
        let quiet_signal: Vec<f32> = (0..frames_per_block * 2)
            .map(|i| {
                let t = i as f32 / (sr * 2) as f32;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.0001
            })
            .collect();
        // Process enough blocks to pass warmup
        let mut last_gain = 1.0;
        for _ in 0..100 {
            last_gain = target.process_block(&quiet_signal, frames_per_block);
        }
        // Gain in dB should be clamped to max_gain_db
        let gain_db = 20.0 * last_gain.log10();
        assert!(gain_db <= 6.5, "Gain exceeds max: {gain_db} dB");
    }

    #[test]
    fn test_reset() {
        let mut target = LufsTarget::new(1, 48000).unwrap();
        let signal = vec![0.5f32; 4800];
        target.process_block(&signal, 4800);
        target.reset();
        assert_eq!(target.current_gain_db(), 0.0);
    }

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(6.0) - 2.0).abs() < 0.05);
        assert!((db_to_linear(-6.0) - 0.5).abs() < 0.03);
    }
}
