//! Type tests for GPUI App types.
//!
//! These tests verify the behavior of types defined in the app module.
//! They are extracted from inline tests to work around GPUI macro recursion issues.

#[path = "types_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "types_tests/tests.rs"]
mod tests;
