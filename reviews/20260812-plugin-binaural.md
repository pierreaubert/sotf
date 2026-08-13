# Binaural audio plugin code review — 2026-08-12

## Remediation status — 0.5.23 complete

Fixed across `0.5.20` through `0.5.23`:

- VBAP uses the correct barycentric sign and unit-sum affine interpolation;
  exact vertex/edge/centroid and constant-field regressions cover both defects.
- The FDN is configured at the initialized engine rate and fully reset.
- Head updates dropped under backpressure are retried, and runtime SOFA changes
  stop/rebind the dataset worker; clearing restores deterministic fallback HRTFs.
- Compile metadata is conservative, generic database files rank last, and the
  previously disconnected HRTF resampling/VBAP tests now run.
- Startup output is gated for a fixed `fft_size` latency across callback sizes.
- HRTF filtering is causal hop-partition overlap-add, verified sample-for-sample
  against direct FIR convolution across irregular callbacks and IR lengths from
  one sample through the supported capacity. Oversized IRs are rejected.
- Reflections retain source ownership; silent configured channels add no paths.
  The implementation and documentation now describe their actual broadband
  HRTF-derived ILD behavior and no longer allocate unused spectral filters.
- Replaced HRTF states are handed to a bounded background reclaimer.
- Construction/runtime state, canonical SOFA naming, exact shared layouts,
  factory admission, and plugin version metadata are aligned and tested.
- Diffuse EQ rejects malformed/non-finite data and uses log-frequency smoothing,
  level-relative regularization, common-ear normalization, and bounded boost.
- SOFA spectra and diffuse EQ are preprocessed/reused for head updates; input
  processing operates on hop partitions instead of shifting an FFT frame.
- Runtime SOFA replacement is transactional and the rebound worker converges to
  the current target after backpressure or dataset changes.
- The remediated crossfade, head-tracking, HRTF-database, and anthropometric
  parameters are present as visible controls in the generated layout, with a
  regression that checks their documented control type and surface.
- Player and engine graph channel-width reconstruction preserve every Binaural
  setting while changing only `input_channels`; exhaustive regressions cover
  the complete settings variant, and the CLI now customizes the canonical
  default rather than maintaining a lossy duplicate constructor.

All P0-P3 findings are resolved; no remediation remains deferred.

Verification after remediation:

- `cargo test -p sotf-plugin-binaural --offline` — 105 passed across 5 suites.
- `cargo test -p sotf-plugins --offline binaural_catalog_and_factory_share_exact_layout_contract` — passed.
- `cargo test -p sotf-plugins --offline --test realtime_allocation_tests test_binaural_zero_alloc` — passed.
- `cargo check -p sotf-engine --offline` — passed.
- `cargo test -p sotf-engine binaural_reconstruction_after_external_preserves_every_non_channel_setting --lib --locked` — passed.
- `cargo test -p sotf-player binaural_channel_reconstruction_preserves_all_non_channel_settings --test plugin_chain_tests --locked` — passed.
- `cargo check -p sotf-player --locked` — passed.
- `cargo check -p app-cli --locked` — Binaural constructor compiles; the crate
  remains blocked by unrelated stale Loudness Compensation, EQ, AB Compare,
  and Downmix constructors.
- `cargo clippy -p sotf-plugin-binaural --all-targets --offline -- -W warnings` — no
  Binaural warnings (shared `sotf-host` warnings remain).

## Findings

### P1 — Fixed: VBAP interpolation computes the first barycentric coordinate with the wrong sign

`calculate_vbap_gains` forms `n = cross(v1-v0, v2-v0)`, then computes `w1` as `dot(cross(n, v2-v0), p-v0) / dot(n,n)` (`src/hrtf/misc.rs:48-70`). For the elementary triangle `v0=(0,0)`, `v1=(1,0)`, `v2=(0,1)`, this returns `w1=-p.x`; the correct expression is equivalently `dot(cross(v2-v0, n), p-v0) / dot(n,n)`. Valid interior targets are consequently classified outside the triangle and clamped, or are interpolated with the wrong measurement weights. This corrupts HRTF magnitude, phase, and ITD for normal speaker positions and head rotations.

Fix the sign (and derive both coordinates from one documented barycentric formulation). Add exact vertex, edge midpoint, centroid, and known interior-point tests before any normalization. The existing test only checks that an intentionally outside target is non-negative after clamping (`src/lib/tests/misc.rs:97-135`), so it cannot catch this defect.

