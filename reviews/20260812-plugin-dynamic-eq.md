# Dynamic EQ plugin code review — 2026-08-12

## Final remediation status — 2026-08-12

All P1–P3 findings are fixed in `0.5.11` (no P0 was reported):

- Factory and bridge construction share the sample-rate-aware fallible validator
  for counts, types/ranges, non-finite values and Nyquist-safe detector edges.
- Dynamic depth uses amplitude-domain signed-dB blend coefficients; direct and
  bulk updates validate atomically before mutation.
- Band count/linking and frequency/Q/gain/active/solo are structural. This
  removes destructive live IIR replacement, stale linked envelopes and
  ambiguous active/solo state transitions. Dynamics and mix remain realtime.
- Reset snaps smoothers, resets cache cadence and publishes cleared monitoring
  immediately (`reset_publishes_zero_monitoring_immediately`).
- Zero-target-gain bands and settled dry mix use exact transparent fast paths;
  wet re-entry resets state deterministically
  (`zero_gain_and_settled_dry_fast_paths_preserve_dsp_state`).
- QA covers 1/2/8/16/32 channels, 1/4/8 bands, zero/nonzero gain,
  linked/unlinked operation and 32–2048-frame blocks with zero allocation and
  callback deadline gates.
- Metadata, documentation, version and lockfile match the final contract.

Final verification:

- `cargo test -p sotf-plugin-dynamic-eq` — 57 passed across three suites.
- `cargo clippy -p sotf-plugin-dynamic-eq --all-targets --no-deps -- -D warnings` — passed.
- `cargo run -p sotf-plugin-dynamic-eq --features qa --bin qa-dynamic-eq` —
  standard zero-allocation/performance QA and the full layout matrix passed.
- The shared facade allocation target is currently blocked by unrelated
  concurrent Loudness Compensation compile errors; Dynamic EQ itself compiles
  and tests cleanly.

## Findings

### P1 — Runtime gain changes update the reported parameter but not the EQ filter coefficients

The `band_N_gain` setter only assigns `band.target_gain_db` (`crates/sotf-plugins/crates/sotf-plugin-dynamic-eq/src/lib/dynamic_eq_plugin.rs:468-471`). Unlike the frequency and Q setters immediately above it, it never calls `rebuild_eq_filters`. The process loop then derives its modulation proportion from the new target but runs the old `eq_filters[ch]` coefficients (`dynamic_eq_plugin.rs:647-656`, `673-677`). The getter and rebuilt schema expose the new value, so the host can show a successful +12 dB change while audio still uses the previous filter—often the default 0 dB passthrough. A later frequency/Q change, reset, or initialize suddenly makes that stored gain audible.

Rebuild or otherwise retarget the filter when gain changes, with an artifact-controlled transition rather than an instantaneous state reset. Add a public-API regression that starts at 0 dB, calls `set_parameter("band_0_gain", +12 dB)`, processes an above-threshold 1 kHz tone without any other setter/reset, and compares the steady-state response with a plugin constructed at +12 dB. Also test the reverse transition and multiple changes mid-stream. The existing per-band roundtrip test only checks the stored value (`tests/integration.rs:66-88`), so it cannot detect this DSP/state divergence.

### P1 — Linear dry/wet interpolation does not implement the requested intermediate gain in dB

`modulation_proportion` divides positive compressor gain reduction in dB by `abs(target_gain_db)` (`crates/sotf-plugins/crates/sotf-plugin-dynamic-eq/src/lib/dyn_eq_band.rs:121-125`), then the hot loop linearly interpolates samples between the dry signal and a filter fixed at the full target gain (`dynamic_eq_plugin.rs:647-656`, `673-677`). Decibels are logarithmic, so this produces the wrong modulation depth and asymmetric boost/cut behavior. At a filter center, a 24 dB target and 10 dB detector reduction selects `10/24` of a 15.85x full-boost response, yielding about +17.1 dB rather than +10 dB; the corresponding -24 dB target yields only about -4.3 dB. Even a half-depth ±6 dB target produces roughly +3.5/-2.5 dB rather than ±3 dB. Tests merely assert the scalar fraction or full-depth steady state (`src/lib/tests.rs:146-161`, `src/lib/tests/misc.rs:424-492`), exactly the cases that hide this error.

