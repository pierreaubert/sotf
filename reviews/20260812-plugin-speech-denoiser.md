# Speech Denoiser plugin code review

Date: 2026-08-12

Scope: `sotf-plugin-speech-denoiser`, its `plugins-denoiser::rnnoise`
backend, the vendored `nnnoiseless` model implementation, and host/factory/AU
integration

Focus: correctness, algorithm quality, realtime allocation, and performance

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-speech-denoiser/`. Other references are
repository-relative.

## Findings

No P0 issue was found. The P1 issues nevertheless make the plugin unsuitable for
general host use in its current form.

### P1 — The plugin rejects ordinary host callback sizes, including its own QA block size

`process_in_place` rejects every call whose frame count is not a multiple of 480
(`src/lib.rs:127-159`). That is a backend implementation quantum, not a valid
host callback contract: engine and Audio Unit callbacks may be 1–1024 frames and
can vary from call to call. The generic AU forwards `frameCount` unchanged
(`crates/sotf-plugins/crates/plugins-au/GenericAU/GenericRustAudioUnit.swift:341-421`).
The engine's 1024-frame integration test explicitly skips Speech Denoiser because
it cannot process the block
(`crates/sotf-plugins/crates/sotf-engine/src/engine/processing_thread/tests/misc.rs:95-115`),
and the all-plugin DSP matrix treats the rejection as expected rather than
establishing useful DSP behavior
(`crates/sotf-plugins/tests/all_plugins_dsp_matrix.rs:189-225`).

This is not only a theoretical integration gap. The plugin's QA binary delegates
to the standard 512-frame helper (`bin/qa_speech_denoiser.rs:1-13`;
`crates/sotf-plugins/crates/sotf-host/src/test_utils/misc.rs:44-79`) and panics on
the first process call with `RNNoise requires block sizes that are a multiple of
480; got 512`. The all-plugin Criterion benchmark also uses 512 frames for Speech
Denoiser
(`crates/sotf-plugins/benches/all-plugins-benchmark/benchmark.rs:786-800`), so it
cannot benchmark this plugin.

Fix: make the plugin a streaming adapter around the fixed-size model. Accumulate
arbitrary input into a preallocated FIFO, process complete 480-sample model
frames, and drain a preallocated, initially zeroed output-delay FIFO. Always
return `context.num_frames`; zero-fill only the latency region. Test 1, 16, 63,
64, 127, 128, 256, 479, 480, 481, 512, and 1024 frames, variable block
sequences, and a partial final block. Compare one continuous stream under every
partitioning to establish block-size invariance.

### P1 — Startup drops the first processed frame instead of implementing the reported latency

The backend writes a processed 480-sample frame at `output_write_pos`, then on
the first frame advances `output_read_pos` by 480 before increasing the write
position (`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:194-209`).
The first call therefore reports zero frames and replaces the whole block with
zeros (`rnnoise.rs:211-234`). On the next call, the newly processed second frame
is written at position 480 and immediately read from position 480; the processed
first frame is never emitted. This is a one-time 480-sample deletion, not a
constant 480-sample delay. Nevertheless the backend and wrapper always report
480 samples of latency (`rnnoise.rs:261-268`; `src/lib.rs:162-170`).

The host propagates the returned frame count and shrinks node-buffer validity
(`crates/sotf-plugins/crates/sotf-host/src/host/daw_host.rs:1180-1220,1770-1810`;
`crates/sotf-plugins/crates/sotf-host/src/host/node_buffer.rs:18-31`). Thus graph
startup can become a zero-length block, while subsequent audio is no longer
aligned with the advertised latency. Existing tests codify this behavior by
expecting zero returned frames on the first block rather than testing the signal
timeline (`tests/integration.rs:63-81`; `tests/test_speech_denoiser_plugin.rs:6-25`;
`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:334-370,647-662`).

Fix: preserve stream length. Initialize a 480-sample output-delay queue with
zeros, enqueue every processed frame, dequeue exactly the requested number of
frames, and return the requested count on every successful call. If the model's
true analysis/synthesis delay differs, measure and report that delay instead.
Add time-coded ramp and impulse tests that compare actual alignment to
`latency_samples()` through both the plugin and a host graph, including startup,
reset, bypass, and variable callback sizes.

### P1 — Linked stereo completely bypasses denoising for anti-phase content and is unstable near cancellation

For two channels the backend downmixes `(L + R) / 2`, runs only that mono signal
through RNNoise, and reconstructs a per-sample gain as `mono_out / mono_in`
(`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:132-156,277-285`).
When `R = -L`, the downmix is zero; `linked_stereo_gain` then returns unity for
every sample, so no denoising occurs. Near cancellation, division by a tiny
time-domain sample produces rapidly varying gains; clamping them to `[0, 2]`
does not make the estimator meaningful and permits unintended +6 dB
amplification. This can add modulation/distortion precisely on wide, decorrelated,
or phase-shifted stereo material.

The stereo tests use identical left and right signals
(`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:443-480`), which is
the one case that cannot reveal downmix cancellation or image problems.

Fix: expose the model's smoothed band suppression gains (and optionally VAD) and
apply one linked spectral gain decision to both channels, or use an
energy-preserving mid/side feature design with explicitly documented linking.
Do not infer suppression through sample-wise division. Test identical,
anti-phase, quadrature, uncorrelated, hard-panned, and unequal-level stereo;
measure suppression, peak gain, inter-channel correlation, ILD, and image
preservation.

### P1 — Enable/bypass transitions are neither state-continuous nor click-safe

The `enabled` parameter changes a boolean immediately with no ramp or crossfade
(`src/lib.rs:101-110`). In bypass, the backend queues raw frames and does not call
`DenoiseState::process_frame`; when enabled, it calls the neural model
(`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:120-193`). All
temporal analysis, pitch, synthesis, and recurrent state therefore freezes while
bypassed. Re-enabling feeds current audio into stale state and hard-switches from
raw to filtered audio. Disabling hard-switches in the other direction. Both can
click, and a long bypass makes the restored model state unrelated to the current
signal.

Fix: maintain a latency-aligned dry path, continue advancing model state while
bypassed, and crossfade dry/wet over a short documented interval. If CPU-saving
hard bypass is required, define a reset/warm-up transition and still crossfade.
Test toggles at model-frame and host-block boundaries, repeated automation,
long-bypass re-entry, DC offsets, impulses, and steady tones; bound sample-to-
sample discontinuity and verify constant latency.

### P1 — FFT planning and allocation can occur on the first audio callback

The vendored model stores FFT plans, windows, and the DCT table in a global lazy
`OnceCell`. Its initializer constructs the tables and calls the forward and
inverse RustFFT planners
(`crates/sotf-plugins/crates/nnnoiseless/src/lib/consts.rs:184-215`). Transform
functions call this lazy accessor (`consts.rs:238-270`). Plugin initialization
creates per-channel `DenoiseState` and wrapper buffers but never forces the
shared common state, so the first `process_frame` may allocate and plan FFTs on
the audio callback.

The workspace zero-allocation harness hides this by running 20 warm-up calls
before it starts counting
(`crates/sotf-plugins/tests/realtime_allocation_tests/tests.rs:40-60,947-953`).
Consequently the passing test does not prove allocation-free first-use behavior.

Fix: add an explicit `nnnoiseless::prepare()` and invoke it during plugin
initialization, or make the immutable FFT resources explicit instance/shared
state created off the audio thread. Add an allocation/deallocation test around
the very first callback with no warm-up, plus first-block maximum-duration
measurement. Also test concurrent first construction so shared initialization is
not serialized on an audio thread.

### P2 — The model uses large per-frame stack workspaces despite the wrapper's preallocation work

The wrapper preallocates interleaving and model I/O buffers, but the vendored
model still constructs substantial fixed arrays on every frame. Examples include
`buf: [f32; 960]` and feature/pitch buffers
(`crates/sotf-plugins/crates/nnnoiseless/src/denoise.rs:96-108,225-301`), several
complex spectra and time-domain arrays in `process_frame`
(`denoise.rs:303-355`), two `[Complex<f32>; 960]` arrays in each transform
(`crates/sotf-plugins/crates/nnnoiseless/src/lib/consts.rs:238-270`), and multiple
128/384-element neural-layer arrays
(`crates/sotf-plugins/crates/nnnoiseless/src/rnn.rs:183-186,234-237`). This is
allocation-free in the heap sense, but likely consumes tens of kilobytes of
callback stack and repeatedly zero-initializes large workspaces.

That can overflow a constrained audio-thread stack, inflates worst-case latency,
and makes the changelog's general claim that scratch allocation was moved to
initialization incomplete.

Fix: put reusable spectral, feature, pitch, transform, and RNN scratch in
`DenoiseState` or a caller-owned workspace prepared at initialization. Keep
channel processing sequential if stack/CPU is the constraint. Measure stack
high-water and callback p50/p95/p99/max before and after, and require output
parity against the current model/reference vectors.

### P2 — Multichannel support has undefined spatial semantics and scales model cost linearly

Only exactly two channels take the linked path. Mono and every layout wider than
stereo run an independent model per channel
(`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:157-183`). Independent
recurrent gain decisions can change inter-channel correlation, ILD, ambience,
and moving-image stability. It also performs an RNN and forward/inverse FFT set
for every channel, so 7.1.4 costs roughly twelve mono model evaluations per 480
samples. The catalog nevertheless advertises all standard channel widths
(`crates/sotf-plugins/src/factory/catalog.rs:923-940`). Existing high-channel
coverage checks only that output remains finite; it does not establish spatial
quality or deadline margin.

Fix: either advertise and enforce mono/stereo only, or define channel-layout
policy: linked front/surround pairs, explicit center treatment, and normally no
speech denoising on LFE. Prefer shared feature/gain computation where channels
are linked. Test 3.0, 5.1, 7.1, and 7.1.4 with correlated programme,
decorrelated ambience, center speech, LFE, and single-channel noise. Track image
metrics and callback deadline distributions, not only finite samples.

### P2 — Non-finite and out-of-domain samples can contaminate persistent model state

The wrapper validates buffer length and frame multiple but not sample values.
The backend scales input by 32768 before model processing
(`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:135-141,160-171`).
NaN or infinity can propagate through persistent high-pass, pitch, spectral, and
RNN state, causing later finite input to remain non-finite. The mono/multichannel
path has no recovery policy. Samples outside nominal `[-1, 1]` also feed the
i16-scaled model outside its trained/expected range; neither clamping nor an
input-headroom contract is documented.

Fix: define one realtime-safe policy: sanitize non-finite samples before state,
return an error and reset state, or both at a safe boundary. Clamp to the model's
documented domain if that matches the desired host contract; otherwise document
headroom and prove numerical stability. Test NaN, positive/negative infinity,
huge finite values, subnormals, and recovery on every channel, including the
stereo linked path.

### P2 — Buffer-length validation can overflow before the backend's checked arithmetic

The wrapper computes `context.num_frames * self.channels` with unchecked `usize`
multiplication (`src/lib.rs:132-142`). Debug builds panic on overflow; optimized
builds wrap and may allow an invalid length past the check. The backend uses
`checked_mul` (`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:103-112`),
but that protection happens too late.

Fix: use `checked_mul` in the public wrapper and return a precise error on
overflow. Centralize validation so wrapper and backend cannot diverge. Reject or
define zero channels explicitly. Test zero and maximum dimensions without
allocating correspondingly large buffers.

### P2 — AU format behavior is advertised generically but fails at common sample rates and buffer sizes

Initialization rejects every sample rate except 48 kHz (`src/lib.rs:115-123`).
The generic AU accepts host format negotiation and channel capabilities without
a Speech Denoiser-specific restriction
(`crates/sotf-plugins/crates/plugins-au/GenericAU/GenericRustAudioUnit.swift:288-333`).
At 44.1 or 96 kHz, FFI plugin creation fails
(`crates/sotf-plugins/crates/plugins-ffi/src/lib/plugin.rs:241-275`), but AU
resource allocation only logs creation failure and does not present a clear
format-negotiation error (`GenericRustAudioUnit.swift:131-155,299-333`). At 48
kHz, ordinary non-480 callbacks fail processing and the render block returns an
Audio Unit error (`GenericRustAudioUnit.swift:341-456`;
`crates/sotf-plugins/crates/plugins-ffi/src/lib/process.rs:25-53`).

Fix: preferably add preallocated asynchronous resampling plus arbitrary callback
framing. If 48 kHz-only is intentional, reject unsupported formats during
resource allocation with a host-visible error and advertise the restriction.
Exercise the real AU at 44.1, 48, 88.2, and 96 kHz with 64/128/256/480/512/1024
and varying render quanta.

### P2 — Reduction metering is computed on the hot path but is not exposed by the plugin

Every processed frame computes input/output sums, division, `log10`, and a
smoothed `avg_reduction_db` (`crates/sotf-plugins/crates/plugins-denoiser/src/rnnoise.rs:160-192,270-274`).
The wrapper exposes no `get_data` implementation or output visualization, so the
host and UI cannot consume the value. The meter also freezes during bypass and
silence, and its `0.9/0.1` smoothing has no documented time constant.

Fix: either publish a typed, preallocated monitoring payload and specify its
update/decay semantics, or remove the unused calculation from the audio path.
If exposed, base smoothing on sample rate/frame duration and test silence,
bypass, reset, and steady known attenuation.

### P2 — Preset/schema coverage does not prove state-version behavior for this plugin

`Params` has one defaulted boolean and declares schema version 1
(`src/params.rs:1-53`), but there is no Speech Denoiser-specific JSON/preset
round-trip, missing-field, unknown-field, or future-version test. Generic preset
serialization is covered elsewhere, yet it does not prove the plugin factory's
parameter migration/default behavior. This is a smaller current risk because
the schema is simple, but it becomes a compatibility trap as controls such as
link mode, latency mode, or metering are added.

Fix: add plugin-level factory/preset tests for absent `enabled`, explicit true and
false, unknown fields, malformed types, version 1 round-trip, and rejection or
migration of unsupported future versions. Keep parameter registration parity
(`PARAMS`, setter, getter/cache, serialized `Params`) in the test.

### P3 — The test and benchmark matrix overstates realtime confidence

The dedicated crate and backend tests cover many basic conditions, but critical
tests encode the zero-frame startup contract, stereo coverage uses only identical
channels, and the allocation test warms the plugin before measurement. The
standard QA and all-plugin performance benchmark cannot run because they use
512-frame blocks, while the catalog still presents the plugin as generally
available and realtime-safe. There are no p95/p99/max deadline measurements for
RNNoise's signal-dependent pitch/RNN work or for high channel counts.

Fix: make QA use the same arbitrary-block contract expected of all plugins, then
benchmark representative speech plus stationary/transient noise at mono,
stereo, 5.1, and 7.1.4. Include cold first process, enabled/bypassed/toggling,
variable callback partitions, and p50/p95/p99/max callback time. Keep the current
480-frame microbenchmark as a backend measurement, not as host-contract evidence.

### P3 — Documentation and changelog claims have drifted from current behavior

The plugin README is only a brief description and does not disclose the strict
48 kHz/480-frame constraints, startup frame deletion, multichannel independence,
or stereo downmix behavior. Shared/plugin agent documentation describes host
framing/resampling responsibilities more broadly than the implementation
provides. Changelog entries about reset allocation and independent stereo
processing are stale relative to the in-place reset and linked-stereo code. The
catalog's broad channel/support claim is also difficult to reconcile with the
engine skip and failing QA.

Fix: after correcting the contracts, document supported sample rates, arbitrary
host framing, true latency, bypass transition, layout policy, model provenance,
and monitoring behavior in one authoritative README. Treat catalog declarations
and executable QA as tested claims. Update historical notes without rewriting
released history: add a current correction entry where necessary.

## Algorithm and realtime assessment

The underlying RNNoise port has the expected broad structure: 20 ms / 960-sample
analysis with a 10 ms / 480-sample process quantum, pitch and Bark-band features,
a recurrent model, spectral suppression, and overlap/synthesis state. The
wrapper correctly refuses to run that fixed-rate model silently at a different
sample rate, preallocates its channel/ring/I/O vectors, uses checked arithmetic in
the backend, and processes multichannel model instances sequentially rather than
recursively or concurrently on the callback.

The main algorithmic priority is to expose or reconstruct suppression decisions
at the spectral-band level. The vendored API currently discards the returned VAD
probability and returns only synthesized mono samples
(`crates/sotf-plugins/crates/nnnoiseless/src/denoise.rs:77-79,303-355`). Deriving
stereo gain by dividing output and input samples loses the model's stable
frequency-domain decision and creates the cancellation defect. A model API that
returns the 22 band gains/VAD, with a stereo- or layout-aware application stage,
would improve correctness and make channel linking explicit.

After streaming, latency, stereo, cold-start allocation, and state-transition
correctness are fixed, optimize the remaining hot path with reusable FFT scratch
and model workspaces, current RustFFT planning APIs, and measured SIMD/quantized
RNN kernels. Those changes should be gated by reference-vector parity and
speech-quality evaluation (for example clean-speech distortion, noise
attenuation, intelligibility, and stereo/spatial preservation), not throughput
alone.

## Strengths

- The 48 kHz model constraint is rejected explicitly instead of silently running
  with a wrong frequency/time scale.
- Backend construction preallocates channel accumulators, output rings, and
  model input/output vectors; reset clears them in place.
- The backend uses checked multiplication before indexing and limits processing
  to the configured channel count.
- The fixed latency intent across bypass is correct even though the current queue
  implementation does not realize it.
- Parameter registration is complete for the single `enabled` control: schema,
  setter, getter/cache, default, and layout are present.
- Backend tests cover silence, reset, invalid sample rate, stereo identity,
  multiple frames, bypass, and high-channel finite output; plugin integration
  tests cover construction, parameters, latency declaration, reset, and factory
  creation.
- The focused warmed-up allocation test passes, establishing that steady-state
  wrapper processing does not perform ordinary heap allocation after lazy model
  resources have been initialized.
- Vendoring makes the exact model and DSP implementation auditable and avoids a
  realtime dependency on an external service or opaque runtime.

## Exhaustive scope reviewed

Every plugin-owned file was read:

- Configuration/documentation: `Cargo.toml`, `AGENTS.md`, `README.md`,
  `CHANGELOG.md`.
- Source/executable: `src/lib.rs`, `src/params.rs`,
  `bin/qa_speech_denoiser.rs`.
- Tests: `tests/integration.rs`, `tests/test_speech_denoiser_plugin.rs`.

The complete shared RNNoise backend was read: `plugins-denoiser/Cargo.toml`,
`README.md`, `CHANGELOG.md`, `src/lib.rs`, and `src/rnnoise.rs`. The unrelated
Hiss and Transient backend implementations were inventoried but are owned by
other plugins and were not treated as Speech Denoiser code.

The vendored `nnnoiseless` implementation was traced completely through
`src/lib.rs`, `src/denoise.rs`, `src/rnn.rs`, `src/lib/consts.rs`, `src/pitch.rs`,
`src/misc.rs`, `src/celt.rs`, and `src/types.rs`, together with its `Cargo.toml`
and `README.md`. `src/model.rs` was checked for generated-model structure, layer
dimensions, and boundaries; the individual generated numeric weight literals
were not manually enumerated because they contain data rather than control flow.

Integration surfaces reviewed include the `sotf-plugins` facade exports and
parameter schema, factory creation/catalog, engine plugin type/settings/default
and configuration conversion, FFI create/process/parameter mapping, AU class,
view controller, metadata and generic lifecycle/render bridge, the
`ParametricInPlacePlugin` adapter, host frame-count/node-buffer propagation,
factory/DSP-matrix/high-channel tests, realtime-allocation tests, all-plugin and
allocation benchmarks, and standard QA helper. TokenSave caller/callee/context
queries were used before targeted source reads.

## Verification

Executed from the workspace root:

```text
cargo test -p plugins-denoiser
  53 passed; 0 failed

