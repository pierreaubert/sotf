# Deterministic workflow contract tests; no physical capture devices.

[group('test')]
test-workflow-contracts:
    cargo test --manifest-path ../sotf-capture/Cargo.toml --features test-util --test emulated_capture
    cargo test --manifest-path ../sotf-capture/Cargo.toml --features test-util --lib recording_helpers::tests
    cargo test --manifest-path ../sotf-capture/Cargo.toml --features test-util --lib capture_session::analysis::tests
    cargo test --manifest-path ../sotf-capture/Cargo.toml --features test-util --lib signal_recorder::write::tests::csv_marks_unavailable_thd_nan_and_preserves_measured_values
    cargo test --manifest-path ../sotf-capture/Cargo.toml --features test-util --lib signal_recorder::tests::thd_warning_issue_ignores_peaks_outside_the_sweep_band
    cargo test --manifest-path ../sotf-daw/Cargo.toml -p sotf-engine --features test-util --lib test_file_output_runs_decode_dsp_and_transport_without_hardware
    {{cargo}} test -p sotf-player --lib ear_training::
    {{cargo}} test -p sotf-player --lib controllers::ab_test
    {{cargo}} test -p sotf-player --lib controllers::eq_audition
    {{cargo}} test -p sotf-player --features testing --test capture_frontend
    {{cargo}} test -p sotf-dev-driver --bin sotf-dev-driver

# Delegate bundle refresh to its single upstream owner.
[group('build')]
refresh-shared-report:
    just --justfile ../autoeq/Justfile report-dist

[group('build')]
prepare-shared-report:
    #!/usr/bin/env bash
    set -euo pipefail
    source scripts/lib/common.sh
    sotf_prepare_report_assets "$PWD"

# The loopback lifecycle test needs local socket access.
[group('test')]
test-shared-report:
    {{cargo}} test -p sotf-report-host

[group('dev-driver')]
test-recording-gpui: dev-driver-build-gpui
    cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/recording_ui.toml -v

# TUI workflows and full RoomEQ optimization run in the dev-driver suite.
[group('dev-driver')]
test-mocked-tui: dev-driver-tui

[group('dev-driver')]
test-listening-gpui: dev-driver-build-gpui
    cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/practice_stopped_ui.toml -v
    cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/practice_lifecycle_ui.toml -v
    cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/listening_abx_results_ui.toml -v
