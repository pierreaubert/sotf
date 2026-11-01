use super::misc::requires_room_eq_graph;
pub use roomeq_model::DspChainOutput;

/// Extension trait for `DspChainOutput` providing player-side helpers.
///
/// Lives here (not in `autoeq`) because it's a player concern: does this
/// chain correspond to a linear rack, or does it need a parallel
/// multi-driver graph? The autoeq crate doesn't know or care about the
/// player's rack model.
pub trait DspChainOutputExt {
    /// Returns true when named main/broadband EQ plugins preserve every DSP stage.
    fn is_rack_compatible(&self) -> bool;

    /// Returns true when graph playback must preserve routing or additional stages.
    fn requires_room_eq_graph(&self) -> bool;
}

impl DspChainOutputExt for DspChainOutput {
    fn is_rack_compatible(&self) -> bool {
        !self.requires_room_eq_graph()
    }

    fn requires_room_eq_graph(&self) -> bool {
        requires_room_eq_graph(self)
    }
}
