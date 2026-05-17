// ============================================================================
// Clip & Region — Non-destructive audio references on the timeline
// ============================================================================

use crate::decoder::source::AudioSource;
use std::cell::Cell;

/// Fade curve type for clip boundaries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadeCurve {
    /// Linear ramp (constant slope)
    Linear,
    /// Quadratic ease-in/out (smooth)
    EqualPower,
    /// Cubic S-curve (very smooth)
    SCurve,
}

impl FadeCurve {
    /// Evaluate the fade gain at position `t` (0.0 = start, 1.0 = end).
    /// For fade-in: t goes 0→1 (silence to full). For fade-out: use 1-t.
    pub fn eval(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FadeCurve::Linear => t,
            FadeCurve::EqualPower => {
                // Equal-power crossfade: sin(t * pi/2)
                (t * std::f32::consts::FRAC_PI_2).sin()
            }
            FadeCurve::SCurve => {
                // Hermite S-curve: 3t^2 - 2t^3
                t * t * (3.0 - 2.0 * t)
            }
        }
    }
}

/// A non-destructive reference to an audio source with editing parameters.
///
/// The Clip does not own the audio data — it references an AudioSource and
/// specifies which portion to play, with gain and fade adjustments.
#[derive(Debug, Clone)]
pub struct Clip {
    /// Audio source (file path, URL, etc.)
    pub source: AudioSource,
    /// Start offset within the source file in samples (trim start)
    pub source_offset_samples: u64,
    /// Duration to play from the source in samples
    pub duration_samples: u64,
    /// Per-clip gain offset in dB (0.0 = no change)
    pub gain_db: f32,
    /// Fade-in duration in samples (0 = no fade)
    pub fade_in_samples: u64,
    /// Fade-out duration in samples (0 = no fade)
    pub fade_out_samples: u64,
    /// Fade curve type
    pub fade_curve: FadeCurve,
    /// Time stretch ratio (1.0 = original speed, 2.0 = double speed/half duration)
    pub time_stretch_ratio: f64,
    /// Play in reverse
    pub reverse: bool,
    cached_gain_db_bits: Cell<u32>,
    cached_linear_gain: Cell<f32>,
}

impl Clip {
    /// Create a new clip referencing an audio source.
    pub fn new(source: AudioSource, duration_samples: u64) -> Self {
        Self {
            source,
            source_offset_samples: 0,
            duration_samples,
            gain_db: 0.0,
            fade_in_samples: 0,
            fade_out_samples: 0,
            fade_curve: FadeCurve::Linear,
            time_stretch_ratio: 1.0,
            reverse: false,
            cached_gain_db_bits: Cell::new(0.0f32.to_bits()),
            cached_linear_gain: Cell::new(1.0),
        }
    }

    /// Create a clip from a file path.
    pub fn from_file(path: impl Into<std::path::PathBuf>, duration_samples: u64) -> Self {
        Self::new(AudioSource::File(path.into()), duration_samples)
    }

    /// Effective duration after time-stretching.
    pub fn effective_duration_samples(&self) -> u64 {
        if self.time_stretch_ratio > 0.0 {
            (self.duration_samples as f64 / self.time_stretch_ratio) as u64
        } else {
            self.duration_samples
        }
    }

    /// Compute the gain multiplier for a given position within the clip (in samples).
    /// Includes per-clip gain and fade envelopes.
    pub fn gain_at(&self, position_in_clip: u64) -> f32 {
        self.linear_gain() * self.fade_gain_at(position_in_clip)
    }

    /// Convert the clip gain from dB to linear gain.
    pub fn linear_gain(&self) -> f32 {
        let gain_db_bits = self.gain_db.to_bits();
        if self.cached_gain_db_bits.get() != gain_db_bits {
            self.cached_gain_db_bits.set(gain_db_bits);
            self.cached_linear_gain
                .set(10.0f32.powf(self.gain_db / 20.0));
        }
        self.cached_linear_gain.get()
    }

