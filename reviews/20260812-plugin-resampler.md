# Resampler plugin code review — 2026-08-12

## Remediation status — 0.5.27

0.5.27 closes the retained copy/profile evidence gap without changing clock-domain semantics.
The redundant `residual_input` to `input_buffer` planar copy was removed; rubato now reads the
preallocated residual planes directly. Automated coverage checks exact complete-stream rate
counts, spectral stop-band rejection, bit-exact callback-partition invariance, allocation-free
dynamic-ratio automation, and the QA callback-deadline matrix.

The remaining P0 wording about producing a fixed output callback for every fixed input callback
is not implementable inside the plugin contract when rates differ. A 256-frame callback spans
different wall-clock durations at 44.1 and 48 kHz (about 279 output frames per 256 input frames).
Enforcing 256 frames on both sides would redefine a clock, change pitch/duration, or require
periodic duplication/drop. Produced frames therefore remain authoritative; a device-facing
fixed-frame consumer must own an output-clock FIFO/pull scheduler outside the plugin graph. This
is an integration architecture boundary, not a Resampler DSP defect.

All P0-P3 findings are remediated. In addition to the 0.5.25 timing,
transactionality, realtime-parameter, choice, latency, rate-negotiation, unity,
known-zero, estimator, documentation, and QA fixes, 0.5.26 adds an object-safe,
allocation-free `Plugin::drain` lifecycle through `DawHost` and the processing
engine. EOF now drains every plugin in causal order; stop/seek reset pending
state. Rubato completion follows its documented cumulative-output procedure,
retaining the exact final sinc tail instead of estimating discard from one
padded chunk. Host nodes after a rate-changing plugin are initialized and
processed at the negotiated output clock.

Regression evidence covers residual boundaries 1/chunk-1/chunk/chunk+1,
up/down/extreme ratios, all quality modes, complete-stream counts, downstream
host propagation, stop/reset, transactional first- and second-chunk retries,
allocation-free ratio automation with pending residual, all bridge choice
indices, quality-dependent filter output, output-domain latency, bit-exact
unity including the documented non-finite policy, known-zero reporting, and
capacity/availability across 1-16 channels. The QA binary retains its
zero-allocation and callback-throughput gate; profile-guided copy/SIMD work is
an optimization opportunity, not an outstanding correctness bug.

## Findings

### P0 — Sub-chunk callbacks corrupt live timing by inserting silence and then emitting burst-sized blocks

`process()` accumulates input until `chunk_size` frames are available and returns zero for every earlier callback; once the residual fills, it emits the entire rubato chunk at once (`resampler_plugin.rs:537-612`). With the default 1024-frame chunk and a 256-frame host callback, the first three calls therefore return 0 and the fourth returns roughly 1115 frames for 44.1 -> 48 kHz. This directly contradicts the crate contract that output is “drained incrementally to match the caller's frame count” (`AGENTS.md:19`). The production host does not queue those variable blocks: when a variable-frame plugin returns fewer frames than the input callback it zero-fills the missing frames and changes the returned count back to the input count (`sotf-host/src/host/daw_host.rs:1817-1821`). The engine then sends that padded frame at the Resampler's declared output rate (`processing_state.rs:529-566, 607-613`). The example above consequently delivers three 256-frame silent blocks followed by a roughly 1115-frame signal block—about 1883 output-rate frames for 1024 input frames instead of about 1115—with audible dropouts and a large clock error. Existing variable-block tests explicitly assert this burst behavior rather than testing the host (`src/tests.rs:487-555`).

Either move sample-rate conversion to a stream boundary that owns an output FIFO and clock, or add a preallocated output FIFO and make the host consume it at the correct output-rate cadence after a defined priming delay. Do not pad a zero-output resampler call as if it were identity-rate processing. Add an end-to-end host/engine test using 128/256/300-frame callbacks with a 1024-frame rubato chunk; assert continuous timestamps, bounded startup latency, the correct cumulative duration, no periodic silence, and block-partition-invariant audio.

### P1 — End-of-stream audio is still lost because `flush()` is not part of the plugin/host contract

The changelog correctly says callers must flush trailing residual frames (`CHANGELOG.md:19-22`), but `flush()` is only an inherent method on `ResamplerPlugin` (`resampler_plugin.rs:311-324`). `Plugin` has no drain/finalize method (`sotf-host/src/plugin.rs:41-231`), and there are no production callers of this concrete method. Engine “flush” messages drain/reset decoder/playback queues; they cannot invoke it through `Box<dyn Plugin>`. Any EOF, stop, graph replacement, seek, or stream deactivation with `1..chunk_size-1` pending input frames therefore discards those frames.

