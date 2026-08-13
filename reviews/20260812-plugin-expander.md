# Expander code review — 2026-08-12

## Remediation status

All P1-P3 findings are remediated and regression-tested in 0.5.26:

- True broadband factory identity/schema/transfer:
  `single_band_identity_schema_and_broadband_path_are_real` and the universal
  factory contract test.
- Live cache publication in both modes: `test_cache_snapshot_updates_after_processing`
  and `spectral_mode_publishes_live_analyzer_snapshots`.
- Structural band/mode controls reject realtime setters and carry Structural
  schema metadata; focused unit/integration tests cover both directions.
- Oversized spectral calls are allocation-free chunks; realtime process and
  parameter allocation tests cover steady and irregular block sizes.
- Bypassed/passive bands use common lookahead:
  `bypassed_band_obeys_common_lookahead_latency` plus reconstruction tests.
- Reset clears crossover, detector HPF, lookahead, smoothers, meters, cadence,
  and spectral state; reset-vs-fresh and impulse-to-silence tests cover both modes.
- Sidechain HPF is active in detector DSP:
  `sidechain_hpf_rejects_low_frequency_trigger` covers 0 and 500 Hz behavior.
- Detection and processing modes are case-insensitive but unknown strings are
  rejected by `test_parse_detection_mode` and `malformed_presets_are_rejected_fallibly`.
- Spectral feature gaps are explicit validation errors rather than ignored
  controls (`spectral_mode_rejects_controls_without_feature_parity`). Its dual
  Hann, 75%-overlap, `N/4` hop, `1/(1.5N)`, `N`-latency contract is documented
  and covered by COLA, impulse latency, partition, amplitude, and null tests.
- Abrupt topology switches are removed from realtime automation. Crossover
  smoothing is per-sample and partition invariant
  (`crossover_automation_is_callback_partition_invariant`).
- Meter cadence is sample-based (`cache_cadence_depends_on_samples_not_callback_count`).
- Fallible construction validates every authoritative global and per-band range,
  finite value, band count, crossover ordering/Nyquist, channel count, and enum
  before DSP construction (`malformed_presets_are_rejected_fallibly` and
  `malformed_band_overrides_and_band_counts_are_rejected_fallibly`).
- Audio invariants now include broadband two-tone unity, reconstruction/null,
  latency impulse, detector rejection, reset equivalence, stereo preservation,
  cadence/partition equivalence, and non-finite recovery—not finiteness alone.

## Findings

### P1 — The catalog's single-band Expander is actually the default multiband processor

`ExpanderPlugin` aliases `MultibandExpanderPlugin` (`crates/sotf-plugins/src/lib.rs:309`), and factory creation does not force a one-band mode (`crates/sotf-plugins/src/factory/create.rs:101-106`). The catalog presents `SINGLE_BAND_LAYOUT` (`catalog.rs:480-496`), while the implementation clamps to at least two bands and reports “Multiband Expander.” Band splitting changes detector levels and therefore the expansion curve; this is not equivalent to broadband expansion.

Create an explicit one-band adapter/implementation and verify its identity, schema, and broadband transfer at the factory boundary. A two-tone regression should demonstrate that `"expander"` makes one detector decision for the combined signal, unlike `"multiband_expander"`.

### P1 — Analyzer snapshots are permanently frozen

`MultibandExpanderData` contains nested `Arc<Vec<_>>` fields (`multiband_expander_data.rs:6-9`). `RealTimeCache` clones the initial data into two outer snapshots, so the inner Arcs are shared and every `Arc::get_mut` in `update` (`:34-53`) fails. Attenuation, gate state, levels, and crossover meters never change through `get_data()`.

Use plain vectors inside the outer cache snapshot or deep-copy inner vectors for spare storage. Add assertions on changed values after processing; present tests check shape, not live cache contents.

### P1 — Structural parameter changes allocate on the audio path and can exceed stale buffers

