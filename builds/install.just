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
post-install: post-install-rust post-install-python

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
