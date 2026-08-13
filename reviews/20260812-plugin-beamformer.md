# Beamformer plugin code review

## Remediation status — 2026-08-12

All P1–P3 findings are fixed in `0.5.4` (this review reported no P0 finding):

- The causal WOLA scheduler advances an independent synthesis cursor by one
  hop, preserves the full 512-sample advertised latency, and is sample-identical
  under 64/127/256/512/1024 and irregular callback partitions
  (`stft_output_is_causal_and_block_partition_invariant`).
- GSC fractional delays use the next older ring sample; fixed and blocking paths
  share the aligned vector; latency is the ceiling of the common compensation
  delay (`fractional_delay_uses_next_older_sample`,
  `target_plane_waves_are_distortionless_across_angles`, `latency_depends_on_type`).
- Microphone count, spacing, steering, and algorithm are setup-only structural
  state. Runtime setters return errors, avoiding allocation, hard resets, stale
  state, and latency changes on the realtime path (`structural_parameters_require_rebuild`,
  `beamformer_type_is_structural`, `steering_is_structural_and_does_not_rebuild_live_state`).
- `params::Params` is the sole runtime/UI serialized schema;
  `BeamformerPluginParams` is only its compatibility alias. Canonical algorithm
  strings are written while legacy numeric values are migrated
  (`runtime_and_ui_state_share_canonical_algorithm_serde`). Both factories use
  the same fallible constructor and reject malformed geometry/state rather than
  asserting (`malformed_construction_returns_errors`,
  `beamformer_factory_matches_fallible_constructor_validation`,
  `beamformer_bridge_returns_errors_for_malformed_state`).
- MVDR covariance acquisition now uses a scale-invariant coherent
  look-direction fraction to protect target-dominant frames and learn
  incoherent noise at any gain. Unchanged covariance skips all weight solves
  (`target_presence_estimator_is_scale_invariant_and_marks_noise_dirty`,
  `mvdr_skips_weight_solves_when_covariance_is_unchanged`).
- MVDR now solves the diagonally loaded Hermitian positive-definite system with
  an in-place Cholesky solve rather than forming an inverse. MVDR and
  superdirective singular fallbacks retain steering phase and unity look gain
  (`singular_fallback_is_steered_and_distortionless`,
  `steered_fallback_preserves_look_direction`).
- Reinitialization rebuilds the frequency-grid-dependent state and clears
  covariance, overlap, OLA, adaptive, and delay history. Reused instances match
  fresh instances across 48→96→44.1 kHz transitions
  (`sample_rate_reinitialization_discards_old_grid_and_pending_audio`).
- Processing validates sample rate and all input/output bounds before state is
  touched (`process_validates_buffers_and_sample_rate_before_mutating_state`).
- Quality tests now require distortionless target transfer at 0/30/60/90° and
  more than 7 dB target-referenced GSC interference improvement. Documentation
  consistently defines 0° as broadside and ±90° as endfire.
- QA exercises maximum eight-microphone layouts for MVDR, superdirective, and
  GSC, verifies zero warmed-callback allocations, and checks p50/p95/maximum
  callback time against the audio deadline.

Final verification commands and measured results are recorded at the end of
this review.

Date: 2026-08-12

Scope: `sotf-plugin-beamformer`