Add an object-safe, allocation-free drain/finalize contract to `Plugin`, propagate it through `DawHost` and the processing engine, and keep processing until every stateful plugin reports drained. Cover EOF after every residual length around 0/1/`chunk_size-1`/`chunk_size`, seek and stop, and a Resampler followed by another stateful plugin. A concrete helper that only offline callers know about is not sufficient.

### P1 — The current one-chunk `flush()` does not correctly drain or identify the sinc tail

`flush()` zero-pads the residual to one input chunk, processes exactly once, and labels `output_frames - ceil(residual * current_ratio)` as discardable padding (`resampler_plugin.rs:334-377`). A sinc resampler has history and an output-domain delay; the response to the last valid input can extend into the nominally padded region and beyond this single call. Conversely, leading delay is a stream-level alignment property, not a per-flush count that can be separated by multiplying residual length by the current ratio. The approximation is even less valid during a ramped ratio change. Tests only prove that one call returns nonzero signal and a nonzero discard count (`src/tests.rs:487-516, 871-910`; `tests/integration.rs:125-145`); they never compare the complete render with a reference or prove that trimming preserves the tail.

Define the exact stream alignment contract: feed enough zero input to drain rubato's FIR history, track rational/cumulative output position rather than estimating one block locally, and return an exact valid range or perform the trimming inside an offline renderer. Add impulse and swept-sine goldens against rubato's documented complete-stream procedure at up/down/extreme ratios, every quality, every residual length, and ratio ramps. Verify both leading-delay removal and preservation of the final ringing sample.

### P1 — Too-small output errors consume input and advance DSP state before returning `Err`

Both processing paths validate destination capacity too late. `process()` clears the full residual and advances rubato at `process_into_buffer`, then checks the caller's output length (`resampler_plugin.rs:563-595`). `flush()` likewise clears `residual_frames` and advances rubato before its check (`:329-365`). Retrying after the documented buffer error silently loses the chunk and starts from a later filter state; `last_output_frames` can also remain stale because the error bypasses its update. The tests merely assert `Err` and never retry or inspect state (`src/tests.rs:820-869`).

Compute a conservative required capacity before mutating residual or rubato state and reject early. If an internal rubato failure is possible after state mutation, define the plugin as failed/reset-required rather than apparently retryable. Add transactional tests comparing error->retry with a fresh uninterrupted instance for process and flush, including errors on the second chunk of a multi-chunk call.

### P1 — Runtime ratio automation allocates on the processing thread, while quality changes allocate and drop audio

Every `set_parameter` first calls `validate_parameter`, which clones `cached_parameters` and its Strings through `parameters()` (`resampler_plugin.rs:429-434`; `sotf-host/src/plugin.rs:65-76`). Successful setters then rebuild the entire cached `Vec<Parameter>` (`resampler_plugin.rs:207-226, 296-308, 382-395, 433-474`). This makes the explicitly runtime-adjustable ratio allocate for each automation event. Quality is marked structural/setup in the static schema (`params.rs:9-19`), yet the live setter immediately constructs a new rubato sinc table, allocates its internal state, clears pending residual, and replaces filter history (`resampler_plugin.rs:169-204, 436-446`). If invoked while active, this can miss a realtime deadline and silently drop buffered audio. The zero-allocation test covers only unchanged 1024-frame processing, not automation (`realtime_allocation_tests/tests.rs:203-224`).

Use allocation-free parameter validation and fixed cached storage/primitive atomics for the realtime ratio path; do not rebuild display metadata for every ratio update. Route quality and setup-mode changes through an off-thread prepare/commit graph rebuild, with an explicit discontinuity or drain/crossfade policy. Add allocation assertions for ratio automation, automation stress with worst-case event density, and pending-residual tests for quality changes.

### P1 — The exported choice parameter cannot set or read quality

The static schema declares `quality` as `ParamSpec::choice` (`params.rs:9-19`). The shared bridge therefore sends `ParameterValue::Int` and expects numeric reads (`plugins-bridge/src/param_bridge.rs:277-301`), while the runtime setter accepts only String and its getter returns String (`resampler_plugin.rs:436-446, 477-483`). FFI/NIH controls backed by `ParamBridge` cannot select Fast/Medium/High; reads of every String normalize to choice zero. Direct plugin tests pass because they bypass the bridge and use Strings (`src/tests.rs:294-329`; `tests/integration.rs:36-59`).

Use choice indices end to end, converting to `ResamplerQuality` at the runtime boundary, or teach the bridge a single canonical label mapping used consistently by all plugins. Add raw and normalized bridge/FFI round-trips for all three choices and an exported-format render proving the filter actually changes.

### P1 — Latency adds values expressed in different sample-rate domains

