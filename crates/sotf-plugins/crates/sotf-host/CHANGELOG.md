# Unreleased

# 0.5.92

## Features

- Added `Plugin::process_f64`, `InPlacePlugin::process_in_place_f64`, and `DawHost::process_f64` so hosts and plugins have a stable f64 processing API. Existing f32 plugins use a compatibility bridge.
- Added lock-free graph mutation handoff in `DawHost`: `take_graph_mutation_sender()` exposes a single-producer `GraphMutationSender` for queued add-node/add-plugin/add-edge/remove-plugin requests, and rebuilt `GraphTopology` snapshots are published through `ArcSwap`.
- Added a preallocated `rtrb` parameter-event queue so `DawHost::set_plugin_parameter()` hands changes to the audio block instead of mutating plugin state directly from the caller. `take_parameter_event_sender()` exposes the single-producer handle for control/UI ownership, `set_plugin_parameter_at()` / `queue_node_parameter_at()` support sample-offset events for fixed-rate f32 blocks and native f64 simple chains, and `set_plugin_parameter_immediate()` remains available for offline setup and tests.
- Added automatic host insertion of `AutoOversampledPlugin` for `Box<dyn Plugin>` values that declare `preferred_oversampling()`.
- `analyzer_channel_correlation`: new `ChannelCorrelationMonitor` maintaining a sliding-window inter-channel Pearson r matrix (400 ms EMA window). Frame-alignment safe across split `add_frames` calls; heap-allocated scratch supports arbitrary channel counts (no >32ch truncation); upper-triangle-only storage halves the memory footprint.
- `LoudnessMonitor` embeds the correlation monitor behind an opt-in `spatial_enabled` flag (default off, builder `with_spatial()`). When on, `LoudnessData.correlation_matrix` carries the row-major matrix and `correlation_samples_seen` distinguishes cold-start from settled state. Default-off keeps CLI / meter consumers free of N² compute and serialization payload.
- `plugin_layout::viz_names::SPATIAL_SPIDER` const so layouts opt into the spatial-spider custom-viz hook without stringly-typed names.
- `SpeakerPosition::to_cartesian()` / `spherical_to_cartesian()` extracted from the inline VBAP path so the spatial-spider widget can reuse the conversion.

## Performance

- Replaced real-time latency-compensation `HashMap<(NodeId, NodeId), LookaheadBuffer>` lookups with edge-indexed `Vec<Option<LookaheadBuffer>>` storage.
- Replaced process-loop automation string-key `HashMap` lookups with indexed automation slots; the map is now control-side lookup only.
- Added SIMD fast paths for contiguous merge, compensation, and multi-output summing via `scale_add_simd`.
- Enabled conservative Rayon execution for independent simple DAG stages while keeping merge, sidechain, and channel-mapped stages on the full sequential path.
- `DawHost::process_f64()` now uses a native f64 simple-chain fast path when every active plugin declares `supports_f64()`, avoiding the f32 compatibility bridge for that common case.

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