Focus: correctness, beamforming quality, realtime allocation, and performance

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-beamformer/` unless a repository-relative
path is shown.

## Findings

### P1 — Consecutive STFT frames are overlap-added at the same ring position

The plugin declares both OLA read and write positions
(`src/lib/beamformer_plugin.rs:36-40`), but synthesis writes every completed
frame relative to `ola_read_pos` (`src/lib/beamformer_plugin.rs:315-325`).
`ola_write_pos` is initialized and reset but never otherwise read or advanced
(`src/lib/beamformer_plugin.rs:86-95`, `225-235`). Because output is drained only
after the entire host block has been ingested (`src/lib/beamformer_plugin.rs:338-346`),
two or more FFT frames produced in one call are accumulated at exactly the same
offset instead of hop-separated offsets. A 512-frame call produces two frames
after startup, so this affects an ordinary block size, not only oversized calls.
It causes amplitude errors, time smearing, and output that changes when the same
stream is divided into different host blocks.

Fix: maintain an independent synthesis write cursor, add each IFFT frame at that
cursor, advance it by the 256-sample hop, and track how many output samples are
ready. Prefer the shared tested STFT/ring accumulator rather than another custom
ring. Add identity delay-and-sum tests comparing 64, 127, 256, 512, 1024, and
irregular block partitions sample-for-sample after latency; check impulse
location, unity DC/sine gain, and no hop-rate amplitude modulation.

### P1 — STFT output is emitted before the plugin's reported 512-sample latency

The first FFT is completed at the end of the first 512 input samples, then its
whole synthesized frame is immediately drained into the *same* output call
(`src/lib/beamformer_plugin.rs:270-336`, `338-346`). The implementation does not
first emit 512 samples of startup silence or queue the frame for a later output
time. Nevertheless `latency_samples()` returns 512 and claims output lags input
(`src/lib/beamformer_plugin.rs:353-360`). This creates noncausal within-block
placement from the host's sample timeline and makes delay compensation wrong.
The existing OLA test says the first call is latency fill but does not assert
that it is silent (`src/lib/tests.rs:117-131`).

Fix: explicitly model startup latency and ready-output count, producing silence
until the declared delay has elapsed. Verify with impulses at every position in
a block and compare detected versus reported latency for MVDR and
superdirective across varied blocks. Assert the entire startup interval, not
only that later output is nonzero.

### P1 — GSC fractional-delay interpolation uses the wrong neighboring sample

For delay `D = floor(D) + frac`, linear interpolation should combine
`x[n-floor(D)]` and the next *older* sample `x[n-floor(D)-1]`. The implementation
sets `idx0` to the integer-delayed sample but uses `(idx0 + 1) % len` for the
second tap (`src/gsc.rs:121-132`). Since the current sample was just written at
`delay_write_pos` (`src/gsc.rs:116-119`), a 1.5-sample request blends `x[n-1]`
with `x[n]`, implementing roughly 0.5 samples; a 0.5-sample request can read the
ring cell ahead of the write head, i.e. stale oldest history. Only an exact
one-sample delay is tested (`src/gsc.rs:312-336`), where `frac == 0` masks the
bug.

Fix: interpolate between the integer-delayed cell and its predecessor in ring
time (or use the shared fractional-delay implementation). Test 0, 0.25, 0.5,
1.0, 1.5, and near-maximum delays with impulses and swept sines, including ring
wrap. Assert magnitude and phase against an offline fractional-delay reference.

### P1 — GSC blocks unaligned microphones and therefore leaks/cancels the look source

The fixed beamformer uses delay-compensated microphone samples
(`src/gsc.rs:116-136`), but the blocking matrix is applied to the original,
unaligned `mic_samples` (`src/gsc.rs:138-145`). The real projection matrix is
constructed as the null space of a uniform, already-aligned steering vector
(`src/gsc.rs:50-67`). For a non-broadside target the raw microphone vector is
not uniform, so `B x` contains target energy. NLMS then treats that leaked target
as a noise reference and adapts to cancel it (`src/gsc.rs:156-189`). This defeats
the distortionless constraint at precisely the angles steering is meant to
support.

Fix: compute and retain the delayed/aligned sample vector once per input sample;
feed both the fixed sum and blocking projection from that same vector. An
alternative frequency-domain GSC must use a blocking matrix satisfying
`B(f)d(f)=0` per bin. Test target-only plane waves at broadside, ±30°, ±60°, and
endfire: blocking-reference energy should be near zero and target transfer gain
should remain near unity before and after long adaptation. Then add an
independent interferer and require measurable SINR improvement.

### P1 — GSC reports zero latency despite steering delay lines

GSC output delays early microphones by geometry-dependent compensation delays;
the delay-line length is derived from the maximum steering delay
(`src/gsc.rs:77-100`) and the fixed beamformer reads those delayed samples
(`src/gsc.rs:121-136`). `latency_samples()` nevertheless returns zero for every
GSC configuration (`src/lib/beamformer_plugin.rs:353-359`). At the supported
maximum of eight microphones spaced 50 cm, endfire compensation is hundreds of
samples at 48 kHz. Returning zero prevents the host from aligning this path with
parallel/dry paths.

Fix: expose the effective maximum compensation delay (with a documented
fractional-delay convention) and report it for GSC, or add a fixed common delay
that makes latency independent of angle. Test detected impulse delay against
metadata at representative spacings, angles, microphone counts, and sample
rates.

### P1 — Live algorithm switching changes latency and reuses incompatible stale state

The schema correctly marks `beamformer_type` structural
(`src/params.rs:50-58`), yet `set_parameter` switches it in-place
(`src/lib/beamformer_plugin.rs:189-202`), and an integration test explicitly
requires mid-stream switching to be allowed (`tests/integration.rs:155-178`).
Switching between GSC and STFT modes changes reported latency from 0 to 512 while
leaving STFT input overlap and OLA contents intact. Audio queued before a switch
can be abandoned and later emitted stale when switching back; adaptive MVDR/GSC
state is also preserved across semantically discontinuous modes. The host cannot
recompile graph delay compensation atomically with this setter.

Fix: enforce graph replacement for the structural algorithm choice. If live
switching is required, run both paths with a common fixed latency, reset/prime
state deterministically, and crossfade at a safe boundary. Test repeated switches
on impulses and tones, checking no stale samples, discontinuities, or latency
metadata mismatch.

### P1 — Steering automation performs large allocation/planning work and hard resets GSC

Steer angle is exposed as a normal realtime parameter (`src/params.rs:39-49`).
Every update calls `update_steering` (`src/lib/beamformer_plugin.rs:189-194`),
which allocates nested steering vectors, constructs 257 diffuse-coherence
matrices and inverses for superdirective weights, and replaces the complete GSC
including delay lines and adaptive weights (`src/lib/beamformer_plugin.rs:139-163`,
`src/superdirective.rs:35-115`). The changelog itself acknowledges that
`set_parameter` allocates (`CHANGELOG.md:90-92`). On a realtime/control boundary
this can cause drop-time work, glitches, and abrupt loss of adaptation; angle
steps also produce no coefficient/delay smoothing.

Fix: compute steering states off the audio thread and swap prepared immutable
weights/state at a defined boundary. For GSC, interpolate delays safely and
crossfade or preserve compatible adaptive state. Alternatively mark angle setup-
only. Add allocation/deadline tests around parameter application, plus rapid and
slow angle sweeps with click-energy and target-gain assertions.

### P1 — Public serialized parameter schemas disagree, and one factory can panic

The file described as the single source of truth serializes `beamformer_type` as
a choice string (`src/params.rs:92-107`, `137-165`). The actual plugin/factories
instead use a second `BeamformerPluginParams` with a numeric `usize`
(`src/lib/beamformer_plugin_params.rs:6-17`), as do engine settings and conversion
(`crates/sotf-engine/src/plugins/plugin_settings.rs:1255-1264`,
`crates/sotf-engine/src/plugins/plugin_config_converter/spatial.rs:7-24`). Thus a
generic versioned state such as `"beamformer_type":"MVDR"` is not accepted by
the factory that expects `0`, and the claimed single source of truth is not the
runtime source.

The primary factory validates 2..=8 microphones and channel agreement
(`crates/sotf-plugins/src/factory/create.rs:447-463`), but plugins-bridge directly
calls the infallible constructor without either check
(`crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:321-324`). Invalid
counts then hit assertions in MVDR/GSC construction (`src/mvdr.rs:53-57`,
`src/gsc.rs:47-48`) and can terminate the bridge process. Neither path validates
finite/ranged spacing and angle before geometry calculations.

Fix: retain one versioned parameter type and one fallible constructor shared by
all factories/bridges. Accept an explicit migration for old numeric/string
algorithm representations, reject non-finite/range-invalid geometry, and return
errors rather than asserting. Add identical malformed-state matrices through
primary factory, bridge, engine conversion, FFI, and preset round trips.

### P1 — MVDR's “noise covariance” updates only below an absolute FFT-energy gate

MVDR averages unnormalized STFT-bin power over microphones and compares it with
a fixed `0.01` threshold (`src/mvdr.rs:67-74`, `87-106`). This is not a target-
absence or noise-presence estimator: ordinary stationary noise above the
threshold is never learned, while sufficiently quiet desired speech/music is
learned as noise. The decision also depends on FFT/window scaling, microphone
gain, and channel count. When the gate stays closed, covariance remains its
identity initialization and MVDR reduces to a steered delay-and-sum response,
despite README's “optimal noise rejection” claim. The existing regression only
checks that one loud channel prevents an update; it does not demonstrate useful
noise tracking or SINR improvement (`src/lib/tests.rs:139-181`).

Fix: use a documented noise/target-presence estimator (e.g. per-bin minima/MCRA,
VAD/reference capture, or covariance decomposition) with calibrated units and
bounded adaptation. Expose learning state or a noise-reference workflow. Test
convergence and tracking for quiet/loud diffuse noise, target-only input, target
at startup, changing interferers, gain scaling, and multiple sample rates;
report distortionless target gain and output SINR, not only finite matrices.

### P2 — Singular fallbacks are not steered delay-and-sum weights

Both MVDR and superdirective claim to fall back to delay-and-sum, but fill every
frequency-bin weight with real `1/M` (`src/mvdr.rs:194-204`,
`src/superdirective.rs:88-107`). Since application computes `w^H x`
(`src/mvdr.rs:276-288`, `src/superdirective.rs:125-135`), an off-broadside
delay-and-sum fallback must retain steering phase, normally `w=d/(d^H d)` for
unit-magnitude entries. Uniform real weights steer only broadside and can comb-
filter or null an intended endfire source. Regularization makes fallback rare,
but fallback correctness matters precisely under numerical stress.

Fix: use normalized steering-vector weights in every inversion/denominator
fallback. Add forced-singular tests at several angles/frequencies and assert
unity response to the look-direction steering vector.

### P2 — Sample-rate reinitialization preserves state expressed on the old frequency grid

`initialize` changes sample rate and reconstructs steering, superdirective, and
GSC state but does not reset MVDR covariance, STFT input overlap, or pending OLA
audio (`src/lib/beamformer_plugin.rs:219-223`). MVDR covariance bins learned at
the old physical frequencies are immediately combined with steering vectors for
the new grid; pending time-domain samples cross the rate boundary. This can emit
stale audio and invalid weights after a device-rate change.

Fix: make initialization a complete state transition: rebuild/reset all adaptive,
FFT overlap, delay, and output state, or require a fresh instance. Test
48→96→44.1 kHz transitions after nonzero/adaptive input, then silence; assert no
stale output and equivalence to a newly constructed instance.

### P2 — `process` indexes caller buffers without validating their contracts

The method trusts `context.num_frames` and directly indexes
`input[i*num_mics+ch]`, `output[i]`, and `output[..nf]`
(`src/lib/beamformer_plugin.rs:238-256`, `270-274`, `342-346`). Short input or
output buffers panic instead of returning the declared `Result`. It also ignores
`context.sample_rate`, so a mismatched context proceeds with steering prepared
for another rate.

Fix: validate exact/at-least input and output lengths and the initialized sample
rate before touching state, returning an error without partial mutation. Add
short/long buffer, zero-frame, and mismatched-rate tests for all three modes.

### P2 — MVDR repeats identical matrix inversions when covariance has not changed

Every STFT frame calls `update_noise_covariance`, then recomputes weights for all
257 bins (`src/lib/beamformer_plugin.rs:289-295`). When the absolute energy gate
classifies the frame as signal, covariance returns unchanged
(`src/mvdr.rs:102-107`), yet `compute_weights` still performs a Gauss-Jordan
inverse per bin (`src/mvdr.rs:150-176`). At eight microphones and a 256-sample
hop this is avoidable O(bins × M³) work on the audio thread. QA measures GSC
throughput after switching back to GSC and does not benchmark MVDR or
superdirective performance (`bin/qa_beamformer.rs:50-55`).

Fix: have covariance update return a dirty/version flag, recompute only changed
bins or on steering changes, and use a Hermitian positive-definite solve
(Cholesky/LDL with robust loading) rather than explicitly forming an inverse.
Benchmark p50/p95/worst block time for 2/4/8 microphones and all modes on
programme+noise, including covariance-changing and unchanged frames.

### P3 — Tests and documentation assert implementation activity, not beamforming quality

Most tests require only successful processing or finite output. The GSC
“noise cancellation” test accumulates error but asserts merely that the number
is finite (`src/gsc.rs:234-261`); the STFT OLA regression asserts only non-silence
(`src/lib/tests.rs:104-137`). There is no beampattern, white-noise gain,
distortionless-response, directivity-index, target leakage, SINR improvement, or
block-invariance reference test. QA uses constant identical channels and reports
GSC performance, which cannot distinguish correct steering from averaging.

Angle documentation is internally contradictory: the public steering doc says
0° endfire and 90° broadside (`src/steering.rs:68-76`), while the implemented
direction vector, delay doc, and numeric tests define 0° broadside and 90°
endfire (`src/steering.rs:92-96`, `135-145`, tests below line 174). Changelog
0.5.1 claims the former (`CHANGELOG.md:60-65`) while 0.5.0 claims the math was
rotated to the latter (`CHANGELOG.md:104-110`). Package version is 0.5.1 while
the changelog starts at 0.5.2 (`Cargo.toml:1-5`, `CHANGELOG.md:1-8`).

Fix: establish one coordinate convention with diagrams and measured plane-wave
fixtures, then test the expected response. Replace finite-only assertions with
quantitative quality thresholds and block/reference comparisons. Align package,
changelog, README, and API docs.

## Algorithm and realtime assessment

The intended contracts are: linear 2–8 microphone array, 1–50 cm spacing,
azimuth steering, mono output; 512-point real STFT with 256-sample hop and
sqrt-periodic-Hann analysis/synthesis for MVDR/superdirective; time-domain
delay-and-sum plus blocking-matrix NLMS for GSC. The inverse FFT is correctly
scaled by `1/512`, matching `RealFftProcessor`'s unnormalized inverse, and the
window pair has a tested 50% COLA property in `math-dsp`.

Hot processing buffers, FFT plans, covariance scratch, output spectra, GSC
reference history, and delay lines are preallocated. Focused tests confirm zero
heap allocation during warmed MVDR and GSC processing. That result does not make
parameter changes realtime-safe, and it does not establish correct time
placement: the OLA write scheduling is currently the dominant correctness issue.

For algorithm quality, the superdirective diffuse-field coherence model and
diagonal regularization are recognizable, and MVDR preserves the standard
`R^-1 d / (d^H R^-1 d)` form. The weak link is covariance acquisition: without a
credible target-absence/noise estimator, correct matrix algebra cannot deliver
the advertised adaptive rejection. GSC likewise needs exact steering alignment
in both fixed and blocking paths before NLMS convergence metrics are meaningful.

## Strengths

- The plugin has clear mode separation and an explicit M-input/mono-output host
  contract.
- FFT plans and main adaptive/spectral scratch storage are prepared outside the
  warmed process path; dedicated MVDR and GSC zero-allocation tests pass.
- The shared periodic sqrt-Hann window and `1/N` inverse normalization are the
  correct ingredients for 50% WOLA when scheduled correctly.
- MVDR covariance uses all available microphones rather than channel 0 only.
- MVDR matrix work uses fixed-size scratch buffers capped at eight microphones,
  avoiding per-frame matrix allocation.
- Superdirective weights are precomputed, so steady-state application is a
  bounded complex dot product per bin.
- Reset clears MVDR covariance/weights, GSC adaptive/delay history, STFT input,
  and OLA storage.
- DC and Nyquist imaginary parts are explicitly repaired before real IFFT.
- Steering utilities support linear, circular, and custom geometries and have
  unit-magnitude and delay tests, even though the plugin currently exposes only
  linear arrays.

## Exhaustive scope reviewed

Every plugin-owned file was read:

- Documentation/configuration: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`,
  `README.md`.
