# Unreleased

# 0.5.100 - 2026-08-13

## External plugin final review evidence

- Add allocation-counted warmed success, timeout, quarantine, and negotiated
  maximum-block regressions for isolated and native external-plugin paths.
- Exercise bounded MIDI/parameter IPC rings at their offset and overflow
  boundaries, and cover live isolated discovery, parameter control, audio,
  state capture, and restart-sidecar refresh through one worker transport.
- Add CPU-contention p95/p99 deadline coverage and deterministic Audio Unit
  event/transport validation; native AU/VST3 fixtures now include nonzero
  automation/MIDI offsets, transport metadata, and state restoration.

## External plugin correctness and realtime behavior

- Fail graph construction when a native backend, binary, launch, or saved-state
  restore is unavailable instead of silently running dry passthrough.
- Keep isolated shared memory mapped and preallocate worker scratch, cap waits
  below the audio block period, and remove the worker's default 1 ms idle sleep.
- Preserve reported plugin latency during runtime fallback and smooth
  processed/fallback transitions over a bounded interval.
- Stop treating filename discovery as stereo metadata; isolated IPC now rejects
  unprobed channel layouts.
- Allocate native CLAP, VST3, and Audio Unit planes from a caller-negotiated
  maximum block size instead of reserving 65,536 frames per channel.
- Version isolated IPC and add bounded MIDI/parameter event transport plus
  parameter discovery/control and native state save/load requests.
- Translate host transport, nonzero MIDI offsets, and sample-accurate parameter
  points into CLAP, VST3, and Audio Unit calls; surface CLAP host lifecycle
  requests instead of discarding them.

# 0.5.99

## Spectrum Analyzer review closure

- Replace nonlinear sample-wise channel selection with independently windowed
  per-channel FFTs and maximum per-line channel power, preserving antiphase,
  isolated, and disjoint-channel content without generating switching products.
- Publish Hann-ENBW-normalized logarithmic band power while retaining calibrated
  peak-line dBFS, explicit empty bands, DC exclusion, and Nyquist handling.
- Preallocate independent nested cache payloads and a third reset generation so
  the first FFT and reset under two held UI snapshots remain allocation-free.
- Reject processing before initialization, sanitize non-finite input for display
  safety, retain hop remainder, and make physical-time smoothing independent of
  callback partitioning for equal elapsed time.
- Publish spectrum magnitudes as `Arc<[f32]>` so the default uncorrected GPUI
  path reuses analyzer storage without a per-render Vec-to-slice allocation.
- Update both GPUI spectrum consumers to treat that payload directly as a
  slice, avoiding the unstable and redundant `slice::as_slice` call.
- Add focused multichannel, calibration, rate/range, large-block, reset,
  non-finite, smoothing, zero-copy UI, and cold real-time allocation regressions.

# 0.5.98

## Loudness Monitor review closure

- Replace callback-averaged cosine similarity with one centered, per-frame exponentially weighted Pearson accumulator shared by stereo and spatial correlation; results are invariant to callback partitioning and DC/gain offsets.
- Publish explicit measurement validity/error generation, enabled state, count-only multichannel-layout compliance, 48 kHz true-peak compliance, and the bounded one-hour integrated-history policy instead of presenting unavailable or approximate data as fully compliant measurements.
- Reset both realtime-cache generations so disable/re-enable cannot republish stale peaks, while preserving allocation-free cold processing, UI contention, reset, and enable changes.
- Add focused regressions for centered correlation, block partitioning, incomplete windows, 44.1–192 kHz true-peak scope, ambiguous 5–16-channel layouts, disable/re-enable state, and cache-contention allocation behavior.

# 0.5.97

## Bugs

- Reject parent-directory traversal in persisted external-plugin sandbox path
  grants before authorization checks.

# 0.5.96

## Fixes

- Made Spectrum Analyzer construction and initialization validate channels,
  sample rate, bin counts, finite ordered frequency bounds, smoothing, and
  Nyquist before configuration-sized allocation.
