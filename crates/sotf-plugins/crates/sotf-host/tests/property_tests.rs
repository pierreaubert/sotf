// ============================================================================
// Property-Based Tests for sotf-host parameter utilities
// ============================================================================
//
// Covers ParamSpec accessors, ParameterId round-trips, ParameterValue parsing,
// and clamp/adjust behavior across all ParamType variants.

use proptest::prelude::*;
use sotf_host::param_specs::ParamSpec;
use sotf_host::parameters::{ParameterId, ParameterValue};

// ============================================================================
// Fixed test specs — one representative per ParamType
// ============================================================================

const FLOAT_SPEC: ParamSpec =
    ParamSpec::float("Gain", "gain_db", -6.0, -60.0, 12.0, 0.5, "dB", "General");

const INT_SPEC: ParamSpec = ParamSpec::int("Bands", "num_bands", 3, 1, 8, 1, "", "General");

const BOOL_SPEC: ParamSpec = ParamSpec::bool_param("Bypass", "bypass", false, "General");

const CHOICE_SPEC: ParamSpec = ParamSpec::choice("Mode", "mode", 1, &["A", "B", "C"], "General");

const FILE_SPEC: ParamSpec = ParamSpec::file_path("IR", "ir_path", "General");

const TEST_SPECS: &[ParamSpec] = &[FLOAT_SPEC, INT_SPEC, BOOL_SPEC, CHOICE_SPEC, FILE_SPEC];

// ============================================================================
// Strategies
// ============================================================================

fn any_spec() -> impl Strategy<Value = &'static ParamSpec> {
    (0usize..TEST_SPECS.len()).prop_map(|i| &TEST_SPECS[i])
}

fn ascii_string() -> impl Strategy<Value = String> {
    prop::collection::vec(32u8..126u8, 0..32).prop_map(|bytes| {
        // 32..126 are printable ASCII, so this is infallible.
        String::from_utf8(bytes).unwrap()
    })
}

fn valid_bool_string() -> impl Strategy<Value = String> {
    prop_oneof![Just("true".to_string()), Just("false".to_string())]
}

fn valid_int_string() -> impl Strategy<Value = String> {
    (-10_000i32..10_000).prop_map(|i| i.to_string())
}

fn valid_float_string() -> impl Strategy<Value = String> {
    prop_oneof![
        (-1000.0f64..1000.0).prop_map(|f| format!("{:.4}", f)),
        (1e-6f64..1e6).prop_map(|f| format!("{:e}", f)),
    ]
}

fn valid_string_literal() -> impl Strategy<Value = String> {
    // Strings that are guaranteed not to parse as bool/int/float.
    prop::collection::vec(65u8..123u8, 1..16).prop_map(|bytes| {
        let core = String::from_utf8(bytes).unwrap();
        // Prefix with '[' so it can never be a Rust numeric/bool literal.
        format!("[{}]", core)
    })
}

// ============================================================================
// ParameterId & ParameterValue parsing properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// INVARIANT: ParameterId round-trips through From<&str> and as_str().
    #[test]
    fn parameter_id_roundtrip(id in ascii_string()) {
        let pid = ParameterId::from(id.as_str());
        prop_assert_eq!(pid.as_str(), id.clone());
        prop_assert_eq!(pid.to_string(), id);
    }

    /// INVARIANT: ParameterValue::parse never panics and returns a finite Float
    /// when the variant is Float.
    #[test]
    fn parse_never_panics_and_float_is_finite(input in ascii_string()) {
        let v = ParameterValue::parse(&input);
        if let ParameterValue::Float(f) = v {
            prop_assert!(f.is_finite(), "parse produced non-finite float {}", f);
        }
    }

    /// INVARIANT: parse(display(x)) round-trips for values that originated as
    /// valid typed strings.
    #[test]
    fn parse_display_roundtrip_for_valid_values(
        source in prop_oneof![
            valid_bool_string(),
            valid_int_string(),
            valid_float_string(),
            valid_string_literal(),
        ]
    ) {
        let first = ParameterValue::parse(&source);
        let display = first.to_string();
        let second = ParameterValue::parse(&display);

        // Bool, Int, and generic String should be exact. Float formatting may
        // alter precision, so only assert variant consistency.
        match (&first, &second) {
            (ParameterValue::Bool(a), ParameterValue::Bool(b)) => prop_assert_eq!(a, b),
            (ParameterValue::Int(a), ParameterValue::Int(b)) => prop_assert_eq!(a, b),
            (ParameterValue::String(a), ParameterValue::String(b)) => prop_assert_eq!(a, b),
            (ParameterValue::Float(_), ParameterValue::Float(_)) => {
                // Variant is preserved; exact textual formatting is not required.
            }
            _ => prop_assert!(
                false,
                "Variant changed across round-trip: {:?} -> {:?}",
                first, second
            ),
        }
    }

    /// INVARIANT: Bool literal strings always parse as Bool.
    #[test]
    fn parse_bool_literals(s in valid_bool_string()) {
        let v = ParameterValue::parse(&s);
        prop_assert!(
            matches!(v, ParameterValue::Bool(_)),
            "Expected Bool for '{}', got {:?}",
            s,
            v
        );
    }

    /// INVARIANT: Numeric strings without a decimal point always parse as Int
    /// (provided they are in i32 range).
    #[test]
    fn parse_int_literals(s in valid_int_string()) {
        let v = ParameterValue::parse(&s);
        prop_assert!(
            matches!(v, ParameterValue::Int(_)),
            "Expected Int for '{}', got {:?}",
            s,
            v
        );
    }

    /// INVARIANT: Numeric strings with a decimal point (or scientific notation)
    /// parse as Float.
    #[test]
    fn parse_float_literals(s in valid_float_string()) {
        // Filter out formatting that accidentally drops the decimal point.
        prop_assume!(s.contains('.') || s.to_lowercase().contains('e'));
        let v = ParameterValue::parse(&s);
        prop_assert!(
            matches!(v, ParameterValue::Float(_)),
            "Expected Float for '{}', got {:?}",
            s,
            v
        );
    }
}

