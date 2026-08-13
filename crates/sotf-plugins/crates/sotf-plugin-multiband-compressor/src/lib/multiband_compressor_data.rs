#[derive(Debug, Clone, Default)]
pub struct MultibandCompressorData {
    /// Gain reduction per band and per channel (flattened: [band0_ch0, band0_ch1, ..., band1_ch0, ...])
    pub gain_reduction_db: Vec<f32>,
    pub band_levels_db: Vec<f32>,
    pub crossover_frequencies: Vec<f32>,
}

impl MultibandCompressorData {
    pub fn new(num_bands: usize, channels: usize) -> Self {
        Self {
            gain_reduction_db: vec![0.0; num_bands * channels],
            band_levels_db: vec![-120.0; num_bands],
            crossover_frequencies: vec![0.0; num_bands.saturating_sub(1)],
        }
    }

    pub fn update(&mut self, gains: &[f32], levels: &[f32], xovers: &[f32]) {
        if self.gain_reduction_db.len() == gains.len() {
            self.gain_reduction_db.copy_from_slice(gains);
        }
        if self.band_levels_db.len() == levels.len() {
            self.band_levels_db.copy_from_slice(levels);
        }
        if self.crossover_frequencies.len() == xovers.len() {
            self.crossover_frequencies.copy_from_slice(xovers);
        }
    }
}
