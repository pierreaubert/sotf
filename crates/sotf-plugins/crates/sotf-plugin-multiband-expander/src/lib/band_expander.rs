use super::types::GateState;

pub(super) struct BandExpander {
    pub(super) envelope: Vec<f32>,
    /// Peak envelope follower per channel (linear amplitude).
    /// Prevents instantaneous zero-crossing dips from inflating expansion.
    pub(super) peak_env: Vec<f32>,
    pub(super) gate_state: Vec<GateState>,
    pub(super) hold_counter: Vec<usize>,
    pub(super) attack_coeff: f32,
    pub(super) release_coeff: f32,
}

impl BandExpander {
    pub(super) fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.peak_env.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
    }
}
