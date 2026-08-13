# Stereo Imager plugin code review — 2026-08-12

## Remediation status (0.5.6)

Fixed in this batch:

- Neutral width processing is sample-transparent and dry/wet mix scales only
  phase-matched side-band corrections.
- Construction rejects non-stereo layouts, unknown fields, non-finite/out-of-range
  values, and unordered crossovers; initialization enforces Nyquist.
- Frequency target crossings are rejected instead of silently swapping identities.
- Checked buffer length/overflow validation runs before dry, mixed, and fully wet paths.
- Reset continues to clear crossover state and snap every smoother deterministically.
- The factory catalog now advertises Stereo Imager as stereo-only, matching its
  constructor and preventing unsupported channel layouts from being presented
  as valid.
- Mono-bass enable is sample-smoothed, with an out-of-phase 80 Hz transition
  regression bounding the first-sample discontinuity.
- Crossover target smoothing remains audio-rate while coefficient redesign is
  limited to one control update per 16 samples; a regression counts redesigns.
- Dry/wet interpolation now uses the current input sample directly, so no fixed
  callback scratch or maximum block cap remains; a 70,000-frame mixed callback
  regression covers the former limit.

Verification: `cargo test -p sotf-plugin-stereo-imager --offline` (44 passed).

## Findings

### P1 — Unity-width wet processing is not transparent for general audio

The signal is split by cascaded LR4 crossovers and reconstructed as `mid_low + mid_mid + mid_high` and the equivalent side sum (`crates/sotf-plugins/crates/sotf-plugin-stereo-imager/src/lib/stereo_imager_plugin.rs:316-337`). Cascaded LR4 multiband outputs have unequal group delay and their sum is not sample-identical to the input. Therefore width=1, all band widths=1, mono-bass=false still phase-rotates/generalizes the signal. Existing “passthrough” tests use DC/constant signals with loose tolerances and do not expose this.

Use a phase-compensated complementary topology, or express width changes as corrections added to an untouched reference so unity settings null exactly. Add impulse, swept-sine, noise-null, and stereo-correlation tests with all controls at unity.

### P1 — Dry/wet mix comb-filters even when width controls are neutral

For intermediate mix, the phase-rotated wet crossover reconstruction is linearly combined with undelayed dry (`stereo_imager_plugin.rs:339-350`). This produces frequency-dependent cancellation/coloration before any intended width change.

Phase-match/delay the dry path, or make “mix” scale the side-width correction around an identity signal instead of crossfading two phase-incompatible paths. Test transfer magnitude across mix at neutral width; it should remain 0 dB within a tight bound.

### P1 — Factory construction bypasses ranges, finite checks, stereo-only, ordering, and Nyquist constraints

`new` copies every public parameter directly and constructs fixed two-channel crossovers regardless of the declared channel count (`stereo_imager_plugin.rs:49-86`). `from_params` adds no validation. NaN, negative/overrange widths/mix, reversed/duplicate crossover points, frequencies above Nyquist, and non-stereo channel counts are all accepted. Runtime adapter validation does not protect factory JSON.

Make construction fallible and validate against the same schema, require exactly two channels, strict crossover ordering, and sample-rate-derived limits during initialization. Add factory tests for malformed numbers and 22.05/32 kHz operation.

### P1 — The fully wet path has no buffer-length check and can panic

Scratch capacity is checked only after the early dry/mix decisions, and the only explicit check is whether `dry_buf` is large enough (`stereo_imager_plugin.rs:273-305`). When mix is fully wet, `need_dry` is false but the loop still indexes `buffer[frame*2+1]`; a short buffer panics. Even with dry mix, copying `buffer[..nf*2]` can panic before returning the intended scratch error. Frame multiplication is unchecked.

Validate required input length with checked arithmetic before all branches/state advances. Add short-buffer and oversized-context tests for dry, transitional, and fully wet states.

### P2 — Crossover automation performs trigonometric coefficient redesign in every sample

Each frame advances two frequency smoothers and calls both LR4 `set_frequency` methods (`stereo_imager_plugin.rs:306-317`). During a 10 ms ramp, each call updates lowpass/highpass biquads for both M/S channels unless the tiny delta threshold suppresses it. This places coefficient design in the hottest loop and can dominate multichannel DSP cost despite this being stereo-only.

Use stable coefficient interpolation or a lower control-rate update cadence, skip settled smoothers before dispatch, and benchmark worst-case dual-crossover automation. Verify response/stability against a per-sample reference.

### P2 — Crossing frequency targets silently swaps instantaneous values and changes parameter identity

When low-mid exceeds mid-high, the process loop swaps the two smoothed values before assigning crossover objects (`stereo_imager_plugin.rs:307-314`), while reported parameter values/targets retain their original IDs. Automating through a crossing makes each named control suddenly govern the opposite crossover and creates a nondifferentiable trajectory.

Reject or constrain targets to strict order at the setter, with a minimum separation. Add crossing automation tests that assert stable identity, finite output, and no discontinuity.

### P2 — `mono_bass` toggles the low-side path without smoothing

`mono_bass` becomes an immediate block-level scalar 0/1 (`stereo_imager_plugin.rs:303-305`) while every width/mix control is smoothed. Toggling it on nonzero low-frequency side content creates a discontinuity/click.

Smooth the low-side enable or route it through the low-width smoother. Test toggles on out-of-phase bass and bound transition energy.

### P2 — Non-stereo configurations silently succeed and bypass processing

If `channels != 2`, processing returns success without touching data (`stereo_imager_plugin.rs:279-281`). This hides graph/configuration mistakes despite the plugin being explicitly stereo-only and still having constructed two-channel filters and large scratch.

Reject non-stereo construction or initialization with a clear error. If bypass is intentional, expose it as unsupported/bypassed status rather than silently claiming successful DSP.

### P3 — Fixed 131,072-sample scratch is wasteful and imposes an arbitrary callback cap

Initialization always allocates `65536 * 2` floats (~512 KiB) (`stereo_imager_plugin.rs:234-256`) even at default full-wet where scratch is unused. Larger legal callbacks fail. Size scratch to the host’s declared maximum block or use a host-provided secondary buffer; retain zero allocation in processing.

## Realtime allocation and performance assessment

Normal processing performs no heap allocation, locking, logging, or I/O. Parameter schema is cached and values are updated in place, smoothers are sample-based, FTZ/DAZ is enabled, and default fully wet/full-dry fast paths avoid unnecessary copies/work. The dominant cost is two LR4 splits for both M/S components plus per-sample coefficient updates during automation. Buffer validation and exact neutral reconstruction are higher priorities than further SIMD work.

## Scope reviewed

Read every plugin-owned file without omission: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`, all six source modules, both unit/integration test suites, and `bin/qa_stereo_imager.rs`. No `USAGE.md`, `UI.md`, property tests, or benchmark exists. Also checked facade/factory/catalog/schema wiring, parametric adapter validation, LR4 crossover implementation, smoothing/compile metadata, and allocation coverage. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-stereo-imager`: 41 tests passed across three suites.
- TokenSave inventory/test-risk preceded reads; it reports `process_in_place` as the highest-risk uncovered symbol and 9% graph-derived symbol coverage.

## Suggested verification after fixes

- Run crate/realtime-allocation tests and QA.
- Add neutral-setting null/impulse/sweep tests, dry/wet transfer tests, and crossing/toggle automation tests.
- Exercise supported sample rates and callback partitions, including malformed buffers.
- Benchmark steady and automated crossover paths.
