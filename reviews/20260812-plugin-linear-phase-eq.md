# Linear Phase EQ audio plugin code review — 2026-08-12

## Final remediation status — 2026-08-14

All retained P1–P3 findings in `0.5.6` are remediated (no P0 was reported).
The engine continues to use its summed queued-work horizon. Direct AU, CLAP,
VST3, and fixed-rate FFI instances now wrap Linear Phase EQ in a bounded,
preallocated asynchronous timeline adapter, so they no longer execute the FIR
worker inside the physical callback.

- The direct-format quantum is
  `max(plugin.realtime_quantum_frames(), negotiated_max_callback_frames)`.
  AU uses `maximumFramesToRender`, NIH uses `BufferConfig.max_buffer_size`, and
  FFI consumes a checked reserved construction-JSON maximum without changing
  its v3 ABI. Facade metadata is stripped before strict plugin schemas are
  deserialized. The adapter reports the inner DSP latency plus exactly two
  adapter quanta. Oversized FFI callbacks are rejected and fully silenced
  without advancing adapter state.
- The callback moves fixed-capacity blocks through safe-Rust SPSC queues. It
  does not wait, lock, allocate, deallocate, log, join, or destroy plugin-owned
  resources. Overload advances absolute input/output time with bounded silence;
  late results and old reset epochs are discarded rather than replayed shifted.
- Parameter commands carry absolute frame timestamps. NIH enables its
  sample-accurate buffer splitting only for Linear Phase EQ, and AU retains its
  existing point/ramp event timeline. Reset preserves accepted realtime state
  and rejects stale work. Worker failures latch full-silence behavior until a
  reset or reconstruction.
- Structural NIH state is hidden and non-automatable, serialized into the
  constructor (including the full ten-band schema), and fingerprinted. A host
  that changes construction state while active receives silence plus an
  explicit reactivation error; reconstruction/destruction never occurs on the
  render thread.
- Deterministic adapter tests cover callback partitions 1/17/32 and the
  negotiated maximum, exact `+2Q` causality, Q−1/Q/Q+1 and tied automation,
  reset epochs, input/output/event saturation, recovery without shifted replay,
  million-frame overload recovery without proportional DSP catch-up, full
  overwrite, inner error/panic latching without post-fault reentry, future-epoch
  event preservation, zero callback allocations and deallocations, and
  control-thread destruction. FFI passes 41 tests, NIH passes its focused
  Linear Phase EQ tests, and the native arm64 AU smoke suite covers same-format
  maximum-frame recreation, latency KVO, event/ramp handling, concurrency, and
  realtime allocation evidence.

- Every FIR-response parameter is structural. Runtime changes return an error,
  so FIR design/planning/allocation, old-tail/new-filter splices, and latency
  changes cannot occur in a warmed callback. Sample-rate lifecycle rebuilds
  immediately in `initialize`.
- Streaming uses 32-sample-head non-uniform partitioned convolution. QA covers
  1/2/8/12 channels, all tap lengths, and 16/32/64/127/256/512/1024-frame
  callbacks with zero allocations and deadline checks.
- The 128-input-frame worst-case work horizon is now an object-safe `Plugin`
  contract, forwarded by the parametric and oversampling adapters, aggregated
  by `DawHost` in its input clock, and consumed by the production engine's
  playback queue. The queue maintains enough upstream work to cover that
  horizon; it does not manufacture a longer physical callback deadline.
  Smaller and irregular partitions remain valid; the 32-frame convolution
  priming delay remains included in reported latency.
- FFI and DSP share `type/freq/q/gain/active` keys and the ten-band limit;
  factory and bridge construction use the same fallible validated constructor.
- Active-band design/schema, malformed state, bounds/Nyquist checks, active-span
  buffer validation, reset, and nested test discovery have focused regressions.
- Dry and wet paths share exact latency. Even tap counts report the designer's
  `N/2` center plus 32 partition samples; minimum phase reports 32 samples.
- Mix smoothing advances per sample and is block-partition invariant.
- Removing recursive callback chunking eliminated its transport/event metadata
  corruption. Stable Auto Gain FIRs retain linear compile metadata.
