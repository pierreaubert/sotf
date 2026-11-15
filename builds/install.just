# ----------------------------------------------------------------------
# Install rustup
# ----------------------------------------------------------------------

[group('install')]
install-rustup:
	curl https://sh.rustup.rs -sSf > ./scripts/install-rustup
	chmod +x ./scripts/install-rustup
	./scripts/install-rustup -y
	~/.cargo/bin/rustup default stable
	~/.cargo/bin/cargo install just
	~/.cargo/bin/cargo install cargo-wizard
	~/.cargo/bin/cargo install cargo-llvm-cov
	~/.cargo/bin/cargo install cross
	~/.cargo/bin/cargo install cargo-binstall
	~/.cargo/bin/cargo binstall cargo-nextest --secure
	~/.cargo/bin/cargo install samply
	~/.cargo/bin/cargo install cargo-insta

# ----------------------------------------------------------------------
# POST
# ----------------------------------------------------------------------

[group('install')]
post-install: check-sibling-workspaces post-install-rust post-install-python

# SotF path-depends on sibling workspaces (../autoeq, ../sotf-daw, ...).
# Fail with the actual remedy instead of a cargo manifest error (#247).
[group('install')]
check-sibling-workspaces:
	#!/usr/bin/env bash
	set -euo pipefail
	missing=""
	for d in ../autoeq ../sotf-daw ../math-audio ../sofa-reader ../sotf-capture ../sotf-systemwide; do
		[ -f "$d/Cargo.toml" ] || missing="$missing $d"
	done
	if [ -n "$missing" ]; then
		echo "error: missing sibling workspace(s):$missing" >&2
		echo "SotF does not build from a standalone clone. Clone the aggregate repo instead:" >&2
		echo "  git clone --recurse-submodules https://github.com/pierreaubert/all_of_sotf" >&2
		echo "  cd all_of_sotf/sotf" >&2
		exit 1
	fi
	echo "sibling workspaces: ok"

[group('install')]
post-install-rust:
	$HOME/.cargo/bin/rustup default stable
	$HOME/.cargo/bin/cargo install just
	$HOME/.cargo/bin/cargo install cargo-wizard
	$HOME/.cargo/bin/cargo install cargo-vcpkg
	$HOME/.cargo/bin/cargo install cargo-llvm-cov
	$HOME/.cargo/bin/cargo install cross
	$HOME/.cargo/bin/cargo install cargo-binstall
	$HOME/.cargo/bin/cargo binstall cargo-nextest --secure
	$HOME/.cargo/bin/cargo check

[group('install')]
post-install-python:
	python3 -m venv venv
	./venv/bin/pip install -U pip
