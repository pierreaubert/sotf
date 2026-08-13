# Mono to Stereo plugin code review — 2026-08-12

## Findings

### P1 — Streaming latency and sample alignment depend on callback size

`process` consumes as much of the current input block as needed to fill a 2048-sample analysis frame, immediately overlap-adds 512 output samples, and drains them into the beginning of that same callback (`crates/sotf-plugins/crates/sotf-plugin-mono-to-stereo/src/lib/mono_to_stereo_plugin.rs:390-478`). With 512-frame callbacks the first input sample appears after roughly 1536 output frames; with 1024-frame callbacks it can appear at frame 1024; with a single block of 2048 or more it is written at output frame zero after the function has already read future samples from later in that same block. Yet `latency_samples` always reports 2048 (`mono_to_stereo_plugin.rs:481-485`).

This violates stream partition invariance and makes host latency compensation wrong. Use a fixed input/output timeline: enqueue input, emit only samples whose algorithmic time has arrived, and prefill the output delay so processing the same stream with different block partitions yields identical samples. Derive and report the actual WOLA latency (normally `FFT_SIZE - HOP_SIZE` for this causal framing, plus any deliberately uncompensated interchannel effect). Add an impulse test that processes identical input at block sizes 1, 64, 256, 512, 1024, and irregular partitions and asserts identical aligned output and reported first-arrival latency.

### P1 — Frequency-dependent blending is not energy normalized and can create deterministic spectral notches

For intermediate frequency weights the right spectrum is a complex linear interpolation between the original and a random-phase version, with a fixed 1.434 gain applied only to the latter (`mono_to_stereo_plugin.rs:288-304`). The magnitude is therefore phase-dependent: `(1-w)X + w·g·e^{jφ}X` can strongly cancel or boost depending on each bin's deterministic random phase. The fixed gain compensates an asserted average window-energy effect, not this crossfade interference. Width is then mixed a second time in the time domain (`mono_to_stereo_plugin.rs:438-442`), adding another correlated crossfade whose energy changes with phase/correlation.

The result can alter tonal balance, contradicting the documentation, and make intermediate widths quieter or peakier than either endpoint. Define one width mapping, normalize it using the actual cross-correlation (or use an equal-power mid/side topology), and validate per-bin magnitude, broadband RMS, peak gain, and mono-fold response across all width/frequency settings. The current QA's broad 0.8–1.2 total-energy allowance does not catch narrow spectral damage.

### P1 — Factory/preset values bypass schema validation

`from_params` directly sets smoother target, boolean, and Haas delay without finiteness/range checks (`mono_to_stereo_plugin.rs:180-185`). The factory uses this constructor, whereas only `param_bridge::set_parameter` enforces the active schema. A preset can therefore supply NaN/infinite/out-of-range width or Haas delay. Casting a negative/NaN delay calculation to `usize` and clamping it (`mono_to_stereo_plugin.rs:188-191`) hides the invalid configuration; a NaN width propagates to output.

Make construction fallible and route it through the same validation as runtime parameters. Check the declared input channel count rather than ignoring `_channels`. Test factory creation with finite endpoints, negative/extreme values, NaN/infinity, and a non-mono channel count.

### P1 — The process path panics instead of returning errors for invalid buffer sizes

Input and output are indexed from `context.num_frames` without checked multiplication or bounds validation (`mono_to_stereo_plugin.rs:401-405`, `436-475`). A short mono input or stereo output panics; an oversized buffer has undocumented tail semantics. This is especially risky for a channel-changing plugin where host sizing mistakes are more likely.

Use checked frame/sample counts, validate both buffers before mutating state, and return descriptive errors. Add zero/short/oversized/overflow tests and ensure an error leaves streaming state unchanged.

### P2 — The Nyquist bin loses unit magnitude

