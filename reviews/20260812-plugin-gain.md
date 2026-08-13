# Gain plugin code review — 2026-08-12

## Remediation status

- **Fixed:** preset/config construction now rejects zero channels, non-finite
  values, and values outside the advertised gain/smoothing ranges.
- **Fixed:** entering per-channel mode preserves each channel's current value
  and global target; returning to global mode starts from channel 0's current
  linear gain, avoiding a dormant-smoother jump.
- **Fixed:** normal and compiled processing now share checked active-prefix
  buffer semantics; zero-frame calls preserve output.
- **Fixed:** reset snaps global and per-channel smoothers to their declared
  targets.
- **Fixed in 0.5.8:** settled global and per-channel gains use one whole-block
  SIMD kernel call, while moving smoothers retain sample-accurate frame
  processing. Deterministic tests lock path selection and partition invariance.
- **Fixed in 0.5.8:** runtime plugin metadata sources its version from
  `CARGO_PKG_VERSION`, with unit and public integration coverage pinning the
  manifest/runtime identity.
- **Fixed and verified:** fallible preset and per-channel mutation paths reject
  invalid values. Legacy infallible global setters retain compatibility by
  treating NaN, infinities, and out-of-range requests as documented atomic
  no-ops. `invalid_infallible_global_setters_are_atomic_no_ops` proves they
  preserve global/per-channel mode, smoother current/target, cached/reported
  values, and rendered audio; the validated parametric API reports errors.
- **Verified in 0.5.9:** a real compiled `DawHost` processes settled unity,
  receives `gain_db = -6` through the host parameter API, and renders the next
  block at exactly the new linear gain. The compiled plan re-queries current
  static metadata per processing segment, so no build-time fused scalar can
  survive automation.

## Findings

### P1 — Preset/config construction bypasses every advertised numeric constraint

The factory deserializes JSON directly into `GainPluginParams` and calls `GainPlugin::from_params` (`crates/sotf-plugins/src/factory/create.rs:82-84`). `from_params` forwards `gain_db`, every `channel_gains` value, and `smoothing_ms` to constructors without checking finiteness or the `[-60, 20] dB` / `[0, 100] ms` schema (`crates/sotf-plugins/crates/sotf-plugin-gain/src/lib.rs:62-78`, `97-140`). The public setters likewise accept arbitrary floats (`lib.rs:147-190`). Only the adapter-driven `parametric_set_parameter` path validates the schema.

A JSON preset containing `NaN` through a programmatic serde value, an extreme finite gain, negative smoothing, or an infinite channel gain can therefore create non-finite output or behavior inconsistent with the UI/API contract. Validate all construction and public mutation through one shared routine before converting dB to linear. Prefer fallible constructors/setters, reject non-finite values, and test factory creation at both boundaries plus NaN/infinities/extremes for global gain, per-channel gain, and smoothing.

### P1 — Entering per-channel mode during a global ramp can freeze unedited channels at an undocumented intermediate gain

`set_channel_gain_db` lazily enters per-channel mode by labeling every channel with `global_gain_db`, but constructs every new smoother from only `global_gain_smoother.current()` (`lib.rs:174-190`). `Smoother::new` starts current and target at that value. If the global smoother is mid-ramp, the unedited channels permanently remain at the intermediate linear value while `channel_gain_db()` and parameter snapshots report the global target dB (`lib.rs:199-206`, `320-337`). The edited channel receives a new target; the rest do not.

This is an audible state/reporting divergence and can alter channel balance when automation switches modes. When cloning global state, preserve both current and target in every channel smoother (or complete/reset the transition according to an explicitly documented policy) before changing the selected channel. Add a sample-accurate regression that starts a nonzero smoothing ramp, processes part of it, sets one channel, and verifies both the edited and untouched channels plus reported values.

### P2 — Switching back to global mode resumes a stale global smoother, causing a discontinuity

Per-channel processing never advances `global_gain_smoother` (`lib.rs:245-261`). `set_gain_db`/`set_gain_linear` then discard the channel smoothers and set a target on that dormant global smoother (`lib.rs:147-157`). After spending any time in per-channel mode, returning to global mode starts from whatever global state existed before that mode—not from the current output gain of any defined channel/reference. This can jump immediately before smoothing toward the new target.

Define the mode-transition rule (for example start from channel 0, an average linear gain, or require an explicit global base that continues advancing), then seed the global smoother consistently. Add transition tests in both directions after long and partial ramps. The UI documentation currently presents global and per-channel controls together, making predictable semantics particularly important.

### P2 — The normal process entry point can panic on valid-looking non-exact host buffers

`process` calls `output.copy_from_slice(input)` before deriving the active sample count (`lib.rs:414-421`), which panics unless the two entire slices have exactly equal length. The compiled path correctly calculates `num_frames * channels`, checks both bounds, and copies only the active region (`lib.rs:424-451`). The two entry points therefore have different robustness and tail-buffer semantics.

Use the same checked sample-count logic in both paths and return `PluginResult::Err` for undersized buffers. Decide whether oversized tails must be preserved or ignored and test it. This also makes zero-frame behavior explicit instead of copying the whole input while reporting zero frames (`tests/integration.rs:268-280`).

### P2 — Reset is a no-op despite stateful gain ramps

`GainPlugin` does not override `ParametricPlugin::plugin_reset`, whose default does nothing (`crates/sotf-plugins/crates/sotf-host/src/parametric_plugin.rs:80`). Thus seek/transport reset preserves partially advanced global and per-channel smoother states. The integration test named `reset_does_not_break_processing` only asserts a frame count at unity and cannot observe this (`crates/sotf-plugins/crates/sotf-plugin-gain/tests/integration.rs:158-171`).

