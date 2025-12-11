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
    Card, HStack, NumberInput, Select, SelectOption, StackSpacing, Text, TextSize, TextWeight,
    Toggle, ToggleSize, ToggleTheme, VStack,
};

use crate::app::types::OptimizationUiState;

use crate::optimization_params::*;
use crate::theme::Theme;
use crate::ui::PlayerView;

impl PlayerView {
    /// Render EQ Design parameters section
    pub fn render_eq_design_params(
        &self,
        params: &OptimizationParams,
        ui_state: &OptimizationUiState,
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
                .child(self.render_dropdown_row(
                    "PEQ Model",
                    &params.peq_model,
                    PEQ_MODEL_OPTIONS,
                    ui_state.peq_model_open,
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
        ui_state: &OptimizationUiState,
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
                .child(self.render_dropdown_row(
                    "Algorithm",
                    &params.algo,
                    ALGORITHM_OPTIONS,
                    ui_state.algo_open,
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
                    d.child(self.render_dropdown_row(
                        "DE Strategy",
                        &params.strategy,
                        DE_STRATEGY_OPTIONS,
                        ui_state.strategy_open,
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
                    d.child(self.render_dropdown_row(
                        "Local Algo",
                        &params.local_algo,
                        LOCAL_ALGO_OPTIONS,
                        ui_state.local_algo_open,
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

    /// Render a stepper row with NumberInput
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

        // Capture weak handle to view for callbacks
        let view = cx.entity().downgrade();
        let prefix_dec = prefix.to_string();
        let _prefix_inc = prefix.to_string();
        let param_name_dec = param_name.to_string();
        let _param_name_inc = param_name.to_string();

        let decimals = if limits.step < 1.0 { 1 } else { 0 };

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::new(label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                NumberInput::new(SharedString::from(format!("{}-{}", prefix, param_name)))
                    .value(value)
                    .min(limits.min)
                    .max(limits.max)
                    .step(limits.step)
                    .decimals(decimals)
                    .width(100.0)
                    .on_change(move |new_val, _window, cx| {
                        if let Some(view) = view.upgrade() {
                            view.update(cx, |view, cx| {
                                view.update_optimization_param(
                                    &prefix_dec,
                                    &param_name_dec,
                                    new_val,
                                    cx,
                                );
                            });
                        }
                    }),
            )
        // Wait, this is getting complicated.
        // The previous code used `cx.listener`.
        // `Button` uses `on_mouse_up(..., cx.listener(...))`.
        // `NumberInput` uses internal `on_mouse_up` and calls `self.on_change`.
        // The `on_change` provided to `NumberInput` takes `&mut App`.
        // Check `NumberInput` definition again.
        // `pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static)`.

        // To update the view from there, we need a handle to the view.
        // `cx` passed to `render_stepper_row` is `&mut Context<PlayerView>`.
        // So we can get a weak handle: `let view = cx.view().downgrade();`
    }

    /// Render dropdown selection row using Select component
    fn render_dropdown_row(
        &self,
        label: &str,
        current_value: &str,
        options: &[(&str, &str)],
        is_open: bool,
        prefix: &str,
        param_name: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let label = label.to_string();
        let prefix = prefix.to_string();
        let param_name = param_name.to_string();
        let current_value = current_value.to_string();

        let select_options: Vec<SelectOption> = options
            .iter()
            .map(|(val, lbl)| SelectOption::new(val.to_string(), lbl.to_string()))
            .collect();

        // Capture weak handle to view for callbacks
        let view = cx.entity().downgrade();
        let view2 = cx.entity().downgrade();

        // Clone for callbacks
        let prefix1 = prefix.clone();
        let param_name1 = param_name.clone();
        let prefix2 = prefix.clone();
        let param_name2 = param_name.clone();

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::new(label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                Select::new(SharedString::from(format!(
                    "select-{}-{}",
                    prefix, param_name
                )))
                .options(select_options)
                .selected(current_value)
                .is_open(is_open)
                .on_change(move |new_val, _window, cx| {
                    let new_val = new_val.to_string();
                    let prefix = prefix1.clone();
                    let param_name = param_name1.clone();

                    if let Some(view) = view.upgrade() {
                        view.update(cx, |view, cx| {
                            view.update_optimization_param_string(
                                &prefix,
                                &param_name,
                                &new_val,
                                cx,
                            );
                            // Close dropdown after selection
                            view.update_optimization_ui_dropdown(&prefix, &param_name, false, cx);
                        });
                    }
                })
                // Select component doesn't have explicit on_toggle handler exposed in the viewed file...
                // Wait, Select component in `select.rs` does NOT expose a toggle handler!
                // It uses `trigger.on_mouse_up(...)` internally?
                // Checking `select.rs` (Step 3809):
                // `trigger = trigger.on_click(...)`?
                // No, `Select` does NOT handle open/close internally if `is_open` is used?
                // `Select` has `is_open` prop.
                // But `Select` implementation (Step 3809) does NOT have an `on_toggle` or binding to click the trigger to toggle state externally.
                // It seems `Select` in `select.rs` handles internal `on_click` only if we ADD it?
                // In `select.rs` line 232: `div()...cursor_pointer()`.
                // But no `on_mouse_up` attached to `trigger` in the `build()` method!
                // THIS IS A BUG/MISSING FEATURE in `Select` component?
                // Wait, `Select` seems designed to be controlled?
                // If I use `Select` without `on_toggle`, how does it open?
                // I must wrap `Select` or `Select` must handle it.
                // The `Select` code shows: `trigger` div built. No event handler attached to it regarding open state.
                // AND `on_change` is used for `option` clicks.
                // I MUST ADD event handler to the `Select` wrapper or proper interactivity.
                // Since I cannot modify `Select` easily (it's in ui-kit which I can modify, but prefer not to if possible).
                // Actually, if `Select` is broken/incomplete, I SHOULD fix `Select`.
                // But simple fix: Wrap `Select` in a div that handles click?
                // No, the trigger is inside.
                // Let's modify `Select` to support `on_toggle`?
                // Or let's assume I can click it?
                // Wait, line 232-244 builds trigger.
                // It DOES NOT attach an event listener.
                // So `Select` as implemented is static unless modified.
                // I WILL MODIFY `Select` component in `gpui-ui-kit/src/select.rs` to support `on_toggle`.
                // But let's finish `optimization_forms.rs` assuming `Select` works or I will fix it.
                // I will assume `Select` has `on_toggle` or I will add it.
                // The user prompt implied "use the new number input... and transform...".
                // I'll stick to `optimization_forms.rs` edits first.
                // I'll add `on_toggle` callback to `Select` usage here.
                // And I'll update `Select` implementation in a separate step.
                .on_toggle(move |open, _window, cx| {
                    let prefix = prefix2.clone();
                    let param_name = param_name2.clone();
                    if let Some(view) = view2.upgrade() {
                        view.update(cx, |view, cx| {
                            view.update_optimization_ui_dropdown(&prefix, &param_name, open, cx);
                        });
                    }
                }),
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
            accent_muted: Theme::opacity_20pct(theme.accent),
            success: theme.success,
            border: theme.border,
            text_on_accent: theme.text_primary,
            text_muted: theme.text_muted,
        };

        HStack::new()
            .spacing(StackSpacing::Md)
            .justify(gpui_ui_kit::StackJustify::SpaceBetween)
            .child(
                Text::new(label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
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
                        view.update_optimization_param_bool(
                            &prefix_str,
                            &param_name_str,
                            new_val,
                            cx,
                        );
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

    /// Update UI state for dropdowns
    pub fn update_optimization_ui_dropdown(
        &mut self,
        prefix: &str,
        param_name: &str,
        is_open: bool,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let ui_state = match prefix {
                "headphone" => &mut state.app.headphone_opt_ui,
                "speaker" => &mut state.app.speaker_opt_ui,
                _ => return,
            };

            match param_name {
                "peq_model" => ui_state.peq_model_open = is_open,
                "algo" => ui_state.algo_open = is_open,
                "strategy" => ui_state.strategy_open = is_open,
                "local_algo" => ui_state.local_algo_open = is_open,
                _ => {}
            }
        });
        cx.notify();
    }
}
