# AAE plugin code review

Date: 2026-08-12

Scope: `sotf-plugin-aae`

Focus: correctness, reverberation and spatial-rendering quality, realtime
allocation, and performance

## Remediation status (0.5.7 — complete)

All P0–P3 findings are fixed and regression-tested in this remediation:

- Level smoothers now advance per frame; a 1/64/257/512/4096-frame partition
  regression proves sample-identical automation timing.
- Construction has a fallible validation path used by the canonical factory,
  rejecting unsupported choices, non-finite/out-of-range values, and conflicting
  solo modes instead of silently substituting state.
- Live speaker-layout and room-preset mutation is rejected and requires host
  reconstruction, preserving the compiled channel/DSP contract.
- Envelopment and height updates reweight immutable base VBAP rows in existing
  storage rather than allocating/rebuilding all routing matrices.
- Plugin metadata now reports the crate version.
- Bypass keeps the reverb, detector, meters, and limiter advancing while
  crossfading the audible output to metadata-identified FL/FR dry channels over
  5 ms. A long-bypass/re-enable regression proves that no frozen tail resumes.
- Pre-delay and FDN room-size changes crossfade dual read heads over 10 ms;
  RT60 tone-filter coefficients interpolate over 5 ms. Tests prove transitions
  start instead of applying discontinuous state changes.
- `room_size` is explicitly an FDN late-delay scale; the README and parameter
  text no longer claim that it changes preset-defined early-reflection timing.
- The synthesized LFE effects send is a fourth-order 120 Hz Linkwitz–Riley
  low-pass. Deterministic sine tests bound rejection at 250 Hz and 1 kHz.
- The feedback soft guard is +dB headroom rather than a below-full-scale
  threshold. Its default +6 dB setting is exactly linear at nominal amplitude,
  with a focused activation/headroom regression.
- AAE no longer changes the caller's FP control register. The host owns its
  processing-thread FTZ policy, and both success and buffer-error exits now
  leave caller state untouched by construction.
- ER and FDN VBAP tables are normalized sparse triplets with LFE excluded,
  reducing the maximum-layout hot loop from every output channel to at most
  three destinations per source. Storage and membership tests enforce this.
- The previously dormant split unit-test file is explicitly compiled. Quality
  regressions now measure normalized FDN inter-line correlation, panned speech
  detection, sparse-percussion rejection, block invariance, LFE response,
  transition behavior, feedback linearity, and bypass-tail continuity.
- `qa-aae` runs zero-allocation and callback timing under 9.1.6 + Cathedral +
  content awareness + auto gain, reporting p50/p95/max deadline margin.
- Crate/plugin version, zero-latency catalog metadata, README, AGENTS, parameter
  docs, bridge construction, and structural cached metadata are aligned.