Specify whether reset snaps each smoother to its target or restarts a transition from a defined value, implement that rule, and verify the first samples following reset for both modes. If preserving state is intentional, document it as a host contract and rename the test to match what it proves.

### P3 — The supposed SIMD hot path is organized as tiny per-frame calls

**Fixed in 0.5.8.** `process_in_place` detects settled global and per-channel
smoothers and applies their gains to the complete active block through one SIMD
kernel call. Only genuinely moving smoothers use the per-frame path required to
preserve sample-accurate gain trajectories. `settled_global_gain_uses_one_block_kernel`,
`settled_per_channel_gain_uses_one_block_kernel`,
`moving_gain_keeps_sample_accurate_smoothed_path`, and
`settled_fast_path_preserves_ramp_partition_invariance` pin the dispatch and
streaming contracts.

The processing loop slices one interleaved frame at a time and invokes scalar/SIMD helpers per frame (`lib.rs:218-261`). Global gain smoothing requires one gain per frame, but processing all channels in tiny slices inhibits vectorization for the common 1–4 channel cases; per-channel mode additionally advances all smoothers and writes `cached_gains` on every frame. For more than four channels it calls `apply_per_channel_gain_simd` on only one frame, paying dispatch/setup repeatedly.

Benchmark realistic 2/6/8/16/32-channel blocks before changing this. Candidate improvements are a host SIMD kernel accepting a per-frame ramp, channel-specialized loops, processing stable settled gains as a whole-buffer operation, and avoiding smoother `advance()` once current equals target. Keep the existing scalar helpers as correctness references. This is a recommendation, not a demonstrated regression.

### P3 — Documentation and package metadata disagree on the shipped smoothing behavior/version

**Fixed in 0.5.8.** The runtime `PluginInfo` version now comes directly from
the crate manifest through `CARGO_PKG_VERSION`; unit and public integration
tests require exact equality. The plugin documentation describes the 10 ms
canonical default and the split between moving and settled processing paths.

`AGENTS.md` says the old default was 20 ms and the canonical default is 10 ms, while `CHANGELOG.md` starts with an unversioned fragment and `Cargo.toml` remains `0.5.5` despite describing later fixes. The plugin reports its own unrelated version `1.2.0` (`lib.rs:267-270`). `USAGE.md` also repeatedly describes 20 ms smoothing. These contradictions make preset compatibility and listening-test expectations difficult to audit.

Choose a single plugin-version policy and update package/plugin/docs together. Add a test pinning the canonical default already sourced from `params::PARAMS`; avoid duplicating the numeric value in prose unless release notes state the change explicitly.

## Algorithm and realtime assessment

The gain conversion is conventional (`10^(dB/20)` through the host helper), channel layout is preserved, latency is zero, and the sample loop itself allocates no memory, takes no locks, and returns `context.num_frames`. Per-channel scratch and parameter keys are preallocated. Allocation does occur when switching modes (`lib.rs:150-180`) and schema/current-value queries construct vectors/maps (`lib.rs:284-338`); the trait documents parameter application as control-thread work, so this is acceptable only if the host upholds that separation.

Compile metadata correctly prevents static fusion while a global smoother is moving and for all per-channel configurations (`lib.rs:454-482`). A settled global gain is safely eligible for fusion. However, construction/state changes and render-plan compilation must remain synchronized so a fused stale gain is never retained after automation—this is primarily a host contract and deserves an integration test.

**Closed in 0.5.9.** `compiled_host_reloads_static_gain_after_automation`
constructs the public adapter inside a compiled `DawHost`, renders unity,
updates `gain_db` through `Host::set_plugin_parameter`, and proves the next
block uses exactly `db_to_linear(-6)`. This pins the host's dynamic metadata
reload rather than inferring it from direct plugin processing.
The complementary host probe
`test_compiled_static_gain_automation_stays_fused_and_reloads_scalar` changes
the mock's advertised scalar through queued automation and asserts that its
regular process-call counter remains zero before and after, proving the plan
both retains fusion and reloads the current scalar.

There is no explicit bypass parameter. Positive gain intentionally can exceed full scale; clipping belongs later in the engine/output callback per repository policy. No denormal handling is needed for a simple multiplication, although the smoother should be checked to snap sufficiently close to target rather than asymptotically carrying subnormal deltas.

## Scope reviewed

Read all plugin-owned material: `AGENTS.md`, `README.md`, `USAGE.md`, `UI.md`, `CHANGELOG.md`, `Cargo.toml`, `src/lib.rs`, `src/params.rs`, all files under `tests/`, and `bin/qa_gain.rs`. Also checked factory construction, catalog/facade registration, the host `ParametricPlugin` adapter/reset/smoothing contracts, compiled-op behavior, and TokenSave callers/test-risk/panic-site results. No source-code changes were made.

## Existing strengths

- Parameter IDs/names and per-channel processing scratch are cached; the audio loop has no allocation, locks, or logging.
- Parameter adapter calls reject non-finite and out-of-range runtime values, and tests cover those public host paths.
- Global and per-channel known-output tests, property tests, sample-rate reinitialization tests, and compiled-vs-regular equivalence provide a useful base.
- Compile/fusion metadata is parameter-sensitive and conservatively refuses per-channel fusion.
- Channel count and zero latency are reported consistently.

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-gain
cargo test -p sotf-plugins --test all_plugins_dsp_matrix
cargo clippy -p sotf-plugin-gain -- -W warnings
cargo check -p sotf-plugins
```
