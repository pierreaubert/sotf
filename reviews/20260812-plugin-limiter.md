# Limiter code review — 2026-08-12

Follow-up 0.5.15 aligns lookahead's host-visible update mode with its
latency-changing rebuild-only contract and adds focused regression coverage.

## Remediation status — 0.5.14 complete

Remediated in 0.5.13: independent per-channel envelopes and lookahead histories at
`link_amount=0`, in addition to the 0.5.12 fixes: true zero-latency operation at zero lookahead, mandatory
predictive detection for nonzero lookahead, finite/schema-bounded preset values,
exact direct-buffer validation before state changes, deterministic reset of DSP
and monitoring state, and the ISP correction release recurrence. Regression tests
cover sample-exact zero lookahead, malformed buffer rejection without state
advance, preset sanitization, latency metadata, and reset observables.

Completed in 0.5.14: the conformant BS.1770 Table-2 true-peak FIR, predictive
and controllable ISP contract, amortized O(1) monotonic lookahead maxima,
dB-domain soft-knee gain computation, sample-based meters, structural latency
updates, non-finite sanitization, and stable parameter-schema storage.

All P0-P3 findings are resolved; no remediation remains deferred.

## Findings

### P1 — Fixed: zero lookahead still delays audio by one sample while reporting zero latency

Construction forces `lookahead_len >= 1` and always reads the delay ring before writing (`limiter_plugin.rs:81, 510-513`). `latency_samples()` returns zero whenever `lookahead_ms == 0` (`:628-633`). Tests explicitly encode the hidden one-sample shift rather than treating it as a defect. This misaligns parallel paths and invalidates host compensation/compiled metadata.

Special-case zero lookahead as true pass-through storage, or report one sample everywhere (the former matches the public contract). Add impulse and sample-exact dry-bypass tests through both regular and compiled paths.

### P1 — Fixed: default lookahead does not maintain the future peak until that sample reaches output

With `feed_forward = false` (the default), `effective_peak` is only the current input frame (`limiter_plugin.rs:468-476`) while gain is applied to audio delayed by the full lookahead. A transient raises gain reduction when unrelated earlier audio is emitted; the envelope then releases for `lookahead_len` samples before the transient itself exits. With long lookahead and short release, the peak can substantially exceed the ceiling. The documented “feedback” topology is not implemented—the detector always reads input, never output.

For a predictive limiter, maintain the maximum required gain over the future window (efficient monotonic deque/running maximum) and align it with the delayed sample. Alternatively make correct feed-forward mandatory when lookahead is nonzero. Add isolated-impulse ceiling tests spanning minimum release and maximum lookahead, at several block partitions.

### P1 — Fixed: `link_amount = 0` is not independent limiting

This was fixed in 0.5.13: the limiter now keeps per-channel envelopes and per-channel
lookahead histories whenever linking is not full. At zero, each channel therefore has
independent attack/release state; full linking retains the shared envelope behavior.

The asymmetric loud/quiet stereo regression now verifies that a quiet channel remains
at its input level while the loud channel is limited.

### P1 — Fixed: unvalidated preset lookahead can index beyond allocated storage

`from_params` forwards serialized values directly to `new` (`limiter_plugin.rs:214-229`). `new` computes active `lookahead_len` from that value but allocates only the parameter-spec maximum (`:81-113`). A factory JSON value above 20 ms therefore makes the active ring longer than its backing vectors and eventually panics in processing. Threshold, release, mix, and non-finite serialized values are also not consistently validated.

Make `from_params` fallible and validate/clamp with one authoritative schema before constructing any size/state. Prefer rejecting malformed presets with a field-specific error. Test values just outside every bound plus NaN/inf and extreme channel counts.

### P1 — Fixed: “True Peak” is not a standards-grade true-peak detector

The shared helper describes ITU-R BS.1770 inspiration but uses a causal Catmull–Rom interpolation with the unknown future point replaced by the current sample (`math-dsp/src/true_peak.rs:47-67`). This is not the specified band-limited oversampling filter and has uncharacterized under/over-read versus reconstructed analogue peaks. Consequently the plugin cannot claim reliable dBTP or streaming-platform compliance.

Use a verified BS.1770/EBU true-peak oversampler with defined filter delay, normalization, and edge handling. Validate against published conformance vectors and high-frequency phase sweeps at 44.1/48/96/192 kHz. Include detector latency in gain alignment and host metadata.

### P1 — Fixed: ISP mode cannot guarantee the advertised ceiling

ISP correction observes output only after it was emitted and applies correction from the next frame (`limiter_plugin.rs:543-566`), so violations are inherently not retroactively prevented. Correction is capped at 12 dB. More decisively, output ISP is measured after dry/wet mixing: with `mix < 1`, an over-ceiling dry component cannot be removed by reducing only the wet path; at `mix = 0` ISP mode has no control at all. Tests skip startup/convergence or check sample peaks rather than proving every output true peak.

Define ISP mode as a predictive true-peak limiter, require/force 100% wet for a guarantee, and reserve adequate lookahead for detector/interpolation delay. Otherwise weaken the UI/documentation claim. Add whole-stream independent true-peak verification including startup, isolated impulses, mode changes, max input, all mix values, and signals exceeding the 12 dB correction cap.

