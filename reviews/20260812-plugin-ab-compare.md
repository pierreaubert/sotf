# A/B Compare code review — 2026-08-12

## Remediation status

- **Fixed:** both empty and nested paths now measure every active block and pass
  only the active slice to the persistent loudness monitors; cache publication
  remains throttled.
- **Fixed:** bypass uses a dedicated dry delay equal to the reported maximum path
  latency and publishes its state immediately.
- **Fixed:** construction rejects invalid channels, non-finite/out-of-range
  numeric state, invalid selected paths, and unordered band masks before DSP
  state is created.
- **Fixed:** callback sample-count multiplication is checked and non-empty paths
  use fixed prepared buffers, returning an error above 48,000 frames rather than
  resizing on the audio thread.
- **Fixed:** failed runtime path parsing/building preserves the previous stored
  configuration and render host; band-mask reset now discards IIR history.
- **Fixed:** the main struct is back within the 30-field budget by grouping path
  scratch buffers and peak diagnostics while adding dry latency state.
- **Fixed and tested:** path and band-mask controls now carry structural update modes. Live writes
  are rejected without changing render state or latency; the outer host reconstructs the complete
  node and recompiles its latency plan before a safe graph-boundary swap.
- **Fixed and tested:** canonical and bridge factories inject their authoritative factory before
  initial Plugin/Rack/Graph path construction, including plugins outside the standalone fallback.
- **Fixed and tested:** the canonical schema now exactly matches the 16-key runtime surface,
  including band masks and structural modes; engine settings/accessors/conversion preserve them.
- **Fixed and tested:** same-source crossfades use a unity-preserving linear law. Identical paths
  remain unity throughout the sweep and inverted paths intentionally cancel at centre.
- **Fixed and tested:** diagnostics publish at 20 Hz from elapsed frames rather than callback count;
  initialize/reset seed the scheduler deterministically and bypass publishes immediately.
- **Fixed and tested:** scalar realtime setters do not rebuild metadata. Representative plugin/rack,
  latency, band-mask QA at 1/2/6/12 channels is zero-allocation; the latest measured CPU was
  0.10–0.86% of realtime in the debug QA build.
- **Fixed and tested:** the nested DawHost linear-plan callback no longer clones heap-backed plans;
  it temporarily takes and restores the immutable compiled plan, removing two allocations per path.

All P0-P3 findings are resolved; no remediation remains deferred.

## Findings

### P1 — Loudness matching analyzes mostly zeros or stale samples and advances its EBU clock incorrectly

The normal path writes only `buffer_[a|b][..expected_samples]` (`abcompare_plugin.rs:857-867`) but passes the entire backing vectors to `AutoGain` (`:882-885`). Those vectors start at one second of audio (`48_000 * channels`) and can only grow (`:167-168, 846-855`), so a 128-frame callback is measured as 128 current frames followed by 47,872 zeros; after a larger block precedes a smaller one, the tail is stale programme audio. The empty-path fast path correctly measures the active input slice (`:399-403`), so merely adding a unity plugin changes the loudness result. Worse, both paths call `measure_*` only on the first and every tenth callback (`:390-404, 869-889`). `AutoGain::measure_input/output` feed every supplied frame directly to the persistent EBU R128 monitor (`sotf-host/src/auto_gain.rs:191-230`), so nine callbacks disappear from the measurement timeline while oversized scratch tails are inserted. Momentary/short-term windows, displayed LUFS, peak timing, gain convergence, and nonlinear/time-varying path matching are therefore block-size- and history-dependent.

Feed `&buffer_[a|b][..expected_samples]` to both monitors on every callback; throttle only `RealTimeCache` publication/UI work. Add streaming-reference tests that render identical audio with 32/128/1024/4800-frame blocks and irregular partitions, compare against direct `AutoGain`/EBU input, switch from a large to small block, and prove adding a unity nested path does not change LUFS or compensation.

### P1 — Bypass violates the plugin's declared latency and can advance audio by an entire nested path