Verification commands and exact results are recorded at the end of this review.

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-aae/` unless a repository-relative path
is shown.

## Findings

### P1 — Level smoothing applies the end-of-block value to the whole block

The four level controls are advanced by the complete host block before any
sample is rendered (`src/lib/aae_plugin.rs:805-809`). The resulting scalar
values are then reused for every frame's dry, early-reflection, late-reverb, and
LFE contribution (`src/lib/aae_plugin.rs:821-932`). The shared `Smoother::next_n`
explicitly computes `target + coeff^n * (current-target)` and returns that final
value (`math-audio/crates/math-dsp/src/smoothing.rs:49-59`). Consequently the
first sample of a 4096-frame block receives the gain that should occur 4096
samples later, while the same stream split into one-sample blocks follows the
intended one-pole envelope. Automation timing and rendered audio therefore
depend on host block size.

Fix: advance each smoother per frame, or generate four preallocated ramps for
the block and consume their per-sample values. Add a parameter-step reference
test that renders the same signal in blocks of 1, 64, 257, 512, and 4096 frames
and compares output after accounting for floating-point tolerance. Assert the
first sample after the step, the 5 ms time constant, and convergence for all
four controls.

### P1 — A setup-only speaker-layout setter changes the live channel contract and allocates

The schema correctly marks `speaker_config` structural and setup-only and
offers only 5.0 through 9.1.6 (`src/params/consts.rs:4-21`). The live setter
nevertheless replaces the speaker config, changes `output_channels()`, and
calls `precompute_gains` immediately (`src/lib/aae_plugin.rs:526-534`). That
method constructs source vectors, nested VBAP matrices, resized rows, and new
flattened vectors (`src/lib/aae_plugin.rs:198-269`). A host that allocated its
output buffer and compiled graph routing from the old channel count can receive
a new count before rebuilding, causing a buffer-size error or invalidating
downstream routing. It also puts substantial allocation and drop work on the
parameter-application path.

The runtime parameter cache represents the choice as an unconstrained string,
not the schema's enumerated choice (`src/params/aae_plugin_params.rs:145-149`).
The integration test therefore successfully sets unsupported `"2.0"` and
expects the live output count to become two (`tests/integration.rs:261-282`).
This test enshrines behavior that contradicts the public schema and setup
contract.

Fix: reject live changes and require construction/graph replacement from a
validated supported layout. If a host needs an explicit prepare/apply flow,
build the immutable routing state off the audio thread and swap it only with a
new output-buffer contract. Make the plugin's cached parameter retain choice
membership validation. Test every supported layout through factory and graph
rebuild, and reject `2.0`, unknown strings, and live structural mutation.

### P1 — Ordinary spatial automation allocates and rebuilds all VBAP tables

`envelopment` and `height_amount` are normal continuous controls, not setup
parameters (`src/params/consts.rs:117-140`). Every update calls the same
allocation-heavy `precompute_gains` routine (`src/lib/aae_plugin.rs:536-546`),
which recomputes direct, early-reflection, and FDN matrices even though only
FDN row weighting changes. `room_preset`, although setup-labelled, also resets
ER state and rebuilds every routing table (`src/lib/aae_plugin.rs:475-481`). The
zero-allocation tests exercise warmed `process` only; they do not cover parameter
application (`bin/qa_aae.rs:54-65`). UI dragging or host automation can thus
allocate repeatedly near the realtime thread and cause deadline misses.

Fix: retain fixed-capacity routing arrays sized for 20 ER taps, 8 FDN lines, and
16 outputs. Precompute immutable base VBAP rows once per layout, then update only
the affected FDN gains in place; preferably smooth/crossfade between old and new
rows. Treat preset changes as prepared state swaps. Add allocation assertions
for every setter and a rapid-automation deadline/click test for envelopment and
height.

### P1 — Deserialized construction bypasses parameter validation and silently changes invalid state

The primary factory deserializes arbitrary JSON and calls the infallible
`AaePlugin::from_params` before `initialize` (`crates/sotf-plugins/src/factory/create.rs:147-155`).
Neither function validates the 25 values against `PARAMS`. Invalid speaker
layouts silently become a 5.1 internal layout while the original invalid string
remains in `params` (`src/lib/aae_plugin.rs:108-123`); invalid room presets
silently become Medium (`src/params/aae_plugin_params.rs:87-94`). Non-finite or
out-of-range floats reach sample-count casts, allpass feedback, FDN delay and
`powf` calculations, limiter thresholds, and smoother construction
(`src/lib/aae_plugin.rs:115-188`, `src/fdn.rs:61-105`). In particular, an
out-of-range `input_diffusion` can construct feedback at or above unity even
though the runtime setter's declared range would prevent it.

This creates different safety rules for restored/configured state and runtime
edits, and can produce misleading state round-trips, unstable processing, or
NaN coefficients. The fact that `Parameter::validate` rejects non-finite and
out-of-range runtime values does not protect construction, because it is never
called there.

Fix: introduce one fallible constructor/validator driven by `PARAMS`, including
finite checks and exact choice membership, and use it in every factory, bridge,
FFI, and state-restore path. Do not silently substitute a different layout or
preset. Add a malformed-state matrix covering NaN, infinities, every range edge,
unknown choices, missing defaults, and conflicting solo flags; require identical
errors across all construction surfaces.

### P2 — `room_size` does not scale all delay lines as documented

The public parameter says it “Scales all delay line lengths”
(`src/params/consts.rs:22-24`). Runtime updates only call `Fdn::set_room_size`
(`src/lib/aae_plugin.rs:437-445`), which changes the eight late-reverb delay
lengths (`src/fdn.rs:184-197`). Early-reflection taps are fixed millisecond
tables selected solely by room preset (`src/early_reflections.rs:180` onward),
and the two input diffusers and pre-delay are likewise unchanged. A very small
or large room can therefore retain exactly the same first-reflection timing and
early/late transition while only the late modal spacing moves. This is an
audible model inconsistency and a documentation/behavior mismatch.

Fix: either rename the control to “FDN delay scale” and document its limited
scope, or derive ER tap times (and, if intended, diffusion delays) from the same
room scale while preserving maximum-capacity bounds. Quantify first-reflection
arrival, early echo density, mixing time, and late delay distribution at 0.2,
1.0, and 3.0; compare them with the declared acoustic model.

### P2 — Live delay, preset, and decay changes can click or pitch-shift the tail

Several exposed controls switch state discontinuously. `pre_delay_ms` moves the
read tap immediately in existing history (`src/lib/aae_plugin.rs:469-473`).
`room_size` changes all eight FDN read delays while retaining delay and
interpolation state (`src/fdn.rs:184-197`); preserving the tail avoids truncation
but an instantaneous read-position jump produces a discontinuity and a pitch
event. `room_preset` replaces tap times and directions and clears ER filter and
allpass states while preserving the delay line (`src/early_reflections.rs:153-167`).
RT60 and tonal ratios replace feedback-filter coefficients immediately
(`src/fdn.rs:172-181`). No transition or crossfade test exists.

Fix: use dual read heads with a short equal-power crossfade for delay changes;
prepare/crossfade preset routing; and interpolate stable filter parameters for
decay changes. Define whether setup-labelled controls may be changed while
running. Test steps during a sustained tail and bound peak derivative, click
energy, pitch excursion, and tail-energy discontinuity at minimum/maximum
values and multiple sample rates.

### P2 — Bypass freezes reverb and metering state, then resumes stale audio

Bypass copies L/R to numeric output channels 0 and 1 and returns before the
pre-delay, ER, FDN, content-aware detector, auto-gain input meter, or limiter is
advanced (`src/lib/aae_plugin.rs:782-803`). Thus an impulse tail is frozen for
the entire bypass interval and resumes from its old time position when bypass
is disabled. Auto gain also misses all input observed during bypass and can
resume with stale estimates. The QA bypass test verifies only immediate sample
copying after reset (`bin/qa_aae.rs:356` onward), so it cannot detect this state
contract.

Fix: specify bypass semantics. For a conventional continuous-tail bypass, keep
the DSP and meters advancing into scratch output while crossfading the audible
dry/wet paths; for a reset-on-bypass policy, clear state explicitly and
crossfade to prevent a click. Route FL/FR by speaker metadata rather than an
implicit numeric assumption. Test impulse → bypassed silence → unbypass, long
bypass intervals, repeated toggles, and auto-gain convergence.

### P2 — The synthesized LFE feed uses only a first-order low-pass

Wet ER and FDN source values are collapsed to a signed RMS proxy and passed
through one one-pole state (`src/lib/aae_plugin.rs:923-932`). This is a
6 dB/octave low-pass, so appreciable midrange reverberation remains above the
120 Hz crossover. Because AAE is synthesizing an LFE channel rather than merely
passing authored LFE, leakage can make the subwoofer localizable and can overlap
main-channel bass unpredictably. Current QA checks only that LFE energy is
nonzero (`bin/qa_aae.rs:172` onward), not spectral containment or crossover
summation.

Fix: define whether this is an effects send or bass-managed complement. For a
bass-managed feed, use a tested fourth-order Linkwitz–Riley low-pass (and the
corresponding main-path policy if complementary summation is promised). Measure
gain and phase around crossover, attenuation at 250 Hz/1 kHz, impulse response,
and energy consistency across layouts and source polarity.

### P2 — The feedback “safety limiter” changes decay and distortion at normal levels

Every FDN feedback sample adds the input and then passes through `soft_clip`
(`src/fdn.rs:158-167`). The default 6 dB threshold converts to approximately
0.5 linear (`src/fdn.rs:103-105`), so the nonlinearity can engage well below
full-scale whenever input and feedback sum. Once active, the FDN is no longer
the linear decay model used to calculate RT60 coefficients: decay becomes
level-dependent and harmonics/intermodulation are injected inside the feedback
loop. Existing tests establish boundedness and a coarse decay distinction, but
not RT60 accuracy or distortion versus input level.

Fix: keep emergency protection outside the modeled feedback loop where
possible, or document the nonlinear design and choose a threshold/curve from
measured headroom. Normalize injection and feedback so nominal programme does
not reach the guard. Sweep impulse level and sustained multitone level; report
RT60 by band, THD/IMD, limiter activity, and decay invariance with the safety
control at each setting.

### P2 — FTZ/DAZ setup permanently changes caller thread floating-point state

`process` calls `enable_ftz_daz` before even validating buffer lengths
(`src/lib/aae_plugin.rs:750-777`). The function explicitly leaves the register
change active on the calling thread; the shared documentation says to use
`ScopedFtz` when lexical scoping is required
(`math-audio/crates/math-dsp/src/simd/misc.rs:187-200`). SOTF's engine sets FTZ
once for its own processing thread, so the extra call is redundant there. In a
standalone/third-party host, however, AAE changes the caller's FP semantics on
success and on every early error, affecting subsequent plugins and host code.

Fix: rely on the host-level thread setup when that contract is guaranteed, or
use `ScopedFtz` so the prior FP control register is restored. Add register-state
tests around successful processing and both buffer-error exits on x86_64 and
AArch64.

### P2 — Dense source-to-speaker loops leave avoidable work in the hottest path

At every sample the renderer visits 12–20 ER taps and 8 FDN lines, then loops
over every output channel for each non-negligible source
(`src/lib/aae_plugin.rs:860-920`). In 9.1.6 this is up to 448 gain/sample pairs,
although a VBAP row normally has only a small number of nonzero speakers. It
also sums ER taps in a separate pass and traverses the final output again for
denormal flushing, auto gain, limiting, and a second flush
(`src/lib/aae_plugin.rs:889-939`). The Criterion benchmark covers useful block,
layout, and preset axes, but QA's hard CPU gate uses the default 5.1 layout and
512-frame blocks (`bin/qa_aae.rs:67-88`); neither records p95/worst callback
time under maximum layout, Cathedral taps, auto gain, and content awareness.

Fix: precompute sparse `(channel,gain)` rows or fixed small VBAP triplets, skip
LFE entries in the spatial rows, and profile whether final passes can be safely
combined. Benchmark release builds across 64–2048 frames, all layouts/presets,
and option combinations; report p50/p95/max time and deadline margin, not only
throughput. Keep the current allocation assertion for every matrix.

### P3 — Quality tests and metadata do not yet substantiate the advertised acoustic model

The suite has useful finite-output, reset, routing-energy, tail-presence, and
zero-allocation checks, but many assertions establish activity rather than
quality. FDN decay checks only that late energy becomes small; decorrelation
compares sums rather than cross-correlation/coherence. There is no bandwise RT60
fit, echo-density buildup, frequency response, spatial energy-vector/diffuseness,
inter-channel coherence, modulation sideband, block-invariance, click, LFE
rejection, dialogue false-positive/false-negative, or limiter-distortion
threshold. The content-aware detector is based on L/R mid-side energy and
modulation, so centered percussion can look like dialogue and off-center speech
can evade it; this needs corpus-level validation rather than a few synthetic
signals.

Documentation and metadata are also sparse/drifting: README is only a minimal
description, package version is 0.5.4 while `PluginInfo` reports 0.5.1
(`Cargo.toml:4`, `src/lib/aae_plugin.rs:406-409`), and the catalog describes
plugin-reported reflection/FDN latency while the implementation always reports
zero (`crates/sotf-plugins/src/factory/catalog.rs:597-617`,
`src/lib/aae_plugin.rs:943-945`). Zero algorithmic latency is defensible because
the direct path is immediate, but the catalog wording should not imply a
configuration-dependent delay.

Fix: add an offline acoustic-quality harness with quantitative acceptance bands
and documented reference signals/rooms. Evaluate the dialogue detector on
labeled speech, music, percussion, anti-phase, and panned material, reporting
precision/recall and gain-pumping metrics. Align crate, plugin, changelog,
catalog, README, and AGENTS descriptions for each release.

## Algorithm and realtime assessment

The architecture is coherent: stereo is converted to a mono wet feed, then
pre-delayed and diffused; 12–20 directional early-reflection taps feed an
eight-line energy-preserving Hadamard FDN with frequency-dependent decay and
slow delay modulation; direct, ER, and FDN sources are rendered by VBAP to 5.0
through 9.1.6; an independent wet-derived LFE feed, content-aware wet ducking,
optional multichannel auto gain, and a final linked ceiling complete the path.
The Hadamard normalization, prime-ish unequal delays, per-line modulation rates,
stateful tone filters, and source-domain LFE extraction are sound foundations.

Steady-state `process` storage is preallocated: delay lines, tap scratch, routing
tables, dialogue state, limiter state, and the optional auto-gain object are
reused. Both the plugin QA allocator and workspace realtime-allocation test pass.
That guarantee is narrower than the plugin's operational realtime surface:
several parameter setters allocate, while block-level smoothing makes otherwise
allocation-free processing incorrect under automation.

The next algorithmic priority should be measurement rather than adding more
features. A bandwise Schroeder RT60 fit, echo-density/mixing-time metric,
inter-channel coherence and spatial-energy analysis, crossover response, and
level-dependent distortion sweep would reveal whether the current constants
deliver the intended room behavior. Those measurements should be repeated for
all room sizes/presets, layouts, sample rates, and block partitions.

## Strengths

- The signal-flow stages and parameter groups are clearly separated, and the
  plugin exposes an explicit stereo-to-configurable-multichannel contract.
- The warmed audio path passes dedicated zero-allocation checks.
- Delay storage is sized during construction/initialization, and FDN room-size
  changes reuse capacity instead of reallocating delay lines.
- The eight-point normalized Hadamard feedback matrix preserves energy before
  filtering/nonlinearity and has focused tests.
- ER modulation uses per-tap state and distinct slow rates; FDN modulation uses
  distinct per-line rates, reducing obvious periodic correlation.
- Frequency-dependent feedback gains are derived from delay length and target
  RT60 ratios rather than arbitrary fixed damping coefficients.
- VBAP gains are precomputed rather than solved per sample, normalized, and
  height/LFE speaker metadata is respected in normal rendering.
- LFE extraction occurs in the source domain, avoiding cancellation from
  summing decorrelated routed channels.
- Output limiting is linked across channels and includes an overshoot guard,
  preserving spatial balance better than independent channel limiters.
- Buffer lengths are validated before indexed processing, and reset clears the
  principal delay, filter, detector, limiter, and auto-gain states.
- Benchmarks cover multiple block sizes, speaker layouts, presets, and a
  production-style configuration; QA checks tail, routing, RT60 sensitivity,
  finite output, bounded energy, allocation, and throughput.

## Exhaustive scope reviewed

Every plugin-owned file was read:

- Documentation/configuration: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`,
  `README.md`.
