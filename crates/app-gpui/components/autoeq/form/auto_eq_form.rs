use super::super::config::AutoEqConfig;
use super::super::constants::OptimizationType;
use super::super::theme::AutoEqFormTheme;
use super::super::ui_state::AutoEqFormUiState;
use super::types::ActionCallback;
use super::types::AutoEqLayoutMode;
use super::types::BoolCallback;
use super::types::F64Callback;
use super::types::StringCallback;
use super::types::ToggleCallback;
use super::types::UsizeCallback;
use crate::i18n::Language;
use gpui::*;

/// A reusable form for AutoEQ optimization parameters.
///
/// Renders three sections:
/// 1. Goals & Configuration - system type, targets, and EQ mode
/// 2. EQ Design Parameters - filter characteristics and frequency ranges
/// 3. Optimization Fine Tuning - algorithm settings and DE parameters
///
/// The form adapts its options based on `optimization_type`:
/// - **Speaker**: Shows system type, target curves include flat, custom, and spinorama curves
/// - **Headphone**: Hides system type, target curves include Harman curves
pub(crate) struct FormMeta {
    pub(crate) layout_mode: AutoEqLayoutMode,
    pub(crate) id: ElementId,
    pub(crate) config: AutoEqConfig,
    pub(crate) ui_state: AutoEqFormUiState,
    pub(crate) disabled: bool,
    pub(crate) optimization_type: OptimizationType,
    pub(crate) available_spinorama_curves: Vec<String>,
    pub(crate) theme: Option<AutoEqFormTheme>,
    pub(crate) allowed_opt_modes: Option<Vec<String>>,
    pub(crate) available_width: f32,
    pub(crate) language: Language,
}

#[derive(Default)]
pub(crate) struct FormVisibility {
    pub(crate) show_goals: bool,
    pub(crate) show_eq_design: bool,
    pub(crate) show_optimization_tuning: bool,
    pub(crate) hide_de_params: bool,
    pub(crate) hide_smoothing: bool,
    pub(crate) hide_spacing: bool,
    pub(crate) hide_tolerance: bool,
    pub(crate) hide_sample_rate: bool,
    pub(crate) hide_phase_alignment: bool,
    pub(crate) hide_multi_seat: bool,
    pub(crate) hide_scenario_a_text: bool,
    pub(crate) hide_room_sections: bool,
    pub(crate) hide_multi_measurement: bool,
    pub(crate) hide_capability_section: bool,
    pub(crate) hide_target_distance_section: bool,
    pub(crate) hide_optimization_goal_section: bool,
    pub(crate) hide_bass_management: bool,
    pub(crate) hide_asymmetric_loss: bool,
    pub(crate) hide_broadband_matching: bool,
    pub(crate) loss_type_options_override: Option<&'static [(&'static str, &'static str)]>,
}

#[derive(Default)]
pub(crate) struct EqDesignCallbacks {
    pub(crate) on_opt_mode_change: Option<StringCallback>,
    pub(crate) on_opt_mode_toggle: Option<ToggleCallback>,
    pub(crate) on_fir_taps_change: Option<UsizeCallback>,
    pub(crate) on_fir_phase_change: Option<StringCallback>,
    pub(crate) on_fir_phase_toggle: Option<ToggleCallback>,
    pub(crate) on_num_filters_change: Option<UsizeCallback>,
    pub(crate) on_sample_rate_change: Option<UsizeCallback>,
    pub(crate) on_min_db_change: Option<F64Callback>,
    pub(crate) on_max_db_change: Option<F64Callback>,
    pub(crate) on_min_q_change: Option<F64Callback>,
    pub(crate) on_max_q_change: Option<F64Callback>,
    pub(crate) on_min_freq_change: Option<F64Callback>,
    pub(crate) on_max_freq_change: Option<F64Callback>,
    pub(crate) on_peq_model_change: Option<StringCallback>,
    pub(crate) on_peq_model_toggle: Option<ToggleCallback>,
    pub(crate) on_spacing_weight_change: Option<F64Callback>,
    pub(crate) on_min_spacing_oct_change: Option<F64Callback>,
}

