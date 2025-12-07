//! Reusable optimization parameter form components
//!
//! These components render the parameter forms for EQ optimization that are
//! shared across headphone, room EQ, and speaker optimization.
//!
//! Uses a compact layout with chip-style buttons for selections and
//! stepper controls for numeric values.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    Toggle, ToggleSize, ToggleTheme, VStack,
};

use crate::optimization_params::*;
use crate::theme::Theme;
use crate::ui::PlayerView;

impl PlayerView {
    /// Render EQ Design parameters section
    pub fn render_eq_design_params(
        &self,
        params: &OptimizationParams,
        prefix: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(
                            Text::new("EQ Design Parameters")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold),
                        )
                        .child(
                            Text::new("Configure filter characteristics and frequency ranges")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Number of Filters + Sample Rate
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(self.render_stepper_row(
                            "Filters",
                            params.num_filters as f64,
                            ParamLimits::NUM_FILTERS,
                            prefix,
                            "num_filters",
                            &theme,
                            cx,
                        ))
                        .child(self.render_stepper_row(
                            "Sample Rate",
                            params.sample_rate as f64,
                            ParamLimits::SAMPLE_RATE,
                            prefix,
                            "sample_rate",
                            &theme,
                            cx,
                        )),
                )
                // dB Range
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(self.render_stepper_row(
                            "Min dB",
                            params.min_db,
                            ParamLimits::DB,
                            prefix,
                            "min_db",
                            &theme,
                            cx,
                        ))
                        .child(self.render_stepper_row(
                            "Max dB",
                            params.max_db,
                            ParamLimits::DB,
                            prefix,
                            "max_db",
                            &theme,
                            cx,
                        )),
                )
                // Q Range
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(self.render_stepper_row(
                            "Min Q",
                            params.min_q,
                            ParamLimits::Q,
                            prefix,
                            "min_q",
                            &theme,
                            cx,
                        ))
                        .child(self.render_stepper_row(
                            "Max Q",
                            params.max_q,
                            ParamLimits::Q,
                            prefix,
                            "max_q",
                            &theme,
                            cx,
                        )),
                )
                // Frequency Range
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(self.render_stepper_row(
                            "Min Freq",
                            params.min_freq,
                            ParamLimits::FREQUENCY,
                            prefix,
                            "min_freq",
                            &theme,
                            cx,
                        ))
                        .child(self.render_stepper_row(
                            "Max Freq",
                            params.max_freq,
                            ParamLimits::FREQUENCY,
                            prefix,
                            "max_freq",
                            &theme,
                            cx,
                        )),
                )
                // PEQ Model
                .child(self.render_chip_select_row(
                    "PEQ Model",
                    &params.peq_model,
                    PEQ_MODEL_OPTIONS,
                    prefix,
                    "peq_model",
                    &theme,
                    cx,
                )),
        )
    }

    /// Render Optimization Fine Tuning parameters section
    pub fn render_optimization_tuning_params(
        &self,
        params: &OptimizationParams,
        prefix: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(
                            Text::new("Optimization Fine Tuning")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold),
                        )
                        .child(
                            Text::new("Advanced optimization algorithm settings")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
                // Algorithm selection
                .child(self.render_chip_select_row(
                    "Algorithm",
                    &params.algo,
                    ALGORITHM_OPTIONS,
                    prefix,
                    "algo",
                    &theme,
                    cx,
                ))
                // Population and MaxEval
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(self.render_stepper_row(
                            "Population",
                            params.population as f64,
                            ParamLimits::POPULATION,
                            prefix,
                            "population",
                            &theme,
                            cx,
                        ))
                        .child(self.render_stepper_row(
                            "Max Evals",
                            params.maxeval as f64,
                            ParamLimits::MAXEVAL,
                            prefix,
                            "maxeval",
                            &theme,
                            cx,
                        )),
                )
                // DE Strategy (only for DE algorithms)
                .when(params.algo.contains("de"), |d| {
                    d.child(self.render_chip_select_row(
                        "DE Strategy",
                        &params.strategy,
                        DE_STRATEGY_OPTIONS,
                        prefix,
                        "strategy",
                        &theme,
                        cx,
                    ))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(self.render_stepper_row(
                                "Mutation (F)",
                                params.de_f,
                                ParamLimits::DE_FACTOR,
                                prefix,
                                "de_f",
                                &theme,
                                cx,
                            ))
                            .child(self.render_stepper_row(
                                "Recomb (CR)",
                                params.de_cr,
                                ParamLimits::DE_CR,
                                prefix,
                                "de_cr",
                                &theme,
                                cx,
                            )),
                    )
                })
                // Refinement toggle
                .child(self.render_toggle_row(
                    "Local Refinement",
                    params.refine,
                    prefix,
                    "refine",
                    &theme,
                    cx,
                ))
                // Local algorithm (only when refine is enabled)
                .when(params.refine, |d| {
                    d.child(self.render_chip_select_row(
                        "Local Algo",
                        &params.local_algo,
                        LOCAL_ALGO_OPTIONS,
                        prefix,
                        "local_algo",
                        &theme,
                        cx,
                    ))
                })
                // Smoothing toggle
                .child(self.render_toggle_row(
                    "Smoothing",
                    params.smooth,
                    prefix,
                    "smooth",
                    &theme,
                    cx,
                )),
        )
    }

    /// Render a stepper row with +/- buttons and value display
    fn render_stepper_row(
        &self,
        label: &str,
        value: f64,
        limits: ParamLimits,
        prefix: &str,
        param_name: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let label = label.to_string();
        let prefix_dec = prefix.to_string();
        let prefix_inc = prefix.to_string();
        let param_name_dec = param_name.to_string();
        let param_name_inc = param_name.to_string();
        let step = limits.step;
        let min = limits.min;
        let max = limits.max;

        // Format value nicely
        let display_value = if step >= 1000.0 {
            format!("{:.0}", value)
        } else if step >= 1.0 {
            format!("{:.0}", value)
        } else if step >= 0.1 {
            format!("{:.1}", value)
        } else {
            format!("{:.2}", value)
        };

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::new(label).size(TextSize::Xs).color(theme.text_secondary))
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    // Decrement button
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-{}-dec", prefix, param_name)))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .bg(theme.surface_hover)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.background_tertiary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    let new_value = (value - step).max(min);
                                    view.update_optimization_param(
                                        &prefix_dec,
                                        &param_name_dec,
                                        new_value,
                                        cx,
                                    );
                                }),
                            )
                            .child("−"),
                    )
                    // Value display
                    .child(
                        div()
                            .min_w(px(48.0))
                            .text_center()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .bg(theme.background_secondary)
                            .text_color(theme.text_primary)
                            .child(display_value),
                    )
                    // Increment button
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-{}-inc", prefix, param_name)))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .bg(theme.surface_hover)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.background_tertiary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    let new_value = (value + step).min(max);
                                    view.update_optimization_param(
                                        &prefix_inc,
                                        &param_name_inc,
                                        new_value,
                                        cx,
                                    );
                                }),
                            )
                            .child("+"),
                    ),
            )
    }

    /// Render chip-style selection row
    fn render_chip_select_row(
        &self,
        label: &str,
        current_value: &str,
        options: &[(&str, &str)],
        prefix: &str,
        param_name: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let current_value = current_value.to_string();
        let options: Vec<(String, String)> = options
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect();

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::new(label).size(TextSize::Xs).color(theme.text_secondary))
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .wrap(true)
                    .children(options.into_iter().map(|(value, display_label)| {
                        let is_selected = current_value == value;
                        let prefix = prefix.to_string();
                        let param_name = param_name.to_string();
                        let value_clone = value.clone();

                        Button::new(
                            SharedString::from(format!("{}-{}-{}", prefix, param_name, value)),
                            SharedString::from(display_label),
                        )
                        .variant(if is_selected {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(ButtonSize::Xs)
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.update_optimization_param_string(
                                    &prefix,
                                    &param_name,
                                    &value_clone,
                                    cx,
                                );
                            }),
                        )
                    })),
            )
    }

    /// Render a toggle row
    fn render_toggle_row(
        &self,
        label: &str,
        value: bool,
        prefix: &str,
        param_name: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let prefix_str = prefix.to_string();
        let param_name_str = param_name.to_string();
        let view = cx.entity().clone();

        let toggle_theme = ToggleTheme {
            checked_bg: theme.accent,
            unchecked_bg: theme.background_tertiary,
            knob: theme.text_primary,
            label: theme.text_secondary,
            accent: theme.accent,
            accent_muted: gpui::rgba(0x007acc33),
            success: gpui::rgb(0x22c55e),
            border: theme.border,
            text_on_accent: theme.text_primary,
            text_muted: theme.text_muted,
        };

        HStack::new()
            .spacing(StackSpacing::Md)
            .justify(gpui_ui_kit::StackJustify::SpaceBetween)
            .child(Text::new(label).size(TextSize::Xs).color(theme.text_secondary))
            .child(
                Toggle::new(SharedString::from(format!(
                    "toggle-{}-{}",
                    prefix, param_name
                )))
                .size(ToggleSize::Sm)
                .checked(value)
                .theme(toggle_theme)
                .on_change(move |new_val, _window, cx| {
                    view.update(cx, |view, cx| {
                        view.update_optimization_param_bool(&prefix_str, &param_name_str, new_val, cx);
                    });
                }),
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

    /// Update a boolean optimization parameter
    pub fn update_optimization_param_bool(
        &mut self,
        prefix: &str,
        param_name: &str,
        value: bool,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let params = match prefix {
                "headphone" => &mut state.app.headphone_params,
                _ => return,
            };

            match param_name {
                "refine" => params.refine = value,
                "smooth" => params.smooth = value,
                _ => {}
            }
        });
        cx.notify();
    }
}