### P1 — Fixed: energy-normalized VBAP gains are then incorrectly used as interpolation weights

Even after the sign is fixed, the in-triangle path normalizes weights to unit *energy*, not unit sum (`src/hrtf/misc.rs:107-121`). Those gains are used as affine weights for ITD and log-magnitude interpolation (`src/hrtf/interpolate.rs:26-68,131-155`). At a triangle centroid, `[1/3,1/3,1/3]` becomes approximately `[0.577,0.577,0.577]`, whose sum is 1.732. A constant -20 dB response therefore becomes about -34.6 dB rather than remaining -20 dB, and ITDs are scaled too. VBAP loudspeaker power normalization is not appropriate for interpolating samples of a transfer-function field.

Keep barycentric HRTF interpolation weights non-negative and normalized to sum one. If constant-power source gain is wanted, apply it after constructing the interpolated HRTF, with an explicit level policy. Add constant-field reproduction tests: identical HRTFs and ITDs at all three vertices must yield that identical HRTF/ITD at every interior target.

### P1 — Fixed: early reflections lose source/channel identity and spatialize the already-summed binaural signal

The image-source model correctly returns one reflection list per speaker (`src/room/calculate.rs:12-17,34-35,173-176`), but initialization flattens all non-LFE lists into one `cached_reflections` vector (`src/lib/binaural_decoder_plugin.rs:1455-1477`). Processing first sums every direct source through its HRTF, then writes that final stereo mixture into one stereo delay line and applies *every speaker's* reflection list to it (`src/lib/binaural_decoder_plugin.rs:825-858,1695-1705`). Thus a signal present only in the left-front input receives reflection paths calculated for center, right, surrounds, and heights as well; reflection energy also grows with configured speaker count even when those input channels are silent. Cross-ear propagation is absent because delayed left only feeds left and delayed right only feeds right.

Preserve reflection ownership and render each input/source contribution through its own delayed path before the final ear sum (or precompute a per-input, per-ear partitioned reflection IR). Add an impulse test per input channel that compares observed reflection delays/ear gains against only that channel's `calculate_reflections` result, plus a test that adding silent configured channels does not change output.

### P1 — Fixed: the advertised HRTF reflection renderer stores full filters but reduces them to broadband ILD gains

Initialization computes and stores full left/right frequency-domain filters for every reflection (`src/lib/binaural_decoder_plugin.rs:1587-1608`; `src/room/reflection_hrtf.rs:5-37`), but realtime rendering uses only `left_gain_broadband` and `right_gain_broadband` (`src/lib/binaural_decoder_plugin.rs:844-855`). It discards reflection HRTF phase, ITD, pinna spectral cues, and elevation cues. The full spectra remain allocated, potentially megabytes for many reflections, despite never being read. This contradicts the documented “individual HRTF per reflection” behavior.

Either convolve reflection paths with their per-ear filters (preferably folded into per-source partitioned IRs), or remove the unused spectra and describe the feature honestly as broadband ILD panning. Add spectral/ITD impulse-reference tests for reflections at left, right, above, and behind.

### P1 — Fixed: the FFT path performs circular, not linear, HRTF convolution

Each `N`-sample Hann-windowed frame and an IR truncated/padded to the same `N` are transformed at length `N` (`src/filter.rs:10-44`; `src/hrtf/interpolate.rs:102-128`), multiplied, inverse transformed, and the entire `N` samples are overlap-added (`src/lib/binaural_decoder_plugin.rs:518-558,739-815`). No `N + L - 1` zero padding, overlap-save discard, or FIR partitioning prevents time aliasing. IRs longer than `N` are silently truncated (`src/filter.rs:20-26`). Late HRTF energy wraps to the beginning of each frame, shifting ITD/tail energy and producing block-periodic artifacts; the same issue affects the LFE spectral filter.

Use a verified linear-convolution implementation (uniform partitioned convolution is the natural fit), or explicitly constrain `L` and use overlap-save with the correct discard region. Reject unsupported IR/FFT combinations rather than truncate silently. Compare streaming output against direct FIR convolution for impulses near every block boundary, multiple block sizes, IR lengths `1`, `<N`, `N`, and `>N`, and both ears.

