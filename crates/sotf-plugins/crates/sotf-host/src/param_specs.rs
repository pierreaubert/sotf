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

pub mod gain {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[ParamSpec::float(
        "Gain", "gain_db", 0.0, -60.0, 20.0, 0.5, "dB", "General",
    )];
}

// ============================================================================
// Compressor Plugin
// ============================================================================

pub mod compressor {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Threshold",
            "threshold",
            -20.0,
            -60.0,
            0.0,
            1.0,
            "dB",
            "Dynamics",
        ),
        ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Dynamics"),
        ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Timing"),
        ParamSpec::float(
            "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Timing",
        ),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics"),
        ParamSpec::float(
            "Makeup Gain",
            "makeup_gain",
            0.0,
            -24.0,
            24.0,
            0.5,
            "dB",
            "Output",
        )
        .output(),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
            .scaled(100.0)
            .output(),
        ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Output").output(),
        ParamSpec::bool_labeled(
            "Link Channels",
            "link_channels",
            true,
            "Linked",
            "Unlinked",
            "Channels",
        )
        .setup(),
        ParamSpec::float(
            "Sidechain HPF",
            "sidechain_hpf_hz",
            80.0,
            0.0,
            200.0,
            5.0,
            "Hz",
            "Sidechain",
        )
        .setup(),
    ];
}

// ============================================================================
// Gate Plugin
// ============================================================================

pub mod gate {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Threshold",
            "threshold",
            -40.0,
            -80.0,
            0.0,
            1.0,
            "dB",
            "Dynamics",
        ),
        ParamSpec::float("Ratio", "ratio", 10.0, 1.0, 100.0, 0.1, ":1", "Dynamics"),
        ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing"),
        ParamSpec::float("Hold", "hold", 10.0, 0.0, 1000.0, 1.0, "ms", "Timing"),
        ParamSpec::float(
            "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing",
        ),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
            .scaled(100.0)
            .output(),
        ParamSpec::bool_labeled(
            "Link Channels",
            "link_channels",
            true,
            "Linked",
            "Unlinked",
            "Channels",
        )
        .setup(),
        ParamSpec::float(
            "Sidechain HPF",
            "sidechain_hpf_hz",
            0.0,
            0.0,
            200.0,
            5.0,
            "Hz",
            "Sidechain",
        )
        .setup(),
    ];
}

// ============================================================================
// Expander Plugin
// ============================================================================

pub mod expander {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Threshold",
            "threshold",
            -40.0,
            -80.0,
            0.0,
            1.0,
            "dB",
            "Dynamics",
        ),
        ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics"),
        ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing"),
        ParamSpec::float(
            "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing",
        ),
        ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Dynamics"),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics"),
        ParamSpec::float(
            "Hysteresis",
            "hysteresis",
            4.0,
            0.0,
            12.0,
            0.1,
            "dB",
            "Dynamics",
        ),
        ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Timing"),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
            .scaled(100.0)
            .output(),
        ParamSpec::bool_labeled(
            "Link Channels",
            "link_channels",
            true,
            "Linked",
            "Unlinked",
            "Channels",
        )
        .setup(),
        ParamSpec::float(
            "Sidechain HPF",
            "sidechain_hpf_hz",
            80.0,
            0.0,
            500.0,
            5.0,
            "Hz",
            "Sidechain",
        )
        .setup(),
    ];
}

// ============================================================================
// Limiter Plugin
// ============================================================================

pub mod limiter {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Threshold",
            "threshold",
            -0.1,
            -20.0,
            0.0,
            0.1,
            "dB",
            "Dynamics",
        ),
        ParamSpec::float(
            "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Timing",
        ),
        ParamSpec::float(
            "Lookahead",
            "lookahead",
            5.0,
            0.0,
            20.0,
            0.5,
            "ms",
            "Timing",
        ),
        ParamSpec::bool_labeled("Soft Knee", "soft", false, "Soft", "Hard", "Dynamics").setup(),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "Output")
            .scaled(100.0)
            .output(),
    ];
}

// ============================================================================
// Delay Plugin
// ============================================================================

pub mod delay {}

// ============================================================================
// Loudness Compensation Plugin
// ============================================================================

pub mod loudness_compensation {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Low Freq", "low_freq", 100.0, 20.0, 500.0, 5.0, "Hz", "Low"),
        ParamSpec::float("Low Gain", "low_gain", 6.0, -20.0, 20.0, 0.5, "dB", "Low"),
        ParamSpec::float(
            "High Freq",
            "high_freq",
            10000.0,
            2000.0,
            20000.0,
            100.0,
            "Hz",
            "High",
        ),
        ParamSpec::float(
            "High Gain",
            "high_gain",
            6.0,
            -20.0,
            20.0,
            0.5,
            "dB",
            "High",
        ),
        ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain")
            .structural()
            .output(),
        ParamSpec::float(
            "Max Auto Gain",
            "auto_gain_max_db",
            12.0,
            0.0,
            24.0,
            1.0,
            "dB",
            "Auto Gain",
        )
        .structural()
        .output(),
        ParamSpec::float(
            "Smoothing",
            "auto_gain_smoothing_ms",
            100.0,
            1.0,
            1000.0,
            5.0,
            "ms",
            "Auto Gain",
        )
        .structural()
        .output(),
    ];
}

// ============================================================================
// Matrix Plugin
// ============================================================================

pub mod matrix {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[ParamSpec::float(
        "Gain", "gain", 0.0, 0.0, 1.0, 0.05, "", "Matrix",
    )];
}

// ============================================================================
// Upmixer Plugin
// ============================================================================

