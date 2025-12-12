//! AutoEQ Parameter Form
//!
//! A reusable form component for AutoEQ optimization parameters.
//! Used by Room EQ, Speaker EQ, Headphone EQ, and Group optimization screens.
//!
//! The form includes:
//! - Algorithm selection (COBYLA, Differential Evolution, Nelder-Mead)
//! - Number of PEQ filters
//! - Q factor range (min/max)
//! - Gain range (min/max dB)
//! - Frequency range (min/max Hz)
//! - Maximum iterations

use gpui::prelude::*;
use gpui::*;

use crate::number_input::{NumberInput, NumberInputSize, NumberInputTheme};
use crate::select::{Select, SelectOption, SelectTheme};
use crate::stack::{HStack, StackSpacing, VStack};
use crate::text::{Text, TextSize, TextWeight};
use crate::theme::{Theme, ThemeExt};

// ============================================================================
// AutoEQ Form State (external state management)
// ============================================================================

/// Optimization algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoEqAlgorithm {
    /// COBYLA (Constrained Optimization BY Linear Approximations)
    #[default]
    Cobyla,
    /// Differential Evolution (global optimization)
    DifferentialEvolution,
    /// Nelder-Mead simplex (local optimization)
    NelderMead,
}

impl AutoEqAlgorithm {
    /// Get all available algorithms
    pub fn all() -> &'static [AutoEqAlgorithm] {
        &[
            AutoEqAlgorithm::Cobyla,
            AutoEqAlgorithm::DifferentialEvolution,
            AutoEqAlgorithm::NelderMead,
        ]
    }

    /// Get display label
    pub fn label(&self) -> &'static str {
        match self {
            AutoEqAlgorithm::Cobyla => "COBYLA",
            AutoEqAlgorithm::DifferentialEvolution => "Differential Evolution",
            AutoEqAlgorithm::NelderMead => "Nelder-Mead",
        }
    }

    /// Get string identifier (for CLI)
    pub fn id(&self) -> &'static str {
        match self {
            AutoEqAlgorithm::Cobyla => "cobyla",
            AutoEqAlgorithm::DifferentialEvolution => "autoeq:de",
            AutoEqAlgorithm::NelderMead => "nelder-mead",
        }
    }

    /// Parse from string identifier
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "cobyla" => Some(AutoEqAlgorithm::Cobyla),
            "autoeq:de" | "de" => Some(AutoEqAlgorithm::DifferentialEvolution),
            "nelder-mead" | "neldermead" => Some(AutoEqAlgorithm::NelderMead),
            _ => None,
        }
    }
}

/// AutoEQ optimization configuration
#[derive(Debug, Clone)]
pub struct AutoEqConfig {
    /// Optimization algorithm
    pub algorithm: AutoEqAlgorithm,
    /// Number of PEQ filters
    pub num_filters: usize,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum iterations
    pub max_iter: usize,
}

impl Default for AutoEqConfig {
    fn default() -> Self {
        Self {
            algorithm: AutoEqAlgorithm::Cobyla,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
        }
    }
}

/// UI state for AutoEQ form dropdowns
#[derive(Debug, Clone, Default)]
pub struct AutoEqFormUiState {
    /// Algorithm dropdown open state
    pub algorithm_open: bool,
    /// Currently editing field (for number inputs)
    pub editing_field: Option<AutoEqField>,
    /// Edit text for current field
    pub edit_text: String,
}

/// Field identifiers for the AutoEQ form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoEqField {
    NumFilters,
    MinQ,
    MaxQ,
    MinDb,
    MaxDb,
    MinFreq,
    MaxFreq,
    MaxIter,
}

// ============================================================================
// AutoEQ Form Theme
// ============================================================================

/// Theme for the AutoEQ form
#[derive(Debug, Clone)]
pub struct AutoEqFormTheme {
    /// Card background
    pub card_bg: Rgba,
    /// Section header color
    pub header_color: Rgba,
    /// Label color
    pub label_color: Rgba,
    /// Description color
    pub description_color: Rgba,
    /// NumberInput theme
    pub number_input_theme: NumberInputTheme,
    /// Select theme
    pub select_theme: SelectTheme,
}