At minimum derive an interpolation coefficient in the linear-amplitude domain so the center-frequency magnitude matches the desired signed dB depth. That still will not reproduce the full complex response of a true intermediate-gain peaking EQ away from center. Higher-quality options are a modulation-safe/TPT filter whose gain can move sample by sample, a bounded-rate stable coefficient update with interpolation, or carefully crossfaded precomputed gain states. Verify positive and negative targets over several target/GR fractions, frequencies, Q values, sample rates, and block partitions against an offline time-varying EQ reference.

### P1 — Per-band setters bypass schema validation and accept non-finite values into filter design

`parametric_set_parameter` explicitly skips `parametric_validate_parameter` for every ID beginning with `band_` (`dynamic_eq_plugin.rs:530-542`). The branches for frequency, Q, gain, band threshold, and band ratio then call `clamp` without an `is_finite` check (`dynamic_eq_plugin.rs:454-484`). A NaN therefore survives clamping; frequency/Q immediately rebuild biquads from NaN, and gain stores NaN for a later rebuild. Wrong value types, out-of-range band indices, and unknown fields also return `Ok(())`; `test_set_parameter_band_unknown_field_ignored` codifies the latter behavior (`src/lib/tests.rs:591-604`). This contradicts the host trait contract that values have been schema-validated and unknown keys are errors (`crates/sotf-plugins/crates/sotf-host/src/parametric_in_place_plugin.rs:45-49`). Bulk `apply_values` similarly trusts whatever caller constructed the map.

Route global and dynamic band parameters through the same schema validation, explicitly reject NaN/infinities, malformed IDs, inactive/out-of-schema band indices, unknown fields, and wrong types, then mutate only after validation succeeds. Make bulk application atomic so one invalid entry cannot leave a partially changed plugin. Add direct adapter and factory/preset tests for all numeric band fields, invalid IDs/types, and non-finite programmatic `serde_json::Value`s.

### P1 — The advertised 20 kHz range is invalid at lower sample rates and initialization does not enforce Nyquist

Frequency is clamped to a fixed `[20, 20000]` range in construction and live updates (`dynamic_eq_plugin.rs:148-154`, `454-459`), matching the static schema (`src/params.rs:70-82`). `bandpass_edges` likewise caps its upper sidechain edge at 20 kHz (`src/lib/misc.rs:7-16`). `initialize` rebuilds the peak, high-pass, and low-pass biquads directly at those values for any supplied sample rate (`dynamic_eq_plugin.rs:547-557`; `src/lib/dyn_eq_band.rs:128-159`). At 32 kHz, for example, 20 kHz is above the 16 kHz Nyquist limit; even below that, the upper band edge can exceed Nyquist. Depending on the biquad implementation, this yields a warped, invalid, or unstable detector/EQ rather than the schema-promised band.

Validate a nonzero supported sample rate and constrain every designed frequency to a documented margin below Nyquist (including both sidechain edges), or reject presets that cannot be represented. Ideally expose a sample-rate-aware effective range to the UI. Test 8/16/32/44.1/48/96 kHz at frequency and Q extremes with impulse stability, finite output, and measured center/bandwidth.

### P2 — Active/solo and link-mode transitions revive stale state and can click

Inactive and non-solo bands are skipped before either their sidechain or EQ biquads and dynamics envelopes advance (`dynamic_eq_plugin.rs:617-628`). Their IIR/envelope histories therefore freeze at an earlier point in the stream; setting `active=true` or clearing another band's solo resumes those stale states without reset or warm-up (`dynamic_eq_plugin.rs:486-493`). The resulting first samples can contain an unrelated old filter transient or gain envelope. Similarly, linked mode advances only `band.cores[0]` (`dynamic_eq_plugin.rs:630-656`); after live switching to unlinked mode, channels 1..N resume their older envelopes while channel 0 continues the linked envelope, producing a channel-balance jump. Although `link_channels` is marked `.setup()` in the declarative schema (`src/params.rs:57-66`), the runtime setter changes it immediately (`dynamic_eq_plugin.rs:435-437`).