pub mod upmixer {
    // Surround routing parameters
    // Sub-harmonic synthesis parameters
    // Decorrelation parameters
    // Height channel parameters
    // Ambient gain boost (sqrt(1-coherence) multiplier)
    // Dialogue detection parameters
    // Dialogue detection sub-weights (centroid, variance, coherence)
    // ML vocal detection
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::choice(
            "Speaker Config",
            "speaker_config",
            2,
            &[
                "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
            ],
            "Output",
        )
        .structural()
        .setup(),
        // Gains
        ParamSpec::float(
            "Front Direct",
            "gain_front_direct",
            1.0,
            0.0,
            2.0,
            0.05,
            "x",
            "Gains",
        ),
        ParamSpec::float(
            "Front Ambient",
            "gain_front_ambient",
            0.5,
            0.0,
            2.0,
            0.05,
            "x",
            "Gains",
        ),
        ParamSpec::float(
            "Rear Ambient",
            "gain_rear_ambient",
            1.0,
            0.0,
            2.0,
            0.05,
            "x",
            "Gains",
        ),
        ParamSpec::float(
            "Height Gain",
            "height_gain",
            1.0,
            0.0,
            2.0,
            0.05,
            "x",
            "Gains",
        ),
        // LFE
        ParamSpec::float("LFE Gain", "lfe_gain", 1.0, 0.0, 2.0, 0.05, "x", "LFE")
            .secondary("LFE & Bass"),
        ParamSpec::float(
            "LFE Cutoff",
            "lfe_cutoff_hz",
            120.0,
            20.0,
            180.0,
            5.0,
            "Hz",
            "LFE",
        )
        .secondary("LFE & Bass"),
        ParamSpec::bool_param(
            "Subharmonic Synth",
            "enable_subharmonic_synth",
            false,
            "LFE",
        )
        .secondary("LFE & Bass"),
        ParamSpec::float(
            "Sub Gain",
            "subharmonic_gain",
            0.5,
            0.0,
            1.0,
            0.05,
            "x",
            "LFE",
        )
        .secondary("LFE & Bass"),
        ParamSpec::float(
            "Sub Freq",
            "subharmonic_freq_hz",
            40.0,
            20.0,
            80.0,
            1.0,
            "Hz",
            "LFE",
        )
        .secondary("LFE & Bass"),
        ParamSpec::float(
            "Sub Attack",
            "subharmonic_attack_ms",
            10.0,
            1.0,
            100.0,
            1.0,
            "ms",
            "LFE",
        )
        .secondary("LFE & Bass"),
        ParamSpec::float(
            "Sub Release",
            "subharmonic_release_ms",
            50.0,
            10.0,
            500.0,
            5.0,
            "ms",
            "LFE",
        )
        .secondary("LFE & Bass"),
        // Spatial
        ParamSpec::float(
            "Stereo Width",
            "stereo_width",
            0.5,
            0.0,
            1.0,
            0.05,
            "",
            "Spatial",
        ),
        ParamSpec::float(
            "Center Spread",
            "center_spread",
            0.0,
            0.0,
            1.0,
            0.05,
            "",
            "Spatial",
        ),
        ParamSpec::float(
            "Upmix Crossover",
            "bandpass_hz",
            250.0,
            150.0,
            350.0,
            5.0,
            "Hz",
            "Spatial",
        ),
        // Enhancement
        ParamSpec::bool_param("HR Direct", "enable_hr_direct", true, "Enhancement")
            .secondary("Enhancement"),
        ParamSpec::float(
            "HR Sharpen",
            "hr_sharpen",
            1.0,
            0.0,
            1.0,
            0.05,
            "",
            "Enhancement",
        )
        .secondary("Enhancement"),
        ParamSpec::float(
            "Ambient Boost",
            "ambient_boost",
            1.2,
            0.5,
            2.0,
            0.05,
            "x",
            "Enhancement",
        )
        .secondary("Enhancement"),
        ParamSpec::choice(
            "Decor Mode",
            "decorrelation_mode",
            0,
            &["Velvet Noise", "LFO Phase"],
            "Enhancement",
        )
        .secondary("Enhancement"),
        ParamSpec::float(
            "Decor LFO Rate",
            "decorrelation_lfo_rate_hz",
            0.15,
            0.01,
            1.0,
            0.01,
            "Hz",
            "Enhancement",
        )
        .secondary("Enhancement"),
        ParamSpec::float(
            "Velvet Duration",
            "velvet_noise_duration_ms",
            30.0,
            10.0,
            100.0,
            1.0,
            "ms",
            "Enhancement",
        )
        .secondary("Enhancement"),
        ParamSpec::float(
            "Velvet Density",
            "velvet_noise_density",
            2000.0,
            500.0,
            5000.0,
            100.0,
            "",
            "Enhancement",
        )
        .secondary("Enhancement"),
        // Height
        ParamSpec::float(
            "Height HF Cap",
            "height_hf_cap_hz",
            16000.0,
            8000.0,
            20000.0,
            100.0,
            "Hz",
            "Height",
        )
        .secondary("Height"),
        ParamSpec::float(
            "Height Trans Red",
            "height_transient_reduction",
            0.6,
            0.0,
            1.0,
            0.05,
            "",
            "Height",
        )
        .secondary("Height"),
        ParamSpec::float(
            "Height Direct Leak",
            "height_direct_leak",
            0.15,
            0.0,
            0.5,
            0.01,
            "",
            "Height",
        )
        .secondary("Height"),
        // Surround
        ParamSpec::float(
            "Surround Bleed",
            "surround_direct_bleed",
            0.5,
            0.0,
            1.0,
            0.05,
            "",
            "Surround",
        )
        .secondary("Surround"),
        ParamSpec::float(
            "Rear Amb Boost",
            "rear_ambient_boost",
            1.5,
            1.0,
            3.0,
            0.05,
            "x",
            "Surround",
        )
        .secondary("Surround"),
        ParamSpec::float(
            "Rear Late Refl",
            "rear_late_reflection",
            0.1,
            0.0,
            0.5,
            0.01,
            "",
            "Surround",
        )
        .secondary("Surround"),
        // Dialogue
        ParamSpec::float(
            "Dialogue Weight",
            "dialogue_weight",
            0.4,
            0.0,
            1.0,
            0.05,
            "",
            "Dialogue",
        )
        .secondary("Dialogue"),
        ParamSpec::float(
            "Voice Freq Min",
            "voice_freq_min_hz",
            500.0,
            200.0,
            800.0,
            10.0,
            "Hz",
            "Dialogue",
        )
        .secondary("Dialogue"),
        ParamSpec::float(
            "Voice Freq Max",
            "voice_freq_max_hz",
            3000.0,
            2000.0,
            5000.0,
            50.0,
            "Hz",
            "Dialogue",
        )
        .secondary("Dialogue"),
        ParamSpec::float(
            "Diag Centroid W",
            "dialogue_centroid_weight",
            0.3,
            0.0,
            1.0,
            0.05,
            "",
            "Dialogue",
        )
        .secondary("Dialogue"),
        ParamSpec::float(
            "Diag Variance W",
            "dialogue_variance_weight",
            0.2,
            0.0,
            1.0,
            0.05,
            "",
            "Dialogue",
        )
        .secondary("Dialogue"),
        ParamSpec::float(
            "Diag Coherence W",
            "dialogue_coherence_weight",
            0.5,
            0.0,
            1.0,
            0.05,
            "",
            "Dialogue",
        )
        .secondary("Dialogue"),
        // Output
        ParamSpec::float(
            "Safety Cap",
            "safety_cap_db",
            3.0,
            0.0,
            3.0,
            0.1,
            "dB",
            "Output",
        )
        .output(),
        // Diagnostics
        ParamSpec::bool_param("Bypass Decor", "bypass_decorrelation", false, "Diagnostics")
            .diagnostic(),
        ParamSpec::bool_param(
            "Bypass Transients",
            "bypass_transient_detection",
            false,
            "Diagnostics",
        )
        .diagnostic(),
        ParamSpec::bool_param("Bypass All", "bypass_all_processing", false, "Diagnostics")
            .diagnostic(),
        ParamSpec::bool_param("ML Detection", "enable_ml_detection", false, "Diagnostics")
            .diagnostic(),
    ];
}