- README, AGENTS, changelog, QA, crate version, and lockfile describe the queued
  engine contract; the direct-format adapter above separately satisfies the
  physical callback contract without changing engine scheduling.

## Findings

### P1 — Confirmed: parameter changes design and allocate an FIR on the audio callback

Every band change, phase-mode change, Auto Gain change, sample-rate change, and band-count change sets `fir_dirty` (`crates/sotf-plugins/crates/sotf-plugin-linear-phase-eq/src/lib/linear_phase_eq_plugin.rs:560-665,675-685`). The next `process_in_place` synchronously calls `rebuild_fir` before processing audio (`linear_phase_eq_plugin.rs:730-734`). That rebuild evaluates up to 16,384 response points, calls the FIR designer, converts its result, and transforms the new FIR (`linear_phase_eq_plugin.rs:245-333`). The dependency's convenience function constructs a fresh `FirDesignContext` on every call (`/Volumes/home_ext1/src_pierre/all_of_sotf/math-audio/crates/math-iir-fir/src/fir_design/generate.rs:27-32`); the context grows several work vectors, while generation also creates interpolation, magnitude, spectrum, impulse, final-IR, and window vectors (`fir_design/context.rs:61-80,121-157`; `fir_design/fir_phase.rs:47-71`). These are both heap allocations/deallocations and large O(N log N) work in the realtime callback. Changing FIR length is worse if the setter is invoked on a realtime/control callback: it replans both FFTs and resizes every processing, spectrum, scratch, and overlap buffer synchronously (`linear_phase_eq_plugin.rs:583-592,444-469`). The changelog's assertion that rebuild allocation is “not in the audio thread” is contradicted by the process path (`CHANGELOG.md:70-71`).

Build a complete immutable FIR/convolution backend off-thread, tag it with a monotonically increasing parameter generation, atomically install only the latest completed generation, and retire the old allocation on a non-realtime thread. Align and crossfade old/new engines as described below. The realtime test must count allocations **and deallocations** during repeated band, phase, Auto Gain, length, and sample-rate changes; it must also record callback p99/max duration, not only steady-state allocation count.

### P1 — Confirmed: every callback performs full-length FFT convolution, making CPU cost explode at small host blocks

The processing FFT is always `2 * fir_length`, hence 2,048–16,384 points (`linear_phase_eq_plugin.rs:173-180`; `src/lib/misc.rs:5-12`). For every callback and every channel, the plugin zeroes the full input, performs one full forward FFT, a full-bin complex multiply, one full inverse FFT, and normalizes the complete FFT buffer (`linear_phase_eq_plugin.rs:743-779`). Thus a 32-frame callback with an 8,192-tap FIR runs two 16,384-point FFTs per channel to produce only 32 new samples; at 48 kHz that is 1,500 transform pairs per second per channel. The implementation does not accumulate a partition before transforming and does not use uniform/non-uniform partitioned convolution. Serial channel processing also repeats the transforms without batching (`linear_phase_eq_plugin.rs:743-811`). This is not bounded by the QA result: QA uses mono, 2,048 taps, and 1,024-frame callbacks (`bin/qa_linear_phase_eq.rs:13-18,35-49`), then reports average throughput rather than callback jitter.

Replace the monolithic callback-sized overlap-add with partitioned convolution sized for the host's realtime block contract, or accumulate fixed partitions in a preallocated input queue and emit from an output ring. Share immutable FIR partitions across channels and investigate batched/SIMD complex multiply. Benchmark 1/2/8/12 channels, all FIR lengths, and blocks 16/32/64/127/256/512/1,024 under realistic system load; gate on deadline misses and p99/max callback time as well as average CPU.

### P1 — Confirmed: the Audio Unit bridge publishes parameter IDs and bands that the DSP rejects