Rubato documents `output_delay()` in output frames, and the plugin exposes it that way (`resampler_plugin.rs:267-275`). `latency_samples()` then adds `chunk_size - 1`, which is a count of input frames (`:615-621`). The sum is dimensionally invalid whenever the rates differ: 1023 input frames at 96 -> 44.1 kHz correspond to about 470 output frames, while at 22.05 -> 44.1 kHz they correspond to about 2046. The exact-value test locks in the incorrect mixed-domain arithmetic for just 44.1 -> 48 kHz (`src/tests.rs:775-802`). Further, the current burst architecture has phase-dependent buffering delay rather than one stable compensable delay.

Define the host latency domain across rate-changing nodes, convert buffering time into that domain with conservative rational arithmetic, and distinguish fixed algorithmic delay from scheduler/FIFO priming. Add impulse-position tests for upsampling, downsampling, unity, extreme ratios, multiple callback sizes, and plugin chains with parallel latency compensation.

### P1 — Initialization accepts a host rate that makes the declared output clock false

Construction permanently builds rubato from configured input/output rates (`resampler_plugin.rs:95-166`). `initialize()` only logs when the host input rate differs and still succeeds (`:489-500`), while `output_sample_rate()` unconditionally reports the configured output rate (`:638-640`). Processing samples actually arriving at the host rate through the configured ratio therefore produces the wrong pitch/duration and misleading timestamps. A warning cannot make this graph valid.

Reject a mismatched host rate before activation, or rebuild from the negotiated host rate off-thread and update the graph's rate contract atomically. Add factory/host tests for matching and mismatching rates and assert failure occurs before any audio or topology is committed.

### P2 — Unity-rate conversion is neither passthrough nor transparent as documented

The crate instructions promise passthrough when input and output rates match (`AGENTS.md:41`), but construction always creates rubato and `process()` always chunk-buffers and filters (`resampler_plugin.rs:95-166, 537-607`). Unity-rate use still incurs sinc coloration, FIR/chunk latency, copies, and the same burst behavior. This matters because shared DSP/factory tests instantiate Resampler at equal rates (`all_plugins_dsp_matrix.rs:55-59`; `parameter_roundtrip_tests.rs:83-87`).

Implement an exact interleaved copy path with identity frame/rate/latency metadata when the effective ratio is exactly one and dynamic ratio is disabled. If dynamic mode is enabled, define a safe transition into/out of bypass. Add bit-exact unity tests over arbitrary block partitions, reset, and silence/non-finite policy.

### P2 — `last_output_frames()` reports “unknown” instead of a known zero

The trait defines `None` as unknown/not tracked and asks variable-output plugins to override it (`sotf-host/src/plugin.rs:212-218`). Resampler returns `None` whenever the known last count is zero (`resampler_plugin.rs:642-647`). Any caller using this method cannot distinguish “this sub-chunk produced zero” from “plugin does not track output,” and `DawHost::last_output_frames()` then falls back to an earlier plugin (`daw_host.rs:973-979`).

Always return `Some(self.last_output_frames)` after construction/process/reset/flush, and update the field consistently on all successful and error paths according to a documented rule. Add direct and multi-plugin host tests for a zero-output call.

### P2 — The frame estimator greatly overstates incomplete input and drives avoidable buffer growth

`output_frames_for_input()` uses `div_ceil`, so even one pending input frame reserves a full `output_frames_max()` block (`resampler_plugin.rs:248-259, 624-635`). That maximum also includes rubato's allowed 2x dynamic-ratio envelope (`:156-163`). The engine resizes its processing buffer to this estimate before every call (`processing_state.rs:529-539`), although the plugin will emit zero until a full chunk exists. This is safe for capacity but can retain large buffers and distort graph scheduling/cached identity decisions, especially with many channels, small callbacks, or extreme ratios.

Separate `maximum_output_capacity(input)` from `expected/available_output_frames(input)` in the host API. Use floor(full chunks) for immediately available frames and retain a checked maximum for allocation. Benchmark retained memory and resize behavior for 1-16 channels, 1-frame through multi-chunk callbacks, and both ratio bounds.

### P2 — Documentation describes a different implementation and public API

The crate guide says output is drained incrementally, interpolation is Cubic, initialize rebuilds from placeholder rates, equal rates bypass, and public constructors omit `chunk_size` (`AGENTS.md:19, 23-24, 38-42`). The code uses Linear interpolation, constructs the final resampler immediately, never bypasses, and requires chunk size (`resampler_plugin.rs:62-83, 148-163`). It also lists `input_sample_rate`/`output_sample_rate` as parameters although runtime parameters are quality/dynamic_ratio/ratio (`AGENTS.md:27`; `params.rs:9-24`). Changelog paths/signatures are stale and its deferred statement that chunking latency is not reported conflicts with the current implementation (`CHANGELOG.md:19, 61-64`). These contradictions obscure precisely the scheduling and drain contracts that callers must get right.

Rewrite the README/guide around the actual rate, frame-count, latency, residual, drain, dynamic-ratio, and realtime contracts; keep examples compiled as doctests or integration tests. Document Linear-vs-Cubic quality rationale and measured passband/stopband targets rather than relying only on tap labels.