The plugin reports `max(path_a_latency, path_b_latency)` to the host (`abcompare_plugin.rs:552-554, 975-980`) and normally delays the shorter path to that latency (`:494-534, 865-867`). Bypass instead copies the undelayed input and returns before either delay line (`:835-839`). In a compiled graph, toggling bypass on a 2048-sample-latency comparison advances this node by 2048 samples while the host continues compensating it as a latent plugin, breaking alignment with parallel branches and causing a discontinuity. Bypass also freezes both path states, loudness/gain/mix smoothers, and diagnostic cache even though monitoring promises a live bypass state (`USAGE.md:47-54`).

Keep a dedicated dry delay equal to the reported plugin latency (or make bypass a host structural recompile, which is much less suitable for an A/B switch), crossfade into/out of bypass, and define whether nested states continue running. Add impulse tests with latency on A, on B, and equal latency, both in isolation and a parallel host graph; assert the impulse position and reported latency are invariant across bypass transitions.

### P1 — Initial path construction cannot use the advertised plugin catalog

The documentation promises Expander, Denoiser, Loudness Compensation “and others” in nested paths (`USAGE.md:19-30`). `ABComparePlugin::from_params` constructs both paths immediately through the six-type fallback factory (`abcompare_plugin.rs:99-103`; `factory.rs:18-80`). The canonical SOTF factory injects the full factory only *after* `from_params` returns (`sotf-plugins/src/factory/create.rs:425-431`), so an initial preset containing any other allowed plugin fails before injection. The plugins-bridge factory never injects the authoritative factory at all (`plugins-bridge/src/factory.rs:236-239`), leaving AU/VST/bridge runtime path changes limited to the same six types.

Make the external factory a constructor input and build initial paths only after it is installed; expose the same authoritative factory through bridge creation. Keep the fallback only for explicit standalone/test construction and document its capability. Add canonical and bridge factory tests whose initial Plugin/Rack/Graph paths contain several allowed plugins outside the fallback set, then change those paths at runtime and render audio.

### P1 — Path changes are advertised as realtime parameters but parse, allocate, rebuild graphs, and change latency in place

Static specs make the two path controls structural `FilePath` parameters (`params.rs:92-95`; `sotf-host/src/param_specs/param_spec.rs:153-165`), but runtime metadata builds them with `Parameter::new_string`, whose default update mode is Realtime, and never overrides it (`abcompare_plugin.rs:298-315`; `sotf-host/src/parameters.rs:216-233`). Their setter parses arbitrary JSON, allocates nested plugins/hosts, builds graphs, and resizes delay lines (`abcompare_plugin.rs:685-699, 319-340, 494-534`; `delay_line.rs:17-26`). It can also change `latency_samples()` after the outer host plan was compiled. All ordinary scalar setters then call `rebuild_cached_parameters`, reallocating the parameter vector, descriptions/groups, and serialized path Strings (`abcompare_plugin.rs:188-316, 703-704`), so even `mix` automation is not allocation-safe.

Enforce structural updates at the host boundary: prepare a complete initialized A/B state off-thread, recompile outer latency compensation, then swap it at a safe boundary. Set runtime update modes from `PARAMS`. Keep immutable parameter metadata separate from current values and do not rebuild it for scalar automation. Add allocation-count tests for every realtime setter, host-plan tests for path latency changes, and a guard proving structural setters are never called from the callback.

### P1 — Failed path changes are non-transactional and expose configuration that is not being processed

The setters assign `path_[a|b]_config` before attempting a fallible rebuild (`abcompare_plugin.rs:685-699`). If plugin creation fails, the old `DawHost` remains but `get_parameter` serializes the new invalid config (`:707-734`). If host build/latency compensation fails after assignment, a partly replaced host may remain while both delay lines are forced to zero (`:319-340, 500-534`). `initialize` similarly writes the new sample rate and rebuilds A, then B, then AutoGain/filter state through separate fallible steps (`:739-767`), allowing mixed-rate partial state on error.

Build and validate a complete candidate—both hosts, latency plan/delays, AutoGain, filters, and cached representation—before committing any field. On failure preserve the previous render and every getter bit-for-bit. Add failure-injection tests for unknown plugins, malformed/cyclic graphs, nested initialize failure, and delay allocation/latency changes, asserting complete rollback.

### P1 — Deserialized construction bypasses the public parameter constraints and can create panic-prone DSP state