- QA: `bin/qa_beamformer.rs`.
- Core: `src/lib.rs`, `src/gsc.rs`, `src/mvdr.rs`, `src/params.rs`,
  `src/steering.rs`, `src/superdirective.rs`.
- Split implementation: `src/lib/beamformer_plugin.rs`,
  `src/lib/beamformer_plugin_params.rs`, `src/lib/default.rs`,
  `src/lib/misc.rs`, `src/lib/types.rs`.
- Tests: `src/lib/tests.rs`, `tests/integration.rs`, plus inline tests in GSC,
  MVDR, steering, superdirective, and params modules.

No plugin-owned examples or benchmark files exist. The relevant workspace
allocation benchmark and realtime-allocation test were read. Integration review
also covered facade exports and param-spec re-exports; primary factory validation;
catalog input/output, latency, schema, and zero-allocation claims; plugins-bridge
construction; FFI/NIH schema mapping; engine settings, generated accessors, and
config conversion; available preset/config searches (no Beamformer preset
fixture was found); and `math-dsp` real FFT/window implementation and tests.

TokenSave located the active symbols, callers, tests, and integration surfaces
before source reads; it saved approximately 44,600 context tokens during this
review. Its separate `math-audio` graph was on an older schema, so the three
known relevant STFT files were read directly after that query failed.

## Verification

Executed from the workspace root:

```text
cargo test -p sotf-plugin-beamformer
  58 passed; 0 failed (3 suites)

cargo check -p sotf-plugin-beamformer
  passed

cargo clippy -p sotf-plugin-beamformer --all-targets --no-deps -- -D warnings
  passed

cargo test -p sotf-plugins --test realtime_allocation_tests test_beamformer_zero_alloc --no-default-features --offline
  1 passed; 0 failed; 49 filtered out

cargo run -p sotf-plugin-beamformer --features qa --bin qa-beamformer
  MVDR 8-mic p50/p95/max: 0.135/0.156/0.191 ms
  Superdirective 8-mic p50/p95/max: 0.042/0.043/0.043 ms
  GSC 8-mic p50/p95/max: 0.106/0.108/0.112 ms
  callback deadline: 10.667 ms; zero allocations: PASS

cargo test -p plugins-bridge beamformer_bridge_returns_errors_for_malformed_state
  1 passed; 0 failed

cargo test -p sotf-plugins beamformer_factory_matches_fallible_constructor_validation --no-default-features --offline
  1 passed; 0 failed
```