- QA and benchmarks: `bin/qa_aae.rs`, `benches/aae-benchmark.rs`.
- Core DSP: `src/delay_line.rs`, `src/early_reflections.rs`, `src/fdn.rs`,
  `src/hadamard.rs`, `src/tone_filter.rs`.
- Public/module surfaces: `src/lib.rs`, `src/params.rs`.
- Split plugin implementation: `src/lib/aae_plugin.rs`,
  `src/lib/allpass_diffuser.rs`, `src/lib/consts.rs`, `src/lib/misc.rs`,
  `src/lib/smoothing.rs`, `src/lib/types.rs`.
- Parameter implementation: `src/params/aae_plugin_params.rs`,
  `src/params/consts.rs`, `src/params/default.rs`.
- Tests: `src/lib/tests.rs`, `src/lib/tests/misc.rs`, `tests/integration.rs`, and
  every inline test in the delay-line, early-reflection, FDN, Hadamard,
  tone-filter, parameter, and helper modules.

Integration review covered facade exports; primary factory construction;
catalog channel, latency, schema, stability, and allocation claims; NIH and FFI
parameter schema mapping; engine settings, conversion, accessors, channel-count
propagation, and chain tests; the workspace high-channel, factory, robustness,
round-trip, allocation benchmark, and realtime-allocation surfaces; shared VBAP,
plugin validation, smoother, auto-gain, and FTZ implementations. No
plugin-owned examples or fixture directories exist.

