# Aggregate recipes that depend on sub-crate Justfiles.
# This file is optionally imported so Docker builds (which only copy the root
# Justfile) won't fail on unknown dependencies.

[group('bench')]
bench:
	cd ../sotf-daw && just plugins-bench

[group('download')]
download-once: download-sofa generate-audio-tests

# Engine/plugin gates live in ../sotf-daw; delegate across the workspace
# boundary, including the format, FFI, and bridge gates in imported Justfiles.
[group('qa')]
qa:
	cd ../sotf-daw && just qa-plugins
	cd ../sotf-daw && just qa-ffi
	cd ../sotf-daw && just qa-bridge
	cd ../sotf-daw && just qa-engine