- Preserved antiphase and isolated multichannel content with signed
  maximum-power channel aggregation, retained only the latest FFT window for
  bounded display freshness, and cleared partial analysis state on reset.
- Rejected malformed interleaved buffers and post-initialization structural
  shape changes with descriptive errors instead of panics or audio-thread
  allocation.
- Published summed display-band power with explicit unavailable bins and made
  smoothing depend on elapsed audio time rather than callback cadence.
- Added Spectrum Analyzer creation to the VST3/CLAP/AU bridge for canonical and
  snake-case aliases, and made the GPUI tilt/reference controls apply their
  documented display correction.

## Performance

- Replaced the overflowing four-window FIFO with a fixed latest-window buffer,
  prepared both cache generations before structural activation, and skipped
  unchanged NIH parameter synchronization.

# 0.5.95

## Fixes

- Removed the Loudness Monitor's sample ring so wrapped and oversized
  callbacks are analyzed directly without dropping complete frames.
- Added explicit Loudness Monitor construction, initialization, sample-rate,
  checked frame-count, and exact input/output buffer validation.
- Reset and publish empty Loudness Monitor state when disabling the analyzer,
  so re-enabling begins a fresh measurement instead of reviving stale meters.
- Preserve detailed EBU R128 frame-ingestion errors instead of reducing them
  to an opaque `EBU` message.

## Performance

- Deep-initialized both Loudness Monitor cache slots, including opt-in spatial
  matrices, and reset cache payloads in place so cold processing, reset,
  disable, and the first spatial callback remain allocation-free.
- Analyze contiguous host input directly, removing redundant per-sample ring
  writes and reads from every Loudness Monitor callback.

# 0.5.93

## Features

- Derived serde serialization/deserialization for host automation modes,
  curves, and Bezier points so engine project files can persist automation.

## Fixes

- Fixed queued graph mutation node-id ordering by reserving node IDs for queued
  chain plugin appends and returning the reserved ID from `queue_add_plugin()`.

## Performance

- Preallocated parameter-event drain scratch storage to the event queue capacity
  so the first audio-block drain does not allocate.
- Reused preallocated Rayon stage scratch/result storage during graph execution
  instead of allocating a per-stage result vector in the audio path.

# 0.5.92

## Features

- Added `Plugin::process_f64`, `ParametricInPlacePlugin::process_in_place_f64`, and `DawHost::process_f64` so hosts and plugins have a stable f64 processing API. Native f64 simple chains and DAGs are used when every active plugin declares `supports_f64()`; existing f32-only plugins use a compatibility bridge.
- Added lock-free graph mutation handoff in `DawHost`: `take_graph_mutation_sender()` exposes a single-producer `GraphMutationSender` for queued add-node/add-plugin/add-edge/remove-plugin requests, and rebuilt `GraphTopology` snapshots are published through `ArcSwap`.
- Added a preallocated `rtrb` parameter-event queue so `DawHost::set_plugin_parameter()` hands changes to the audio block instead of mutating plugin state directly from the caller. `take_parameter_event_sender()` exposes the single-producer handle for control/UI ownership, `set_plugin_parameter_at()` / `queue_node_parameter_at()` support sample-offset events for fixed-rate f32/f64 blocks, and `set_plugin_parameter_immediate()` remains available for offline setup and tests.
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
- `DawHost::process_f64()` now uses generic graph buffers, merge/compensation helpers, and native f64 DAG processing when every active plugin declares `supports_f64()`, avoiding the f32 compatibility bridge for those graphs.

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
- Compact extended `ParametricInPlacePluginAdapter` output back to audio channels after processing so sidechain lanes are not exposed downstream.
- Grow host scratch input buffers before large-block copies to avoid panics on offline render or high-channel-count blocks.
- Add regression coverage for sidechain routing through an extended in-place plugin and large input blocks.
