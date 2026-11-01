# --------------------------------------------------------- -*- just -*-
# How to install Just?
#	  cargo install just
# ----------------------------------------------------------------------

cargo_bin := `if command -v mbx >/dev/null 2>&1; then echo mbx; else echo cargo; fi`
# Filter only vendored warnings from Cargo's structured diagnostics on Unix.
# Direct Cargo commands and Windows recipes retain their existing output.
cargo := if os() == "windows" { cargo_bin } else {
    "python3 '" + justfile_directory() + "/../scripts/cargo/quiet-thirdparty-lints.py' " + cargo_bin
}

_default:
	just --list

import 'builds/install.just'
import 'builds/updates.just'
import 'builds/docs.just'
import 'builds/cross.just'
import 'builds/macos.just'
import 'builds/windows.just'
import 'builds/linux.just'
import 'builds/ios.just'
import 'builds/tvos.just'
import 'builds/dev-driver.just'
import 'builds/mocked-workflows.just'

import 'builds/downloads.just'
# Engine/plugin recipes moved with their crates to ../sotf-daw (see
# sotf-daw/Justfile). Run those from the sotf-daw checkout.

import 'builds/aggregates.just'

# ----------------------------------------------------------------------
# VARIABLES
# ----------------------------------------------------------------------

list_test_features := "qa, onnx, hal, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls, tidal, spotify"
list_prod_features := "onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls, tidal, spotify"

test_features := '--features="qa, onnx, hal, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls, tidal, spotify"'
release_test_features := '--features="qa, onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls, tidal, spotify"'
prod_features := '--features="onnx, hal, gpu-2d, gpu-3d, iamf, streaming, hls, tidal, spotify"'

# `sotf-dev-api/server` is intentionally unavailable in release builds.
# Keep the release TUI shortcuts on the matching feature set.
test_features_macos := release_test_features
test_features_linux := '--features="qa, onnx, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls, tidal, spotify"'
test_features_windows := '--features="qa, onnx, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls, tidal, spotify"'

# ----------------------------------------------------------------------
# TEST
# ----------------------------------------------------------------------

[group('test')]
check:
	#!/usr/bin/env bash
	set -euo pipefail
	CLANG_CACHE_DIR="${TMPDIR:-/tmp}/sotf-clang-module-cache"
	mkdir -p "$CLANG_CACHE_DIR"
	CLANG_MODULE_CACHE_PATH="$CLANG_CACHE_DIR" {{cargo}} check --workspace --lib --bins --tests --examples {{test_features}}

[group('test')]
test: prebuild-report-producer
	#!/usr/bin/env bash
	set -euo pipefail
	CLANG_CACHE_DIR="${TMPDIR:-/tmp}/sotf-clang-module-cache"
	mkdir -p "$CLANG_CACHE_DIR"
	# GPUI test apps share the process-wide dev selector registry.
	# Keep this reference run serial; nextest isolates parallel tests in processes.
	CLANG_MODULE_CACHE_PATH="$CLANG_CACHE_DIR" {{cargo}} test --workspace --lib --bins --tests --examples {{test_features}} -- --test-threads=1

[group('test')]
test-negative:
	#!/usr/bin/env bash
	set -euo pipefail
	CLANG_CACHE_DIR="${TMPDIR:-/tmp}/sotf-clang-module-cache"
	mkdir -p "$CLANG_CACHE_DIR"
	CLANG_MODULE_CACHE_PATH="$CLANG_CACHE_DIR" {{cargo}} test --test negative --release {{release_test_features}}

[group('test')]
test-proptest:
	#!/usr/bin/env bash
	set -euo pipefail
	CLANG_CACHE_DIR="${TMPDIR:-/tmp}/sotf-clang-module-cache"
	mkdir -p "$CLANG_CACHE_DIR"
	CLANG_MODULE_CACHE_PATH="$CLANG_CACHE_DIR" PROPTEST_CASES=10000 {{cargo}} test --test proptest_tests --release {{release_test_features}}

# Warm Cargo's cache for the report-producer binary that sotf-report-host
# tests spawn through autoeq's native_report.py `cargo run` fallback.
# Without this, cold checkouts exceed the tests' 60s generation timeout:
# parallel `cargo run` builds serialize on cargo's package lock while each
# test's clock runs. This must match the fallback invocation (dev profile,
# same manifest and target dir) so the cache hits.
[group('test')]
prebuild-report-producer:
	{{cargo}} build --quiet --manifest-path ../autoeq/Cargo.toml -p autoeq-report-wasm --bin report-producer

[group('test')]
ntest: prebuild-report-producer
	CARGO_PROFILE_RELEASE_LTO=off {{cargo}} nextest run --release --no-fail-fast --workspace --lib --bins --examples {{release_test_features}}

