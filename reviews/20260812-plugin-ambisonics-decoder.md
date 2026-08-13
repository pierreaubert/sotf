# Ambisonics Decoder code review — 2026-08-12

## Remediation status (2026-08-12)

### Final closure — 0.5.8

All P0–P3 findings are fixed and covered; no review item remains deferred.

| Finding | Closure and test evidence |
|---|---|
| P1 algorithm mislabeled AllRAD | Product, source, metadata, and README consistently identify the implementation as regularized mode matching and explicitly state that no virtual-speaker/VBAP stage exists. |
| P1 incorrect max-rE weights | Exact Legendre-root degree constants for orders 1–3 remain covered per degree and every ACN component, plus dense directional-response tests. |
| P1 failed/successful live structural mutation | Every changed structural value remains rejected after initialization and before mutation; order/layout/max-rE/dual-band rollback and process-after-rejection regressions preserve topology and state. |
| P1 bridge layout choice mismatch | Runtime, schema, and generic bridge all use integer choice indices. `ambisonics_layout_choice_roundtrips_every_raw_and_normalized_value` covers every layout. |
| P1 catalog omitted SOA/TOA | Catalog admission now enumerates 4/9/16 ACN/SN3D inputs with configurable output topology. Main and bridge factory tests construct orders 1/2/3 and reject width mismatches. |
| P1 8192-frame panic/limit | Frame-sized heap scratch and the limit were removed. Dual-band uses two fixed 16-sample frames; 8193-frame and allocation-counter regressions pass. Process-before-initialize remains checked. |
| P2 invalid order/factory drift | Persisted orders outside 1–3 are rejected, never clamped; both factories share exact runtime-width checks, including all 4/9/16 valid widths and mismatches. |
| P2 normal equations/rank quality | Matrix construction now uses a rank-revealing SVD pseudoinverse with scale-relative Tikhonov/rank thresholds. It exposes rank, singular extrema, condition, reconstruction error, and peak coefficient; gain above 8 is rejected. All shipped layout/order pairs, underdetermined TOA, and dense directions are covered. |
| P2 invalid crossover sample rates | Dual-band still rejects rates at or below 1400 Hz without mutation. Boundary and 44.1/48/96/192 kHz impulse regressions require finite output. |
| P2 non-finite poisoning | NaN/±Inf reject the complete block before state mutation; subnormals flush to zero before LR4 state. Poison/recovery, reset-equivalence, and subnormal regressions cover the policy. |
| P2 parameter type key drift | Runtime state is consolidated onto canonical `params::Params`; `PLUGIN_TYPE_KEY`, catalog, both factories, and aliases use `ambisonics_decoder`. |
| P3 memory/performance evidence | Fixed dual-band scratch is 32 samples rather than about 1 MiB. Benchmarks cover every output layout and single/dual worst-case TOA at 64/512/2048 frames. QA reports zero allocations and 0.13% CPU for default FOA plus zero allocations and 2.31% CPU for worst-case 16-in/16-out TOA dual-band. |
| P3 missing signal/host contract | README now defines ACN/SN3D widths, layout/LFE order, SVD degradation diagnostics, exact max-rE/dual-band behavior, sample-rate/init/structural rules, phase/latency, non-finite/subnormal policy, headroom, unbounded host-block support, and realtime guarantees. |

Final verification: `cargo test -p sotf-plugin-ambisonics` (65 passed), focused
Ambisonics realtime allocation test (passed), both plugins-bridge Ambisonics tests
(passed), the main catalog/all-order factory regression (passed),
`cargo check -p sotf-plugin-ambisonics --offline` (passed), standard QA (default
FOA zero allocations/0.13% CPU and worst-case TOA dual-band zero allocations/
2.31% CPU, passed), and the expanded Criterion benchmark target build (passed).
The final attempt to repeat the cross-crate checks was blocked during compilation
by an unrelated concurrent Beamformer factory change that boxes
`Result<BeamformerPlugin, String>` instead of the plugin; the Ambisonics bridge
and main-factory tests had already passed immediately before that change.

Implemented in the 0.5.5 remediation and 0.5.6 follow-up:

- Correct exact max-rE weights for orders 1–3 with per-degree/per-ACN goldens.
- Reject live structural changes, preserving topology and initialized state.
- Use integer choice indices for `target_layout`; align the parameter type key.
- Advertise FOA (4-channel) and SOA (9-channel) input in the built-in catalog;
  SOA uses the built-in 7.1.4 target. TOA (16-channel) is not advertised
  because the largest built-in layout has only 15 non-LFE feeds, so the
  decoder correctly rejects every current TOA target.