#[derive(Default)]
pub(crate) struct OptimizationCallbacks {
    pub(crate) on_algo_change: Option<StringCallback>,
    pub(crate) on_algo_toggle: Option<ToggleCallback>,
    pub(crate) on_population_change: Option<UsizeCallback>,
    pub(crate) on_maxeval_change: Option<UsizeCallback>,
    pub(crate) on_tolerance_change: Option<F64Callback>,
    pub(crate) on_atolerance_change: Option<F64Callback>,
    pub(crate) on_bo_initial_samples_change: Option<UsizeCallback>,
    pub(crate) on_bo_batch_size_change: Option<UsizeCallback>,
    pub(crate) on_bo_posterior_std_threshold_change: Option<F64Callback>,
    pub(crate) on_bo_acquisition_change: Option<StringCallback>,
    pub(crate) on_bo_acquisition_toggle: Option<ToggleCallback>,
    pub(crate) on_bo_ehvi_change: Option<BoolCallback>,
    pub(crate) on_de_f_change: Option<F64Callback>,
    pub(crate) on_de_cr_change: Option<F64Callback>,
    pub(crate) on_strategy_change: Option<StringCallback>,
    pub(crate) on_strategy_toggle: Option<ToggleCallback>,
    pub(crate) on_adaptive_weight_f_change: Option<F64Callback>,
    pub(crate) on_adaptive_weight_cr_change: Option<F64Callback>,
    pub(crate) on_refine_change: Option<BoolCallback>,
    pub(crate) on_local_algo_change: Option<StringCallback>,
    pub(crate) on_local_algo_toggle: Option<ToggleCallback>,
    pub(crate) on_smooth_change: Option<BoolCallback>,
    pub(crate) on_smooth_n_change: Option<UsizeCallback>,
    pub(crate) on_psychoacoustic_change: Option<BoolCallback>,
    pub(crate) on_asymmetric_loss_change: Option<BoolCallback>,
}

#[derive(Default)]
pub(crate) struct GoalsCallbacks {
    pub(crate) on_loss_type_change: Option<StringCallback>,
    pub(crate) on_loss_type_toggle: Option<ToggleCallback>,
    pub(crate) on_target_curve_change: Option<StringCallback>,
    pub(crate) on_target_curve_toggle: Option<ToggleCallback>,
    pub(crate) on_edit_custom_target: Option<ActionCallback>,
    pub(crate) on_system_type_change: Option<StringCallback>,
    pub(crate) on_system_type_toggle: Option<ToggleCallback>,
}

#[derive(Default)]
pub(crate) struct RoomCorrectionCallbacks {
    pub(crate) on_use_target_tilt_change: Option<BoolCallback>,
    pub(crate) on_tilt_type_change: Option<StringCallback>,
    pub(crate) on_tilt_type_toggle: Option<ToggleCallback>,
    pub(crate) on_tilt_slope_change: Option<F64Callback>,
    pub(crate) on_tilt_reference_freq_change: Option<F64Callback>,
    pub(crate) on_tilt_bass_shelf_db_change: Option<F64Callback>,
    pub(crate) on_tilt_bass_shelf_freq_change: Option<F64Callback>,
    pub(crate) on_use_excursion_protection_change: Option<BoolCallback>,
    pub(crate) on_excursion_auto_detect_f3_change: Option<BoolCallback>,
    pub(crate) on_excursion_manual_f3_change: Option<F64Callback>,
    pub(crate) on_excursion_filter_order_change: Option<UsizeCallback>,
    pub(crate) on_excursion_filter_type_change: Option<StringCallback>,
    pub(crate) on_excursion_filter_type_toggle: Option<ToggleCallback>,
    pub(crate) on_excursion_margin_octaves_change: Option<F64Callback>,
    pub(crate) on_use_schroeder_split_change: Option<BoolCallback>,
    pub(crate) on_schroeder_freq_change: Option<F64Callback>,
    pub(crate) on_schroeder_low_max_q_change: Option<F64Callback>,
    pub(crate) on_schroeder_low_allow_boost_change: Option<BoolCallback>,
    pub(crate) on_schroeder_high_max_q_change: Option<F64Callback>,
    pub(crate) on_schroeder_high_shelving_only_change: Option<BoolCallback>,
    pub(crate) on_use_phase_alignment_change: Option<BoolCallback>,
    pub(crate) on_phase_min_freq_change: Option<F64Callback>,
    pub(crate) on_phase_max_freq_change: Option<F64Callback>,
    pub(crate) on_phase_optimize_polarity_change: Option<BoolCallback>,
    pub(crate) on_phase_max_delay_ms_change: Option<F64Callback>,
    pub(crate) on_use_multi_seat_change: Option<BoolCallback>,
    pub(crate) on_multi_seat_strategy_change: Option<StringCallback>,
    pub(crate) on_multi_seat_strategy_toggle: Option<ToggleCallback>,
    pub(crate) on_multi_seat_primary_seat_change: Option<UsizeCallback>,
    pub(crate) on_multi_seat_max_deviation_db_change: Option<F64Callback>,
}