[group('test')]
itest:
	#!/usr/bin/env bash
	set -euo pipefail
	# Keep scheduler-sensitive measurements out of the highly parallel
	# workspace pass. They are run immediately below, serially, so the test
	# set remains complete without timing thresholds depending on unrelated
	# test load.
	SERIAL='test(test_play_to_audible_latency) | test(test_loudness_compensation_zero_alloc) | test(test_upmixer_plugin_timing)'
	# GPUI deliberately makes debug_selector a no-op in release builds.
	# Run the selector-dependent hover regressions in debug below, preserving
	# their assertions without adding selector bookkeeping to shipping code.
	DEBUG_UI='test(test_studio_picker_description_tooltip) | binary(studio_picker_tooltip_tests) | binary(studio_tabs_click_tests)'
	EXCLUDE="not ($SERIAL | $DEBUG_UI)"
	PROPTEST_CASES=10000 CARGO_PROFILE_RELEASE_LTO=off {{cargo}} nextest run --release --no-fail-fast --workspace --tests {{release_test_features}} -E "$EXCLUDE"
	{{cargo}} nextest run --no-fail-fast -p sotf-gpui --tests {{release_test_features}} -E "$DEBUG_UI"
	# The serial tests live in the sotf-daw workspace (sotf-engine /
	# sotf-plugins), so run that pass against its manifest; from this
	# workspace the filter matches nothing and nextest errors out.
	PROPTEST_CASES=10000 CARGO_PROFILE_RELEASE_LTO=off {{cargo}} nextest run --release --no-fail-fast --manifest-path ../sotf-daw/Cargo.toml -p sotf-engine -p sotf-plugins --features sotf-plugins/qa --tests --test-threads=1 -E "$SERIAL"

[group('test')]
atest: test-negative test-proptest ntest itest

# Fast deterministic unit tier. Keep hardware-backed tests out of this recipe.

[group('test')]
test-unit-core:
	{{cargo}} test -p sotf-player --lib
	cd ../sotf-daw && just test-unit-core

# Decoder and manager integration tests. Tests that require a virtual device
# skip through the shared testkit helper when the device is unavailable.

[group('test')]
test-integration-engine:
	cd ../sotf-daw && just test-integration-engine

[group('test')]
test-integration-player:
	{{cargo}} test -p sotf-player --test error_handling_tests
	{{cargo}} test -p sotf-player --test plugin_chain_tests
	{{cargo}} test -p sotf-player --test library_tests
	{{cargo}} test -p sotf-player --test roomeq_measured_configs

# Device-selection helpers are deterministic and do not enumerate hardware.
# Platform smoke tests live in ../sotf-systemwide (systemwide-lab recipe).

[group('test')]
test-device-fakes:
	cd ../sotf-daw && just test-device-fakes

[group('test')]
test-realtime-safety:
	cd ../sotf-daw && just test-realtime-safety

# Client/server contracts: SOTF API loopback (typed client against the real
# server), provider mocks, and MPD/DLNA/federation protocol tests. Loopback
# only; live provider tests stay env-gated per TEST-CLIENTSERVER.md.
[group('test')]
test-clientserver:
	{{cargo}} test -p sotf-player --lib server::tests
	{{cargo}} test -p sotf-player --lib sotf_api_client
	{{cargo}} test -p sotf-service-tidal
	{{cargo}} test -p sotf-service-spotify --lib
	{{cargo}} test -p sotf-services --lib
	{{cargo}} test -p sotf-federation
	{{cargo}} test -p sotf-mpd
	{{cargo}} test -p sotf-dlna --lib
	{{cargo}} test -p sotf-cast --lib
	{{cargo}} test -p sotf-tls --lib

[group('test')]
test-pr: test-unit-core test-integration-engine test-integration-player test-device-fakes test-realtime-safety test-workflow-contracts test-clientserver

# Systemwide audio lab, contract gates, and installer packaging moved to
# ../sotf-systemwide (Justfile, builds/systemwide.just, scripts/).

# ----------------------------------------------------------------------
# COVERAGE
# ----------------------------------------------------------------------

# Requires: cargo install cargo-llvm-cov
# Generates an LCOV report for CI / Codecov upload.
[group('coverage')]
coverage:
	{{cargo}} llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --lcov --output-path target/lcov.info

# Generates an HTML coverage report and opens it.
[group('coverage')]
coverage-html:
	{{cargo}} llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --html --open

# Prints a text summary to stdout (fastest coverage recipe).
[group('coverage')]
coverage-summary:
	{{cargo}} llvm-cov --workspace --lib --bins --tests --examples {{test_features}} --text --summary-only

# Core coverage report used by the coverage ratchet. Thresholds are applied
# after the baseline is recorded; this command intentionally only reports.

[group('coverage')]
coverage-core:
	mkdir -p target/coverage
	{{cargo}} llvm-cov --package sotf-player --lib --tests --json --summary-only --output-path target/coverage/sotf-player.json
	cd ../sotf-daw && just coverage-core

# Removes stale coverage artifacts.
[group('coverage')]
coverage-clean:
	{{cargo}} llvm-cov clean

# ----------------------------------------------------------------------
# LINT
# ----------------------------------------------------------------------

