# Loudness compensation plugin review — 2026-08-12

## Remediation status

All P1–P2 findings are closed in version 0.5.6, with focused regression coverage.
The legacy `auto_gain_enabled` schema entry is now explicitly referenced by a
hidden compatibility control, so layout coverage remains complete without
rendering a second AutoGain state beside canonical `auto_gain_position`.
CLI rack and traditional builders now also populate the canonical position,
headroom, and calibration fields explicitly. Manual mode derives Disabled/Post
position from its legacy AutoGain flag while preserving uncalibrated,
reference-preserving defaults; Fletcher-Munson compatibility enters calibrated
Auto mode without enabling hidden normalization or AutoGain.
The ISO bank is now jointly fitted and error-bounded, reference preservation is
the default explicit level policy, filter/mode updates crossfade, Auto requires a
measured SPL calibration, and coefficient preparation never runs in the callback.
AutoGain has one typed runtime state, manual/ISO headroom uses the realized
Nyquist-wide cascade, reset is in-place, frequencies are rate-safe, standalone
processing does not alter FP mode, and legacy migration/docs/QA/benchmarks cover
the complete contract.

Verification evidence for the closure includes malformed direct updates, exact
buffer/rate checks, 16–192 kHz and 32-channel processing, 20–90 phon response
error, 1 kHz preservation, normalized cuts/overlap, callback preparation guards,
transition discontinuity bounds, canonical AutoGain transitions, Auto calibration,
Pre/Post output metering, reset/fresh equivalence, and allocation-counted process
and reset paths.

Remediated in version 0.5.4:

- Headroom demand now considers positive cascade response only, so all-cut configurations are not attenuated twice. The identical ISO scan is also computed once per rebuild rather than per channel.
- The factory constructor now rejects nonempty legacy `channel_params` instead of silently discarding serialized per-channel curves, and validates channel count, finite/ranged fields, mode, and AutoGain position.
- Initialization rejects zero or sample-rate-unsafe designs and propagates the active rate into AutoGain. Reset clears AutoGain and compensation smoother temporal state.
- Pre AutoGain measures the final post-EQ output, and processing validates exact interleaved length and context sample rate before touching the buffer.

The follow-ups previously deferred from 0.5.4 are included in the 0.5.5 closure above.

## Findings

### P1 — Headroom compensation treats cuts as boosts and doubles their attenuation

The ISO/Auto peak scan takes `combined_db.abs()` and the manual estimator takes the absolute value of every requested gain (`loudness_compensation_plugin.rs:402-443`). A response whose largest excursion is a -12 dB cut therefore receives another -12 dB of broadband attenuation, producing as much as -24 dB at that frequency and -12 dB even where the EQ is nominally flat. This is not peak protection: negative gain cannot create positive headroom demand.

Track only positive cascade gain, `max(0, response_db)`. Use the actual combined response for both manual and ISO banks. Add tests for all-negative settings (zero broadband attenuation), mixed cuts/boosts (only the positive maximum controls attenuation), and ISO curves whose largest absolute excursion is a cut.

### P1 — The serialized per-channel EQ configuration is silently discarded

`LoudnessCompensationPluginParams` publicly serializes `channel_params` (`types.rs:20-59`), and each entry carries low/high/mid frequency, gain, and Q (`channel_loudness_params.rs:10-39`). `from_params` never reads that vector and builds one global filter specification for every channel (`loudness_compensation_plugin.rs:447-495`); the runtime schema exposes no per-channel controls either (`params.rs:24-135`). A successfully loaded preset can therefore sound different from what it stores without any error.

Either implement per-channel banks and dynamic parameters, or remove/version-migrate the dead field and reject nonempty legacy values instead of ignoring them. Add stereo and high-channel factory round trips with deliberately different channel curves and verify both parameters and frequency response.

### P1 — Factory construction bypasses the canonical parameter validation

Both the workspace factory and plugin bridge deserialize JSON and call `from_params` directly (`src/factory/create.rs:250-266`; `plugins-bridge/src/factory.rs:126-145`). That constructor directly assigns frequencies, gains, Q, mode, levels, AutoGain limits, and smoothing (`loudness_compensation_plugin.rs:447-485`) rather than validating against `PARAMS`. It also maps every unknown AutoGain position to Post (`auto_gain_position.rs:26-32`). Consequently NaN/Inf, out-of-range or above-Nyquist frequencies, invalid Q, `mode > 2`, and invalid AutoGain values can enter coefficient and monitor construction even though public runtime setters have a schema validator.