The random phase filter is unit magnitude for ordinary bins, but the code makes the Nyquist bin real by discarding its imaginary component while retaining `cos(phase)` as the real part (`mono_to_stereo_plugin.rs:194-213`). Real-FFT DC/Nyquist bins must be real, but setting the imaginary part to zero this way changes magnitude to `|cos φ|`; it should be a real sign (`+1` or `-1`) or remain unity. At some sample rates this attenuates the top bin and undermines the all-pass claim.

Set both constrained bins explicitly according to the intended response and add a unit-magnitude/conjugate-validity test for every filter bin.

### P2 — Parameter updates can abruptly replace the decorrelator while audio state is live

Changing either decorrelation boundary regenerates the entire deterministic phase filter immediately (`mono_to_stereo_plugin.rs:156-170`, `194-217`). Existing overlap-add frames were synthesized with the old filter while subsequent frames use the new one, with no crossfade. `freq_dependent` also switches the spectral topology instantly. These controls are exposed as normal runtime parameters.

Prepare new curves/filters on the control thread and crossfade old/new processed spectra over a bounded interval, or mark these parameters rebuild-only. Add click/peak tests for boundary and mode automation at adversarial phases and block offsets.

### P2 — Random independent bin phases produce a long circular/noncausal decorrelation impulse response

`decorrelation_phase` hashes each FFT bin independently (`mono_to_stereo_plugin.rs:219-230`). The resulting phase response has no smoothness or causal all-pass structure; its IFFT spreads a transient across the full 2048-sample frame. Repeated STFT processing hides circular wrap with windowing but still smears attacks and can create pre-echo-like energy relative to frame alignment. This is algorithmically unlike a short decorrelator even though the docs market natural widening.

Compare against cascaded Schroeder/all-pass decorrelators or a designed minimum/controlled-phase filter, with listening tests and objective transient spread, interchannel coherence, mono-fold, and spectral-flatness metrics. This is an algorithm-improvement recommendation rather than proof that decorrelation itself is invalid.

### P3 — Settled width still advances a smoother sample by sample and every hop performs two inverse FFTs

The hot path is allocation-free and preplanned, but always performs separate left and right inverse transforms (`mono_to_stereo_plugin.rs:273-319`) and advances width per emitted sample (`mono_to_stereo_plugin.rs:436-442`). At width zero with zero Haas delay, exact duplication needs no right FFT/decorrelation; settled static widths can avoid smoother calls. Similar special cases exist below the decorrelation band.

Add fast paths for width=0, settled parameters, and possibly reuse the direct left path without inverse-transforming it twice. Benchmark before more complex SIMD work. Ensure fast paths preserve the corrected fixed-latency stream.

### P3 — User documentation advertises controls that were removed

`USAGE.md` and `UI.md` still describe `enable_comp_eq` and `comp_eq_depth_db`, while the active five-parameter schema explicitly removed them (`src/params.rs:1-105`). `USAGE.md` also says latency is 2048 samples, reinforcing the incorrect runtime report, and describes tonal preservation more strongly than tests justify. Package `0.5.4` also trails the `0.5.5` changelog.

Update docs/UI/package metadata from the active schema and measured latency after the streaming fix.

## Algorithm and realtime assessment

The WOLA implementation preallocates FFT, accumulator, delay, and scratch buffers; FFT plans are created outside processing. The audio callback takes no locks, allocates no memory on successful transforms, and returns `context.num_frames`. Hann dual-window scaling for 75% overlap is structurally appropriate for the coherent path. Reset clears input, overlap-add, output, latency, width, and Haas state.

The plugin intentionally changes one input channel to two outputs and marks itself as a channel-mixing compile boundary. The optional Haas delay affects only the right channel and is intentionally omitted from host latency, but the common STFT component still must be fixed and reported consistently. Parameter/schema cloning and filter regeneration allocate or do substantial work only on control paths; the host must not call setters on the realtime thread.

## Scope reviewed