    /// Compute only the fade envelope for a position within the clip.
    pub fn fade_gain_at(&self, position_in_clip: u64) -> f32 {
        let effective_dur = self.effective_duration_samples();

        // Fade-in
        let fade_in_gain = if self.fade_in_samples > 0 && position_in_clip < self.fade_in_samples {
            let t = position_in_clip as f32 / self.fade_in_samples as f32;
            self.fade_curve.eval(t)
        } else {
            1.0
        };

        // Fade-out
        let fade_out_gain = if self.fade_out_samples > 0
            && effective_dur > self.fade_out_samples
            && position_in_clip >= effective_dur - self.fade_out_samples
        {
            let remaining = effective_dur - position_in_clip;
            let t = remaining as f32 / self.fade_out_samples as f32;
            self.fade_curve.eval(t)
        } else {
            1.0
        };

        fade_in_gain * fade_out_gain
    }
}

/// A clip placed at a specific position on a track's timeline.
#[derive(Debug, Clone)]
pub struct Region {
    /// The clip (audio source + editing parameters)
    pub clip: Clip,
    /// Start position on the timeline in samples
    pub position_samples: u64,
}

impl Region {
    pub fn new(clip: Clip, position_samples: u64) -> Self {
        Self {
            clip,
            position_samples,
        }
    }

    /// End position on the timeline in samples.
    pub fn end_samples(&self) -> u64 {
        self.position_samples + self.clip.effective_duration_samples()
    }

    /// Check if this region overlaps with a given time range [start, start+length).
    pub fn overlaps(&self, start: u64, length: u64) -> bool {
        let region_end = self.end_samples();
        let range_end = start + length;
        self.position_samples < range_end && region_end > start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_gain_no_fade() {
        let clip = Clip::new(AudioSource::Driver, 48000);
        assert!((clip.gain_at(0) - 1.0).abs() < 1e-6);
        assert!((clip.gain_at(24000) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_clip_gain_with_db() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        clip.gain_db = -6.0;
        let expected = 10.0f32.powf(-6.0 / 20.0);
        assert!((clip.gain_at(24000) - expected).abs() < 1e-4);
    }

    #[test]
    fn test_clip_linear_gain_cache_tracks_public_gain_field() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        assert!((clip.linear_gain() - 1.0).abs() < 1e-6);

        clip.gain_db = -12.0;
        let expected = 10.0f32.powf(-12.0 / 20.0);
        assert!((clip.linear_gain() - expected).abs() < 1e-6);

        clip.gain_db = 6.0;
        let expected = 10.0f32.powf(6.0 / 20.0);
        assert!((clip.linear_gain() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_clip_fade_in() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        clip.fade_in_samples = 4800; // 100ms at 48kHz
        // At position 0: fade-in gain = 0
        assert!((clip.gain_at(0)).abs() < 1e-6);
        // At half fade: 0.5 (linear)
        assert!((clip.gain_at(2400) - 0.5).abs() < 1e-4);
        // After fade: 1.0
        assert!((clip.gain_at(4800) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_clip_fade_out() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        clip.fade_out_samples = 4800;
        // Well before fade-out: 1.0
        assert!((clip.gain_at(0) - 1.0).abs() < 1e-6);
        // At fade-out start (48000 - 4800 = 43200): 1.0
        assert!((clip.gain_at(43200) - 1.0).abs() < 1e-4);
        // At end: 0.0
        assert!((clip.gain_at(48000)).abs() < 1e-4);
    }

    #[test]
    fn test_fade_curve_scurve() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        clip.fade_in_samples = 1000;
        clip.fade_curve = FadeCurve::SCurve;
        // S-curve at t=0.5: 3*(0.25) - 2*(0.125) = 0.75 - 0.25 = 0.5
        assert!((clip.gain_at(500) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_region_overlaps() {
        let clip = Clip::new(AudioSource::Driver, 48000);
        let region = Region::new(clip, 10000);
        // Region: [10000, 58000)
        assert!(region.overlaps(0, 20000)); // overlaps start
        assert!(region.overlaps(50000, 20000)); // overlaps end
        assert!(region.overlaps(20000, 10000)); // fully inside
        assert!(!region.overlaps(0, 5000)); // before
        assert!(!region.overlaps(60000, 5000)); // after
    }

    #[test]
    fn test_effective_duration_with_time_stretch() {
        let mut clip = Clip::new(AudioSource::Driver, 48000);
        clip.time_stretch_ratio = 2.0; // Double speed = half duration
        assert_eq!(clip.effective_duration_samples(), 24000);
    }
}
