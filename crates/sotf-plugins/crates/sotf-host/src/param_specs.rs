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
    (@one $params:expr, $fn_name:ident, String, $key:literal) => {
        fn $fn_name() -> String {
            $crate::param_specs::find_by_key($params, $key).default_choice_label()
        }
    };
}
/// Builds a `&[ParamSpec]` array with the 6 standard multiband crossover params
/// (bands, preset, crossover 1-4) followed by plugin-specific params.
///
/// ```ignore
/// pub const GLOBAL_PARAMS: &[ParamSpec] = multiband_global_params![
///     ParamSpec::float("Threshold", "threshold", ...),
///     // more plugin-specific params...
/// ];
/// ```
#[macro_export]
macro_rules! multiband_global_params {
    ($($extra:expr),* $(,)?) => {
        &[
            $crate::param_specs::ParamSpec::int("Bands", "num_bands", 3, 2, 5, 1, "", "Global")
                .structural()
                .setup()
                .doc("Number of frequency bands"),
            $crate::param_specs::ParamSpec::int("Preset", "crossover_preset", 1, 0, 3, 1, "", "Global")
                .structural()
                .setup()
                .doc("Crossover frequency preset"),
            $crate::param_specs::ParamSpec::float(
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
            .setup()
            .doc("Low/mid split frequency"),
            $crate::param_specs::ParamSpec::float(
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
            .setup()
            .doc("Mid/high split frequency"),
            $crate::param_specs::ParamSpec::float(
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
            .setup()
            .doc("High/air split frequency"),
            $crate::param_specs::ParamSpec::float(
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
            .setup()
            .doc("Band 4/5 split frequency"),
            $($extra),*
        ]
    };
}

mod param_spec;
mod types;

#[cfg(test)]
mod tests;

pub use param_spec::*;
pub use types::*;