The DSP's live per-band IDs are `band_N_type`, `band_N_freq`, `band_N_q`, `band_N_gain`, and `band_N_active` (`linear_phase_eq_plugin.rs:397-437,620-667`). The shared parameter template instead uses `filter_type`, `frequency`, `q`, `gain_db`, and `active` (`src/params.rs:50-74`). The FFI bridge mechanically expands those keys to IDs such as `band_0_filter_type` and `band_0_frequency` and explicitly states that they match the DSP convention (`crates/sotf-plugins/crates/plugins-ffi/src/parameter_map.rs:312-327`), but three of five do not match. Changing filter type, frequency, or gain through the AU bridge therefore reaches the unknown-parameter error path rather than changing audio. The bridge also expands 20 bands for Linear Phase EQ (`parameter_map.rs:293-308`), while the schema and implementation cap it at 10 (`params.rs:29-32`; `linear_phase_eq_plugin.rs:102,562`). Bands 10–19 are guaranteed to be nonexistent.

Make the canonical `BAND_TEMPLATE` engine keys exactly match the DSP IDs, or make the plugin consume the canonical long names everywhere and provide explicit legacy aliases. Set the expansion limit from the same single source as `num_filters.max`, not a duplicated literal. Add an end-to-end AU/FFI test that enumerates every published ID, validates and sets boundary/mid values, reads the value back, and verifies the expected transfer-function change; assert that no published band index exceeds the DSP maximum.

### P1 — Confirmed: reducing `num_filters` does not remove the disabled bands from the FIR response

On a count reduction, the plugin deliberately retains the old `bands` vector and changes only `num_filters` (`linear_phase_eq_plugin.rs:560-578`). FIR design then evaluates `band_contribution_db(&self.bands, ...)` over the **entire retained vector**, not `&self.bands[..self.num_filters]` (`linear_phase_eq_plugin.rs:226-242,260-275`). A boost, cut, low-pass, or high-pass in a removed band therefore remains audible after the UI says the band count has decreased. `rebuild_cached_parameters` and `current_values` also iterate the entire retained vector (`linear_phase_eq_plugin.rs:397-438,530-551`), so hidden bands remain host-visible in the native schema.

Use the active prefix consistently for design and host exposure, while retaining capacity privately if avoiding deallocation is important. Define how restored bands are initialized when the count grows again. Add a regression that installs a clearly measurable filter in the highest band, shrinks below it, and verifies both the response and parameter schema lose that band; then grow again and verify the documented restoration/default behavior.

### P1 — Confirmed: ordinary FIR changes splice a new filter onto the old filter's overlap tail without a transition

`rebuild_fir` replaces coefficients and spectrum in place but does not clear, version, or crossfade the per-channel overlap (`linear_phase_eq_plugin.rs:245-352`). The next callback convolves current input with the new spectrum, then adds the pending `fir_len - 1` samples produced by the old filter (`linear_phase_eq_plugin.rs:781-803`). Consequently a parameter or phase change creates an output that corresponds to neither filter: old-filter tail and new-filter response are summed, and the transfer switches discontinuously. FIR-length changes happen to clear overlap during resizing (`linear_phase_eq_plugin.rs:465-469`), but they still cause a hard discontinuity.

Run old and new complete convolution states concurrently, latency-align them, and crossfade over a bounded interval; never combine a tail from one coefficient generation with another. If transitions are restricted to a stopped/silent setup boundary, enforce that contract rather than silently rebuilding during process. Test A→B with impulses before and on every callback boundary, long low-frequency tails, linear↔minimum transitions, and randomized automation; compare against a generation-aware offline oracle and set a bound on sample-to-sample discontinuity.

### P1 — Confirmed: dry/wet mixing combines undelayed dry audio with delayed linear-phase audio

The plugin copies the current input callback as dry and mixes it directly with the convolved output (`linear_phase_eq_plugin.rs:736-741,805-810`). Linear mode reports hundreds or thousands of samples of latency (`linear_phase_eq_plugin.rs:817-823`), so any `0 < mix < 1` combines the current dry waveform with a delayed wet waveform. This produces frequency-dependent comb filtering and phase cancellation rather than a coherent dry/wet interpolation. At `mix=0`, the active test explicitly requires immediate passthrough (`tests/integration.rs:102-135`), even while the host still sees the FIR latency. Parallel render paths are therefore also misaligned when mix is zero or partially dry.

