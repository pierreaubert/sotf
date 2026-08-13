# Spectral Compressor code review — 2026-08-12

## Remediation status (0.5.25)

All P1–P3 findings are remediated and regression-tested:

- Both factories preserve the complete strict serialized state; invalid/unknown
  values fail before DSP construction. Runtime and exported target choices use
  integer indices and raw/normalized FFI round trips cover all choices.
- Tonal/transient masks and adaptive state are per channel. Fresh/reset equivalence
  covers both target modes, and asymmetric 12-channel processing proves that an
  unlinked channel cannot alter another channel's control path.
- FFT size is structural in static and runtime schemas. Every ordinary setter is
  schema-validated and allocation-free without rebuilding heap-backed metadata.
- Spectral smoothing is edge-normalized and reversal-invariant, preserves flat
  fields/boundaries, and maps 0–100% to a documented 0–12-bin radius.
- Adaptive timing derives from hop size and sample rate for a 500 ms time constant,
  primes from the first spectrum, and reprimes on enable/reset.
- Threshold is explicitly local narrowband coherent amplitude. Five-bin Hann-energy
  aggregation stabilizes equal tones across FFT sizes and fractional-bin alignment;
  broadband per-bin level is documented as resolution-dependent rather than PSD/LUFS.
- A continuous channel-link control blends independent detection toward maximum
  per-bin gain reduction. Linked and independent regressions cover image behavior.
- Circular input history removes the 3N/4 shift; FFT-bin copies and redundant
  full-frame accumulator clears are removed. Long-stream reconstruction exercises
  ring wrap, and QA benchmarks 1/2/6/8/12/16 channels, every FFT size, and targeted mode.
- Documentation now defines WOLA geometry, latency, calibration, smoothing,
  adaptive/reset, linking, maximum blocks, and realtime constraints. QA includes
  calibrated audio behavior, zero allocations, latency, and worst-case callback timing.

Verification commands are recorded in the completion handoff.

## Findings

### P1 — Engine/factory construction silently discards all four advanced controls

The canonical parameter model and engine settings expose `target_mode`, `delta_listen`, `adaptive_threshold`, and `adaptive_offset_db` (`params.rs:70-93, 148-176`; `plugin_settings.rs:1384-1409`). The engine converter emits all four into JSON (`plugin_config_converter/dynamics.rs:359-395`), but both factories deserialize that JSON into `SpectralCompressorPluginParams`, which only contains the original eight fields (`spectral_compressor_plugin_params.rs:7-25`; `factory/create.rs:222-226`; `plugins-bridge/src/factory.rs:281-287`). Serde ignores the unknown fields, and `from_params` then hard-codes All/disabled/zero (`spectral_compressor_plugin.rs:80-105`). Consequently saved UI settings can look correct while the instantiated processor always runs the defaults.

Use one serializable/runtime parameter type, or add the four fields to `SpectralCompressorPluginParams` and apply them in `from_params`. Prefer `deny_unknown_fields` at factory boundaries so future drift is an error rather than silent data loss. Add an end-to-end test that creates the plugin through both factories with every non-default advanced value and asserts `Plugin::get_parameter` plus a behavior-changing render.

### P1 — Tonal/transient masks cross-contaminate channels and use inconsistent frame timing

There is one separator per channel, but only one shared tonal mask and one shared transient mask (`stft_state.rs:52-60, 104-110`). Inside the channel loop, a channel consumes those shared masks at `spectral_compressor_plugin.rs:218-225`, then overwrites them with its own separator output at `:238-248`. In stereo, channel 0 therefore consumes channel 1's previous-hop masks, while channel 1 consumes channel 0's current-hop masks. This creates control-path crosstalk and makes the result channel-order dependent. The claimed “previous hop” invariant is false for every channel after the first.

Store masks as `[channel][bin]`, or split each hop into explicit all-channel analysis/mask generation and gain-computation phases with defined timing. Add asymmetric stereo and 12-channel tests where only one channel is tonal or transient; swapping channels must only swap outputs, and muting one channel must not change another channel's gain trace.