// ============================================================================
// Convolution Plugin
// ============================================================================

pub mod convolution {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::file_path("IR File", "ir_file", "General").setup(),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "General").output(),
        ParamSpec::float("Gain", "gain_db", 0.0, -20.0, 20.0, 0.5, "dB", "General").output(),
    ];
}

// ============================================================================
// Channel Mute/Solo Plugin
// ============================================================================

pub mod channel_mute_solo {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] =
        &[ParamSpec::bool_param("Enabled", "enabled", true, "General")];
}

// ============================================================================
// HAL Input/Output Plugins
// ============================================================================

pub mod hal {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::int(
            "Input Channels",
            "input_channels",
            2,
            1,
            16,
            1,
            "ch",
            "General",
        )
        .structural(),
        ParamSpec::int(
            "Output Channels",
            "output_channels",
            2,
            1,
            16,
            1,
            "ch",
            "General",
        )
        .structural(),
    ];
}

// ============================================================================
// Binaural Plugin
// ============================================================================

pub mod binaural {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::file_path("SOFA File", "sofa_file", "General").setup(),
        ParamSpec::int(
            "Input Channels",
            "input_channels",
            2,
            2,
            16,
            1,
            "ch",
            "General",
        )
        .structural()
        .setup(),
        ParamSpec::bool_param("Optimization", "enable_optimization", true, "General")
            .structural()
            .setup(),
        ParamSpec::float(
            "Externalization",
            "externalization",
            0.0,
            0.0,
            1.0,
            0.05,
            "",
            "General",
        ),
        ParamSpec::float(
            "Near-field",
            "near_field_strength",
            0.0,
            0.0,
            1.0,
            0.05,
            "",
            "General",
        )
        .structural(),
    ];
}

// ============================================================================
// Spectrum Analyzer
// ============================================================================

pub mod spectrum {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::int("Num Bins", "num_bins", 30, 8, 120, 1, "", "General")
            .structural()
            .setup(),
        ParamSpec::float(
            "Min Freq", "min_freq", 20.0, 10.0, 100.0, 1.0, "Hz", "General",
        )
        .setup(),
        ParamSpec::float(
            "Max Freq", "max_freq", 20000.0, 5000.0, 22050.0, 100.0, "Hz", "General",
        )
        .setup(),
        ParamSpec::float("Smoothing", "smoothing", 0.7, 0.0, 1.0, 0.01, "", "General").setup(),
        ParamSpec::choice(
            "Tilt Correction",
            "tilt_correction",
            0,
            &["None", "3dB/oct", "6dB/oct", "Pink"],
            "General",
        )
        .setup(),
        ParamSpec::choice(
            "Tilt Reference",
            "tilt_reference",
            0,
            &["Standard", "1kHz", "2kHz", "Min Freq"],
            "General",
        )
        .setup(),
    ];
}

// ============================================================================
// EQ Plugin
// ============================================================================

pub mod eq {
    // EQ filters are dynamic, but we can define common ranges for filter parameters
    use super::ParamSpec;
    /// Global params before the per-filter params.
    pub const GLOBAL_PARAMS: &[ParamSpec] =
        &[ParamSpec::int("Max Filters", "max_filters", 20, 1, 20, 1, "", "Global").structural()];
    /// Template for each filter band (repeated per filter).
    pub const BAND_TEMPLATE: &[ParamSpec] = &[
        ParamSpec::float(
            "Frequency",
            "frequency",
            1000.0,
            20.0,
            20000.0,
            10.0,
            "Hz",
            "Filter",
        ),
        ParamSpec::float("Q", "q", 1.0, 0.1, 10.0, 0.05, "", "Filter"),
        ParamSpec::float("Gain", "gain_db", 0.0, -24.0, 24.0, 0.5, "dB", "Filter"),
        ParamSpec::choice(
            "Type",
            "filter_type",
            0,
            &[
                "Peak",
                "Lowshelf",
                "Highshelf",
                "Lowpass",
                "Highpass",
                "Bandpass",
                "Notch",
            ],
            "Filter",
        ),
    ];
}

