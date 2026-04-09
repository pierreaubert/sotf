//! Tests for universal argument (C-u) prefix system.

use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// universal_argument
// ---------------------------------------------------------------------------

#[test]
fn universal_argument_default_is_4() {
    let mut s = state_with("");
    s.universal_argument();
    assert_eq!(s.universal_arg, Some(4));
}

#[test]
fn universal_argument_double_is_16() {
    let mut s = state_with("");
    s.universal_argument(); // 4
    s.universal_argument(); // 4 * 4 = 16
    assert_eq!(s.universal_arg, Some(16));
}

#[test]
fn universal_argument_triple_is_64() {
    let mut s = state_with("");
    s.universal_argument(); // 4
    s.universal_argument(); // 16
    s.universal_argument(); // 64
    assert_eq!(s.universal_arg, Some(64));
}

// ---------------------------------------------------------------------------
// universal_arg_digit
// ---------------------------------------------------------------------------

#[test]
fn universal_arg_digit_replaces_default() {
    // C-u then 7 => arg=7 (replaces the default 4)
    let mut s = state_with("");
    s.universal_argument(); // 4, accumulating=true
    let consumed = s.universal_arg_digit(7);
    assert!(consumed);
    assert_eq!(s.universal_arg, Some(7));
}

#[test]
fn universal_arg_digit_builds_number() {
    // C-u then 1 then 2 => arg=12
    let mut s = state_with("");
    s.universal_argument();
    s.universal_arg_digit(1); // replaces 4 with 1
    s.universal_arg_digit(2); // 1*10 + 2 = 12
    assert_eq!(s.universal_arg, Some(12));
}

#[test]
fn universal_arg_digit_three_digits() {
    // C-u then 3 then 2 then 5 => arg=325
    let mut s = state_with("");
    s.universal_argument();
    s.universal_arg_digit(3); // replaces 4 with 3
    s.universal_arg_digit(2); // 3*10 + 2 = 32
    s.universal_arg_digit(5); // 32*10 + 5 = 325
    assert_eq!(s.universal_arg, Some(325));
}

#[test]
fn universal_arg_digit_not_consumed_without_prefix() {
    let mut s = state_with("");
    // Not accumulating, so digit should not be consumed
    let consumed = s.universal_arg_digit(5);
    assert!(!consumed);
    assert_eq!(s.universal_arg, None);
}

// ---------------------------------------------------------------------------
// take_universal_arg
// ---------------------------------------------------------------------------

#[test]
fn take_universal_arg_returns_value_and_resets() {
    let mut s = state_with("");
    s.universal_argument(); // 4
    let val = s.take_universal_arg();
    assert_eq!(val, 4);
    assert_eq!(s.universal_arg, None);
    assert!(!s.universal_arg_accumulating);
}

#[test]
fn take_universal_arg_without_prefix_returns_1() {
    let mut s = state_with("");
    let val = s.take_universal_arg();
    assert_eq!(val, 1, "take_universal_arg with no prefix should return 1");
}

#[test]
fn take_universal_arg_with_digit_override() {
    let mut s = state_with("");
    s.universal_argument();
    s.universal_arg_digit(7);
    let val = s.take_universal_arg();
    assert_eq!(val, 7);
    assert_eq!(s.universal_arg, None);
}

// ---------------------------------------------------------------------------
// cancel_universal_arg
// ---------------------------------------------------------------------------

#[test]
fn cancel_universal_arg_clears() {
    let mut s = state_with("");
    s.universal_argument();
    assert!(s.universal_arg.is_some());
    assert!(s.universal_arg_accumulating);

    s.cancel_universal_arg();
    assert_eq!(s.universal_arg, None);
    assert!(!s.universal_arg_accumulating);
}

#[test]
fn cancel_universal_arg_when_none_is_noop() {
    let mut s = state_with("");
    s.cancel_universal_arg();
    assert_eq!(s.universal_arg, None);
    assert!(!s.universal_arg_accumulating);
}
