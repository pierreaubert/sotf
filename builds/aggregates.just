# Aggregate recipes that depend on sub-crate Justfiles.
# This file is optionally imported so Docker builds (which only copy the root
# Justfile) won't fail on unknown dependencies.

[group('bench')]
bench: plugins-bench

[group('download')]
download-once: download-sofa generate-audio-tests

[group('qa')]
qa: qa-plugins \
	qa-engine \
	qa-bridge \
	qa-ffi