### P1 — Fixed: processing latency depends on host block size while `latency_samples()` always reports `fft_size`

The plugin buffers until a complete `N`-sample analysis frame, then immediately drains the first hop into the current output slice (`src/lib/binaural_decoder_plugin.rs:1670-1729`). With callback blocks smaller than `N`, the first output appears after roughly `N-hop_size`; with a callback of `N` or larger, the implementation consumes future samples from the same callback and writes the corresponding frame at output index zero. Yet it always reports `N` (`src/lib/binaural_decoder_plugin.rs:1734-1736`), and the apparently intended `latency_filled` counter is written but never read (`src/lib/binaural_decoder_plugin.rs:280,822,1073`). Host delay compensation can therefore be wrong and render output noncausally within large blocks.

Choose a fixed causal streaming latency, gate draining until that latency is filled, and report exactly that value. Add impulse-delay tests for callback sizes `1`, `hop-1`, `hop`, `hop+1`, `N`, and `>N`, asserting identical absolute delay and identical sample output after alignment. The current latency test only checks the hard-coded return value (`tests/integration.rs:341-345`).

### P1 — Fixed: head-tracking requests can permanently stop short of the final target

The audio thread uses a capacity-one `sync_channel` and drops a request when `try_send` fails, but updates `last_hrtf_*` regardless (`src/lib/binaural_decoder_plugin.rs:1648-1667`). If the final smoothed angle is dropped while the worker is busy, subsequent blocks see no >0.5° difference and never retry it; the installed HRTF remains at an older angle. This directly contradicts the comment that a newer angle will be sent next frame.

Update `last_hrtf_*` only after a successful enqueue, or use an atomic latest-value mailbox plus generation counter so the worker always converges to the newest target. Test a deliberately blocked/slow worker with a burst ending at a known angle and assert the final installed generation/angle matches the target.

### P1 — Fixed: runtime SOFA changes do not maintain the head-tracking worker/state contract

Loading `hrtf_file` at runtime synchronously builds and stores a state (`src/lib/binaural_decoder_plugin.rs:1130-1209`) but does not start/restart the worker. If the plugin initialized without SOFA, no worker exists (`src/lib/binaural_decoder_plugin.rs:936-943,1623-1625`), so head angles remain ineffective after later loading a file. If a worker already exists, an in-flight recomputation can load the old SOFA state and subsequently overwrite the newly selected state (`src/lib/binaural_decoder_plugin.rs:951-968,1007-1064`). Clearing the path sets only `config.hrtf_path=None`; it neither restores default filters nor stops the worker, so audio and `get_parameter` disagree.

Make SOFA replacement one control-thread transaction: stop/version the worker, build the state off-thread, atomically install it, reset the angle generation, then start a worker bound to that dataset. Clearing must explicitly choose and install a documented fallback state. Test load-after-initialize, replace-during-recompute, clear-after-load, and head tracking after each transition.

### P1 — Fixed: late-reverb state is initialized for 44.1 kHz, not configured on initialize, and survives reset

The FDN is constructed at the hard-coded constructor rate of 44.1 kHz (`src/lib/binaural_decoder_plugin.rs:163,283-289`). `initialize(sr)` never calls `set_room_params`; that happens only when RT60 or damping is later changed (`src/lib/binaural_decoder_plugin.rs:1329-1352,1398-1437`). At 48/96 kHz the default delay lengths and feedback therefore represent the wrong times/RT60. `reset_state` clears STFT, reflection, and RTPGHI state but never calls `fdn.reset()` (`src/lib/binaural_decoder_plugin.rs:1067-1085`), so old reverb leaks across seeks/restarts.

Configure the FDN unconditionally in `initialize(sr)` from current parameters and reset it in `reset_state`. Add impulse RT60/delay tests at 44.1/48/96 kHz and a reset-then-silence test with late reverb enabled.

### P2 — Fixed: state replacement can deallocate large HRTF states on the realtime thread

The worker allocates a new state, `process_audio_block` clones old/new `Arc`s to crossfade, and the audio thread later sets `crossfade_prev_state=None` (`src/lib/binaural_decoder_plugin.rs:450-485,675-687`). If that is the last strong reference, dropping the nested HRTF/filter vectors occurs in the audio callback. Frequent head tracking makes this an intermittent unbounded deallocation hazard that the steady-state zero-allocation test cannot observe.