// ============================================================================
// ParamSpec accessor properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// INVARIANT: default_f64() is always finite and lies within [min, max].
    #[test]
    fn default_f64_is_within_range(spec in any_spec()) {
        let d = spec.default_f64();
        prop_assert!(d.is_finite(), "default_f64() returned non-finite {}", d);
        prop_assert!(
            d >= spec.min_f64() && d <= spec.max_f64(),
            "default {} outside [{}, {}] for {:?}",
            d,
            spec.min_f64(),
            spec.max_f64(),
            spec.param_type
        );
    }

    /// INVARIANT: min_f64() is never greater than max_f64().
    #[test]
    fn min_leq_max(spec in any_spec()) {
        prop_assert!(
            spec.min_f64() <= spec.max_f64(),
            "min {} > max {} for {:?}",
            spec.min_f64(),
            spec.max_f64(),
            spec.param_type
        );
    }
}

// ============================================================================
// ParamSpec clamp_f64 properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// INVARIANT: clamp_f64 returns a finite value for any finite input.
    #[test]
    fn clamp_returns_finite(spec in any_spec(), value in -1e6f64..1e6) {
        let c = spec.clamp_f64(value);
        prop_assert!(
            c.is_finite(),
            "clamp_f64 returned non-finite {} for {:?}",
            c,
            spec.param_type
        );
    }

    /// INVARIANT: clamp_f64 returns a value inside [min_f64, max_f64] for all
    /// types except FilePath, where it is defined as an identity passthrough.
    #[test]
    fn clamp_within_range(spec in any_spec(), value in -1e6f64..1e6) {
        let c = spec.clamp_f64(value);
        match spec.param_type {
            sotf_host::param_specs::ParamType::FilePath => {
                prop_assert_eq!(c, value, "FilePath clamp should pass through");
            }
            _ => {
                prop_assert!(
                    c >= spec.min_f64() && c <= spec.max_f64(),
                    "clamp_f64({}) = {} outside [{}, {}] for {:?}",
                    value,
                    c,
                    spec.min_f64(),
                    spec.max_f64(),
                    spec.param_type
                );
            }
        }
    }

    /// INVARIANT: clamp_f64 is monotonic for ordered types (Float/Int).
    #[test]
    fn clamp_monotonic_for_ordered_types(
        spec_idx in prop_oneof![Just(0usize), Just(1usize)],
        a in -1e6f64..1e6,
        b in -1e6f64..1e6
    ) {
        let spec = &TEST_SPECS[spec_idx];
        let ca = spec.clamp_f64(a);
        let cb = spec.clamp_f64(b);
        if a <= b {
            prop_assert!(
                ca <= cb,
                "clamp not monotonic: {} <= {} but clamp({})={} > clamp({})={}",
                a, b, a, ca, b, cb
            );
        }
    }

    /// INVARIANT: clamping an in-range Float returns the same value.
    #[test]
    fn clamp_float_identity(value in -60.0f64..12.0) {
        let c = FLOAT_SPEC.clamp_f64(value);
        prop_assert!((c - value).abs() < f64::EPSILON * 100.0);
    }

    /// INVARIANT: clamping an in-range integer value for an Int spec returns
    /// the same integer as f64.
    #[test]
    fn clamp_int_identity(value in 1i64..8) {
        let f = value as f64;
        prop_assert_eq!(INT_SPEC.clamp_f64(f), f);
    }

    /// INVARIANT: clamping an in-range index for a Choice spec returns the
    /// truncated index.
    #[test]
    fn clamp_choice_identity(idx in 0usize..3) {
        let f = idx as f64;
        prop_assert_eq!(CHOICE_SPEC.clamp_f64(f), f);
    }

    /// INVARIANT: FilePath clamp is a passthrough.
    #[test]
    fn clamp_file_path_identity(value in -1e6f64..1e6) {
        prop_assert_eq!(FILE_SPEC.clamp_f64(value), value);
    }

    /// INVARIANT: Out-of-range values clamp to the nearest bound.
    #[test]
    fn clamp_hits_bounds(value in prop_oneof![
        -1e9f64..-61.0,
        13.0f64..1e9
    ]) {
        let c = FLOAT_SPEC.clamp_f64(value);
        if value < FLOAT_SPEC.min_f64() {
            prop_assert_eq!(c, FLOAT_SPEC.min_f64());
        } else {
            prop_assert_eq!(c, FLOAT_SPEC.max_f64());
        }
    }
}

