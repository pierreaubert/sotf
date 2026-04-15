//! UI state for AutoEQ form dropdowns.

pub use sotf_audio_player::autoeq::DetailLevel;

/// UI state for AutoEQ form dropdowns
#[derive(Debug, Clone, Default)]
pub struct AutoEqFormUiState {
    /// How much detail to show (Simple / Intermediate / Expert).
    pub detail_level: DetailLevel,
    /// Preset selector dropdown open state.
    pub preset_open: bool,
    /// Currently selected preset id (e.g. "balanced", "custom").
    pub selected_preset: Option<String>,
    /// EQ Mode dropdown open state
    pub opt_mode_open: bool,
    /// FIR Phase dropdown open state
    pub fir_phase_open: bool,
    /// Algorithm dropdown open state
    pub algo_open: bool,
    /// PEQ model dropdown open state
    pub peq_model_open: bool,
    /// DE strategy dropdown open state
    pub strategy_open: bool,
    /// Local algorithm dropdown open state
    pub local_algo_open: bool,
    /// Loss type dropdown open state
    pub loss_type_open: bool,
    /// Target curve dropdown open state
    pub target_curve_open: bool,
    /// System type dropdown open state
    pub system_type_open: bool,

    /// Tilt type dropdown open state
    pub tilt_type_open: bool,
    /// Excursion filter type dropdown open state
    pub excursion_filter_type_open: bool,
    /// Multi-seat strategy dropdown open state
    pub multi_seat_strategy_open: bool,

    // v2 dropdown states
    /// Mixed crossover type dropdown open state
    pub mixed_crossover_type_open: bool,
    /// Mixed FIR band dropdown open state
    pub mixed_fir_band_open: bool,
    /// VoG reference channel dropdown open state
    pub vog_reference_channel_open: bool,
    /// Multi-measurement strategy dropdown open state
    pub multi_measurement_strategy_open: bool,

    /// Currently focused parameter block (drives the docs panel).
    /// Set by hovering over a block section. `None` shows the overview.
    pub focused_block: Option<&'static str>,

    /// Selected target distance preset (near/mid/far/custom).
    /// UI helper only — drives `tilt_slope` pre-fill.
    pub selected_target_distance: Option<String>,
    /// Selected optimization goal preset (match_target/natural/psychoacoustic).
    /// Derived from loss_type + asymmetric_loss + psychoacoustic config fields.
    pub selected_optimization_goal: Option<String>,
}
