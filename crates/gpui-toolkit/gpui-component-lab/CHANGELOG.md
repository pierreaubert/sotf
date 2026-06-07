# Unreleased

## Features

- Added `gpui-component-lab` with prop-driven story registry types,
  responsive preview matrices, designer story JSON persistence, and safe
  watch-mode reload support.
- Registered starter story metadata for `gpui-ui-kit`, `gpui-px`, and
  `gpui-audio-kit`.
- Added safe watch surfaces for story/token JSON reloads and supervised Rust
  source relaunches, without dynamic library loading.
- Added `MotionPreset` plus interactive motion and builder layout constraint
  controls in the lab, persisted through `*.story.json`.
- Added component-lab conformance reports for story metadata, motion coverage,
  prop labels/options, responsive viewport coverage, persisted layout
  constraints, and DesignSystem token findings, with JSON/Markdown output.
- Wired `--watch` into the running GPUI lab so story JSON and token JSON reload
  in-process while Rust source watching remains a supervised relaunch mode.
- Added renderer-backed audio-kit stories for horizontal meter bars and
  reusable spectrum frequency/dB axes, plus conformance coverage that flags
  first-party toolkit stories without a lab renderer.
- Added renderer-backed audio-kit stories for `VerticalSlider` and
  `VolumeKnob`, including live prop updates for value and mute state.
- Expanded renderer-backed lab coverage for UI-kit status/navigation/feedback
  and card compositions, plus responsive PX stories for scatter, area, heatmap,
  contour, isoline, pie/donut, boxplot, and treemap charts.
- Added full `gpui-ui-kit` showcase story coverage in the lab by registering
  every public showcase section as an embedded first-party story.
- Added `px.surface3d` story coverage with responsive fill/fixed sizing,
  colormap, wireframe, and design-aware rendering controls.
- Expanded conformance checks for rendered preview bounds, PX chart responsive
  expectations, touch target metadata, focus labels, and persisted builder
  layout fields.
- Expanded WYSIWYG layout editing beyond size constraints with persisted
  horizontal/vertical alignment, overflow, surface, gap, and border controls.
- Added public `StoryMetadataItem` support so stories carry inspector-visible,
  conformance-checked metadata in story JSON.