// ============================================================================
// ParamSpec adjust_f64 properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// INVARIANT: adjust_f64 returns a finite value for finite inputs.
    #[test]
    fn adjust_returns_finite(
        spec in any_spec(),
        current in -1e6f64..1e6,
        delta in -100.0f64..100.0
    ) {
        let r = spec.adjust_f64(current, delta);
        prop_assert!(
            r.is_finite(),
            "adjust_f64 returned non-finite {} for {:?}",
            r,
            spec.param_type
        );
    }

    /// INVARIANT: adjust_f64 result lies within [min_f64, max_f64] for bounded
    /// numeric types (Float/Int).
    #[test]
    fn adjust_within_range_for_numeric(
        spec_idx in prop_oneof![Just(0usize), Just(1usize)],
        current in -1e6f64..1e6,
        delta in -100.0f64..100.0
    ) {
        let spec = &TEST_SPECS[spec_idx];
        let r = spec.adjust_f64(current, delta);
        prop_assert!(
            r >= spec.min_f64() && r <= spec.max_f64(),
            "adjust_f64({}, {}) = {} outside [{}, {}]",
            current,
            delta,
            r,
            spec.min_f64(),
            spec.max_f64()
        );
    }

    /// INVARIANT: adjust_f64 with delta=0 is equivalent to clamp_f64 for
    /// Float and Int parameters.
    #[test]
    fn adjust_zero_delta_equals_clamp_for_numeric(
        spec_idx in prop_oneof![Just(0usize), Just(1usize)],
        current in -1e6f64..1e6
    ) {
        let spec = &TEST_SPECS[spec_idx];
        let adjusted = spec.adjust_f64(current, 0.0);
        let clamped = spec.clamp_f64(current);
        prop_assert_eq!(adjusted, clamped);
    }

    /// INVARIANT: For Float, positive delta never decreases the value.
    #[test]
    fn adjust_float_positive_delta_non_decreasing(
        current in -1e6f64..1e6,
        delta in 0.0f64..100.0
    ) {
        let before = FLOAT_SPEC.clamp_f64(current);
        let after = FLOAT_SPEC.adjust_f64(current, delta);
        prop_assert!(
            after >= before,
            "adjust with positive delta decreased value: {} -> {}",
            before,
            after
        );
    }

    /// INVARIANT: For Int, positive delta never decreases the value.
    #[test]
    fn adjust_int_positive_delta_non_decreasing(
        current in -1e6f64..1e6,
        delta in 0.0f64..100.0
    ) {
        let before = INT_SPEC.clamp_f64(current);
        let after = INT_SPEC.adjust_f64(current, delta);
        prop_assert!(
            after >= before,
            "adjust with positive delta decreased value: {} -> {}",
            before,
            after
        );
    }

    /// INVARIANT: Bool toggles twice returns the clamped original value.
    #[test]
    fn adjust_bool_double_toggle(current in -1e6f64..1e6) {
        let first = BOOL_SPEC.adjust_f64(current, 1.0);
        let second = BOOL_SPEC.adjust_f64(first, -1.0);
        prop_assert_eq!(second, BOOL_SPEC.clamp_f64(current));
    }

    /// INVARIANT: Choice advances by whole steps and wraps around.
    #[test]
    fn adjust_choice_wraps(start in 0usize..3, steps in 1usize..20) {
        let count = 3usize;
        let current = start as f64;
        let mut cursor = current;
        for _ in 0..steps {
            cursor = CHOICE_SPEC.adjust_f64(cursor, 1.0);
        }
        let expected = ((start + steps) % count) as f64;
        prop_assert_eq!(cursor, expected);
    }

    /// INVARIANT: FilePath adjust is a passthrough of the current value.
    #[test]
    fn adjust_file_path_identity(current in -1e6f64..1e6, delta in -100.0f64..100.0) {
        prop_assert_eq!(FILE_SPEC.adjust_f64(current, delta), current);
    }
}