- Return checked errors for invalid orders/sample rates, non-finite input,
  process-before-initialize, and dual-band blocks above 8192 frames.
- Rename/document the implemented algorithm as regularized mode matching.

Deferred follow-ups: implementing true AllRAD, replacing normal equations with a
rank-revealing SVD/QR and quality policy, negotiating scratch size with the host,
and expanding spatial/bridge/performance conformance coverage. The existing
fixed scratch remains bounded and allocation-free but reserves about 1 MiB in
dual-band mode.

Verification after remediation: `cargo test -p sotf-plugin-ambisonics` (57
passed); `cargo check -p sotf-plugin-ambisonics` passed; clippy completed with no
plugin warnings (two pre-existing `sotf-host` warnings); focused realtime
allocation test passed (1 passed, 47 filtered).

Follow-up verification: `cargo test -p sotf-plugin-ambisonics --offline` (57
passed) and the factory catalog regression (1 passed). The built-in catalog is
FOA-only so its fixed 5.1 output contract is truthful; higher-order layouts
remain explicit custom configurations until matching built-in output contracts
are available.

## Findings

### P1 — The implementation is mode matching, not the advertised AllRAD/VBAP decoder

The README promises AllRAD, including VBAP remapping for arbitrary and irregular loudspeaker layouts (`README.md:3-14`), and the matrix file is headed “AllRAD Decode Matrix Builder” (`decode_matrix.rs:1-9`). The implementation instead evaluates the real loudspeaker directions directly and computes `D = Y(Y^T Y + epsilon I)^-1` (`decode_matrix.rs:46-87, 163-240`). There is no virtual spherical loudspeaker grid, triangulation, VBAP gain calculation, or virtual-to-real remapping—the defining AllRAD stage. It also rejects every layout with fewer non-LFE loudspeakers than Ambisonics components (`decode_matrix.rs:55-70`), a limitation of this chosen normal-equation solve rather than the behavior claimed for AllRAD. Users therefore select an algorithm with different irregular-layout robustness and spatial/energy behavior than the documentation promises.

Either rename and document this as a regularized mode-matching decoder, or implement actual AllRAD: choose and document a sufficiently dense virtual sphere, decode HOA to that grid, map each virtual direction through normalized VBAP gains on a validated physical-speaker triangulation, and accumulate the real-speaker matrix. Add golden matrix/directivity tests against a trusted AllRAD implementation for regular and deliberately irregular 2-D/3-D layouts, including fewer physical speakers than HOA components.

### P1 — The purported max-rE degree weights are mathematically wrong

`compute_max_re_weights` uses `cos(l*pi/(2*(N+1)))` and cites Zotter & Frank equation 10 (`decode_matrix.rs:144-160`). Standard 3-D max-rE weights are `g_l = P_l(r_E)`, where `r_E` is the largest root of `P_(N+1)` (or a documented approximation to it). For FOA the code produces `[1, 0.7071]`, while exact max-rE is `[1, 1/sqrt(3)] = [1, 0.57735]`; for second order it produces degree weights `[1, 0.8660, 0.5]` instead of `[1, sqrt(3/5), 0.4]`. Both single-band max-rE and the high-frequency half of dual-band decoding therefore use the wrong spatial weighting (`decode_matrix.rs:89-100`; `ambisonics_decoder_plugin.rs:327-379`). Existing tests only check broad non-zero/different-output properties and never assert reference weights or energy-vector maxima.

Implement the Legendre-polynomial/root definition for supported orders (precomputed exact constants for orders 1-3 would be simple and realtime-neutral), correct the citation, and add exact per-degree golden tests plus directional energy-vector tests over a dense sphere. Include a test proving every ACN component of a degree receives the same weight.

### P1 — Failed structural updates leave a self-contradictory live plugin

The `order` setter assigns `self.order` before attempting a fallible matrix rebuild (`ambisonics_decoder_plugin.rs:203-213`). On a default 5.1 decoder, setting order 2 fails because five non-LFE speakers are fewer than nine components (`decode_matrix.rs:55-70`), but leaves `order == 2` while `input_channels`, the decode matrix, and cached parameters still describe FOA. `target_layout` has the same write-before-rebuild pattern (`ambisonics_decoder_plugin.rs:215-227`): selecting a known but insufficient layout can leave the new label attached to the old topology. Subsequent reads, serialization, host graph decisions, and processing no longer agree.

