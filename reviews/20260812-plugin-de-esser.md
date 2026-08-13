# De-Esser plugin code review — 2026-08-12

## Remediation status

Completed in `0.5.11`:

- **Fixed:** Split-Band uses the LR4 low+high sum as its phase-matched reference;
  Mix scales only high-band gain reduction. Ratio-one/inactive null tests prove
  every Mix value is sample-identical, eliminating the reported comb filter.
- **Fixed:** Q now controls only symmetric detector-band edges. Edge biquads use
  fixed Butterworth pole Q, eliminating the double bandwidth/resonance encoding.
- **Fixed:** Frequency, Q, and Mode are structural in both schemas and reject
  live mutation without changing state. Host rebuilds avoid coefficient clicks,
  stale detector/crossover state, and audio-thread allocation.
- **Fixed:** meter cadence uses elapsed samples at a sample-rate-derived 30 Hz;
  reset clears cadence and held-snapshot publication has regression coverage.
- **Fixed:** realtime setters use cached validation and direct application without
  rebuilding metadata; allocation tests cover every realtime control.
- **Fixed:** runtime version metadata, strict unknown-field preset rejection, and
  non-finite audio recovery now have focused tests.

Together with `0.5.7`–`0.5.10`, all P1–P3 findings are remediated.

Additional remediation in `0.5.10`:

- **Fixed:** Frequency and Q updates now validate the complete detector band
  against the initialized sample rate before rebuilding filters. A nominally
  in-range value that reaches the Nyquist safety margin is rejected without
  changing plugin state. A 32 kHz regression covers the frequency case.

Additional remediation in `0.5.9`:

- **Fixed:** P1 process buffer contract. Frame/channel multiplication and active-buffer validation
  now complete before DSP setup; undersized buffers and overflowing frame counts return errors
  without modifying samples or advancing plugin state. Regression tests cover both cases.

Follow-up in `0.5.8`: the canonical facade and bridge factories now route
through sample-rate-aware fallible construction, so malformed serialized
presets cannot silently clamp values or map unknown modes to Split-Band.

Implemented in `0.5.7`:

- **Fixed:** Q-defined sidechain filtering now drives detection in Split-Band as well as Wideband.
- **Partially fixed:** fallible construction, the plugin bridge factory, and initialization reject malformed modes, zero
  channels/sample rates, non-finite/out-of-range values, and detection bands reaching Nyquist.
- **Fixed:** reset clears both sidechain filter banks and the mix smoother.
- **Fixed:** checked active-buffer sizing replaces panic-prone indexing, and tail samples are not
  included in denormal processing.
- **Fixed:** monitoring cache data deep-clones its vector so publication works with held snapshots.
- **Deferred:** phase-matched Split-Band wet/dry mixing, normalized constant-Q detector design,
  realtime-safe mode/frequency automation, and sample-count-based meter cadence require broader
  signal-path changes.

## Findings

### P1 — Q has no DSP effect in the default Split-Band mode

Q only changes the HP/LP detection banks (`crates/sotf-plugins/crates/sotf-plugin-de-esser/src/lib/de_esser_plugin.rs:181-209,345-370`). Split-Band processing never reads those banks; it detects directly from the LR4 high output and uses only `frequency` as the crossover point (`de_esser_plugin.rs:515-553`). Since construction defaults to Split-Band (`de_esser_plugin.rs:119`), the advertised Q/bandwidth control is a no-op in the normal mode.

Either use the Q-defined bandpass for detection in both modes, redefine Split-Band Q as an actual high-band shaping control, or hide/disable it by mode. Add response/behavior tests showing that every exposed Q setting measurably changes detection at fixed center frequency in each supported mode.

### P1 — Split-Band wet/dry mixing can comb-filter even with zero gain reduction

The wet signal is `low + high * gain` from an LR4 crossover, then linearly mixed with the original dry input (`de_esser_plugin.rs:527-548`). With `gain = 1`, an LR4 low+high sum is flat in magnitude but has frequency-dependent phase; it is not sample-identical to dry. Intermediate `mix` therefore combines phase-rotated wet with dry and can cause coloration even when the de-esser is inactive.

Use a phase-matched dry path, formulate HF reduction as subtractive band processing (`input + high * (gain - 1)`) with a suitable complementary extraction, or define mix as gain-reduction depth rather than dry/wet. Add null and swept-sine tests at ratio 1/threshold 0 across mix values.

### P1 — Diagnostic gain-reduction vectors are permanently stuck at their initial zeros

`DeEsserData` derives `Clone` around `Arc<Vec<f32>>`, and `RealTimeCache::new` clones the initial value into its shared/spare slots. Both outer values therefore share the same inner vector. `DeEsserData::update` requires `Arc::get_mut`, which can never succeed while the sibling cache slot owns the clone (`src/lib/de_esser_data.rs:4-31`; `crates/sotf-plugins/crates/sotf-host/src/analyzer.rs:29-38`). Scalar cache swaps occur, but the meter vector remains zero forever.

