# Unreleased

## 0.5.8 acoustic-quality measurement program

- Add a deterministic offline quality runner spanning Small–Cathedral presets,
  5.1/9.1.6 layouts, 44.1/48 kHz, and 64/257/1024-frame partitions.
- Add regression-tested measurements for octave-band Schroeder RT60, normalized
  echo density/mixing time, inter-channel coherence, energy entropy/vector and
  diffuseness, LFE magnitude/phase, modulation sidebands, THD/IMD, exact linked
  limiter gain, detector precision/recall, and gain pumping.
- Expose read-only dialogue and limiter telemetry through `AaeData`; processing
  behavior and callback allocation remain unchanged.
- Document a reproducible external corpus/listening protocol and explicitly keep
  it separate from deterministic synthetic acceptance.

## 0.5.7 complete 2026-08-12 review remediation

- Make every public and bridge construction path fallible and validate all
  restored values, supported choices, and mutually exclusive solo modes.
- Preserve the live channel contract by marking layout/preset metadata
  structural and rejecting live replacement.
- Add click-safe 10 ms dual-read transitions for pre-delay and room-size
  changes plus 5 ms RT60 coefficient interpolation.
- Replace the synthesized LFE one-pole with a fourth-order 120 Hz
  Linkwitz-Riley low-pass and exclude LFE from spatial VBAP rows.
- Convert ER/FDN routing to normalized sparse triplets, removing dense
  source-to-channel loops from the sample path.
- Interpret the FDN safety control as positive headroom above nominal full
  scale, keeping normal-level modeled decay linear while retaining an
  emergency soft guard.
- Stop mutating caller floating-point control state; hosts remain responsible
  for processing-thread FTZ policy.
- Activate the previously uncompiled split unit-test module and add
  quantitative LFE, correlation, dialogue/percussion, transition, routing,
  construction, and maximum-layout realtime QA coverage.
- Align package/plugin version, zero-latency catalog metadata, README, and
  parameter documentation.

## 0.5.6 review remediation

- Bypass now advances the complete reverb, content-aware detector, auto-gain,
  and limiter state instead of freezing it. The audible path uses a fixed 5 ms
  crossfade to metadata-identified FL/FR dry channels, avoiding clicks and stale
  tail resumption without realtime allocations.

## 0.5.5 review remediation

- Advance dry, early, late, and LFE level smoothing per sample so automation is
  invariant to host block partitioning.
- Add `try_from_params` validation for finite/ranged values, supported layouts
  and presets, and mutually exclusive solo modes; the canonical factory now
  uses the fallible path.
- Reject live speaker-layout and room-preset changes because they require host
  graph reconstruction and prepared DSP state.
- Preserve immutable FDN VBAP rows and apply envelopment/height weighting in
  preallocated storage, removing matrix construction and allocation from those
  ordinary automation setters.
- Report the crate version through `PluginInfo`.

- Layout opts into the spatial-spider visualiser via `VizSlot::Custom { name: viz_names::SPATIAL_SPIDER, position: VizPosition::FullCenter }`. The app-gpui layout renderer picks this up automatically and appends the spider panel below the main control row.
- `AaePlugin::process` now hoists per-frame input/output base indices in the
  scalar render loop, trimming repeated offset arithmetic while the larger
  SIMD/block-processing refactor remains deferred.
- `qa-aae` now excludes the low-passed LFE feed from the full-range channel
  energy balance assertion, while still checking that LFE energy remains finite.
- `qa-aae` now scopes the zero-allocation assertion to `process()` because
  `get_data()` returns freshly allocated UI meter data outside the audio callback.

# 0.5.3

- Content-aware dialogue ducking now uses windowed envelope evidence with a short hold, avoiding false ducking on quiet centered noise or steady mono-compatible music while staying active through sustained speech.
- LFE extraction now follows source-domain wet ER/FDN energy instead of signed routed speaker sums, so decorrelated channels cannot cancel the LFE send.
- The final rendered output now uses a linked safety limiter after auto-gain, preserving multichannel ratios while bounding summed dry, early, late, and LFE contributions.
- `ToneFilter`: clamp `a1` to `[-0.999, 0.999]` in both `new()` and `set_gains()`. Without the clamp, extreme gain ratios (e.g. `treble_ratio=0.2` + short RT60) place the feedback pole within ~0.001 of the unit circle, producing a reverberation tail that is effectively infinite (~millions of samples). Applies to both the `new` constructor and `set_gains` (fixes `src/tone_filter.rs`).
- `DelayLine`: fix `new(max_samples)` to allocate `max_samples + 2` slots internally so that `max_delay_samples()` returns exactly `max_samples`. Previously callers had to over-request by 1 sample to reach their target delay (fixes `src/delay_line.rs`).
- `EarlyReflections`: switch modulated tap reads from `read_linear` to `read_allpass`. Linear interpolation acts as a mild lowpass filter; allpass preserves the flat frequency response when delay lengths are time-varied. Each tap now maintains its own allpass state for continuity across calls (fixes `src/early_reflections.rs:100`).
- `EarlyReflections`: hard-code `MAX_PRESET_DELAY_MS = 154.5` to replace the O(presets × taps) scan inside `max_tap_delay_samples`. A `debug_assert` validates the constant against computed values during testing (fixes `src/early_reflections.rs`).
- `AaePlugin`: always pre-allocate `AutoGain` (even when `auto_gain_enabled=false`) in `from_params` and `initialize()`. This eliminates the heap allocation in `ensure_auto_gain()` that would otherwise occur on the audio thread the first time auto-gain is enabled via `set_parameter` (fixes `src/lib.rs`).
- `AaePlugin::process`: replace impossible runtime bounds checks (`tap_idx >= er_gains.len()`, `line_idx >= fdn_gains.len()`) with `debug_assert!` — these conditions are structurally impossible given construction invariants, but the checks were silently skipping taps in release builds (fixes `src/lib.rs`).
- `AaePlugin`: LFE source-domain extraction now uses unsigned energy magnitude instead of signed fallback to avoid low-frequency polarity flips on symmetric or cancelling feeds (`signed_rms` behavior).
- `AaePlugin`: `FDN::set_room_size` now updates delay lengths in place rather than resetting delay lines, preventing abrupt tail truncation and audible clicks on room-size changes.
- `AaePlugin`: VBAP routing matrices (`er_gains` and `fdn_gains`) were flattened from `Vec<Vec<f32>>` to a contiguous row-major `Vec<f32>` layout to improve cache locality.
- `AaePlugin`: `compute_lp_coeff` now uses the bilinear one-pole low-pass coefficient form (`(1 - sin(ω)) / cos(ω)`) with valid-range clamping.

**Deferred (require cross-crate changes or larger refactors):**
- Issue #10 (SIMD block processing): out of scope for a bug-fix release.

# 0.5.2

- ER delay storage now provisions max preset delay plus max modulation headroom, uses fixed tap/state arrays, and supports preset changes without reallocating.
- Delay reads are clamped to capacity so out-of-range modulation cannot panic.
- FDN delay lines allocate for max room size up front; room_size updates now adjust delay lengths in place.
- Process buffer sizes now return clean errors, AAE reports zero host latency, content-aware dialogue ducking is implemented, ER stale tap summing is fixed, routing gain buffers are reused, and cached parameter updates avoid rebuilding the whole parameter list on every set.

# 0.5.1

Bug fixes

# 0.5.0

Initial version
