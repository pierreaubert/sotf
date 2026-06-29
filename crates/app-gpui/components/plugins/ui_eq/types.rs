use sotf_audio_player::EQFilter;
use sotf_audio_player_midi::mapping::MidiOverlay;

/// View variant for the shared EQ renderer.
///
/// `Standard` is the regular biquad EQ (per-channel mode toggle visible).
/// `LinearPhase` reuses the same band editor for the linear-phase FIR EQ:
/// per-channel mode is hidden (always global), and a header shows the FIR
/// latency plus linear-phase-only controls.
#[derive(Debug)]
pub enum EqViewMode {
    Standard,
    LinearPhase {
        /// Latency in samples = (fir_length - 1) / 2
        latency_samples: usize,
        /// Latency in milliseconds at the current sample rate
        latency_ms: f32,
        /// Current FIR length (samples) for display
        fir_length: usize,
        /// Whether auto_gain is enabled
        auto_gain: bool,
        /// Wet/dry mix
        mix: f64,
    },
    FirDesigner {
        /// Latency in samples = (fir_length - 1) / 2 in linear mode.
        latency_samples: usize,
        /// Latency in milliseconds at the current sample rate.
        latency_ms: f32,
        /// Current FIR length (samples) for display.
        fir_length: usize,
        /// Current FIR phase design mode.
        phase_mode: &'static str,
        /// Whether auto_gain is enabled.
        auto_gain: bool,
        /// Wet/dry mix
        mix: f64,
    },
}

/// State for rendering the EQ plugin
#[derive(Debug)]
pub struct EqRenderState<'a> {
    /// Number of channels
    pub channels: usize,
    /// Global filters (used when per_channel_mode is false)
    pub filters: &'a [EQFilter],
    /// Per-channel filters (used when per_channel_mode is true)
    pub channel_filters: &'a Option<Vec<Vec<EQFilter>>>,
    /// Whether to use per-channel mode
    pub per_channel_mode: bool,
    /// Whether the user is currently dragging or editing a band
    pub is_editing: bool,
    /// Index of the selected parameter (e.g. gain, frequency, q)
    pub selected_param: usize,
    /// Index of the currently selected EQ band
    pub selected_band_idx: usize,
    /// MIDI overlay for displaying controller assignments on EQ bands
    pub midi_overlay: Option<&'a MidiOverlay>,
    /// View variant — Standard biquad EQ or LinearPhase FIR EQ
    pub mode: EqViewMode,
    /// Number of active filters (for EQ, LinearPhaseEq, FirDesigner)
    pub num_filters: usize,
    /// Use Transposed Direct Form II (Standard EQ only)
    pub tdf2: bool,
    /// Filter topology: 0 = Biquad, 1 = SVF (Standard EQ only)
    pub topology: f64,
    /// Available width in pixels for responsive compact-layout selection
    pub available_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqCompactLayout {
    Current,
    BottomStrip,
    Inspector,
}

impl EqCompactLayout {
    pub fn from_width(width: f32) -> Self {
        if width >= 900.0 {
            Self::Current
        } else if width >= 600.0 {
            Self::BottomStrip
        } else {
            Self::Inspector
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EqCompactLayout;

    #[test]
    fn layout_selection_breakpoints() {
        assert_eq!(EqCompactLayout::from_width(1000.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(900.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(750.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(600.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(599.0), EqCompactLayout::Inspector);
        assert_eq!(EqCompactLayout::from_width(320.0), EqCompactLayout::Inspector);
    }
}
