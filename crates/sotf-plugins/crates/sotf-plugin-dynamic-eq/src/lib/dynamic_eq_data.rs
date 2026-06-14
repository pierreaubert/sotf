use std::sync::Arc;

/// Per-band gain reduction for UI meters.
#[derive(Debug, Clone)]
pub struct DynamicEqData {
    /// Gain reduction in dB per band.
    pub gain_reduction_db: Arc<Vec<f32>>,
}

impl Default for DynamicEqData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
        }
    }
}

impl DynamicEqData {
    pub fn new(num_bands: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; num_bands]),
        }
    }

    pub fn update(&mut self, gr: &[f32]) {
        if let Some(v) = Arc::get_mut(&mut self.gain_reduction_db)
            && v.len() == gr.len()
        {
            v.copy_from_slice(gr);
        }
    }
}
