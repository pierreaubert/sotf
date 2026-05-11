# Unreleased

## Features

- `analyzer_channel_correlation`: new `ChannelCorrelationMonitor` maintaining a sliding-window inter-channel Pearson r matrix (400 ms EMA window). Frame-alignment safe across split `add_frames` calls; heap-allocated scratch supports arbitrary channel counts (no >32ch truncation); upper-triangle-only storage halves the memory footprint.
- `LoudnessMonitor` embeds the correlation monitor behind an opt-in `spatial_enabled` flag (default off, builder `with_spatial()`). When on, `LoudnessData.correlation_matrix` carries the row-major matrix and `correlation_samples_seen` distinguishes cold-start from settled state. Default-off keeps CLI / meter consumers free of N² compute and serialization payload.
- `plugin_layout::viz_names::SPATIAL_SPIDER` const so layouts opt into the spatial-spider custom-viz hook without stringly-typed names.
- `SpeakerPosition::to_cartesian()` / `spherical_to_cartesian()` extracted from the inline VBAP path so the spatial-spider widget can reuse the conversion.

# 0.5.91

## Fixes

- Invalidate built graph topology after direct `add_node` and `add_edge` mutations so processing rebuilds stale stages and buffers before the next block.
- Size latency-compensation delay buffers to the routed channel count for channel-mapped graph edges.
- Propagate inner `process_in_place` errors from oversampled plugin wrappers.

## Performance

- Replace per-chunk oversampler residual shifting with input/output cursors and a reusable chunk buffer, reducing hot-path memory traffic for oversampled processing.

# 0.5.90

## Fixes

- Route sidechain graph edges into extended per-frame input lanes instead of dropping them during input merge.
- Compact extended `InPlacePluginAdapter` output back to audio channels after processing so sidechain lanes are not exposed downstream.
- Grow host scratch input buffers before large-block copies to avoid panics on offline render or high-channel-count blocks.
- Add regression coverage for sidechain routing through an extended in-place plugin and large input blocks.
