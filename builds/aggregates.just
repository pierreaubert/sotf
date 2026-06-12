# Aggregate recipes that depend on sub-crate Justfiles.
# This file is optionally imported so Docker builds (which only copy the root
# Justfile) won't fail on unknown dependencies.

[group('bench')]
bench: plugins-bench bench-autoeq bench-math

[group('demo')]
demo: demo-headphone-loss

[group('examples')]
examples: examples-autoeq examples-math

[group('download')]
download-once: download-sofa download-speakers generate-audio-tests generate-ml-dataset-muslan generate-ml-dataset-ava

[group('publish')]
publish: publish-autoeq publish-math

[group('qa')]
qa: qa-autoeq \
	qa-math \
	qa-roomeq \
	qa-plugins \
	qa-engine \
	qa-bridge \
	qa-ffi

