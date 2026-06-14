//! Integration tests for app-cli binaries.
//!
//! These tests invoke the actual `player-cli` and `sotf-recorder-cli`
//! binaries and assert on their exit codes and stdout/stderr output.
//! They require the binaries to be built first (via `cargo build -p app-cli`).

#[path = "integration_tests/assert_cmd.rs"]
mod assert_cmd;

#[path = "integration_tests/misc.rs"]
mod misc;