### P1 — Tonal/transient startup and reset use invalid mask state

The processor comment says the first hop uses all-one masks (`spectral_compressor_plugin.rs:212-217`), but construction initializes both masks to zero (`stft_state.rs:108-110`). The first targeted hop therefore applies no compression. Reset clears separator history but does not clear or neutralize either mask (`stft_state.rs:115-136`), so the first post-reset hop instead uses stale pre-reset classification—also shared from the last channel processed.

Initialize and reset each channel's masks to an explicitly defined neutral state (normally ones for the selected component), or compute current-frame masks before applying gain. Add fresh-instance versus reset equivalence tests for Tonal and Transient modes, checking the first several hops rather than only finiteness.

### P1 — The public direct setter and serialized constructor bypass parameter validation

The authoritative specs constrain FFT index, finite ranges, ratio, timing, smoothing, and mix (`params.rs:28-93`). The backward-compatible inherent `set_parameter` calls `apply_values` directly instead of `parametric_set_parameter` (`spectral_compressor_plugin.rs:449-454`), and `apply_values` accepts most floats without range or finite checks (`:528-607`). The factory likewise deserializes directly into the runtime struct and calls the infallible constructor (`factory/create.rs:222-226`). Examples include a NaN mix producing NaN output, NaN attack/release poisoning envelope coefficients, and an out-of-range FFT index being stored while `fft_size_from_index` silently falls back to 2048 (`misc.rs:2-5`). Existing unit/integration tests predominantly call the unvalidated inherent method, so they also fail to exercise the adapter's schema validation.

Make construction fallible and validate one canonical parameter state before allocating DSP state. Remove the inherent setter or delegate it to `ParametricInPlacePlugin::parametric_set_parameter`; make `apply_values` reject invalid types instead of coercing them to defaults. Test every field just below/above its bounds, negative FFT indices, unknown target labels, NaN/inf, and malformed factory JSON through both direct and adapted APIs.

### P1 — The generated AU/VST parameter bridge cannot set or read Target mode

`target_mode` is a `ParamSpec::choice` (`params.rs:71-72`), so the generic bridge converts it to `ParameterValue::Int` (`plugins-bridge/src/param_bridge.rs:266-283`). The runtime schema declares the same ID as a string (`spectral_compressor_plugin.rs:407-414`), and `apply_values` accepts String or Float but not Int (`:580-588`). Adapter validation therefore rejects host writes before application; even without validation an Int falls back to All. Reads are also broken because the bridge maps a returned String to raw zero. The FFI always takes this bridge path (`plugins-ffi/src/parameter_map.rs:197-207, 231-257`), so the exported choice control cannot round-trip Tonal or Transient.

Use the same choice representation end to end—prefer integer index in the runtime schema/current values/application path, converting to labels only for display. Add ParameterMap raw and normalized round-trips for every spectral-compressor choice, not just generic bridge unit tests.

### P2 — “Symmetric” spectral smoothing is not reversal-invariant and collapses toward DC at high settings

The forward pass is seeded from bin 0, mutates the array, and the backward pass is then seeded from that already-filtered result (`misc.rs:31-53`). This does not make a zero-phase symmetric filter. The existing test's own expected values prove the asymmetry: a DC impulse and a reversed Nyquist impulse do not produce reversed outputs. At `spectral_smoothing = 1`, allowed by the schema, the forward pass copies bin 0 across the whole spectrum, so all gain reduction becomes the DC-bin value.

Use an edge-normalized symmetric convolution/median kernel, or compute independent forward and reverse passes from the original envelope and combine them. Add reversal-invariance, flat-field preservation, isolated DC/Nyquist, and alpha endpoints/property tests; characterize the smoothing width in Hz or octaves rather than an undocumented recursion coefficient.

### P2 — Adaptive threshold timing and startup depend strongly on FFT size and sample rate