Delay the dry branch by the actual fixed processing latency in linear mode and preserve that delay at mix=0/bypass, or make latency/mix changes trigger a host render-plan rebuild under an explicit zero-latency bypass contract. Minimum-phase FIR has frequency-dependent group delay, so document that intermediate dry/wet values inherently alter phase unless a different parallel-EQ design is intended. Test impulse alignment and transfer magnitude for mix 0, 0.25, 0.5, 0.75, and 1, including host-parallel branch summation.

### P2 — Confirmed: even tap counts do not have an integer `(N-1)/2` latency, and the designer's centering does not establish the claimed symmetry

Every offered FIR length is even (`src/params.rs:22`). A truly symmetric even-length FIR has half-integer group delay `(N-1)/2`, but `latency_samples` truncates the integer expression to 511, 1,023, 2,047, or 4,095 samples (`linear_phase_eq_plugin.rs:817-823`). The dependency's linear-phase finalizer instead rotates the zero-phase impulse around integer `n_taps / 2` and then applies an even-length symmetric window centered between samples (`math-iir-fir/src/fir_design/fir_phase.rs:37-71`); this construction needs an explicit symmetry/phase audit rather than assuming the reported truncation. Current latency tests merely repeat the implementation formula (`src/lib/tests.rs:158-163`; `tests/integration.rs:177-192`) and do not measure impulse phase or branch alignment.

Prefer odd tap counts so the symmetric FIR has exact integer group delay, or deliberately design and compensate a half-sample delay and expose a host contract capable of representing it. Verify coefficient symmetry, unwrapped phase-slope/group delay across the passband, impulse peak/centroid, and cancellation against an equivalently delayed reference for every length and phase mode.

### P2 — Confirmed: mix smoothing is callback-quantized and changes with host block size

The smoother is advanced by the whole callback using `next_n(nf)`, and the resulting end-of-block value is applied to every sample in that callback (`linear_phase_eq_plugin.rs:741,805-810`). `Smoother::next_n` explicitly computes the state after N sample steps (`math-audio/crates/math-dsp/src/smoothing.rs:49-59`). A mix change therefore jumps immediately to the callback-end value, stays constant for the block, and follows a different audible trajectory when the host changes callback size. Recursive large-block chunking changes the trajectory again because each chunk advances independently.

Generate the smoothing envelope per sample into a preallocated control buffer (or advance within the writeback loop once per frame and reuse that value across channels). Compare identical automation rendered with blocks 1, 16, 63, 64, 127, 256, 1,024, and oversized/chunked blocks; outputs should agree within numerical tolerance after latency alignment.

### P2 — Confirmed: factory construction bypasses parameter bounds and can panic the FIR designer

The serialized construction type accepts unrestricted `mix`, band frequency, Q, and gain (`src/lib/types.rs:11-38`). `from_params` clamps only counts and choice indices, then passes band values directly into `EqBand` and the FIR design (`linear_phase_eq_plugin.rs:94-142`). This bypasses the live schema's mix 0–1, frequency 20–20,000 Hz, Q 0.1–10, and gain ±24 dB limits (`src/params.rs:29-43,60-73`). Nonfinite or invalid values can propagate into `Biquad::log_result`; the FIR dependency asserts that every resulting magnitude is finite and panics rather than returning an error (`math-iir-fir/src/fir_design/context.rs:95-119`). Frequencies are also not clamped to a sample-rate-dependent Nyquist limit, so the nominal 20 kHz schema is invalid at common lower sample rates.

Validate one canonical, versioned parameter representation before constructing any biquad or allocating an FIR. Reject nonfinite values and nonpositive sample rates, constrain frequency below Nyquist with a documented guard band, and return `PluginResult` rather than allowing a dependency assertion to cross the host boundary. Add factory JSON tests for NaN/Inf where representable, zero/negative Q, extreme gain/mix, malformed filter names, 22.05/32/44.1/48/96/192 kHz, and exact/above-Nyquist bands.

### P2 — Confirmed: malformed process buffers panic, and the plugin mutates samples outside the active audio span

