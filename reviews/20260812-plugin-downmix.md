# Downmix plugin code review

Date: 2026-08-12

Scope: `sotf-plugin-downmix` plus its factory, catalog, engine, host, FFI, and
Audio Unit integration

Focus: correctness, algorithm quality, realtime allocation, and performance

## Remediation status

Engine/factory follow-up after 0.5.28 closes a default-construction regression:

- Default engine settings now leave `input_layout` unspecified so the factory
  can adapt them to an unambiguous live chain width instead of carrying a stale
  `5.1` identity into stereo construction.
- Explicit layout/channel mismatches are rejected before factory adaptation,
  and unspecified ambiguous 8/10-channel inputs remain rejected rather than
  guessed. When an upstream engine plugin supplies a known layout such as
  `5.1.4`, channel propagation preserves it into Downmix construction.
- Focused engine regressions cover adaptive stereo defaults, ambiguous-width
  rejection, explicit mismatch rejection, and valid known-layout 10-channel
  construction.

Remediated in version 0.5.27:

- The P1 partition-dependent WOLA timeline now advances sample by sample behind an explicit fixed 2048-sample startup delay. Regression coverage compares one-block and highly variable partitions exactly.
- `phase_coherence` and the newly canonical `matrix_ltrt` parameter are structural. Construction rejects simultaneous selection instead of silently ignoring Lt/Rt.
- Lt/Rt surround encoding now performs an exact unity-magnitude spectral quadrature rotation in the fixed-latency WOLA path. Tests pin matrix gain, opposite Lt/Rt polarity, and variable-block behavior.
- Construction and processing now validate channel counts, finite/ranged parameters, crossover ordering, checked dimensions, sample rate/Nyquist, and exact buffer sizes before mutation. The factory uses fallible construction.
- Compile metadata is now a conservative stateful boundary, and package/plugin/changelog versions agree.
- Engine `PluginSettings` accessors, defaults, channel-adaptation copies, and
  config conversion now include the canonical `matrix_ltrt` entry. A regression
  test verifies index 8 round-trips to the serialized DSP configuration.

Final closure in version 0.5.28:

- Explicit `input_layout` now selects the routing matrix. Ambiguous 8- and
  10-channel inputs without a layout are rejected, and engine/factory/AU
  construction carries layout identity. The player CLI preserves an explicit
  `--downmix-input-layout` value and intentionally leaves it unspecified for
  unambiguous channel widths. Unit coverage distinguishes 7.1 from 5.1.2
  routing and checks the unknown-layout error and CLI builder behavior.
- Silent global normalization was removed. Published coefficients remain exact,
  documentation states that downstream headroom/limiting is required, and a
  regression test pins unity front routes while the other gains are maximal.
- Phase alignment now preserves the ordinary downmix bin magnitude, uses a
  bounded phase-vector confidence gate, and normalizes complex vectors directly
  without callback `atan2`/`sin`/`cos`. Correlated and offset-phase test signals
  pin the magnitude bound.
- Coefficient recomputation fills preallocated arrays, cached parameter values
  update in place, blend controls avoid unrelated recomputation, structural
  changes after initialization require plugin reconstruction, and `reset()`
  resets biquads in place. The QA allocation counter covers setters and reset.
- Sample-rate reinitialization clears WOLA and filter history; regression
  coverage compares a reused dirty instance with a fresh instance exactly.
- Generic AU render state now tracks independent input/output widths, pulls into
  a dedicated input buffer, sizes scratch storage for both buses, and
  deinterleaves using the output width. Downmix publishes N→2 capabilities,
  derives construction JSON from the Core Audio layout tag, and the FFI factory
  rejects width mismatches. Rust FFI coverage pins 6→2 construction.

