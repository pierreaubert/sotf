# Transient Shaper plugin code review — 2026-08-12

## Remediation status

Implemented through `0.5.9`:

- **Fixed:** Attack uses a positive-only differential envelope, preventing attack polarity from
  inversely shaping decays.
- **Fixed:** gain monitoring selects the largest absolute deviation from unity and therefore
  reports attenuation-only shaping.
- **Fixed:** fallible construction, the plugin bridge factory, and the shared facade factory
  reject zero channels and non-finite/out-of-range values.
- **Fixed:** zero sample rate, sample-count overflow, and short buffers return errors before DSP
  state changes; denormal flushing is active-region-only.
- **Verified:** reset already clears both envelope banks, all three smoothers, and meter cadence.
- **Fixed:** sensitivity thresholds and output gains are sample-smoothed as linear
  coefficients, avoiding per-sample transcendental work, and are partition-invariant.
- **Fixed:** documentation now describes the actual shape detector plus sensitivity gate.
- **Fixed:** detector gain is linked across channels to preserve spatial ratios.
- **Fixed:** monitoring uses accumulated extrema on a sample-derived 30 Hz cadence.
- **Fixed:** shaping gain is bounded and a linked soft ceiling protects plugin-generated
  boosts while neutral overrange passthrough remains unchanged.
- **Fixed:** parameter updates mutate cached schema values in place without rebuilding storage.

Follow-up verification: the shared-factory regression
`transient_shaper_facade_factory_validates_constructor_contract` rejects an
out-of-range attack value and zero channels.

Focused verification: `cargo test -p sotf-plugin-transient-shaper --offline`
(43 passed), including automation partitioning, asymmetric stereo linking,
attenuation metering, output bounds, schema-storage reuse, and exact buffers.

## Findings

### P1 — Attack control also shapes decays with the opposite sign

The “transient” signal is `fast_env - slow_env`; it becomes positive on attacks and negative when the fast envelope falls below the slow envelope during decay (`crates/sotf-plugins/crates/sotf-plugin-transient-shaper/src/lib/transient_shaper_plugin.rs:316-332`). The signed ratio is then multiplied by `attack_amt` (`transient_shaper_plugin.rs:325-333`). Thus boosting attack attenuates decay portions, while cutting attack boosts them. This entangles the advertised independent attack and sustain controls.

Define the attack component as positive-only for attack shaping, or use a documented bipolar differential model with separate rise/fall controls. Add isolated attack/decay envelope tests and assert that attack changes do not invert sustain-tail gain.

### P1 — The gain meter cannot report attenuation

`last_gain` starts at 1.0 and is updated with `max(gain)` (`transient_shaper_plugin.rs:281-283,342-346`). Any block whose shaping only cuts gain keeps the published value at 1.0. The changelog describes `max` as a fix across channels, but maximum linear gain is not maximum gain reduction.

Track minimum gain for attenuation, maximum absolute dB deviation, or publish separate boost/cut extrema with a clear UI contract. Add tests for attack/sustain cuts across multiple channels and blocks.

### P1 — Factory construction accepts NaN and invalid values

`from_params` relies on `clamp`, which preserves NaN, and returns `Self` rather than validation errors (`transient_shaper_plugin.rs:84-96`). Non-finite attack/sustain/mix can enter smoothers, while non-finite sensitivity/output gain feeds `powf` and poisons audio. Runtime adapter validation does not protect factory/preset JSON.

Make construction fallible and validate all fields against the authoritative parameter specs, including finiteness. Add universal-factory tests for NaN/infinity and every boundary.

### P1 — Processing trusts context buffer dimensions and can panic

The loop directly indexes `buffer[frame * channels + c]` without validating length or checked multiplication (`transient_shaper_plugin.rs:258-350`). A malformed context panics after advancing smoother/envelope state. Zero channels also return arbitrary requested frames for an empty buffer.

Reject zero channels, validate exact required length with checked arithmetic before state changes, and return an error. Add short-buffer/overflow-shaped context tests.

### P2 — Output gain and sensitivity automation is unsmoothed