cargo test -p sotf-plugin-speech-denoiser
  19 passed; 0 failed

cargo test -p sotf-plugins --test realtime_allocation_tests \
  tests::test_speech_denoiser_zero_alloc -- --exact
  1 passed; 0 failed

cargo run -p sotf-plugin-speech-denoiser --features qa --bin qa-speech-denoiser
  FAILED: panicked because the standard 512-frame QA block is rejected;
  RNNoise requires a multiple of 480
```

`cargo test -p nnnoiseless` was not available as a workspace-package command:
the vendored crate is excluded from the workspace and its standalone test setup
requires development dependencies. No workspace or dependency configuration was
changed merely to force that test.

The passing tests establish the current 480-frame warmed-up baseline. They do
not establish arbitrary host framing, correct latency/timeline preservation,
stereo cancellation behavior, click-free bypass automation, cold-first-callback
allocation freedom, non-finite recovery, spatial multichannel behavior, or AU
format compatibility identified above.

## Remediation status (2026-08-13)

All P1–P3 findings are closed; this review reported no P0 finding.

Retained quality correction: the first closure used a smoothed broadband
output/input RMS ratio for stereo. The final implementation now exposes the
model's actual 22 suppression gains after RNNoise release smoothing and applies
those frequency-dependent decisions to both original channels. A
polarity-aware, energy-normalized detector prevents anti-phase cancellation;
fixed-size cached analyzer data publishes the bounded gains and VAD probability
without audio-thread allocation. This is deterministic implementation evidence,
not an external-corpus speech-quality claim.

- **Arbitrary framing and latency:** preallocated input/output FIFOs accept
  arbitrary callback partitions, return the requested frame count, and emit a
  real constant 480-sample delay without deleting the first processed frame.
  Backend partition/timeline tests cover 1, 16, 31, 63, 64, 127, 128, 256,
  479, 480, 481, 512, 1024, and 4093-frame calls; the engine no longer skips
  Speech Denoiser in its 1024-frame plugin processing test.
- **Stereo correctness:** sample-wise `mono_out / mono_in` and later broadband
  RMS gain inference were removed. Identical, anti-phase, quadrature,
  uncorrelated, hard-panned, and unequal-level tests establish one common set of
  bounded model band gains, no requested amplification, channel-swap symmetry,
  and preservation of phase/level relationships where the inputs are linearly
  related.
- **Bypass continuity:** the model always advances, delayed dry and wet paths
  remain aligned, and enable/bypass changes crossfade over 480 samples. Focused
  tests cover long bypass/re-entry, repeated toggles, bounded discontinuity, and
  constant latency.
- **Realtime memory/performance:** FFT preparation occurs in initialization;
  large FFT, analysis, pitch, synthesis, and RNN workspaces moved into reusable
  state. Tests cover the cold first callback and live toggle with zero heap
  allocations, processing on a 64 KiB thread stack, and reference-vector output
  parity. QA reports mono/stereo p50, p95, p99, maximum callback time, arbitrary
  partitions, toggling, and cold allocation counts.
- **Layout, samples, and validation:** factories/backend/catalog enforce mono or
  stereo, the AU rejects non-48-kHz and non-mono/stereo formats during resource
  allocation, checked multiplication rejects overflow, and non-finite or
  out-of-domain input is sanitized/clamped before persistent model state. Tests
  cover zero/wide layouts, rate/context mismatch, NaN, infinities, huge finite
  input, both stereo channels, and recovery.
- **Meter/schema/docs:** fixed-size cached monitoring publishes actual smoothed
  band gains, bounded VAD probability, and a model-frame generation. Strict
  parameter JSON tests cover absent/default/true/false values, malformed and
  unknown fields, schema v1 round-trip, and future-version rejection. README,
  changelog, catalog evidence, and package versions now state the executable
  48-kHz mono/stereo, latency, bypass, and realtime contracts.

Retained-quality verification (2026-08-13):

```text
cargo test --offline -p plugins-denoiser
  64 passed; 0 failed
cargo test --offline -p sotf-plugin-speech-denoiser
  25 passed; 0 failed
cargo test --offline -p sotf-plugins --test speech_denoiser_factory
  1 passed; 0 failed
cargo test --offline -p sotf-plugins --test realtime_allocation_tests speech_denoiser_zero_alloc
  1 passed; 0 failed
cargo run --offline -p sotf-plugin-speech-denoiser --features qa --bin qa-speech-denoiser
  mono max 162.417 us; stereo max 247.541 us; zero cold allocations
cargo clippy --offline -p plugins-denoiser -p sotf-plugin-speech-denoiser --all-targets --no-deps -- -D warnings
cargo fmt --all -- --check
cargo check --offline -p plugins-denoiser -p sotf-plugin-speech-denoiser
git diff --check
  passed
```

The standalone vendored `nnnoiseless` manifest test could not run offline
because its legacy Criterion development graph requires an uncached `clap`
2.x package. Its reference-vector parity is exercised through the passing
`plugins-denoiser` suite. The repository struct-size checker reports three
pre-existing, unrelated unallowlisted structs in host Convolution/Delay code;
this change adds no over-budget struct.