[group('lint')]
lint:
	{{cargo}} clippy --all {{test_features}} -- -D warnings

# Short release-mode realtime smoke gate. For a historical comparison use
# `just perf-regression baseline.csv candidate.csv`.

[group('bench')]
perf-smoke:
	mkdir -p target/perf
	{{cargo}} run --manifest-path ../sotf-daw/Cargo.toml --release -p sotf-plugins --bin daw-scale-stress -- --chain mixed --mode serial --tracks 16 --plugins 4 --blocks 256 --warmup-blocks 64 > target/perf/daw-smoke.csv
	./scripts/daw-perf-gate.sh target/perf/daw-smoke.csv

[group('bench')]
perf-regression baseline candidate tolerance='0.05':
	./scripts/perf-regression-gate.sh {{baseline}} {{candidate}} {{tolerance}}

[group('test')]
test-nightly: ntest itest test-proptest coverage-core perf-smoke

# ----------------------------------------------------------------------
# RUN
# ----------------------------------------------------------------------

# Run the GPUI player (debug mode with ad-hoc signing for macOS file dialogs).
# Debug builds enable `dev-api`, exposing the scripted-test HTTP endpoint on
# 127.0.0.1:7777 (override via SOTF_DEV_API_PORT). Release builds never include it.
[group('run')]
run-gpui:
	{{cargo}} build --bin sotf-desktop {{test_features}}
	codesign --force --deep --sign - --entitlements scripts/debug.entitlements target/debug/sotf-desktop
	./target/debug/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-release:
	{{cargo}} build --release --bin sotf-desktop {{prod_features}}
	codesign --force --deep --sign - --entitlements builds/macos/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the GPUI player (release mode)
[group('run')]
run-gpui-leaks:
	RUSTFLAGS="-C debuginfo=2" {{cargo}} build --release --bin sotf-desktop {{test_features}}
	codesign --force --deep --sign - --entitlements builds/macos/entitlements.plist target/release/sotf-desktop
	./target/release/sotf-desktop

# Run the TUI player
[group('run')]
[macos]
run-tui:
	{{cargo}} run --release --bin sotf-tui {{test_features_macos}}

[group('run')]
[linux]
run-tui:
	{{cargo}} run --release --bin sotf-tui {{test_features_linux}}

[group('run')]
[windows]
run-tui:
	{{cargo}} run --release --bin sotf-tui {{test_features_windows}}

# Run the TUI player (with debug info for leak detection)
[group('run')]
[macos]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" {{cargo}} run --release --bin sotf-tui {{test_features_macos}}

[group('run')]
[linux]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" {{cargo}} run --release --bin sotf-tui {{test_features_linux}}

[group('run')]
[windows]
run-tui-leaks:
	RUSTFLAGS="-C debuginfo=2" {{cargo}} run --release --bin sotf-tui {{test_features_windows}}

# ----------------------------------------------------------------------
# FORMAT
# ----------------------------------------------------------------------

alias format := fmt

fmt:
	{{cargo}} fmt --all

# ----------------------------------------------------------------------
# DIST — release-cut profile (fat LTO + codegen-units = 1)
# ----------------------------------------------------------------------
# Builds land in `target/dist/` (NOT `target/release/`). Compile time is
# noticeably longer than `prod-*`; only run these for actual release cuts.

# Top-level umbrella — builds everything that ships.
[group('dist')]
dist: dist-sotf-gpui dist-sotf-tui dist-sotf-recorder

[group('dist')]
dist-sotf-gpui:
	{{cargo}} build --profile dist --bin sotf-desktop -p sotf-gpui --features shipping,onnx

[group('dist')]
dist-sotf-tui:
	{{cargo}} build --profile dist --bin sotf-tui -p sotf-tui --features "onnx, streaming, hls"

[group('dist')]
dist-sotf-recorder:
	{{cargo}} build --profile dist --bin sotf-recorder-cli -p app-cli

# Whole workspace under the dist profile (slow — 10+ minutes typical).
[group('dist')]
dist-workspace:
	{{cargo}} build --profile dist --workspace

# ----------------------------------------------------------------------
# BUILD
# ----------------------------------------------------------------------

# shortcuts
[group('build')]
[macos]
tui:
	{{cargo}} run --release --bin sotf-tui -p sotf-tui {{test_features_macos}}

[group('build')]
[linux]
tui:
	{{cargo}} run --release --bin sotf-tui -p sotf-tui {{test_features_linux}}

[group('build')]
[windows]
tui:
	{{cargo}} run --release --bin sotf-tui -p sotf-tui {{test_features_windows}}

alias terminal := tui

[group('build')]
gpui:
	{{cargo}} run --release --bin sotf-desktop -p sotf-gpui {{release_test_features}}

alias desktop := gpui
alias native := gpui

# ----------------------------------------------------------------------
# CLEAN
# ----------------------------------------------------------------------

clean:
	{{cargo}} clean
	find . -name '*~' -exec rm {} \; -print
	find . -name 'Cargo.lock' -exec rm {} \; -print