The “~500 ms” estimator uses a fixed per-hop coefficient of 0.98 (`spectral_compressor_plugin.rs:164-167, 194-201`). With hop `N/4`, its time constant is about 264 ms at N=1024, 528 ms at N=2048, and 1.06 s at N=4096 at 48 kHz; it changes again with sample rate. Every bin is also initialized/reset to -20 dB regardless of actual programme level (`stft_state.rs:111, 126-128`), creating long under- or over-compression transients when adaptive mode starts.

Derive `alpha = exp(-hop_size / (tau_seconds * sample_rate))`, expose/document the intended time constant, and prime each bin from the first valid spectrum (or provide a controlled warm-up). Test estimator trajectories and steady gain for identical material across all FFT sizes, 44.1/48/96/192 kHz, block partitions, enable transitions, and reset.

### P2 — Per-bin threshold calibration changes with FFT size and tone/bin alignment

The magnitude normalization is coherent-amplitude calibration for a bin-centred sinusoid (`spectral_compressor_plugin.rs:154-192`). It is not power/PSD calibration: broadband energy per bin falls as FFT size grows, and an off-bin tone spreads across the Hann main lobe, reducing the level seen by each independently compressed bin. Choosing a larger FFT or shifting a tone fractionally between bins can therefore move material across the same dB threshold even though input level is unchanged. Current calibration tests use one FFT size and one steady sine, so they do not define this behavior.

Decide whether threshold means coherent tone amplitude, band power, or power spectral density. For level-stable broadband behavior, include Hann energy/ENBW and bin bandwidth in the detector; for tone behavior, consider local spectral-energy aggregation before gain computation. Add bin-centred/off-bin sweeps, white/pink noise, impulses, and identical-level comparisons across all FFT sizes.

### P2 — Independent channel envelopes can shift stereo and immersive images

Every channel has independent bin envelopes (`stft_state.rs:29-31`), and there is no link control. A transient or resonance present more strongly in one side is attenuated differently, changing inter-channel level and phase relationships. That can be desirable for restoration but conflicts with the README's broad transparency claim for bus/mastering use. The existing “stereo independence” test treats independence as the only desired topology and does not measure image stability.

Add an explicit channel-link mode/amount, with layout-aware groups or max/RMS-linked detector envelopes, while retaining independent mode for surgical work. Test correlated stereo, centre-panned transients, antiphase signals, and 5.1/7.1.4 groups for image/correlation preservation.

### P2 — Structural changes and ordinary parameter updates are not realtime-safe

Changing FFT size replaces the complete STFT state and replans per-channel FFTs (`spectral_compressor_plugin.rs:129-135, 531-539`), while changing any parameter rebuilds a heap-backed schema containing Strings (`:341-430, 606`). FFT size is correctly marked structural/setup in the static specs (`params.rs:28-32`), but the public setter and FFI surface do not themselves enforce a non-audio-thread boundary. A live FFT change also changes `latency_samples()` immediately (`spectral_compressor_plugin.rs:625-629`), which can desynchronize an already compiled host plan until it is rebuilt.

Reject structural mutation while active or build/swap prepared state off-thread and require the host to recompile latency compensation. Keep immutable schema metadata separate from current values so automatable setters do not allocate. Add allocation-count tests around every non-structural setter and a host-plan test proving FFT-size changes trigger state/latency recompilation rather than callback allocation.

### P2 — The STFT scheduler performs avoidable whole-frame memory traffic per hop

At every N/4 hop, each channel shifts 3N/4 input samples (`spectral_compressor_plugin.rs:687-695`), writes an N-sample window, copies N/2+1 complex bins twice, overlap-adds N samples, and clears another full N-frame accumulator region (`:169-179, 305-336`). Drained accumulator frames are cleared again (`:724-750`). This is bounded and allocation-free, but scales poorly at 75% overlap and high channel counts; the QA binary processes only one silent stereo block and provides no throughput evidence.