Store a plain pre-sized `Vec<f32>` in each outer cache slot, or implement a deep clone. Add a test that processes clear sibilance past the ten-block publication threshold and asserts nonzero data, including while UI snapshots are held.

### P1 — Factory construction accepts NaN and sample-rate-invalid filter frequencies

`from_params` uses `f32::clamp`, which preserves NaN, and silently maps unknown mode strings to Split-Band (`de_esser_plugin.rs:140-167`). It also allows frequency up to 16 kHz and bandpass upper edges up to 20 kHz before sample rate is known (`de_esser_plugin.rs:181-204`). At 32 kHz, 16 kHz is Nyquist and the upper sidechain cutoff can exceed it. Runtime validation does not protect factory/preset JSON.

Make construction fallible, reject non-finite/unknown values, and revalidate frequency and both band edges against the initialized sample rate. Test NaN/infinity and 22.05/32/44.1 kHz initialization at maximum frequency/Q.

### P1 — `reset` does not clear the sidechain filter state

`reset` says it rebuilds filters, but calls `rebuild_detection_filters`, which invokes `BiquadBank::update_params` (`de_esser_plugin.rs:202-209,447-462`). The dependency explicitly preserves delay state in `update_params`; its separate `reset` method clears state (`math-audio/crates/math-iir-fir/src/iir/biquad_bank.rs:112-145`). Wideband detection therefore retains pre-reset history.

Call `hp_filters.reset()` and `lp_filters.reset()` after retaining coefficients. Add a state-null test comparing post-reset impulse/silence output and envelope behavior with a fresh instance.

### P1 — Processing trusts the context buffer length and can panic

Both modes slice/index `buffer` using `num_frames * channels` without validation (`de_esser_plugin.rs:464-558`). A short host buffer panics after partially advancing state, and multiplication is unchecked.

Use checked multiplication and validate the required length before DSP. Reject zero channels at construction rather than treating arbitrary frames with an empty buffer as successfully processed. Add short-buffer and overflow-shaped context tests.

### P2 — The Q control double-encodes bandwidth and resonance

`bandpass_edges` uses Q to move both cutoff frequencies, then the same Q is passed as the pole Q of both highpass and lowpass biquads (`de_esser_plugin.rs:181-204`). This is not a conventional constant-Q bandpass design: high Q simultaneously narrows edge spacing and adds resonant peaks at both edges, making detector calibration/threshold response unintuitive.

Specify bandwidth in octaves or Q once, then design a normalized bandpass response with controlled passband gain. Validate center gain, -3 dB bandwidth, peak ripple, and threshold equivalence across Q/sample-rate settings.

### P2 — Frequency/Q and mode automation is discontinuous and mode state goes stale

Frequency/Q updates replace coefficients immediately; mode switches merely change an index (`de_esser_plugin.rs:345-419`). Updates now reject detector bands that would reach Nyquist at the active sample rate, but valid transitions still replace coefficients immediately. While Wideband runs, split crossovers stop advancing; while Split-Band runs, detector bank history stops advancing. Switching modes exposes stale states, and live coefficient jumps can false-trigger the dynamics detector.

Smooth or crossfade filter changes and reset/warm the destination mode deliberately. Add automation tests during tones/noise for clicks, false gain-reduction bursts, and callback-partition equivalence.

### P3 — Meter cadence is callback-count based

Cache publication occurs every ten callbacks (`de_esser_plugin.rs:557-566`), giving widely different UI rates at different block sizes. Use a processed-sample accumulator and a sample-rate-derived display cadence.

## Realtime allocation and performance assessment

Steady-state processing uses preallocated BiquadBank/crossover/core/scratch arrays, has no explicit allocation, locks, logging, or I/O, enables FTZ/DAZ, returns requested frames, and applies a SIMD per-channel gain in Wideband mode. Cost is O(frames × channels); Split-Band runs eight biquad sections per channel/sample through LR4 low/high paths plus dynamics math. `fast_log10` and `fast_pow10` remain significant but are appropriate optimization targets only after the algorithm/contract defects above.

## Scope reviewed

Read every plugin-owned file without omission: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`, all six source modules, both unit/integration test suites, and `bin/qa_de_esser.rs`. No `USAGE.md`, `UI.md`, property suite, or benchmark exists in this crate. Also checked facade/factory/catalog/schema wiring, parametric adapter validation, DynamicsCore/cache/SIMD contracts, LR4 and BiquadBank implementations, and allocation coverage. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-de-esser`: 41 tests passed across three suites.
- TokenSave file/test-risk context preceded reads; it reports `process_in_place` as the highest-risk uncovered symbol and 9% graph-derived symbol coverage.

## Suggested verification after fixes

- Run crate tests, realtime allocation coverage, and QA for both modes.
- Add independent frequency-response, detector bandwidth, null, reset, parameter-transition, and diagnostic-data tests.
- Exercise 22.05–192 kHz, mono–12 channels, and multiple block partitions.
- Benchmark both modes with meter readers active and automation in flight.