Changing `num_bands` grows numerous vectors and FFT metadata (`multiband_expander_plugin.rs:920-984`) and rebuilds the parameter schema. Time-domain `band_buffers` remains sized for the band count present at `initialize()` (`:1336`), so a later increase can make the release build index beyond storage for sufficiently large blocks. Changing `processing_mode` creates or drops the complete FFT state in the setter (`:880-914`), causing large allocation/planning/destruction work at an automation boundary.

Treat band count and processing mode as structural: build a replacement off-thread and swap at a safe graph boundary, or prebuild both maximum-sized states. Test all count/mode transitions at 4,096 frames in release and assert zero allocations during the host setter/process sequence.

### P1 — Spectral processing allocates when a block exceeds the preallocated dry buffer

`process_spectral_in_place` calls `dry_buffer.resize(buffer.len(), 0.0)` (`multiband_expander_plugin.rs:752-755`). Unlike the time-domain path, it does not chunk blocks over 4,096. This directly allocates in realtime and contradicts the crate's zero-allocation/catalog evidence.

Preallocate to a negotiated maximum or chunk spectral input without altering stream scheduling. Add counting-allocator tests at 4,097, 8,192, and irregular block sizes after warmup.

### P1 — Lookahead misaligns bypassed/passive bands from active bands

Time-domain lookahead is applied only inside the active dynamics branch. Bypassed and passive bands return early and remain undelayed (`multiband_expander_plugin.rs:1475-1490`), while active bands and dry mix are delayed. Recombining those paths creates time/phase cancellation around crossovers and violates bypass expectations.

Delay every band equally whenever lookahead is active, irrespective of dynamics bypass, and reserve bypass for gain computation only. Test an impulse and broadband null with mixed active/bypassed bands at multiple lookaheads.

### P1 — Reset leaves crossover filter and control-smoother state alive

`reset()` clears expander, detector, lookahead, buffers, makeup, and spectral state (`multiband_expander_plugin.rs:1364-1390`) but never resets `crossover_points`, `threshold_smoother`, `mix_smoother`, `xover_smoothers`, cache counters, or monitoring state. After a transport reset, old IIR tails can leak into silence and automation ramps resume from pre-reset values. Compressor reset does clear crossovers, so the sibling contracts are inconsistent.

Define reset semantics and clear all signal-history state deterministically. Test impulse→reset→silence for exact/near-zero output in both modes, then compare the first post-reset block with a freshly initialized instance.

### P1 — Sidechain HPF is exposed, writable, serialized, and never used

`sidechain_hpf_hz` is part of the single-band schema and runtime cached parameters (`multiband_expander_plugin.rs:342-350`), setters/getters accept it (`:1049-1055, 1204-1206`), but the DSP never reads the field after construction. Users can automate a control with no audible effect.

Implement pre-detector HPF with per-channel state and reset/sample-rate handling, or remove it from every public surface with preset migration. Add low-frequency trigger/rejection tests at 0, 80, and 500 Hz settings.

### P1 — JSON detection mode is case-sensitive in a way the public schema does not describe

The public choices/defaults are `"Peak"` and `"RMS"`, but `parse_detection_mode` recognizes only lowercase `"rms"` (`misc.rs:7-11`). A factory JSON preset using the displayed `"RMS"` silently selects peak detection. Runtime choice automation happens to normalize to lowercase, so preset and UI paths disagree.

Deserialize to a typed, case-insensitive enum and reject unknown strings. Test all serialized/display spellings and round-trip through factory creation.

### P2 — Spectral mode silently drops much of the advertised Expander behavior

The spectral path ignores RMS/link-channel detection, sidechain HPF, time-domain lookahead, measured/heuristic auto makeup, the global threshold smoother, and level/attenuation cache updates. Switching modes therefore changes more than time/frequency resolution. This is acknowledged as deferred in the changelog but remains exposed through the same plugin schema.

Either implement feature parity or make unsupported controls unavailable/disabled in spectral mode with an explicit state migration. Add paired mode tests for every common parameter and document intentional differences.

### P2 — Spectral window/latency documentation contradicts current code and changelog

