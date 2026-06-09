use super::param_spec::{find_by_key, index_of, ParamSpec};
use super::types::{ParamCategory, UpdateMode};

#[test]
fn float_default_f64_returns_default() {
    let spec = ParamSpec::float("Gain", "gain_db", -6.0, -60.0, 12.0, 0.1, "dB", "General");
    assert!((spec.default_f64() - (-6.0)).abs() < f64::EPSILON);
}

#[test]
fn int_default_f64_casts_to_f64() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.default_f64(), 3.0);
}

#[test]
fn bool_default_f64_returns_zero_or_one() {
    let on = ParamSpec::bool_param("Bypass", "bypass", true, "General");
    assert_eq!(on.default_f64(), 1.0);
    let off = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert_eq!(off.default_f64(), 0.0);
}

#[test]
fn choice_default_f64_returns_index() {
    let spec = ParamSpec::choice("Mode", "mode", 2, &["A", "B", "C", "D"], "General");
    assert_eq!(spec.default_f64(), 2.0);
}

#[test]
fn file_path_default_f64_returns_zero() {
    let spec = ParamSpec::file_path("IR", "ir_path", "General");
    assert_eq!(spec.default_f64(), 0.0);
}

#[test]
fn default_bool_returns_boolean_value() {
    let on = ParamSpec::bool_param("Bypass", "bypass", true, "General");
    assert!(on.default_bool());
    let off = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert!(!off.default_bool());
}

#[test]
#[should_panic(expected = "default_bool() called on non-Bool param")]
fn default_bool_panics_on_float_param() {
    let spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General");
    let _ = spec.default_bool();
}

#[test]
fn float_min_max_f64_returns_range() {
    let spec = ParamSpec::float("Freq", "freq", 1000.0, 20.0, 20000.0, 1.0, "Hz", "General");
    assert!((spec.min_f64() - 20.0).abs() < f64::EPSILON);
    assert!((spec.max_f64() - 20000.0).abs() < f64::EPSILON);
}

#[test]
fn int_min_max_f64_casts_to_f64() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.min_f64(), 1.0);
    assert_eq!(spec.max_f64(), 8.0);
}

#[test]
fn bool_min_max_f64_is_zero_and_one() {
    let spec = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert_eq!(spec.min_f64(), 0.0);
    assert_eq!(spec.max_f64(), 1.0);
}

#[test]
fn choice_max_f64_is_last_index() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &["A", "B", "C"], "General");
    assert_eq!(spec.min_f64(), 0.0);
    assert_eq!(spec.max_f64(), 2.0);
}

#[test]
fn empty_choice_max_f64_returns_zero() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &[], "General");
    assert_eq!(spec.max_f64(), 0.0);
}

#[test]
fn file_path_min_max_f64_returns_zero() {
    let spec = ParamSpec::file_path("IR", "ir_path", "General");
    assert_eq!(spec.min_f64(), 0.0);
    assert_eq!(spec.max_f64(), 0.0);
}

#[test]
fn clamp_f64_clamps_float_to_range() {
    let spec = ParamSpec::float("Gain", "gain_db", -6.0, -60.0, 12.0, 0.1, "dB", "General");
    assert!((spec.clamp_f64(-100.0) - (-60.0)).abs() < f64::EPSILON);
    assert!((spec.clamp_f64(0.0) - 0.0).abs() < f64::EPSILON);
    assert!((spec.clamp_f64(100.0) - 12.0).abs() < f64::EPSILON);
}

#[test]
fn clamp_f64_clamps_int_to_range() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.clamp_f64(-5.0), 1.0);
    assert_eq!(spec.clamp_f64(3.7), 3.0);
    assert_eq!(spec.clamp_f64(20.0), 8.0);
}

#[test]
fn clamp_f64_for_bool_thresholds_at_half() {
    let spec = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert_eq!(spec.clamp_f64(0.0), 0.0);
    assert_eq!(spec.clamp_f64(0.4), 0.0);
    assert_eq!(spec.clamp_f64(0.5), 0.0);
    assert_eq!(spec.clamp_f64(0.51), 1.0);
    assert_eq!(spec.clamp_f64(1.0), 1.0);
}

#[test]
fn clamp_f64_for_choice_clamps_to_label_count() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &["A", "B", "C"], "General");
    assert_eq!(spec.clamp_f64(-1.0), 0.0);
    assert_eq!(spec.clamp_f64(1.0), 1.0);
    assert_eq!(spec.clamp_f64(10.0), 2.0);
}