impl From<&Theme> for AutoEqFormTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            card_bg: theme.surface,
            header_color: theme.text_primary,
            label_color: theme.text_secondary,
            description_color: theme.text_muted,
            number_input_theme: NumberInputTheme::from(theme),
            select_theme: SelectTheme::from(theme),
        }
    }
}

// ============================================================================
// AutoEQ Form Component
// ============================================================================

/// A reusable form for AutoEQ optimization parameters
#[derive(IntoElement)]
pub struct AutoEqForm {
    id: ElementId,
    config: AutoEqConfig,
    ui_state: AutoEqFormUiState,
    disabled: bool,
    compact: bool, // Compact layout (single column)
    show_algorithm: bool,
    show_iterations: bool,
    theme: Option<AutoEqFormTheme>,

    // Callbacks
    on_algorithm_change: Option<Box<dyn Fn(AutoEqAlgorithm, &mut Window, &mut App) + 'static>>,
    on_algorithm_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_num_filters_change: Option<Box<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_min_q_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_max_q_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_min_db_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_max_db_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_min_freq_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_max_freq_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_max_iter_change: Option<Box<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_field_edit_start: Option<Box<dyn Fn(AutoEqField, &mut Window, &mut App) + 'static>>,
    on_field_edit_end: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_edit_text_change: Option<Box<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl AutoEqForm {
    /// Create a new AutoEQ form
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            config: AutoEqConfig::default(),
            ui_state: AutoEqFormUiState::default(),
            disabled: false,
            compact: false,
            show_algorithm: true,
            show_iterations: true,
            theme: None,
            on_algorithm_change: None,
            on_algorithm_toggle: None,
            on_num_filters_change: None,
            on_min_q_change: None,
            on_max_q_change: None,
            on_min_db_change: None,
            on_max_db_change: None,
            on_min_freq_change: None,
            on_max_freq_change: None,
            on_max_iter_change: None,
            on_field_edit_start: None,
            on_field_edit_end: None,
            on_edit_text_change: None,
        }
    }

    /// Set the configuration values
    pub fn config(mut self, config: AutoEqConfig) -> Self {
        self.config = config;
        self
    }

    /// Set UI state
    pub fn ui_state(mut self, ui_state: AutoEqFormUiState) -> Self {
        self.ui_state = ui_state;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Use compact layout (single column)
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Show/hide algorithm selector
    pub fn show_algorithm(mut self, show: bool) -> Self {
        self.show_algorithm = show;
        self
    }

    /// Show/hide max iterations field
    pub fn show_iterations(mut self, show: bool) -> Self {
        self.show_iterations = show;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: AutoEqFormTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set algorithm change handler
    pub fn on_algorithm_change(
        mut self,
        handler: impl Fn(AutoEqAlgorithm, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_algorithm_change = Some(Box::new(handler));
        self
    }

    /// Set algorithm dropdown toggle handler
    pub fn on_algorithm_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_algorithm_toggle = Some(Box::new(handler));
        self
    }

    /// Set number of filters change handler
    pub fn on_num_filters_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_num_filters_change = Some(Box::new(handler));
        self
    }

    /// Set min Q change handler
    pub fn on_min_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_min_q_change = Some(Box::new(handler));
        self
    }

    /// Set max Q change handler
    pub fn on_max_q_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_max_q_change = Some(Box::new(handler));
        self
    }

    /// Set min dB change handler
    pub fn on_min_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_min_db_change = Some(Box::new(handler));
        self
    }

    /// Set max dB change handler
    pub fn on_max_db_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_max_db_change = Some(Box::new(handler));
        self
    }

    /// Set min frequency change handler
    pub fn on_min_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_min_freq_change = Some(Box::new(handler));
        self
    }

    /// Set max frequency change handler
    pub fn on_max_freq_change(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_max_freq_change = Some(Box::new(handler));
        self
    }

    /// Set max iterations change handler
    pub fn on_max_iter_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_max_iter_change = Some(Box::new(handler));
        self
    }

    /// Set field edit start handler
    pub fn on_field_edit_start(
        mut self,
        handler: impl Fn(AutoEqField, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_field_edit_start = Some(Box::new(handler));
        self
    }

    /// Set field edit end handler
    pub fn on_field_edit_end(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_field_edit_end = Some(Box::new(handler));
        self
    }

    /// Set edit text change handler
    pub fn on_edit_text_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit_text_change = Some(Box::new(handler));
        self
    }

}

