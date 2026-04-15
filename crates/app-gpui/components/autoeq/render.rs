//! RenderOnce implementation for the AutoEQ form.
//!
//! Renders a two-panel layout: form parameters on the left, contextual
//! documentation on the right. The docs panel collapses on narrow screens.

use gpui::prelude::*;
use gpui::*;

use gpui_ui_kit::button::{Button, ButtonSize, ButtonVariant};
use gpui_ui_kit::card::Card;
use gpui_ui_kit::number_input::{NumberInput, NumberInputSize};
use gpui_ui_kit::select::SelectSize;
use gpui_ui_kit::select::{Select, SelectOption};
use gpui_ui_kit::stack::{HStack, StackAlign, StackJustify, StackSpacing, VStack};
use gpui_ui_kit::text::{Text, TextSize, TextWeight};
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::toggle::{Toggle, ToggleSize};

use super::config::ParamLimits;
use super::constants::*;
use super::docs;
use super::form::{
    AutoEqForm, AutoEqLayoutMode, is_narrow_default_layout, is_narrow_room_eq_layout,
};
use super::theme::AutoEqFormTheme;
use super::ui_state::DetailLevel;

#[allow(clippy::too_many_lines)]
impl RenderOnce for AutoEqForm {
    #[allow(clippy::cognitive_complexity)]
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| AutoEqFormTheme::from(&global_theme));

        let id = self.id;
        let config = self.config;
        let ui_state = self.ui_state;
        let disabled = self.disabled;
        let show_goals = self.show_goals;
        let _show_eq_design = self.show_eq_design;
        let show_optimization_tuning = self.show_optimization_tuning;
        let optimization_type = self.optimization_type;
        let available_spinorama_curves = self.available_spinorama_curves;
        let allowed_opt_modes = self.allowed_opt_modes;
        let hide_de_params = self.hide_de_params;
        let hide_smoothing = self.hide_smoothing;
        let hide_spacing = self.hide_spacing;
        let hide_tolerance = self.hide_tolerance;
        let hide_sample_rate = self.hide_sample_rate;
        let hide_phase_alignment = self.hide_phase_alignment;
        let hide_multi_seat = self.hide_multi_seat;
        let _hide_scenario_a_text = self.hide_scenario_a_text;
        let hide_room_sections = self.hide_room_sections;
        let hide_multi_measurement = self.hide_multi_measurement;
        let hide_capability_section = self.hide_capability_section;
        let hide_target_distance_section = self.hide_target_distance_section;
        let hide_optimization_goal_section = self.hide_optimization_goal_section;
        let hide_bass_management = self.hide_bass_management;
        let hide_asymmetric_loss = self.hide_asymmetric_loss;
        let hide_broadband_matching = self.hide_broadband_matching;
        let loss_type_options_override = self.loss_type_options_override;
        let available_width = self.available_width;
        let layout_mode = self.layout_mode;
        // Wrap callbacks in Rc for sharing
        let on_opt_mode_change_rc = self.on_opt_mode_change.map(std::rc::Rc::new);
        let _on_opt_mode_toggle_rc = self.on_opt_mode_toggle.map(std::rc::Rc::new);
        let on_fir_taps_change_rc = self.on_fir_taps_change.map(std::rc::Rc::new);
        let on_fir_phase_change_rc = self.on_fir_phase_change.map(std::rc::Rc::new);
        let on_fir_phase_toggle_rc = self.on_fir_phase_toggle.map(std::rc::Rc::new);
        let on_num_filters_change_rc = self.on_num_filters_change.map(std::rc::Rc::new);
        let on_sample_rate_change_rc = self.on_sample_rate_change.map(std::rc::Rc::new);
        let on_min_db_change_rc = self.on_min_db_change.map(std::rc::Rc::new);
        let on_max_db_change_rc = self.on_max_db_change.map(std::rc::Rc::new);
        let on_min_q_change_rc = self.on_min_q_change.map(std::rc::Rc::new);
        let on_max_q_change_rc = self.on_max_q_change.map(std::rc::Rc::new);
        let on_min_freq_change_rc = self.on_min_freq_change.map(std::rc::Rc::new);
        let on_max_freq_change_rc = self.on_max_freq_change.map(std::rc::Rc::new);
        let on_peq_model_change_rc = self.on_peq_model_change.map(std::rc::Rc::new);
        let on_peq_model_toggle_rc = self.on_peq_model_toggle.map(std::rc::Rc::new);
        let on_spacing_weight_change_rc = self.on_spacing_weight_change.map(std::rc::Rc::new);
        let on_min_spacing_oct_change_rc = self.on_min_spacing_oct_change.map(std::rc::Rc::new);
        let on_algo_change_rc = self.on_algo_change.map(std::rc::Rc::new);
        let on_algo_toggle_rc = self.on_algo_toggle.map(std::rc::Rc::new);
        let on_population_change_rc = self.on_population_change.map(std::rc::Rc::new);
        let on_maxeval_change_rc = self.on_maxeval_change.map(std::rc::Rc::new);
        let on_tolerance_change_rc = self.on_tolerance_change.map(std::rc::Rc::new);
        let on_atolerance_change_rc = self.on_atolerance_change.map(std::rc::Rc::new);
        let on_de_f_change_rc = self.on_de_f_change.map(std::rc::Rc::new);
        let on_de_cr_change_rc = self.on_de_cr_change.map(std::rc::Rc::new);
        let on_strategy_change_rc = self.on_strategy_change.map(std::rc::Rc::new);
        let on_strategy_toggle_rc = self.on_strategy_toggle.map(std::rc::Rc::new);
        let on_adaptive_weight_f_change_rc = self.on_adaptive_weight_f_change.map(std::rc::Rc::new);
        let on_adaptive_weight_cr_change_rc =
            self.on_adaptive_weight_cr_change.map(std::rc::Rc::new);
        let on_refine_change_rc = self.on_refine_change.map(std::rc::Rc::new);
        let on_local_algo_change_rc = self.on_local_algo_change.map(std::rc::Rc::new);
        let on_local_algo_toggle_rc = self.on_local_algo_toggle.map(std::rc::Rc::new);
        let on_smooth_change_rc = self.on_smooth_change.map(std::rc::Rc::new);
        let on_smooth_n_change_rc = self.on_smooth_n_change.map(std::rc::Rc::new);
        let on_psychoacoustic_change_rc = self.on_psychoacoustic_change.map(std::rc::Rc::new);
        let on_asymmetric_loss_change_rc = self.on_asymmetric_loss_change.map(std::rc::Rc::new);
        let on_loss_type_change_rc = self.on_loss_type_change.map(std::rc::Rc::new);
        let on_loss_type_toggle_rc = self.on_loss_type_toggle.map(std::rc::Rc::new);
        let on_target_curve_change_rc = self.on_target_curve_change.map(std::rc::Rc::new);
        let on_target_curve_toggle_rc = self.on_target_curve_toggle.map(std::rc::Rc::new);
        let on_edit_custom_target_rc = self.on_edit_custom_target.map(std::rc::Rc::new);
        let on_system_type_change_rc = self.on_system_type_change.map(std::rc::Rc::new);
        let on_system_type_toggle_rc = self.on_system_type_toggle.map(std::rc::Rc::new);

        // Advanced callbacks Rc
        let on_use_target_tilt_change_rc = self.on_use_target_tilt_change.map(std::rc::Rc::new);
        let on_tilt_type_change_rc = self.on_tilt_type_change.map(std::rc::Rc::new);
        let on_tilt_type_toggle_rc = self.on_tilt_type_toggle.map(std::rc::Rc::new);
        let on_tilt_slope_change_rc = self.on_tilt_slope_change.map(std::rc::Rc::new);
        let on_tilt_reference_freq_change_rc =
            self.on_tilt_reference_freq_change.map(std::rc::Rc::new);
        let on_tilt_bass_shelf_db_change_rc =
            self.on_tilt_bass_shelf_db_change.map(std::rc::Rc::new);
        let on_tilt_bass_shelf_freq_change_rc =
            self.on_tilt_bass_shelf_freq_change.map(std::rc::Rc::new);
        let on_use_excursion_protection_change_rc = self
            .on_use_excursion_protection_change
            .map(std::rc::Rc::new);
        let on_excursion_auto_detect_f3_change_rc = self
            .on_excursion_auto_detect_f3_change
            .map(std::rc::Rc::new);
        let on_excursion_manual_f3_change_rc =
            self.on_excursion_manual_f3_change.map(std::rc::Rc::new);
        let on_excursion_filter_order_change_rc =
            self.on_excursion_filter_order_change.map(std::rc::Rc::new);
        let on_excursion_filter_type_change_rc =
            self.on_excursion_filter_type_change.map(std::rc::Rc::new);
        let on_excursion_filter_type_toggle_rc =
            self.on_excursion_filter_type_toggle.map(std::rc::Rc::new);
        let on_excursion_margin_octaves_change_rc = self
            .on_excursion_margin_octaves_change
            .map(std::rc::Rc::new);
        let on_use_schroeder_split_change_rc =
            self.on_use_schroeder_split_change.map(std::rc::Rc::new);
        let on_schroeder_freq_change_rc = self.on_schroeder_freq_change.map(std::rc::Rc::new);
        let on_schroeder_low_max_q_change_rc =
            self.on_schroeder_low_max_q_change.map(std::rc::Rc::new);
        let on_schroeder_low_allow_boost_change_rc = self
            .on_schroeder_low_allow_boost_change
            .map(std::rc::Rc::new);
        let on_schroeder_high_max_q_change_rc =
            self.on_schroeder_high_max_q_change.map(std::rc::Rc::new);
        let on_schroeder_high_shelving_only_change_rc = self
            .on_schroeder_high_shelving_only_change
            .map(std::rc::Rc::new);
        let on_use_phase_alignment_change_rc =
            self.on_use_phase_alignment_change.map(std::rc::Rc::new);
        let on_phase_min_freq_change_rc = self.on_phase_min_freq_change.map(std::rc::Rc::new);
        let on_phase_max_freq_change_rc = self.on_phase_max_freq_change.map(std::rc::Rc::new);
        let on_phase_optimize_polarity_change_rc =
            self.on_phase_optimize_polarity_change.map(std::rc::Rc::new);
        let on_phase_max_delay_ms_change_rc =
            self.on_phase_max_delay_ms_change.map(std::rc::Rc::new);
        let on_use_multi_seat_change_rc = self.on_use_multi_seat_change.map(std::rc::Rc::new);
        let on_multi_seat_strategy_change_rc =
            self.on_multi_seat_strategy_change.map(std::rc::Rc::new);
        let on_multi_seat_strategy_toggle_rc =
            self.on_multi_seat_strategy_toggle.map(std::rc::Rc::new);
        let on_multi_seat_primary_seat_change_rc =
            self.on_multi_seat_primary_seat_change.map(std::rc::Rc::new);
        let on_multi_seat_max_deviation_db_change_rc = self
            .on_multi_seat_max_deviation_db_change
            .map(std::rc::Rc::new);

        // v2 callbacks Rc
        let on_allow_delay_change_rc = self.on_allow_delay_change.map(std::rc::Rc::new);
        let on_seed_enabled_change_rc = self.on_seed_enabled_change.map(std::rc::Rc::new);
        let on_seed_change_rc = self.on_seed_change.map(std::rc::Rc::new);
        let on_gd_opt_enabled_change_rc = self.on_gd_opt_enabled_change.map(std::rc::Rc::new);
        let on_gd_opt_target_ms_change_rc = self.on_gd_opt_target_ms_change.map(std::rc::Rc::new);
        let on_vog_enabled_change_rc = self.on_vog_enabled_change.map(std::rc::Rc::new);
        let on_vog_reference_channel_change_rc =
            self.on_vog_reference_channel_change.map(std::rc::Rc::new);
        let on_vog_reference_channel_toggle_rc =
            self.on_vog_reference_channel_toggle.map(std::rc::Rc::new);
        let on_broadband_target_matching_change_rc = self
            .on_broadband_target_matching_change
            .map(std::rc::Rc::new);
        let on_mixed_crossover_freq_change_rc =
            self.on_mixed_crossover_freq_change.map(std::rc::Rc::new);
        let on_mixed_crossover_type_change_rc =
            self.on_mixed_crossover_type_change.map(std::rc::Rc::new);
        let on_mixed_crossover_type_toggle_rc =
            self.on_mixed_crossover_type_toggle.map(std::rc::Rc::new);
        let on_mixed_fir_band_change_rc = self.on_mixed_fir_band_change.map(std::rc::Rc::new);
        let on_mixed_fir_band_toggle_rc = self.on_mixed_fir_band_toggle.map(std::rc::Rc::new);

        // Multi-measurement callbacks Rc
        let on_use_multi_measurement_change_rc =
            self.on_use_multi_measurement_change.map(std::rc::Rc::new);
        let on_multi_measurement_strategy_change_rc = self
            .on_multi_measurement_strategy_change
            .map(std::rc::Rc::new);
        let on_multi_measurement_strategy_toggle_rc = self
            .on_multi_measurement_strategy_toggle
            .map(std::rc::Rc::new);
        let on_multi_measurement_variance_lambda_change_rc = self
            .on_multi_measurement_variance_lambda_change
            .map(std::rc::Rc::new);
        let on_multi_measurement_weight_change_rc = self
            .on_multi_measurement_weight_change
            .map(std::rc::Rc::new);

        let on_block_focus_rc = self.on_block_focus.map(std::rc::Rc::new);
        let on_detail_level_change_rc = self.on_detail_level_change.map(std::rc::Rc::new);
        let on_preset_change_rc = self.on_preset_change.map(std::rc::Rc::new);
        let on_preset_toggle_rc = self.on_preset_toggle.map(std::rc::Rc::new);

        // Complex mode section callbacks Rc
        let on_target_distance_change_rc = self.on_target_distance_change.map(std::rc::Rc::new);
        let on_optimization_goal_change_rc =
            self.on_optimization_goal_change.map(std::rc::Rc::new);

        // Build the form body - branch on detail level
        let detail_level = ui_state.detail_level;
        let form_body = match detail_level {
            // Simple and Intermediate: simplified render with preset selector
            DetailLevel::Simple | DetailLevel::Intermediate => {
                include!("render_body_simple.rs")
            }
            // Expert: full parameter form
            DetailLevel::Expert => match layout_mode {
                AutoEqLayoutMode::Default => {
                    include!("render_body.rs")
                }
                AutoEqLayoutMode::RoomEq => {
                    include!("render_body_room_eq.rs")
                }
            },
        };

        // Full-width layout (no docs panel)
        div().w_full().h_full().child(form_body)
    }
}
