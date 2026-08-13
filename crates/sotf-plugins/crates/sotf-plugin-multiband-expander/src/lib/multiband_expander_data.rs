#[derive(Debug, Clone, Default)]
pub struct MultibandExpanderData {
    /// Attenuation per band and per channel (flattened)
    pub attenuation_db: Vec<f32>,
    pub is_open: Vec<bool>,
    pub band_levels_db: Vec<f32>,
    pub crossover_frequencies: Vec<f32>,
}

impl MultibandExpanderData {
    pub fn new(num_bands: usize, channels: usize) -> Self {
        Self {
            attenuation_db: vec![0.0; num_bands * channels],
            is_open: vec![false; num_bands],
            band_levels_db: vec![-120.0; num_bands],
            crossover_frequencies: vec![0.0; num_bands.saturating_sub(1)],
        }
    }

    pub fn update(&mut self, atten: &[f32], open: &[bool], levels: &[f32], xovers: &[f32]) {
        if self.attenuation_db.len() == atten.len() {
            self.attenuation_db.copy_from_slice(atten);
        }
        if self.is_open.len() == open.len() {
            self.is_open.copy_from_slice(open);
        }
        if self.band_levels_db.len() == levels.len() {
            self.band_levels_db.copy_from_slice(levels);
        }
        if self.crossover_frequencies.len() == xovers.len() {
            self.crossover_frequencies.copy_from_slice(xovers);
        }
    }
}