Create one fallible typed validator used by every factory, compatibility converter, bridge, and runtime update. Check finiteness, schema ranges/choices, cross-field constraints, channel count, nonzero supported sample rate, and frequency below a safe fraction of Nyquist. Reject unknown position strings. Add malformed JSON and direct-construction tests for every boundary, NaN/Inf, low sample rates, and invalid mode/position.

### P1 — `initialize` leaves an enabled AutoGain running at the constructor's 48 kHz rate

`from_params` creates AutoGain using the plugin's initial 48 kHz sample rate (`loudness_compensation_plugin.rs:475-485`). Later, `initialize` changes the plugin and filter rate but never calls `AutoGain::set_sample_rate` (`loudness_compensation_plugin.rs:771-778`). The helper's rate setter reconstructs both EBU R128 monitors and recalculates its smoother and attack/release coefficients (`sotf-host/src/auto_gain.rs:127-136`), so a plugin initialized at 44.1, 96, or 192 kHz has incorrect loudness windows and time constants.

Propagate initialization to AutoGain and handle failure before publishing the plugin. Add rate-equivalence tests that compare convergence time and reported LUFS at 44.1/48/96/192 kHz, plus reinitialization tests.

### P1 — Pre AutoGain reports and controls the pre-EQ signal, not the plugin output

In Pre mode the plugin measures input, applies AutoGain, then immediately measures “output” before the EQ filters and compensation gain run (`loudness_compensation_plugin.rs:863-886`). The comment claiming the correct output level is reported is therefore false. AutoGain's target is derived from the difference between these two measurements (`sotf-host/src/auto_gain.rs:204-228`), so this arrangement is driven mostly toward undoing its own pre-gain and ignores the spectral/headroom transformation whose loudness it is meant to match. Post mode correctly measures the final buffer (`loudness_compensation_plugin.rs:888-914`).

Keep the gain application before EQ if that topology is desired, but measure the final post-EQ buffer for feedback/reporting; expose a separate pre-EQ meter only if useful. Add strong shelf/ISO tests asserting reported output LUFS matches the returned buffer and that Pre/Post converge to their documented objective.

### P1 — Auto mode performs expensive coefficient design and response scans in the audio callback

Every Auto-mode block calls `maybe_rebuild_auto_filters` (`loudness_compensation_plugin.rs:844-856`). A volume change of at least 0.5 dB rebuilds 29 ISO values, updates seven biquads for every channel, and invokes a 128-frequency-by-seven-filter response scan once per channel (`loudness_compensation_plugin.rs:520-535,346-388,402-443`). This currently reuses allocated vectors, as the allocation tests demonstrate, but zero allocation does not make transcendental-heavy, volume-step-dependent work realtime deterministic. The setter also rebuilds immediately (`loudness_compensation_plugin.rs:726-733`), making the process-side fallback redundant in normal synchronized use.

Prepare the next coefficients and headroom target on a control/manager thread, then atomically swap or crossfade preallocated banks. If callback-side tracking remains necessary, bound and amortize it explicitly. Benchmark worst-case 16/32-channel blocks with a 0.5 dB step every callback and record p99/max deadline time, not only average throughput.

### P1 — Direct processing can panic or measure/process different sample regions

`process_in_place` trusts `context.num_frames` and indexes `frame * num_channels + ch` without checking `buffer.len()` (`loudness_compensation_plugin.rs:844-928`). A short buffer panics. A long buffer is passed in full to the loudness monitors and denormal flusher while filtering and AutoGain application cover only `num_frames * channels`, so measurement, returned audio, and reported frame count describe different regions. The in-place adapter forwards the slice unchanged and adds no contract validation (`sotf-host/src/parametric_in_place_plugin.rs:264-277`).

Use checked multiplication and require the exact interleaved sample count (or consistently slice to the required prefix and document surplus handling). Validate context sample rate as well. Test short, long, non-channel-aligned, zero-frame, huge/overflow-shaped contexts in Disabled, Pre, and Post modes.

### P2 — Seven sampled filter gains are not a fit to the ISO 226 target curve