#[derive(Default)]
pub(crate) struct V2Callbacks {
    pub(crate) on_allow_delay_change: Option<BoolCallback>,
    pub(crate) on_seed_enabled_change: Option<BoolCallback>,
    pub(crate) on_seed_change: Option<UsizeCallback>,
    pub(crate) on_vog_enabled_change: Option<BoolCallback>,
    pub(crate) on_vog_reference_channel_change: Option<StringCallback>,
    pub(crate) on_vog_reference_channel_toggle: Option<ToggleCallback>,
    pub(crate) on_broadband_target_matching_change: Option<BoolCallback>,
    pub(crate) on_mixed_crossover_freq_change: Option<F64Callback>,
    pub(crate) on_mixed_crossover_type_change: Option<StringCallback>,
    pub(crate) on_mixed_crossover_type_toggle: Option<ToggleCallback>,
    pub(crate) on_mixed_fir_band_change: Option<StringCallback>,
    pub(crate) on_mixed_fir_band_toggle: Option<ToggleCallback>,
}

#[derive(Default)]
pub(crate) struct MultiMeasurementCallbacks {
    pub(crate) on_use_multi_measurement_change: Option<BoolCallback>,
    pub(crate) on_multi_measurement_strategy_change: Option<StringCallback>,
    pub(crate) on_multi_measurement_strategy_toggle: Option<ToggleCallback>,
    pub(crate) on_multi_measurement_variance_lambda_change: Option<F64Callback>,
    pub(crate) on_multi_measurement_weight_change:
        Option<Box<dyn Fn(usize, f64, &mut Window, &mut App)>>,
}

#[derive(Default)]
pub(crate) struct FormLifecycleCallbacks {
    pub(crate) on_block_focus: Option<StringCallback>,
    pub(crate) on_detail_level_change: Option<StringCallback>,
    pub(crate) on_preset_change: Option<StringCallback>,
    pub(crate) on_preset_toggle: Option<ToggleCallback>,
    pub(crate) on_target_distance_change: Option<StringCallback>,
    pub(crate) on_optimization_goal_change: Option<StringCallback>,
}

#[derive(IntoElement)]
pub struct AutoEqForm {
    pub(crate) meta: FormMeta,
    pub(crate) visibility: FormVisibility,
    pub(crate) eq_design: EqDesignCallbacks,
    pub(crate) optimization: OptimizationCallbacks,
    pub(crate) goals: GoalsCallbacks,
    pub(crate) room_correction: RoomCorrectionCallbacks,
    pub(crate) v2: V2Callbacks,
    pub(crate) multi_measurement: MultiMeasurementCallbacks,
    pub(crate) lifecycle: FormLifecycleCallbacks,
}