`total_samples = nf * nc` is unchecked, and both the dry copy and channel indexing assume the supplied slice contains that many samples (`linear_phase_eq_plugin.rs:736-750`). A short host buffer panics instead of returning `PluginResult::Err`; multiplication can also overflow for adversarial contexts. Conversely, if a host supplies a larger reusable buffer, only the first active span is processed but `flush_denormals_inplace(buffer)` mutates the entire slice (`linear_phase_eq_plugin.rs:805-814`). This violates the usual valid-frame boundary and can zero subnormal sentinel/tail data outside the current callback.

Use checked multiplication and the host's canonical interleaved-buffer validator before any indexing, then operate and flush only `&mut buffer[..total_samples]`. Test zero frames/channels with both empty and oversized buffers, one-sample-short buffers, oversized buffers with sentinel normal and subnormal tails, and large-count overflow without panics.

### P2 — Confirmed test defect: the 438-line DSP regression module is not compiled

The crate root loads `src/lib/tests.rs` as a path-attributed module, and that file declares plain `mod misc;` (`src/lib.rs:13-15`; `src/lib/tests.rs:14`). Rust resolves it to the production sibling `src/lib/misc.rs`, not `src/lib/tests/misc.rs`. `cargo test -- --list` therefore contains none of the latter file's six tests, including passthrough, boost, impulse symmetry/phase linearity, shelf behavior, and low-pass attenuation (`src/lib/tests/misc.rs:14-438`). Some of those tests are cited in the changelog as regressions (`CHANGELOG.md:49-52`), but they provide no CI protection.

Give the nested test module an explicit path or move these tests to an unambiguous integration target. Add a CI assertion for the critical test names so module-resolution changes cannot silently delete coverage. Once active, tighten them with offline frequency-response and convolution oracles; the current `max_asymmetry < 0.01` threshold is too loose to prove linear phase for mastering use.

### P2 — Confirmed: reset leaves the mix smoother in its pre-reset trajectory

`reset` clears only convolution overlap (`linear_phase_eq_plugin.rs:690-695`). If transport stops during a mix ramp, the next render resumes from the old smoother's current value rather than starting from the canonical `mix_value`. The integration reset test checks only that output is finite (`tests/integration.rs:222-248`), so it cannot detect nondeterministic restart output.

Reset or snap the smoother to the documented target/current value, and make the same decision explicit for any future FIR transition state. Test set mix→process part of the ramp→reset→replay against a freshly constructed instance with the same parameters, across multiple block sizes.

### P3 — Recommendation: preserve full event/transport context when internally chunking

For oversized blocks, the plugin first adjusts `sample_position`, then calls `with_transport(context.transport)`, which overwrites it with the original transport (`linear_phase_eq_plugin.rs:710-724`). It also forwards the complete MIDI slice to every chunk without offset filtering and drops note-expression events. Linear Phase EQ does not currently consume those events, so this has no present audio effect, but it makes the generic processing contract wrong and will break sample-accurate automation if the plugin begins using event context.

Use the host's canonical sub-context builder or clone transport and modify the chunk position afterward; slice/rebase events to the chunk and preserve all context fields. Add a recording test plugin/helper that validates context equivalence between one oversized call and explicit external chunking.

### P3 — Recommendation: Auto Gain should not force conservative nonlinear compile metadata

With Auto Gain enabled, `compile_metadata` returns a generic boundary whose contract marks the plugin nonlinear and not block-invariant (`linear_phase_eq_plugin.rs:483-498`; `crates/sotf-plugins/crates/sotf-host/src/plugin/types.rs:87-102`). Auto Gain merely multiplies the static FIR coefficients by a scalar during design (`linear_phase_eq_plugin.rs:299-330`); once the FIR generation is installed, convolution remains linear. This needlessly blocks legal scheduling/fusion optimizations, while the genuinely unsafe transition/rebuild state is not represented separately.

Return metadata based on actual runtime state: stable Auto Gain FIRs may remain linear/stateful, while dirty, rebuilding, or crossfading states should be conservative boundaries. Add compiled-plan versus ordinary-render comparisons for Auto Gain on/off and during parameter transitions.

### P3 — Recommendation: update documentation and QA claims to match the implementation