// ============================================================================
// Multiband Compressor Plugin
// ============================================================================

pub mod multiband_compressor {
    // Number of bands
    // Crossover preset: 0=Custom, 1=200/2k, 2=100/3k, 3=250/4k
    // Crossover frequencies (Hz)
    // Global compression parameters (same as compressor)
    // Per-band flags
    use super::ParamSpec;
    /// Global params for multiband compressor.
    pub const GLOBAL_PARAMS: &[ParamSpec] = &[
        ParamSpec::int("Bands", "num_bands", 3, 2, 5, 1, "", "Global")
            .structural()
            .setup(),
        ParamSpec::int("Preset", "crossover_preset", 1, 0, 3, 1, "", "Global")
            .structural()
            .setup(),
        ParamSpec::float(
            "Crossover 1",
            "crossover_freq_1",
            200.0,
            20.0,
            500.0,
            10.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 2",
            "crossover_freq_2",
            2000.0,
            500.0,
            5000.0,
            50.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 3",
            "crossover_freq_3",
            8000.0,
            5000.0,
            15000.0,
            100.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 4",
            "crossover_freq_4",
            12000.0,
            10000.0,
            18000.0,
            100.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Threshold",
            "threshold",
            -20.0,
            -60.0,
            0.0,
            1.0,
            "dB",
            "Global",
        ),
        ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Global"),
        ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Global"),
        ParamSpec::float(
            "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Global",
        ),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Global"),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Global")
            .scaled(100.0)
            .output(),
        ParamSpec::bool_labeled(
            "Link Channels",
            "link_channels",
            true,
            "Linked",
            "Unlinked",
            "Global",
        )
        .setup(),
    ];
    /// Template for each compressor band (repeated per band).
    pub const BAND_TEMPLATE: &[ParamSpec] = &[
        ParamSpec::bool_param("Solo", "solo", false, "Band"),
        ParamSpec::bool_param("Bypass", "bypass", false, "Band"),
        ParamSpec::float(
            "Threshold",
            "threshold",
            -20.0,
            -60.0,
            0.0,
            1.0,
            "dB",
            "Band",
        ),
        ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Band"),
        ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Band"),
        ParamSpec::float("Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Band"),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Band"),
        ParamSpec::float(
            "Makeup Gain",
            "makeup_gain",
            0.0,
            -24.0,
            24.0,
            0.5,
            "dB",
            "Band",
        ),
    ];
}

// ============================================================================
// Polyphonic Note Detection (PND) & Varispeed Plugin
// ============================================================================

pub mod pnd {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Correction",
            "correction_strength",
            1.0,
            0.0,
            2.0,
            0.05,
            "",
            "General",
        )
        .scaled(100.0),
        ParamSpec::float(
            "Analysis Window",
            "analysis_window_ms",
            100.0,
            20.0,
            500.0,
            5.0,
            "ms",
            "General",
        ),
        ParamSpec::float(
            "Drift Smoothing",
            "drift_smoothing",
            0.1,
            0.001,
            1.0,
            0.001,
            "",
            "General",
        )
        .scaled(1000.0),
    ];
}

// ============================================================================
// Denoiser Plugin
// ============================================================================

pub mod denoiser {
    // MCRA-specific parameters (advanced/expert use)
    // Psychoacoustic masking
    // Noise profile capture
    pub const LEARN_FRAMES: usize = 50; // ~1s at typical hop rates