impl AutoEqForm {
    /// Create a new AutoEQ form
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            meta: FormMeta {
                layout_mode: AutoEqLayoutMode::Default,
                id: id.into(),
                config: AutoEqConfig::default(),
                ui_state: AutoEqFormUiState::default(),
                disabled: false,
                optimization_type: OptimizationType::default(),
                available_spinorama_curves: Vec::new(),
                theme: None,
                allowed_opt_modes: None,
                available_width: 0.0,
                language: Language::English,
            },
            visibility: FormVisibility {
                show_goals: true,
                show_eq_design: true,
                show_optimization_tuning: true,
                hide_de_params: false,
                hide_smoothing: false,
                hide_spacing: false,
                hide_tolerance: false,
                hide_sample_rate: false,
                hide_phase_alignment: false,
                hide_multi_seat: false,
                hide_scenario_a_text: false,
                hide_room_sections: false,
                hide_multi_measurement: false,
                hide_capability_section: false,
                hide_target_distance_section: false,
                hide_optimization_goal_section: false,
                hide_bass_management: false,
                hide_asymmetric_loss: false,
                hide_broadband_matching: false,
                loss_type_options_override: None,
            },
            eq_design: EqDesignCallbacks {
                on_opt_mode_change: None,
                on_opt_mode_toggle: None,
                on_fir_taps_change: None,
                on_fir_phase_change: None,
                on_fir_phase_toggle: None,
                on_num_filters_change: None,
                on_sample_rate_change: None,
                on_min_db_change: None,
                on_max_db_change: None,
                on_min_q_change: None,
                on_max_q_change: None,
                on_min_freq_change: None,
                on_max_freq_change: None,
                on_peq_model_change: None,
                on_peq_model_toggle: None,
                on_spacing_weight_change: None,
                on_min_spacing_oct_change: None,
            },
            optimization: OptimizationCallbacks {
                on_algo_change: None,
                on_algo_toggle: None,
                on_population_change: None,
                on_maxeval_change: None,
                on_tolerance_change: None,
                on_atolerance_change: None,
                on_bo_initial_samples_change: None,
                on_bo_batch_size_change: None,
                on_bo_posterior_std_threshold_change: None,
                on_bo_acquisition_change: None,
                on_bo_acquisition_toggle: None,
                on_bo_ehvi_change: None,
                on_de_f_change: None,
                on_de_cr_change: None,
                on_strategy_change: None,
                on_strategy_toggle: None,
                on_adaptive_weight_f_change: None,
                on_adaptive_weight_cr_change: None,
                on_refine_change: None,
                on_local_algo_change: None,
                on_local_algo_toggle: None,
                on_smooth_change: None,
                on_smooth_n_change: None,
                on_psychoacoustic_change: None,
                on_asymmetric_loss_change: None,
            },
            goals: GoalsCallbacks {
                on_loss_type_change: None,
                on_loss_type_toggle: None,
                on_target_curve_change: None,
                on_target_curve_toggle: None,
                on_edit_custom_target: None,
                on_system_type_change: None,
                on_system_type_toggle: None,
            },
            room_correction: RoomCorrectionCallbacks {
                on_use_target_tilt_change: None,
                on_tilt_type_change: None,
                on_tilt_type_toggle: None,
                on_tilt_slope_change: None,
                on_tilt_reference_freq_change: None,
                on_tilt_bass_shelf_db_change: None,
                on_tilt_bass_shelf_freq_change: None,
                on_use_excursion_protection_change: None,
                on_excursion_auto_detect_f3_change: None,
                on_excursion_manual_f3_change: None,
                on_excursion_filter_order_change: None,
                on_excursion_filter_type_change: None,
                on_excursion_filter_type_toggle: None,
                on_excursion_margin_octaves_change: None,
                on_use_schroeder_split_change: None,
                on_schroeder_freq_change: None,
                on_schroeder_low_max_q_change: None,
                on_schroeder_low_allow_boost_change: None,
                on_schroeder_high_max_q_change: None,
                on_schroeder_high_shelving_only_change: None,
                on_use_phase_alignment_change: None,
                on_phase_min_freq_change: None,
                on_phase_max_freq_change: None,
                on_phase_optimize_polarity_change: None,
                on_phase_max_delay_ms_change: None,
                on_use_multi_seat_change: None,
                on_multi_seat_strategy_change: None,
                on_multi_seat_strategy_toggle: None,
                on_multi_seat_primary_seat_change: None,
                on_multi_seat_max_deviation_db_change: None,
            },
            v2: V2Callbacks {
                on_allow_delay_change: None,
                on_seed_enabled_change: None,
                on_seed_change: None,
                on_vog_enabled_change: None,
                on_vog_reference_channel_change: None,
                on_vog_reference_channel_toggle: None,
                on_broadband_target_matching_change: None,
                on_mixed_crossover_freq_change: None,
                on_mixed_crossover_type_change: None,
                on_mixed_crossover_type_toggle: None,
                on_mixed_fir_band_change: None,
                on_mixed_fir_band_toggle: None,
            },
            multi_measurement: MultiMeasurementCallbacks {
                on_use_multi_measurement_change: None,
                on_multi_measurement_strategy_change: None,
                on_multi_measurement_strategy_toggle: None,
                on_multi_measurement_variance_lambda_change: None,
                on_multi_measurement_weight_change: None,
            },
            lifecycle: FormLifecycleCallbacks {
                on_block_focus: None,
                on_detail_level_change: None,
                on_preset_change: None,
                on_preset_toggle: None,
                on_target_distance_change: None,
                on_optimization_goal_change: None,
            },
        }
    }

    /// Set the configuration values
    pub fn config(mut self, config: AutoEqConfig) -> Self {
        self.meta.config = config;
        self
    }

    /// Set UI state
    pub fn ui_state(mut self, ui_state: AutoEqFormUiState) -> Self {
        self.meta.ui_state = ui_state;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.meta.disabled = disabled;
        self
    }

    /// Show/hide Goals section
    pub fn show_goals(mut self, show: bool) -> Self {
        self.visibility.show_goals = show;
        self
    }

    /// Show/hide EQ Design section
    pub fn show_eq_design(mut self, show: bool) -> Self {
        self.visibility.show_eq_design = show;
        self
    }

    /// Show/hide Optimization Tuning section
    pub fn show_optimization_tuning(mut self, show: bool) -> Self {
        self.visibility.show_optimization_tuning = show;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: AutoEqFormTheme) -> Self {
        self.meta.theme = Some(theme);
        self
    }

    /// Set allowed optimization modes (e.g., vec!["iir".to_string(), "fir".to_string()])
    pub fn allowed_opt_modes(mut self, modes: Vec<String>) -> Self {
        self.meta.allowed_opt_modes = Some(modes);
        self
    }

    /// Set the optimization type (Speaker or Headphone)
    pub fn optimization_type(mut self, opt_type: OptimizationType) -> Self {
        self.meta.optimization_type = opt_type;
        self
    }

    /// Set available spinorama curves for speaker mode
    pub fn available_spinorama_curves(mut self, curves: Vec<String>) -> Self {
        self.meta.available_spinorama_curves = curves;
        self
    }

    /// Hide DE-specific parameters (strategy, mutation F, crossover CR)
    pub fn hide_de_params(mut self, hide: bool) -> Self {
        self.visibility.hide_de_params = hide;
        self
    }

    /// Hide smoothing toggle and window size
    pub fn hide_smoothing(mut self, hide: bool) -> Self {
        self.visibility.hide_smoothing = hide;
        self
    }

    /// Hide spacing weight and min spacing octaves
    pub fn hide_spacing(mut self, hide: bool) -> Self {
        self.visibility.hide_spacing = hide;
        self
    }

    /// Hide tolerance and absolute tolerance
    pub fn hide_tolerance(mut self, hide: bool) -> Self {
        self.visibility.hide_tolerance = hide;
        self
    }

    /// Hide sample rate input
    pub fn hide_sample_rate(mut self, hide: bool) -> Self {
        self.visibility.hide_sample_rate = hide;
        self
    }

    /// Hide phase alignment in Advanced System Optimization section
    pub fn hide_phase_alignment(mut self, hide: bool) -> Self {
        self.visibility.hide_phase_alignment = hide;
        self
    }

    /// Hide multi-seat in Advanced System Optimization section
    pub fn hide_multi_seat(mut self, hide: bool) -> Self {
        self.visibility.hide_multi_seat = hide;
        self
    }

    /// Hide the "Scenario A" subtitle text
    pub fn hide_scenario_a_text(mut self, hide: bool) -> Self {
        self.visibility.hide_scenario_a_text = hide;
        self
    }

    /// Hide room-specific sections (Advanced Room Correction, System Optimization, Advanced Tuning)
    pub fn hide_room_sections(mut self, hide: bool) -> Self {
        self.visibility.hide_room_sections = hide;
        self
    }

    /// Set available width for responsive layout
    pub fn available_width(mut self, width: f32) -> Self {
        self.meta.available_width = width;
        self
    }

    /// Set the first-party UI language used by all form sections.
    pub fn language(mut self, language: Language) -> Self {
        self.meta.language = language;
        self
    }

    /// Set the layout mode (Default or RoomEq)
    pub fn layout_mode(mut self, mode: AutoEqLayoutMode) -> Self {
        self.meta.layout_mode = mode;
        self
    }
    // EQ Design callbacks

    /// Set optim mode change handler
    pub fn on_opt_mode_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_opt_mode_change = Some(Box::new(handler));
        self
    }

    /// Set optim mode dropdown toggle handler
    pub fn on_opt_mode_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_opt_mode_toggle = Some(Box::new(handler));
        self
    }

    /// Set FIR taps change handler
    pub fn on_fir_taps_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_fir_taps_change = Some(Box::new(handler));
        self
    }

    /// Set FIR phase change handler
    pub fn on_fir_phase_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_fir_phase_change = Some(Box::new(handler));
        self
    }

    /// Set FIR phase dropdown toggle handler
    pub fn on_fir_phase_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_fir_phase_toggle = Some(Box::new(handler));
        self
    }

    /// Set number of filters change handler
    pub fn on_num_filters_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_num_filters_change = Some(Box::new(handler));
        self
    }

    /// Set sample rate change handler
    pub fn on_sample_rate_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_sample_rate_change = Some(Box::new(handler));
        self
    }

    /// Set min dB change handler
    pub fn on_min_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_min_db_change = Some(Box::new(handler));
        self
    }

    /// Set max dB change handler
    pub fn on_max_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_max_db_change = Some(Box::new(handler));
        self
    }

    /// Set min Q change handler
    pub fn on_min_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_min_q_change = Some(Box::new(handler));
        self
    }

    /// Set max Q change handler
    pub fn on_max_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_max_q_change = Some(Box::new(handler));
        self
    }

    /// Set min frequency change handler
    pub fn on_min_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_min_freq_change = Some(Box::new(handler));
        self
    }

    /// Set max frequency change handler
    pub fn on_max_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_max_freq_change = Some(Box::new(handler));
        self
    }

    /// Set PEQ model change handler
    pub fn on_peq_model_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_peq_model_change = Some(Box::new(handler));
        self
    }

    /// Set PEQ model dropdown toggle handler
    pub fn on_peq_model_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_peq_model_toggle = Some(Box::new(handler));
        self
    }

    /// Set spacing weight change handler
    pub fn on_spacing_weight_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_spacing_weight_change = Some(Box::new(handler));
        self
    }

    /// Set min spacing octaves change handler
    pub fn on_min_spacing_oct_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.eq_design.on_min_spacing_oct_change = Some(Box::new(handler));
        self
    }

    // Optimization callbacks

    /// Set algorithm change handler
    pub fn on_algo_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_algo_change = Some(Box::new(handler));
        self
    }

    /// Set algorithm dropdown toggle handler
    pub fn on_algo_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_algo_toggle = Some(Box::new(handler));
        self
    }

    /// Set population change handler
    pub fn on_population_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_population_change = Some(Box::new(handler));
        self
    }

    /// Set maxeval change handler
    pub fn on_maxeval_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_maxeval_change = Some(Box::new(handler));
        self
    }

    /// Set relative tolerance change handler
    pub fn on_tolerance_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_tolerance_change = Some(Box::new(handler));
        self
    }

    /// Set absolute tolerance change handler
    pub fn on_atolerance_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_atolerance_change = Some(Box::new(handler));
        self
    }

    /// Set BO initial sample count change handler
    pub fn on_bo_initial_samples_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_initial_samples_change = Some(Box::new(handler));
        self
    }

    /// Set BO batch size change handler
    pub fn on_bo_batch_size_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_batch_size_change = Some(Box::new(handler));
        self
    }

    /// Set BO posterior std handoff threshold change handler
    pub fn on_bo_posterior_std_threshold_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_posterior_std_threshold_change = Some(Box::new(handler));
        self
    }

    /// Set BO acquisition change handler
    pub fn on_bo_acquisition_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_acquisition_change = Some(Box::new(handler));
        self
    }

    /// Set BO acquisition dropdown toggle handler
    pub fn on_bo_acquisition_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_acquisition_toggle = Some(Box::new(handler));
        self
    }

    /// Set BO qEHVI toggle handler
    pub fn on_bo_ehvi_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_bo_ehvi_change = Some(Box::new(handler));
        self
    }

    /// Set DE mutation factor (F) change handler
    pub fn on_de_f_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_de_f_change = Some(Box::new(handler));
        self
    }

    /// Set DE crossover rate (CR) change handler
    pub fn on_de_cr_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_de_cr_change = Some(Box::new(handler));
        self
    }

    /// Set DE strategy change handler
    pub fn on_strategy_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_strategy_change = Some(Box::new(handler));
        self
    }

    /// Set DE strategy dropdown toggle handler
    pub fn on_strategy_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_strategy_toggle = Some(Box::new(handler));
        self
    }

    /// Set adaptive weight F change handler (DE adaptive strategies only)
    pub fn on_adaptive_weight_f_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_adaptive_weight_f_change = Some(Box::new(handler));
        self
    }

    /// Set adaptive weight CR change handler (DE adaptive strategies only)
    pub fn on_adaptive_weight_cr_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_adaptive_weight_cr_change = Some(Box::new(handler));
        self
    }

    /// Set local refinement toggle handler
    pub fn on_refine_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_refine_change = Some(Box::new(handler));
        self
    }

    /// Set local algorithm change handler
    pub fn on_local_algo_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_local_algo_change = Some(Box::new(handler));
        self
    }

    /// Set local algorithm dropdown toggle handler
    pub fn on_local_algo_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_local_algo_toggle = Some(Box::new(handler));
        self
    }

    /// Set smoothing toggle handler
    pub fn on_smooth_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_smooth_change = Some(Box::new(handler));
        self
    }

    /// Set smoothing window size change handler
    pub fn on_smooth_n_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_smooth_n_change = Some(Box::new(handler));
        self
    }

    /// Set psychoacoustic toggle handler
    pub fn on_psychoacoustic_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_psychoacoustic_change = Some(Box::new(handler));
        self
    }

    /// Set asymmetric loss toggle handler
    pub fn on_asymmetric_loss_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.optimization.on_asymmetric_loss_change = Some(Box::new(handler));
        self
    }

    // Goals callbacks

    /// Set loss type change handler
    pub fn on_loss_type_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_loss_type_change = Some(Box::new(handler));
        self
    }

    /// Set loss type dropdown toggle handler
    pub fn on_loss_type_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_loss_type_toggle = Some(Box::new(handler));
        self
    }

    /// Set target curve change handler
    pub fn on_target_curve_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_target_curve_change = Some(Box::new(handler));
        self
    }

    /// Set target curve dropdown toggle handler
    pub fn on_target_curve_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_target_curve_toggle = Some(Box::new(handler));
        self
    }

    /// Set edit custom target curve handler (opens the custom target modal)
    pub fn on_edit_custom_target(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_edit_custom_target = Some(Box::new(handler));
        self
    }

    /// Set system type change handler
    pub fn on_system_type_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_system_type_change = Some(Box::new(handler));
        self
    }

    /// Set system type dropdown toggle handler
    pub fn on_system_type_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.goals.on_system_type_toggle = Some(Box::new(handler));
        self
    }

    // Advanced Room Correction (Scenario B) callbacks

    pub fn on_use_target_tilt_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_use_target_tilt_change = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_type_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_type_change = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_type_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_type_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_slope_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_slope_change = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_reference_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_reference_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_bass_shelf_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_bass_shelf_db_change = Some(Box::new(handler));
        self
    }

    pub fn on_tilt_bass_shelf_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_tilt_bass_shelf_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_use_excursion_protection_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_use_excursion_protection_change = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_auto_detect_f3_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_auto_detect_f3_change = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_manual_f3_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_manual_f3_change = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_filter_order_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_filter_order_change = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_filter_type_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_filter_type_change = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_filter_type_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_filter_type_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_excursion_margin_octaves_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_excursion_margin_octaves_change = Some(Box::new(handler));
        self
    }

    pub fn on_use_schroeder_split_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_use_schroeder_split_change = Some(Box::new(handler));
        self
    }

    pub fn on_schroeder_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_schroeder_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_schroeder_low_max_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_schroeder_low_max_q_change = Some(Box::new(handler));
        self
    }

    pub fn on_schroeder_low_allow_boost_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_schroeder_low_allow_boost_change = Some(Box::new(handler));
        self
    }

    pub fn on_schroeder_high_max_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_schroeder_high_max_q_change = Some(Box::new(handler));
        self
    }

    pub fn on_schroeder_high_shelving_only_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_schroeder_high_shelving_only_change = Some(Box::new(handler));
        self
    }

    // Advanced System Optimization (Scenario A) callbacks

    pub fn on_use_phase_alignment_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_use_phase_alignment_change = Some(Box::new(handler));
        self
    }

    pub fn on_phase_min_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_phase_min_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_phase_max_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_phase_max_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_phase_optimize_polarity_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_phase_optimize_polarity_change = Some(Box::new(handler));
        self
    }

    pub fn on_phase_max_delay_ms_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_phase_max_delay_ms_change = Some(Box::new(handler));
        self
    }

    pub fn on_use_multi_seat_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_use_multi_seat_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_seat_strategy_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_multi_seat_strategy_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_seat_strategy_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_multi_seat_strategy_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_multi_seat_primary_seat_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_multi_seat_primary_seat_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_seat_max_deviation_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.room_correction.on_multi_seat_max_deviation_db_change = Some(Box::new(handler));
        self
    }

    // v2 callbacks

    pub fn on_allow_delay_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_allow_delay_change = Some(Box::new(handler));
        self
    }

    pub fn on_seed_enabled_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_seed_enabled_change = Some(Box::new(handler));
        self
    }

    pub fn on_seed_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_seed_change = Some(Box::new(handler));
        self
    }

    pub fn on_vog_enabled_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_vog_enabled_change = Some(Box::new(handler));
        self
    }

    pub fn on_vog_reference_channel_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_vog_reference_channel_change = Some(Box::new(handler));
        self
    }

    pub fn on_vog_reference_channel_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_vog_reference_channel_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_broadband_target_matching_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_broadband_target_matching_change = Some(Box::new(handler));
        self
    }

    pub fn on_mixed_crossover_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_mixed_crossover_freq_change = Some(Box::new(handler));
        self
    }

    pub fn on_mixed_crossover_type_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_mixed_crossover_type_change = Some(Box::new(handler));
        self
    }

    pub fn on_mixed_crossover_type_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_mixed_crossover_type_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_mixed_fir_band_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_mixed_fir_band_change = Some(Box::new(handler));
        self
    }

    pub fn on_mixed_fir_band_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.v2.on_mixed_fir_band_toggle = Some(Box::new(handler));
        self
    }

    // Multi-measurement callbacks

    pub fn hide_multi_measurement(mut self, hide: bool) -> Self {
        self.visibility.hide_multi_measurement = hide;
        self
    }

    pub fn hide_capability_section(mut self, hide: bool) -> Self {
        self.visibility.hide_capability_section = hide;
        self
    }

    pub fn hide_target_distance_section(mut self, hide: bool) -> Self {
        self.visibility.hide_target_distance_section = hide;
        self
    }

    pub fn hide_optimization_goal_section(mut self, hide: bool) -> Self {
        self.visibility.hide_optimization_goal_section = hide;
        self
    }

    pub fn hide_bass_management(mut self, hide: bool) -> Self {
        self.visibility.hide_bass_management = hide;
        self
    }

    pub fn hide_asymmetric_loss(mut self, hide: bool) -> Self {
        self.visibility.hide_asymmetric_loss = hide;
        self
    }

    pub fn hide_broadband_matching(mut self, hide: bool) -> Self {
        self.visibility.hide_broadband_matching = hide;
        self
    }

    pub fn loss_type_options(mut self, options: &'static [(&'static str, &'static str)]) -> Self {
        self.visibility.loss_type_options_override = Some(options);
        self
    }

    pub fn on_use_multi_measurement_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.multi_measurement.on_use_multi_measurement_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_measurement_strategy_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.multi_measurement.on_multi_measurement_strategy_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_measurement_strategy_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.multi_measurement.on_multi_measurement_strategy_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_multi_measurement_variance_lambda_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.multi_measurement
            .on_multi_measurement_variance_lambda_change = Some(Box::new(handler));
        self
    }

    pub fn on_multi_measurement_weight_change(
        mut self,
        handler: impl Fn(usize, f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.multi_measurement.on_multi_measurement_weight_change = Some(Box::new(handler));
        self
    }

    /// Set a callback fired when the user hovers over a parameter block.
    /// The callback receives the block id (e.g., `"goals"`, `"eq-design"`).
    pub fn on_block_focus(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_block_focus = Some(Box::new(handler));
        self
    }

    /// Set detail level change handler (called with "simple", "intermediate", or "expert")
    pub fn on_detail_level_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_detail_level_change = Some(Box::new(handler));
        self
    }

    /// Set preset change handler (called with preset id like "balanced", "custom")
    pub fn on_preset_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_preset_change = Some(Box::new(handler));
        self
    }

    /// Set preset dropdown toggle handler
    pub fn on_preset_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_preset_toggle = Some(Box::new(handler));
        self
    }

    /// Set handler for target distance preset changes (near/mid/far/custom)
    pub fn on_target_distance_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_target_distance_change = Some(Box::new(handler));
        self
    }

    /// Set handler for optimization goal preset changes (match_target/natural/psychoacoustic)
    pub fn on_optimization_goal_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.lifecycle.on_optimization_goal_change = Some(Box::new(handler));
        self
    }
}
