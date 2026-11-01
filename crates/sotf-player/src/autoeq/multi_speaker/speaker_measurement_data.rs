/// Measurement data for a single speaker in multi-speaker optimization
#[derive(Debug, Clone)]
pub struct SpeakerMeasurementData {
    /// Speaker/channel name (e.g., "Left", "Right", "Center")
    pub name: String,
    /// Input measurement curve
    pub input_curve: autoeq_core::Curve,
    /// Target curve for this speaker
    pub target_curve: autoeq_core::Curve,
    /// Weight for this speaker in the combined loss (default 1.0)
    pub weight: f64,
}

impl SpeakerMeasurementData {
    /// Create a new speaker measurement with default weight
    pub fn new(
        name: &str,
        input_curve: autoeq_core::Curve,
        target_curve: autoeq_core::Curve,
    ) -> Self {
        Self {
            name: name.to_string(),
            input_curve,
            target_curve,
            weight: 1.0,
        }
    }

    /// Set the weight for this speaker
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}
