# Denoiser plugin code review

## Remediation status — 2026-08-12

- **Fixed:** the FFT path has no raw-pointer slice or aliased `&mut self`; disjoint state is passed
  to a safe helper and the zero-allocation scratch path is preserved.
- **Fixed:** low-latency and multi-resolution topology changes are rejected live before mutation.
- **Fixed:** small and large analyzers advance on the same frame timeline; callback partitions from
  64 through 4096 frames now produce matching multi-resolution output.
- **Fixed:** factories use fallible construction and reject zero channels, non-finite values, and
  every serialized numeric value outside its declared range.
- **Fixed:** spatial coherence uses matched cross/auto-power smoothing, with silence reset and
  explicit stereo, 3.0, 5.1, and 7.1 pair/LFE behavior covered by signal fixtures.
- **Fixed:** MCRA automation updates both estimators without reallocating.
- **Fixed:** monitoring publishes effective captured-profile use, and documentation examples now
  distinguish normalized serialized fractions from UI percentages.
- **Fixed:** profile learning derives an approximately one-second target from sample rate/hop size;
  small-FFT errors propagate through `PluginResult`; QA covers the optional-mode/channel/FFT matrix.
- **Fixed:** package version, changelog, architecture notes, feature/latency documentation, and UI
  contract now describe the shipped implementation.

Verification after remediation: plugin suite (63 passed), optional-mode QA/allocation matrix,
realtime allocation test, strict plugin clippy, and diff check.

Date: 2026-08-12

Scope: `sotf-plugin-denoiser`