`ABComparePluginParams` accepts raw values with serde but has no validation (`config.rs:76-142`), and `from_params` forwards mix/timing/auto-gain values directly to smoothers and `AutoGain`; only the two band edges are individually clamped (`abcompare_plugin.rs:99-141`). This bypasses the runtime/static ranges. For example, a negative `max_auto_gain_db` is stored unchanged by `AutoGain::new` (`sotf-host/src/auto_gain.rs:93-120`); the next valid loudness measurement calls `target.clamp(-max, max)` with minimum greater than maximum and can panic (`:219-222`). Out-of-range/NaN mix or timing can poison smoother/math state, and an arbitrary binary `selected_path` silently behaves as B. The canonical and bridge factories both deserialize directly into this type.

Make construction fallible through one canonical validator shared with `set_parameter`: reject non-finite values, enforce all ranges/enums, validate sample rate/channel count and band ordering/Nyquist, and reject unknown fields where compatibility permits. Test every bound plus NaN/Inf through the Rust constructor, canonical factory, bridge/state restore, and older-preset migration.

### P1 — The generated schema, engine settings, presets, and runtime parameter surface have already diverged

Runtime metadata/set/get expose `band_mask_low_hz` and `band_mask_high_hz` (`abcompare_plugin.rs:278-297, 667-683, 727-728`), but the purported single-source `PARAMS` ends at `difference_mode` (`params.rs:29-102`). Consequently `PluginSettings::ABCompare`, its accessor macro, defaults, UI layout, FFI map, and engine converter omit both band controls (`plugin_settings.rs:1078-1124, 1942-1961`; `plugin_param_accessors.rs:362-371`; `plugin_config_converter/effects.rs:449-500`). Saved engine presets cannot represent them and generated UIs cannot edit them. Existing parity tests only verify spec→runtime keys and therefore cannot detect extra runtime parameters (`param_parity_tests.rs:212-274`). The two models also disagree on minimum gain smoothing (static 1 ms vs runtime 10 ms) and mix transition (static 1 ms vs runtime 5 ms; `params.rs:69-91`; `abcompare_plugin.rs:246-265`).

Choose one serializable parameter model and generate runtime metadata, engine settings/accessors/conversion, UI, FFI, and preset defaults from it. Add bidirectional exact parity (keys, types, defaults, ranges, units, update modes), factory round-trips with every non-default field, and migration tests. File-backed UI state should be explicitly separate from the JSON path configuration rather than represented as the same parameter.

### P2 — Reset preserves band-mask IIR history instead of clearing it

`reset` claims to reset the band-mask filters but calls `rebuild_band_mask_filters` (`abcompare_plugin.rs:770-794`). When vectors already have the correct channel count, that helper explicitly updates coefficients in place and preserves delay state (`:443-466`). A reset during an active mask therefore leaks the pre-reset filter tail into the next render, violating deterministic reset and making fresh-instance versus reset output differ.

Add/reset Biquad state explicitly (or reconstruct filters outside realtime), while retaining current coefficients. Test an impulse/step that fills both HP and LP histories, reset, then compare the following silence and new impulse sample-for-sample with a fresh initialized plugin at every supported sample rate/channel count.

### P2 — Equal-power blending is the wrong default law for two correlated versions of the same signal

At the default centre mix, identical A and B are summed with `cos(pi/4)+sin(pi/4)=sqrt(2)`, a +3.01 dB boost (`abcompare_plugin.rs:913-935`); the empty-path fast path caches the same gain (`:181-185, 406-420`). Tests explicitly bless the boost (`src/tests.rs:181-197`; `tests/integration.rs:247-266`). A/B paths normally share the same source and remain strongly correlated, so the law advertised as preventing dips (`USAGE.md:148`) instead changes loudness, consumes headroom, and can exceed ±1 before downstream processing. Correlation/phase changes across frequency can also create a moving comb response during the blend.

Use a unity-preserving linear/correlation-aware crossfade by default for same-source comparison, or expose the law and label equal-power as appropriate for uncorrelated material. Add identical/inverted/uncorrelated path tests for gain, peak headroom, correlation, and spectral cancellation across the full mix sweep.

### P2 — Oversized callbacks allocate on the audio thread and the allocation test exercises only the shortcut

