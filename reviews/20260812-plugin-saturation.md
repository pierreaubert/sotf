# Saturation plugin review — 2026-08-12

## Remediation status

### Metadata follow-up (0.5.12, 2026-08-13)

The host-visible parameter surface now marks every topology-affecting control
(`mode`, `exciter_freq`, `oversampling`, `dc_blocker`, and `use_adaa`) as
structural, matching the initialized setter's graph-rebuild requirement.
`topology_controls_are_advertised_as_structural` guards the complete contract.

Closed in `0.5.11` (2026-08-13): all reported P1-P3 findings are fixed and
covered; this review reported no P0 finding.

- **Signal composition and automation:** dynamic drive now feeds exactly one
  selected topology, including the Exciter high band, and drive/mix/gain
  smoothers advance once per source frame. Tests cover every mode with dynamics
  off/on and Off/2x/4x host factors, randomized callback partitions, and actual
  4x alias rejection.
- **Oversampling and latency:** oversampling is host-owned. The inner plugin
  advertises 2x/4x, declares zero latency, and contains no resampler; the host
  wrapper consumes the preference exactly once, reports its latency, and keeps
  dry/wet paths aligned. Impulse and spectral regressions cover the contract.
- **Realtime topology contract:** mode, oversampling, crossover, ADAA, and DC
  blocker changes are structural after initialization and return an error so
  the graph can rebuild off-thread. Atomic rejection and allocation-counted
  continuous updates cover the parameter path.
- **Validation and failure behavior:** typed schema validation, fallible factory
  construction, low-rate Nyquist checks, initialization/context-rate checks,
  zero-channel rejection, checked sample-count multiplication, and fixed
  callback capacity errors cover malformed and overflow-prone inputs. Removing
  the internal oversampler also removes the former swallowed-error path.
- **Reset, documentation, and memory:** reset settles all smoothers; Tube and
  Tape are documented truthfully as static waveshapers, with an odd-symmetry
  regression for Tube; scratch storage uses a 65,536-frame maximum callback
  contract and no longer grows with sample rate or on the callback.
- **Composed QA:** the QA executable runs non-silent stereo Exciter + dynamics +
  partial mix through the actual 4x host wrapper for 1,000 callbacks, asserts
  finite output and zero callback allocations, and reports CPU cost.

Verification on the exact plugin changes:

- `rtk cargo test -p sotf-plugin-saturation` — 55 tests passed across three suites.
- `rtk cargo run -p sotf-plugin-saturation --features qa --bin qa-saturation` —
  composed 4x path passed at 1.17% CPU with zero callback allocations.
- `rtk cargo clippy -p sotf-plugin-saturation -- -W warnings` — plugin clean;
  only three pre-existing warnings in shared host analyzer/auto-gain code were emitted.

Implemented in the `0.5.9` follow-up:

- **Fixed:** oversampled dynamic processing now feeds the selected nonlinearity;
  dynamic Exciter processing continues to saturate only its high band rather than
  silently behaving like static processing.
- **Covered:** a regression compares static and dynamic 2x-oversampled Exciter
  output and requires the dynamic control to affect the selected topology.

Implemented in the `0.5.10` follow-up:

- **Fixed:** bulk mode/oversampling updates reject unknown enum values before
  changing state, so malformed presets cannot silently select another topology
  or partially apply a multi-parameter update.
- **Covered:** a regression verifies rejection and atomicity for an invalid
  oversampling enum paired with a valid mode update.

Implemented in the `0.5.8` remediation:

- **Fixed:** dynamic processing no longer overwrites the selected direct, ADAA, or non-oversampled
  Exciter path; dynamic Exciter modulation is applied to its high-band nonlinearity.
- **Fixed:** internal oversampler latency is reported and `preferred_oversampling()` returns `None`,
  preventing automatic double oversampling.
- **Fixed:** construction/factory validation rejects malformed modes, factors, channel counts, and
  non-finite or out-of-range values; zero sample rate is rejected and Exciter frequency is bounded
  below Nyquist.
- **Fixed:** reset settles drive, mix, and output smoothers.
- **Fixed:** oversampler processing failures now propagate as errors instead of
  being reported as successful full-block processing; regression coverage
  injects a failure and verifies it is not converted to full consumption.
- **Partially fixed:** direct Exciter drive now follows the per-frame smoother ramp. Oversampled paths
  use the source block's maximum envelope control; exact source-frame interpolation still needs an
  Oversampler control API.
- **Deferred:** dry/wet latency alignment, realtime-safe topology changes, tighter maximum-block
  scratch sizing, analog-model improvements, and broader spectral/
  allocation QA. These need larger host or algorithm changes than this scoped remediation.

## Findings

### P1 — Dynamic saturation replaces, rather than augments, every primary processing path