Use circular input indexing instead of `copy_within`, process FFT buffers in place where the helper permits, and prove whether clear-on-drain alone is sufficient before removing the forward full-frame clear. Benchmark 1/2/6/8/12/16 channels, every FFT size and sample rate, with target modes both off and on; report callback CPU percent and worst-case time, not only average throughput.

### P3 — Documentation and verification do not cover the implemented feature surface

README and nested instructions list only `lib.rs`/`params.rs` and describe the original per-bin compressor, omitting STFT geometry/latency, dry alignment, target modes, adaptive threshold, delta semantics, reset behavior, channel independence, maximum block size, and realtime constraints. The changelog records previous fixes well, but the QA binary is only a silence smoke call. There are no factory round-trips for advanced fields, FFI choice tests, targeted multichannel classification tests, adaptive invariance tests, or performance benchmarks.

Document the actual signal and host contracts and promote the missing cases above into focused regression/QA suites. The catalog's zero-allocation evidence should link the dedicated allocation test explicitly rather than only listing functional tests (`factory/catalog.rs:752-769`).

## Strengths

- The core WOLA convention is explicit and internally consistent: periodic Hann analysis and synthesis, 75% overlap, unnormalized inverse FFT, and `1/(1.5N)` scaling (`stft_state.rs:15-21, 66-77`; `spectral_compressor_plugin.rs:305-318`). Identity and magnitude-calibration tests exercise the main reconstruction path.
- Host latency is deliberately one full FFT frame, the dry path is delayed to match it, output always returns `context.num_frames`, and varied block-size tests demonstrate partition independence (`spectral_compressor_plugin.rs:625-777`).
- The normal process path validates exact buffer length and maximum block size, preallocates its owned buffers, avoids locks/logging/I/O, enables FTZ/DAZ, and passed the dedicated zero-allocation test.
- Attack/release coefficients are evaluated at hop rate and recomputed after sample-rate/FFT changes; zero timing values are guarded. The soft-knee equation itself is the standard continuous quadratic knee.
- State is sensibly grouped in `StftState`, the main struct stays below the 30-field budget, FFT plans/buffers are created outside processing, and tests cover silence, denormals, reset, latency, dry/wet alignment, hard/soft knee, loud/quiet signals, delta mode, and malformed audio-buffer length.

## Realtime and performance assessment

The ordinary initialized `process_in_place` path performs zero observed heap allocation; `cargo test -p sotf-plugins --test realtime_allocation_tests test_spectral_compressor_zero_alloc` passed. Work is bounded, with no mutex, I/O, or publication cache in the callback. The dominant costs are four STFT hops per FFT frame, per-bin `sqrt/log10/powf`, optional tonal/transient median filtering, repeated planar/interleaved copies, overlap-add, and redundant-looking accumulator clearing. Parameter application and especially FFT-size mutation are not callback-safe and need an explicit host boundary.

## Focused verification

- `cargo test -p sotf-plugin-spectral-compressor` — 28 passed across three suites.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_spectral_compressor_zero_alloc` — 1 passed, 45 filtered out.
- `cargo test -p sotf-plugins --test factory_integration_tests advertised_factory_types_are_smoke_covered_or_documented_special_cases` — 1 passed, 16 filtered out.

These passing tests establish the current baseline; they do not contradict the parameter-wiring and channel-mask findings because none asserts those contracts.

## Coverage reviewed

Reviewed every plugin-owned file: nested `AGENTS.md`, README, full changelog, manifest, QA binary, crate root, parameter/spec/layout/serde models, helpers, complete processor and STFT state, all unit/helper/integration tests. Integration review covered facade exports, both factories, catalog metadata/evidence, FFI parameter mapping and generic parameter bridge, engine plugin type/settings/accessors/config conversion, factory/high-channel/latency/allocation tests, and the `ParametricInPlacePlugin` adapter. Shared DSP review covered `RealFftProcessor`, periodic Hann generation, `Smoother`, `DeltaMonitor`, and the complete `TonalTransientSeparator` implementation. No production code was changed and no broad workspace build was run.