Construct a complete candidate state first and commit it only after every validation/allocation succeeds, or restore all fields on error. Add regression tests for each failing structural transition asserting that every getter, channel count, matrix, schema value, and rendered output remains bit-identical to the pre-call state.

### P1 — Successful live structural updates can panic in the next audio block

All four parameters are correctly marked structural (`params.rs:25-43`), but the public `Plugin::set_parameter` still mutates active state in place. Enabling `dual_band` after `initialize` creates matrices and a crossover (`ambisonics_decoder_plugin.rs:99-106, 240-248`) but does not allocate `lf_buffer`/`hf_buffer`; those buffers are allocated only by `initialize` (`:266-285`). The next callback passes only debug assertions and then indexes empty buffers (`:334-360`), panicking in release too. Changing the target layout while dual-band is active similarly changes `output_channels` without resizing `lf_frame`/`hf_frame` (`:90-111`); the summation loop then indexes the old-size frame buffers (`:367-379`). The existing toggle test stops after checking `basic_matrix` and `crossover` and never processes a block (`:749-790`).

Require structural changes to build a new initialized instance and trigger host graph recompilation, or make the setter explicitly unavailable while active. If in-place transitions remain supported, prepare every matrix/filter/scratch buffer off-thread and atomically swap a complete state with the new input/output contract. Add initialize→toggle→process and initialize→every-layout-transition→process tests in debug and release, plus a host-plan test that verifies channel topology is rebuilt before the callback sees the state.

### P1 — The AU/VST/bridge choice control cannot set or read the target layout

The static model declares `target_layout` as `ParamSpec::choice` (`params.rs:29-37`), so the generic bridge converts host values to `ParameterValue::Int` (`plugins-bridge/src/param_bridge.rs:277-285`). The runtime setter accepts only `ParameterValue::String`, while its getter returns String (`ambisonics_decoder_plugin.rs:215-218, 256-263`); the reverse bridge maps any String to raw zero (`plugins-bridge/src/param_bridge.rs:288-301`). The FFI parameter map is backed by this bridge, so exported hosts cannot select a non-default layout and reads always appear as choice zero.

Use one representation end to end—prefer the choice index in runtime parameter I/O, converting to a layout ID at the configuration boundary. Add raw and normalized FFI/ParamBridge round-trip tests for every target layout and an end-to-end exported-plugin test that confirms the output channel topology follows the selection.

### P1 — Catalog metadata advertises only FOA although the plugin supports SOA and TOA

The catalog registers the decoder with `FIRST_ORDER_AMBISONIC_WIDTH`, defined as only `[4]` (`factory/catalog.rs:91-95, 1241-1254`), while the public order parameter supports 1-3 and the runtime exposes 4, 9, or 16 input channels (`params.rs:25-28`; `ambisonics_decoder_plugin.rs:49-61`). Any host that uses catalog metadata for compatibility filtering or graph admission can hide/reject valid SOA and TOA configurations before the plugin's own checks run.

Advertise `[4, 9, 16]` under an Ambisonics-specific constant, and make catalog compatibility conditional on the selected structural order where possible. Add catalog/factory tests that create all three orders with suitable layouts and prove admission metadata agrees with `input_channels()`.

### P1 — Blocks above 8192 frames turn a documented host assumption into a release callback panic

Dual-band scratch is fixed at `8192 * 16` samples per band (`consts.rs:4-11`; `ambisonics_decoder_plugin.rs:274-280`). `process` checks capacity only with `debug_assert!` and immediately indexes it (`ambisonics_decoder_plugin.rs:334-360`). A release host block of 8193 frames therefore panics rather than returning `PluginResult::Err`; calling dual-band `process` before `initialize` has the same failure mode. The large-block regression stops at 5000 frames (`ambisonics_decoder_plugin.rs:714-746`), so it does not exercise the boundary.

Make maximum block size part of an explicit initialization contract, size scratch from the host-provided maximum, and retain a normal checked error before indexing. Test exactly 8192/8193 frames, multiplication overflow, and process-before-initialize in both debug and release profiles. A callback error still needs host handling, but it avoids unwinding/abort from an audio thread.

### P2 — Invalid configured orders are silently clamped and can be misreported by factories