`process` grows both scratch vectors with `Vec::resize` when needed (`abcompare_plugin.rs:846-855`), explicitly allowing callback allocation. In practice construction preallocates one second per path (`:167-168`), not the documented 4096-frame amount (`:758-765`), so the failure appears only above 48,000 frames until a previous resize changes the threshold. The dedicated zero-allocation test uses default empty paths (`realtime_allocation_tests/tests.rs:350-371`), which take the fast path before the resize and nested-host processing (`abcompare_plugin.rs:841-844`); it proves neither the main path nor measurement publications are allocation-free.

Negotiate maximum block size during setup, preallocate exactly that capacity, and return a checked error for an unsupported larger realtime block; offer a separate offline mode if growth is required. Expand allocation tests to non-empty Plugin/Rack/Graph paths, active band mask, auto-gain publication blocks, latency compensation, every standard channel width, and the exact maximum/+1 boundary.

### P2 — Band-mask validation ignores cutoff ordering and Nyquist, while live coefficient jumps preserve incompatible state

Both cutoffs are independently limited to the fixed range 20-20,000 Hz (`config.rs:135-141`; `abcompare_plugin.rs:117-140, 667-683`). There is no `low < high` rule and no `cutoff < sample_rate/2` rule. At low sample rates, activating one edge also processes the other filter even if its 20 kHz cutoff is above Nyquist (`abcompare_plugin.rs:939-949`). Runtime changes replace coefficients immediately while intentionally retaining old delay state (`:443-466`), which is not inherently click-free for a large filter change and can produce a transient.

Validate an ordered passband against a safe Nyquist margin at construction, initialize, and parameter update; make updates transactional. Smooth/morph coefficients or crossfade old/new filter states. Add low≥high, 32/44.1/48/96/192 kHz, cutoff-near-Nyquist, rapid automation, impulse stability, finite-output, and post-update transient-bound tests.

### P2 — Diagnostic publication is block-count-dependent and bypass state can remain stale indefinitely

Cache publication happens only on the first/every tenth processed non-bypass block (`abcompare_plugin.rs:869-889, 952-966`), making UI cadence range from milliseconds to seconds with host block size. Bypass returns before publishing, so `ABCompareData.bypass_active` can remain false for the entire bypass interval despite the documented monitoring field (`:835-839`; `USAGE.md:47-54`). `reset` also leaves `cache_update_counter` unchanged (`abcompare_plugin.rs:770-809`), so post-reset publication timing depends on pre-reset history.

Schedule publication from elapsed frames/time rather than callback count, publish control-state changes immediately without doing loudness work, and reset the scheduler deterministically. Test cache trajectories across block partitions, held `Arc` readers, bypass toggles, and reset.

### P3 — Memory and performance evidence do not cover the actual workload

Each instance allocates two one-second interleaved buffers regardless of host maximum (`abcompare_plugin.rs:167-168`): about 0.4 MiB at stereo and 4.6 MiB at 12 channels, before two nested hosts, two EBU monitors, filters, delays, and cache/schema Strings. The normal callback always renders both complete paths and performs per-frame auto-gain exponential approximation and trigonometric crossfade, then optionally two biquads per sample (`:857-949`). There is no plugin-owned Criterion benchmark; the QA binary runs only standard tests on the default empty-path shortcut (`bin/qa_ab_compare.rs:8-24`). Catalog “zero alloc” evidence cites functional tests rather than representative callback configurations (`factory/catalog.rs:1184-1201`).

Size buffers from the negotiated block maximum and profile the full matrix: 1/2/4/6/8/12 channels, empty/plugin/rack/graph paths, binary and transitioning mixes, auto-gain publication blocks, latency differences, band mask, and large offline blocks. Report worst callback time and allocation count, not only average throughput. Consider advancing sine/cosine gains recursively or with a paired smoother and document the deliberate CPU/state tradeoff of keeping the inaudible path warm.

### P3 — The main struct exceeds the repository field budget and mixes unrelated responsibilities

`ABComparePlugin` has 31 fields spanning graph ownership, control state, loudness matching, delay compensation, scratch, band filtering, diagnostics, and schema cache (`abcompare_plugin.rs:22-83`). That exceeds the repository's 30-field approval threshold and makes transactional updates/reset invariants difficult to reason about.