impl RenderOnce for AutoEqForm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| AutoEqFormTheme::from(&global_theme));

        // Extract all values before moving self
        let id = self.id;
        let config = self.config;
        let ui_state = self.ui_state;
        let disabled = self.disabled;
        let show_algorithm = self.show_algorithm;
        let show_iterations = self.show_iterations;

        // Wrap callbacks in Rc for sharing
        let on_algorithm_change_rc = self.on_algorithm_change.map(std::rc::Rc::new);
        let on_algorithm_toggle_rc = self.on_algorithm_toggle.map(std::rc::Rc::new);
        let on_num_filters_change_rc = self.on_num_filters_change.map(std::rc::Rc::new);
        let on_min_q_change_rc = self.on_min_q_change.map(std::rc::Rc::new);
        let on_max_q_change_rc = self.on_max_q_change.map(std::rc::Rc::new);
        let on_min_db_change_rc = self.on_min_db_change.map(std::rc::Rc::new);
        let on_max_db_change_rc = self.on_max_db_change.map(std::rc::Rc::new);
        let on_min_freq_change_rc = self.on_min_freq_change.map(std::rc::Rc::new);
        let on_max_freq_change_rc = self.on_max_freq_change.map(std::rc::Rc::new);
        let on_max_iter_change_rc = self.on_max_iter_change.map(std::rc::Rc::new);
        let on_field_edit_start_rc = self.on_field_edit_start.map(std::rc::Rc::new);
        let on_field_edit_end_rc = self.on_field_edit_end.map(std::rc::Rc::new);
        let on_edit_text_change_rc = self.on_edit_text_change.map(std::rc::Rc::new);

        // Build algorithm options
        let algorithm_options: Vec<SelectOption> = AutoEqAlgorithm::all()
            .iter()
            .map(|alg| SelectOption::new(alg.id(), alg.label()))
            .collect();

        // Build the form
        let mut form = VStack::new().spacing(StackSpacing::Lg);

        // Algorithm selection
        if show_algorithm {
            let mut algo_select = Select::new("autoeq-algorithm")
                .label("Algorithm")
                .options(algorithm_options)
                .selected(config.algorithm.id())
                .is_open(ui_state.algorithm_open)
                .disabled(disabled)
                .theme(theme.select_theme.clone());

            if let Some(ref handler) = on_algorithm_toggle_rc {
                let h = handler.clone();
                algo_select = algo_select.on_toggle(move |open, w, cx| h(open, w, cx));
            }

            if let Some(ref handler) = on_algorithm_change_rc {
                let h = handler.clone();
                algo_select = algo_select.on_change(move |value, w, cx| {
                    if let Some(alg) = AutoEqAlgorithm::from_id(value.as_ref()) {
                        h(alg, w, cx);
                    }
                });
            }

            form = form.child(algo_select);
        }

        // Helper closure to create number input with common boilerplate
        // This is repeated inline due to Rust lifetime constraints

        // Number of filters
        let editing_num_filters = ui_state.editing_field == Some(AutoEqField::NumFilters);
        let mut num_filters_input = NumberInput::new("autoeq-num-filters")
            .value(config.num_filters as f64)
            .min(1.0)
            .max(30.0)
            .step(1.0)
            .decimals(0)
            .label("Number of Filters")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_num_filters)
            .theme(theme.number_input_theme.clone());

        if editing_num_filters {
            num_filters_input = num_filters_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_num_filters_change_rc {
            let h = handler.clone();
            num_filters_input =
                num_filters_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            num_filters_input =
                num_filters_input.on_edit_start(move |w, cx| h(AutoEqField::NumFilters, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_num_filters_change_rc.clone();
            num_filters_input = num_filters_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v.round() as usize, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            num_filters_input = num_filters_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        form = form.child(num_filters_input);

        // Q Factor Range
        let editing_min_q = ui_state.editing_field == Some(AutoEqField::MinQ);
        let mut min_q_input = NumberInput::new("autoeq-min-q")
            .value(config.min_q)
            .min(0.1)
            .max(20.0)
            .step(0.1)
            .decimals(1)
            .label("Min Q")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_min_q)
            .theme(theme.number_input_theme.clone());

        if editing_min_q {
            min_q_input = min_q_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_min_q_change_rc {
            let h = handler.clone();
            min_q_input = min_q_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            min_q_input = min_q_input.on_edit_start(move |w, cx| h(AutoEqField::MinQ, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_min_q_change_rc.clone();
            min_q_input = min_q_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            min_q_input = min_q_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let editing_max_q = ui_state.editing_field == Some(AutoEqField::MaxQ);
        let mut max_q_input = NumberInput::new("autoeq-max-q")
            .value(config.max_q)
            .min(0.1)
            .max(20.0)
            .step(0.1)
            .decimals(1)
            .label("Max Q")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_max_q)
            .theme(theme.number_input_theme.clone());

        if editing_max_q {
            max_q_input = max_q_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_max_q_change_rc {
            let h = handler.clone();
            max_q_input = max_q_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            max_q_input = max_q_input.on_edit_start(move |w, cx| h(AutoEqField::MaxQ, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_max_q_change_rc.clone();
            max_q_input = max_q_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            max_q_input = max_q_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let q_row = HStack::new()
            .spacing(StackSpacing::Md)
            .child(min_q_input)
            .child(max_q_input);

        form = form.child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("Q Factor Range")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Medium)
                        .color(theme.label_color),
                )
                .child(q_row),
        );

        // Gain Range (dB)
        let editing_min_db = ui_state.editing_field == Some(AutoEqField::MinDb);
        let mut min_db_input = NumberInput::new("autoeq-min-db")
            .value(config.min_db)
            .min(-24.0)
            .max(0.0)
            .step(0.5)
            .decimals(1)
            .unit("dB")
            .label("Min")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_min_db)
            .theme(theme.number_input_theme.clone());

        if editing_min_db {
            min_db_input = min_db_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_min_db_change_rc {
            let h = handler.clone();
            min_db_input = min_db_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            min_db_input = min_db_input.on_edit_start(move |w, cx| h(AutoEqField::MinDb, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_min_db_change_rc.clone();
            min_db_input = min_db_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            min_db_input = min_db_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let editing_max_db = ui_state.editing_field == Some(AutoEqField::MaxDb);
        let mut max_db_input = NumberInput::new("autoeq-max-db")
            .value(config.max_db)
            .min(0.0)
            .max(24.0)
            .step(0.5)
            .decimals(1)
            .unit("dB")
            .label("Max")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_max_db)
            .theme(theme.number_input_theme.clone());

        if editing_max_db {
            max_db_input = max_db_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_max_db_change_rc {
            let h = handler.clone();
            max_db_input = max_db_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            max_db_input = max_db_input.on_edit_start(move |w, cx| h(AutoEqField::MaxDb, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_max_db_change_rc.clone();
            max_db_input = max_db_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            max_db_input = max_db_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let db_row = HStack::new()
            .spacing(StackSpacing::Md)
            .child(min_db_input)
            .child(max_db_input);

        form = form.child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("Gain Range")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Medium)
                        .color(theme.label_color),
                )
                .child(db_row),
        );

        // Frequency Range
        let editing_min_freq = ui_state.editing_field == Some(AutoEqField::MinFreq);
        let mut min_freq_input = NumberInput::new("autoeq-min-freq")
            .value(config.min_freq)
            .min(20.0)
            .max(500.0)
            .step(10.0)
            .decimals(0)
            .unit("Hz")
            .label("Min")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_min_freq)
            .theme(theme.number_input_theme.clone());

        if editing_min_freq {
            min_freq_input = min_freq_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_min_freq_change_rc {
            let h = handler.clone();
            min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            min_freq_input =
                min_freq_input.on_edit_start(move |w, cx| h(AutoEqField::MinFreq, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_min_freq_change_rc.clone();
            min_freq_input = min_freq_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            min_freq_input = min_freq_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let editing_max_freq = ui_state.editing_field == Some(AutoEqField::MaxFreq);
        let mut max_freq_input = NumberInput::new("autoeq-max-freq")
            .value(config.max_freq)
            .min(1000.0)
            .max(24000.0)
            .step(100.0)
            .decimals(0)
            .unit("Hz")
            .label("Max")
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .editing(editing_max_freq)
            .theme(theme.number_input_theme.clone());

        if editing_max_freq {
            max_freq_input = max_freq_input.edit_text(ui_state.edit_text.clone());
        }

        if let Some(ref handler) = on_max_freq_change_rc {
            let h = handler.clone();
            max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        if let Some(ref handler) = on_field_edit_start_rc {
            let h = handler.clone();
            max_freq_input =
                max_freq_input.on_edit_start(move |w, cx| h(AutoEqField::MaxFreq, w, cx));
        }

        if let Some(ref handler) = on_field_edit_end_rc {
            let h = handler.clone();
            let change_handler = on_max_freq_change_rc.clone();
            max_freq_input = max_freq_input.on_edit_end(move |val, w, cx| {
                if let Some(v) = val {
                    if let Some(ref ch) = change_handler {
                        ch(v, w, cx);
                    }
                }
                h(w, cx);
            });
        }

        if let Some(ref handler) = on_edit_text_change_rc {
            let h = handler.clone();
            max_freq_input = max_freq_input.on_text_change(move |text, w, cx| h(text, w, cx));
        }

        let freq_row = HStack::new()
            .spacing(StackSpacing::Md)
            .child(min_freq_input)
            .child(max_freq_input);

        form = form.child(
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new("Frequency Range")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Medium)
                        .color(theme.label_color),
                )
                .child(freq_row),
        );

        // Max iterations
        if show_iterations {
            let editing_max_iter = ui_state.editing_field == Some(AutoEqField::MaxIter);
            let mut max_iter_input = NumberInput::new("autoeq-max-iter")
                .value(config.max_iter as f64)
                .min(100.0)
                .max(100000.0)
                .step(1000.0)
                .decimals(0)
                .label("Max Iterations")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .editing(editing_max_iter)
                .theme(theme.number_input_theme.clone());

            if editing_max_iter {
                max_iter_input = max_iter_input.edit_text(ui_state.edit_text.clone());
            }

            if let Some(ref handler) = on_max_iter_change_rc {
                let h = handler.clone();
                max_iter_input =
                    max_iter_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
            }

            if let Some(ref handler) = on_field_edit_start_rc {
                let h = handler.clone();
                max_iter_input =
                    max_iter_input.on_edit_start(move |w, cx| h(AutoEqField::MaxIter, w, cx));
            }

            if let Some(ref handler) = on_field_edit_end_rc {
                let h = handler.clone();
                let change_handler = on_max_iter_change_rc.clone();
                max_iter_input = max_iter_input.on_edit_end(move |val, w, cx| {
                    if let Some(v) = val {
                        if let Some(ref ch) = change_handler {
                            ch(v.round() as usize, w, cx);
                        }
                    }
                    h(w, cx);
                });
            }

            if let Some(ref handler) = on_edit_text_change_rc {
                let h = handler.clone();
                max_iter_input = max_iter_input.on_text_change(move |text, w, cx| h(text, w, cx));
            }

            form = form.child(max_iter_input);
        }

        // Wrap in a div with the form ID
        div().id(id).child(form)
    }
}