Construction clamps every serialized order into 1-3 instead of rejecting it (`ambisonics_decoder_plugin.rs:49-51`). The canonical factory then compares the clamped runtime width with the supplied channels but prints the original order in its error (`factory/create.rs:467-479`), so order 0 with four channels silently becomes FOA, while a mismatch can report “Order-0 … requires 4.” The bridge factory constructs and initializes without checking its `channels` argument at all (`plugins-bridge/src/factory.rs:295-300`). This makes malformed presets non-round-trippable and creates inconsistent behavior across entry points.

Validate `1..=3` before construction, reject rather than normalize persisted structural state, and share one channel-contract check across both factories. Add malformed JSON tests for 0, 4, `usize::MAX`, negative/non-integer JSON, and mismatched 4/9/16-channel factory requests.

### P2 — Fixed normal-equation regularization hides rank loss without measuring decoder quality

The decoder forms `Y^T Y`, adds a fixed absolute `1e-6` diagonal, and inverts it with hand-written Gauss-Jordan elimination (`decode_matrix.rs:163-240`). Forming normal equations squares the condition number. The code intentionally accepts rank-deficient planar layouts—5.1 has a zero vertical harmonic column (`:165-172`)—but reports neither rank/condition nor which Ambisonics components are discarded; near-degenerate layouts can instead create very large gains. A speaker-count check does not establish geometric rank or acceptable decoder energy.

Use a rank-revealing SVD/QR pseudoinverse (or a small, well-tested linear-algebra routine), scale regularization relative to singular values, and define an explicit rank/condition/gain policy. Add singular-value, reconstruction-error, peak-coefficient, diffuse-field energy, and directional response tests for every shipped layout/order; surface a useful error or degradation report when full 3-D decoding is impossible.

### P2 — Dual-band initialization accepts sample rates at which the 700 Hz crossover is invalid

`initialize` accepts any `u32` and constructs a fixed 700 Hz LR4 crossover without checking zero/finite Nyquist margin (`ambisonics_decoder_plugin.rs:266-285`; `consts.rs:1-2`). Rebuilds also use the stored sample rate unconditionally (`ambisonics_decoder_plugin.rs:99-106`). At zero or at rates at/below 1400 Hz, coefficient design is undefined or places the cutoff at/above Nyquist, risking non-finite coefficients/output and poisoned IIR state.

Reject unsupported sample rates before changing state and validate the shared crossover constructor's frequency contract. Add coefficient/output finiteness and complementary-sum tests around the lower boundary and at 44.1/48/96/192 kHz.

### P2 — One non-finite sample can permanently poison the dual-band filter state

The dual-band loop passes input directly into persistent IIR state and writes its result without a finite guard (`ambisonics_decoder_plugin.rs:352-379`). A NaN/Inf sample can therefore contaminate the crossover state and all later blocks even after finite input resumes. `reset` clears crossover state (`:288-294`), but no test defines recovery or rejection behavior, and there is no denormal handling visible in this plugin path.

Define the project-wide non-finite policy at the plugin boundary—reject the block, sanitize samples before stateful processing, or reset affected channel state—and enable/verify the normal FTZ/DAZ callback policy. Test isolated NaN/+Inf/-Inf in every Ambisonics channel followed by finite blocks, as well as subnormal tails and reset equivalence.

### P2 — The canonical parameter type key disagrees with every factory/catalog key

`PluginParamDef::PLUGIN_TYPE_KEY` is `"ambisonics"` (`params.rs:116-120`), while the canonical catalog and factories use `"ambisonics_decoder"` (`factory/catalog.rs:1241-1249`; `factory/create.rs:467-471`). Code that keys generated layouts, snapshots, preset migrations, or parameter registries by `PluginParamDef` can therefore diverge from discovery and construction.

Use the canonical `ambisonics_decoder` identifier everywhere and add a registry invariant test that every exported parameter definition, catalog entry, factory alias, and engine type maps to the same stable key.

### P3 — Performance evidence omits the expensive modes and over-allocates fixed scratch

The initialized dual-band instance reserves two `8192 * 16` float buffers regardless of actual order or host block size—about 1 MiB total (`ambisonics_decoder_plugin.rs:274-280`)—then filters every Ambisonics component and performs two scalar matrix decodes per frame (`:352-379`). The benchmark covers only single-band FOA/SOA and even compares max-rE on/off although weighting is baked into the matrix and has identical callback cost; it contains no dual-band or worst-case 16-in/16-out case (`benches/ambisonics-benchmark.rs:40-79`).