All P0–P3 findings in this review are closed; none remain deferred.

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-downmix/`. Other references are
repository-relative.

## Findings

No P0 issue was found. Several P1 issues break the advertised streaming,
latency, Lt/Rt, and Audio Unit contracts.

### P1 — Phase-coherent output placement and effective latency depend on host block size

The STFT path does not maintain a fixed-latency output timeline. It accumulates
input until a complete 2048-sample FFT frame exists, produces only one ready
1024-sample hop, and drains that hop immediately into the *current* callback
(`src/lib/downmix_plugin.rs:841-897`). The output queue is initially empty rather
than prefilled with the declared latency, yet `latency_samples()` always reports
2048 when phase coherence is enabled (`src/lib/downmix_plugin.rs:936-938`).

For 512-frame callbacks, the first three calls are zero and the first processed
samples appear in the fourth call: effective startup delay is 1536 samples. For
1024-frame callbacks the first processed hop appears in the second call: delay
is 1024. For a single 2048-frame call, the first processed hop is written at the
start of that same output block and its second half remains zero. The same input
stream therefore moves in time and has different startup holes solely because
the host partition changed. Returning `context.num_frames` (`:899`) preserves
the buffer length but does not preserve the signal timeline.

The existing small-block test asserts only that unavailable samples were zeroed
(`src/lib/tests.rs:421-449`), the COLA tests discard startup, and the cross-plugin
block-size matrix checks only success/finiteness. None compare the actual stream
under different partitions.

Fix: use an explicitly pre-zeroed output-delay FIFO and place each WOLA hop on a
single sample timeline. Always drain exactly the requested frames, with a fixed
latency equal to the measured analysis/synthesis delay. Add impulse and
time-coded-ramp reference tests for 1, 16, 31, 64, 257, 480, 512, 1024, 2048,
4093, and variable block sequences; compare the concatenated outputs sample for
sample after compensating by the one reported latency. Include reset and end-of-
stream flush behavior.

### P1 — `phase_coherence` is advertised as realtime automation even though it changes topology and latency

All parameter specs default to `UpdateMode::Realtime`; the phase toggle is not
marked structural (`src/params.rs:21-92`; host default behavior at
`crates/sotf-plugins/crates/sotf-host/src/param_specs/param_spec.rs:82-102`). The
setter flips the boolean and recomputes coefficients but does not clear or align
the STFT input/output state (`src/lib/downmix_plugin.rs:163-179,774-778`). This
switches between zero-latency scalar processing and the claimed 2048-sample STFT
latency immediately (`:823-899,936-938`).

Enabling produces an un-crossfaded startup gap. Disabling abandons queued WOLA
audio. Re-enabling later can drain stale accumulator/input state because the
setter never resets those buffers. The compile metadata and graph latency are
normally captured during graph construction, so changing `latency_samples()` on
an existing instance also invalidates compensation. The integration test
explicitly celebrates the changing latency value without testing audio or graph
alignment (`tests/integration.rs:166-190`).

Fix: mark the mode structural and rebuild/replace the plugin off the audio
thread, or keep both paths latency-aligned and preinitialized and crossfade at a
safe boundary while preserving one constant reported latency. Test automation
at every callback/hop boundary, repeated on/off transitions, parallel dry/wet
null alignment, stale-tail absence, host graph recompilation, and click bounds.

### P1 — The Lt/Rt “90-degree” branch is strongly high-pass in magnitude, not a unity-magnitude Hilbert transform

Each surround signal is encoded as the difference between a two-stage all-pass
chain and a one-sample delay (`src/lib/lt_rt_allpass.rs:4-23,48-59`), then used
directly at a fixed -3 dB coefficient
(`src/lib/downmix_plugin.rs:540-563`). Although each branch individually has
unit magnitude, their *difference* does not. Its magnitude approaches zero when
the branches have similar low-frequency phase and rises toward two at high
frequency. A transfer-function evaluation of the checked-in coefficients at 48
kHz gives approximately 0.026 at 200 Hz, 0.129 at 1 kHz, 0.510 at 4 kHz, and
0.985 at 8 kHz before the 0.707 matrix coefficient. Dialogue-band surround
content is therefore attenuated by roughly 18–38 dB more than the documented
matrix, while upper treble approaches the intended level.

The phase-only test (`src/lib/tests.rs:452-522`) cannot detect this: it measures
angle but never transfer magnitude, broadband energy, or decode compatibility.
The implementation comments themselves derive a difference whose magnitude is
frequency dependent (`src/lib/lt_rt_allpass.rs:13-17`) but then call it a
broadband phase shifter.

Fix: implement a genuine complementary all-pass Hilbert pair with matched
magnitude and documented delay, or use a tested FIR Hilbert transformer and
latency-align the direct paths. Validate complex transfer function magnitude
and phase at logarithmically spaced frequencies for every supported sample
rate. Add encode/decode tests with isolated L, R, C, Ls, and Rs; measure
separation, gain, frequency response, and polarity against a Dolby/Pro Logic
reference matrix.

### P1 — Lt/Rt response changes with sample rate despite documentation claiming proportional tuning

The corner constants remain fixed at 100 and 132 Hz
(`src/lib/consts.rs:7-10`), and both construction and sample-rate update pass
those same Hz values to coefficient calculation
(`src/lib/lt_rt_allpass.rs:31-45`). The comments claim the corner frequencies
are proportional to sample rate and preserve `fc/fs` (`consts.rs:7-9`;
`lt_rt_allpass.rs:22-25`), but fixed Hz does the opposite. At 96 kHz, for
example, the checked-in difference network's magnitude at 8 kHz is about 0.514,
versus 0.985 at 48 kHz. The matrix tonal balance therefore changes substantially
with session rate.

Fix: decide whether the design is specified in absolute acoustic frequency or
normalized digital frequency, then make code and documentation agree. For a
sample-rate-invariant audio-band Hilbert response, redesign coefficients for
each rate rather than merely scaling a questionable prototype. Test magnitude,
phase, group delay, and decoded separation at 44.1, 48, 88.2, 96, and 192 kHz.

### P1 — Lt/Rt is silently ignored under the default phase-coherent mode and is absent from the canonical parameter surface

`matrix_ltrt` exists only in the legacy construction struct
(`src/lib/types.rs:12-35`). The canonical `PARAMS`, serializable `Params`, plugin
getter/setter/cache, engine settings/converter, and FFI parameter map contain
only eight other controls (`src/params.rs:21-92,138-235`;
`crates/sotf-engine/src/plugins/plugin_settings.rs:1158-1176`;
`crates/sotf-engine/src/plugins/plugin_config_converter/spatial.rs:316-340`). It
cannot be enabled or preserved through normal UI automation/preset workflows.

Worse, matrix processing is reached only from `process_simple` when
`phase_coherence` is false (`src/lib/downmix_plugin.rs:483-487,823-834`). The
default phase setting is true (`src/params.rs:66-67`), so JSON containing only
`"matrix_ltrt": true` silently runs the ordinary phase-coherent downmixer and
never encodes Lt/Rt. The sole processing test explicitly sets phase coherence
false and merely checks for any nonzero output (`src/lib/tests.rs:559-582`).

Fix: make output mode a single explicit choice such as Lo/Ro, phase-coherent
Lo/Ro, and Lt/Rt, registered in every schema/settings/preset/bridge surface.
Reject incompatible combinations rather than silently prioritizing one flag.
Add factory, engine, FFI, UI/preset, and signal-reference tests for every mode
and migration tests for the legacy `dolby_ltrt` alias.

### P1 — The published Downmix Audio Unit cannot instantiate or represent N→2 processing

The generic AU always creates plugins from `"{}"` and passes the same channel
count as both input and output
(`crates/sotf-plugins/crates/plugins-au/GenericAU/GenericRustAudioUnit.swift:131-156`).
Downmix's factory deserializes `DownmixPluginParams`, whose `input_channels` is a
required field (`src/lib/types.rs:12-16`), so `{}` fails before construction
(`crates/sotf-plugins/src/factory/create.rs:158-170`). The Downmix AU subclass
adds no override (`crates/sotf-plugins/crates/plugins-au/DownmixAudioUnit/DownmixAudioUnit.swift:1-10`).

Even if default construction were fixed, the generic AU advertises equal input
and output widths (`GenericRustAudioUnit.swift:291-308`), allocates scratch for
one `channels` value, and creates the FFI handle with equal widths. FFI sizes the
output slice from the caller-supplied handle width, not the plugin's actual
`output_channels()`
(`crates/sotf-plugins/crates/plugins-ffi/src/lib/process.rs:25-53`). Downmix writes
stereo interleaving into that slice, while Swift deinterleaves it as N channels
(`GenericRustAudioUnit.swift:405-451`), corrupting layout for N > 2.

Fix: give channel-changing AUs independent input/output bus formats and scratch
capacities, pass the real widths through FFI, validate them against the created
plugin, and provide Downmix construction parameters derived from the input bus.
Add AU integration/render tests for 5.1→2, 7.1→2, 5.1.4→2, format changes, and
unsupported layouts. Until that exists, do not publish the Downmix AU as a
working effect.

### P1 — Compile metadata incorrectly declares the signal-dependent phase aligner linear and block-time-invariant

`compile_metadata()` always uses `PluginCompileMetadata::linear_transform`
(`src/lib/downmix_plugin.rs:756-769`). That helper explicitly sets `linear =
true`, `time_invariant_for_block = true`, and permits global gains to move across
the operation
(`crates/sotf-plugins/crates/sotf-host/src/plugin/types.rs:105-125`). The
phase-coherent algorithm measures input magnitudes/phases and replaces the
ordinary sum with a signal-dependent aligned magnitude/phase
(`src/lib/downmix_plugin.rs:625-703`); it is not additive and therefore is not a
linear transform. The scalar path is linear only while its coefficient
smoothers are settled; during automation its gains change sample by sample
(`:483-505`).

Incorrect metadata can authorize invalid graph optimization/reordering today or
as compiled-plan coverage expands. It also obscures the stateful LFE and Lt/Rt
filters.

Fix: report a conservative nonlinear boundary for phase-coherent mode. Report
linear/time-invariant scalar metadata only when every smoother is settled and
the host's gain-movement rules are valid across LFE/LtRt state. Add metadata
tests for each mode and during/after automation, plus equivalence tests for any
compiled gain motion.

### P2 — Channel count is insufficient to identify the layout, so valid layouts are silently misrouted

Construction selects a speaker layout solely with
`get_speaker_config_by_channels` (`src/lib/downmix_plugin.rs:102-106`). The host
resolver explicitly chooses 7.1 for eight channels even though the stream may be
5.1.2, and chooses 5.1.4 for ten channels even though it may be 7.1.2
(`crates/sotf-plugins/crates/sotf-host/src/speaker_config/get.rs:43-57`). Those
formats have different channel roles and ordering. A valid 5.1.2 stream can
therefore have height channels treated as rear surrounds; a valid 7.1.2 stream
can have bed surrounds treated as heights, changing gain and pan and affecting
Lt/Rt eligibility.

The coefficient test acknowledges ambiguous-count mismatch and relaxes its
tolerance rather than specifying the correct layout (`src/lib/tests.rs:203-334`).
Engine `PluginSettings::Downmix` stores only `input_channels`, not a layout ID
(`crates/sotf-engine/src/plugins/plugin_settings.rs:1158-1176`).

Fix: make input layout/channel labels a required structural property supplied by
the graph/stream, with a clearly named fallback only for genuinely unknown
layouts. Add exact routing matrices for 5.1.2 versus 7.1 and 5.1.4 versus 7.1.2,
plus explicit unknown-layout tests.

### P2 — “Normalization prevents clipping” is false and silently couples all user gains

Standard coefficients are globally reduced whenever either absolute coefficient
sum exceeds 2.0 (`src/lib/downmix_plugin.rs:426-441`). A sum of 2.0 can still
produce a +6 dBFS peak from correlated full-scale channels, so this does not
prevent clipping. ITU mode is not normalized at all and its documented L +
0.707C + 0.707Ls sum can reach 2.414. The phase aligner explicitly sums channel
magnitudes, making coherent peaks especially likely
(`src/lib/downmix_plugin.rs:651-702`). No limiter, headroom control, or output
clamp follows.

At the same time, normalization rescales *every* route, including unity front
L/R. Increasing surround or LFE gain can therefore lower dialogue and front
channels, so the effective center/surround dB values no longer match the UI.
`USAGE.md:118-121` claims the 2.0 cap prevents clipping, while high-level tests
check only finiteness, not peak or true-peak bounds.

Fix: define the gain contract explicitly. Prefer standard coefficients plus a
separate documented downmix headroom/output-gain control; optionally provide a
true-peak limiter as an explicit mode. Do not silently renormalize unrelated
routes. Test all coherent sign/polarity combinations, full-scale channel
impulses, correlated programme, parameter isolation, peak/true peak, and exact
published coefficient gains.

### P2 — Phase alignment converts magnitude sums into coherent energy without a power-preserving bound or temporal stability

For every bin/output, the phase path adds all weighted magnitudes and places that
entire sum at an energy-weighted average phase
(`src/lib/downmix_plugin.rs:640-703`). For decorrelated channels, physical power
adds approximately with the square root of channel count; magnitude summation
grows linearly with channel count. The algorithm therefore creates excess energy
as channels are aligned, suppresses intended cancellations, and can alter spatial
balance. When the weighted phase vector approaches zero, small input changes can
rotate the chosen phase sharply between frames; there is no temporal phase or
gain continuity beyond WOLA.

Fix: state the perceptual objective and use a bounded energy model—for example,
preserve the standard mix magnitude while choosing a robust reference phase, or
blend toward a power-normalized aligned result with coherence/confidence gating
and temporal smoothing. Evaluate correlated, anti-correlated, diffuse,
decorrelated, transient, and moving-source multichannel material. Measure output
power, peak, spectral error, inter-channel image, phase jumps, and listening-test
preference against ordinary ITU/LoRo downmix.

### P2 — Realtime parameter updates allocate and recompute unrelated state

Every setter, including the two blend frequencies and boolean phase toggle,
calls `compute_coefficients` and rebuilds the cached parameter vector
(`src/lib/downmix_plugin.rs:774-778`). Coefficient computation allocates a new
`Vec`, clears/resizes LFE lookup storage, and may grow smoother storage
(`:208-331,339-443`); cache rebuilding allocates another vector (`:177-179`). All
eight controls are declared realtime, whose host contract means zero-dropout
updates.

The process-only allocation test does not measure setters
(`crates/sotf-plugins/tests/realtime_allocation_tests/tests.rs:561-591`).

Fix: dispatch side effects by returned parameter index. Blend controls need only
store validated scalars; gain/ITU changes should fill preallocated coefficient
and LFE arrays in place; structural mode changes should rebuild off-thread.
Publish parameter snapshots through a control-thread cache strategy rather than
reallocating realtime storage. Add allocation tests around each setter and
block-boundary automation.

### P2 — Reset allocates, and sample-rate reinitialization preserves incompatible buffered state

`reset()` reconstructs and collects a new vector of LFE biquad pairs
(`src/lib/downmix_plugin.rs:901-934`), violating an allocation-free reset
contract. `initialize()` likewise replaces LFE storage, but when Lt/Rt channel
count is unchanged it updates all-pass coefficients without resetting their
history (`:783-821`). It also leaves STFT input/overlap/output accumulator state
untouched. Reinitializing an existing instance at a new sample rate can therefore
emit old-rate buffered audio and process old filter state under new
coefficients.

Fix: provide in-place reset methods for biquad state and pre-size the arrays at
construction. On sample-rate change, clear every delay/filter/WOLA/smoother state
after updating coefficients, or replace the instance off-thread. Test reset
allocations, reset output equivalence, 44.1→96→48 kHz transitions after nonzero
audio, and absence of stale tails/non-finite samples.

### P2 — Construction bypasses schema validation and processing trusts buffer dimensions

`from_params` copies raw deserialized floats directly (`src/lib/downmix_plugin.rs:181-194`).
Unlike runtime `param_bridge`, it does not reject NaN/infinity, enforce declared
ranges, or ensure a meaningful sample rate and channel count. Non-finite blend
values reach crossover arithmetic (`:628-638`); non-finite gains reach `powf`
and persistent filter/output state (`:339-347`). Extreme channel counts can
overflow constructor size products (`:114-126`). The QA itself uses -100 dB even
though canonical ranges bottom out at -12 or -60 dB (`bin/qa_downmix.rs:8-21`),
demonstrating that construction and automation have different contracts.

`process` clears output but never checks `input.len() == frames * input_ch` or
`output.len() == frames * 2` before indexing (`src/lib/downmix_plugin.rs:823-899`),
and FFT errors are unwrapped on the audio path (`:598-604,707-729`). Malformed
direct callers panic rather than return `PluginResult` errors.

Fix: centralize fallible construction validation with checked arithmetic and
reuse canonical parameter normalization. Validate exact input/output lengths
before mutation and propagate FFT errors. Test zero/huge channels, zero sample
rate, min/max/out-of-range/NaN/infinity construction, short/long buffers, and
recovery after errors.

### P3 — The phase hot path performs avoidable trigonometry per channel and bin

For every active bin and input channel, the code computes `atan2`, then `cos` and
`sin`, only to reconstruct the unit complex phase; it repeats atan/trig for each
output average (`src/lib/downmix_plugin.rs:654-699`). The normalized phase is
already `val / |val|`, and the final direction is the normalized phase vector.
Using complex normalization would remove most transcendental calls and likely
improve both speed and approximation accuracy. LFE is also filtered in ITU mode
even though its coefficient is zero.

Fix: accumulate weighted normalized complex vectors using the existing inverse
square-root result, normalize once, and skip channels/outputs with zero gain.
Skip discarded LFE before filtering. Benchmark mono/5.1/7.1.4/9.1.6 with
representative correlated and diffuse signals at all sample rates; report
p50/p95/p99/max callback time against the audio deadline. Preserve reference
output or document intentional numeric changes.

### P3 — Documentation, versions, and test evidence have drifted

`Cargo.toml` reports 0.5.23, the changelog leads with 0.5.25, plugin info reports
2.0.0 (`src/lib/downmix_plugin.rs:746-749`), and the AU plist reports 0.7.10. The
UI/usage documents claim gain ranges of -24 to +12 dB and phase defaults of
200/5000 Hz, while the compiled schema uses different ranges and defaults
(`UI.md:27-43`; `USAGE.md:17-44`; `src/params.rs:21-92`). `USAGE.md` still says
75% overlap/full Hann even though code is 50% sqrt-Hann, and the changelog's
deferred step-blend issue is already a smoothstep in source. `AGENTS.md` describes
an obsolete two-file architecture.

The standard QA forces phase coherence off and measures only the scalar path
(`bin/qa_downmix.rs:8-21`), yet catalog evidence claims WOLA, Lt/Rt, routing,
bypass, and high-layout realtime coverage
(`crates/sotf-plugins/src/factory/catalog.rs:620-637`). No plugin benchmark covers
phase mode or Lt/Rt. Finally, `src/lib/misc.rs` is an orphan duplicate test file;
`src/lib.rs:1-20` does not compile it, while the live copy is
`src/lib/tests/misc.rs`.

Fix: establish one generated source for parameters/layout docs and one version
policy. Update QA to cover default phase mode, fixed latency, Lt/Rt transfer, and
high layouts; add real deadline benchmarks. Remove or reattach orphan tests and
make catalog evidence point to executable checks that test the claimed contract.

## Algorithm and realtime assessment

The scalar Lo/Ro path has a clear interleaved N→2 overwrite contract, uses
speaker positions for constant-power surround/height panning, applies a two-pole
120 Hz LFE low-pass, and advances coefficient smoothing per sample. The STFT
uses a 2048-point transform, 1024-sample hop, periodic sqrt-Hann analysis and
synthesis windows, unnormalized-IFFT compensation of `1/N`, and real-spectrum
DC/Nyquist cleanup. That window/hop/scaling combination is appropriate for WOLA
reconstruction after startup alignment is corrected.

Steady-state `process()` is heap-allocation-free in the tested phase-coherent
configuration. FFT plans, spectra, windows, input buffers, accumulator, and I/O
scratch are created up front. There are no locks, logs, or filesystem calls in
the callback. The principal realtime risks are instead topology-changing
automation, allocating setters/reset, unchecked panics, denormal tails in IIR/
all-pass state, and worst-case CPU scaling as
`O(channels * (FFT_SIZE log FFT_SIZE + bins))`.

Algorithm work should proceed in this order: fix the output timeline and mode/
latency contract; replace or remove the invalid Lt/Rt shifter; make layout
explicit; define headroom and phase-alignment energy behavior; then optimize
complex phase accumulation. Performance improvements before those signal
contracts are pinned would merely make incorrect output faster.

## Strengths

- The scalar path overwrites every stereo sample and the STFT path pre-zeroes
  unavailable output rather than exposing stale host-buffer contents.
- FFT plans, windows, channel spectra, mixed spectra, WOLA accumulator, and
  transform I/O buffers are preallocated during construction.
- The current steady-state phase-coherent process path passed the dedicated
  zero-heap-allocation test.
- Sqrt-Hann analysis/synthesis at 50% overlap and `1/N` inverse scaling are the
  right reconstruction ingredients; focused COLA/tone tests cover steady state.
- Coefficient transitions are smoothed, with per-sample advancement in scalar
  mode and per-hop advancement in STFT mode.
- LFE channel lookup is O(1), LFE filter state is per channel, and reset clears
  audio buffers, accumulator, all-pass state, and smoothers deterministically
  apart from its allocation issue.
- Real-spectrum DC and Nyquist imaginary components are forced to zero before
  inverse transforms.
- Tests cover standard/ITU coefficients, speaker-side polarity, height/surround
  power, 5.1/7.1.4 finite output, varied block acceptance, silence/denormals,
  parameter parity, factory channel adaptation, reset, and scalar bypass
  fidelity.
- Factory construction adapts `input_channels` to the live chain width, avoiding
  one common graph mismatch.

## Exhaustive scope reviewed

Every plugin-owned file was read without skipping any portion:

- Configuration/documentation: `AGENTS.md`, `Cargo.toml`, `README.md`,
  `CHANGELOG.md`, `UI.md`, `USAGE.md`.
- Source: `src/lib.rs`, `src/params.rs`, `src/lib/allpass_stage.rs`,
  `src/lib/consts.rs`, `src/lib/default.rs`, `src/lib/downmix_plugin.rs`,
  `src/lib/lt_rt_allpass.rs`, `src/lib/types.rs`, and the orphan
  `src/lib/misc.rs`.
- Tests/QA: `src/lib/tests.rs`, `src/lib/tests/misc.rs`,
  `tests/integration.rs`, `tests/test_cola.rs`, and `bin/qa_downmix.rs`.

The workspace plugin fuzzer's Downmix implementation was read. No plugin-owned
example or benchmark exists. Relevant cross-plugin tests were reviewed: realtime
allocation, distortion regression, DSP/block-size matrix, high-channel,
parameter robustness/parity/round-trip/layout invariants, factory integration,
host integration, and channel-preservation chains.

Integration surfaces reviewed include facade exports and canonical parameter
schema, primary factory construction, catalog declaration, host `Plugin` and
compile-metadata contracts, speaker-layout resolution, engine plugin type,
settings/defaults/config conversion/accessors and channel adaptation, FFI
factory/create/process/parameter mapping, Downmix AU metadata/subclass/view
controller, and the generic AU bus/allocation/render/state bridge. TokenSave
context, file inventory, symbol search, and caller traversal were used before
targeted source reads.

## Verification

Executed from the workspace root:

```text
cargo test -p sotf-plugin-downmix
  45 passed; 0 failed

