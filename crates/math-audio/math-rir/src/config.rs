/// Configuration for SSIR (Spatial Segmentation of Impulse Response) analysis.
///
/// Default values correspond to the SSIR-Mk2 configuration from
/// Pawlak & Lee (Applied Acoustics 249, 2026), Table 1.
#[derive(Debug, Clone)]
pub struct SsirConfig {
    /// Sample rate in Hz
    pub sample_rate: f64,

    /// Direct sound window: (pre, post) in ms relative to detected onset.
    /// Reflections within this window are excluded from detection.
    /// Default: (0.5, 3.5) — the direct sound typically occupies ~4ms.
    pub direct_sound_window_ms: (f64, f64),

    /// Local Energy Ratio analysis window length in ms.
    /// The RIR is divided into consecutive windows of this length.
    /// Default: 1.0 ms (48 samples @ 48kHz).
    pub ler_window_ms: f64,

    /// Energy threshold as a multiple of the per-window median energy.
    /// A sample is considered a reflection candidate if its energy exceeds
    /// this multiple of the window's median energy.
    /// Default: 3.0
    pub energy_threshold: f64,

    /// Minimum angular distance (degrees) between consecutive reflections
    /// for them to be considered distinct events.
    /// Pairs below this threshold are merged.
    /// Default: 9.0 degrees. Only used with multi-channel (SRIR) input.
    pub doa_threshold_deg: f64,

    /// Minimum time-of-arrival difference (ms) between consecutive reflections.
    /// Pairs closer than this are merged regardless of DOA.
    /// Default: 0.5 ms.
    pub toa_threshold_ms: f64,

    /// Minimum segment duration (ms) for early reflections.
    /// Segments shorter than this are merged with the preceding segment.
    /// Default: 0.5 ms.
    pub min_segment_ms: f64,

    /// Mixing time in ms (boundary between early reflections and reverberant tail).
    /// If None, estimated automatically from the Schroeder decay curve.
    /// Default: None (auto-estimate, typical values: 30-50ms for small rooms).
    pub mixing_time_ms: Option<f64>,

    /// Pre-onset window length (ms) for refining segment boundaries.
    /// For each detected reflection, the onset is searched within
    /// [TOA - onset_window_ms, TOA].
    /// Default: 0.5 ms.
    pub onset_window_ms: f64,

    /// Duration (ms) of the optional final segment after the last detected event.
    /// Default: 2.0 ms.
    pub final_segment_ms: f64,

    /// Minimum peak distance (ms) for direct sound onset detection.
    /// Peaks closer than this are suppressed when searching for the direct sound.
    /// Default: 0.1 ms (5 samples @ 48kHz).
    pub min_peak_distance_ms: f64,
}

impl SsirConfig {
    /// Create a config with the given sample rate and all other values at defaults.
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            ..Self::default_at(sample_rate)
        }
    }

    /// Create default config at a specific sample rate.
    fn default_at(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            direct_sound_window_ms: (0.5, 3.5),
            ler_window_ms: 1.0,
            energy_threshold: 3.0,
            doa_threshold_deg: 9.0,
            toa_threshold_ms: 0.5,
            min_segment_ms: 0.5,
            mixing_time_ms: None,
            onset_window_ms: 0.5,
            final_segment_ms: 2.0,
            min_peak_distance_ms: 0.1,
        }
    }

    // -- helper conversions --

    /// Convert milliseconds to samples at the configured sample rate.
    pub(crate) fn ms_to_samples(&self, ms: f64) -> usize {
        (ms * self.sample_rate / 1000.0).round() as usize
    }

    /// LER window length in samples.
    pub(crate) fn ler_window_samples(&self) -> usize {
        self.ms_to_samples(self.ler_window_ms)
    }

    /// Direct sound window as (pre_samples, post_samples) relative to onset.
    pub(crate) fn direct_sound_window_samples(&self) -> (usize, usize) {
        (
            self.ms_to_samples(self.direct_sound_window_ms.0),
            self.ms_to_samples(self.direct_sound_window_ms.1),
        )
    }

    /// TOA threshold in samples.
    pub(crate) fn toa_threshold_samples(&self) -> usize {
        self.ms_to_samples(self.toa_threshold_ms)
    }

    /// Minimum segment duration in samples.
    pub(crate) fn min_segment_samples(&self) -> usize {
        self.ms_to_samples(self.min_segment_ms)
    }

    /// Onset window in samples.
    pub(crate) fn onset_window_samples(&self) -> usize {
        self.ms_to_samples(self.onset_window_ms)
    }

    /// Mixing time in samples (using configured or default fallback of 38ms).
    pub(crate) fn mixing_time_samples(&self) -> usize {
        self.ms_to_samples(self.mixing_time_ms.unwrap_or(38.0))
    }

    /// Final segment duration in samples.
    pub(crate) fn final_segment_samples(&self) -> usize {
        self.ms_to_samples(self.final_segment_ms)
    }

    /// Minimum peak distance in samples for onset detection.
    pub(crate) fn min_peak_distance_samples(&self) -> usize {
        self.ms_to_samples(self.min_peak_distance_ms).max(1)
    }
}

impl Default for SsirConfig {
    fn default() -> Self {
        Self::default_at(48000.0)
    }
}