#[test]
fn clamp_f64_for_empty_choice_returns_input() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &[], "General");
    assert_eq!(spec.clamp_f64(7.0), 7.0);
}

#[test]
fn clamp_f64_for_file_path_returns_input() {
    let spec = ParamSpec::file_path("IR", "ir_path", "General");
    assert_eq!(spec.clamp_f64(42.0), 42.0);
}

#[test]
fn adjust_f64_float_applies_step_and_clamps() {
    let spec = ParamSpec::float("Gain", "gain_db", -6.0, -60.0, 12.0, 0.5, "dB", "General");
    assert!((spec.adjust_f64(-6.0, 2.0) - (-5.0)).abs() < f64::EPSILON);
    assert!((spec.adjust_f64(-6.0, -2.0) - (-7.0)).abs() < f64::EPSILON);
    assert!((spec.adjust_f64(10.0, 10.0) - 12.0).abs() < f64::EPSILON);
    assert!((spec.adjust_f64(-70.0, -10.0) - (-60.0)).abs() < f64::EPSILON);
}

#[test]
fn adjust_f64_int_uses_integer_step() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 2, "", "General");
    assert_eq!(spec.adjust_f64(3.0, 1.0), 5.0);
    assert_eq!(spec.adjust_f64(3.0, -1.0), 1.0);
}

#[test]
fn adjust_f64_bool_toggles() {
    let spec = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert_eq!(spec.adjust_f64(0.0, 1.0), 1.0);
    assert_eq!(spec.adjust_f64(1.0, -1.0), 0.0);
}

#[test]
fn adjust_f64_choice_wraps() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &["A", "B", "C"], "General");
    assert_eq!(spec.adjust_f64(0.0, 1.0), 1.0);
    assert_eq!(spec.adjust_f64(2.0, 1.0), 0.0);
    assert_eq!(spec.adjust_f64(0.0, -1.0), 2.0);
}

#[test]
fn adjust_f64_empty_choice_returns_current() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &[], "General");
    assert_eq!(spec.adjust_f64(3.0, 1.0), 3.0);
}

#[test]
fn precision_derived_from_step() {
    let p0 = ParamSpec::float("Coarse", "coarse", 0.0, 0.0, 100.0, 1.0, "", "General");
    assert_eq!(p0.precision(), 0);
    let p1 = ParamSpec::float("Medium", "medium", 0.0, 0.0, 1.0, 0.1, "", "General");
    assert_eq!(p1.precision(), 1);
    let p2 = ParamSpec::float("Fine", "fine", 0.0, 0.0, 1.0, 0.01, "", "General");
    assert_eq!(p2.precision(), 2);
    let p3 = ParamSpec::float("Finer", "finer", 0.0, 0.0, 1.0, 0.001, "", "General");
    assert_eq!(p3.precision(), 3);
    let p4 = ParamSpec::float("Ultra", "ultra", 0.0, 0.0, 1.0, 0.0001, "", "General");
    assert_eq!(p4.precision(), 4);
}

#[test]
fn non_float_precision_is_zero() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.precision(), 0);
}

#[test]
fn format_value_float_with_precision() {
    let spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General");
    assert_eq!(spec.format_value(3.5), "3.5");
}

#[test]
fn format_value_percent_unit_multiplies_by_hundred() {
    let spec = ParamSpec::float("Mix", "mix", 0.5, 0.0, 1.0, 0.01, "%", "General");
    assert_eq!(spec.format_value(0.25), "25%");
}

#[test]
fn format_value_int_as_integer() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.format_value(4.0), "4");
}

#[test]
fn format_value_bool_uses_labels() {
    // bool_labeled(name, key, default, true_label, false_label, group)
    let spec = ParamSpec::bool_labeled("Polarity", "polarity", false, "Inverted", "Normal", "General");
    assert_eq!(spec.format_value(0.0), "Normal");
    assert_eq!(spec.format_value(1.0), "Inverted");
}

#[test]
fn format_value_choice_uses_label_or_fallback() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &["A", "B", "C"], "General");
    assert_eq!(spec.format_value(1.0), "B");
    assert_eq!(spec.format_value(10.0), "10");
}

#[test]
fn format_value_file_path_is_empty() {
    let spec = ParamSpec::file_path("IR", "ir_path", "General");
    assert_eq!(spec.format_value(0.0), "");
}

