//! AutoEQ Form Debug Example
//!
//! Interactive showcase for the AutoEQ Form component:
//! - Algorithm selection
//! - Number input fields with various parameters
//! - Compact vs standard layout
//! - Disabled state

use gpui::*;
use gpui_ui_kit::autoeq_form::{
    AutoEqAlgorithm, AutoEqConfig, AutoEqField, AutoEqForm, AutoEqFormUiState,
};
use gpui_ui_kit::i18n::{I18nExt, TranslationKey};
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::*;

/// Demo state
pub struct AutoEqFormDebug {
    /// Main form config
    config: AutoEqConfig,
    /// UI state for main form
    ui_state: AutoEqFormUiState,
    /// Compact form config
    compact_config: AutoEqConfig,
    /// UI state for compact form
    compact_ui_state: AutoEqFormUiState,
    /// Entity reference
    entity: Entity<Self>,
}

impl AutoEqFormDebug {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            config: AutoEqConfig::default(),
            ui_state: AutoEqFormUiState::default(),
            compact_config: AutoEqConfig {
                algorithm: AutoEqAlgorithm::DifferentialEvolution,
                num_filters: 7,
                min_q: 1.0,
                max_q: 8.0,
                min_db: -6.0,
                max_db: 6.0,
                min_freq: 50.0,
                max_freq: 16000.0,
                max_iter: 5000,
            },
            compact_ui_state: AutoEqFormUiState::default(),
            entity: cx.entity().clone(),
        }
    }
}

impl Render for AutoEqFormDebug {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = self.entity.clone();
        let theme = cx.theme();