Decompose it into focused `PathPair`, `MixState`, `BandMaskState`, and `DiagnosticsState` sub-structs, with a documented ownership/update plan. This also creates natural units for atomic path swaps and deterministic reset tests.

## Strengths

- The basic signal topology is intelligible: two independently hosted paths, explicit shorter-path delay, B-to-A loudness correction, polarity/null controls, and a final mask. Difference mode and latency compensation have useful focused tests.
- Normal buffer lengths are checked before processing, outputs are overwritten, the callback returns exactly `context.num_frames`, and delay processing is allocation-free once sized (`abcompare_plugin.rs:811-839, 857-968`; `delay_line.rs:29-42`).
- Nested graph building validates unknown edge endpoints and delegates cycle detection to `DawHost::build`; channel-route destination offsets have a concrete stereo routing test (`factory.rs:133-177`; `src/tests.rs:983-1016`).
- AutoGain uses persistent EBU R128 monitors, finite loudness guards, a bounded correction target, and sample-based attack/release advancement. The plugin exposes useful LUFS/peak/gain/mix state through an RT-oriented double buffer.
- Phase inversion, difference mode, active band masking, bypass, invalid audio sizes, runtime path JSON errors, latency alignment/reset/build failure, multichannel operation, and path/rack/graph creation all have tests.
- There are no locks, filesystem calls, or logging in the ordinary processing path, and the default shortcut passed the dedicated zero-allocation test.

## Realtime and performance assessment

The settled default empty-path shortcut is bounded and allocation-free in the observed test. The normal path has no explicit locks or I/O, but it can resize two vectors in the callback, and every scalar parameter setter rebuilds heap-backed metadata. Structural path setters parse JSON, allocate/build nested graphs, resize delays, and can change declared latency, while runtime metadata incorrectly labels them realtime. The heaviest periodic work is currently the erroneous full-capacity EBU analysis every tenth callback; after correcting it, two nested path renders plus per-sample gain/crossfade and optional filters remain the main cost.

## Focused verification

- `cargo test -p sotf-plugin-ab-compare --offline` — 103 passed across three suites.
- `cargo test -p plugins-bridge --offline ab_compare_bridge_builds_initial_path_with_authoritative_factory` — passed before an unrelated in-progress Beamformer constructor change made the shared bridge factory temporarily fail to compile.
- `cargo test -p sotf-plugins --no-default-features --offline ab_compare_facade_injects_factory_before_initial_path_build` — passed before the same unrelated Beamformer factory mismatch.
- `cargo test -p sotf-host --offline host --lib` — 84 passed.
- `cargo check -p sotf-engine --offline` — passed before the unrelated Beamformer factory mismatch.
- `cargo clippy -p sotf-plugin-ab-compare --all-targets --offline -- -W warnings` — no AB Compare warnings; three pre-existing/shared host warnings remain.
- `cargo run -p sotf-plugin-ab-compare --features qa --bin qa-ab-compare --offline` — full Plugin/Rack paths at 1/2/6/12 channels passed latency, zero-allocation, and performance checks.
- Targeted `rustfmt --check --edition 2024` over every touched Rust file — passed.

The focused regressions cover active-slice loudness and callback-partition independence, bypass and nested-path latency, authoritative-factory construction, transactional structural rejection, constructor/sample-rate validation, band-mask reset/schema parity, unity/cancellation crossfades, and representative non-empty Plugin/Rack allocation behavior.

## Coverage reviewed

Reviewed every plugin-owned file without omission: nested `AGENTS.md`, README, complete USAGE and changelog, manifest, crate root, configuration and graph models, complete built-in/external factory and graph builder, full processor, delay line/data types, complete static parameter/layout/serde model, all unit/helper and integration tests, and the QA binary. Integration review covered facade exports, canonical factory/catalog, plugins-bridge factory/state path, FFI/NIH/AU parameter exposure, engine type/settings/accessors/config conversion and render snapshots, factory/high-channel/robustness/parity/allocation tests, shared `DawHost` latency/build contract, `AutoGain`, EBU loudness monitor, smoothing, realtime cache, Biquad update behavior, and parameter update-mode defaults. Production, tests, schema, documentation, version, and lock metadata were updated as described above.