The implementation calls its ISO bank a “fit,” but simply samples the 29-point desired delta at seven fixed centers and assigns those values independently to overlapping shelves/peaks (`loudness_compensation_plugin.rs:346-385`). Cascade interaction means the realized response is not the sampled target and can create ripple between centers. No test compares the final cascade response against the full desired curve; ISO tests validate only the table formula, direction, 1 kHz normalization, and interpolation (`iso226.rs:137-248`). The 10 kHz high shelf also extends the clamped 12.5 kHz table value indefinitely (`iso226.rs:109-134`).

Jointly optimize band gains—then frequencies/Q if needed—against all 29 targets with a weighted least-squares or minimax objective and stability constraints. Publish max/RMS error by band and phon pair, including 20–90 phon extremes and an explicit 12.5 kHz-to-Nyquist policy. If seven biquads cannot meet the tolerance, choose a better basis or increase the bank deliberately.

### P2 — Built-in headroom normalization destroys the advertised 1 kHz reference

`compute_iso226_delta` deliberately normalizes the target to 0 dB at 1 kHz (`iso226.rs:59-106`), but `process_sample` then applies a global attenuation equal to the largest estimated boost (`loudness_compensation_plugin.rs:402-443,497-514`). Thus the realized 1 kHz gain becomes negative by the maximum boost. The QA explicitly expects +6 dB bass plus -6 dB compensation to yield unchanged bass and a -6 dB midrange (`qa_loudness_compensation.rs:42-62`). That is a headroom-normalized contour, not equal-loudness compensation at a fixed 1 kHz SPL reference, and optional AutoGain adds a second level-control loop.

Choose and document one objective: preserve the phon/1 kHz reference and require upstream headroom/limiting, or expose an explicit “headroom-normalized” policy whose level shift is visible to the user. Validate achieved absolute and relative transfer functions and perform level-matched listening tests without letting AutoGain conceal contour error.

### P2 — Instant coefficient and mode changes are not guaranteed click-free

Manual and ISO rebuilds replace coefficients in live filters while preserving delay state and label this click-free (`loudness_compensation_plugin.rs:251-301,346-370`). Preserving state avoids clearing a tail but does not guarantee output continuity when the difference equation changes instantly. Switching mode jumps immediately between two independent state banks (`loudness_compensation_plugin.rs:497-514,710-725`), while Auto volume steps replace all seven sections at once. Current click coverage permits a comparatively large discontinuity and does not exercise all modes, signal phases, or block partitions.

Interpolate a proven-stable parameterization or crossfade old/new preallocated banks, including mode changes. Test DC, sine at several phases/frequencies, noise, and ringing tails; quantify first-difference/click energy across parameter, mode, and volume steps with randomized callback boundaries.

### P2 — Auto mode assumes an acoustic calibration that the plugin does not possess

Auto estimates listening level as `reference_level_db + playback_volume_db`, then clamps it to 20–90 phon (`loudness_compensation_plugin.rs:516-535`). This assumes engine volume 0 dB produces exactly the configured SPL at the listener and ignores DAC/OS/amp gain, speaker sensitivity, distance, room field, and headphone transfer. ISO 226 is a population-average equal-loudness relationship under specified presentation conditions; digital dBFS or a volume scalar alone does not establish dB SPL, and phon compensation is not programme loudness/LUFS.

Require a calibrated SPL-at-known-volume reference and state the presentation model (free/diffuse field or headphone transfer) before enabling Auto. Separate this calibration from LUFS AutoGain. Validate against published ISO points, calibrated playback measurements, listener populations, and controlled level-matched listening tests; otherwise label Auto as an uncalibrated heuristic.

### P2 — Reset is not equivalent to a fresh instance and may reconstruct vectors

`reset` clears and repopulates both filter vectors but does not reset the compensation smoothers, AutoGain monitors/gain, realtime cache, or Auto-mode volume sentinel (`loudness_compensation_plugin.rs:780-842`). The shared helper has a dedicated reset that clears both monitors and gain state (`sotf-host/src/auto_gain.rs:139-152`) but it is never called. Vector capacity happens to be retained in the normal case, yet reset's realtime allocation safety is untested and depends on prior capacity.

Reset every temporal state and cached report explicitly, use in-place biquad state reset or guaranteed preallocated banks, and define whether targets return to current configuration immediately. Compare reset output/data state with a fresh instance in every mode and AutoGain position, including mid-smoothing and after sample-rate changes, under an allocation counter.