    // Transparency: blend computed gain toward 1.0 (0 = full denoising, 1 = pass-through)
    // Decision-Directed SNR estimation
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Reduction",
            "reduction_db",
            10.0,
            0.0,
            40.0,
            0.5,
            "dB",
            "General",
        ),
        ParamSpec::float(
            "Floor", "floor_db", -20.0, -60.0, -10.0, 0.5, "dB", "General",
        ),
        ParamSpec::float(
            "Smoothing",
            "smoothing",
            0.3,
            0.0,
            0.99,
            0.01,
            "",
            "General",
        )
        .scaled(100.0),
        ParamSpec::float("Attack", "attack_ms", 5.0, 0.1, 100.0, 0.5, "ms", "Timing"),
        ParamSpec::float(
            "Release",
            "release_ms",
            50.0,
            10.0,
            500.0,
            5.0,
            "ms",
            "Timing",
        ),
        ParamSpec::bool_param("Low Latency", "low_latency", false, "General")
            .structural()
            .setup(),
        ParamSpec::bool_param("Polyphonic", "polyphonic_detection", false, "Analysis")
            .secondary("Analysis"),
        ParamSpec::float(
            "Crack Sens.",
            "crack_sensitivity",
            10.0,
            1.0,
            100.0,
            1.0,
            "",
            "Analysis",
        )
        .secondary("Analysis"),
        ParamSpec::float(
            "MCRA Alpha S",
            "mcra_alpha_s",
            0.9,
            0.5,
            0.99,
            0.01,
            "",
            "Advanced",
        )
        .secondary("MCRA"),
        ParamSpec::float(
            "MCRA Alpha P",
            "mcra_alpha_p",
            0.7,
            0.1,
            0.99,
            0.01,
            "",
            "Advanced",
        )
        .secondary("MCRA"),
        ParamSpec::int("MCRA Window", "mcra_l", 50, 10, 200, 1, "fr", "Advanced").secondary("MCRA"),
        ParamSpec::float(
            "MCRA Delta",
            "mcra_delta",
            5.0,
            1.0,
            20.0,
            0.5,
            "",
            "Advanced",
        )
        .secondary("MCRA"),
        ParamSpec::float(
            "Transparency",
            "transparency",
            0.0,
            0.0,
            1.0,
            0.01,
            "",
            "General",
        )
        .scaled(100.0),
        ParamSpec::bool_param("DD SNR", "dd_enabled", true, "Analysis").secondary("Analysis"),
        ParamSpec::float(
            "DD Alpha", "dd_alpha", 0.98, 0.5, 0.999, 0.001, "", "Analysis",
        )
        .secondary("Analysis"),
        ParamSpec::bool_param("Psychoacoustic", "psychoacoustic_masking", true, "Analysis")
            .secondary("Analysis"),
        ParamSpec::bool_param("Transient", "transient_enabled", true, "Analysis")
            .secondary("Analysis"),
        ParamSpec::bool_param(
            "Spectral Smooth",
            "spectral_smoothing_enabled",
            true,
            "Analysis",
        )
        .secondary("Analysis"),
        ParamSpec::bool_param(
            "Temporal Smooth",
            "temporal_smoothing_enabled",
            true,
            "Analysis",
        )
        .secondary("Analysis"),
        ParamSpec::bool_param("Hiss Remover", "hiss_enabled", false, "Hiss").secondary("Hiss"),
        ParamSpec::float(
            "Hiss Threshold",
            "hiss_threshold_db",
            -30.0,
            -60.0,
            -10.0,
            0.5,
            "dB",
            "Hiss",
        )
        .secondary("Hiss"),
        ParamSpec::float(
            "Hiss Frequency",
            "hiss_frequency_hz",
            4000.0,
            1000.0,
            16000.0,
            100.0,
            "Hz",
            "Hiss",
        )
        .secondary("Hiss"),
        ParamSpec::float(
            "Hiss Strength",
            "hiss_strength",
            0.5,
            0.0,
            1.0,
            0.01,
            "",
            "Hiss",
        )
        .scaled(100.0)
        .secondary("Hiss"),
        ParamSpec::bool_param(
            "Spectral Sub",
            "spectral_sub_enabled",
            false,
            "Spectral Sub",
        )
        .secondary("Spectral Sub"),
        ParamSpec::float(
            "Oversub Factor",
            "spectral_sub_alpha",
            2.0,
            0.5,
            6.0,
            0.1,
            "",
            "Spectral Sub",
        )
        .secondary("Spectral Sub"),
        ParamSpec::float(
            "Spectral Floor",
            "spectral_sub_beta",
            0.02,
            0.001,
            0.5,
            0.001,
            "",
            "Spectral Sub",
        )
        .secondary("Spectral Sub"),
        ParamSpec::bool_labeled(
            "Learn Noise",
            "learn_noise",
            false,
            "Active",
            "Off",
            "Noise Profile",
        )
        .structural()
        .secondary("Noise Profile"),
        ParamSpec::bool_param(
            "Use Profile",
            "use_captured_profile",
            false,
            "Noise Profile",
        )
        .secondary("Noise Profile"),
        ParamSpec::bool_labeled(
            "Clear Profile",
            "clear_profile",
            false,
            "Trigger",
            "Off",
            "Noise Profile",
        )
        .structural()
        .secondary("Noise Profile"),
    ];
}

// ============================================================================
// Multiband Expander Plugin
// ============================================================================

// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================

pub mod fletcher_munson {
    // Playback volume (set by engine/UI when master volume changes)
    // Reference level where response is flat (corresponds to ~80 dB SPL)
    // Smoothing time for gain transitions (ms)
    // Band frequency ranges
    // Band Q ranges
    // Band max gain ranges
    // Band slope ranges (dB gain per dB volume delta)
    // Band 1: Sub-bass (~60 Hz) - ISO 226 shows largest compensation needed here
    // Band 2: Mid-bass (~250 Hz) - moderate compensation
    // Band 3: Presence (~3.5 kHz) - small boost (ear most sensitive here)
    // Band 4: Air/brilliance (~12 kHz) - treble compensation
    // Enabled default
    // Auto-gain parameters
    // 0 = Momentary (400ms), 1 = ShortTerm (3s)
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Playback Volume",
            "playback_volume_db",
            0.0,
            -80.0,
            0.0,
            0.5,
            "dB",
            "Global",
        )
        .setup(),
        ParamSpec::float(
            "Reference",
            "reference_level_db",
            -14.0,
            -40.0,
            0.0,
            0.5,
            "dB",
            "Global",
        )
        .setup(),
        ParamSpec::bool_param("Enabled", "enabled", true, "Global").setup(),
        ParamSpec::float(
            "Smoothing",
            "smoothing_ms",
            30.0,
            1.0,
            200.0,
            1.0,
            "ms",
            "Global",
        )
        .setup(),
        ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain").output(),
        ParamSpec::float(
            "Max Correction",
            "auto_gain_max_db",
            12.0,
            0.0,
            24.0,
            1.0,
            "dB",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "AG Smoothing",
            "auto_gain_smoothing_ms",
            100.0,
            10.0,
            500.0,
            5.0,
            "ms",
            "Auto Gain",
        )
        .output(),
        ParamSpec::choice(
            "AG Loudness Type",
            "auto_gain_loudness_type",
            0,
            &["Momentary", "ShortTerm"],
            "Auto Gain",
        )
        .output(),
        // Band 1
        ParamSpec::float(
            "Band 1 Freq",
            "band1_freq",
            60.0,
            20.0,
            20000.0,
            5.0,
            "Hz",
            "Band 1",
        ),
        ParamSpec::float("Band 1 Q", "band1_q", 0.5, 0.1, 10.0, 0.05, "", "Band 1"),
        ParamSpec::float(
            "Band 1 Max",
            "band1_max_gain",
            15.0,
            0.0,
            24.0,
            0.5,
            "dB",
            "Band 1",
        ),
        ParamSpec::float(
            "Band 1 Slope",
            "band1_slope",
            0.6,
            0.0,
            1.0,
            0.01,
            "",
            "Band 1",
        ),
        // Band 2
        ParamSpec::float(
            "Band 2 Freq",
            "band2_freq",
            250.0,
            20.0,
            20000.0,
            10.0,
            "Hz",
            "Band 2",
        ),
        ParamSpec::float("Band 2 Q", "band2_q", 0.707, 0.1, 10.0, 0.05, "", "Band 2"),
        ParamSpec::float(
            "Band 2 Max",
            "band2_max_gain",
            8.0,
            0.0,
            24.0,
            0.5,
            "dB",
            "Band 2",
        ),
        ParamSpec::float(
            "Band 2 Slope",
            "band2_slope",
            0.4,
            0.0,
            1.0,
            0.01,
            "",
            "Band 2",
        ),
        // Band 3
        ParamSpec::float(
            "Band 3 Freq",
            "band3_freq",
            3500.0,
            20.0,
            20000.0,
            50.0,
            "Hz",
            "Band 3",
        ),
        ParamSpec::float("Band 3 Q", "band3_q", 1.0, 0.1, 10.0, 0.05, "", "Band 3"),
        ParamSpec::float(
            "Band 3 Max",
            "band3_max_gain",
            4.0,
            0.0,
            24.0,
            0.5,
            "dB",
            "Band 3",
        ),
        ParamSpec::float(
            "Band 3 Slope",
            "band3_slope",
            0.2,
            0.0,
            1.0,
            0.01,
            "",
            "Band 3",
        ),
        // Band 4
        ParamSpec::float(
            "Band 4 Freq",
            "band4_freq",
            12000.0,
            20.0,
            20000.0,
            100.0,
            "Hz",
            "Band 4",
        ),
        ParamSpec::float("Band 4 Q", "band4_q", 0.707, 0.1, 10.0, 0.05, "", "Band 4"),
        ParamSpec::float(
            "Band 4 Max",
            "band4_max_gain",
            6.0,
            0.0,
            24.0,
            0.5,
            "dB",
            "Band 4",
        ),
        ParamSpec::float(
            "Band 4 Slope",
            "band4_slope",
            0.3,
            0.0,
            1.0,
            0.01,
            "",
            "Band 4",
        ),
    ];
}