Retire old states through a control/background-thread reclamation queue or another RT-safe deferred-drop mechanism. Extend allocation/realtime tests across repeated state swaps and crossfade completion, measuring both allocation and deallocation on the callback thread.

### P2 — Fixed: plugin compile metadata claims linearity when late reverb can clamp

`compile_metadata` always returns `linear_transform` and allows input/output gain absorption (`src/lib/binaural_decoder_plugin.rs:1105-1113`). With late reverb enabled, the FDN is stateful and its dependency clamps outputs to ±4, making the path nonlinear; runtime HRTF crossfades and parameter smoothing also make transfer behavior time-varying. A compiled host plan may legally move gains across the plugin based on false metadata and change sound.

Return conservative boundary metadata whenever late reverb, externalization smoothing, or an HRTF transition is active; advertise a linear transform only for a stable direct convolution state. Add compiled-vs-regular path equivalence tests at high level with reverb/clamping and during transitions.

### P2 — Fixed: catalog/parameter channel contracts admit layouts the DSP silently maps to stereo

The catalog advertises `[1,2,4,6,8,12]` (`src/factory/catalog.rs:91,999-1016`) and the generated parameter accepts every integer 2–16 (`crates/sotf-plugin-binaural/src/params.rs:27-39`). The actual speaker lookup supports `1,2,3,5,6,8,10,12,14,16` (`sotf-host/src/speaker_config/get.rs:43-58`). For unsupported counts such as 4, `new` silently falls back to the two-channel layout, leaving channels 2+ absent from `main_channels` and therefore silent (`src/lib/binaural_decoder_plugin.rs:170-183`). Factory creation checks only that JSON count equals graph count (`src/factory/create.rs:329-339`).

Use one shared enumerated layout contract and reject unknown/ambiguous channel counts. If 4-channel input means a specific quad layout, define it explicitly. Add factory and per-channel impulse tests for every advertised width, including a failure test for unsupported counts.

### P2 — Fixed: HRTF database ranking lets an unlabelled generic file beat a measured near-match

Files with no parsed dimensions score `0.0`, while every non-exact measured candidate has a negative score (`src/hrtf_database.rs:143-156`). Sorting descending therefore selects any generic file ahead of a 0.1 cm near-match; an exact match ties generic and alphabetical path order decides (`src/hrtf_database.rs:76-93`). The test explicitly accepts this ordering rather than checking semantic best match.

Rank candidates by evidence class first (both dimensions, one dimension, generic fallback), then normalized distance within class, or assign generic a worst/fallback score. Test exact-vs-generic and near-match-vs-generic selection.

### P2 — Fixed: parameter/preset schemas are split and stale

Construction/factory deserializes `BinauralDecoderParams`, which contains SOFA/DSP/room fields but no crossfade or late-reverb fields (`src/config.rs:58-94`). Generated UI/preset state uses a different `params::Params` with crossfade/reverb but no SOFA path or construction-only DSP fields (`src/params.rs:125-202`). Runtime parameters expose both `sofa_file` from `PARAMS` and an appended `hrtf_file` alias. `UI.md:28-59` still documents removed optimization and obsolete indices, while `USAGE.md:11-16` advertises the removed optimization parameter and `USAGE.md:114` incorrectly says HRTF files load asynchronously (the setter performs synchronous file I/O and resampling).

Define an explicit versioned mapping between construction state, generated preset state, and aliases; choose one canonical SOFA key; migrate legacy keys. Update docs from live `PARAMS/LAYOUT`. Add a factory round-trip test containing every public parameter and a legacy-preset migration test that verifies DSP state, not only deserialization success.

### P2 — Fixed: diffuse-field EQ lacks empty/invalid-data guards and uses frequency-independent regularization

`compute_diffuse_field_eq` divides by `sofa.num_measurements` without verifying it is nonzero or counting only valid slices (`src/filter.rs:69-92`). It adds a fixed `0.001` power regularizer at every bin, then normalizes at a single 1 kHz bin (`src/filter.rs:94-125`). This makes the result depend on absolute SOFA gain and can over/under-regularize frequency regions, with up to +12 dB narrowband boost.

Reject empty/inconsistent datasets, count valid measurements, smooth the diffuse-field power spectrum perceptually, and use relative/frequency-dependent regularization or a bounded target curve. Add malformed-data tests and compare the equalized diffuse-field average against a declared tolerance over the audible band, including maximum boost and left/right balance.