### P2 — Fixed: ISP correction release uses the wrong recurrence

The code converts dB correction to a linear factor and multiplies that factor by `release_coeff` (`limiter_plugin.rs:560-565`). A correction factor must decay toward unity, not toward zero: the one-pole form is `1 + coeff * (factor - 1)`. The current expression subtracts an approximately constant amount in dB per sample until clamped, with duration dependent on starting correction, so it does not implement the advertised release time constant.

Use a mathematically defined state domain and one-pole target. Test 63.2%/time-constant points for several starting corrections, releases, and sample rates.

### P2 — Fixed: feed-forward scanning is still O(lookahead) per frame

The current improvement reduces the scan from interleaved samples to `lookahead_peaks`, but still folds the complete active window for every frame (`limiter_plugin.rs:468-474`). At 192 kHz/20 ms this is roughly 737 million comparisons/s before channel DSP. The unreleased changelog claims amortized O(1), which source does not implement.

Use a monotonic maximum queue or two-stack sliding-window maximum with preallocated storage. Benchmark worst-case 192 kHz, 20 ms, high channel counts and assert zero allocations.

### P2 — Fixed: reset/reinitialize leaves observable and control state inconsistent

`reset()` clears envelopes/detectors/buffers but not `lookahead_pos`, monitoring values, cache counter/cache snapshot, ISP monitoring scratch, or threshold/mix smoother progress (`limiter_plugin.rs:389-400`). `initialize()` does not call reset and can retain envelope/delay content when reinitialized at the same geometry. UI may display stale limiting/ISP data for up to ten more blocks.

Specify whether reset preserves parameter targets but resets current smoothing, then make reset/reinitialize deterministic and refresh monitoring immediately. Compare post-reset output/data with a fresh instance for impulse→reset→silence and sample-rate reinitialization.

### P2 — Fixed: parameter changes rebuild heap-backed schemas and can resize in setters

Every `apply_values` ends in `rebuild_cached_parameters()` (`limiter_plugin.rs:367`), allocating a new parameter vector and strings. `update_coefficients` contains defensive vector resizes (`:233-243`). The normal spec-bounded initialized path has capacity, but direct/out-of-contract values can allocate. If host automation invokes parameter application on the audio thread, this violates realtime safety.

Keep immutable schema metadata separate from atomic/current values; validate structure before the realtime boundary and preallocate all maximum storage. Add allocation-count tests around every automatable parameter after initialization.

### P2 — Fixed: soft mode is a knee plus hard ceiling, not high-quality soft limiting

The cubic curve operates only over the last 10% of linear threshold and hard-clamps above threshold (`limiter_plugin.rs:515-538`). It is not oversampled, so nonlinear use produces aliased harmonics. Documentation alternates between “soft knee” and “soft clipping/warm saturation,” which are different algorithms.

Choose the intended behavior. For a transparent limiter, implement a dB-domain knee in gain computation. For saturation, oversample a characterized waveshaper and account for filter latency. Add transfer-curve, THD/alias, and level-dependent tests.

### P2 — Fixed: monitoring cadence is block-size dependent

Cache updates occur every ten process calls, not after elapsed samples (`limiter_plugin.rs:578-604`). At 32-frame blocks meters update extremely fast; at 4,096 frames they update below 2 Hz. Peak and GR values also represent only the last block/envelope point rather than a defined integration/hold behavior.

Use a sample counter and specify peak hold/decay. Test identical monitoring traces across block partitioning.

### P3 — Fixed: buffer and non-finite contracts are unchecked in the direct in-place path

`process_in_place` indexes `num_frames * channels` without validating the slice length, while the compiled path does validate. Non-finite audio can propagate through delayed/wet output, and final denormal flushing is not sanitization.

Return a clear error for undersized buffers and define a finite-input policy consistent with the host. Add short-buffer and NaN/inf tests without panics.

## Realtime and performance assessment

The ordinary fixed-parameter limiter process path performs no intended heap allocation. True-peak interpolation, per-frame log/pow, and channel scans are bounded; the feed-forward full-window scan is the dominant scaling problem. Schema rebuilding, defensive resizing, and analyzer publication must remain outside the callback. The cache itself is safe here because `LimiterData` owns a plain `Vec`, unlike the nested-Arc multiband caches.

## Coverage reviewed

Reviewed all limiter crate files: nested instructions, README/changelog/UI/usage, manifest, QA binary, every source module, full unit/integration/property/dynamics tests, allocation and realtime callers/bench references, and factory/catalog/type wiring. Relevant shared/host code reviewed includes `ParametricInPlacePlugin`/adapter validation, `RealTimeCache`, `LookaheadBuffer`, `TruePeakDetector`, `DualRelease`, smoothers, compiled metadata and host latency propagation. No code was changed and no broad workspace build was run.

## Post-remediation verification

- `cargo test -p sotf-plugin-limiter --offline` — 100 passed across five suites.
- `cargo clippy -p sotf-plugin-limiter --all-targets --offline -- -W warnings` — passed.
- `cargo run -p sotf-plugin-limiter --bin qa-limiter --features qa --offline` — passed, including zero-allocation processing.
