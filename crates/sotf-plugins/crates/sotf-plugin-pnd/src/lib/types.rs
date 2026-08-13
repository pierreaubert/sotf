/// Data exposed by the PND plugin for drift monitoring.
#[derive(Debug, Clone)]
pub struct PndData {
    /// Current raw drift ratio from analysis (1.0 = no drift).
    pub drift_ratio: f64,
    /// Current pitch ratio applied by the duration-preserving engine.
    pub correction_ratio: f64,
    /// Confidence of the drift estimate (0.0 to 1.0).
    pub confidence: f32,
    /// Number of matched partials in the last FFT frame.
    pub matched_partials: usize,
    /// Total number of detected peaks in the last FFT frame.
    pub total_peaks: usize,
}

impl Default for PndData {
    fn default() -> Self {
        Self {
            drift_ratio: 1.0,
            correction_ratio: 1.0,
            confidence: 0.0,
            matched_partials: 0,
            total_peaks: 0,
        }
    }
}
