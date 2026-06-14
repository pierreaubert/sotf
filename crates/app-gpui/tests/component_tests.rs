//! Component Tests for GPUI App
//!
//! Tests for component logic using **real production types and functions**.
//! Since the lib has `test = false` due to GPUI macro recursion issues,
//! these tests live here as integration tests.
//!
//! All imports point to the production crate — no mirror types.

#[allow(dead_code)]
mod common;

#[path = "component_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "component_tests/tests.rs"]
mod tests;
