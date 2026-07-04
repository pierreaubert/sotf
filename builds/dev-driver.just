# Dev-driver scenario suites.

dev_driver_gpui_features := "qa, onnx, hal, gpu-2d, gpu-3d, iamf, dev-api, streaming, hls"

# Build the GPUI binary used by GPUI suite TOMLs.
[group('dev-driver')]
dev-driver-build-gpui:
	cargo build -p sotf-gpui --bin sotf-desktop --features "{{dev_driver_gpui_features}}"

# Build the TUI binary used by the TUI suite TOML.
[group('dev-driver')]
dev-driver-build-tui:
	cargo build -p sotf-tui --features dev-api --bin sotf-tui

# Run one scenario against an already-running app with dev-api enabled.
[group('dev-driver')]
dev-driver-scenario scenario url='http://127.0.0.1:7777':
	cargo run -p sotf-dev-driver -- {{scenario}} --url {{url}} -v

# Run the GPUI smoke suite.
[group('dev-driver')]
dev-driver-smoke: dev-driver-build-gpui
	cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/smoke.toml -v

# Run the full GPUI screen/action matrix.
[group('dev-driver')]
dev-driver-full: dev-driver-build-gpui
	cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/full_matrix.toml -v

# Run the RoomEQ wizard matrix.
[group('dev-driver')]
dev-driver-roomeq: dev-driver-build-gpui
	cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/roomeq_matrix.toml -v

# Run all TUI scenarios.
[group('dev-driver')]
dev-driver-tui: dev-driver-build-tui
	cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/tui.toml -v

# Run every checked-in suite. This is intentionally long-running.
[group('dev-driver')]
dev-driver-all: dev-driver-smoke dev-driver-full dev-driver-roomeq dev-driver-tui
	@echo "All sotf-dev-driver suites passed."
