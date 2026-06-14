pub use sotf_audio_player::room_eq_types::AutoEqField;

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub opt_mode_open: bool,
    pub fir_phase_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// DE strategy dropdown
    pub strategy_open: bool,
    /// Local algorithm dropdown
    pub local_algo_open: bool,
    /// Bayesian optimization acquisition dropdown
    pub bo_acquisition_open: bool,
    /// Loss type dropdown
    pub loss_type_open: bool,
    /// Target curve dropdown
    pub target_curve_open: bool,
    /// System type dropdown
    pub system_type_open: bool,

    // Advanced dropdowns
    pub tilt_type_open: bool,
    pub excursion_filter_type_open: bool,
    pub multi_seat_strategy_open: bool,

    // v2 dropdowns
    pub mixed_crossover_type_open: bool,
    pub mixed_fir_band_open: bool,
    pub vog_reference_channel_open: bool,
    pub multi_measurement_strategy_open: bool,

    /// Review step smoothing dropdown
    pub review_smoothing_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
    /// Custom target curve editor modal open
    pub custom_target_modal_open: bool,
    /// Custom target presets dropdown open
    pub custom_target_presets_open: bool,
    /// Currently dragging control point index (None if not dragging)
    pub dragging_control_point: Option<usize>,
}

impl Default for RoomEqDropdowns {
    fn default() -> Self {
        Self {
            data_source_open: false,
            opt_mode_open: false,
            fir_phase_open: false,
            algorithm_open: false,
            peq_model_open: false,
            crossover_type_open: false,
            export_format_open: false,
            strategy_open: false,
            local_algo_open: false,
            bo_acquisition_open: false,
            loss_type_open: false,
            target_curve_open: false,
            system_type_open: false,
            tilt_type_open: true,
            excursion_filter_type_open: false,
            multi_seat_strategy_open: false,
            mixed_crossover_type_open: false,
            mixed_fir_band_open: false,
            vog_reference_channel_open: false,
            multi_measurement_strategy_open: false,
            review_smoothing_open: false,
            autoeq_editing_field: None,
            autoeq_edit_text: String::new(),
            custom_target_modal_open: false,
            custom_target_presets_open: false,
            dragging_control_point: None,
        }
    }
}