### P2 — Legacy Fletcher–Munson conversion silently ignores `enabled` and `smoothing_ms`

The compatibility struct accepts `enabled`, `auto_gain_enabled`, and `smoothing_ms` (`fletcher_munson_compat.rs:5-21`), but conversion copies only playback volume, translated reference level, and AutoGain enabled (`fletcher_munson_compat.rs:23-35`). A disabled legacy instance is always converted to active Auto mode, and saved smoothing is lost. The legacy default smoothing is 0 ms, which is outside the new 1–1000 ms schema, further showing that no explicit migration contract exists.

Define exact legacy semantics and versioned migration: preserve bypass/enabled through the host's canonical bypass field, translate or reject smoothing intentionally, and test every legacy field through both factories, bridge/FFI paths, save/reload, and audio output.

### P2 — AutoGain enable and position have divergent schemas and state transitions

The factory params contain both `auto_gain_enabled` and a position string (`types.rs:40-48`), while the canonical `PARAMS` and serializable UI `Params` expose only the boolean/max/smoothing controls (`params.rs:65-94,230-255`). Runtime cached parameters add a string position anyway (`loudness_compensation_plugin.rs:585-590`). Toggling `auto_gain_enabled` creates/drops the helper but does not update `auto_gain_position` (`loudness_compensation_plugin.rs:649-667`), so enabling an instance constructed as Disabled can leave a live helper that the processing match never uses. Setting the position follows a separate path and unknown strings become Post (`loudness_compensation_plugin.rs:688-709`; `auto_gain_position.rs:26-32`).

Use one typed source of truth—prefer a three-state position choice—and derive enabled from it. Include it in the canonical schema, UI serialization, cached/current values, and compatibility migration. Test every transition, invalid string, factory round trip, and whether helper existence matches the active processing branch.

### P2 — Manual peak estimation is inaccurate and ISO scanning repeats identical work per channel

Manual headroom uses the maximum individual requested gain, not the actual five-filter cascade (`loudness_compensation_plugin.rs:432-440`), so overlap can underestimate constructive peaks or overattenuate separated bands. Conversely, ISO filters have identical coefficients on every channel, but the full 128-by-seven response scan is repeated inside the channel loop while always reading channel zero (`loudness_compensation_plugin.rs:402-430`).

Evaluate each active cascade once over a sample-rate-aware grid, retain only its positive peak, then set all channel smoother targets. Cache the result until coefficients change. Add adversarial overlap/Q cases and a channel-scaling benchmark that isolates coefficient-update cost.

### P2 — Frequency policies are not sample-rate safe and the headroom grid stops at 20 kHz

The public high-frequency range reaches 20 kHz regardless of sample rate (`params.rs:31-40`), filter construction passes it directly to Biquad with the current rate (`loudness_compensation_plugin.rs:251-340`), and ISO uses fixed centers through 10 kHz (`loudness_compensation_plugin.rs:356-385`). At low rates these values approach or exceed Nyquist. At 96/192 kHz, meanwhile, the purported clipping-protection scan ends at 20 kHz (`loudness_compensation_plugin.rs:391-430`) although a high shelf remains active above it; ultrasonic content can exceed the estimated headroom.

Constrain every designed frequency to a documented safe fraction of Nyquist. If the goal is numerical clipping prevention, scan through Nyquist; if it is audible-band normalization, state that it does not guarantee full-band headroom. Add 16/22.05/32/44.1/48/96/192 kHz stability, finite-output, and swept-response tests.

### P2 — Processing permanently changes the host thread's floating-point mode

Every callback invokes `enable_ftz_daz` and never restores the previous control state (`loudness_compensation_plugin.rs:844-850`). AutoGain's compensation path does the same (`sotf-host/src/auto_gain.rs:304-318`). A plugin should not silently alter floating-point semantics for later plugins or host code on the shared audio thread.

Configure FTZ/DAZ once at the audio-thread owner with an explicit host contract, or use a scoped guard that restores the prior state. Add a test that captures the control word before/after standalone processing on supported architectures.

### P2 — Documentation and verification describe only the old manual plugin