Define these as graph-rebuild-only controls or implement explicit transition semantics. For active/solo, either keep detector/filter state warm while suppressing its audible contribution or reset/crossfade deterministically. For link changes, seed all per-channel envelopes from the linked value (and define the reverse aggregation) before a short crossfade. Add long-history transition tests with impulses and asymmetric stereo, checking bounded first-sample discontinuity and block-size-independent results. Current tests mostly assert flags; `test_solo_mutes_other_bands` does not inspect output at all (`src/lib/tests.rs:264-313`).

### P2 — Reset leaves parameter smoothers mid-ramp

`reset` rebuilds band filters/cores and clears monitoring, but does not reset `mix_smoother` or `threshold_smoother` (`dynamic_eq_plugin.rs:571-576`). Both continue advancing their pre-reset ramps in subsequent processing (`dynamic_eq_plugin.rs:616`, `692`). A seek/transport reset can therefore restart DSP state while retaining an undocumented intermediate threshold or wet mix, and the first post-reset block depends on how far the old block sequence had progressed.

Specify the reset policy and reset each smoother consistently—normally snap current to its stored target while preserving the parameter value. Also reset `cache_counter` and publish cleared monitoring if the UI contract requires an immediate cold state. Add sample-accurate tests that reset halfway through both ramps and compare the first post-reset samples with a freshly initialized plugin having the same parameter targets.

### P2 — Live frequency/Q changes hard-reset three IIRs per channel with no smoothing or crossfade

The frequency and Q setters synchronously replace both sidechain biquads and the audio peaking biquad (`dynamic_eq_plugin.rs:454-466`; `src/lib/dyn_eq_band.rs:128-159`). Replacement discards filter history, and it occurs through the ordinary runtime parameter API. Even if setters run on a control thread, the next audio sample observes a discontinuous detector and signal transfer; rapid automation also repeats trigonometric coefficient design work proportional to bands × channels. `apply_values` then reconstructs the entire dynamic parameter schema, including formatted IDs and vector allocation, after every value change (`dynamic_eq_plugin.rs:177-290`, `499`).

Separate control-thread schema mutation from realtime automation. Smooth frequency/Q in a stable filter structure, or prepare new filter state off-thread and crossfade at a block boundary. Do not rebuild an unchanged schema for ordinary value updates; rebuild only when `num_bands` changes. Add automation tests that sweep frequency/Q across varied block sizes and bound discontinuity, allocations, and CPU time.

### P3 — Default zero-gain bands still run the complete detector and EQ hot path

The default declares four active bands, each with zero target gain (`src/params.rs:21-29`, `88-104`, `251-265`). `modulation_proportion` correctly returns zero for a near-zero target, but only after each band has run two sidechain biquads per channel, absolute/log conversion, compressor curve/envelope, and one audio EQ biquad per channel (`dynamic_eq_plugin.rs:617-680`). Thus an untouched default Dynamic EQ spends most of its normal cost producing bit-identical dry output. The same full processing occurs while wet mix is settled at zero before the final dry overwrite (`dynamic_eq_plugin.rs:690-702`). At eight bands and many channels, the per-sample `fast_log10` and three biquads per band dominate.

Add settled-state fast paths: zero-gain bands can be skipped entirely because no audible modulation is possible, while mix=0 can at least skip audio EQ filtering (whether detectors remain warm should follow the transition policy). Cache whether any solo/active/audible bands exist when controls change. Benchmark 1/2/8/16/32 channels, 1/4/8 bands, zero/nonzero gain, linked/unlinked modes, and 32–2048-frame blocks before restructuring the loop. The existing QA benchmark covers only one mono band and therefore cannot characterize the worst or default cases (`bin/qa_dynamic_eq.rs:11-58`).

## Algorithm and realtime assessment

