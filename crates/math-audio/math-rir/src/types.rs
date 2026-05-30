/// A single segment of the RIR identified by SSIR analysis.
///
/// Each segment represents a discrete acoustic event (direct sound or early reflection)
/// with a constant direction of arrival (DOA). Segments are consecutive — the end of
/// one segment is the onset of the next, preserving the full temporal energy profile.
#[derive(Debug, Clone)]
pub struct RirSegment {
    /// Start sample of this segment (onset)
    pub onset_sample: usize,
    /// End sample (exclusive) — equals the next segment's onset, or mixing time for the last segment
    pub end_sample: usize,
    /// Sample index of the peak arrival (TOA) within this segment
    pub toa_sample: usize,
    /// Direction of arrival as a unit vector [x, y, z], if available from multi-channel input
    pub doa: Option<[f32; 3]>,
    /// Peak energy (squared amplitude) at the TOA sample
    pub peak_energy: f64,
    /// Whether this segment contains the direct sound
    pub is_direct_sound: bool,
}

impl RirSegment {
    /// Duration of this segment in samples
    pub fn len(&self) -> usize {
        self.end_sample.saturating_sub(self.onset_sample)
    }

    /// Whether this segment has zero length
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Duration of this segment in seconds
    pub fn duration_secs(&self, sample_rate: f64) -> f64 {
        self.len() as f64 / sample_rate
    }

    /// Duration of this segment in milliseconds
    pub fn duration_ms(&self, sample_rate: f64) -> f64 {
        self.duration_secs(sample_rate) * 1000.0
    }

    /// Time of arrival relative to the RIR start, in milliseconds
    pub fn toa_ms(&self, sample_rate: f64) -> f64 {
        self.toa_sample as f64 / sample_rate * 1000.0
    }

    /// DOA azimuth in degrees (0 = front, positive = left), if DOA is available.
    /// Computed from the x,y components of the DOA unit vector.
    pub fn azimuth_deg(&self) -> Option<f32> {
        self.doa.map(|d| d[1].atan2(d[0]).to_degrees())
    }

    /// DOA elevation in degrees, if DOA is available.
    /// Computed from the z component of the DOA unit vector.
    pub fn elevation_deg(&self) -> Option<f32> {
        self.doa
            .map(|d| d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt()).to_degrees())
    }
}

/// Result of SSIR analysis on a room impulse response.
#[derive(Debug, Clone)]
pub struct SsirResult {
    /// Ordered sequence of segments covering the early RIR.
    /// First segment is always the direct sound.
    /// Segments are consecutive: segment[i].end_sample == segment[i+1].onset_sample
    pub segments: Vec<RirSegment>,
    /// Estimated mixing time in samples (boundary between early reflections and reverberant tail)
    pub mixing_time_samples: usize,
    /// Sample rate used for analysis
    pub sample_rate: f64,
}

impl SsirResult {
    /// Number of detected sound events (direct sound + early reflections)
    pub fn num_events(&self) -> usize {
        self.segments.len()
    }

    /// Number of early reflections (excludes direct sound)
    pub fn num_reflections(&self) -> usize {
        self.segments.len().saturating_sub(1)
    }

    /// Mixing time in milliseconds
    pub fn mixing_time_ms(&self) -> f64 {
        self.mixing_time_samples as f64 / self.sample_rate * 1000.0
    }

    /// Iterator over only the early reflection segments (excludes direct sound)
    pub fn reflections(&self) -> impl Iterator<Item = &RirSegment> {
        self.segments.iter().filter(|s| !s.is_direct_sound)
    }

    /// The direct sound segment, if detected
    pub fn direct_sound(&self) -> Option<&RirSegment> {
        self.segments.first().filter(|s| s.is_direct_sound)
    }

    /// Direction of arrival for the direct sound, if detected from SRIR input.
    pub fn direct_sound_doa(&self) -> Option<[f32; 3]> {
        self.direct_sound().and_then(|s| s.doa)
    }
}
