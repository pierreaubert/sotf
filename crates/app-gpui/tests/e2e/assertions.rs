//! Test utilities for E2E tests
//!
//! Provides helper functions and macros for common test assertions and operations.

/// Assert helper that provides better error messages
#[macro_export]
macro_rules! assert_matches {
    ($expr:expr, $pattern:pat_param) => {
        match $expr {
            $pattern => {}
            ref e => panic!(
                "Assertion failed: expected pattern {:?}, got {:?}",
                stringify!($pattern),
                e
            ),
        }
    };
    ($expr:expr, $pattern:pat_param if $guard:expr) => {
        match $expr {
            $pattern if $guard => {}
            ref e => panic!(
                "Assertion failed: expected pattern {:?} if {}, got {:?}",
                stringify!($pattern),
                stringify!($guard),
                e
            ),
        }
    };
}

/// Assert helper for Option types with custom message
#[macro_export]
macro_rules! assert_some {
    ($expr:expr) => {
        assert!($expr.is_some(), "Expected Some, got None")
    };
    ($expr:expr, $msg:expr) => {
        assert!($expr.is_some(), "Expected Some: {}", $msg)
    };
}

/// Assert helper for Result types
#[macro_export]
macro_rules! assert_ok {
    ($expr:expr) => {
        assert!($expr.is_ok(), "Expected Ok, got Err({:?})", $expr.err())
    };
    ($expr:expr, $msg:expr) => {
        assert!(
            $expr.is_ok(),
            "Expected Ok: {}, got {:?}",
            $msg,
            $expr.err()
        )
    };
}

/// Assert helper for boolean conditions
#[macro_export]
macro_rules! assert_true {
    ($expr:expr) => {
        assert!($expr, "Expected true, got false")
    };
    ($expr:expr, $msg:expr) => {
        assert!($expr, "Expected true: {}", $msg)
    };
}

/// Assert helper for equality with nice diff
#[macro_export]
macro_rules! assert_eq {
    ($left:expr, $right:expr) => {
        assert_eq!(($left), ($right), "lhs = {:?}, rhs = {:?}", $left, $right)
    };
    ($left:expr, $right:expr, $msg:expr) => {
        assert_eq!(
            ($left),
            ($right),
            "{}: lhs = {:?}, rhs = {:?}",
            $msg,
            $left,
            $right
        )
    };
}

/// Wait for a condition with timeout
///
/// Usage:
/// ```ignore
/// wait_for!(driver, condition, 5000, "condition description")
/// ```
#[macro_export]
macro_rules! wait_for {
    ($driver:expr, $condition:expr, $timeout_ms:expr, $description:expr) => {{
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis($timeout_ms);
        while !$condition() {
            if start.elapsed() > timeout {
                panic!("Timeout waiting for: {}", $description);
            }
            $driver.run_until_parked();
        }
    }};
}