pub mod multiband_expander {
    // Number of bands (same as multiband compressor)
    // Crossover preset: 0=Custom, 1=200/2k, 2=100/3k, 3=250/4k
    // Crossover frequencies (Hz) - same as multiband compressor
    // Global expansion parameters (same as expander)
    // Per-band flags
    use super::ParamSpec;
    /// Global params for multiband expander.
    pub const GLOBAL_PARAMS: &[ParamSpec] = &[
        ParamSpec::int("Bands", "num_bands", 3, 2, 5, 1, "", "Global")
            .structural()
            .setup(),
        ParamSpec::int("Preset", "crossover_preset", 1, 0, 3, 1, "", "Global")
            .structural()
            .setup(),
        ParamSpec::float(
            "Crossover 1",
            "crossover_freq_1",
            200.0,
            20.0,
            500.0,
            10.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 2",
            "crossover_freq_2",
            2000.0,
            500.0,
            5000.0,
            50.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 3",
            "crossover_freq_3",
            8000.0,
            5000.0,
            15000.0,
            100.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Crossover 4",
            "crossover_freq_4",
            12000.0,
            10000.0,
            18000.0,
            100.0,
            "Hz",
            "Global",
        )
        .structural()
        .setup(),
        ParamSpec::float(
            "Threshold",
            "threshold",
            -40.0,
            -80.0,
            0.0,
            1.0,
            "dB",
            "Global",
        ),
        ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Global"),
        ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Global"),
        ParamSpec::float(
            "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Global",
        ),
        ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Global"),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Global"),
        ParamSpec::float(
            "Hysteresis",
            "hysteresis",
            4.0,
            0.0,
            12.0,
            0.1,
            "dB",
            "Global",
        ),
        ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Global"),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Global")
            .scaled(100.0)
            .output(),
        ParamSpec::bool_labeled(
            "Link Channels",
            "link_channels",
            true,
            "Linked",
            "Unlinked",
            "Global",
        )
        .setup(),
    ];
    /// Template for each expander band (repeated per band).
    pub const BAND_TEMPLATE: &[ParamSpec] = &[
        ParamSpec::bool_param("Solo", "solo", false, "Band"),
        ParamSpec::bool_param("Bypass", "bypass", false, "Band"),
        ParamSpec::float(
            "Threshold",
            "threshold",
            -40.0,
            -80.0,
            0.0,
            1.0,
            "dB",
            "Band",
        ),
        ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Band"),
        ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Band"),
        ParamSpec::float("Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Band"),
        ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Band"),
        ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Band"),
        ParamSpec::float(
            "Hysteresis",
            "hysteresis",
            4.0,
            0.0,
            12.0,
            0.1,
            "dB",
            "Band",
        ),
        ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Band"),
    ];
}

// ============================================================================
// Band Split Plugin
// ============================================================================

// ============================================================================
// Mono to Stereo Plugin
// ============================================================================

pub mod mono_to_stereo {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Width", "stereo_width", 0.5, 0.0, 1.0, 0.05, "", "General"),
        ParamSpec::float(
            "Haas Delay",
            "haas_delay_ms",
            1.5,
            0.0,
            5.0,
            0.1,
            "ms",
            "General",
        ),
        ParamSpec::bool_param("Comp EQ", "enable_comp_eq", true, "EQ").setup(),
        ParamSpec::float(
            "Comp EQ Depth",
            "comp_eq_depth_db",
            1.0,
            0.0,
            3.0,
            0.1,
            "dB",
            "EQ",
        ),
        ParamSpec::float(
            "Decor Low",
            "decor_low_hz",
            300.0,
            100.0,
            500.0,
            10.0,
            "Hz",
            "General",
        ),
        ParamSpec::float(
            "Decor High",
            "decor_high_hz",
            2000.0,
            1000.0,
            5000.0,
            10.0,
            "Hz",
            "General",
        ),
    ];
}