Allocate from the negotiated maximum block and actual input width. Benchmark single/dual band for 4/9/16 inputs, every output layout, representative block sizes, and reset/parameter rebuild costs; report worst callback time as well as throughput. If profiling justifies it, specialize the small dot products, decode blocks speaker-major for better coefficient reuse, or use a proven matrix kernel while preserving interleaved I/O and zero allocation.

### P3 — Documentation and QA do not define the real signal/host contract

The README contains only the incorrect AllRAD/VBAP claim and a file list (`README.md:1-31`). It omits ACN/SN3D input convention, supported orders/layouts, LFE-zero policy, regularization/rank loss, exact max-rE definition, dual-band phase behavior, required initialization, maximum block size, structural-rebuild rules, output gain/headroom, and realtime guarantees. The QA/bench coverage does not establish reference spatial metrics, bridge round-trips, factory parity, structural transition safety, or worst-case timing.

Document those contracts and promote the golden, topology, boundary, and performance cases above into focused CI/QA. Keep algorithm names tied to conformance tests so a future decoder change cannot leave product claims behind.

## Strengths

- The input convention and spherical-harmonic implementation are explicitly ACN/SN3D, bounded to orders 1-3, and covered at cardinal directions plus approximate orthogonality. Matrix construction occurs outside the callback.
- LFE is handled correctly: it is excluded from the directional solve and its full-matrix row remains zero (`decode_matrix.rs:55-59, 104-118`). This avoids treating the subwoofer as a coincident centre loudspeaker.
- Audio-buffer sizing is checked through the shared overflow-aware interleaved-I/O validator before normal processing (`ambisonics_decoder_plugin.rs:296-312`), and the single-band callback is a compact bounded fused multiply-add loop (`decode_matrix.rs:122-140`).
- The normal initialized process paths contain no locks, logging, file I/O, FFT planning, or observed heap allocation. The dedicated allocation test covers both single- and dual-band at 1024 frames (`realtime_allocation_tests/tests.rs:994-1011`) and passed.
- Reset clears crossover state and scratch, matrices are built in `f64` before conversion to `f32`, and tests cover silence, basic channel contracts, invalid layouts, buffer length, omni distribution, large blocks within the fixed limit, dual-band behavior, and reset determinism.
- The main plugin struct remains below the 30-field budget, and plugin-owned source is separated into configuration, parameters, spherical harmonics, matrix construction, and runtime processing.

## Realtime and performance assessment

For a correctly initialized, unchanged topology and blocks no larger than 8192 frames, both callback paths are bounded and passed the zero-allocation test. Single-band cost is approximately `frames * outputs * (order+1)^2` scalar FMAs. Dual-band additionally runs two biquad crossover branches per Ambisonics channel and two matrix decodes, so it is the dominant case. The callback has no synchronization or I/O, but its oversized-block and post-structural-update paths can panic, and stateful non-finite handling is undefined. Matrix/schema/crossover rebuilds allocate heavily and must remain outside the audio thread.

## Focused verification

- `cargo test -p sotf-plugin-ambisonics` — 51 passed across three suites.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_ambisonics_decoder_zero_alloc` — 1 passed, 45 filtered out.
- `cargo test -p sotf-plugins ambisonics_decoder` — 1 passed, 278 filtered out across 32 suites.

These passing tests establish the existing baseline. They do not contradict the findings: there are no AllRAD/max-rE reference goldens, bridge choice round-trips, failed-update rollback assertions, post-toggle processing tests, catalog admission tests for 9/16 channels, or 8193-frame boundary tests.

## Coverage reviewed

Reviewed every plugin-owned file without omission: nested `AGENTS.md`, README, complete changelog, manifest, crate root, configuration, static parameter/layout/serde definition, constants/types facade, complete ACN/SN3D spherical-harmonic implementation and tests, complete decode-matrix builder/inverter and tests, complete plugin implementation and unit tests, integration suite, QA binary, and Criterion benchmark. Integration review covered the public facade, canonical factory and catalog, plugins-bridge factory/state/parameter bridge, FFI parameter mapping, NIH/AU exposure, engine spatial configuration conversion/settings/accessors/type mapping, shared speaker layouts and interleaved-I/O validation, focused factory/high-channel/parameter/allocation tests, and the shared LR4 crossover implementation/tests. No production code was changed and no broad workspace build was run.
