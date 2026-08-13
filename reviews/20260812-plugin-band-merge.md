# Band Merge plugin code review — 2026-08-12

## Remediation status

Final exhaustive follow-up in version 0.5.6:

- Engine-generated Band Merge configs now keep `channels` as graph topology instead
  of emitting it into the strict runtime parameter object; a public-factory default
  regression prevents recurrence.

- Processing now requires initialization and the initialized sample rate, in
  addition to the existing exact checked buffer contract.
- Canonical and facade preset structs reject unknown fields; public factory tests
  cover invalid channel/band layouts, gain ranges, unknown state, and a valid
  8-input/4-band construction.
- Successful gain/mute writes update host-visible cached values in place, and
  plugin version/compile metadata/lifecycle contracts have focused regressions.

Additional remediation in version 0.5.5:

- **Fixed:** The armed, high-error diagnostic callback path now has a serialized regression test
  that combines allocation counting with atomic logger interception. It performs no allocation or
  realtime logging.
- **Fixed:** `reconstruction_error_db` now measures normalized RMS of the actual
  `output - unity-band-sum` error instead of an output/reference RMS-level ratio. Exact
  reconstruction is reported at the -60 dB floor, while a nonzero output against a cancelled
  reference reaches the finite +60 dB ceiling. Equal-RMS but waveform-wrong output is covered.
- **Fixed:** The scalar sum dispatches all public 2–8-band layouts to explicit unrolled reductions,
  with deterministic scalar-reference/dispatch tests and Criterion cases for the reviewed 2x2,
  2x4, 6x4, and 8x8 channel-by-band layouts.

Additional remediation in version 0.5.4:

- **Fixed:** Reset now snaps muted bands to zero and unmuted bands to their configured linear gain.
  Regression coverage verifies that muting a band, resetting, and processing a settled DC block
  leaves that band silent.

Remediated in version 0.5.3:

- Band gain smoothing now emits the recurrence per frame and is sample-identical across single, fixed, and irregular callback partitions.
- Mute state retargets the same gain smoother to zero and unmute retargets it to the configured linear gain.
- Factory/runtime gain validation rejects non-finite and values outside `[-60, +24]` dB. Construction and processing validate nonzero channels/rate, checked dimensions, and exact buffers before advancing smoothers.
- `bands` follows the canonical 2–8 schema and live changes are rejected as structural/rebuild-required rather than mutating the channel contract.
- The armed diagnostic no longer formats or logs from the realtime callback.

The three previously deferred review items are now remediated without renaming the serialized
parameter ID. A broader paired BandSplit→BandMerge signal corpus and explicit SIMD/layout redesign
remain optional coverage and optimization work, not correctness blockers for this scalar plugin.

## Findings

### P1 — Gain smoothing applies only the end-of-block gain, making automation callback-size dependent

For each band, `process` calls `next_n(num_frames)` once and applies that returned end state as a constant gain to every sample in the block (`crates/sotf-plugins/crates/sotf-plugin-band-merge/src/lib/band_merge_plugin.rs:249-279`). Advancing state by N samples is not equivalent to emitting the N intermediate gains. A gain step at the block boundary therefore jumps immediately near its target for a large block, while the same stream in small callbacks follows a ramp.

This defeats the stated zipper-noise fix and breaks render partition invariance. Apply the exact one-pole recurrence per frame, generate a closed-form exponential gain ramp, or use a host kernel that accepts start/coefficient. Add a reference test processing identical automation with 1/32/128/512/irregular frame partitions and assert sample-identical (or tightly bounded) output and the intended 10 ms time constant.

### P1 — Mute toggles are completely unsmoothed

Mute changes update `band_mutes` immediately (`band_merge_plugin.rs:178-187`), and effective gain becomes exactly zero for the whole next block (`band_merge_plugin.rs:252-259`). The gain smoother is neither retargeted nor used for the mute transition, so toggling a band at a non-zero waveform sample produces a discontinuity/click. Unmuting likewise jumps to the current band-gain smoother value.

Fold mute into the smoothed target (zero when muted, configured linear gain otherwise) and preserve the configured gain separately. Test mute/unmute at DC and peak-phase sine positions, across block boundaries and all partition sizes.

### P1 — Gain setters accept values outside the schema and silently accept NaN/infinity

The dynamic schema advertises `[-60, +24] dB`, but `set_parameter` performs no range check; finite values of any magnitude are converted to linear gain (`band_merge_plugin.rs:159-175`). Non-finite values do nothing yet return `Ok(())`. `from_params` also accepts every preset gain without range/finiteness checks and immediately resets smoothers to it (`band_merge_plugin.rs:67-82`). Extreme/NaN gains can overflow or propagate non-finite output, while the host believes the update succeeded.

Centralize validation for factory and runtime paths, reject rather than silently ignore invalid values, and test endpoints plus out-of-range, NaN, and infinity through both construction and `set_parameter`.

### P1 — An optional diagnostic logs from the realtime process callback

Reading `reconstruction_error_db` arms a flag (`band_merge_plugin.rs:192-199`). The following audio callback computes a metric and calls `log::warn!` when it exceeds 3 dB (`band_merge_plugin.rs:282-298`). Logging can format, allocate, take locks, and invoke arbitrary logger sinks on the realtime thread, directly violating the repository callback rules.