### P3 — Fixed: reduce setup and head-update work through HRTF preprocessing/caching

Every speaker/head update transforms the same three source HRIRs anew; diffuse EQ transforms every measurement again (`src/hrtf/interpolate.rs:102-128`; `src/filter.rs:73-84`). `ir_to_freq` allocates two vectors and logs for every transform (`src/filter.rs:17-42`). This is off the audio thread after initialization, but it increases head-tracking lag and state-swap frequency pressure.

Precompute per-measurement spectra and onsets once per loaded/resampled SOFA, reuse FFT scratch, cache diffuse EQ, and interpolate into preallocated output state. Benchmark worker convergence time for realistic 7.1.4 data and rapid yaw/pitch motion, not only callback enqueue time.

### P3 — Fixed: replace input shifting and cache-unfriendly reflection traversal

Each hop shifts `N-hop` samples for every input channel with `copy_within` (`src/lib/binaural_decoder_plugin.rs:1687-1693`). Reflection rendering then performs one pseudo-random delay-line read per reflection per sample (`src/lib/binaural_decoder_plugin.rs:829-860`). Both become expensive at 12–16 channels or second-order rooms.

Use a circular per-channel analysis buffer and group/merge reflection taps by delay (or render precomputed partitioned room IRs). Bench realistic SOFA + first/second-order reflection configurations; current main benchmarks often use no SOFA and therefore do not measure representative HRTF/reflection cost.

## DSP and streaming contracts observed

- Input/output: interleaved `N`-channel input to interleaved stereo; output is overwritten for all requested frames, with zero fill during startup.
- Analysis: causal zero-padded hop partitions, FFT size `N`, hop `N/4`; forward FFT per active input channel and inverse FFT per ear plus LFE.
- Scaling: inverse output is scaled by `1/N`, matching the unnormalized inverse FFT used by the overlap-add convolution.
- HRTF state: background-created `ArcSwap<BinauralState>` with 50 ms hop-quantized crossfade; optional RTPGHI magnitude crossfade.
- Channels: only the exact shared layouts `[1,2,3,5,6,8,10,12,14,16]` are accepted; LFE is separated by `SpeakerConfig`; output is always two channels.
- Externalization: source-owned broadband HRTF-derived ILD reflection paths, followed by optional stereo FDN.
- Reset: convolution/output/reflection/RTPGHI buffers and FDN state are cleared.
- Latency: fixed and reported as `fft_size`, independent of callback size.
- Bypass: there is no plugin-local bypass. Host bypass rejects unequal input/output widths, so the N→2 decoder cannot use normal host pass-through bypass; the UI's toggle/solo affordance needs an explicit channel-aware policy rather than pretending transparent bypass is available.

## Scope reviewed

Read completely: nested `AGENTS.md`; `README.md`, `CHANGELOG.md`, `UI.md`, `USAGE.md`; `Cargo.toml` and features; all plugin source modules (`config`, `error`, `filter`, `hrtf/*`, `hrtf_database`, main plugin/types, `params`, `room/*`); all inline/unit/integration tests and fixtures; QA binary; full Criterion benchmark; workspace re-exports; factory create/catalog wiring; `Plugin`/compile metadata and speaker-layout contracts; realtime allocation caller coverage; `sofa-reader` position/nearest-neighbour contracts; and the directly used `math-dsp` FDN/RTPGHI implementations. No plugin source was skipped.

Original review verification: `cargo test -p sotf-plugin-binaural` — 86 passed across five suites before remediation. The complete post-remediation commands and results are recorded above.

## Test gaps and strengths

The suite has useful buffer-length validation, silence/finite smoke tests, SOFA resampling coverage, state-crossfade smoke coverage, layout/parameter invariants, second-order reflection deduplication, and a steady-state zero-allocation caller. The code also preplans FFTs, preallocates the steady-state spectral buffers, uses `try_send` rather than blocking in `process`, flushes denormals, and keeps SOFA recomputation off the callback.

The remediated suite adds the formerly missing acceptance oracles: exact direct-FIR comparisons, fixed-latency tests across callback sizes, constant-field interpolation, per-source reflections, final-target convergence under backpressure, transactional SOFA replacement, FDN reset/sample-rate coverage, background state reclamation, and conservative compile-metadata tests.
