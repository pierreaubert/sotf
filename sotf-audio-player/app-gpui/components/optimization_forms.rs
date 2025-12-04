//! Reusable optimization parameter form components
//!
//! These components render the parameter forms for EQ optimization that are
//! shared across headphone, room EQ, and speaker optimization.

use gpui::*;

use crate::optimization_params::*;
use crate::theme::Theme;
use crate::ui::PlayerView;

impl PlayerView {
    /// Render EQ Design parameters section
    ///
    /// Displays: num_filters, sample_rate, dB ranges, Q ranges, frequency ranges,
    /// PEQ model, spacing parameters
    pub fn render_eq_design_params(
        &self,
        params: &OptimizationParams,
        prefix: &str, // "headphone", "roomeq", or "speaker"
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let params_clone = params.clone();
        let theme = theme.clone();
        let prefix_str = prefix.to_string();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .bg(theme.surface)
            .rounded_lg()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("EQ Design Parameters"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("Configure filter characteristics and frequency ranges"),
            )
    }

    /// Render Optimization Fine Tuning parameters section
    ///
    /// Displays: algorithm, population, maxeval, DE strategy, tolerances,
    /// refinement, smoothing
    pub fn render_optimization_tuning_params(
        &self,
        params: &OptimizationParams,
        prefix: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let params_clone = params.clone();
        let theme = theme.clone();
        let prefix_str = prefix.to_string();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .bg(theme.surface)
            .rounded_lg()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Optimization Fine Tuning"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("Advanced optimization algorithm settings"),
            )
    }

    /// Update a numeric optimization parameter
    pub fn update_optimization_param(
        &mut self,
        prefix: &str,
        param_name: &str,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let params = match prefix {
                "headphone" => &mut state.app.headphone_params,
                _ => return,
            };

            match param_name {
                "num_filters" => params.num_filters = value as usize,
                "sample_rate" => params.sample_rate = value as u32,
                "min_db" => params.min_db = value,
                "max_db" => params.max_db = value,
                "min_q" => params.min_q = value,
                "max_q" => params.max_q = value,
                "min_freq" => params.min_freq = value,
                "max_freq" => params.max_freq = value,
                "min_spacing_oct" => params.min_spacing_oct = value,
                "spacing_weight" => params.spacing_weight = value,
                "population" => params.population = value as usize,
                "maxeval" => params.maxeval = value as usize,
                "de_f" => params.de_f = value,
                "de_cr" => params.de_cr = value,
                "smooth_n" => params.smooth_n = value as usize,
                _ => {}
            }
        });
        cx.notify();
    }

    /// Update a string optimization parameter
    pub fn update_optimization_param_string(
        &mut self,
        prefix: &str,
        param_name: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let params = match prefix {
                "headphone" => &mut state.app.headphone_params,
                _ => return,
            };

            match param_name {
                "peq_model" => params.peq_model = value.to_string(),
                "algo" => params.algo = value.to_string(),
                "strategy" => params.strategy = value.to_string(),
                "local_algo" => params.local_algo = value.to_string(),
                _ => {}
            }
        });
        cx.notify();
    }
}