### P2 — Audio-quality and performance QA cover one favorable operating point

The QA binary measures only stereo Medium 44.1 -> 48 kHz with 1024-frame blocks, checks merely “some output,” and reports average throughput (`qa_resampler.rs:8-89`). The zero-allocation test uses the same full-chunk shape. The only spectral assertion is a loose RMS check for Fast downsampling of a single 22.5 kHz tone (`src/tests.rs:713-773`); there are no passband-ripple, stopband, alias-grid, phase/delay, THD+N, block-partition, high-quality, high-channel, worst-callback, or dynamic-ratio-ramp goldens. Meanwhile each full chunk is copied interleaved -> planar residual -> planar input and then interleaved again (`resampler_plugin.rs:552-569, 597-603`), a known deferred bandwidth cost (`CHANGELOG.md:51-60`).

Add swept/multitone reference measurements across supported rates/ratios and qualities, cumulative rational frame-count tests with exact bounds, and worst-case callback-time percentiles for 1/2/8/16 channels and small/irregular/multi-chunk blocks. Profile before optimizing, then remove the residual-to-input copy by adapting the canonical planar buffer directly and consider a blocked/SIMD interleave kernel where channel-count profiling justifies it.

## Strengths

- Construction rejects zero channels, rates, and chunk size before allocating (`resampler_plugin.rs:85-93`), and rubato ratio updates are committed to the tracked value only after rubato accepts them (`:293-308, 382-395`). Rubato's own reset restores the original ratio, so the plugin's nominal tracking in `reset()` is consistent.
- The DSP core uses a maintained resampling library, quality-dependent sinc length and cutoff calculated for Blackman-Harris 2, preallocated planar buffers, and `process_into_buffer` rather than allocation-returning convenience APIs (`resampler_quality.rs:14-33`; `resampler_plugin.rs:140-166, 572-584`).
- Capacity arithmetic is conservative and uses saturating addition/multiplication in the public estimator (`resampler_plugin.rs:248-259`). Complete full-chunk processing is allocation-free in the measured steady state.
- Tests cover constructors, up/downsampling, five/six-channel routing, quality selection, dynamic ratio bounds delegated to rubato, reset, malformed input/output lengths, variable block accumulation, multi-chunk capacity, ten-second cumulative counts, a basic anti-alias case, and the existing flush behavior.
- The plugin struct has 17 fields, below the 30-field budget, and the callback contains no locks, logging, file I/O, or explicit heap allocation.

## Realtime and performance assessment

For unchanged parameters and full 1024-frame chunks, the callback is bounded and passed the allocation test; the QA run measured about 0.55% of one core for stereo Medium 44.1 -> 48 kHz on this machine. The cost is dominated by rubato's sinc interpolation plus scalar deinterleave/interleave and one avoidable full planar copy. Those favorable results do not make the plugin realtime-correct in the engine: irregular/sub-chunk callbacks break timing, runtime ratio setters allocate, quality changes allocate and discard state, and retained capacity can be much larger than immediately produced output. Worst-callback latency and high-channel/extreme-ratio behavior remain unmeasured.

## Focused verification

- `cargo test -p sotf-plugin-resampler` — 56 passed across four suites.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_resampler_zero_alloc` — 1 passed, 45 filtered out.
- `cargo test -p sotf-plugins --test factory_integration_tests` — 17 passed.
- `cargo test -p sotf-plugins --test parameter_roundtrip_tests` — 2 passed.
- `cargo run -p sotf-plugin-resampler --features qa --bin qa-resampler` — passed; reported 0 allocations and approximately 0.55% average CPU for its single benchmark case.

These establish the current baseline but do not contradict the findings: no test runs sub-chunk Resampler calls through `DawHost`/the engine, invokes a polymorphic EOF drain, checks transactional retry, round-trips quality through `ParamBridge`, measures physical latency across ratios, rejects host-rate mismatch, or automates ratio under the allocation counter.

## Coverage reviewed

Reviewed every plugin-owned file without omission: nested `AGENTS.md`, README, complete changelog, manifest, crate root, static parameter schema, quality implementation, complete Resampler implementation and embedded tests, complete unit-test module, both integration suites, and QA binary. Integration review covered the public `Plugin` frame/rate/latency contract, DawHost variable-frame detection/estimation/processing/zero-padding and node-buffer semantics, engine processing buffer sizing/frame construction/sample-rate propagation, offline/decoder/signal-recorder Resampler call sites, canonical facade/factory/catalog, shared allocation/DSP/factory/parameter/high-channel tests, ParamBridge plus FFI/NIH exposure, and rubato 1.0.1 ratio/reset/delay semantics. No production code was changed and no broad workspace build was run.