        div()
            .id("autoeq-form-debug-root")
            .w_full()
            .h_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            // Header
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h1("AutoEQ Form Component Debug"))
                    .child(
                        Text::new("Configure EQ optimization parameters. Click number inputs to edit, use dropdowns for selection.")
                            .muted(true),
                    ),
            )
            // i18n Status Bar - demonstrates language switching works
            .child(
                div()
                    .flex()
                    .gap_4()
                    .p_3()
                    .bg(theme.surface)
                    .rounded_lg()
                    .child(Text::new(format!("🌐 {}: ", cx.t(TranslationKey::MenuLanguage))).weight(TextWeight::Medium))
                    .child(Text::new(cx.language().native_name()).color(theme.accent))
                    .child(Text::new(" | "))
                    .child(Text::new(cx.t(TranslationKey::SectionFormControls)).color(theme.text_secondary)),
            )
            // Current Config Display
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .child(
                        Text::new("Current Configuration")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(Text::new(format!("Algorithm: {}", self.config.algorithm.label())).size(TextSize::Sm))
                            .child(Text::new(format!("Filters: {}", self.config.num_filters)).size(TextSize::Sm))
                            .child(Text::new(format!("Q: {:.1} - {:.1}", self.config.min_q, self.config.max_q)).size(TextSize::Sm))
                            .child(Text::new(format!("dB: {:.1} - {:.1}", self.config.min_db, self.config.max_db)).size(TextSize::Sm))
                            .child(Text::new(format!("Freq: {:.0} - {:.0} Hz", self.config.min_freq, self.config.max_freq)).size(TextSize::Sm))
                            .child(Text::new(format!("Iterations: {}", self.config.max_iter)).size(TextSize::Sm)),
                    ),
            )
            // Main Form
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .child(
                        Text::new("Standard Form (Full)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(Text::new("All options visible, interactive editing").size(TextSize::Sm).muted(true))
                    .child({
                        let config = self.config.clone();
                        let ui_state = self.ui_state.clone();
                        AutoEqForm::new("main-form")
                            .config(config)
                            .ui_state(ui_state)
                            .show_algorithm(true)
                            .show_iterations(true)
                            .on_algorithm_change({
                                let entity = entity.clone();
                                move |alg, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.algorithm = alg;
                                    });
                                }
                            })
                            .on_algorithm_toggle({
                                let entity = entity.clone();
                                move |open, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.ui_state.algorithm_open = open;
                                    });
                                }
                            })
                            .on_num_filters_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.num_filters = val;
                                    });
                                }
                            })
                            .on_min_q_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.min_q = val;
                                    });
                                }
                            })
                            .on_max_q_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.max_q = val;
                                    });
                                }
                            })
                            .on_min_db_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.min_db = val;
                                    });
                                }
                            })
                            .on_max_db_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.max_db = val;
                                    });
                                }
                            })
                            .on_min_freq_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.min_freq = val;
                                    });
                                }
                            })
                            .on_max_freq_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.max_freq = val;
                                    });
                                }
                            })
                            .on_max_iter_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.config.max_iter = val;
                                    });
                                }
                            })
                            .on_field_edit_start({
                                let entity = entity.clone();
                                move |field, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.ui_state.editing_field = Some(field);
                                        this.ui_state.edit_text = match field {
                                            AutoEqField::NumFilters => this.config.num_filters.to_string(),
                                            AutoEqField::MinQ => format!("{:.1}", this.config.min_q),
                                            AutoEqField::MaxQ => format!("{:.1}", this.config.max_q),
                                            AutoEqField::MinDb => format!("{:.1}", this.config.min_db),
                                            AutoEqField::MaxDb => format!("{:.1}", this.config.max_db),
                                            AutoEqField::MinFreq => format!("{:.0}", this.config.min_freq),
                                            AutoEqField::MaxFreq => format!("{:.0}", this.config.max_freq),
                                            AutoEqField::MaxIter => this.config.max_iter.to_string(),
                                        };
                                    });
                                }
                            })
                            .on_field_edit_end({
                                let entity = entity.clone();
                                move |_w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.ui_state.editing_field = None;
                                        this.ui_state.edit_text.clear();
                                    });
                                }
                            })
                            .on_edit_text_change({
                                let entity = entity.clone();
                                move |text, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.ui_state.edit_text = text;
                                    });
                                }
                            })
                    }),
            )
            // Compact Form (no algorithm, no iterations)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .child(
                        Text::new("Compact Form (No Algorithm/Iterations)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(Text::new("Simplified form without algorithm and iterations fields").size(TextSize::Sm).muted(true))
                    .child({
                        let config = self.compact_config.clone();
                        let ui_state = self.compact_ui_state.clone();
                        AutoEqForm::new("compact-form")
                            .config(config)
                            .ui_state(ui_state)
                            .show_algorithm(false)
                            .show_iterations(false)
                            .on_num_filters_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.num_filters = val;
                                    });
                                }
                            })
                            .on_min_q_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.min_q = val;
                                    });
                                }
                            })
                            .on_max_q_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.max_q = val;
                                    });
                                }
                            })
                            .on_min_db_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.min_db = val;
                                    });
                                }
                            })
                            .on_max_db_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.max_db = val;
                                    });
                                }
                            })
                            .on_min_freq_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.min_freq = val;
                                    });
                                }
                            })
                            .on_max_freq_change({
                                let entity = entity.clone();
                                move |val, _w, cx| {
                                    entity.update(cx, |this, _| {
                                        this.compact_config.max_freq = val;
                                    });
                                }
                            })
                    }),
            )
            // Disabled Form
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .child(
                        Text::new("Disabled Form")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(Text::new("All inputs are disabled - useful during optimization").size(TextSize::Sm).muted(true))
                    .child(
                        AutoEqForm::new("disabled-form")
                            .config(AutoEqConfig::default())
                            .ui_state(AutoEqFormUiState::default())
                            .disabled(true),
                    ),
            )
            // Algorithm Reference
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .child(
                        Text::new("Algorithm Reference")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(Text::new("COBYLA:").weight(TextWeight::Medium).size(TextSize::Sm))
                                    .child(Text::new("Fast local optimizer, good for quick results").size(TextSize::Sm).muted(true)),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(Text::new("Differential Evolution:").weight(TextWeight::Medium).size(TextSize::Sm))
                                    .child(Text::new("Global optimizer, finds best overall solution").size(TextSize::Sm).muted(true)),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(Text::new("Nelder-Mead:").weight(TextWeight::Medium).size(TextSize::Sm))
                                    .child(Text::new("Simple local optimizer, good starting point").size(TextSize::Sm).muted(true)),
                            ),
                    ),
            )
    }
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("AutoEQ Form Debug")
            .size(800.0, 950.0)
            .scrollable(true)
            .with_theme(true)
            .with_i18n(true),
        |cx| cx.new(|cx| AutoEqFormDebug::new(cx)),
    );
}