`process_in_place` first runs the selected exciter, oversampled, ADAA, or direct path (`saturation_plugin.rs:700-807`), but when `dynamic_amount > 0.001` it overwrites every wet sample with a fresh call to `saturate(dry, mode, dynamic_drive, tone)` (`saturation_plugin.rs:809-827`). The expensive first pass is discarded. This also removes the requested anti-aliasing: the replacement is neither oversampled nor ADAA. In Exciter mode `saturate(..., Exciter, ...)` is the identity, so any nonzero dynamic amount replaces the split-band exciter with dry audio.

Fix by calculating the envelope-scaled drive before the selected nonlinearity and feeding that drive through exactly one selected processing topology. For oversampling, provide an oversampled drive envelope (or interpolate control values in the closure); for Exciter, apply it only to the high band. Add combination tests for every mode with dynamic amount off/on and oversampling off/2x/4x, including a spectral assertion that Exciter remains active and an alias-energy comparison.

### P1 — Internal oversampling has latency, but the plugin declares zero latency and also requests host oversampling

The plugin constructs and runs its own `Oversampler` (`saturation_plugin.rs:225-237,700-764`) while `compile_metadata` declares latency 0 (`saturation_plugin.rs:400-402`). It additionally returns the same factor from `preferred_oversampling` (`saturation_plugin.rs:853-858`), inviting a supporting host to oversample a plugin that already oversamples itself. Dry/wet mixing at `saturation_plugin.rs:835-845` does not delay the dry path, so a partial mix comb-filters even if the host compensates downstream latency.

Choose one ownership model: preferably let the graph own oversampling and make the plugin process at the supplied rate, or keep it internal, report the oversampler's exact latency, and stop advertising host oversampling. Delay dry by the wet-path latency for parallel mix. Add impulse tests for reported latency, dry/wet alignment, factor changes, and a host integration test proving the nonlinearity is oversampled exactly once.

### P1 — Drive automation is block-constant in Exciter and oversampled modes

The smoother computes a sample ramp (`saturation_plugin.rs:666-674`) but both oversampled closures and the direct Exciter path use only `drive_end` (`saturation_plugin.rs:692-745,747-758`). Comments claim the ramp is applied in the final mix loop, but that loop only ramps output gain and mix (`saturation_plugin.rs:835-845`). Consequently output depends on callback partitioning and a parameter step takes effect at the next block's final value rather than over the documented smoothing interval.

Generate a per-frame drive control buffer before oversampling and interpolate it to the oversampled rate, or extend `Oversampler` to expose source-frame phase. Pass the per-frame value through direct Exciter processing. Add a one-block-versus-randomly-partitioned automation equivalence test for all modes and factors.

### P1 — Live quality/topology changes allocate, reset state, and change delay without a graph rebuild

Setting `oversampling` calls `rebuild_oversampler`, which allocates a new filter/buffer graph (`saturation_plugin.rs:496-512,225-237`). Mode, ADAA, and DC-blocker switches are also applied immediately (`saturation_plugin.rs:458-472,558-565`) without crossfade or compatible-state transition. These operations can occur on the real-time parameter path, causing allocation/free work, discontinuities, changed latency, and stale state when switching back.

Classify topology/latency-affecting settings as compile-time parameters and rebuild off the callback thread, then atomically swap at a block boundary with latency-aware crossfade. If live switching is required, prebuild all paths and crossfade between continuously maintained states. Test allocation freedom and click bounds during automated switches.

### P1 — Construction validation silently accepts invalid configuration states

`from_params` silently maps unknown strings to defaults and uses floating-point `clamp` without first rejecting non-finite inputs (`saturation_plugin.rs:160-192`); NaN survives Rust's `f32::clamp`. Runtime `apply_values` likewise converts invalid mode/oversampling types and strings to a valid but unrelated choice (`saturation_plugin.rs:458-470,496-508`). `exciter_freq` is capped at 10 kHz independent of sample rate, allowing a crossover at or above Nyquist for low-rate/offline hosts (`saturation_plugin.rs:174,493-495,574-580`). `nf * nc` and the initialization scratch-size product are unchecked (`saturation_plugin.rs:603-604,644-646`).

Return errors for unknown enum values, wrong parameter types, non-finite numbers, zero/unsupported channel counts, zero sample rate, multiplication overflow, and frequencies outside a conservative fraction of Nyquist. Route factory construction through the same schema validation as runtime updates. Add malformed-factory, NaN/Inf, low-sample-rate, zero-channel, and overflow tests.

**Remediation:** the enum fallback portion is fixed in 0.5.10. `apply_values()`
preflights all mode and oversampling strings before mutating the plugin, and the
new regression verifies that an invalid value leaves every topology field
unchanged. The larger scratch/low-rate and numeric overflow concerns remain
deferred because they require a shared host/block-size contract.

### P2 — Oversampler errors are converted into apparently valid output