Store an atomic/cached diagnostic event and let the control/telemetry thread format/log it. Add allocation/lock-sensitive QA for both normal processing and the armed/high-error diagnostic path; current QA does not arm this branch.

### P1 — Runtime band-count mutation changes the plugin channel contract in place

The `bands` parameter is structural, but the plugin setter directly changes `num_bands` and therefore `input_channels()` (`band_merge_plugin.rs:149-157`, `128-132`) without rebuilding the graph or validating the currently supplied buffer layout. Tests explicitly exercise this in-place mutation. If the host calls the setter before graph reconstruction, the next callback indexes a different channel stride and can panic or reinterpret channels.

Reject structural changes in the live instance (return a rebuild-required result) or make the host replace the plugin atomically before audio resumes. Add an engine integration test demonstrating graph recompilation and buffer resizing; do not treat a changed accessor alone as success.

### P2 — Buffer sizing and channel-count multiplication can panic/overflow

`input_channels` and `process` multiply output channels, bands, and frames without checked arithmetic; the inner loops index both slices directly (`band_merge_plugin.rs:128-130`, `231-280`). Short buffers panic, and oversized-buffer tail behavior is unspecified. `new` also permits zero output channels.

Validate nonzero output channels, checked sample counts, and input/output bounds before changing smoother state. Return errors and test zero channels, short/oversized buffers, zero frames, and multiplication overflow.

### P2 — “Reconstruction error” measures only output/reference RMS ratio

The diagnostic compares RMS of the processed sum with RMS of the unity-gain sum (`band_merge_plugin.rs:242-301`). Equal-RMS but phase/waveform-wrong output reports 0 dB; a reference that cancels near zero is declared no error; ordinary intentional gain/mute is labeled reconstruction error. The changelog acknowledges the naming problem, but the UI/API still presents it as deviation from perfect reconstruction.

Either rename it to reconstruction level difference, or compute an actual error signal such as RMS(`output-reference`) relative to reference with correlation/NRMSE. Reconstruction fidelity of a BandSplit→BandMerge pair should be measured by an integration test using impulses, noise, tones, and block partitions—not arbitrary independent band inputs.

### P2 — Active schema and implementation disagree on maximum band count

The central `PARAMS`/UI declares 2–8 bands (`src/params.rs:15-18`), while the plugin constructor, dynamic descriptor, and setter allow up to `MAX_BANDS = 32` (`src/lib/misc.rs:1-2`; `band_merge_plugin.rs:39-50`, `85-120`, `149-157`). Factory validation, UI, presets, and direct construction can therefore expose different supported ranges.

Choose one limit and derive constructor/descriptors from the canonical spec (with a separate internal capacity if needed). Add a facade/catalog/factory parity test.

### P3 — The scalar loop has an unfavorable layout for band summation

Input is frame-major and band-major within each frame, so each output sample walks bands with a stride of `out_ch` (`band_merge_plugin.rs:261-278`). The local fixed gain array is good and processing allocates nothing, but LLVM cannot generally vectorize a short strided reduction effectively. For typical 2–8 bands, channel/band-specialized unrolling or a planar upstream band layout may improve throughput; for many channels/bands, explicit SIMD reduction may help.

Benchmark realistic 2×2, 2×4, 6×4, and 8×8 layouts before redesign. Correct smoothing will add per-frame work, so benchmark it together with the fix.

## Algorithm and realtime assessment

The basic mapping matches the documented band-major layout: output channel `ch` sums input `band * output_channels + ch`. It is linear, zero-latency, and a graph/channel-mixing boundary. Per-band arrays and smoothers have fixed capacity, the normal hot path allocates no memory and takes no locks, and denormal handling is present. The diagnostic logging exception above must be removed.

No explicit bypass exists; unity, unmuted gains reproduce the direct band sum. Reset snaps smoothers to configured targets, which is reasonable for a transport reset if documented, but should be tested after partial automation. Initialization retunes all fixed-capacity smoother coefficients to the host sample rate.

## Scope reviewed

Read every plugin-owned file: all five Markdown documents, `Cargo.toml`, every source module, all unit/integration/property tests, and `bin/qa_band_merge.rs`. Also checked factory/catalog/facade registration, host smoother/compile-metadata contracts, the documented BandSplit pairing, and TokenSave test-risk/caller context. No production code was changed.

## Existing strengths

- Correct band-major channel mapping is clear and covered for mono and multichannel cases.
- Hot-path storage is fixed/preallocated, with no normal callback allocation or locking.
- Parameter discovery exposes per-band gain/mute controls and tests cover basic round trips and output sums.
- Property tests exercise finite input, NaN behavior, unity summation, monotonic mute behavior, and gain round trips.
- Structural metadata marks the plugin as a channel-mixing boundary and latency is correctly zero for summation alone.

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-band-merge
cargo test -p sotf-plugin-band-split
cargo test -p sotf-plugins --test all_plugins_dsp_matrix
cargo clippy -p sotf-plugin-band-merge -- -W warnings
cargo check -p sotf-plugins
```