Focus: correctness, algorithm quality, realtime allocation, and performance

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-denoiser/` unless a repository-relative
path is shown.

## Findings

### P1 — The main FFT path creates an aliased reference with `unsafe`

`process_fft_block` takes a pointer into `self.io.temp_input_block`, constructs an
immutable slice from it, and then calls a method requiring `&mut self` while that
slice remains live (`src/lib/denoiser_plugin.rs:628-640`). The callee consumes the
slice while mutating `self.fft` (`src/fft.rs:10-26`). The raw-pointer conversion
bypasses the borrow checker, so the call simultaneously has a shared reference
into `self` and an exclusive reference to all of `self`. This is an aliasing
contract violation and makes an otherwise safe FFT pipeline depend on undefined
behavior. It also conflicts with the repository rule that new/existing `unsafe`
must be explicitly justified and approved.

Fix: remove the raw pointer. Split disjoint field borrows and make the FFT helper
operate on `&[f32]`, `&Config`, and `&mut FftState`, or make it a method on the FFT
state. Keep the existing scratch allocation. Add a regression that runs the
streaming/varied-block reconstruction case under Miri where supported, plus the
existing zero-allocation test.

### P1 — Runtime `low_latency` changes the reported value but not the DSP topology

The schema correctly marks `low_latency` structural/setup
(`src/params/consts.rs:47-50`). Construction chooses 512- or 2048-point FFT state
from it (`src/lib/denoiser_plugin.rs:197-199`, `532-533`). At runtime, however,
index 5 only changes `self.params.low_latency`
(`src/lib/denoiser_plugin.rs:492-500`), and `apply_values` has no index-5 side
effect (`src/lib/denoiser_plugin.rs:807-856`). The getter then reports the new
flag even though FFT size, hop, plans, windows, buffers, maximum block size, and
reported latency remain unchanged. A host can therefore show “Low Latency: On”
while still processing and reporting the original mode.

Fix: reject in-place changes to setup parameters and require graph replacement,
or build a complete replacement state off the audio thread and swap it at a safe
boundary. Test both construction modes and a live setter attempt, asserting FFT
size, hop, maximum accepted block, and `latency_samples()` remain mutually
consistent.

### P1 — Multi-resolution output depends on host block size and uses future analysis

When multi-resolution is enabled, `process_in_place` feeds the *entire* incoming
host block to the small-FFT analyzer before any large FFT frame is processed
(`src/lib/denoiser_plugin.rs:973-991`). It then processes one or more large frames
(`src/lib/denoiser_plugin.rs:994-1014`), and each frame blends with only the
small analyzer's latest gain/flux state (`src/lib/denoiser_plugin.rs:664-672`,
`src/multi_resolution.rs:342-377`). For a large host block, the first large frame
therefore uses analysis from later samples in that block, and every large frame
in the call can reuse the same final small-FFT state. Splitting the identical
stream into smaller calls changes which small gains are paired with each large
frame. This violates streaming block-size invariance and can move transient
protection earlier than the transient itself.

Fix: advance both analyzers on one shared timeline. Queue timestamped small-FFT
gain/flux frames and consume the state aligned to each large-frame analysis
position; do not pre-feed future host samples. Add sample-exact comparisons for
the same impulse, burst, step, and noise stream divided into 64, 257, 512, 2048,
and 4096-frame host blocks. Include transients near every block boundary.

### P1 — Construction bypasses parameter-range and finite-value validation

The public/factory construction path deserializes parameters and calls the
infallible `from_params` directly
(`crates/sotf-plugins/src/factory/create.rs:294-298`).
`from_params` clamps several fields but does not reject non-finite values; `f32`
`clamp` does not sanitize NaN. More seriously, all four MCRA controls bypass
their declared ranges: alpha-S, alpha-P, and delta are assigned directly, while
the window is only forced above zero (`src/lib/denoiser_plugin.rs:531-560`).
These values enter recursive estimators and threshold calculations. Other NaNs
can reach `powf` and the smoothing kernel immediately
(`src/lib/denoiser_plugin.rs:562-588`). Runtime parameter updates go through the
host bridge, so construction and automation enforce different contracts.

Fix: introduce one fallible validation/normalization routine shared by factory
construction and runtime updates. Reject every non-finite float and enforce the
`ParamSpec` ranges for all 29 fields before mutating DSP state. Return a factory
error rather than silently accepting unstable values. Test every numeric field
at min/max, just outside the range, NaN/infinity through the Rust API, and invalid
JSON/preset inputs through the factory.

### P1 — Spatial coherence mixes a smoothed cross-spectrum with instantaneous powers

The implementation exponentially averages the complex cross-spectrum but divides
its squared magnitude by the *current-frame* channel powers
(`src/wiener/consts.rs:213-234`). The documented magnitude-squared coherence is
`|E[X0 X1*]|^2 / (E|X0|^2 E|X1|^2)` (also claimed in
`CHANGELOG.md:20-22`); numerator and denominator must use compatible averaging.
With amplitude modulation, attacks, fades, or unequal channel dynamics, the
current ratio can spike or collapse and then be hidden by the `[0,1]` clamp. That
drives audible, signal-dependent extra attenuation.

Fix: retain independently smoothed auto-power spectra for both channels using the
same time constant as the cross-spectrum, and divide by their product with a
well-defined silence transition. Test identical signals with fixed unequal gain,
level modulation, fades, and transients; phase-shifted coherent signals;
decorrelated noise; and transitions into/out of silence.

### P1 — MCRA automation leaves the enabled small-FFT estimator stale

`MultiResState` copies alpha-S, alpha-P, L, and delta only at creation
(`src/multi_resolution.rs:136-165`). Runtime indices 7-10 update only the main
estimator (`src/lib/denoiser_plugin.rs:501-504`); `apply_values` handles no side
effects for those indices (`src/lib/denoiser_plugin.rs:813-856`). If multi-res is
already enabled, the 2048-point and 512-point paths silently use different MCRA
tuning until multi-res is toggled off and on.

Fix: update both estimators atomically and define whether estimator history is
preserved or reset when hyperparameters change. Prefer an explicit setter on
`MultiResState` rather than exposing fields. Test all four controls before and
after enabling multi-res and assert both paths receive the same values and reset
semantics.

### P2 — “Multichannel” spatial denoising affects only channels 0 and 1

The parameter is described as multichannel/stereo+ (`USAGE.md:49-52`,
`src/params/consts.rs:222-223`), and the integration accepts standard multichannel
layouts. The algorithm nevertheless computes only pair 0/1 and modifies only
`smoothed_gain[0]` and `[1]` (`src/wiener/consts.rs:150-162`). Channels 2 and above
receive no spatial processing. In 5.1/7.1 content this produces channel-dependent
noise character and makes the public description false.

Fix: define a channel-topology contract. Either restrict the feature to stereo
and disable it for wider layouts, or compute coherence for documented pairs/a
reference model and apply it consistently to every eligible channel. Add 3.0,
5.1, and 7.1 tests with coherent fronts, decorrelated surrounds, and LFE policy.

### P2 — Toggling multi-resolution performs FFT planning/allocation in `apply_values`

The multi-res parameter is marked structural/setup
(`src/params/consts.rs:210-214`), but ordinary runtime application constructs or
drops `MultiResState` (`src/lib/denoiser_plugin.rs:841-853`). Construction plans
per-channel FFTs and allocates channel state and sample buffers
(`src/multi_resolution.rs:143-165`). If a host dispatches parameter changes on or
near the audio thread, this introduces allocator activity, FFT-planner work, and
potentially expensive destruction into a realtime boundary.

Fix: enforce the setup flag in the adapter/host and rebuild off-thread, or prepare
both modes during setup and switch only preallocated state at a defined boundary.
Add an allocation test around parameter application as well as audio processing,
and document which thread may call `apply_values`.

### P2 — Noise-profile state can claim “in use” when no profile exists

Runtime index 21 freely sets `use_captured_profile`
(`src/lib/denoiser_plugin.rs:515`). DSP correctly falls back to live MCRA unless
both the flag and profile availability are true (`src/noise_profile.rs:88-96`),
but UI monitoring publishes the raw flag as `using_captured_profile`
(`src/lib/denoiser_plugin.rs:696-718`). Enabling “Use Profile” before learning can
therefore display an active captured profile while the audio uses MCRA.

Fix: report effective state (`use && has`) and either reject/disable the control
without a profile or expose requested and effective states separately. Test
enable-before-learn, learning completion, clear-while-enabled, and preset restore
without profile storage.

### P2 — “~1 second” profile learning varies fourfold between modes

Learning uses a fixed 50 STFT frames (`src/params/consts.rs:4`), incremented once
per processed hop (`src/noise_profile.rs:21-34`). At 48 kHz this is about 1.07 s
with the normal 1024-sample hop but only 0.27 s with the low-latency 256-sample
hop, despite the documented approximately-one-second behavior
(`src/noise_profile.rs:9-12`). Duration also changes with sample rate. The shorter
capture has higher variance and can materially change the learned spectrum.

Fix: express learning duration in seconds and derive the target frame count from
sample rate and hop size, or document the mode-dependent duration. Test completion
time at 44.1/48/96 kHz in both FFT modes and verify estimator variance against a
stationary-noise reference.

### P2 — A recoverable small-FFT error panics on the audio path

The main forward/inverse FFT paths return errors, but the multi-resolution forward
FFT uses `.expect("small FFT forward failed")`
(`src/multi_resolution.rs:205-230`). Even if correctly sized preallocated buffers
make failure unlikely today, a library error or future state mismatch aborts the
audio processing call rather than following the plugin's `PluginResult` contract.

Fix: return `Result` from `process_small_block` and `feed_and_process`, propagate
it through `process_in_place`, and include context (channel and FFT size) without
formatting on the realtime thread if the host contract forbids it. Add a fault-
injection/unit seam that proves the error is returned and state remains resettable.

### P2 — Published JSON examples use UI-scaled percentages as raw values

The raw `smoothing`, `transparency`, and formant-strength parameters are normalized
fractions; `.scaled(100.0)` is presentation metadata
(`src/params/consts.rs:22-33`, `93-104`, `197-209`). `from_params` expects those
raw fractions and clamps them to their normalized maxima
(`src/lib/denoiser_plugin.rs:543-546`, `562-565`, `590-594`). `USAGE.md` instead
shows values such as `smoothing: 70.0` and `transparency: 80.0`
(`USAGE.md:74-98`, `101-147`). Following the examples silently produces 0.99/1.0,
not 70%/80%; the preset recipes therefore do not implement their descriptions.

Fix: change JSON examples to 0.70/0.80/etc. and explicitly distinguish serialized
units from display units. Add documentation examples as deserialization tests so
their effective values cannot drift.

### P3 — The expensive mode matrix has no representative performance coverage

The processing path is allocation-free in the tested default configuration, but
optional modes add a second per-channel FFT, Bark-domain masking passes, formant
analysis, harmonic/percussive weighting, spectral subtraction, median/frequency
smoothing, and PND analysis. The QA benchmark covers five seconds of default
silence and reports one aggregate CPU figure; the realtime allocation test also
covers one configuration. There are no benches in this crate. Regressions in the
most expensive stereo/multichannel combinations can pass current QA.

Fix: add benchmark/allocation matrices for mono, stereo, 5.1, and 7.1; 512/2048
FFT; small/large/irregular host blocks; default and all major optional modes.
Benchmark representative noise+programme input rather than silence. Record p50,
p95, and worst block time against the audio deadline, not only total throughput.

### P3 — Documentation and package metadata have drifted

`Cargo.toml:1-5` declares 0.5.5 while `CHANGELOG.md:1-10` documents an unreleased
0.5.6 behavior. `README.md:1-5` omits nearly the entire current feature and
latency surface. `AGENTS.md:5-14` says the architecture is only `lib.rs` and
`params.rs`, although the implementation is split across the FFT, MCRA, Wiener,
masking, multi-resolution, profile, polyphonic, and spectral-subtraction modules.
`UI.md:6-29` specifies an eight-column layout, while the compiled layout is now a
config section, main groups, and four tabs (`src/params/consts.rs:237-313`). This
drift makes review and maintenance harder and can lead UI work toward dead specs.

Fix: either release/bump 0.5.6 or mark it explicitly unreleased; expand README and
AGENTS architecture/latency notes; generate or test UI documentation from
`PluginLayout` where practical.

## Algorithm and realtime assessment

The core normal-mode structure is sound: 2048-point FFT / 1024-point hop (512/256
in low latency), square-root Hann analysis/synthesis, inverse-FFT normalization,
and 50% weighted overlap-add. Startup and unavailable output are explicitly
zero-filled, and `process_in_place` returns the requested frame count. Normal DSP
buffers, spectra, gain state, noise/profile storage, and output rings are
preallocated. The default processing path passed the dedicated zero-allocation
test.

The main residual algorithm risk is adaptive-state alignment rather than basic
STFT reconstruction. MCRA/decision-directed state, captured profiles, spatial
coherence, and dual-resolution analysis all evolve in time, so host-block
invariance, time-aligned estimator tests, and nonstationary signal tests are more
important than additional static gain-range assertions.

The hot path also performs multiple full-spectrum passes. Existing separation of
state and scratch storage is a good base for later pass fusion, sparse/precomputed
masking weights, and mode-specific early exits, but correctness and temporal
alignment should be fixed before micro-optimization.

## Strengths

- WOLA reconstruction, varied block sizes, startup latency, silence, impulse, and
  reset behavior have focused tests.
- The plugin preserves the STFT host contract by returning `context.num_frames`
  and zero-filling output that is not ready yet.
- Audio processing in the tested default configuration performs zero heap
  allocations; monitoring uses a preallocated realtime cache.
- FFT plans, windows, ring buffers, spectra, MCRA state, gains, and captured-profile
  storage are prepared up front for the normal process path.
- Attack/release coefficients are hop-rate based, and the implementation uses an
  RAII FTZ/DAZ guard around processing.
- DC/Nyquist storage follows the real-FFT spectrum size, and forward/inverse FFT
  failures in the main path are propagated rather than ignored.
- Recent tests cover complex cross terms, no double temporal smoothing in the
  small path, multires toggling, maximum safe in-place blocks, and FPU-state
  restoration on early errors.

## Exhaustive scope reviewed

Every plugin-owned file was read:

- Documentation/configuration: `AGENTS.md`, `README.md`, `USAGE.md`, `UI.md`,
  `CHANGELOG.md`, `Cargo.toml`.
- Executables/examples: `bin/qa_denoiser.rs`, `examples/denoiser_demo.rs`.
- Core source: `src/config.rs`, `src/fft.rs`, `src/lib.rs`, `src/masking.rs`,
  `src/mcra.rs`, `src/multi_resolution.rs`, `src/noise_profile.rs`,
  `src/params.rs`, `src/polyphonic.rs`, `src/spectral_sub.rs`, `src/wiener.rs`.
- Split source: `src/lib/denoiser_data.rs`, `src/lib/denoiser_plugin.rs`,
  `src/lib/misc.rs`, `src/params/consts.rs`, `src/params/d.rs`,
  `src/wiener/consts.rs`, `src/wiener/formant_preserver.rs`.
- Tests: `src/tests.rs`, `src/tests/current.rs`, `src/tests/make.rs`,
  `src/tests/misc.rs`, `src/params/tests.rs`,
  `tests/test_polyphonic_denoiser.rs`.

Integration surfaces reviewed: plugin facade exports/dependencies, primary factory
construction, catalog/schema registration, plugins-bridge construction, engine
plugin-config conversion/settings, host `ParametricInPlacePlugin` adapter and
realtime cache behavior, and the workspace realtime-allocation test. No plugin-
owned benches exist.

## Verification

Executed from the workspace root:

```text
cargo test -p sotf-plugin-denoiser
  54 passed; 0 failed

cargo test -p sotf-plugins --test realtime_allocation_tests test_denoiser_zero_alloc
  1 passed; 0 failed; 45 filtered out

cargo run -p sotf-plugin-denoiser --features qa --bin qa-denoiser
  silence: PASS
  reported latency (2048 samples): PASS
  zero allocations: PASS
  5.0 s audio in 95.94 ms; estimated CPU 1.92%: PASS
```

These checks establish the current default-path baseline, but they do not cover
the P1 structural-parameter, unsafe-aliasing, block-size-invariance, construction-
validation, spatial-estimator, or stale multi-resolution-state cases above.
