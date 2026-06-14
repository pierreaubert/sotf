/// Summary statistics for an internal Upmixer control vector.
#[derive(Debug, Clone, Copy, Default)]
pub struct UpmixerControlStats {
    pub mean: f32,
    pub min: f32,
    pub max: f32,
    pub stddev: f32,
}

/// Snapshot of internal Upmixer control signals useful for artifact diagnosis.
#[derive(Debug, Clone, Default)]
pub struct UpmixerDiagnostics {
    pub sample_rate: u32,
    pub fft_size: usize,
    pub hop_size: usize,
    pub output_channels: usize,
    pub speaker_config: String,
    pub dialogue_probability: f32,
    pub dialogue_spatial_control: f32,
    pub dialogue_spectral_centroid_hz: f32,
    pub dialogue_envelope_variance: f32,
    pub decorrelation_strength: f32,
    pub hr_direct_envelope: f32,
    pub hr_transient_env: f32,
    pub height_transient_env: f32,
    pub spectral_flux_smooth: f32,
    pub height_spectral_flux_smooth: f32,
    pub safety_scale: f32,
    pub output_accumulator_fill: usize,
    pub height_gain: UpmixerControlStats,
    pub height_flux_gate: UpmixerControlStats,
    pub coherence: UpmixerControlStats,
}

pub(super) fn control_stats(values: &[f32]) -> UpmixerControlStats {
    if values.is_empty() {
        return UpmixerControlStats::default();
    }

    let mut sum = 0.0_f32;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values {
        sum += v;
        min = min.min(v);
        max = max.max(v);
    }

    let mean = sum / values.len() as f32;
    let mut variance = 0.0_f32;
    for &v in values {
        let d = v - mean;
        variance += d * d;
    }
    variance /= values.len() as f32;

    UpmixerControlStats {
        mean,
        min,
        max,
        stddev: variance.sqrt(),
    }
}