// ============================================================================
// Downmix Plugin
// ============================================================================

pub mod downmix {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Center Gain",
            "center_gain_db",
            -3.0,
            -12.0,
            0.0,
            0.5,
            "dB",
            "Gains",
        ),
        ParamSpec::float(
            "Surround Gain",
            "surround_gain_db",
            -3.0,
            -12.0,
            0.0,
            0.5,
            "dB",
            "Gains",
        ),
        ParamSpec::float(
            "Height Gain",
            "height_gain_db",
            -6.0,
            -60.0,
            0.0,
            0.5,
            "dB",
            "Gains",
        ),
        ParamSpec::float(
            "LFE Gain",
            "lfe_gain_db",
            -10.0,
            -60.0,
            0.0,
            0.5,
            "dB",
            "Gains",
        ),
        ParamSpec::bool_param("Phase Coherence", "phase_coherence", true, "Phase"),
        ParamSpec::float(
            "Phase Blend Low",
            "phase_blend_low_hz",
            500.0,
            100.0,
            1000.0,
            10.0,
            "Hz",
            "Phase",
        ),
        ParamSpec::float(
            "Phase Blend High",
            "phase_blend_high_hz",
            2000.0,
            1000.0,
            5000.0,
            10.0,
            "Hz",
            "Phase",
        ),
    ];
}

// ============================================================================
// Band Split Plugin
// ============================================================================

pub mod band_split {
    /// Crossover frequency in Hz
    /// Crossover type: "LR24" (24 dB/oct) or "LR48" (48 dB/oct)
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float(
            "Frequency",
            "frequency",
            300.0,
            20.0,
            20000.0,
            10.0,
            "Hz",
            "General",
        )
        .structural(),
        ParamSpec::choice("Type", "type", 0, &["LR24", "LR48"], "General").structural(),
    ];
}

// ============================================================================
// Band Merge Plugin
// ============================================================================

pub mod band_merge {
    /// Number of bands to merge
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] =
        &[ParamSpec::int("Bands", "bands", 2, 2, 8, 1, "", "General").structural()];
}

// ============================================================================
// XTC (Crosstalk Cancellation) Plugin
// ============================================================================

pub mod xtc {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        // Geometry
        ParamSpec::float(
            "Distance",
            "distance_m",
            2.0,
            0.5,
            10.0,
            0.05,
            "m",
            "Geometry",
        ),
        ParamSpec::float(
            "Speaker Angle",
            "speaker_angle_deg",
            30.0,
            10.0,
            90.0,
            0.5,
            "\u{00b0}",
            "Geometry",
        ),
        ParamSpec::float(
            "Head Radius",
            "head_radius_m",
            0.0875,
            0.05,
            0.12,
            0.001,
            "m",
            "Geometry",
        )
        .scaled(100.0),
        // Head Tracking
        ParamSpec::float(
            "Head Offset X",
            "head_offset_x",
            0.0,
            -0.5,
            0.5,
            0.01,
            "m",
            "Head Tracking",
        ),
        ParamSpec::float(
            "Head Offset Z",
            "head_offset_z",
            0.0,
            -0.5,
            0.5,
            0.01,
            "m",
            "Head Tracking",
        ),
        ParamSpec::float(
            "Head Yaw",
            "head_yaw_deg",
            0.0,
            -90.0,
            90.0,
            1.0,
            "\u{00b0}",
            "Head Tracking",
        ),
        ParamSpec::float(
            "Head Tracking Smooth",
            "head_tracking_smooth_s",
            0.1,
            0.0,
            1.0,
            0.01,
            "s",
            "Head Tracking",
        ),
        // Beta
        ParamSpec::float(
            "Beta Base",
            "beta_base",
            0.001,
            0.0001,
            0.1,
            0.001,
            "",
            "Beta",
        )
        .scaled(1000.0),
        ParamSpec::float(
            "Beta Low Boost",
            "beta_low_freq_boost",
            10.0,
            0.0,
            30.0,
            0.5,
            "",
            "Beta",
        ),
        ParamSpec::float(
            "Beta High Boost",
            "beta_high_freq_boost",
            10.0,
            0.0,
            30.0,
            0.5,
            "",
            "Beta",
        ),
        // Shadow
        ParamSpec::float(
            "Shadow Cutoff",
            "head_shadow_cutoff_hz",
            4000.0,
            1000.0,
            10000.0,
            50.0,
            "Hz",
            "Shadow",
        ),
        ParamSpec::float(
            "Shadow Slope",
            "head_shadow_slope_db_per_octave",
            6.0,
            0.0,
            12.0,
            0.5,
            "dB/oct",
            "Shadow",
        ),
        // Filter
        ParamSpec::float(
            "Max Gain",
            "max_gain_db",
            12.0,
            3.0,
            30.0,
            1.0,
            "dB",
            "Filter",
        ),
        // Advanced
        ParamSpec::bool_param("Spectral Norm", "spectral_normalization", true, "Advanced"),
        ParamSpec::bool_param("Pinna Model", "pinna_model_enabled", false, "Advanced"),
        // Room
        ParamSpec::bool_param(
            "Room Reflections",
            "room_reflections_enabled",
            false,
            "Room",
        ),
        ParamSpec::float(
            "Room Width",
            "room_width_m",
            4.0,
            2.0,
            10.0,
            0.1,
            "m",
            "Room",
        ),
        ParamSpec::float(
            "Room Depth",
            "room_depth_m",
            5.0,
            2.0,
            15.0,
            0.1,
            "m",
            "Room",
        ),
        ParamSpec::float(
            "Wall Absorption",
            "wall_absorption",
            0.3,
            0.0,
            1.0,
            0.05,
            "",
            "Room",
        ),
        ParamSpec::float(
            "Reflection Beta",
            "reflection_beta_boost",
            3.0,
            1.0,
            10.0,
            0.1,
            "",
            "Room",
        ),
        // Diagnostic
        ParamSpec::bool_param(
            "Bypass XTC Filters",
            "bypass_xtc_filters",
            false,
            "Diagnostic",
        )
        .diagnostic(),
        ParamSpec::bool_param(
            "Bypass Spectral Norm",
            "bypass_spectral_normalization",
            false,
            "Diagnostic",
        )
        .diagnostic(),
        ParamSpec::bool_param(
            "Bypass Neumann",
            "bypass_neumann_refinement",
            false,
            "Diagnostic",
        )
        .diagnostic(),
        // Auto Gain
        ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", true, "Auto Gain").output(),
        ParamSpec::float(
            "AG Max",
            "auto_gain_max_db",
            12.0,
            0.0,
            24.0,
            1.0,
            "dB",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "AG Smoothing",
            "auto_gain_smoothing_ms",
            100.0,
            10.0,
            500.0,
            5.0,
            "ms",
            "Auto Gain",
        )
        .output(),
    ];
}

