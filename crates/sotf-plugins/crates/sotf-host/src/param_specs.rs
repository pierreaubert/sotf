//! Centralized Parameter Specifications
//!
//! This module defines all plugin parameter specifications in one place.
//! Both plugin implementations and UI code should reference these constants
//! to ensure consistency and single-source-of-truth for parameter ranges,
//! defaults, and metadata.
//!
//! Each plugin module exports a `PARAMS` array of `ParamSpec` that serves as
//! the single source of truth for parameter name, engine key, type, range,
//! step size, unit, group, and update mode. All consumer code (TUI descriptors,
//! GPUI editing, engine param mapping, serde defaults) derives from these specs.

// ============================================================================
// Rich Parameter Specification Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamType {
    Float {
        default: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Int {
        default: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    Bool {
        default: bool,
        true_label: &'static str,
        false_label: &'static str,
    },
    Choice {
        default_index: usize,
        labels: &'static [&'static str],
    },
    FilePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    /// Parameter can be updated without rebuilding the plugin (zero-dropout).
    Realtime,
    /// Parameter change requires rebuilding the plugin (e.g., channel count change).
    Structural,
}

/// UI layout category for automatic 3-column plugin rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamCategory {
    /// Left column: structural params, mode selectors, channel config.
    Setup,
    /// Center-top: main controls users adjust frequently.
    Primary,
    /// Center-bottom with tabs: advanced/fine-tuning params. Tab name groups them.
    Secondary(&'static str),
    /// Right column: meter, AutoGain, Mix, Makeup.
    Output,
    /// Center-bottom tab: bypass toggles, debug params.
    Diagnostic,
}

#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub name: &'static str,
    pub engine_key: &'static str,
    pub param_type: ParamType,
    pub unit: &'static str,
    pub group: &'static str,
    pub update_mode: UpdateMode,
    /// Multiplier from internal value to display value.
    /// E.g., 100.0 for a 0..1 value displayed as 0..100%.
    /// `set_plugin_param()` divides incoming display values by this.
    pub display_scale: f64,
    /// UI layout category for automatic 3-column rendering.
    /// Defaults to `Primary` — override with `.setup()`, `.output()`, etc.
    pub category: ParamCategory,
    /// Short documentation string shown in the simple plugin editor.
    pub doc: &'static str,
}