The plugin is an interleaved, in-place, channel-preserving processor with zero reported latency. Detection is peak-based: dry input is passed through a cascaded HP/LP sidechain approximation for each parametric band, converted to dB, mapped by the shared compressor soft-knee curve, and attack/release-smoothed. Linked mode uses the maximum instantaneous bandpassed magnitude across channels. Audio bands are cascaded in band order; sidechains correctly read the original dry block, avoiding order-dependent detector contamination. The process path validates exact `num_frames * channels`, preserves `context.num_frames`, flushes denormals, and has no normal-path heap allocation, locks, logging, I/O, or FFT planning. The focused allocation test and standard QA both confirm zero allocations for their tested mono configuration.

The main algorithmic limitation is more serious than normal detector topology tradeoffs: interpolating dry and full-gain peaking-filter output by a dB fraction does not produce that fraction of gain in dB. The two-biquad sidechain is also only an approximation to the peaking EQ's detection bandwidth, especially after its 20 Hz/20 kHz edge clamps. There is no lookahead, so fast transients necessarily pass before attack settles; that is acceptable only if documented. Cascading multiple dynamically blended filters is nonlinear/time-varying, making broadband, multi-tone, and block-partition reference tests essential.

Setup allocates all band/channel biquads, dynamics cores, monitoring data, and a dry buffer. `initialize` may resize the dry buffer to one second at 96 kHz (`dynamic_eq_plugin.rs:562-567`), safely outside processing, and oversized blocks return an error rather than allocate. Control mutations do allocate through `ParameterSet`, schema cloning/rebuilding, and formatted per-band IDs; this is acceptable only on a manager/control thread and should not be conflated with audio-rate automation. Filter rebuilds themselves are allocation-free at this level but are state-destructive and computationally inappropriate for dense realtime automation.

## Scope reviewed

Read every one of the 16 indexed plugin-owned files without omission: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`, `src/lib.rs`, `src/params.rs`, all seven modules under `src/lib/`, the nested `src/lib/tests/misc.rs`, `tests/integration.rs`, and `bin/qa_dynamic_eq.rs`. There are no plugin-owned benches or additional feature-gated source files; the sole `qa` feature gates the QA binary.

Also checked active compilation and public wiring in the facade `Cargo.toml`/exports, main and bridge factories, factory catalog metadata, parameter adapter and `RealTimeCache` host contracts, the shared `DynamicsCore` gain/envelope implementation, engine `PluginSettings` conversion/preset serialization, the custom GPUI Dynamic EQ controls/monitoring, factory/parameter/allocation callers, and TokenSave test-risk/panic-site results. The crate is actively compiled and registered as the canonical `dynamic_eq` processor. No production source was changed.

## Existing strengths

- Dry-buffer sidechain detection prevents earlier EQ bands from contaminating later detectors.
- Per-band filters, dynamics cores, monitoring storage, and block scratch are preallocated; the valid processing path is zero-allocation and lock-free.
- Exact buffer-size and checked multiplication guards return errors rather than panicking, including oversized realtime blocks.
- Threshold and final dry/wet mix advance per frame rather than once per block, so those two ramps are independent of callback partitioning.
- Linked and unlinked processing, inactive transparency, dry mix, reset, frequency selectivity, sidechain isolation, filter rebuilds, non-finite global parameters, and zero allocation all have at least focused baseline coverage.
- Factory, facade, engine converter, custom UI, and parameter-spec keys agree on the canonical `dynamic_eq` identity and global parameter units.

## Verification performed

```bash
cargo test -p sotf-plugin-dynamic-eq
# 51 passed across three suites

cargo test -p sotf-plugins --test realtime_allocation_tests test_dynamic_eq_zero_alloc
# 1 passed; 45 filtered out

cargo run -p sotf-plugin-dynamic-eq --features qa --bin qa-dynamic-eq
# standard latency, zero-allocation, and mono performance QA passed
```

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-dynamic-eq
cargo test -p sotf-plugins --test realtime_allocation_tests test_dynamic_eq_zero_alloc
cargo run -p sotf-plugin-dynamic-eq --features qa --bin qa-dynamic-eq
cargo clippy -p sotf-plugin-dynamic-eq -- -W warnings
cargo check -p sotf-plugins
```
