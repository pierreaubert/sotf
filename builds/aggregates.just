# Aggregate recipes that depend on sub-crate Justfiles.
# This file is optionally imported so Docker builds (which only copy the root
# Justfile) won't fail on unknown dependencies.

[group('bench')]
bench:
	cd ../sotf-daw && just plugins-bench

[group('download')]
download-once: download-sofa generate-audio-tests

# Engine/plugin gates live in ../sotf-daw; delegate across the workspace
# boundary. (qa-bridge/qa-ffi name no recipe in any imported Justfile, so
# they are intentionally not listed: they never resolved.)
[group('qa')]
qa:
	cd ../sotf-daw && just qa-plugins
	cd ../sotf-daw && just qa-engine

