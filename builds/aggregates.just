# Aggregate recipes that depend on sub-crate Justfiles.
# This file is optionally imported so Docker builds (which only copy the root
# Justfile) won't fail on unknown dependencies.

[group('bench')]
bench: plugins-bench bench-autoeq bench-math

[group('demo')]
demo: demo-d3rs demo-px demo-ui-kit demo-headphone-loss

[group('examples')]
examples: examples-autoeq examples-math examples-ui-kit

[group('download')]
download-once: download-sofa download-speakers generate-audio-tests generate-ml-dataset-muslan generate-ml-dataset-ava

alias build := prod

[group('build')]
prod: prod-workspace prod-sotf-player prod-sotf-recorder prod-generate-audio-tests prod-autoeq prod-roomeq prod-math

[group('publish')]
publish: publish-autoeq publish-math

[group('qa')]
qa: qa-autoeq \
	qa-math \
	plugins-qa \
	qa-roomeq \
	qa-engine \
	qa-bridge \
	qa-ffi