#[test]
fn engine_value_string_float_has_decimal() {
    let spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General");
    assert_eq!(spec.engine_value_string(3.0), "3.0");
    assert_eq!(spec.engine_value_string(2.5), "2.5");
}

#[test]
fn engine_value_string_scientific_float_kept_as_float() {
    let spec = ParamSpec::float("Freq", "freq", 1000.0, 20.0, 20000.0, 1.0, "Hz", "General");
    let s = spec.engine_value_string(1e3);
    assert!(s.contains('e') || s.contains('E') || s == "1000.0");
}

#[test]
fn engine_value_string_int_is_integer() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.engine_value_string(4.0), "4");
}

#[test]
fn engine_value_string_bool_is_true_or_false() {
    let spec = ParamSpec::bool_param("Bypass", "bypass", false, "General");
    assert_eq!(spec.engine_value_string(1.0), "true");
    assert_eq!(spec.engine_value_string(0.0), "false");
}

#[test]
fn engine_value_string_choice_is_index() {
    let spec = ParamSpec::choice("Mode", "mode", 0, &["A", "B", "C"], "General");
    assert_eq!(spec.engine_value_string(1.0), "1");
}

#[test]
fn engine_value_string_file_path_is_empty() {
    let spec = ParamSpec::file_path("IR", "ir_path", "General");
    assert_eq!(spec.engine_value_string(0.0), "");
}

#[test]
fn choice_labels_returns_labels_or_empty() {
    let labels: &[&str] = &["A", "B", "C"];
    let spec = ParamSpec::choice("Mode", "mode", 0, labels, "General");
    assert_eq!(spec.choice_labels(), labels);

    let float_spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General");
    assert!(float_spec.choice_labels().is_empty());
}

#[test]
fn default_choice_label_returns_label_at_default_index() {
    let spec = ParamSpec::choice("Mode", "mode", 1, &["A", "B", "C"], "General");
    assert_eq!(spec.default_choice_label(), "B");
}

#[test]
#[should_panic(expected = "default_choice_label called on non-Choice param")]
fn default_choice_label_panics_on_non_choice() {
    let spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General");
    let _ = spec.default_choice_label();
}

#[test]
fn default_usize_i32_f32_cast_from_default_f64() {
    let spec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");
    assert_eq!(spec.default_usize(), 3);
    assert_eq!(spec.default_i32(), 3);
    assert!((spec.default_f32() - 3.0f32).abs() < f32::EPSILON);
}

#[test]
fn builder_methods_set_metadata() {
    let spec = ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General")
        .structural()
        .scaled(100.0)
        .setup()
        .doc("Output gain");
    assert_eq!(spec.update_mode, UpdateMode::Structural);
    assert!((spec.display_scale - 100.0).abs() < f64::EPSILON);
    assert_eq!(spec.category, ParamCategory::Setup);
    assert_eq!(spec.doc, "Output gain");

    let out = ParamSpec::float("Mix", "mix", 0.5, 0.0, 1.0, 0.01, "", "General").output();
    assert_eq!(out.category, ParamCategory::Output);

    let sec = ParamSpec::float("Depth", "depth", 0.5, 0.0, 1.0, 0.01, "", "General").secondary("Mod");
    assert_eq!(sec.category, ParamCategory::Secondary("Mod"));

    let diag = ParamSpec::bool_param("Bypass", "bypass", false, "General").diagnostic();
    assert_eq!(diag.category, ParamCategory::Diagnostic);
}

#[test]
fn index_of_finds_key_at_compile_time() {
    const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General"),
        ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General"),
        ParamSpec::bool_param("Bypass", "bypass", false, "General"),
    ];
    assert_eq!(index_of(PARAMS, "gain_db"), 0);
    assert_eq!(index_of(PARAMS, "num_bands"), 1);
    assert_eq!(index_of(PARAMS, "bypass"), 2);
}

#[test]
fn find_by_key_returns_matching_spec() {
    const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General"),
        ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General"),
    ];
    let spec = find_by_key(PARAMS, "num_bands");
    assert_eq!(spec.name, "Bands");
    assert_eq!(spec.default_f64(), 3.0);
}

#[test]
#[should_panic(expected = "no ParamSpec with engine_key")]
fn find_by_key_panics_when_missing() {
    const PARAMS: &[ParamSpec] = &[
        ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 12.0, 0.1, "dB", "General"),
    ];
    let _ = find_by_key(PARAMS, "missing");
}