Attack, sustain, and mix use sample smoothers, but `sensitivity_db` and `output_gain_db` change immediately; both are converted once per callback (`transient_shaper_plugin.rs:190-229,265-272`). Output gain jumps at block boundaries, and a threshold jump can abruptly enable/disable shaping.

Smooth output gain in the linear/dB domain and crossfade or smoothly move the sensitivity threshold. Test automation with different callback partitions and bound discontinuity energy.

### P2 — “Threshold-independent” documentation contradicts the implemented hard threshold

The README/AGENTS repeatedly market threshold-independent behavior, but sensitivity is implemented as a hard gate: shaping occurs only when `slow > 10^(sensitivity/20) * 1e-3` (`transient_shaper_plugin.rs:265-268,323-335`). This creates a discontinuity around roughly -72 to -48 dBFS over the allowed sensitivity range and makes results level-dependent.

Correct the product claim or replace the hard gate with a smooth, scale-relative confidence/noise-floor mechanism. Add level-scaling tests and threshold-boundary continuity tests.

### P2 — Independent channel envelopes can destabilize stereo image

Each channel has separate fast/slow envelopes and gain (`transient_shaper_plugin.rs:274-341`). A transient on one side changes width/pan even for material expected to remain linked. The changelog defers this, but it is a material algorithm limitation.

Expose linked/unlinked detection and use max/RMS or configurable linking for stereo/multichannel groups. Add asymmetric stereo impulse tests measuring correlation and image shift.

### P2 — Monitoring magnitude and cadence depend on callback partitioning

Peak transient/sustain values are local to one callback, but only every tenth callback is published; the preceding nine blocks are discarded (`transient_shaper_plugin.rs:278-283,360-371`). The publication interval is callback-count based, so both peak capture and UI rate vary with block size.

Accumulate extrema over a sample-rate-derived display window, publish then reset. Test identical streams under several callback partitions.

### P2 — Gain law permits large uncontrolled boosts and no output protection

The additive law `1 + attack*ratio + sustain*ratio` can reach roughly 3× before the additional +12 dB output gain, producing nearly 12× input amplitude; only the lower side is clamped to zero (`transient_shaper_plugin.rs:323-340`). This may be intentional, but no headroom/soft limiting or meter warns users, and parameter extremes can clip downstream badly.

Document maximum gain, consider a dB-domain bounded mapping, and expose output peak/clip indication. Add worst-case boundedness/headroom tests using impulses and dense material.

### P3 — Parameter setters rebuild and allocate the whole schema

Every `apply_values` call rebuilds a new parameter vector (`transient_shaper_plugin.rs:105-158,190-229`). Project history assumes setters are off the realtime thread, but automation delivery should make that contract explicit and allocation-tested.

## Realtime allocation and performance assessment

The normal sample loop is allocation-, lock-, log-, and I/O-free. Envelope/state vectors and cache are preallocated, FTZ/DAZ is enabled, explicit state flushing handles denormals, and work is O(frames × channels) with simple one-pole math. `powf` conversions are block-level, not per sample. Main realtime risks are unchecked buffers and unsmoothed block-level controls rather than CPU cost.

## Scope reviewed

Read every plugin-owned file without omission: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`, all six source modules including the nested 17 KB test file, unit/integration/property suites, and `bin/qa_transient_shaper.rs`. No `USAGE.md`, `UI.md`, or benchmark exists. Also checked facade/factory/catalog/schema wiring, parametric adapter validation, smoother/cache/metadata contracts, and allocation coverage. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-transient-shaper`: 33 tests passed across four suites.
- TokenSave inventory/test-risk preceded reads; it identified `time_to_coeff` as the highest-risk graph-uncovered helper and 17% symbol coverage.

## Suggested verification after fixes

- Run crate, realtime-allocation, and QA suites.
- Add analytic attack/decay, level-scaling, stereo-link, automation, and monitoring partition tests.
- Benchmark mono–12 channels across supported sample rates.
- Measure maximum output/headroom under parameter extremes.
