// ============================================================================
// Transport — Global playback state for the timeline
// ============================================================================

/// Global transport state controlling timeline playback position and tempo.
///
/// All positions are in samples for sample-accurate operation.
/// Tempo is stored for future MIDI sync but does not affect audio playback speed.
#[derive(Debug, Clone)]
pub struct Transport {
    /// Current playback position in samples
    pub position_samples: u64,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Tempo in beats per minute (for MIDI sync / grid snapping)
    pub tempo_bpm: f64,
    /// Whether playback is active
    pub playing: bool,
    /// Loop region (start_samples, end_samples). None = no looping.
    pub loop_range: Option<(u64, u64)>,
}

impl Transport {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            position_samples: 0,
            sample_rate,
            tempo_bpm: 120.0,
            playing: false,
            loop_range: None,
        }
    }

    /// Current position in seconds.
    pub fn position_seconds(&self) -> f64 {
        self.position_samples as f64 / self.sample_rate as f64
    }

    /// Advance the transport by `num_frames` samples.
    /// Handles looping: if position crosses loop end, wraps to loop start.
    pub fn advance(&mut self, num_frames: usize) {
        if !self.playing {
            return;
        }
        self.position_samples += num_frames as u64;

        if let Some((loop_start, loop_end)) = self.loop_range
            && loop_end > loop_start
            && self.position_samples >= loop_end
        {
            let overshoot = self.position_samples - loop_end;
            let loop_len = loop_end - loop_start;
            self.position_samples = loop_start + (overshoot % loop_len);
        }
    }

    /// Seek to an absolute position in samples.
    pub fn seek(&mut self, position_samples: u64) {
        self.position_samples = position_samples;
    }

    /// Seek to a position in seconds.
    pub fn seek_seconds(&mut self, seconds: f64) {
        if !seconds.is_finite() || seconds <= 0.0 {
            self.position_samples = 0;
            return;
        }

        let samples = seconds * self.sample_rate as f64;
        self.position_samples = if samples >= u64::MAX as f64 {
            u64::MAX
        } else {
            samples as u64
        };
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.position_samples = 0;
    }

    /// Set loop region. Pass None to disable looping.
    pub fn set_loop(&mut self, range: Option<(u64, u64)>) {
        self.loop_range = range;
    }

    /// Convert a beat position to samples at the current tempo.
    pub fn beats_to_samples(&self, beats: f64) -> u64 {
        let seconds_per_beat = 60.0 / self.tempo_bpm;
        (beats * seconds_per_beat * self.sample_rate as f64) as u64
    }

    /// Convert samples to beat position at the current tempo.
    pub fn samples_to_beats(&self, samples: u64) -> f64 {
        let seconds = samples as f64 / self.sample_rate as f64;
        seconds * self.tempo_bpm / 60.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_advance() {
        let mut t = Transport::new(48000);
        t.play();
        t.advance(1024);
        assert_eq!(t.position_samples, 1024);
        t.advance(1024);
        assert_eq!(t.position_samples, 2048);
    }

    #[test]
    fn test_transport_advance_paused() {
        let mut t = Transport::new(48000);
        // Not playing — advance should not change position
        t.advance(1024);
        assert_eq!(t.position_samples, 0);
    }

    #[test]
    fn test_transport_loop() {
        let mut t = Transport::new(48000);
        t.play();
        t.set_loop(Some((0, 4800))); // Loop over 100ms
        t.advance(4800);
        assert_eq!(t.position_samples, 0); // Wrapped
        t.advance(2400);
        assert_eq!(t.position_samples, 2400);
        t.advance(3000); // 2400 + 3000 = 5400 > 4800
        assert_eq!(t.position_samples, 600); // 5400 - 4800 = 600
    }

    #[test]
    fn test_transport_seek() {
        let mut t = Transport::new(48000);
        t.seek(96000);
        assert_eq!(t.position_seconds(), 2.0);
    }

    #[test]
    fn test_transport_seek_seconds_clamps_invalid_values() {
        let mut t = Transport::new(48000);
        t.seek_seconds(f64::NAN);
        assert_eq!(t.position_samples, 0);
        t.seek_seconds(-1.0);
        assert_eq!(t.position_samples, 0);
        t.seek_seconds(f64::MAX);
        assert_eq!(t.position_samples, u64::MAX);
    }

    #[test]
    fn test_transport_beats_to_samples() {
        let t = Transport::new(48000);
        // 120 BPM = 2 beats/sec, so 1 beat = 0.5 sec = 24000 samples
        assert_eq!(t.beats_to_samples(1.0), 24000);
        assert_eq!(t.beats_to_samples(4.0), 96000);
    }

    #[test]
    fn test_transport_stop() {
        let mut t = Transport::new(48000);
        t.play();
        t.advance(10000);
        t.stop();
        assert!(!t.playing);
        assert_eq!(t.position_samples, 0);
    }
}