Both calls use `.unwrap_or(nf)` (`saturation_plugin.rs:720-728,750-758`). If processing fails, the plugin claims the entire block was written and continues with a potentially partial/stale buffer instead of returning the error or producing a defined safe fallback.

Propagate the error. If the host contract requires audio continuity, explicitly zero the wet result or use a latency-aligned bypass and report diagnostics; do not reinterpret failure as success. Inject an oversampler failure in a test and assert deterministic output plus error behavior.

### P2 — Reset leaves parameter smoothers at their pre-reset trajectories

`reset` clears crossover, oversampler, DC, ADAA, and envelope state, but not the three smoothers (`saturation_plugin.rs:614-636`). After transport restart or graph reuse, drive/mix/output can resume partway through an earlier automation ramp rather than from their current parameter values.

Reset each smoother to its current target/value according to the host reset contract. Add a test that interrupts each ramp, resets, and checks the first post-reset sample.

### P2 — The advertised analog models are materially simpler than their descriptions

The parameter schema describes Tube tone as an “even/odd balance” (`saturation_plugin.rs:256-265`), but the reviewed tube function is odd-symmetric and therefore cannot generate even harmonics from a symmetric input. Tape is a memoryless sigmoid rather than a tape model with hysteresis, bias, head bump, frequency-dependent saturation, or level-dependent dynamics. These may be useful waveshapers, but the naming/documentation overstates the algorithms.

Either rename/document them as static tube- and tape-flavoured waveshapers, or implement controlled asymmetry for even harmonics and a modest stateful tape model. Add harmonic-distribution tests and reference plots over drive/tone, plus level/frequency sweeps if the stronger emulation claims remain.

### P2 — Scratch sizing consumes far more memory than real-time blocks require

Initialization reserves three interleaved buffers for `(sample_rate + 8192) * channels` samples (`saturation_plugin.rs:601-609`) regardless of the host's maximum block size. At 192 kHz and 12 channels this is about 28.8 MiB just for these three `f32` buffers, before oversampler and crossover state. Normal processing also performs multiple full-buffer copies/passes (`saturation_plugin.rs:663-664,705-717,730-742,835-849`).

Accept a declared maximum block size during graph build and size scratch exactly to that contract. Reuse or alias scratch where lifetimes do not overlap, fuse output/DC/mix passes where practical, and benchmark common channel/block/factor combinations. Keep the existing hard capacity error rather than growing on the callback.

### P3 — QA coverage is broad at the unit level but misses the risky composed paths

The focused tests pass, but the QA executable exercises silence and the suite does not catch dynamic+Exciter becoming dry, loss of ADAA/oversampling under dynamics, block-size-dependent drive automation, latency/dry alignment, topology switch clicks/allocations, oversampler failure, or non-finite factory parameters.

Add impulse, sine/FFT alias, harmonic, randomized block partition, state-transition, and allocation-counting tests. Include non-silent multichannel signals and parameter combinations rather than testing features only in isolation.

## Algorithm assessment

The plugin provides useful static nonlinearities, per-channel ADAA state, optional oversampling, an LR4 split-band Exciter, envelope-driven drive, DC blocking, smoothing, and dry/wet control. The primary quality problem is composition: independently reasonable features are executed sequentially and then overwritten or misaligned. Resolve topology ownership and signal-flow composition before investing in more elaborate analog models.

## Real-time allocation and performance assessment

The steady-state process path does not intentionally grow scratch buffers, uses per-channel persistent DSP state, and flushes denormals. However, live oversampling changes allocate/free DSP state; initialization overallocates three large buffers; failed capacity is handled safely; and dynamic mode currently pays for a complete wet pass that it discards. After fixing correctness, fuse passes and benchmark oversampling, Exciter, dynamics, DC blocking, and partial mix independently and in combination.

## Scope reviewed

Read in full: `Cargo.toml`, `README.md`, `CHANGELOG.md`, crate `AGENTS.md`, `src/lib.rs`, `src/params.rs`, all files under `src/lib/` including nested unit tests, `tests/integration.rs`, and `bin/qa_saturation.rs`. Relevant host contracts reviewed include plugin compile metadata, smoothing, ADAA, LR4 crossover, DC blocker, envelope follower, oversampler, parameter schema/application, factory registration, and graph oversampling/latency behavior. No production code was changed.

## Strengths

- Scratch buffers and per-channel state are prepared outside normal processing, with an explicit capacity error rather than callback growth.
- ADAA state is correctly separated per channel, avoiding interleaved-channel history corruption.
- Numeric runtime setters reject non-finite values, and normal numeric controls are bounded.
- Denormal handling, sample-count return, buffer-length checks, and focused tests are present.
- The code is organized into small waveform/default/parameter modules and documents intended signal flow well enough to expose where implementation diverges.

## Verification

`rtk cargo test -p sotf-plugin-saturation` — 42 tests passed across three suites.