The README says the source consists of `lib.rs` and `params.rs`, omitting the actual split implementation, tests, and QA (`README.md:27-33`). It describes frequency-domain convolution as efficient without qualifying the per-callback full-FFT behavior (`README.md:9-15`). The catalog advertises “zero allocation evidence” for varied blocks and FIR rebuilds (`crates/sotf-plugins/src/factory/catalog.rs:733-750`), while QA measures allocations only after construction and never changes a parameter (`bin/qa_linear_phase_eq.rs:30-59`); the rebuild path is allocating as described above. The changelog likewise claims rebuilds do not run on the audio thread (`CHANGELOG.md:70-71`).

Document supported block/channel/sample-rate limits, latency and dry-path policy, phase-mode semantics, transition behavior, and expected CPU scaling. Make QA print its channel/tap/block matrix and test steady state separately from every state transition. Catalog evidence should name exactly what was measured and must not imply parameter-change realtime safety until it is covered.

## DSP and streaming contracts observed

- Input/output is interleaved and channel-preserving. Each channel is convolved independently with one shared FIR spectrum; there is no cross-channel matrix.
- FIR lengths are 1,024/2,048/4,096/8,192 taps. The processing FFT is always twice the tap count. Blocks larger than `fft_size - (fir_len - 1)` are recursively chunked.
- FIR magnitude is derived by summing active biquad log responses on a logarithmic 1 Hz-to-Nyquist grid. Linear or minimum phase is then synthesized by `math-audio-iir-fir` and Kaiser-windowed.
- Streaming convolution is monolithic FFT overlap-add with `fir_len - 1` pending samples per channel. FFT plans and steady-state work buffers are preallocated and reused.
- Band/phase/Auto Gain changes are deferred with `fir_dirty`, but actual FIR generation and installation happen at the start of the next audio callback.
- Mix uses a 20 ms one-pole smoother, sampled once at the end of each callback. Dry audio is not latency compensated.
- Linear mode reports integer `(fir_len - 1) / 2`; minimum mode reports zero. No fractional or frequency-dependent latency representation is provided.
- Reset clears overlap only; parameters, FIR coefficients, dirty state, and smoother history remain.

## Scope reviewed

Read completely: repository and nested `AGENTS.md`; plugin `Cargo.toml`, `README.md`, `CHANGELOG.md`; every source module; all inline, nested, integration, and property tests; QA binary; facade exports and legacy `fir_designer` alias; factory create/catalog entries; FFI per-band parameter expansion; Audio Unit class/view-controller wiring; host parameter/compile-metadata contracts; shared smoothing and denormal helpers; and the directly used `math-audio-iir-fir` response-design configuration, context, allocation, phase-finalization, and window path. TokenSave was used first for semantic scope and wiring; its separate `math-audio` graph was schema-incompatible, so that dependency was then read directly. No plugin code was skipped and no production code was changed.

Verification run:

- `cargo test -p sotf-plugin-linear-phase-eq` — 63 tests passed across active unit, integration, property, and doc-test suites.
- `cargo test -p sotf-plugin-linear-phase-eq -- --list` — confirmed all six tests in `src/lib/tests/misc.rs` are absent.
- `cargo run -p sotf-plugin-linear-phase-eq --features qa --bin qa-linear-phase-eq` — passed; reported the expected 1 kHz boost, 1,023-sample latency, zero steady-state allocations, and 0.31% average mono CPU for the single 2,048-tap/1,024-frame scenario. It did not exercise parameter rebuilds or the small-block/multichannel worst case.

No full-workspace build was run.

## Final verification

The final command results below supersede the pre-remediation baseline above:

- `cargo test -p sotf-plugin-linear-phase-eq` — all active unit, nested DSP,
  integration, property and doc-test suites passed.
- `cargo clippy -p sotf-plugin-linear-phase-eq --all-targets --no-deps -- -D warnings` — passed.
- `cargo run -p sotf-plugin-linear-phase-eq --features qa --bin qa-linear-phase-eq` —
  response, latency, zero-allocation, throughput, and full layout/tap/block
  deadline matrix passed.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_linear_phase_eq_zero_alloc --no-default-features --offline` — passed when the shared facade baseline compiled.