Read every plugin-owned file: `AGENTS.md`, `README.md`, `USAGE.md`, `UI.md`, `CHANGELOG.md`, `Cargo.toml`, all source modules, all unit/integration tests, and `bin/qa_mono_to_stereo.rs`. Also checked factory/catalog/facade wiring, host `Plugin`/smoother/parameter bridge contracts, FFT conventions, and TokenSave test-risk results. No production code was changed.

## Existing strengths

- FFT plans and all large audio scratch/ring buffers are preallocated; the hot path is lock- and allocation-free.
- Parameters are smoothed where appropriate, parameter IDs/defaults are centralized, and runtime adapter values are clamped/type-checked.
- Reset and sample-rate initialization cover the main state buffers and recalculate rate-dependent decorrelation/delay state.
- Tests cover parameter wiring, state reset, basic width behavior, finite output, energy, and factory integration, providing a base for stronger streaming/reference tests.
- Compile metadata correctly declares channel mixing and a hard optimization boundary.

## Remediation status

- **Fixed and tested — P1 stream timing:** the random-phase WOLA pipeline was removed. The causal
  sample-by-sample all-pass path has zero lookahead and reports zero latency. Partition-invariance
  tests compare sample, 64/256/512/1024-frame, and irregular callback partitions.
- **Fixed and tested — P1 energy normalization:** width now moves stable all-pass poles and
  coefficients instead of linearly summing coherent dry/random spectra. First- and second-order
  analytic responses are checked at every FFT test frequency; rendered tones cover five widths and
  six frequencies and verify settled left/right RMS balance, bounded channel peaks, and a
  non-boosting mono fold.
- **Fixed and tested — P1 factory validation:** `try_from_params` rejects non-mono construction,
  non-finite values, and values outside all declared numeric ranges. Facade construction preserves
  the structural decorrelation crossover fields, rejects crossovers at/above host Nyquist, and has
  endpoint, non-finite, out-of-range, low-rate, and round-trip regressions.
- **Fixed and tested — P1 buffer validation:** checked stereo-length multiplication and short input
  and output checks run before state mutation. Exact-size semantics reject oversized buffers too;
  short, oversized, and overflow regressions verify unchanged decorrelator state/output sentinels.
- **Fixed by replacement — P2 Nyquist attenuation:** the real-FFT filter no longer exists. Every
  causal all-pass section is unit magnitude by construction, including DC and Nyquist, and the
  bin-wise analytic tests cover both endpoints.
- **Fixed and tested — P2 live topology replacement:** `decor_low_hz`, `decor_high_hz`, and
  `freq_dependent` are structural schema parameters. They may be configured before initialization;
  post-initialization writes return an error requiring a graph rebuild and leave values unchanged.
- **Fixed and tested — P2 circular/noncausal decorrelation:** independent random bins were replaced
  with a stable causal cascade of three second-order all-pass sections and one first-order section.
  An offset-impulse regression proves exact silence before the impulse and immediate causal output.
- **Fixed and tested — P3 avoidable hot-path work:** removing WOLA eliminates both inverse FFTs and
  their per-hop buffer work. Settled widths use one block-level branch and never advance the
  smoother; settled width zero with zero Haas delay bit-exactly duplicates input without running
  the decorrelator. Deterministic counters prove both selected paths, and the QA binary retains the
  counting-allocator realtime check.
- **Fixed — P3 documentation/metadata:** README, usage signal flow/latency, UI structural-control
  behavior, changelog, package and host-visible versions, and exact lockfile entry now describe
  version 0.5.8. Zero/invalid sample rates are rejected atomically before DSP state changes.
- **Intentional, tested — Haas interchannel offset:** Haas delay remains an audible right-channel
  widening effect and is intentionally excluded from host latency. Its live parameter update and
  latency contract have regression coverage.

All P0-P3 findings in this review are resolved; no remediation remains deferred.

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-mono-to-stereo --offline
cargo clippy -p sotf-plugin-mono-to-stereo --all-targets --offline -- -W warnings
cargo check -p sotf-plugins --no-default-features --offline
cargo test -p sotf-plugins --test all_plugins_dsp_matrix --no-default-features --offline
```
