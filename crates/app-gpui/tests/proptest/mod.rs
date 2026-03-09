#![allow(clippy::duplicate_mod)]
//! Property-Based Tests for player-gpui
//!
//! Uses proptest to automatically generate test inputs and verify invariants
//! that should hold for all inputs.
//!
//! # Philosophy
//!
//! Property-based testing verifies invariants like:
//! - "Search results are always a subset of unfiltered results"
//! - "Volume is always clamped to [0.0, 1.0]"
//! - "Sort order is stable (same input, same output)"
//!
//! # Usage
//!
//! ```bash
//! # Run with default cases
//! cargo test -p sotf-gpui --test proptest_tests
//!
//! # Run with more cases for thorough testing
//! PROPTEST_CASES=1000 cargo test -p sotf-gpui --test proptest_tests
//! ```

mod library_properties;
mod ui_properties;