cargo run -p sotf-plugin-downmix --features qa --bin qa-downmix
  scalar center fold-down: PASS
  scalar latency 0: PASS
  scalar zero allocations: PASS
  scalar estimated CPU 0.05%: PASS

cargo test -p sotf-plugins --test realtime_allocation_tests \
  tests::test_downmix_zero_alloc -- --exact
  1 passed; 0 failed

cargo test -p sotf-plugins --test all_plugins_dsp_matrix \
  every_builtin_obeys_its_block_size_contract -- --exact
  1 passed; 0 failed

cargo test -p sotf-plugins --test plugin_high_channel_tests \
  downmix_from_5_1_and_7_1_4_to_stereo_is_finite -- --exact
  1 passed; 0 failed
```

A read-only transfer-function diagnostic evaluated the checked-in two-allpass-
minus-delay Lt/Rt network at 44.1/48/96 kHz. It confirmed the phase-only unit
test's broad angle tolerance but exposed the severe, sample-rate-dependent
magnitude response reported above.

These checks establish compilation, steady-state WOLA reconstruction, scalar QA,
warmed process allocation freedom, block acceptance, and finite high-layout
output. They do not establish partition-invariant timing, truthful latency,
topology automation, Lt/Rt magnitude/decode compatibility, unambiguous layout
routing, bounded headroom, nonlinear metadata, setter/reset allocation freedom,
or a working channel-changing Audio Unit.