#[allow(clippy::too_many_arguments)]
impl ParamSpec {
    pub const fn float(
        name: &'static str,
        engine_key: &'static str,
        default: f64,
        min: f64,
        max: f64,
        step: f64,
        unit: &'static str,
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::Float {
                default,
                min,
                max,
                step,
            },
            unit,
            group,
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    pub const fn int(
        name: &'static str,
        engine_key: &'static str,
        default: i64,
        min: i64,
        max: i64,
        step: i64,
        unit: &'static str,
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::Int {
                default,
                min,
                max,
                step,
            },
            unit,
            group,
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    pub const fn bool_param(
        name: &'static str,
        engine_key: &'static str,
        default: bool,
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::Bool {
                default,
                true_label: "On",
                false_label: "Off",
            },
            unit: "",
            group,
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    pub const fn bool_labeled(
        name: &'static str,
        engine_key: &'static str,
        default: bool,
        true_label: &'static str,
        false_label: &'static str,
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::Bool {
                default,
                true_label,
                false_label,
            },
            unit: "",
            group,
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    pub const fn choice(
        name: &'static str,
        engine_key: &'static str,
        default_index: usize,
        labels: &'static [&'static str],
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::Choice {
                default_index,
                labels,
            },
            unit: "",
            group,
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    pub const fn file_path(
        name: &'static str,
        engine_key: &'static str,
        group: &'static str,
    ) -> Self {
        Self {
            name,
            engine_key,
            param_type: ParamType::FilePath,
            unit: "",
            group,
            update_mode: UpdateMode::Structural,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        }
    }

    /// Mark this parameter as requiring a structural rebuild.
    pub const fn structural(self) -> Self {
        Self {
            update_mode: UpdateMode::Structural,
            ..self
        }
    }

    /// Set display_scale: multiplier from internal to display value.
    pub const fn scaled(self, display_scale: f64) -> Self {
        Self {
            display_scale,
            ..self
        }
    }

    /// Set category to Setup (left column).
    pub const fn setup(self) -> Self {
        Self {
            category: ParamCategory::Setup,
            ..self
        }
    }

    /// Set category to Output (right column).
    pub const fn output(self) -> Self {
        Self {
            category: ParamCategory::Output,
            ..self
        }
    }

    /// Set category to Secondary with a tab name (center-bottom tabs).
    pub const fn secondary(self, tab: &'static str) -> Self {
        Self {
            category: ParamCategory::Secondary(tab),
            ..self
        }
    }

    /// Set category to Diagnostic (center-bottom diagnostic tab).
    pub const fn diagnostic(self) -> Self {
        Self {
            category: ParamCategory::Diagnostic,
            ..self
        }
    }

    /// Set a documentation string for display in the simple editor.
    pub const fn doc(self, doc: &'static str) -> Self {
        Self { doc, ..self }
    }

    /// Clamp a raw internal value to this param's valid range.
    pub fn clamp_f64(&self, value: f64) -> f64 {
        match self.param_type {
            ParamType::Float { min, max, .. } => value.clamp(min, max),
            ParamType::Int { min, max, .. } => (value as i64).clamp(min, max) as f64,
            ParamType::Bool { .. } => {
                if value > 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            ParamType::Choice { labels, .. } => {
                if labels.is_empty() {
                    return value;
                }
                (value as usize).min(labels.len() - 1) as f64
            }
            ParamType::FilePath => value,
        }
    }

    /// Adjust a value by delta steps, returns the clamped new value.
    /// For Float/Int: applies delta * step and clamps.
    /// For Bool: toggles (ignores delta direction).
    /// For Choice: cycles forward/backward through labels.
    pub fn adjust_f64(&self, current: f64, delta: f64) -> f64 {
        match self.param_type {
            ParamType::Float { min, max, step, .. } => (current + delta * step).clamp(min, max),
            ParamType::Int { min, max, step, .. } => {
                let new_val = (current as i64).saturating_add((delta as i64).saturating_mul(step));
                new_val.clamp(min, max) as f64
            }
            ParamType::Bool { .. } => {
                if current > 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            ParamType::Choice { labels, .. } => {
                let count = labels.len();
                if count == 0 {
                    return current;
                }
                let idx = current as usize;
                if delta > 0.0 {
                    ((idx + 1) % count) as f64
                } else {
                    ((idx + count - 1) % count) as f64
                }
            }
            ParamType::FilePath => current,
        }
    }

    /// Get the default value as f64.
    pub fn default_f64(&self) -> f64 {
        match self.param_type {
            ParamType::Float { default, .. } => default,
            ParamType::Int { default, .. } => default as f64,
            ParamType::Bool { default, .. } => {
                if default {
                    1.0
                } else {
                    0.0
                }
            }
            ParamType::Choice { default_index, .. } => default_index as f64,
            ParamType::FilePath => 0.0,
        }
    }

    /// Get the default value as bool (panics if not a Bool param).
    pub fn default_bool(&self) -> bool {
        match self.param_type {
            ParamType::Bool { default, .. } => default,
            _ => panic!("default_bool() called on non-Bool param '{}'", self.name),
        }
    }

    /// Get the default value as usize.
    pub fn default_usize(&self) -> usize {
        self.default_f64() as usize
    }

    /// Get the default value as i32.
    pub fn default_i32(&self) -> i32 {
        self.default_f64() as i32
    }

    /// Get the default value as f32.
    pub fn default_f32(&self) -> f32 {
        self.default_f64() as f32
    }

    /// Get the minimum value as f64.
    pub fn min_f64(&self) -> f64 {
        match self.param_type {
            ParamType::Float { min, .. } => min,
            ParamType::Int { min, .. } => min as f64,
            ParamType::Bool { .. } => 0.0,
            ParamType::Choice { .. } => 0.0,
            ParamType::FilePath => 0.0,
        }
    }

    /// Get the maximum value as f64.
    pub fn max_f64(&self) -> f64 {
        match self.param_type {
            ParamType::Float { max, .. } => max,
            ParamType::Int { max, .. } => max as f64,
            ParamType::Bool { .. } => 1.0,
            ParamType::Choice { labels, .. } => {
                if labels.is_empty() {
                    0.0
                } else {
                    (labels.len() - 1) as f64
                }
            }
            ParamType::FilePath => 0.0,
        }
    }

    /// Derive display precision from step size.
    pub fn precision(&self) -> usize {
        match self.param_type {
            ParamType::Float { step, .. } => {
                if step >= 1.0 {
                    0
                } else if step >= 0.1 {
                    1
                } else if step >= 0.01 {
                    2
                } else if step >= 0.001 {
                    3
                } else {
                    4
                }
            }
            _ => 0,
        }
    }

    /// Format a f64 value according to this param's type, precision, and labels.
    pub fn format_value(&self, value: f64) -> String {
        match self.param_type {
            ParamType::Float { .. } => {
                if self.unit == "%" {
                    format!("{:.0}%", value * 100.0)
                } else {
                    match self.precision() {
                        0 => format!("{:.0}", value),
                        1 => format!("{:.1}", value),
                        2 => format!("{:.2}", value),
                        3 => format!("{:.3}", value),
                        _ => format!("{:.4}", value),
                    }
                }
            }
            ParamType::Int { .. } => format!("{}", value as i64),
            ParamType::Bool {
                true_label,
                false_label,
                ..
            } => {
                if value > 0.5 {
                    true_label.to_string()
                } else {
                    false_label.to_string()
                }
            }
            ParamType::Choice { labels, .. } => {
                let idx = value as usize;
                if idx < labels.len() {
                    labels[idx].to_string()
                } else {
                    format!("{}", idx)
                }
            }
            ParamType::FilePath => String::new(),
        }
    }

    /// Format a raw f64 value as the engine expects it.
    /// Float: raw number, Int: integer, Bool: "true"/"false", Choice: index as integer.
    pub fn engine_value_string(&self, value: f64) -> String {
        match self.param_type {
            ParamType::Float { .. } => format!("{}", value),
            ParamType::Int { .. } => format!("{}", value as i64),
            ParamType::Bool { .. } => {
                if value > 0.5 {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            ParamType::Choice { .. } => format!("{}", value as i64),
            ParamType::FilePath => String::new(),
        }
    }

    /// Get the labels for a Choice parameter (empty slice for other types).
    pub fn choice_labels(&self) -> &'static [&'static str] {
        match self.param_type {
            ParamType::Choice { labels, .. } => labels,
            _ => &[],
        }
    }
}

/// Look up a `ParamSpec` by its `engine_key` within a PARAMS slice.
/// Panics if the key is not found (programmer error).
/// Look up the index of a parameter by its `engine_key` at compile time.
///
/// Panics at compile time if the key is not found, making stale
/// hardcoded indices a compilation error.
///
/// ```ignore
/// const GAIN_IDX: usize = index_of(gain::PARAMS, "gain_db");
/// ```
pub const fn index_of(params: &[ParamSpec], key: &str) -> usize {
    let key_bytes = key.as_bytes();
    let mut i = 0;
    while i < params.len() {
        let ek = params[i].engine_key.as_bytes();
        if ek.len() == key_bytes.len() {
            let mut j = 0;
            let mut eq = true;
            while j < ek.len() {
                if ek[j] != key_bytes[j] {
                    eq = false;
                    break;
                }
                j += 1;
            }
            if eq {
                return i;
            }
        }
        i += 1;
    }
    panic!("index_of: no ParamSpec with the given engine_key")
}

pub fn find_by_key<'a>(params: &'a [ParamSpec], key: &str) -> &'a ParamSpec {
    params
        .iter()
        .find(|s| s.engine_key == key)
        .unwrap_or_else(|| panic!("no ParamSpec with engine_key '{}'", key))
}

/// Generate serde default wrapper functions from PARAMS arrays.
///
/// Usage:
/// ```ignore
/// sotf_plugins::serde_param_default! {
///     module::PARAMS;
///     fn default_foo() -> f64 = "engine_key";
///     fn default_bar() -> bool = "engine_key";
///     fn default_baz() -> usize = "engine_key";
/// }
/// ```
#[macro_export]
macro_rules! serde_param_default {
    ($params:expr; $(fn $fn_name:ident() -> $ret:ident = $key:literal;)*) => {
        $(
            $crate::serde_param_default!(@one $params, $fn_name, $ret, $key);
        )*
    };
    (@one $params:expr, $fn_name:ident, f64, $key:literal) => {
        fn $fn_name() -> f64 {
            $crate::param_specs::find_by_key($params, $key).default_f64()
        }
    };
    (@one $params:expr, $fn_name:ident, bool, $key:literal) => {
        fn $fn_name() -> bool {
            $crate::param_specs::find_by_key($params, $key).default_bool()
        }
    };
    (@one $params:expr, $fn_name:ident, usize, $key:literal) => {
        fn $fn_name() -> usize {
            $crate::param_specs::find_by_key($params, $key).default_usize()
        }
    };
    (@one $params:expr, $fn_name:ident, i32, $key:literal) => {
        fn $fn_name() -> i32 {
            $crate::param_specs::find_by_key($params, $key).default_i32()
        }
    };
    (@one $params:expr, $fn_name:ident, f32, $key:literal) => {
        fn $fn_name() -> f32 {
            $crate::param_specs::find_by_key($params, $key).default_f32()
        }
    };
}

// ============================================================================
// Gain Plugin
// ============================================================================

// gain: Migrated to sotf-plugin-gain/src/params.rs
// Access via sotf_plugins::param_specs::gain

// compressor: Migrated to sotf-plugin-compressor/src/params.rs
// Access via sotf_plugins::param_specs::compressor

// gate: Migrated to sotf-plugin-gate/src/params.rs
// Access via sotf_plugins::param_specs::gate

// expander: Migrated to sotf-plugin-expander/src/params.rs
// Access via sotf_plugins::param_specs::expander

// limiter: Migrated to sotf-plugin-limiter/src/params.rs
// Access via sotf_plugins::param_specs::limiter

// delay: Migrated to sotf-plugin-delay/src/params.rs
// Access via sotf_plugins::param_specs::delay

// loudness_compensation: Migrated to sotf-plugin-loudness-compensation/src/params.rs
// Access via sotf_plugins::param_specs::loudness_compensation

// matrix: Migrated to sotf-plugin-matrix/src/params.rs
// Access via sotf_plugins::param_specs::matrix

// ============================================================================
// Upmixer Plugin
// ============================================================================

// upmixer: Migrated to sotf-plugin-upmixer/src/params.rs
// Access via sotf_plugins::param_specs::upmixer

// convolution: Migrated to sotf-plugin-convolution/src/params.rs
// Access via sotf_plugins::param_specs::convolution

// channel_mute_solo: Migrated to sotf-plugin-channel-mute-solo/src/params.rs
// Access via sotf_plugins::param_specs::channel_mute_solo

// hal_input: Migrated to sotf-plugin-hal-input/src/params.rs
// Access via sotf_plugins::param_specs::hal_input
// hal_output: Migrated to sotf-plugin-hal-output/src/params.rs
// Access via sotf_plugins::param_specs::hal_output

// binaural: Migrated to sotf-plugin-binaural/src/params.rs
// Access via sotf_plugins::param_specs::binaural

// ============================================================================
// Spectrum Analyzer
// ============================================================================

pub mod spectrum {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::int("Num Bins", "num_bins", 30, 8, 120, 1, "", "General")
            .structural()
            .setup()
            .doc("Number of frequency bands"),
        ParamSpec::float(
            "Min Freq", "min_freq", 20.0, 10.0, 100.0, 1.0, "Hz", "General",
        )
        .setup()
        .doc("Lowest displayed frequency"),
        ParamSpec::float(
            "Max Freq", "max_freq", 20000.0, 5000.0, 22050.0, 100.0, "Hz", "General",
        )
        .setup()
        .doc("Highest displayed frequency"),
        ParamSpec::float("Smoothing", "smoothing", 0.7, 0.0, 1.0, 0.01, "", "General")
            .setup()
            .doc("Temporal averaging factor"),
        ParamSpec::choice(
            "Tilt Correction",
            "tilt_correction",
            0,
            &["None", "3dB/oct", "6dB/oct", "Pink"],
            "General",
        )
        .setup()
        .doc("Slope compensation for display"),
        ParamSpec::choice(
            "Tilt Reference",
            "tilt_reference",
            0,
            &["Standard", "1kHz", "2kHz", "Min Freq"],
            "General",
        )
        .setup()
        .doc("Reference frequency for tilt"),
    ];
}

// eq: Migrated to sotf-plugin-eq/src/params.rs
// Access via sotf_plugins::param_specs::eq

// multiband_compressor: Migrated to sotf-plugin-multiband-compressor/src/params.rs
// Access via sotf_plugins::param_specs::multiband_compressor

// pnd: Migrated to sotf-plugin-pnd/src/params.rs
// Access via sotf_plugins::param_specs::pnd

// denoiser: Migrated to sotf-plugin-denoiser/src/params.rs
// Access via sotf_plugins::param_specs::denoiser

// ============================================================================
// Multiband Expander Plugin
// ============================================================================

// fletcher_munson: Migrated to sotf-plugin-fletcher-munson/src/params.rs
// Access via sotf_plugins::param_specs::fletcher_munson

// multiband_expander: Migrated to sotf-plugin-multiband-expander/src/params.rs
// Access via sotf_plugins::param_specs::multiband_expander

// ============================================================================
// Band Split Plugin
// ============================================================================

// mono_to_stereo: Migrated to sotf-plugin-mono-to-stereo/src/params.rs
// Access via sotf_plugins::param_specs::mono_to_stereo

// downmix: Migrated to sotf-plugin-downmix/src/params.rs
// Access via sotf_plugins::param_specs::downmix

// band_split: Migrated to sotf-plugin-band-split/src/params.rs
// Access via sotf_plugins::param_specs::band_split

// band_merge: Migrated to sotf-plugin-band-merge/src/params.rs
// Access via sotf_plugins::param_specs::band_merge

// xtc: Migrated to sotf-plugin-xtc/src/params.rs
// Access via sotf_plugins::param_specs::xtc

// ab_compare: Migrated to sotf-plugin-ab-compare/src/params.rs
// Access via sotf_plugins::param_specs::ab_compare

// crossfeed: Migrated to sotf-plugin-crossfeed/src/params.rs
// Access via sotf_plugins::param_specs::crossfeed

// ambisonics: Migrated to sotf-plugin-ambisonics/src/params.rs
// Access via sotf_plugins::param_specs::ambisonics

// aec: Migrated to sotf-plugin-aec/src/params.rs
// Access via sotf_plugins::param_specs::aec

// beamformer: Migrated to sotf-plugin-beamformer/src/params.rs
// Access via sotf_plugins::param_specs::beamformer