`USAGE.md` calls this a simple fixed shelf alternative to Fletcher–Munson and omits Manual/ISO/Auto modes, the mid band, position semantics, calibration, and realized ISO error (`USAGE.md:1-133`). It says cascades produce approximately twice the requested gain even though the implementation splits gain in half across the two shelves (`loudness_compensation_plugin.rs:255-260`), and lists a 10 kHz high-frequency default while the schema uses 8 kHz (`USAGE.md:15-20`; `params.rs:31-40`). The QA is mono/manual-only and intentionally expects bass boost to be cancelled by global attenuation (`qa_loudness_compensation.rs:14-66`). The dedicated benchmark covers manual mode only; workspace allocation coverage proves steady-state zero allocation but does not step Auto volume or exercise AutoGain.

Rewrite docs from the canonical schema and document actual transfer functions, headroom policy, calibration assumptions, and mode/position flows. Expand QA to ISO/Auto, stereo/high-channel, calibrated response error, transitions, reset, malformed buffers, rate changes, and final LUFS reporting. Benchmark every mode/position with and without control changes over common block sizes and channel counts, reporting tail latency.

## Algorithm assessment

The ISO 226:2003 table and equations are recognizable and internally well covered: the implementation normalizes at 1 kHz, has the correct low-level bass direction, interpolates in log frequency, and returns a transparent curve at equal phon levels. Manual mode's two half-gain shelf cascades and optional mid peak are a pragmatic tone-control basis. The quality gap lies in application: an uncalibrated digital-volume-to-SPL assumption, an unfitted seven-filter approximation, and mandatory global normalization mean the emitted signal is not demonstrably the requested equal-loudness contour at the stated acoustic level. Treat the standard calculation, filter fitting, acoustic calibration, and headroom policy as four separate contracts and validate each independently.

## Real-time allocation and performance assessment

Steady-state processing is allocation-free in the tested manual and Auto configurations. Filter vectors, smoothers, meter data, and realtime cache are preallocated; filter state is updated in place; and normal sample processing uses scalar fixed-size biquad cascades. The dominant realtime risk is not steady allocation but callback-side ISO redesign/response scanning on volume steps, plus monitor work on every AutoGain block. Other opportunities are computing the identical peak scan once rather than per channel, caching it until coefficients change, block/SIMD biquad processing where measured beneficial, and avoiding redundant FTZ/DAZ setup. Existing average-throughput QA is not a deadline/tail-latency test.

## Scope reviewed

Read in full: plugin `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, `UI.md`, `USAGE.md`; `benches/loudness-benchmark.rs`; `bin/qa_loudness_compensation.rs`; every source file (`iso226.rs`, `params.rs`, `lib.rs`, and all files under `src/lib/`), including all 1,551 lines of inline tests; and both integration suites (`tests/integration.rs`, `tests/test_loudness_compensation.rs`). Relevant active integration reviewed includes workspace catalog/factory registration, Fletcher–Munson alias conversion, plugin bridge, NIH/FFI parameter mapping, parametric adapters/validation, shared AutoGain and loudness monitor behavior, all-plugin benchmarks, fuzz registration, high-channel/DSP/parameter matrices, and realtime-allocation tests. No production code was changed.

## Strengths

- ISO 226 formula/table tests cover 1 kHz reference behavior, low-frequency direction, same-level transparency, interpolation, and endpoint clamping.
- Both filter banks are preallocated and update coefficients without replacing their vectors during ordinary parameter changes.
- Compensation gain uses a per-sample smoother, and AutoGain has explicit attack/release behavior and preallocated monitor data.
- Steady-state manual and Auto paths pass allocation-counting tests; plugin tests cover public factory/parameter behavior, multiple modes, high channel counts, reset basics, and many finite-output cases.
- The plugin reports zero algorithmic latency and avoids audio-buffer scratch allocation in its normal process path.

## Verification

- `rtk cargo test -p sotf-plugin-loudness-compensation` — 85 tests passed across four suites.
- `rtk cargo test -p sotf-plugins --test realtime_allocation_tests loudness_compensation_zero_alloc` — 1 passed, 45 filtered out.
- `rtk cargo test -p sotf-plugins --test realtime_allocation_tests fletcher_munson_zero_alloc` — 1 passed, 45 filtered out.
- `rtk cargo run -p sotf-plugin-loudness-compensation --features qa --bin qa-loudness-compensation` — all QA checks passed; reported 0.04% average CPU for its mono/manual 5-second benchmark.
