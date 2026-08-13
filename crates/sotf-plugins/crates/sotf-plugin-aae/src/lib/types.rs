use sotf_host::auto_gain::AutoGainData;

#[derive(Debug, Clone)]
pub struct AaeData {
    pub auto_gain: AutoGainData,
    /// Current content-aware wet gain. One means no ducking.
    pub dialogue_duck_gain: f32,
    /// Current detector decision, including the configured hold interval.
    pub dialogue_active: bool,
    /// Current linked output-limiter gain. One means no limiting.
    pub output_limiter_gain: f32,
}