`SpectralState` uses a Hann analysis+synthesis pair with `hop_size = fft_size / 4`, scale `1/(1.5N)`, one-window startup padding, and one-window dry delay (`spectral_state.rs:104-163`). This 75%-overlap Hann-squared arrangement is COLA, and the unit test confirms it. The current changelog instead claims a change to 50% overlap and `fft_size-hop_size = 512` latency, while source/tests report 1,024 samples. Comments in the state file also disagree about overlap and dry-delay sizing.

Choose the measured streaming contract as authoritative, correct all docs/comments, and add amplitude/null and impulse-latency tests for every supported block partition. If changing hop size, derive normalization and latency together rather than editing constants independently.

### P2 — Spectral mode changes are abrupt and can emit discontinuities

Switching processing mode immediately replaces one state with another; changing crossovers instantly remaps bins, while time-domain crossovers move only once per block. No crossfade spans topology/mode transitions. Threshold, ratio, range, knee, hysteresis, and band overrides are unsmoothed in the spectral path.

Crossfade old/new processing over a bounded interval and update bin ownership with interpolation or dual processing. Add maximum-jump and energy-continuity tests on steady tones/noise.

### P2 — Crossover automation is block-size dependent

The time-domain path advances each `LogSmoother` by the whole block and installs one coefficient set (`multiband_expander_plugin.rs:1433-1436`). A render split into 64-frame blocks follows a different coefficient trajectory from a 1,024-frame block. Existing automation partition tests cover only threshold and mix.

Interpolate coefficients per sample/small fixed quantum, or crossfade filters independently of host block size. Extend partition-equivalence tests to every crossover.

### P2 — UI cache cadence depends on host block size

The time-domain cache updates every ten calls (`multiband_expander_plugin.rs:1666-1684`), not by elapsed samples. UI rate ranges widely across device/offline block sizes, and spectral mode never updates it.

Use a sample/time counter as the compressor does and populate equivalent spectral metrics. Test cadence across 32–4,096-frame blocks.

### P2 — Factory presets are not fully validated before DSP construction

`with_params` accepts out-of-range/non-finite global values, unsorted or non-finite crossover frequencies, arbitrary processing-mode strings, and invalid timing/range values. Factory deserialization does not run runtime schema validation. These can generate invalid coefficients or silently select a different mode.

Make `from_params` fallible, validate authoritative specs, enforce finite ascending crossovers below Nyquist, and reject unknown enums. Add malformed-preset tests.

### P3 — Tests emphasize finiteness instead of audio invariants

Many branch tests assert only “finite” output. Unity reconstruction permits 0.7–1.3 RMS, dry-only permits 0.5–1.5, and measured-makeup “no jitter” checks only for NaN. These bounds cannot detect severe coloration, gain errors, pumping, or channel instability.

Add offline reference comparisons, null depth, swept response, attack/release timing, gate-state transition timing, stereo-image preservation, and measured-makeup modulation metrics. Cover silence, DC, impulse, steps, tones at/between bin centers, and non-finite inputs.

## Realtime and performance assessment

Fixed-structure time-domain processing chunks oversized calls and is allocation-free in its steady loop. Spectral steady state reuses FFT plans/buffers for blocks within existing dry capacity. The primary realtime failures are mode/band structural setters, spectral dry-buffer growth, dynamic parameter-schema reconstruction, detector mode resizing, and cache design. Per-bin log/pow and fourfold overlap are expensive but should be optimized only after correctness and profiling; bin envelopes can then be vectorized or grouped by band.

## Coverage reviewed

Reviewed all crate files: instructions, README/changelog/UI/usage, manifest, QA binary, every source module including STFT state, all unit/integration/multiband tests, and factory alias/catalog/create wiring. Relevant host/shared code reviewed includes adapters, parameter bridge/validation, `RealTimeCache`, `LookaheadBuffer`, `LevelDetector`, `MeasuredMakeup`, LR4 crossover, smoothers, FFT wrapper use, latency metadata, reset and caller contracts. No code was changed and no broad workspace build was run.
