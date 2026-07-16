# Systemwide contract/component test gates.
#
# These recipes do not install or exercise the machine-wide CoreAudio HAL
# bundle. The installed-HAL smoke tests remain a separate manual layer.

[group('test')]
test-systemwide-macos:
	./scripts/test-systemwide.sh macos

[group('test')]
test-systemwide-linux-arm64:
	#!/usr/bin/env bash
	set -euo pipefail
	image="sotf-systemwide-test-linux-arm64"
	project_root=$(pwd -P)
	docker build \
		--platform linux/arm64 \
		--progress plain \
		--tag "$image" \
		--file builds/systemwide/Dockerfile.test-linux-arm64 \
		.
	docker run --rm \
		--platform linux/arm64 \
		--volume "$project_root:/project:ro" \
		--volume sotf-systemwide-cargo-registry-linux-arm64:/usr/local/cargo/registry \
		--volume sotf-systemwide-cargo-git-linux-arm64:/usr/local/cargo/git \
		--volume sotf-systemwide-target-linux-arm64:/build-target \
		--workdir /project \
		--env CARGO_TARGET_DIR=/build-target \
		--env CARGO_INCREMENTAL=0 \
		"$image" \
		./scripts/test-systemwide.sh linux

# Deliberately sequential: prove the native macOS boundary before Linux ARM64.
[group('test')]
test-systemwide-macos-linux-arm64: test-systemwide-macos test-systemwide-linux-arm64
