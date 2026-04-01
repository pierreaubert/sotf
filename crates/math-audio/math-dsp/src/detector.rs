// ============================================================================
// Level Detector — Peak and RMS detection for dynamics plugins
// ============================================================================

/// Detection mode for level measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionMode {
    /// Instantaneous peak (absolute value).
    Peak,
    /// RMS with a sliding window.
    Rms { window_ms: f32 },
}

/// A level detector that can operate in Peak or RMS mode.
///
/// Used by compressor, expander, gate, and multiband dynamics plugins
/// to measure the input signal level for gain computation.
#[derive(Debug, Clone)]
pub struct LevelDetector {
    mode: DetectionMode,
    /// Running sum of squared samples (for RMS).
    sum_sq: f64,
    /// Circular buffer of squared samples (for RMS).
    window_buf: Vec<f32>,
    /// Write position in the circular buffer.
    window_pos: usize,
    /// Window length in samples.
    window_len: usize,
    sample_rate: u32,
}

impl LevelDetector {
    pub fn new(mode: DetectionMode, sample_rate: u32) -> Self {
        let window_len = match mode {
            DetectionMode::Peak => 0,
            DetectionMode::Rms { window_ms } => {
                (window_ms * 0.001 * sample_rate as f32).round() as usize
            }
        }
        .max(1);

        Self {
            mode,
            sum_sq: 0.0,
            window_buf: if matches!(mode, DetectionMode::Rms { .. }) {
                vec![0.0; window_len]
            } else {
                Vec::new()
            },
            window_pos: 0,
            window_len,
            sample_rate,
        }
    }

    /// Process one sample and return the detected level in dB.
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        let lin = self.process_linear(sample);
        if lin < 1e-12 {
            -120.0
        } else {
            20.0 * lin.log10()
        }
    }

    /// Process one sample and return the detected level as linear amplitude (not dB).
    #[inline]
    pub fn process_linear(&mut self, sample: f32) -> f32 {
        match self.mode {
            DetectionMode::Peak => sample.abs(),
            DetectionMode::Rms { .. } => {
                let sq = (sample * sample) as f64;
                let oldest = self.window_buf[self.window_pos] as f64;
                self.sum_sq = (self.sum_sq + sq - oldest).max(0.0);
                self.window_buf[self.window_pos] = sample * sample;
                self.window_pos = (self.window_pos + 1) % self.window_len;

                (self.sum_sq / self.window_len as f64).sqrt() as f32
            }
        }
    }

    pub fn reset(&mut self) {
        self.sum_sq = 0.0;
        self.window_buf.fill(0.0);
        self.window_pos = 0;
    }

    /// Change detection mode. Resets internal state.
    pub fn set_mode(&mut self, mode: DetectionMode) {
        self.mode = mode;
        let new_len = match mode {
            DetectionMode::Peak => 1,
            DetectionMode::Rms { window_ms } => {
                (window_ms * 0.001 * self.sample_rate as f32).round() as usize
            }
        }
        .max(1);

        self.window_len = new_len;
        if matches!(mode, DetectionMode::Rms { .. }) {
            self.window_buf.resize(new_len, 0.0);
        }
        self.reset();
    }

    pub fn mode(&self) -> DetectionMode {
        self.mode
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_detection() {
        let mut det = LevelDetector::new(DetectionMode::Peak, 48000);
        // 1.0 amplitude → 0 dB
        let db = det.process(1.0);
        assert!((db - 0.0).abs() < 0.01);

        // 0.1 amplitude → -20 dB
        let db = det.process(0.1);
        assert!((db - (-20.0)).abs() < 0.01);

        // Negative sample → same as positive
        let db = det.process(-0.5);
        let expected = 20.0 * 0.5f32.log10();
        assert!((db - expected).abs() < 0.01);
    }

    #[test]
    fn test_rms_detection() {
        let mut det = LevelDetector::new(DetectionMode::Rms { window_ms: 10.0 }, 48000);
        // Feed constant 1.0 for window_len samples
        let window_len = (10.0f32 * 0.001 * 48000.0).round() as usize;
        for _ in 0..window_len {
            det.process(1.0);
        }
        // RMS of constant 1.0 = 1.0 → 0 dB
        let db = det.process(1.0);
        assert!((db - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_silence_floor() {
        let mut det = LevelDetector::new(DetectionMode::Peak, 48000);
        let db = det.process(0.0);
        assert!(db <= -120.0);
    }

    #[test]
    fn test_rms_reset() {
        let mut det = LevelDetector::new(DetectionMode::Rms { window_ms: 10.0 }, 48000);
        for _ in 0..1000 {
            det.process(1.0);
        }
        det.reset();
        let db = det.process(0.0);
        assert!(db <= -120.0);
    }
}