// ============================================================================
// AB Compare Plugin
// ============================================================================

pub mod ab_compare {
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Mix (A/B)", "mix", 0.0, -1.0, 1.0, 0.05, "", "Mix").scaled(100.0),
        ParamSpec::choice("Mix Mode", "mix_mode", 0, &["Pot", "Binary"], "Mix"),
        ParamSpec::choice("Selected Path", "selected_path", 0, &["A", "B"], "Mix"),
        ParamSpec::bool_labeled("Bypass", "bypass", false, "Yes", "No", "Mix"),
        ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", true, "Auto Gain").output(),
        ParamSpec::choice(
            "Loudness Type",
            "loudness_type",
            0,
            &["Momentary", "ShortTerm"],
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "Max Auto Gain",
            "max_auto_gain_db",
            12.0,
            0.0,
            24.0,
            1.0,
            "dB",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "Gain Smoothing",
            "gain_smoothing_ms",
            100.0,
            1.0,
            500.0,
            5.0,
            "ms",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "Mix Transition",
            "mix_transition_ms",
            50.0,
            1.0,
            500.0,
            5.0,
            "ms",
            "Mix",
        ),
        ParamSpec::file_path("Path A Config", "path_a_config", "Configuration"),
        ParamSpec::file_path("Path B Config", "path_b_config", "Configuration"),
    ];
}

// ============================================================================
// Crossfeed Plugin
// ============================================================================

pub mod crossfeed {
    // Bauer mode
    // Meier mode
    // Multiband mode
    // Auto gain
    // Global
    use super::ParamSpec;
    pub const PARAMS: &[ParamSpec] = &[
        ParamSpec::choice(
            "Mode",
            "crossfeed_mode",
            0,
            &["Off", "Bauer", "Meier", "Mb"],
            "General",
        )
        .structural()
        .setup(),
        ParamSpec::choice(
            "Preset",
            "crossfeed_preset",
            0,
            &["Default", "Cmoy", "Meier", "Mb", "Off"],
            "General",
        )
        .structural()
        .setup(),
        ParamSpec::bool_param("Enabled", "enabled", true, "General").setup(),
        ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "General").output(),
        // Bauer
        ParamSpec::float(
            "Bauer Cutoff",
            "bauer_fcut_hz",
            700.0,
            400.0,
            1000.0,
            10.0,
            "Hz",
            "Bauer",
        ),
        ParamSpec::float(
            "Bauer Feed",
            "bauer_feed_db",
            4.5,
            0.0,
            15.0,
            0.5,
            "dB",
            "Bauer",
        ),
        // Meier
        ParamSpec::float(
            "Meier Level",
            "meier_level",
            30.0,
            0.0,
            100.0,
            1.0,
            "%",
            "Meier",
        ),
        // Multiband
        ParamSpec::float(
            "MB Low Freq",
            "mb_low_freq_hz",
            150.0,
            50.0,
            500.0,
            5.0,
            "Hz",
            "Multiband",
        ),
        ParamSpec::float(
            "MB Mid/High Freq",
            "mb_mid_high_freq_hz",
            5700.0,
            2000.0,
            15000.0,
            50.0,
            "Hz",
            "Multiband",
        ),
        ParamSpec::float(
            "MB Low Feed",
            "mb_low_feed_db",
            0.0,
            -20.0,
            0.0,
            0.5,
            "dB",
            "Multiband",
        ),
        ParamSpec::float(
            "MB Mid Feed",
            "mb_mid_feed_db",
            6.0,
            0.0,
            15.0,
            0.5,
            "dB",
            "Multiband",
        ),
        ParamSpec::float(
            "MB High Feed",
            "mb_high_feed_db",
            3.0,
            0.0,
            15.0,
            0.5,
            "dB",
            "Multiband",
        ),
        // Auto Gain
        ParamSpec::bool_param("Auto Gain", "autogain_enabled", false, "Auto Gain").output(),
        ParamSpec::float(
            "Target LUFS",
            "autogain_target_lufs",
            -18.0,
            -40.0,
            -12.0,
            0.5,
            "LUFS",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "Max Gain",
            "autogain_max_gain_db",
            12.0,
            0.0,
            24.0,
            1.0,
            "dB",
            "Auto Gain",
        )
        .output(),
        ParamSpec::float(
            "Smoothing",
            "autogain_smoothing_ms",
            100.0,
            10.0,
            5000.0,
            10.0,
            "ms",
            "Auto Gain",
        ),
    ];
}