TokenSave was used before source reads to locate active symbols, callers, tests,
and host integration. Across the continued and preceding review queries it
saved approximately 39,700 context tokens.

## Verification

Executed from the workspace root:

```text
cargo test -p sotf-plugin-aae
  79 passed; 0 failed (3 suites)

cargo check -p sotf-plugin-aae
  passed

cargo clippy -p sotf-plugin-aae --lib --no-deps -- -D warnings
  passed

cargo test -p plugins-bridge aae_bridge_rejects_invalid_restored_state_without_panicking
  1 passed; 0 failed

cargo check -p plugins-bridge
  passed

cargo test -p sotf-plugins --test realtime_allocation_tests test_aae_zero_alloc
  blocked by unrelated concurrent ABCompare PARAMS/accessor-count mismatch

cargo run -p sotf-plugin-aae --features qa --bin qa-aae
  all 10 QA groups passed under 9.1.6/Cathedral/content-aware/auto-gain
  process zero allocations: PASS
  5.0 s audio in 123.88 ms; estimated CPU 2.48%: PASS
  callback p50/p95/max 0.257/0.287/1.028 ms vs 10.667 ms deadline

cargo check -p sotf-plugins --no-default-features --offline
  passed

cargo bench -p sotf-plugin-aae --no-run
  passed; library and aae-benchmark executables built

cargo check -p sotf-plugins --benches --no-default-features --offline
  passed (one unrelated unused import and one dependency future-incompat warning)

git diff --check
  passed
```
